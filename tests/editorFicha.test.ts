// Pruebas de la lógica de la ventana de ficha (src/lib/editorFicha.svelte.ts) con un Rust falso.
import { describe, expect, it, vi } from "vitest";

type Args = Record<string, any>;

const DIR = "archivos/entidad_7/";
let llamadas: { cmd: string; args: Args }[] = [];
let respuestas: Record<string, (a: Args) => any> = {};
let partidaFalsa: any;

const llamadasA = (cmd: string) => llamadas.filter((l) => l.cmd === cmd).map((l) => l.args);

// Los mismos que `parametros_por_defecto()` en Rust
const def = (id: number, clave: string, n: number, visibilidad: string, extra: Record<string, any> = {}) => ({
  id, clave, valor_por_defecto: { Numero: n }, tiene_maximo: false, representar_mapa: null, visibilidad, unidad: null, ...extra
});
const DEFINICIONES = [
  def(1, "Vida", 10, "Variable", { tiene_maximo: true }),
  def(2, "Armadura", 10, "Variable"),
  def(3, "Velocidad", 6, "SiemprePublico", { representar_mapa: { Rombo: [59, 130, 246, 90] }, unidad: "m" }),
  def(4, "Rango arma", 1, "SiemprePublico", { representar_mapa: { Circulo: [239, 68, 68, 90] }, unidad: "m" })
];

function entidad(extra: Record<string, any> = {}) {
  return {
    id: 7, nombre: "Orco", tipo: "Enemigo",
    parametros: [{ id_definicion: 1, valor: { Numero: 18 }, publico: true }],
    cuerpo: "Pequenio", vision: 0, zona_id: 1, foto_url: null, archivos_adjuntos: [], es_unico: false, ...extra
  } as any;
}

async function nuevoEditor(e = entidad(), esBorrador = false) {
  vi.resetModules();
  llamadas = [];
  respuestas = {
    leer_archivo_texto: (a) => `disco:${a.rutaRelativa}`,
    crear_documento_texto: () => ({ ruta: DIR + "Documento_1.txt" }),
    importar_archivo: (a) => ({ ruta: DIR + a.origen.split("/").pop() }),
    renombrar_archivo_adjunto: (a) => ({ ruta: DIR + a.nuevoNombre }),
    importar_imagen: (a) => "imagenes/" + a.origen.split("/").pop()
  };
  vi.doMock("@tauri-apps/api/core", () => ({
    invoke: async (cmd: string, args: Args = {}) => {
      llamadas.push({ cmd, args: structuredClone(args) });
      return respuestas[cmd]?.(args);
    },
    convertFileSrc: (p: string) => p
  }));
  partidaFalsa = {
    errorEditor: "",
    definiciones: structuredClone(DEFINICIONES),
    conjuntos: { activo: 0, guardados: [] },
    instancias: [] as any[],
    entidadEnEdicion: e,
    avisar: vi.fn(),
    guardarEntidad: vi.fn(async () => true),
    cancelarEditorEntidad: vi.fn(async () => {}),
    borrarEntidad: vi.fn(async () => {
      partidaFalsa.entidadEnEdicion = null;
    })
  };
  vi.doMock("$lib/partida.svelte", () => ({ partida: partidaFalsa, LIMITE_VALOR_ACTUAL: 16000 }));
  const { EditorFicha } = await import("$lib/editorFicha.svelte");
  return new EditorFicha(e, esBorrador);
}

