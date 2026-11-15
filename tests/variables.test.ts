// Variables de las fichas (src/lib/variables.ts) y cómo le llegan al jugador (src/lib/red/replica.ts).
import { describe, expect, it } from "vitest";
import { modificadorVariable, normalizarVariables, textoModificador, VARIABLE_NUEVA } from "$lib/variables";
import { aplicarCambios, partesCambiadas, responder, vistaVacia } from "$lib/red/replica";

const fuerza = { id: 1, nombre: "Fuerza", abreviatura: "f", neutro: 10, escala: 2 };
const cordura = { id: 2, nombre: "Cordura", abreviatura: "cor", neutro: 7, escala: 3 };

describe("resultado de una variable (igual que variables::modificador en Rust)", () => {
  it("se redondea hacia abajo, como en D&D", () => {
    expect(modificadorVariable(fuerza, 12)).toBe(1);
    expect(modificadorVariable(fuerza, 8)).toBe(-1);
    expect(modificadorVariable(fuerza, 9)).toBe(-1);
    expect(modificadorVariable(fuerza, 10)).toBe(0);
    expect(modificadorVariable(fuerza, 11)).toBe(0);
    expect(modificadorVariable(fuerza, 20)).toBe(5);
    expect(modificadorVariable(cordura, 9)).toBe(0);
    expect(modificadorVariable(cordura, 6)).toBe(-1);
  });

  it("se escribe con signo si es positivo", () => {
    expect([1, -1, 0, 12].map(textoModificador)).toEqual(["+1", "-1", "0", "+12"]);
  });

  it("una variable nueva nace con neutro 10 y escala 2", () => {
    expect(VARIABLE_NUEVA).toEqual({ neutro: 10, escala: 2 });
  });

  it("normalizar deja una por definición, en su orden, con el neutro en las que faltan", () => {
    expect(normalizarVariables([fuerza, cordura], [{ id_definicion: 2, valor: 9 }, { id_definicion: 9, valor: 1 }])).toEqual([
      { id_definicion: 1, valor: 10 },
      { id_definicion: 2, valor: 9 }
    ]);
  });
});

describe("las variables en la réplica del jugador", () => {
  it("llegan con la vista y con los cambios, y hacen releer las definiciones", () => {
    let vista = vistaVacia();
    expect(responder("obtener_definiciones_variables", {}, vista)).toEqual([]);
    vista = aplicarCambios(vista, { definiciones_variables: [fuerza] });
    expect(responder("obtener_definiciones_variables", {}, vista)).toEqual([fuerza]);
    expect(partesCambiadas({ definiciones_variables: [fuerza] }).definiciones).toBe(true);
    // Unos cambios sin ellas no las tocan
    vista = aplicarCambios(vista, { definiciones: [] });
    expect(vista.definiciones_variables).toEqual([fuerza]);
  });

  it("una vista de un host antiguo (sin el campo) no se rompe", () => {
    const { definiciones_variables: _, ...antigua } = vistaVacia();
    expect(responder("obtener_definiciones_variables", {}, antigua)).toEqual([]);
  });
});
