// Pruebas de qué pasa al soltar una ficha (src/lib/arrastreFicha.svelte.ts) y de las fichas del
// tablero (src/lib/fichasTablero.ts).
import { beforeEach, describe, expect, it, vi } from "vitest";

let accionAlSoltar: typeof import("$lib/arrastreFicha.svelte").accionAlSoltar;
let moverEnLista: typeof import("$lib/arrastreFicha.svelte").moverEnLista;
let fichas: typeof import("$lib/fichasTablero");

beforeEach(async () => {
  vi.resetModules();
  vi.doMock("@tauri-apps/api/core", () => ({ invoke: async () => [], convertFileSrc: (p: string) => p }));
  ({ accionAlSoltar, moverEnLista } = await import("$lib/arrastreFicha.svelte"));
  fichas = await import("$lib/fichasTablero");
});

const base = (id: number, cuerpo: any = "Pequenio"): any => ({
  id, nombre: "F" + id, tipo: "Enemigo", parametros: [], cuerpo, vision: 0, zona_id: 1, foto_url: null, archivos_adjuntos: [], es_unico: false
});
const instancia = (id: number, idMapa: number | null, casilla: { x: number; y: number } | null, cuerpo?: any): any => ({
  id_instancia: id, id_mapa: idMapa, base: base(1, cuerpo), valores_actuales: [],
  coordenadas: casilla ? { Casilla: casilla } : "NoPuesto", activo: true
});
const destino = { id_mapa: 1, casilla: { x: 2, y: 3 } };

describe("accionAlSoltar", () => {
  it("una ficha del banco se instancia en el tablero o, en «Fichas activas», sin casilla", () => {
    const origen = { entidad: base(7) };
    expect(accionAlSoltar(origen, "tablero", destino)).toEqual({ tipo: "instanciar", idEntidad: 7, destino });
    expect(accionAlSoltar(origen, "activas", null)).toEqual({ tipo: "instanciar", idEntidad: 7, destino: null });
    // Fuera del mapa, en el propio banco o en ninguna parte: nada
    expect(accionAlSoltar(origen, "tablero", null)).toBeNull();
    expect(accionAlSoltar(origen, "banco", null)).toBeNull();
    expect(accionAlSoltar(origen, null, null)).toBeNull();
  });

  it("varias fichas del banco (selección con Ctrl) se instancian todas de golpe", () => {
    const origen = { entidades: [base(7), base(3), base(9)] };
    expect(accionAlSoltar(origen, "tablero", destino)).toEqual({ tipo: "instanciarVarias", idsEntidades: [7, 3, 9], destino });
    expect(accionAlSoltar(origen, "activas", null)).toEqual({ tipo: "instanciarVarias", idsEntidades: [7, 3, 9], destino: null });
    expect(accionAlSoltar(origen, "tablero", null)).toBeNull();
    expect(accionAlSoltar(origen, "banco", null)).toBeNull();
  });

  it("arrastrar desde el banco con el botón derecho instancia oculto; el resto de acciones no cambia", () => {
    const una = { entidad: base(7) };
    const varias = { entidades: [base(7), base(3)] };
    expect(accionAlSoltar(una, "tablero", destino, [], null, true)).toEqual({ tipo: "instanciar", idEntidad: 7, destino, oculto: true });
    expect(accionAlSoltar(una, "activas", null, [], null, true)).toEqual({ tipo: "instanciar", idEntidad: 7, destino: null, oculto: true });
    expect(accionAlSoltar(varias, "tablero", destino, [], null, true)).toEqual({ tipo: "instanciarVarias", idsEntidades: [7, 3], destino, oculto: true });
    expect(accionAlSoltar(varias, "activas", null, [], null, true)).toEqual({ tipo: "instanciarVarias", idsEntidades: [7, 3], destino: null, oculto: true });
    // Sin pedirlo (botón izquierdo) la acción ni lleva el campo
    expect(accionAlSoltar(una, "tablero", destino, [], null, false)).toEqual({ tipo: "instanciar", idEntidad: 7, destino });
    expect(accionAlSoltar(una, "banco", null, [], null, true)).toBeNull();
    // Mover una instancia nunca cambia si está oculta
    const colocada = { instancia: instancia(4, 1, { x: 0, y: 0 }), desde: "tablero" as const };
    expect(accionAlSoltar(colocada, "tablero", destino, [], null, true)).toEqual({ tipo: "mover", idInstancia: 4, destino });
  });

  it("una instancia se mueve en el tablero, sale de él en «Fichas activas» y se elimina en el banco", () => {
    const colocada = { instancia: instancia(4, 1, { x: 0, y: 0 }), desde: "tablero" as const };
    expect(accionAlSoltar(colocada, "tablero", destino)).toEqual({ tipo: "mover", idInstancia: 4, destino });
    expect(accionAlSoltar(colocada, "activas", null)).toEqual({ tipo: "quitarDelTablero", idInstancia: 4 });
    expect(accionAlSoltar(colocada, "banco", null)).toEqual({ tipo: "borrar", idInstancia: 4 });
    expect(accionAlSoltar(colocada, "tablero", null)).toBeNull();

    // Una que ya está fuera del tablero, soltada en «Fichas activas»: no hay nada que hacer
    const sinColocar = { instancia: instancia(5, null, null), desde: "tablero" as const };
    expect(accionAlSoltar(sinColocar, "activas", null)).toBeNull();
    expect(accionAlSoltar(sinColocar, "banco", null)).toEqual({ tipo: "borrar", idInstancia: 5 });
  });

  it("un grupo del tablero se mueve junto lo que se movió la cogida, sale entero o se borra entero", () => {
    const cogida = instancia(4, 1, { x: 5, y: 5 });
    const grupo = [instancia(2, 1, { x: 1, y: 1 }), cogida];
    const origen = { instancia: cogida, desde: "tablero" as const, grupo };
    // De (5, 5) a (2, 3): todas -3, -2
    expect(accionAlSoltar(origen, "tablero", destino)).toEqual({ tipo: "moverVarias", ids: [2, 4], idMapa: 1, dx: -3, dy: -2 });
    expect(accionAlSoltar(origen, "activas", null)).toEqual({ tipo: "quitarVariasDelTablero", ids: [2, 4] });
    expect(accionAlSoltar(origen, "banco", null)).toEqual({ tipo: "borrarVarias", ids: [2, 4] });
    expect(accionAlSoltar(origen, "tablero", null)).toBeNull();
    // Un grupo de una sola es como sin grupo
    expect(accionAlSoltar({ ...origen, grupo: [cogida] }, "tablero", destino)).toEqual({ tipo: "mover", idInstancia: 4, destino });
  });
});