describe("textos adjuntos", () => {
  it("al guardar solo escribe los .txt modificados que siguen adjuntos", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "a.txt", DIR + "b.txt", DIR + "c.txt"] }));
    await ed.abrirPestana(0); // leído pero sin tocar: no se reescribe
    await ed.abrirPestana(1);
    ed.escribir("nuevo b");
    await ed.abrirPestana(2);
    ed.escribir("nuevo c");
    ed.quitar(2); // modificado pero quitado: tampoco

    expect(await ed.guardar()).toBe(true);
    expect(llamadasA("guardar_archivo_texto").map((a) => [a.adjunto.ruta, a.contenido])).toEqual([
      [DIR + "b.txt", "nuevo b"]
    ]);
    expect(partidaFalsa.guardarEntidad).toHaveBeenCalledOnce();
  });

  it("un .txt que no se pudo leer no se puede editar (así no se pisa el archivo)", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "a.txt"] }));
    respuestas.leer_archivo_texto = () => {
      throw "sin permiso";
    };
    await ed.abrirPestana(0);
    ed.escribir("esto no debe llegar al disco");

    expect(ed.lecturaFallida(DIR + "a.txt")).toBe(true);
    expect(ed.error).toContain("sin permiso");
    await ed.guardar();
    expect(llamadasA("guardar_archivo_texto")).toEqual([]);
  });

  it("solo lee cada .txt una vez", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "a.txt", DIR + "b.pdf"] }));
    await ed.abrirPestana(0);
    await ed.abrirPestana(1);
    await ed.abrirPestana(0);
    expect(llamadasA("leer_archivo_texto")).toHaveLength(1);
    expect(ed.textos[DIR + "a.txt"]).toBe("disco:" + DIR + "a.txt");
  });

  it("al quitar una pestaña anterior a la activa, la activa sigue siendo el mismo archivo", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "a.txt", DIR + "b.txt", DIR + "c.txt"] }));
    await ed.abrirPestana(2);
    ed.quitar(0);
    expect(ed.pestana).toBe(1);
    expect(ed.rutaActiva).toBe(DIR + "c.txt");
    ed.quitar(1);
    expect(ed.rutaActiva).toBe(DIR + "b.txt");
  });
});

describe("renombrar", () => {
  it("un archivo ya guardado se copia (la copia es de la sesión); uno de la sesión se sustituye", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "viejo.txt"] }));
    await ed.renombrar(0, "nuevo.txt");
    await ed.crearTexto();
    await ed.renombrar(1, "notas.txt");

    expect(ed.adjuntos).toEqual([DIR + "nuevo.txt", DIR + "notas.txt"]);
    await ed.cancelar();
    expect(partidaFalsa.cancelarEditorEntidad).toHaveBeenCalledWith([DIR + "nuevo.txt", DIR + "notas.txt"]);
  });

  it("el texto editado se guarda con la ruta nueva", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "a.txt"] }));
    await ed.abrirPestana(0);
    ed.escribir("hola");
    await ed.renombrar(0, "b.txt");
    await ed.guardar();
    expect(llamadasA("guardar_archivo_texto").map((a) => [a.adjunto.ruta, a.contenido])).toEqual([
      [DIR + "b.txt", "hola"]
    ]);
  });

  it("no llama a Rust si el nombre está vacío o no cambia", async () => {
    const ed = await nuevoEditor(entidad({ archivos_adjuntos: [DIR + "a.txt"] }));
    await ed.renombrar(0, "  ");
    await ed.renombrar(0, "a.txt");
    expect(llamadasA("renombrar_archivo_adjunto")).toEqual([]);
  });
});

