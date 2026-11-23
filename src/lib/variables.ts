// src/lib/variables.ts
//
// Las variables de las fichas (Fuerza «f», Acrobacias «acro»...), en GRUPOS de bases (el host define nombre, abreviatura,
// neutro y escala, y cada ficha pone su valor) o de derivadas (suman el resultado de sus bases y, si la ficha las marca,
// el de sus opcionales). Lo que cuenta en el chat (`2d6 + f`) es el RESULTADO, que se calcula.
// Espejo de src-tauri/src/tablero_general/variables.rs (Rust lo vuelve a comprobar y es quien tira).
import type {
  CalculoParametro,
  DefinicionParametro,
  DefinicionVariable,
  Entidad,
  FormulaEntrada,
  OrigenEntrada,
  Parametro,
  ValorVariable
} from "./partida.svelte";

/** Con lo que nace una variable nueva en el formulario del host. */
export const VARIABLE_NUEVA = { neutro: 10, escala: 2 } as const;

/** Tope del valor de una variable y de su neutro (`LIMITE_VALOR_VARIABLE` en Rust). */
export const LIMITE_VALOR_VARIABLE = 9999;
export const MAX_ESCALA_VARIABLE = 100;
export const MAX_NOMBRE_VARIABLE = 30;
export const MAX_ABREVIATURA_VARIABLE = 10;
export const MAX_NOMBRE_GRUPO = 30;
/** Tope del multiplicador y del divisor de una fórmula, de las opciones de una derivada y del nombre de una opción. */
export const MAX_FACTOR_FORMULA = 100;
export const MAX_OPCIONES_VARIABLE = 12;
export const MAX_NOMBRE_OPCION = 30;
/** Tope del valor de un parámetro calculado (`LIMITE_PARAMETRO_CALCULADO` en Rust). */
export const LIMITE_PARAMETRO_CALCULADO = 9999;

/** `floor(suma · multiplicador / divisor) + constante` (como `variables::formula` en Rust). */
export function formulaVariable(suma: number, multiplicador = 1, divisor = 1, constante = 0): number {
  return Math.floor((suma * multiplicador) / Math.max(1, divisor)) + constante;
}

/** ¿Tiene esta derivada una fórmula que no sea la suma tal cual? */
export const tieneFormula = (d: Pick<DefinicionVariable, "multiplicador" | "divisor" | "constante">) =>
  (d.multiplicador ?? 1) !== 1 || (d.divisor ?? 1) !== 1 || (d.constante ?? 0) !== 0;

/** El bono de la opción que la ficha ha elegido en una derivada con rangos (0 si no tiene). */
export function bonoElegido(def: Pick<DefinicionVariable, "opciones">, valor: ValorVariable | undefined): number {
  const opciones = def.opciones ?? [];
  return (opciones[valor?.eleccion ?? 0] ?? opciones[0])?.bonus ?? 0;
}

/** `floor((valor − neutro) / escala)`: con neutro 10 y escala 2, 12 → +1, 9 → −1. */
export function modificadorVariable(def: Pick<DefinicionVariable, "neutro" | "escala">, valor: number): number {
  return Math.floor((valor - def.neutro) / Math.max(1, def.escala));
}

/** «+1», «-1» o «0», como sale en la tabla de la ficha. */
export function textoModificador(n: number): string {
  return n > 0 ? `+${n}` : String(n);
}

/** ¿Es derivada? (tiene bases o parámetros fijos; una base no tiene ninguno). */
export const esDerivada = (d: Pick<DefinicionVariable, "bases" | "parametros">) => (d.bases?.length ?? 0) > 0 || (d.parametros?.length ?? 0) > 0;

/** Los parámetros de una ficha y sus definiciones (para el nombre): lo que necesita una derivada que suma parámetros. */
export interface FuentesParametros {
  definiciones: DefinicionParametro[];
  valores: Parametro[];
}

/** ¿Puede un parámetro ser fuente de una derivada? Solo un número escrito a mano (como `parametro_como_fuente` en Rust). */
export const parametroComoFuente = (p: DefinicionParametro) => !p.calculo && typeof p.valor_por_defecto === "object" && "Numero" in p.valor_por_defecto;

/** Lo que aporta un parámetro de la ficha: su valor si es un número (0 si falta o es texto). */
function valorDeParametro(fuentes: FuentesParametros, id: number): number {
  const v = fuentes.valores.find((p) => p.id_definicion === id)?.valor;
  return v && typeof v === "object" && "Numero" in v && typeof v.Numero === "number" && Number.isFinite(v.Numero) ? v.Numero : 0;
}

/** ¿Es el mismo origen de entrada? (una variable y un parámetro con el mismo número NO lo son) */
export function mismoOrigen(a: OrigenEntrada, b: OrigenEntrada): boolean {
  if ("Variable" in a && "Variable" in b) return a.Variable === b.Variable;
  if ("Parametro" in a && "Parametro" in b) return a.Parametro === b.Parametro;
  return false;
}

