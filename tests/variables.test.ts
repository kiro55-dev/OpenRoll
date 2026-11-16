// Variables de las fichas (src/lib/variables.ts) y cómo le llegan al jugador (src/lib/red/replica.ts).
import { describe, expect, it } from "vitest";
import type { DefinicionVariable } from "$lib/partida.svelte";
import {
  avisoDeBorrar,
  bonoElegido,
  calcularParametro,
  desgloseResultado,
  formulaVariable,
  recalcularParametros,
  textoCalculo,
  tieneFormula,
  efectoDeBorrar,
  etiquetaVariable,
  modificadorVariable,
  normalizarVariables,
  partesResultado,
  resultadoVariable,
  textoModificador,
  VARIABLE_NUEVA
} from "$lib/variables";
import { aplicarCambios, partesCambiadas, responder, vistaVacia } from "$lib/red/replica";

const base = (id: number, nombre: string, abreviatura: string, neutro: number, escala: number): DefinicionVariable => ({
  id, nombre, abreviatura, neutro, escala, grupo: 1, bases: [], opcionales: []
});
const derivada = (id: number, nombre: string, abreviatura: string, bases: number[], opcionales: number[] = []): DefinicionVariable => ({
  id, nombre, abreviatura, neutro: 0, escala: 1, grupo: 2, bases, opcionales
});

const fuerza = base(1, "Fuerza", "f", 10, 2);
const cordura = base(2, "Cordura", "cor", 7, 3);

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
      { id_definicion: 1, valor: 10, opcionales: [], eleccion: 0 },
      { id_definicion: 2, valor: 9, opcionales: [], eleccion: 0 }
    ]);
  });
});

describe("variables derivadas (igual que variables::resultado en Rust)", () => {
  const destreza = base(1, "Destreza", "des", 10, 2);
  const fue = base(2, "Fuerza", "fue", 10, 2);
  const competencia = base(3, "Competencia", "com", 0, 1);
  const habichuela = base(6, "Habichuela", "", 0, 1);
  const acrobacias = derivada(4, "Acrobacias", "acro", [1], [3, 6]);
  const atletismo = derivada(5, "Atletismo", "atl", [2, 1], [3]);
  const defs = [destreza, fue, competencia, acrobacias, atletismo, habichuela];
  const valores = (marcadas: number[] = []) => [
    { id_definicion: 1, valor: 16 },
    { id_definicion: 2, valor: 8 },
    { id_definicion: 3, valor: 2 },
    { id_definicion: 4, valor: 0, opcionales: marcadas },
    { id_definicion: 5, valor: 0 },
    { id_definicion: 6, valor: 1 }
  ];

  it("suma el resultado de sus bases y de las opcionales que marca la ficha", () => {
    expect(resultadoVariable(defs, valores(), acrobacias)).toBe(3);
    expect(resultadoVariable(defs, valores([3]), acrobacias)).toBe(5);
    expect(resultadoVariable(defs, valores([3, 6]), acrobacias)).toBe(6);
    expect(resultadoVariable(defs, valores(), atletismo)).toBe(3 - 1); // dos bases
    expect(resultadoVariable(defs, valores(), destreza)).toBe(3); // una base, su modificador
    expect(resultadoVariable(defs, [], acrobacias)).toBe(0); // sin valores, el neutro
  });

  it("el desglose lleva la abreviatura de cada parte o, sin ella, el nombre", () => {
    expect(partesResultado(defs, valores([3, 6]), acrobacias)).toEqual([
      { etiqueta: "des", valor: 3 },
      { etiqueta: "com", valor: 2 },
      { etiqueta: "Habichuela", valor: 1 }
    ]);
    expect(etiquetaVariable(habichuela)).toBe("Habichuela");
    expect(etiquetaVariable(competencia)).toBe("com");
  });

  it("normalizar solo deja marcadas las opcionales que lo siguen siendo, sin repetir", () => {
    const [, , , acro] = normalizarVariables(defs, [{ id_definicion: 4, valor: 0, opcionales: [3, 3, 1, 99] }]);
    expect(acro.opcionales).toEqual([3]);
    const [des] = normalizarVariables(defs, [{ id_definicion: 1, valor: 12, opcionales: [3] }]);
    expect(des.opcionales).toEqual([]); // una base no lleva
  });

  it("borrar avisa de la cascada: quién la usa y qué derivadas se quedan sin bases", () => {
    expect(efectoDeBorrar(defs, [1])).toEqual({ afectadas: [5], borradas: [1, 4] });
    expect(efectoDeBorrar(defs, [3])).toEqual({ afectadas: [4, 5], borradas: [3] });
    expect(avisoDeBorrar(defs, [1])).toBe("La usan Atletismo: se quitará de ellas; Acrobacias se quedará sin bases y se borrará.");
    expect(avisoDeBorrar(defs, [3])).toBe("La usan Acrobacias, Atletismo: se quitará de ellas.");
    expect(avisoDeBorrar(defs, [4])).toBeNull(); // una derivada no la usa nadie
    // El grupo de bases entero: todas las derivadas se quedan sin bases
    expect(efectoDeBorrar(defs, [1, 2, 3, 6]).borradas.sort()).toEqual([1, 2, 3, 4, 5, 6]);
  });
});