describe("guardar, cancelar y borrar", () => {
  it("al guardar descarta lo creado en la sesión (Rust conserva lo que se use)", async () => {
    const ed = await nuevoEditor(entidad({ foto_url: "imagenes/1.png" }), true);
    await ed.ponerFoto("/fotos/2.png");
    await ed.importarArchivos(["/docs/ficha.pdf"]);

    expect(ed.entidad.foto_url).toBe("imagenes/2.png");
    expect(ed.rutaActiva).toBe(DIR + "ficha.pdf");
    expect(await ed.guardar()).toBe(true);
    expect(llamadasA("descartar_archivos_borrador")).toEqual([
      { rutas: ["imagenes/1.png", "imagenes/2.png", DIR + "ficha.pdf"] }
    ]);
  });

  it("si Rust rechaza la ficha no se descarta nada", async () => {
    const ed = await nuevoEditor(entidad(), true);
    await ed.crearTexto();
    partidaFalsa.guardarEntidad.mockResolvedValueOnce(false);
    expect(await ed.guardar()).toBe(false);
    expect(llamadasA("descartar_archivos_borrador")).toEqual([]);
  });

  it("una ficha no válida no llega a Rust", async () => {
    const ed = await nuevoEditor(entidad({ nombre: "  ", archivos_adjuntos: [DIR + "a.txt"] }));
    await ed.abrirPestana(0);
    ed.escribir("x");
    expect(await ed.guardar()).toBe(false);
    expect(ed.error).toBe("La ficha necesita un nombre");

    ed.entidad.nombre = "Orco";
    ed.ponerNumero(1, "abc" as any); // lo que deja un input numérico raro
    await ed.guardar();
    expect(ed.error).toBe("Vida: debe ser un número entero");
    expect(llamadasA("guardar_archivo_texto")).toEqual([]);
    expect(partidaFalsa.guardarEntidad).not.toHaveBeenCalled();
  });

  it("borrar descarta lo de la sesión solo si la ficha se borró", async () => {
    const ed = await nuevoEditor();
    await ed.crearTexto();
    await ed.borrar();
    expect(partidaFalsa.borrarEntidad).toHaveBeenCalledOnce();
    expect(llamadasA("descartar_archivos_borrador")).toEqual([{ rutas: [DIR + "Documento_1.txt"] }]);
  });
});

