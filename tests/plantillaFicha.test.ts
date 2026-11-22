// La plantilla de ficha (src/lib/plantillaFicha.ts): generarla con los datos dentro, leerla (entrada NO fiable) y pasarla a una ficha.
import { describe, expect, it } from "vitest";
import type { DefinicionParametro, DefinicionVariable, Entidad, GrupoVariables } from "$lib/partida.svelte";
import { aplicarAFicha, datosDePlantilla, esArchivoPlantilla, esDeEstaFicha, fichaDePlantilla, generarPlantilla, leerPlantilla, nombreTxt } from "$lib/plantillaFicha";
import plantillaHtml from "$lib/plantilla/plantilla.html?raw";

const num = (id: number, clave: string, extra: Partial<DefinicionParametro> = {}): DefinicionParametro => ({
  id, clave, valor_por_defecto: { Numero: 0 }, tiene_maximo: false, representar_mapa: null, visibilidad: "Variable", unidad: null,
  atenuar_si_negativo: false, calculo: null, ...extra
});
const PARAMETROS: DefinicionParametro[] = [
  num(1, "Vida", { tiene_maximo: true }),
  num(2, "Armadura"),
  { ...num(3, "Notas"), valor_por_defecto: { Texto: "" } },
  num(4, "Puntos de magia", { calculo: { variables: [1], multiplicador: 1, divisor: 5, constante: 0 } })
];
const GRUPOS: GrupoVariables[] = [
  { id: 1, nombre: "Características", tipo: "Base" },
  { id: 2, nombre: "Habilidades", tipo: "Derivadas" }
];
const base = (id: number, nombre: string, abreviatura: string): DefinicionVariable => ({
  id, nombre, abreviatura, neutro: 10, escala: 2, grupo: 1, bases: [], opcionales: []
});
const VARIABLES: DefinicionVariable[] = [
  base(1, "Destreza", "des"),
  base(2, "Competencia", "com"),
  {
    id: 3, nombre: "Acrobacia", abreviatura: "acro", neutro: 0, escala: 1, grupo: 2, bases: [1], opcionales: [2],
    parametros: [], parametros_opcionales: [2], opciones: [{ nombre: "Sin entrenar", bonus: 0 }, { nombre: "Experto", bonus: 4 }]
  }
];

function ficha(): Entidad {
  return {
    id: 7, nombre: "Orco", tipo: "Enemigo",
    parametros: [
      { id_definicion: 1, valor: { Numero: 20 }, publico: true },
      { id_definicion: 2, valor: { Numero: 12 }, publico: false },
      { id_definicion: 3, valor: { Texto: "" }, publico: false },
      { id_definicion: 4, valor: { Numero: 3 }, publico: false }
    ],
    variables: [
      { id_definicion: 1, valor: 14, opcionales: [], eleccion: 0, parametros_marcados: [] },
      { id_definicion: 2, valor: 12, opcionales: [], eleccion: 0, parametros_marcados: [] },
      { id_definicion: 3, valor: 0, opcionales: [2], eleccion: 1, parametros_marcados: [2] }
    ],
    cuerpo: { Grande: [2, 3] }, vision: 18, zona_id: 1, foto_url: null, miniatura_url: null,
    foto_encuadre: { x: 50, y: 50, zoom: 100 }, archivos_adjuntos: [], es_unico: false, propietarios: []
  } as unknown as Entidad;
}

const ctx = { definiciones: PARAMETROS, variables: VARIABLES, conjunto: "D&D 5e" };
const exportar = (entidad: Entidad | null, extra: Record<string, unknown> = {}) =>
  generarPlantilla({
    ...datosDePlantilla({ definiciones: PARAMETROS, grupos: GRUPOS, variables: VARIABLES, conjunto: "D&D 5e", entidad, idFicha: entidad?.id ?? null }),
    ...extra
  });