describe("soltar en otra caja de una batalla", () => {
  const deActivas = { instancia: instancia(1, 1, { x: 0, y: 0 }), desde: "activas" as const };

  it("soltar en otra caja cambia de grupo (con el foco entero) en vez de reordenar", () => {
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], 0, false, { destino: 2, origen: 1, ids: [1, 3] })).toEqual({ tipo: "aGrupo", ids: [1, 3], idGrupo: 2 });
    // «No participa» es la caja 0
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], 0, false, { destino: 0, origen: 1, ids: [1] })).toEqual({ tipo: "aGrupo", ids: [1], idGrupo: 0 });
  });

  it("soltar en su propia caja (o fuera de cualquiera) sigue reordenando", () => {
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], 2, false, { destino: 1, origen: 1, ids: [1] })).toEqual({ tipo: "reordenar", ids: [2, 3, 1] });
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], 2, false, { destino: null, origen: 1, ids: [1] })).toEqual({ tipo: "reordenar", ids: [2, 3, 1] });
  });
});

describe("reordenar «Fichas activas»", () => {
  it("mover un id a otra posición", () => {
    expect(moverEnLista([1, 2, 3, 4], 1, 2)).toEqual([2, 3, 1, 4]);
    expect(moverEnLista([1, 2, 3, 4], 4, 0)).toEqual([4, 1, 2, 3]);
    expect(moverEnLista([1, 2, 3], 2, 99)).toEqual([1, 3, 2]);
  });

  it("una tarjeta arrastrada dentro de la lista la reordena; si no cambia de sitio, nada", () => {
    const deActivas = { instancia: instancia(2, 1, { x: 0, y: 0 }), desde: "activas" as const };
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], 0)).toEqual({ tipo: "reordenar", ids: [2, 1, 3] });
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], 1)).toBeNull();
    expect(accionAlSoltar(deActivas, "activas", null, [1, 2, 3], null)).toBeNull();
    // Desde la lista al tablero o al banco, lo de siempre
    expect(accionAlSoltar(deActivas, "tablero", destino)).toEqual({ tipo: "mover", idInstancia: 2, destino });
    expect(accionAlSoltar(deActivas, "banco", null)).toEqual({ tipo: "borrar", idInstancia: 2 });
  });
});