describe("variables", () => {
  const VARIABLES = [
    { id: 1, nombre: "Fuerza", abreviatura: "f", neutro: 10, escala: 2, grupo: 1, bases: [], opcionales: [] },
    { id: 2, nombre: "Cordura", abreviatura: "cor", neutro: 7, escala: 3, grupo: 1, bases: [], opcionales: [] }
  ];
  const GRUPOS = [{ id: 1, nombre: "Variables", tipo: "Base" as const }];

  async function conVariables(e = entidad(), variables: any[] = VARIABLES, grupos: any[] = GRUPOS) {
    const ed = await nuevoEditor(e);
    const { EditorFicha } = await import("$lib/editorFicha.svelte");
    return new EditorFicha(ed.entidad, false, structuredClone(DEFINICIONES) as any, structuredClone(variables), structuredClone(grupos));
  }

  it("una ficha sin valores recibe el neutro de cada variable, en su orden", async () => {
    const ed = await conVariables(entidad({ variables: [{ id_definicion: 2, valor: 9 }, { id_definicion: 99, valor: 1 }] }));
    expect(ed.entidad.variables).toEqual([
      { id_definicion: 1, valor: 10, opcionales: [], eleccion: 0, parametros_marcados: [] },
      { id_definicion: 2, valor: 9, opcionales: [], eleccion: 0, parametros_marcados: [] }
    ]);
  });

  it("una derivada se recalcula sola al cambiar sus bases o marcar una opcional («+ com»)", async () => {
    const variables = [
      { id: 1, nombre: "Destreza", abreviatura: "des", neutro: 10, escala: 2, grupo: 1, bases: [], opcionales: [] },
      { id: 3, nombre: "Competencia", abreviatura: "com", neutro: 0, escala: 1, grupo: 1, bases: [], opcionales: [] },
      { id: 4, nombre: "Acrobacias", abreviatura: "acro", neutro: 0, escala: 1, grupo: 2, bases: [1], opcionales: [3] }
    ];
    const grupos = [...GRUPOS, { id: 2, nombre: "Habilidades", tipo: "Derivadas" as const }];
    const ed = await conVariables(entidad(), variables, grupos);
    const acrobacias = ed.variables[2];
    expect(ed.variablesDe(ed.gruposVariables[1]).map((d) => d.nombre)).toEqual(["Acrobacias"]);
    // Por defecto se ven todos los grupos («Todos»); al elegir uno solo el suyo, y si ese grupo desaparece, vuelve a «Todos»
    expect(ed.grupoVistaEfectivo).toBeNull();
    ed.grupoVista = 2;
    expect(ed.grupoVistaEfectivo).toBe(2);
    ed.grupoVista = 99;
    expect(ed.grupoVistaEfectivo).toBeNull();
    ed.grupoVista = null;
    ed.ponerValorVariable(1, 16);
    ed.ponerValorVariable(3, 2);
    expect(ed.resultadoVariable(acrobacias)).toBe(3);
    expect(ed.opcionalMarcada(4, 3)).toBe(false);
    ed.alternarOpcional(4, 3);
    expect(ed.opcionalMarcada(4, 3)).toBe(true);
    expect(ed.resultadoVariable(acrobacias)).toBe(5);
    expect(ed.desgloseVariable(acrobacias)).toBe("des (+3) + com (+2)");
    // Junto al nombre, de qué bases depende (sus abreviaturas); una base no depende de nada
    expect(ed.dependeDe(acrobacias)).toBe("des");
    expect(ed.dependeDe(ed.variables[0])).toBe("");
    ed.ponerValorVariable(1, 18); // cambia la base: la derivada la sigue
    expect(ed.resultadoVariable(acrobacias)).toBe(6);
    ed.alternarOpcional(4, 3);
    expect(ed.resultadoVariable(acrobacias)).toBe(4);
    // Guardar no exige valor a la derivada (se calcula) y lleva los checks
    ed.alternarOpcional(4, 3);
    expect(await ed.guardar()).toBe(true);
    expect(ed.entidad.variables.find((v: any) => v.id_definicion === 4)?.opcionales).toEqual([3]);
  });

  it("una derivada con rangos suma el que elige la ficha, y un parámetro calculado sale de las variables", async () => {
    const variables = [
      { id: 1, nombre: "Destreza", abreviatura: "des", neutro: 10, escala: 2, grupo: 1, bases: [], opcionales: [] },
      {
        id: 4, nombre: "Acrobacias", abreviatura: "acro", neutro: 0, escala: 1, grupo: 2, bases: [1], opcionales: [],
        opciones: [{ nombre: "Sin entrenar", bonus: 0 }, { nombre: "Entrenado", bonus: 2 }]
      }
    ];
    const grupos = [...GRUPOS, { id: 2, nombre: "Habilidades", tipo: "Derivadas" as const }];
    const ed0 = await nuevoEditor(entidad());
    const { EditorFicha } = await import("$lib/editorFicha.svelte");
    // El primer parámetro (el 1) se calcula: (Destreza) ÷ 2 + 1
    const defs: any = structuredClone(DEFINICIONES);
    defs[0].calculo = { variables: [1], multiplicador: 1, divisor: 2, constante: 1 };
    const ed = new EditorFicha(ed0.entidad, false, defs, structuredClone(variables), structuredClone(grupos));
    const acro = ed.variables[1];
    ed.ponerValorVariable(1, 16);
    expect(ed.resultadoVariable(acro)).toBe(3);
    expect(ed.eleccion(4)).toBe(0);
    ed.elegirOpcion(4, 1);
    expect(ed.resultadoVariable(acro)).toBe(5);
    expect(ed.desgloseVariable(acro)).toBe("des (+3) + Entrenado (+2)");
    // El parámetro calculado sigue a la variable y no se escribe
    expect(ed.esCalculado(defs[0].id)).toBe(true);
    expect(ed.numero(defs[0].id)).toBe(Math.floor(16 / 2) + 1);
    ed.ponerValorVariable(1, 20);
    expect(ed.numero(defs[0].id)).toBe(11);
    expect(ed.descripcionCalculo(defs[0].id)).toBe("Se calcula con las variables: (des) ÷2 + 1");
    expect(ed.esCalculado(defs[1].id)).toBe(false);
    expect(ed.descripcionCalculo(defs[1].id)).toBeUndefined();
  });

  it("el resultado se recalcula con el valor escrito y se redondea hacia abajo", async () => {
    const ed = await conVariables();
    ed.ponerValorVariable(1, 12);
    expect(ed.resultadoVariable(VARIABLES[0])).toBe(1);
    ed.ponerValorVariable(1, 9);
    expect(ed.resultadoVariable(VARIABLES[0])).toBe(-1);
    ed.ponerValorVariable(2, 9);
    expect(ed.resultadoVariable(VARIABLES[1])).toBe(0);
    ed.ponerValorVariable(1, null as any); // campo vacío
    expect(ed.resultadoVariable(VARIABLES[0])).toBeNull();
  });

  it("un valor que no es un entero o se pasa del tope no llega a Rust", async () => {
    const ed = await conVariables();
    ed.ponerValorVariable(1, 12.5);
    expect(await ed.guardar()).toBe(false);
    expect(ed.error).toBe("Fuerza: debe ser un número entero");
    ed.ponerValorVariable(1, 10000);
    expect(await ed.guardar()).toBe(false);
    expect(ed.error).toBe("Fuerza: no puede pasar de 9999");
    ed.ponerValorVariable(1, -3); // negativo sí vale
    expect(await ed.guardar()).toBe(true);
    expect(partidaFalsa.guardarEntidad).toHaveBeenCalledOnce();
  });

  it("una ficha de antes de las variables no se rompe sin variables definidas", async () => {
    const ed = await nuevoEditor();
    expect(ed.variables).toEqual([]);
    expect(ed.entidad.variables).toEqual([]);
  });
});