/** La fórmula individual que tiene esa entrada de la derivada (`undefined` = aporta lo suyo tal cual). */
export const formulaDeEntrada = (def: Pick<DefinicionVariable, "formulas_entradas">, origen: OrigenEntrada): FormulaEntrada | undefined =>
  (def.formulas_entradas ?? []).find((f) => mismoOrigen(f.origen, origen));

/** Una fórmula dicha corta: «×2 ÷3 +1» (solo lo que no es neutro). */
export function textoFormula(f: { multiplicador?: number; divisor?: number; constante?: number }): string {
  const m = f.multiplicador ?? 1;
  const d = f.divisor ?? 1;
  const c = f.constante ?? 0;
  return [m !== 1 ? `×${m}` : "", d !== 1 ? `÷${d}` : "", c !== 0 ? (c > 0 ? `+${c}` : `-${-c}`) : ""].filter(Boolean).join(" ");
}

/** Cómo se nombra una variable en pantalla y en el desglose: su abreviatura o, sin ella, su nombre. */
export const etiquetaVariable = (d: Pick<DefinicionVariable, "nombre" | "abreviatura">) => d.abreviatura || d.nombre;

/** Lo que suma cada parte de una variable en una ficha (una base: ella misma; una derivada: sus bases y sus opcionales
 *  marcadas), con su etiqueta. Es el desglose que sale al pasar el ratón: «des (+3) + com (+2)». */
export function partesResultado(
  defs: DefinicionVariable[],
  valores: ValorVariable[],
  def: DefinicionVariable,
  fuentes: FuentesParametros = { definiciones: [], valores: [] }
): { etiqueta: string; valor: number; antes?: number; formula?: string }[] {
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
  const marcados = valorDe(def.id)?.parametros_marcados ?? [];
  const deParametro = (id: number) => {
    const p = fuentes.definiciones.find((d) => d.id === id);
    return p ? [{ etiqueta: p.clave, valor: valorDeParametro(fuentes, id) }] : [];
  };
  // La fórmula individual de una entrada (si la tiene) se aplica a lo que aporta ANTES de sumarlas todas
  const conFormula = (origen: OrigenEntrada, partes: { etiqueta: string; valor: number }[]) => {
    const f = formulaDeEntrada(def, origen);
    return partes.map((p) =>
      f ? { etiqueta: p.etiqueta, valor: formulaVariable(p.valor, f.multiplicador, f.divisor, f.constante), antes: p.valor, formula: textoFormula(f) } : p
    );
  };
  return [
    ...[...(def.bases ?? []), ...(def.opcionales ?? []).filter((o) => marcadas.includes(o))].flatMap((id) => conFormula({ Variable: id }, deBase(id) ? [deBase(id)!] : [])),
    ...[...(def.parametros ?? []), ...(def.parametros_opcionales ?? []).filter((p) => marcados.includes(p))].flatMap((id) => conFormula({ Parametro: id }, deParametro(id)))
  ];
}

/** Igual que `variables::resultado` en Rust: una base, su modificador; una derivada, la suma de sus bases y de sus
 *  opcionales marcadas en la ficha pasada por su fórmula, más el bono del rango elegido. Un valor que falte cuenta como el neutro. */
export function resultadoVariable(
  defs: DefinicionVariable[],
  valores: ValorVariable[],
  def: DefinicionVariable,
  fuentes?: FuentesParametros
): number {
  const suma = partesResultado(defs, valores, def, fuentes).reduce((s, p) => s + p.valor, 0);
  if (!esDerivada(def)) return suma;
  const bono = bonoElegido(def, valores.find((v) => v.id_definicion === def.id));
  return formulaVariable(suma, def.multiplicador, def.divisor, def.constante) + bono;
}

/** De dónde sale el resultado de una variable, para el `title`: «des (+3) + com (+2)»; con fórmula, «(des (+3) + com (+2)) ÷2 + 1»,
 *  y con rango elegido, «… + Entrenado (+2)». */
export function desgloseResultado(
  defs: DefinicionVariable[],
  valores: ValorVariable[],
  def: DefinicionVariable,
  fuentes?: FuentesParametros
): string {
  // Una entrada con fórmula individual se escribe «des (+3 ×2 = +6)»
  const partes = partesResultado(defs, valores, def, fuentes).map((p) =>
    p.formula ? `${p.etiqueta} (${textoModificador(p.antes ?? 0)} ${p.formula} = ${textoModificador(p.valor)})` : `${p.etiqueta} (${textoModificador(p.valor)})`
  );
  let texto = partes.join(" + ");
  if (!esDerivada(def)) return texto;
  const m = def.multiplicador ?? 1;
  const d = def.divisor ?? 1;
  if (m !== 1 || d !== 1) texto = `(${texto})${m !== 1 ? ` ×${m}` : ""}${d !== 1 ? ` ÷${d}` : ""}`;
  const c = def.constante ?? 0;
  if (c !== 0) texto += c > 0 ? ` + ${c}` : ` - ${-c}`;
  const opciones = def.opciones ?? [];
  if (opciones.length > 0) {
    const v = valores.find((x) => x.id_definicion === def.id);
    const o = opciones[v?.eleccion ?? 0] ?? opciones[0];
    texto += ` + ${o.nombre} (${textoModificador(o.bonus)})`;
  }
  return texto;
}