describe("fichas del tablero", () => {
  it("solo las colocadas en ese mapa, en orden", () => {
    const lista = [instancia(1, 1, { x: 0, y: 0 }), instancia(2, 2, { x: 0, y: 0 }), instancia(3, null, null), instancia(4, 1, { x: 5, y: 5 })];
    expect(fichas.fichasDelMapa(lista, 1).map((f) => f.instancia.id_instancia)).toEqual([1, 4]);
    expect(fichas.fichasDelMapa(lista, null)).toEqual([]);
  });

  it("una ficha girada ocupa sus casillas giradas", () => {
    const tumbada = { ...instancia(1, 1, { x: 5, y: 5 }, { Grande: [1, 3] }), rotacion: 1 }; // 3×1: x 4..6
    const enMapa = fichas.fichasDelMapa([tumbada], 1);
    expect(fichas.fichaEnCasilla(enMapa, { x: 4, y: 5 })?.id_instancia).toBe(1);
    expect(fichas.fichaEnCasilla(enMapa, { x: 5, y: 4 })).toBeNull();
  });

  it("la ficha bajo una casilla cubre todo su cuerpo, y gana la de encima", () => {
    const grande = instancia(1, 1, { x: 5, y: 5 }, { Grande: [3, 3] }); // ocupa 4..6
    const pequena = instancia(2, 1, { x: 6, y: 6 });
    const enMapa = fichas.fichasDelMapa([grande, pequena], 1);
    expect(fichas.fichaEnCasilla(enMapa, { x: 4, y: 4 })?.id_instancia).toBe(1);
    expect(fichas.fichaEnCasilla(enMapa, { x: 6, y: 6 })?.id_instancia).toBe(2); // la última, encima
    expect(fichas.fichaEnCasilla(enMapa, { x: 7, y: 5 })).toBeNull();
  });

  it("la de capa mayor (la última puesta o movida) se dibuja encima y es la que se coge", () => {
    const abajo = { ...instancia(1, 1, { x: 2, y: 2 }), capa: 5 };
    const arriba = { ...instancia(2, 1, { x: 2, y: 2 }), capa: 9 };
    const sinCapa = instancia(3, 1, { x: 7, y: 7 }); // partida antigua: capa 0
    const enMapa = fichas.fichasDelMapa([arriba, abajo, sinCapa], 1);
    expect(enMapa.map((f) => f.instancia.id_instancia)).toEqual([3, 1, 2]);
    expect(fichas.fichaEnCasilla(enMapa, { x: 2, y: 2 })?.id_instancia).toBe(2);
  });

  it("el recuadro de selección coge las que toca, aunque no queden dentro del todo", () => {
    const r = { columnas: 10, filas: 8, anchoCasilla: 10, altoCasilla: 10, ancho: 100, alto: 80 };
    const enMapa = fichas.fichasDelMapa(
      [instancia(1, 1, { x: 1, y: 1 }), instancia(2, 1, { x: 5, y: 5 }), instancia(3, 1, { x: 3, y: 1 }, { Grande: [3, 1] })],
      1
    );
    const ids = (desde: any, hasta: any) => fichas.fichasEnRectangulo(enMapa, r, desde, hasta).map((i) => i.id_instancia);
    // De (15, 15) a (25, 18): toca la 1 (x 10..20) y la 3 (x 20..50)
    expect(ids({ x: 15, y: 15 }, { x: 25, y: 18 })).toEqual([1, 3]);
    // Las esquinas en cualquier orden
    expect(ids({ x: 25, y: 18 }, { x: 15, y: 15 })).toEqual([1, 3]);
    // Solo rozar el borde no cuenta
    expect(ids({ x: 0, y: 0 }, { x: 10, y: 10 })).toEqual([]);
    expect(ids({ x: 0, y: 0 }, { x: 100, y: 80 })).toEqual([1, 2, 3]);
  });

  it("cada tipo tiene su color", () => {
    expect(fichas.COLOR_TIPO).toEqual({ Enemigo: "#ef4444", NPC: "#3b82f6", Jugador: "#22c55e", Objeto: "#6D6B65" });
  });
});

