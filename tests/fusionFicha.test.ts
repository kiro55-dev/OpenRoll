// La fusión a tres bandas de la ventana de una ficha con lo que cambia fuera (src/lib/fusionFicha.ts) y su uso en el editor.
import { describe, expect, it } from "vitest";
import { actualesTocados, fusionarFicha, iguales } from "$lib/fusionFicha";

const ficha = (cambios: Record<string, unknown> = {}): any => ({
  id: 1, nombre: "Orco", tipo: "Enemigo", cuerpo: "Pequenio", vision: 0, zona_id: 1, foto_url: null, miniatura_url: null,
  foto_encuadre: { zoom: 100, x: 50, y: 50 }, archivos_adjuntos: [], es_unico: true, propietarios: [], pasos_valor: [],
  parametros: [
    { id_definicion: 1, valor: { Numero: 30 }, publico: false },
    { id_definicion: 2, valor: { Numero: 12 }, publico: false }
  ],
  variables: [
    { id_definicion: 1, valor: 10, opcionales: [] },
    { id_definicion: 2, valor: 14, opcionales: [] }
  ],
  actuales: [{ id_definicion: 1, valor: 30 }],
  ...cambios
});

describe("iguales", () => {
  it("compara a fondo sin importar el orden de las claves, y null y undefined cuentan igual", () => {
    expect(iguales({ a: 1, b: [1, { c: 2 }] }, { b: [1, { c: 2 }], a: 1 })).toBe(true);
    expect(iguales({ a: 1 }, { a: 2 })).toBe(false);
    expect(iguales([1, 2], [2, 1])).toBe(false);
    expect(iguales(null, undefined)).toBe(true);
    expect(iguales({ a: null }, {})).toBe(true);
    expect(iguales(0, null)).toBe(false);
  });
});

describe("fusionarFicha", () => {
  it("lo que la ventana no tocó sigue a lo de fuera; lo que tocó se queda", () => {
    const original = ficha();
    const editada = ficha({ nombre: "Orco jefe" }); // el usuario cambió el nombre
    const fuera = ficha({ vision: 18, actuales: [{ id_definicion: 1, valor: 21 }] }); // fuera: visión y vida actual
    const r = fusionarFicha(original, editada, fuera);
    expect(r.nombre).toBe("Orco jefe");
    expect(r.vision).toBe(18);
    expect(r.actuales).toEqual([{ id_definicion: 1, valor: 21 }]); // la vida de fuera, no la que había al abrir
  });

  it("parámetros y variables se fusionan uno a uno: tocar uno no pisa otro cambiado fuera", () => {
    const original = ficha();
    const editada = ficha({
      variables: [{ id_definicion: 1, valor: 16, opcionales: [] }, { id_definicion: 2, valor: 14, opcionales: [] }]
    });
    const fuera = ficha({
      variables: [{ id_definicion: 1, valor: 10, opcionales: [] }, { id_definicion: 2, valor: 8, opcionales: [] }],
      parametros: [{ id_definicion: 1, valor: { Numero: 35 }, publico: false }, { id_definicion: 2, valor: { Numero: 12 }, publico: true }]
    });
    const r = fusionarFicha(original, editada, fuera);
    expect(r.variables.map((v: any) => v.valor)).toEqual([16, 8]);
    expect(r.parametros[0].valor).toEqual({ Numero: 35 });
    expect(r.parametros[1].publico).toBe(true);
  });

  it("si los dos cambian lo mismo gana la ventana, y lo que es de Rust siempre viene de fuera", () => {
    const r = fusionarFicha(
      ficha(),
      ficha({ actuales: [{ id_definicion: 1, valor: 5 }], pasos_valor: [{ id_definicion: 1, paso: 9 }], miniatura_url: "x" }),
      ficha({ actuales: [{ id_definicion: 1, valor: 21 }], pasos_valor: [{ id_definicion: 1, paso: 3 }], miniatura_url: "m.webp" })
    );
    expect(r.actuales).toEqual([{ id_definicion: 1, valor: 5 }]);
    expect(r.pasos_valor).toEqual([{ id_definicion: 1, paso: 3 }]);
    expect(r.miniatura_url).toBe("m.webp");
  });

  it("actualesTocados da solo los valores actuales cambiados en la ventana", () => {
    const original = ficha({ actuales: [{ id_definicion: 1, valor: 30 }, { id_definicion: 5, valor: 4 }] });
    expect(actualesTocados(original, original)).toEqual([]);
    expect(actualesTocados(original, ficha({ actuales: [{ id_definicion: 1, valor: 30 }, { id_definicion: 5, valor: 2 }] }))).toEqual([
      { id_definicion: 5, valor: 2 }
    ]);
  });
});