describe("parámetros", () => {
  it("rellena los que faltan con su valor por defecto y en el orden de las definiciones", async () => {
    const ed = await nuevoEditor();
    expect(ed.entidad.parametros).toEqual([
      { id_definicion: 1, valor: { Numero: 18 }, publico: true },
      { id_definicion: 2, valor: { Numero: 10 }, publico: false },
      { id_definicion: 3, valor: { Numero: 6 }, publico: true }, // visibilidad fija: siempre público
      { id_definicion: 4, valor: { Numero: 1 }, publico: true }
    ]);
  });

  it("sin definiciones (no se pudieron leer) no toca los parámetros de la ficha", async () => {
    vi.resetModules();
    const e = entidad();
    vi.doMock("$lib/partida.svelte", () => ({ partida: { definiciones: [] } }));
    const { EditorFicha } = await import("$lib/editorFicha.svelte");
    new EditorFicha(e, false);
    expect(e.parametros).toHaveLength(1);
  });

  it("agrupa los numéricos de dos en dos y deja aparte los de texto", async () => {
    const ed = await nuevoEditor();
    expect(ed.campos.numericos.map((par) => par.map((d) => d.clave))).toEqual([
      ["Vida", "Armadura"],
      ["Velocidad", "Rango arma"]
    ]);
    expect(ed.campos.textos).toEqual([]);
  });

  it("lee y escribe valores y visibilidad por id de definición", async () => {
    const ed = await nuevoEditor();
    ed.ponerNumero(2, 4);
    ed.ponerPublico(2, true);
    expect([ed.numero(2), ed.esPublico(2), ed.numero(99)]).toEqual([4, true, 0]);
    expect(await ed.guardar()).toBe(true);
    expect(partidaFalsa.guardarEntidad.mock.calls[0][0].parametros[1]).toEqual({
      id_definicion: 2, valor: { Numero: 4 }, publico: true
    });
  });
});

describe("cuerpo", () => {
  it("1×1 es Pequenio, lo demás Grande, y los valores se acotan a 1..255", async () => {
    const ed = await nuevoEditor();
    ed.cuerpoX = 3;
    expect(ed.entidad.cuerpo).toEqual({ Grande: [3, 1] });
    ed.cuerpoY = 999;
    expect(ed.entidad.cuerpo).toEqual({ Grande: [3, 255] });
    ed.cuerpoX = 1;
    ed.cuerpoY = null as any; // input vacío
    expect(ed.entidad.cuerpo).toBe("Pequenio");
    expect([ed.cuerpoX, ed.cuerpoY]).toEqual([1, 1]);
  });
});