describe("formas de los parámetros en el tablero", () => {
  const rejilla = { columnas: 10, filas: 8, anchoCasilla: 10, altoCasilla: 10, ancho: 100, alto: 80 };
  const cam = { x: 0, y: 0, zoom: 1 };
  const definiciones: any[] = [
    { id: 1, clave: "Rango", valor_por_defecto: { Numero: 2 }, tiene_maximo: false, representar_mapa: { Cuadrado: [255, 0, 0, 90] }, visibilidad: "Publico", unidad: null, atenuar_si_negativo: false },
    { id: 2, clave: "Vida", valor_por_defecto: { Numero: 5 }, tiene_maximo: true, representar_mapa: { BarraVida: [0, 255, 0, 255] }, visibilidad: "Publico", unidad: null, atenuar_si_negativo: true }
  ];
  /** Un contexto de mentira que cuenta qué se ha dibujado */
  const contexto = () => {
    const llamadas: string[] = [];
    const ctx: any = new Proxy({}, { get: (_t, nombre: string) => (nombre === "save" || nombre === "restore" ? () => {} : (...a: any[]) => void llamadas.push(nombre)), set: () => true });
    return { ctx, llamadas };
  };
  const dibujar = (tipo: string) => {
    const { ctx, llamadas } = contexto();
    const instancia_ = instancia(1, 1, { x: 4, y: 4 });
    instancia_.base.tipo = tipo;
    fichas.dibujarFormas(ctx, cam, rejilla, { instancia: instancia_, casilla: { x: 4, y: 4 } }, definiciones, () => 3, () => 5);
    return llamadas;
  };

  it("un Enemigo dibuja el rango y la barra de vida; un Objeto no dibuja nada", () => {
    expect(dibujar("Enemigo")).toEqual(expect.arrayContaining(["rect", "fillRect"]));
    expect(dibujar("Objeto")).toEqual([]);
  });

  it("la caja «Parámetros» oculta a un Objeto los de forma visual, pero no el Resumen ni los que no se representan", () => {
    const [rango, vida] = definiciones;
    const resumen = { ...rango, id: 3, representar_mapa: { Resumen: [255, 255, 255, 255] } };
    const sinForma = { ...rango, id: 4, representar_mapa: null };
    expect([rango, vida, resumen, sinForma].map((d) => fichas.parametroOculto("Objeto", d))).toEqual([true, true, false, false]);
    expect([rango, vida].map((d) => fichas.parametroOculto("Enemigo", d))).toEqual([false, false]);
  });
});

describe("instancias ocultas en el tablero", () => {
  it("una oculta se dibuja al 50 % de transparencia y una visible, al 100 %", () => {
    const alfas = (oculta: boolean | undefined) => {
      const valores: number[] = [];
      let alfa = 1;
      const ctx: any = new Proxy({}, {
        get: (_t, nombre: string) => {
          if (nombre === "globalAlpha") return alfa;
          return nombre === "save" || nombre === "restore" ? () => {} : () => { valores.push(alfa); };
        },
        set: (_t, nombre: string, v: any) => { if (nombre === "globalAlpha") alfa = v; return true; }
      });
      const rejilla = { columnas: 10, filas: 8, anchoCasilla: 10, altoCasilla: 10, ancho: 100, alto: 80 };
      fichas.dibujarFicha(ctx, { x: 0, y: 0, zoom: 1 }, rejilla, { instancia: instancia(1, 1, { x: 4, y: 4 }), casilla: { x: 4, y: 4 } }, null, { oculta });
      return new Set(valores);
    };
    expect(alfas(undefined)).toEqual(new Set([1]));
    expect(alfas(true)).toEqual(new Set([0.5]));
  });
});

describe("coger una ficha que quedó fuera del mapa", () => {
  it("se busca por el rectángulo de la ficha, no por su casilla, y gana la de encima", () => {
    const r = { columnas: 10, filas: 8, anchoCasilla: 10, altoCasilla: 10, ancho: 100, alto: 80 };
    // Una ficha en la casilla (14, 3): fuera del mapa (que acaba en x=100), pero a la vista
    const fuera = fichas.fichasDelMapa([instancia(1, 1, { x: 14, y: 3 })], 1);
    expect(fichas.fichaEnPunto(fuera, r, 145, 35)?.id_instancia).toBe(1);
    expect(fichas.fichaEnPunto(fuera, r, 155, 35)).toBeNull();

    const dos = fichas.fichasDelMapa([instancia(1, 1, { x: 2, y: 2 }), instancia(2, 1, { x: 2, y: 2 })], 1);
    expect(fichas.fichaEnPunto(dos, r, 25, 25)?.id_instancia).toBe(2);
  });
});
