// src/lib/variables.ts
//
// Las variables de las fichas (Fuerza «f», Inteligencia «in»...): el host las define (nombre, abreviatura, valor
// neutro y escala) y cada ficha pone su valor. Lo que cuenta en el chat (`2d6 + f`) es el RESULTADO, que se
// calcula. Espejo de src-tauri/src/tablero_general/variables.rs (Rust lo vuelve a comprobar y es quien tira).
import type { DefinicionVariable, ValorVariable } from "./partida.svelte";

/** Con lo que nace una variable nueva en el formulario del host. */
export const VARIABLE_NUEVA = { neutro: 10, escala: 2 } as const;

/** Tope del valor de una variable y de su neutro (`LIMITE_VALOR_VARIABLE` en Rust). */
export const LIMITE_VALOR_VARIABLE = 9999;
export const MAX_ESCALA_VARIABLE = 100;
export const MAX_NOMBRE_VARIABLE = 30;
export const MAX_ABREVIATURA_VARIABLE = 10;

/** `floor((valor − neutro) / escala)`: con neutro 10 y escala 2, 12 → +1, 9 → −1. */
export function modificadorVariable(def: Pick<DefinicionVariable, "neutro" | "escala">, valor: number): number {
  return Math.floor((valor - def.neutro) / Math.max(1, def.escala));
}

/** «+1», «-1» o «0», como sale en la tabla de la ficha. */
export function textoModificador(n: number): string {
  return n > 0 ? `+${n}` : String(n);
}

/** Igual que `normalizar_variables` en Rust: una por definición y en su orden; las que faltan, con el neutro. */
export function normalizarVariables(defs: DefinicionVariable[], valores: ValorVariable[]): ValorVariable[] {
  return defs.map((d) => {
    const v = valores.find((x) => x.id_definicion === d.id);
    return { id_definicion: d.id, valor: v ? v.valor : d.neutro };
  });
}
