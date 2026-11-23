// src/lib/conversionImagen.svelte.ts
//
// Qué mapas están convirtiendo ahora su imagen de fondo (Rust la pasa a WebP al importarla y tarda un par de segundos
// con una imagen grande). Solo es pantalla: la tarjeta del mapa y el tablero enseñan «Convirtiendo imagen…» mientras tanto.

class ConversionImagen {
  /** Ids de los mapas que cambian su imagen ahora mismo. */
  mapas = $state<number[]>([]);
  /** Cuántos mapas NUEVOS se están creando a partir de una imagen (todavía no tienen id). */
  nuevos = $state(0);

  /** Mapas que ya tienen su imagen nueva pero todavía no cargaron su miniatura: siguen enseñando «Convirtiendo imagen». */
  esperando = $state<number[]>([]);

  convierte(idMapa: number): boolean {
    return this.mapas.includes(idMapa);
  }

  /** ¿Enseñar el aviso en la tarjeta? Mientras convierte, o hasta que su miniatura (`lista`) aparezca. */
  enCurso(idMapa: number, miniaturaLista: boolean): boolean {
    return this.convierte(idMapa) || (this.esperando.includes(idMapa) && !miniaturaLista);
  }

  /** Tras cambiar la imagen de un mapa: el aviso sigue hasta que salga su miniatura (con tope, por si no hay: SVG, error...). */
  esperarMiniatura(idMapa: number) {
    this.esperando = [...this.esperando.filter((i) => i !== idMapa), idMapa];
    setTimeout(() => (this.esperando = this.esperando.filter((i) => i !== idMapa)), 15000);
  }

  /** Ejecuta `tarea` marcando el mapa (o un mapa nuevo, con `null`) como «convirtiendo» hasta que termine, falle o no. */
  async durante<T>(idMapa: number | null, tarea: () => Promise<T>): Promise<T> {
    if (idMapa === null) this.nuevos++;
    else this.mapas = [...this.mapas, idMapa];
    try {
      return await tarea();
    } finally {
      if (idMapa === null) this.nuevos--;
      else {
        const i = this.mapas.indexOf(idMapa);
        if (i >= 0) this.mapas = this.mapas.filter((_, j) => j !== i);
      }
    }
  }
}

export const conversionImagen = new ConversionImagen();
export const TEXTO_CONVIRTIENDO = "Convirtiendo imagen";