describe("fórmula y rangos de una derivada (igual que variables::resultado en Rust)", () => {
  const rangos = [{ nombre: "Sin entrenar", bonus: 0 }, { nombre: "Entrenado", bonus: 2 }, { nombre: "Experto", bonus: 4 }];
  const nivel = base(3, "Nivel", "niv", 0, 1);
  const destreza = base(1, "Destreza", "des", 10, 2);

  it("la fórmula redondea hacia abajo y suma la constante", () => {
    expect(formulaVariable(7)).toBe(7);
    expect(formulaVariable(7, 1, 2)).toBe(3);
    expect(formulaVariable(-7, 1, 2)).toBe(-4);
    expect(formulaVariable(3, 2, 1, 10)).toBe(16);
    expect(formulaVariable(5, 3, 2, -1)).toBe(6);
    expect(tieneFormula({})).toBe(false);
    expect(tieneFormula({ divisor: 2 })).toBe(true);
  });

  it("el rango elegido suma su bono; una elección perdida vuelve al primero", () => {
    const acro: DefinicionVariable = { ...derivada(4, "Acrobacias", "acro", [1, 3]), opciones: rangos };
    const defs = [destreza, nivel, acro];
    const valores = [
      { id_definicion: 1, valor: 14 },
      { id_definicion: 3, valor: 3 },
      { id_definicion: 4, valor: 0, eleccion: 0 }
    ];
    expect(resultadoVariable(defs, valores, acro)).toBe(2 + 3);
    valores[2].eleccion = 2;
    expect(resultadoVariable(defs, valores, acro)).toBe(2 + 3 + 4);
    expect(bonoElegido(acro, valores[2])).toBe(4);
    expect(desgloseResultado(defs, valores, acro)).toBe("des (+2) + niv (+3) + Experto (+4)");
    valores[2].eleccion = 9;
    expect(normalizarVariables(defs, valores)[2].eleccion).toBe(0);
    expect(normalizarVariables([destreza, nivel, derivada(4, "Acrobacias", "acro", [1, 3])], [{ id_definicion: 4, valor: 0, eleccion: 2 }])[2].eleccion).toBe(0);
  });

  it("con multiplicador, divisor y constante", () => {
    const esq = base(1, "Esquivar", "esq", 0, 1);
    const mitad: DefinicionVariable = { ...derivada(2, "Esquivar difícil", "esqd", [1]), divisor: 2 };
    const valores = [{ id_definicion: 1, valor: 65 }, { id_definicion: 2, valor: 0 }];
    expect(resultadoVariable([esq, mitad], valores, mitad)).toBe(32);
    expect(desgloseResultado([esq, mitad], valores, mitad)).toBe("(esq (+65)) ÷2");
    const con: DefinicionVariable = { ...mitad, multiplicador: 2, divisor: 3, constante: -1 };
    expect(resultadoVariable([esq, con], valores, con)).toBe(Math.floor((65 * 2) / 3) - 1);
    expect(desgloseResultado([esq, con], valores, con)).toBe("(esq (+65)) ×2 ÷3 - 1");
  });

  it("un parámetro calculado sigue a las variables y se queda entre 0 y 9999", () => {
    const con = base(1, "Constitución", "con", 0, 1);
    const tam = base(2, "Tamaño", "tam", 0, 1);
    const defs = [con, tam];
    const valores = [{ id_definicion: 1, valor: 60 }, { id_definicion: 2, valor: 55 }];
    const calculo = { variables: [1, 2], multiplicador: 1, divisor: 10, constante: 0 };
    expect(calcularParametro(calculo, defs, valores)).toBe(11);
    expect(calcularParametro({ ...calculo, constante: -100 }, defs, valores)).toBe(0);
    expect(calcularParametro({ ...calculo, multiplicador: 100, divisor: 1, constante: 9999 }, defs, valores)).toBe(9999);
    expect(textoCalculo(calculo, defs)).toBe("(con + tam) ÷10");
    const ficha = {
      parametros: [{ id_definicion: 1, valor: { Numero: 99 }, publico: false }, { id_definicion: 2, valor: { Numero: 4 }, publico: false }],
      variables: valores
    } as never;
    const dp = (id: number, calculo: unknown) => ({ id, clave: "P", valor_por_defecto: { Numero: 0 }, tiene_maximo: false, representar_mapa: null, visibilidad: "Variable", unidad: null, atenuar_si_negativo: false, calculo }) as never;
    recalcularParametros([dp(1, calculo), dp(2, null)], defs, ficha);
    expect((ficha as { parametros: { valor: unknown }[] }).parametros.map((p) => p.valor)).toEqual([{ Numero: 11 }, { Numero: 4 }]);
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

  it("los grupos llegan igual y también hacen releer", () => {
    let vista = vistaVacia();
    expect(responder("obtener_grupos_variables", {}, vista)).toEqual([]);
    const grupos = [{ id: 1, nombre: "Características", tipo: "Base" as const }, { id: 2, nombre: "Habilidades", tipo: "Derivadas" as const }];
    vista = aplicarCambios(vista, { grupos_variables: grupos });
    expect(responder("obtener_grupos_variables", {}, vista)).toEqual(grupos);
    expect(partesCambiadas({ grupos_variables: grupos }).definiciones).toBe(true);
  });

  it("una vista de un host antiguo (sin el campo) no se rompe", () => {
    const { definiciones_variables: _, grupos_variables: __, ...antigua } = vistaVacia();
    expect(responder("obtener_definiciones_variables", {}, antigua)).toEqual([]);
    expect(responder("obtener_grupos_variables", {}, antigua)).toEqual([]);
  });
});