describe("la plantilla generada", () => {
  it("lleva los datos dentro y se vuelve a leer igual", () => {
    const leida = leerPlantilla(exportar(ficha()));
    expect(leida.id_ficha).toBe(7);
    expect(leida.conjunto).toBe("D&D 5e");
    expect(leida.ficha?.nombre).toBe("Orco");
    expect(leida.ficha?.cuerpo).toEqual([2, 3]);
    expect(leida.ficha?.vision).toBe(18);
    expect(leida.ficha?.variables.find((v) => v.id === 3)).toMatchObject({ eleccion: 1, eleccion_nombre: "Experto", opcionales: [2], parametros_marcados: [2] });
    expect(leida.nombresVariables.get(2)).toBe("Competencia");
    // En blanco: sin ficha ni id
    const blanca = leerPlantilla(exportar(null));
    expect([blanca.ficha, blanca.id_ficha]).toEqual([null, null]);
  });

  it("un nombre con </script> no puede romper el HTML", () => {
    const e = ficha();
    e.nombre = '</script><script>alert(1)</script>';
    const html = exportar(e);
    expect(html.match(/<\/script>/gi)?.length).toBe(plantillaHtml.match(/<\/script>/gi)?.length);
    expect(leerPlantilla(html).ficha?.nombre).toBe('</script><script>alert(1)</script>');
  });

  it("el JavaScript de la plantilla al menos se puede compilar", () => {
    const scripts = [...plantillaHtml.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
    expect(scripts.length).toBe(1);
    expect(() => new Function(scripts[0])).not.toThrow();
  });

  it("se reconoce por la extensión", () => {
    expect(esArchivoPlantilla("Orco - plantilla.html")).toBe(true);
    expect(esArchivoPlantilla("ficha.HTM")).toBe(true);
    expect(esArchivoPlantilla("foto.png")).toBe(false);
  });
});

describe("leer una plantilla (archivo de fuera)", () => {
  it("rechaza lo que no es una plantilla o está dañado", () => {
    expect(() => leerPlantilla("<html><body>hola</body></html>")).toThrow(/no es una plantilla/);
    expect(() => leerPlantilla(plantillaHtml.replace("__DATOS_OPENROLL__", "{roto"))).toThrow(/dañada/);
    expect(() => leerPlantilla(plantillaHtml.replace("__DATOS_OPENROLL__", JSON.stringify({ formato: "otro", version: 1 })))).toThrow(/no es una plantilla/);
    expect(() => leerPlantilla(exportar(null, { version: 99 }))).toThrow(/más nueva/);
    expect(() => leerPlantilla(exportar(null, { ficha: { nombre: 5 } }))).toThrow(/dañada/);
    expect(() => leerPlantilla(exportar(null, { foto: { mime: "image/png", datos: "<script>", encuadre: {} } }))).toThrow(/dañada/);
    expect(() => leerPlantilla("x".repeat(16 * 1024 * 1024 + 1))).toThrow(/MB/);
  });

  it("acota lo que se pasa de los límites", () => {
    const leida = leerPlantilla(
      exportar(null, {
        ficha: { nombre: "x".repeat(300), tipo: "Dragón", cuerpo: [999, 0], vision: 99999, parametros: [{ id: 2, clave: "Armadura", valor: 99999 }], variables: [{ id: 1, nombre: "Destreza", valor: -50000 }] },
        foto: { mime: "image/png", datos: "AAAA", encuadre: { x: 900, y: -3, zoom: 50 } }
      })
    );
    expect(leida.ficha?.nombre.length).toBe(100);
    expect(leida.ficha?.tipo).toBe("Jugador");
    expect(leida.ficha?.cuerpo).toEqual([255, 1]);
    expect(leida.ficha?.vision).toBe(10000);
    expect(leida.ficha?.parametros[0].valor).toBe(32767);
    expect(leida.ficha?.variables[0].valor).toBe(-9999);
    expect(leida.foto?.encuadre).toEqual({ x: 100, y: 0, zoom: 100 });
  });
});

describe("aplicar una plantilla a una ficha", () => {
  const limpia = (): Entidad => {
    const e = ficha();
    e.nombre = "Nueva";
    e.tipo = "Jugador";
    e.cuerpo = "Pequenio";
    e.vision = 0;
    e.parametros = e.parametros.map((p) => ({ ...p, valor: "Numero" in p.valor ? { Numero: 0 } : { Texto: "" } }));
    e.variables = e.variables.map((v) => ({ ...v, valor: 10, opcionales: [], eleccion: 0, parametros_marcados: [] }));
    return e;
  };

  it("pasa nombre, tipo, cuerpo, visión, parámetros escritos a mano y variables", () => {
    const { entidad, avisos } = aplicarAFicha(limpia(), leerPlantilla(exportar(ficha())), ctx);
    expect(avisos).toEqual([]);
    expect([entidad.nombre, entidad.tipo, entidad.cuerpo, entidad.vision]).toEqual(["Orco", "Enemigo", { Grande: [2, 3] }, 18]);
    expect(entidad.parametros.find((p) => p.id_definicion === 2)?.valor).toEqual({ Numero: 12 });
    expect(entidad.parametros.find((p) => p.id_definicion === 4)?.valor).toEqual({ Numero: 0 }, );
    // «Hacer público» viaja con el parámetro (los de visibilidad variable); la ficha de origen tenía la vida pública
    expect(entidad.parametros.find((p) => p.id_definicion === 1)?.publico).toBe(true);
    expect(entidad.parametros.find((p) => p.id_definicion === 2)?.publico).toBe(false);
    expect(entidad.variables.find((v) => v.id_definicion === 1)?.valor).toBe(14);
    expect(entidad.variables.find((v) => v.id_definicion === 3)).toMatchObject({ eleccion: 1, opcionales: [2], parametros_marcados: [2] });
  });

  it("con otro conjunto empareja por nombre y avisa de lo que falta", () => {
    // La plantilla es de otro conjunto con otros ids: Destreza era la 50, Acrobacia la 60 (Competencia, la 51) y Armadura el 9
    const otra = exportar(null, {
      conjunto: "Otro",
      definiciones: {
        parametros: [num(9, "Armadura"), num(8, "Cordura")],
        grupos_variables: GRUPOS,
        variables: [{ ...base(50, "Destreza", "des") }, { ...base(51, "Competencia", "com") }, { ...VARIABLES[2], id: 60 }]
      },
      ficha: {
        nombre: "Elfo", tipo: "NPC", cuerpo: [1, 1], vision: 30,
        parametros: [{ id: 9, clave: "Armadura", valor: 15 }, { id: 8, clave: "Cordura", valor: 50 }],
        variables: [
          { id: 50, nombre: "Destreza", valor: 16 },
          { id: 60, nombre: "Acrobacia", valor: 0, eleccion: 0, eleccion_nombre: "Experto", opcionales: [51], parametros_marcados: [9] },
          { id: 70, nombre: "Sigilo", valor: 3 }
        ]
      }
    });
    const { entidad, avisos } = aplicarAFicha(limpia(), leerPlantilla(otra), ctx);
    expect(entidad.parametros.find((p) => p.id_definicion === 2)?.valor).toEqual({ Numero: 15 });
    expect(entidad.variables.find((v) => v.id_definicion === 1)?.valor).toBe(16);
    expect(entidad.variables.find((v) => v.id_definicion === 3)).toMatchObject({ eleccion: 1, opcionales: [2], parametros_marcados: [2] });
    expect(avisos[0]).toMatch(/«Otro»/);
    expect(avisos[1]).toMatch(/Cordura, Sigilo/);
  });

  it("un rango que ya no existe vuelve al primero y una opcional que ya no lo es se quita", () => {
    const e = ficha();
    const datos = leerPlantilla(exportar(e));
    const v = datos.ficha!.variables.find((x) => x.id === 3)!;
    v.eleccion = 7;
    v.eleccion_nombre = "Legendario";
    v.opcionales = [1]; // Destreza es su base, no una opcional
    const { entidad } = aplicarAFicha(limpia(), datos, ctx);
    expect(entidad.variables.find((x) => x.id_definicion === 3)).toMatchObject({ eleccion: 0, opcionales: [] });
  });

  it("una plantilla en blanco no cambia nada", () => {
    const { entidad, avisos } = aplicarAFicha(limpia(), leerPlantilla(exportar(null)), ctx);
    expect(entidad).toEqual(limpia());
    expect(avisos).toEqual([]);
  });
});

describe("la identidad de la plantilla (plantilla_id)", () => {
  const UUID = "3f2a9c1e-7b64-4d2a-9a51-0c8e2d7f6b13";
  const leer = (extra: Record<string, unknown>) => leerPlantilla(exportar(null, extra));

  it("una plantilla en blanco no la lleva hasta que se guarda; una ficha que la tiene la exporta", () => {
    expect(leerPlantilla(exportar(null)).plantilla_id).toBeNull();
    const e = ficha();
    e.plantilla_id = UUID;
    expect(leerPlantilla(exportar(e)).plantilla_id).toBe(UUID);
    expect(leer({ plantilla_id: "no vale <script>" }).plantilla_id).toBeNull(); // solo letras, números y guiones
  });

  it("se reconoce la ficha que guarda esa identidad, aunque el archivo venga de otro equipo", () => {
    const a = { ...ficha(), id: 1 };
    const b = { ...ficha(), id: 2, plantilla_id: UUID };
    const p = leer({ plantilla_id: UUID });
    expect(fichaDePlantilla(p, [a, b])?.id).toBe(2);
    // Sin ninguna con esa identidad (y sin ficha de origen): no es de ninguna
    expect(fichaDePlantilla(p, [a])).toBeUndefined();
    // Dos fichas con la misma: no se adivina cuál; se cae a la ficha de origen, si la hay
    const c = { ...ficha(), id: 3, plantilla_id: UUID };
    expect(fichaDePlantilla(p, [b, c])).toBeUndefined();
    expect(fichaDePlantilla(leer({ plantilla_id: UUID, id_ficha: 3 }), [b, c])?.id).toBe(3);
  });

  it("una plantilla exportada de una ficha sigue reconociéndose por su id", () => {
    const p = leerPlantilla(exportar(ficha())); // ficha 7
    expect(fichaDePlantilla(p, [{ ...ficha(), id: 7 }])?.id).toBe(7);
    expect(esDeEstaFicha(p, { id: 7, plantilla_id: null })).toBe(true);
    expect(esDeEstaFicha(p, { id: 8, plantilla_id: null })).toBe(false);
    expect(esDeEstaFicha(leer({ plantilla_id: UUID }), { id: 8, plantilla_id: UUID })).toBe(true);
  });

  it("aplicarla deja la identidad en la ficha", () => {
    const e = ficha();
    const { entidad } = aplicarAFicha(e, leer({ plantilla_id: UUID }), ctx);
    expect(entidad.plantilla_id).toBe(UUID);
    // Una plantilla sin identidad no la toca
    expect(aplicarAFicha({ ...e, plantilla_id: "otra-identidad-1" } as Entidad, leerPlantilla(exportar(null)), ctx).entidad.plantilla_id).toBe("otra-identidad-1");
  });
});

describe("archivos de texto de la plantilla", () => {
  const leer = (archivos: unknown) => leerPlantilla(exportar(null, { archivos }));

  it("el nombre siempre acaba en .txt, venga lo que venga", () => {
    expect(nombreTxt("Notas")).toBe("Notas.txt");
    expect(nombreTxt("Notas.TXT")).toBe("Notas.txt");
    expect(nombreTxt("virus.exe")).toBe("virus.txt");
    expect(nombreTxt("../../etc/passwd")).toMatch(/^[^/\\]+\.txt$/); // nunca una ruta
    expect(nombreTxt("a:b*c?d.pdf")).toBe("a_b_c_d.txt");
    expect(nombreTxt("  ")).toBe("Documento.txt");
    expect(nombreTxt(".txt")).toBe("Documento.txt");
    expect(nombreTxt("con")).toBe("_con.txt");
    expect(nombreTxt("a\u202eb\u0000c")).toBe("abc.txt");
    expect(nombreTxt("x".repeat(300))).toBe("x".repeat(100) + ".txt");
  });

  it("una plantilla sin archivos (de antes) no toca los de la ficha; una con lista vacía sí quiere dejarla sin ninguno", () => {
    expect(leerPlantilla(plantillaHtmlSinArchivos()).archivos).toBeNull();
    expect(leer([]).archivos).toEqual([]);
  });

  it("lee los archivos: fuerza .txt, evita repetidos y solo deja que un archivo reclame cada nombre original", () => {
    const a = leer([
      { original: "Notas.txt", nombre: "Diario.exe", contenido: "uno" },
      { original: "Notas.txt", nombre: "diario", contenido: "dos" },
      { original: null, nombre: "Otro", contenido: "tres" }
    ]).archivos!;
    expect(a.map((x) => x.nombre)).toEqual(["Diario.txt", "diario (2).txt", "Otro.txt"]);
    expect(a.map((x) => x.original)).toEqual(["Notas.txt", null, null]);
  });

  it("rechaza lo desmesurado", () => {
    expect(() => leer(Array.from({ length: 21 }, (_, i) => ({ original: null, nombre: `a${i}`, contenido: "" })))).toThrow(/más de 20/);
    expect(leer(Array.from({ length: 20 }, (_, i) => ({ original: null, nombre: `a${i}`, contenido: "" }))).archivos).toHaveLength(20);
    expect(leer([{ original: null, nombre: "justo", contenido: "x".repeat(100_000) }]).archivos).toHaveLength(1);
    expect(() => leer([{ original: null, nombre: "grande", contenido: "x".repeat(100_001) }])).toThrow(/pasa de/);
    expect(() => leer([{ nombre: 5 }])).toThrow(/dañada/);
  });

  it("al exportar de una ficha, cada archivo sale con su nombre como original y como actual", () => {
    const html = generarPlantilla(datosDePlantilla({
      definiciones: PARAMETROS, grupos: GRUPOS, variables: VARIABLES, conjunto: "X", entidad: ficha(), idFicha: 7,
      archivos: [{ original: "Notas.txt", nombre: "Notas.txt", contenido: "hola" }]
    }));
    expect(leerPlantilla(html).archivos).toEqual([{ original: "Notas.txt", nombre: "Notas.txt", contenido: "hola" }]);
  });
});

/** Una plantilla de antes de los archivos de texto: sin el campo. */
function plantillaHtmlSinArchivos(): string {
  const datos = { formato: "openroll-plantilla", version: 1, conjunto: "X", id_ficha: null, definiciones: {}, ficha: null, foto: null };
  return plantillaHtml.replace("__DATOS_OPENROLL__", JSON.stringify(datos));
}

describe("ficha única y valor actual", () => {
  const unica = () => ({ ...ficha(), es_unico: true, actuales: [{ id_definicion: 1, valor: 7 }] }) as Entidad;

  it("una ficha única exporta que lo es y el valor actual de sus parámetros con máximo; una que no lo es, no", () => {
    const leida = leerPlantilla(exportar(unica()));
    expect(leida.ficha?.unico).toBe(true);
    expect(leida.ficha?.parametros.find((p) => p.id === 1)).toMatchObject({ valor: 20, actual: 7 });
    expect(leida.ficha?.parametros.find((p) => p.id === 2)?.actual).toBeUndefined(); // sin máximo no hay «actual»
    const normal = leerPlantilla(exportar(ficha()));
    expect(normal.ficha?.unico).toBe(false);
    expect(normal.ficha?.parametros.every((p) => p.actual === undefined)).toBe(true);
  });

  it("lee lo que trae, acota el actual y se queda sin ello si falta o está roto", () => {
    const con = (extra: Record<string, unknown>) => {
      const e = exportar(null, { ficha: { nombre: "X", tipo: "Enemigo", cuerpo: [1, 1], vision: 0, parametros: [{ id: 1, clave: "Vida", valor: 5, ...extra }], variables: [], ...extra } });
      return leerPlantilla(e).ficha!;
    };
    expect(con({ actual: 99999 }).parametros[0].actual).toBe(16000);
    expect(con({ actual: -99999 }).parametros[0].actual).toBe(-16000);
    expect(con({ actual: "mucho" }).parametros[0].actual).toBeUndefined();
    expect(con({ unico: "sí" }).unico).toBeUndefined();
    expect(con({ unico: true }).unico).toBe(true);
  });

  it("al aplicarla pone si es única y los valores actuales (vida extra incluida), y quita ambos si no lo es", () => {
    const { entidad } = aplicarAFicha(ficha(), leerPlantilla(exportar(unica(), {})), ctx);
    expect(entidad.es_unico).toBe(true);
    expect(entidad.actuales).toEqual([{ id_definicion: 1, valor: 7 }]);
    const extra = leerPlantilla(exportar(unica()));
    extra.ficha!.parametros.find((p) => p.id === 1)!.actual = 31;
    expect(aplicarAFicha(ficha(), extra, ctx).entidad.actuales).toEqual([{ id_definicion: 1, valor: 31 }]);
    // Una plantilla de una ficha que no es única deja la ficha sin valores actuales
    const deVuelta = aplicarAFicha(unica(), leerPlantilla(exportar(ficha())), ctx).entidad;
    expect([deVuelta.es_unico, deVuelta.actuales]).toEqual([false, []]);
  });

  it("un jugador no puede hacer única una ficha con una plantilla, y una plantilla de antes no lo cambia", () => {
    const leida = leerPlantilla(exportar(unica()));
    const deJugador = aplicarAFicha(ficha(), leida, { ...ctx, puedeUnico: false }).entidad;
    expect([deJugador.es_unico, deJugador.actuales]).toEqual([false, []]);
    const vieja = leerPlantilla(exportar(unica()));
    delete vieja.ficha!.unico;
    expect(aplicarAFicha(unica(), vieja, ctx).entidad.es_unico).toBe(true);
  });
});
