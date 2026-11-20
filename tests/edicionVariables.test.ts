// Las filas de variables con cambios sin guardar (src/lib/edicionVariables.svelte.ts): qué hay pendiente, en qué orden se
// guarda, y las que se guardaron en esta sesión (franja verde).
import { describe, expect, it } from "vitest";
import { EdicionVariables, type FilaEditable } from "$lib/edicionVariables.svelte";

function fila(resultado = true, registro: string[] = [], nombre = "f"): FilaEditable {
  return {
    guardar: async () => {
      registro.push(`guardar ${nombre}`);
      return resultado;
    },
    descartar: () => void registro.push(`descartar ${nombre}`)
  };
}

describe("cambios sin guardar de las variables", () => {
  it("hay cambios mientras alguna fila está sucia, y se cuentan", () => {
    const e = new EdicionVariables();
    const a = Symbol("a");
    const b = Symbol("b");
    e.registrar(a, fila());
    e.registrar(b, fila());
    expect(e.hayCambios).toBe(false);
    e.marcar(a, true);
    e.marcar(b, true);
    expect([e.hayCambios, e.cuantas]).toEqual([true, 2]);
    e.marcar(a, false);
    expect([e.hayCambios, e.cuantas]).toEqual([true, 1]);
    e.marcar(b, false);
    expect(e.hayCambios).toBe(false);
  });

  it("una fila que se desmonta deja de contar", () => {
    const e = new EdicionVariables();
    const a = Symbol("a");
    const quitar = e.registrar(a, fila());
    e.marcar(a, true);
    quitar();
    expect(e.hayCambios).toBe(false);
  });

  it("guardarTodo guarda solo las sucias, en orden de registro, y sigue aunque una falle", async () => {
    const e = new EdicionVariables();
    const registro: string[] = [];
    const [a, b, c, d] = [Symbol("a"), Symbol("b"), Symbol("c"), Symbol("d")];
    e.registrar(a, fila(true, registro, "a"));
    e.registrar(b, fila(false, registro, "b")); // falla
    e.registrar(c, fila(true, registro, "c")); // limpia: no se toca
    e.registrar(d, fila(true, registro, "d"));
    for (const k of [a, b, d]) e.marcar(k, true);
    expect(await e.guardarTodo()).toBe(false);
    expect(registro).toEqual(["guardar a", "guardar b", "guardar d"]);
    // Sin nada que falle devuelve true
    const limpio = new EdicionVariables();
    limpio.registrar(a, fila());
    limpio.marcar(a, true);
    expect(await limpio.guardarTodo()).toBe(true);
    expect(await new EdicionVariables().guardarTodo()).toBe(true);
  });

  it("descartarTodo deshace solo las sucias", () => {
    const e = new EdicionVariables();
    const registro: string[] = [];
    const [a, b] = [Symbol("a"), Symbol("b")];
    e.registrar(a, fila(true, registro, "a"));
    e.registrar(b, fila(true, registro, "b"));
    e.marcar(b, true);
    e.descartarTodo();
    expect(registro).toEqual(["descartar b"]);
  });

  it("los guardados en esta sesión salen en verde hasta quitarlos o empezar de cero", () => {
    const e = new EdicionVariables();
    e.marcarVerde(3);
    e.marcarVerde(4);
    expect(e.verdes.has(3) && e.verdes.has(4)).toBe(true);
    e.quitarVerde(3);
    expect(e.verdes.has(3)).toBe(false);
    e.reiniciar();
    expect(e.verdes.size).toBe(0);
  });

  it("reiniciar olvida filas y pendientes", () => {
    const e = new EdicionVariables();
    const a = Symbol("a");
    e.registrar(a, fila());
    e.marcar(a, true);
    e.reiniciar();
    expect(e.hayCambios).toBe(false);
  });
});