describe("archivos de texto de una plantilla", () => {
  const plantilla = (archivos: { original: string | null; nombre: string; contenido: string }[] | null) =>
    ({
      conjunto: "", id_ficha: null, plantilla_id: null, foto: null, archivos,
      ficha: { nombre: "Orco", tipo: "Enemigo", cuerpo: [1, 1], vision: 0, parametros: [], variables: [] },
      nombresVariables: new Map(), nombresParametros: new Map()
    }) as any;
  const base = () => entidad({ archivos_adjuntos: [DIR + "a.txt", DIR + "b.txt", DIR + "mapa.pdf"] });

  it("renombra, reescribe, crea y quita; lo que no es .txt no se toca", async () => {
    const ed = await nuevoEditor(base());
    await ed.aplicarPlantilla(
      plantilla([
        { original: "a.txt", nombre: "alfa.txt", contenido: "nuevo a" }, // renombrado y editado
        { original: null, nombre: "nueva.txt", contenido: "hola" } // creado en la plantilla; b.txt ya no está: se quita
      ])
    );
    expect(llamadasA("renombrar_archivo_adjunto").map((a) => [a.adjunto.ruta, a.nuevoNombre])).toEqual([
      [DIR + "a.txt", "alfa.txt"],
      [DIR + "Documento_1.txt", "nueva.txt"]
    ]);
    expect(ed.adjuntos).toEqual([DIR + "alfa.txt", DIR + "mapa.pdf", DIR + "nueva.txt"]);
    expect(await ed.guardar()).toBe(true);
    expect(llamadasA("guardar_archivo_texto").map((a) => [a.adjunto.ruta, a.contenido])).toEqual([
      [DIR + "alfa.txt", "nuevo a"],
      [DIR + "nueva.txt", "hola"]
    ]);
  });

  it("importar otra vez el mismo archivo no vuelve a renombrar ni a crear nada", async () => {
    const ed = await nuevoEditor(base());
    const p = plantilla([
      { original: "a.txt", nombre: "alfa.txt", contenido: "nuevo a" },
      { original: null, nombre: "nueva.txt", contenido: "hola" }
    ]);
    await ed.aplicarPlantilla(p);
    const renombres = llamadasA("renombrar_archivo_adjunto").length;
    const creados = llamadasA("crear_documento_texto").length;
    await ed.aplicarPlantilla(p);
    expect(llamadasA("renombrar_archivo_adjunto")).toHaveLength(renombres);
    expect(llamadasA("crear_documento_texto")).toHaveLength(creados);
    expect(ed.adjuntos).toEqual([DIR + "alfa.txt", DIR + "mapa.pdf", DIR + "nueva.txt"]);
  });

  it("una plantilla en blanco con los .txt por defecto, sobre una ficha nueva que ya los trae, no crea ni renombra nada", async () => {
    const ed = await nuevoEditor(base());
    await ed.aplicarPlantilla(plantilla([
      { original: null, nombre: "a.txt", contenido: "disco:" + DIR + "a.txt" },
      { original: null, nombre: "b.txt", contenido: "disco:" + DIR + "b.txt" }
    ]));
    expect(llamadasA("crear_documento_texto")).toEqual([]);
    expect(llamadasA("renombrar_archivo_adjunto")).toEqual([]);
    expect(ed.adjuntos).toEqual([DIR + "a.txt", DIR + "b.txt", DIR + "mapa.pdf"]);
  });

  it("un archivo sin cambios no se reescribe, y una plantilla sin archivos no toca ninguno", async () => {
    const ed = await nuevoEditor(base());
    await ed.aplicarPlantilla(plantilla([
      { original: "a.txt", nombre: "a.txt", contenido: "disco:" + DIR + "a.txt" },
      { original: "b.txt", nombre: "b.txt", contenido: "disco:" + DIR + "b.txt" }
    ]));
    expect(await ed.guardar()).toBe(true);
    expect(llamadasA("guardar_archivo_texto")).toEqual([]);
    expect(llamadasA("renombrar_archivo_adjunto")).toEqual([]);

    const ed2 = await nuevoEditor(base());
    await ed2.aplicarPlantilla(plantilla(null));
    expect(ed2.adjuntos).toEqual([DIR + "a.txt", DIR + "b.txt", DIR + "mapa.pdf"]);
  });

  it("una plantilla con la lista vacía quita todos los .txt de la ficha (los demás archivos se quedan)", async () => {
    const ed = await nuevoEditor(base());
    await ed.aplicarPlantilla(plantilla([]));
    expect(ed.adjuntos).toEqual([DIR + "mapa.pdf"]);
  });

  it("al exportar sale el contenido de cada .txt, incluido lo escrito sin guardar", async () => {
    const ed = await nuevoEditor(base());
    await ed.abrirPestana(0);
    ed.escribir("sin guardar");
    expect(await ed.archivosParaPlantilla()).toEqual([
      { original: "a.txt", nombre: "a.txt", contenido: "sin guardar" },
      { original: "b.txt", nombre: "b.txt", contenido: "disco:" + DIR + "b.txt" }
    ]);
  });
});

