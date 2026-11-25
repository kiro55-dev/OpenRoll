// src/lib/apuntarArea.svelte.ts
//
// Apuntar con el ratón el `AREA` de una macro (`ENTITIES IN AREA:rombo(3)`): antes de ejecutarla, el tablero enseña la forma bajo el
// puntero (con el color de flecha y ping del usuario), un clic fija su centro y se pasa a la siguiente; Esc o el clic derecho cancelan
// toda la macro. Solo es pantalla: no se envía nada a nadie hasta ejecutar.
import type { Casilla } from "$lib/partida.svelte";
import type { FormaZona } from "$lib/fichasTablero";

/** Un `AREA` de una macro, ya calculado por Rust (`preparar_macro`): su forma y sus lados `[arriba, derecha, abajo, izquierda]`. */
export interface AreaResuelta {
  forma: FormaZona;
  lados: [number, number, number, number];
}

/** Lo que se está apuntando: las áreas, los centros ya elegidos (uno por área, en orden) y en qué mapa. */
export interface Apuntado {
  areas: AreaResuelta[];
  centros: Casilla[];
  idMapa: number;
}

/** Tras un clic: los centros con el nuevo, y si ya están todos. Pura. */
export function confirmarCentro(a: Apuntado, casilla: Casilla): { centros: Casilla[]; terminado: boolean } {
  const centros = [...a.centros, { x: casilla.x, y: casilla.y }];
  return { centros, terminado: centros.length >= a.areas.length };
}

class ApuntarArea {
  actual = $state<Apuntado | null>(null);
  /** La casilla bajo el puntero (la vista previa del área que toca va centrada en ella). */
  puntero = $state<Casilla | null>(null);
  #resolver: ((centros: Casilla[] | null) => void) | null = null;

  /** El área que se apunta ahora (la siguiente sin centro). */
  get areaActual(): AreaResuelta | null {
    const a = this.actual;
    return a ? (a.areas[a.centros.length] ?? null) : null;
  }

  /** Pide un centro por cada área. `null` si se cancela (Esc, clic derecho u otra macro). */
  pedir(areas: AreaResuelta[], idMapa: number): Promise<Casilla[] | null> {
    this.cancelar(); // una petición anterior sin terminar, se abandona
    if (!areas.length) return Promise.resolve([]);
    this.actual = { areas, centros: [], idMapa };
    this.puntero = null;
    return new Promise((resolver) => (this.#resolver = resolver));
  }

  confirmar(casilla: Casilla) {
    const a = this.actual;
    if (!a) return;
    const { centros, terminado } = confirmarCentro(a, casilla);
    if (terminado) this.#terminar(centros);
    else this.actual = { ...a, centros };
  }

  cancelar() {
    if (this.actual) this.#terminar(null);
  }

  #terminar(centros: Casilla[] | null) {
    const r = this.#resolver;
    this.#resolver = null;
    this.actual = null;
    this.puntero = null;
    r?.(centros);
  }
}

export const apuntarArea = new ApuntarArea();