/** Lo que cuenta de una variable al calcular un parámetro: en una base, el valor escrito; en una derivada, su resultado. */
function valorParaCalculo(defs: DefinicionVariable[], valores: ValorVariable[], id: number, fuentes?: FuentesParametros): number {
  const d = defs.find((x) => x.id === id);
  if (!d) return 0;
  if (esDerivada(d)) return resultadoVariable(defs, valores, d, fuentes);
  const v = valores.find((x) => x.id_definicion === id)?.valor;
  return typeof v === "number" && Number.isFinite(v) ? v : d.neutro;
}

/** Igual que `variables::calcular_parametro` en Rust: el valor de un parámetro calculado con esas variables, entre 0 y 9999. */
export function calcularParametro(
  calculo: CalculoParametro,
  defs: DefinicionVariable[],
  valores: ValorVariable[],
  fuentes?: FuentesParametros
): number {
  const suma = calculo.variables.reduce((s, id) => s + valorParaCalculo(defs, valores, id, fuentes), 0);
  const v = formulaVariable(suma, calculo.multiplicador, calculo.divisor, calculo.constante);
  return Math.min(LIMITE_PARAMETRO_CALCULADO, Math.max(0, v));
}

/** El cálculo de un parámetro dicho en una línea: «(con + tam) ÷10». Para los títulos y la lista de parámetros. */
export function textoCalculo(calculo: CalculoParametro, defs: DefinicionVariable[]): string {
  const nombres = calculo.variables.map((id) => defs.find((d) => d.id === id)).flatMap((d) => (d ? [etiquetaVariable(d)] : []));
  let t = `(${nombres.join(" + ")})`;
  if (calculo.multiplicador !== 1) t += ` ×${calculo.multiplicador}`;
  if (calculo.divisor !== 1) t += ` ÷${calculo.divisor}`;
  if (calculo.constante !== 0) t += calculo.constante > 0 ? ` + ${calculo.constante}` : ` - ${-calculo.constante}`;
  return t;
}

/** Pone en la ficha el valor de cada parámetro calculado (como `variables::recalcular_parametros`). */
export function recalcularParametros(defsParametros: DefinicionParametro[], defs: DefinicionVariable[], ficha: Entidad) {
  for (const d of defsParametros) {
    if (!d.calculo) continue;
    const p = ficha.parametros.find((x) => x.id_definicion === d.id);
    // Un parámetro calculado nunca es fuente de una derivada: los valores que se leen aquí no cambian al ir recalculando
    if (p) p.valor = { Numero: calcularParametro(d.calculo, defs, ficha.variables, { definiciones: defsParametros, valores: ficha.parametros }) };
  }
}

/** Igual que `normalizar_variables` en Rust: una por definición y en su orden; las que faltan, con el neutro; en una
 *  derivada solo quedan marcadas las opcionales que lo siguen siendo, y en una base ninguna. */
export function normalizarVariables(defs: DefinicionVariable[], valores: ValorVariable[]): ValorVariable[] {
  return defs.map((d) => {
    const v = valores.find((x) => x.id_definicion === d.id);
    const opcionales = [...new Set((v?.opcionales ?? []).filter((o) => (d.opcionales ?? []).includes(o)))];
    // La opción elegida existe, o se vuelve a la primera (y sin opciones no hay elección)
    const eleccion = (v?.eleccion ?? 0) < (d.opciones?.length ?? 0) ? (v?.eleccion ?? 0) : 0;
    const parametros_marcados = [...new Set((v?.parametros_marcados ?? []).filter((p) => (d.parametros_opcionales ?? []).includes(p)))];
    return { id_definicion: d.id, valor: v ? v.valor : d.neutro, opcionales, eleccion, parametros_marcados };
  });
}

/** Igual que `efecto_de_borrar` en Rust: al borrar `ids`, las derivadas que las usaban y siguen (`afectadas`) y todo lo que
 *  se borra (`borradas`: esas y las derivadas que se quedan sin bases). Para el aviso antes de confirmar. */
export function efectoDeBorrar(defs: DefinicionVariable[], ids: number[]): { afectadas: number[]; borradas: number[] } {
  const borradas = [...ids];
  const sinBases = defs.filter((d) => esDerivada(d) && !borradas.includes(d.id) && (d.parametros ?? []).length === 0 && d.bases.every((b) => borradas.includes(b)));
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