describe("valor actual de una ficha única", () => {
  const vida = (e: any) => e.parametros.find((p: any) => p.id_definicion === 1);

  it("solo una ficha única enseña actual y máximo, y solo en los parámetros numéricos con máximo", async () => {
    const ed = await nuevoEditor(entidad());
    const [pg, armadura] = ed.definiciones;
    expect([ed.muestraActual(pg), ed.muestraActual(armadura)]).toEqual([false, false]);
    ed.entidad.es_unico = true;
    expect([ed.muestraActual(pg), ed.muestraActual(armadura)]).toEqual([true, false]);
  });

  it("sin escribir, el actual es el de su instancia si la tiene y, si no, el máximo", async () => {
    const ed = await nuevoEditor(entidad({ es_unico: true }));
    expect(ed.actual(1)).toBe(18);
    partidaFalsa.instancias = [{ base: { id: 7 }, valores_actuales: [{ id_definicion: 1, valor: 5 }] }];
    expect(ed.actual(1)).toBe(5);
    ed.ponerActual(1, 25); // vida extra: pasa del máximo
    expect(ed.actual(1)).toBe(25);
  });

  it("al guardar manda los actuales de una única (acotados) y nada si no lo es", async () => {
    const ed = await nuevoEditor(entidad({ es_unico: true }));
    ed.ponerActual(1, 99999);
    ed.ponerActual(2, 4); // la armadura no tiene máximo: no vale
    expect(await ed.guardar()).toBe(true);
    expect(partidaFalsa.guardarEntidad).toHaveBeenCalled();
    expect(ed.entidad.actuales).toEqual([{ id_definicion: 1, valor: 16000 }]);

    const otra = await nuevoEditor(entidad({ es_unico: false, actuales: [{ id_definicion: 1, valor: 3 }] }));
    expect(await otra.guardar()).toBe(true);
    expect(otra.entidad.actuales).toEqual([]);
  });

  it("una plantilla marca la ficha como única y trae su valor actual; un jugador no la hace única", async () => {
    const plantilla = (unico: boolean) =>
      ({
        conjunto: "", id_ficha: null, plantilla_id: null, foto: null, archivos: null,
        ficha: { nombre: "Orco", tipo: "Enemigo", cuerpo: [1, 1], vision: 0, unico, parametros: [{ id: 1, clave: "Vida", valor: 18, actual: 7 }], variables: [] },
        nombresVariables: new Map(), nombresParametros: new Map()
      }) as any;
    const ed = await nuevoEditor(entidad());
    await ed.aplicarPlantilla(plantilla(true));
    expect(ed.entidad.es_unico).toBe(true);
    expect(ed.entidad.actuales).toEqual([{ id_definicion: 1, valor: 7 }]);
    expect(ed.actual(1)).toBe(7);
    await ed.aplicarPlantilla(plantilla(false));
    expect(ed.entidad.es_unico).toBe(false);
    expect(ed.entidad.actuales).toEqual([]);
    // Al exportar, una única lleva su valor actual
    ed.entidad.es_unico = true;
    ed.ponerActual(1, 9);
    expect(ed.entidadParaPlantilla().actuales).toEqual([{ id_definicion: 1, valor: 9 }]);
  });
});
