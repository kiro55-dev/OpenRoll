// Las macros de una ficha en la caja «Parámetros» (macros.svelte.ts): cuáles salen, en qué orden y cuáles como botón.
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: async () => [], convertFileSrc: (s: string) => s }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));

const { macrosDeInstancia, macrosEditables, nombreNuevaMacro, CODIGO_NUEVA_MACRO, macros, macrosDeLaVentana, esGeneral } = await import("$lib/macros.svelte");

const macro = (id: number, al_pulsar: boolean, en_turno: boolean, codigo = "x") => ({ id, nombre: `M${id}`, codigo, autor: 0, al_pulsar, en_turno });
const asoc = (id: number, id_macro: number, destino: { Ficha: number } | { Instancia: number }) => ({ id, id_macro, destino, autor: 0 });

describe("macro nueva y borradores", () => {
  it("se llama «Macro N», con el primer número libre (sin importar espacios ni mayúsculas)", () => {
    expect(nombreNuevaMacro([])).toBe("Macro 1");
    expect(nombreNuevaMacro(["Macro 1", "macro2", "Otra"])).toBe("Macro 3");
    expect(nombreNuevaMacro(["Macro 2"])).toBe("Macro 1");
    expect(CODIGO_NUEVA_MACRO.startsWith("ONCLICK")).toBe(true);
  });

  it("el texto sin aplicar se conserva por macro y se quita al aplicarlo", () => {
    macros.cargarBorradores();
    macros.ponerBorrador(4, "ONCLICK\nEXECUTE 1d");
    expect(macros.borradores[4]).toBe("ONCLICK\nEXECUTE 1d");
    macros.ponerBorrador(5, "x");
    macros.ponerBorrador(4, undefined);
    expect(macros.borradores).toEqual({ 5: "x" });
    macros.ponerBorrador(5, undefined);
  });
});

describe("macros de un personaje y generales", () => {
  const general = { ...macro(1, true, false), propietaria: null };
  const delMago = { ...macro(2, true, false), propietaria: 7 };
  const delOrco = { ...macro(3, true, false), propietaria: 8 };
  const deJugador = { ...macro(4, true, false), autor: 1, propietaria: null }; // de antes de las de personaje

  it("la ventana enseña las generales y, desde un personaje, solo las suyas", () => {
    const todas = [general, delMago, delOrco, deJugador];
    expect(macrosDeLaVentana(todas, undefined).map((m) => m.id)).toEqual([1, 4]);
    expect(macrosDeLaVentana(todas, 7).map((m) => m.id)).toEqual([1, 2, 4]);
    expect(macrosDeLaVentana(todas, 8).map((m) => m.id)).toEqual([1, 3, 4]);
  });

  it("general = sin personaje y del host", () => {
    expect(esGeneral(general)).toBe(true);
    expect(esGeneral(delMago)).toBe(false);
    expect(esGeneral(deJugador)).toBe(false);
  });
});

describe("macros de una instancia", () => {
  const inst = { id_instancia: 7, base: { id: 2 } } as never;
  const lista = [macro(1, true, false), macro(2, false, true), macro(3, true, true), macro(4, true, false)];

  it("primero las de su ficha, luego las suyas; las ONCLICK (también si son de turno) van como botón", () => {
    const asociaciones = [asoc(10, 2, { Instancia: 7 }), asoc(11, 1, { Ficha: 2 }), asoc(12, 3, { Instancia: 7 }), asoc(13, 4, { Ficha: 9 })];
    const r = macrosDeInstancia(inst, asociaciones, lista);
    expect(r.map((x) => [x.macro.id, x.boton])).toEqual([
      [1, true],
      [2, false],
      [3, true]
    ]);
  });

  it("la misma macro dos veces (en la ficha y en la instancia) sale una vez, y una asociación sin macro no sale", () => {
    const r = macrosDeInstancia(inst, [asoc(1, 1, { Ficha: 2 }), asoc(2, 1, { Instancia: 7 }), asoc(3, 99, { Ficha: 2 })], lista);
    expect(r.map((x) => x.asociacion.id)).toEqual([1]);
  });

  it("un jugador solo edita las suyas (las ajenas le llegan sin código); el host, todas", () => {
    const mezcla = [macro(1, true, false), macro(2, true, false, "")];
    expect(macrosEditables(mezcla, true).map((m) => m.id)).toEqual([1]);
    expect(macrosEditables(mezcla, false).map((m) => m.id)).toEqual([1, 2]);
  });
});
