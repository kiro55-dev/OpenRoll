// src/lib/variables.ts
//
// Las variables de las fichas (Fuerza «f», Acrobacias «acro»...), en GRUPOS de bases (el host define nombre, abreviatura,
// neutro y escala, y cada ficha pone su valor) o de derivadas (suman el resultado de sus bases y, si la ficha las marca,
// el de sus opcionales). Lo que cuenta en el chat (`2d6 + f`) es el RESULTADO, que se calcula.
// Espejo de src-tauri/src/tablero_general/variables.rs (Rust lo vuelve a comprobar y es quien tira).
import type { DefinicionVariable, ValorVariable } from "./partida.svelte";

/** Con lo que nace una variable nueva en el formulario del host. */
export const VARIABLE_NUEVA = { neutro: 10, escala: 2 } as const;

/** Tope del valor de una variable y de su neutro (`LIMITE_VALOR_VARIABLE` en Rust). */
export const LIMITE_VALOR_VARIABLE = 9999;
export const MAX_ESCALA_VARIABLE = 100;
export const MAX_NOMBRE_VARIABLE = 30;
export const MAX_ABREVIATURA_VARIABLE = 10;
export const MAX_NOMBRE_GRUPO = 30;

/** `floor((valor − neutro) / escala)`: con neutro 10 y escala 2, 12 → +1, 9 → −1. */
export function modificadorVariable(def: Pick<DefinicionVariable, "neutro" | "escala">, valor: number): number {
  return Math.floor((valor - def.neutro) / Math.max(1, def.escala));
}

/** «+1», «-1» o «0», como sale en la tabla de la ficha. */
export function textoModificador(n: number): string {
  return n > 0 ? `+${n}` : String(n);
}

/** ¿Es derivada? (tiene bases; una base no tiene ninguna). */
export const esDerivada = (d: Pick<DefinicionVariable, "bases">) => (d.bases?.length ?? 0) > 0;

/** Cómo se nombra una variable en pantalla y en el desglose: su abreviatura o, sin ella, su nombre. */
export const etiquetaVariable = (d: Pick<DefinicionVariable, "nombre" | "abreviatura">) => d.abreviatura || d.nombre;

/** Lo que suma cada parte de una variable en una ficha (una base: ella misma; una derivada: sus bases y sus opcionales
 *  marcadas), con su etiqueta. Es el desglose que sale al pasar el ratón: «des (+3) + com (+2)». */
export function partesResultado(
  defs: DefinicionVariable[],
  valores: ValorVariable[],
  def: DefinicionVariable
): { etiqueta: string; valor: number }[] {
  const valorDe = (id: number) => valores.find((v) => v.id_definicion === id);
  const deBase = (id: number) => {
    const b = defs.find((d) => d.id === id && !esDerivada(d));
    if (!b) return null;
    const v = valorDe(id);
    const valor = v && typeof v.valor === "number" && Number.isFinite(v.valor) ? v.valor : b.neutro;
    return { etiqueta: etiquetaVariable(b), valor: modificadorVariable(b, valor) };
  };
  if (!esDerivada(def)) return [deBase(def.id) ?? { etiqueta: etiquetaVariable(def), valor: 0 }];
  const marcadas = valorDe(def.id)?.opcionales ?? [];
  return [...def.bases, ...def.opcionales.filter((o) => marcadas.includes(o))].flatMap((id) => deBase(id) ?? []);
}

/** Igual que `variables::resultado` en Rust: una base, su modificador; una derivada, la suma de sus bases y de sus
 *  opcionales marcadas en la ficha. Un valor que falte cuenta como el neutro. */
export function resultadoVariable(defs: DefinicionVariable[], valores: ValorVariable[], def: DefinicionVariable): number {
  return partesResultado(defs, valores, def).reduce((s, p) => s + p.valor, 0);
}

/** Igual que `normalizar_variables` en Rust: una por definición y en su orden; las que faltan, con el neutro; en una
 *  derivada solo quedan marcadas las opcionales que lo siguen siendo, y en una base ninguna. */
export function normalizarVariables(defs: DefinicionVariable[], valores: ValorVariable[]): ValorVariable[] {
  return defs.map((d) => {
    const v = valores.find((x) => x.id_definicion === d.id);
    const opcionales = [...new Set((v?.opcionales ?? []).filter((o) => (d.opcionales ?? []).includes(o)))];
    return { id_definicion: d.id, valor: v ? v.valor : d.neutro, opcionales };
  });
}

/** Igual que `efecto_de_borrar` en Rust: al borrar `ids`, las derivadas que las usaban y siguen (`afectadas`) y todo lo que
 *  se borra (`borradas`: esas y las derivadas que se quedan sin bases). Para el aviso antes de confirmar. */
export function efectoDeBorrar(defs: DefinicionVariable[], ids: number[]): { afectadas: number[]; borradas: number[] } {
  const borradas = [...ids];
  const sinBases = defs.filter((d) => esDerivada(d) && !borradas.includes(d.id) && d.bases.every((b) => borradas.includes(b)));
  borradas.push(...sinBases.map((d) => d.id));
  const afectadas = defs
    .filter((d) => !borradas.includes(d.id) && [...d.bases, ...d.opcionales].some((x) => borradas.includes(x)))
    .map((d) => d.id);
  return { afectadas, borradas };
}

/** El aviso de borrar `ids` («La usan Acrobacias, Sigilo: …»), o `null` si no afecta a ninguna otra variable. */
export function avisoDeBorrar(defs: DefinicionVariable[], ids: number[]): string | null {
  const { afectadas, borradas } = efectoDeBorrar(defs, ids);
  const nombres = (lista: number[]) => lista.map((id) => defs.find((d) => d.id === id)?.nombre ?? "?").join(", ");
  const enCascada = borradas.filter((id) => !ids.includes(id));
  if (afectadas.length === 0 && enCascada.length === 0) return null;
  const partes: string[] = [];
  if (afectadas.length) partes.push(`la usan ${nombres(afectadas)}: se quitará de ellas`);
  if (enCascada.length) partes.push(`${nombres(enCascada)} se quedará${enCascada.length > 1 ? "n" : ""} sin bases y se borrará${enCascada.length > 1 ? "n" : ""}`);
  const texto = partes.join("; ");
  return texto.charAt(0).toUpperCase() + texto.slice(1) + ".";
}
