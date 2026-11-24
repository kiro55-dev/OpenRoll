// Las macros de una ficha en la caja «Parámetros» (macros.svelte.ts): cuáles salen, en qué orden y cuáles como botón.
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: async () => [], convertFileSrc: (s: string) => s }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));

const { macrosDeInstancia, macrosEditables } = await import("$lib/macros.svelte");

const macro = (id: number, al_pulsar: boolean, en_turno: boolean, codigo = "x") => ({ id, nombre: `M${id}`, codigo, autor: 0, al_pulsar, en_turno });
const asoc = (id: number, id_macro: number, destino: { Ficha: number } | { Instancia: number }) => ({ id, id_macro, destino, autor: 0, atributos: [] });

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
