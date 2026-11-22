// src/lib/plantillaFicha.ts
//
// La plantilla de ficha: un HTML autocontenido (src/lib/plantilla/plantilla.html) que se rellena FUERA de la app con cualquier
// navegador y guarda sus datos dentro de sí mismo; después ese mismo archivo se arrastra a la app (al banco o a la ventana de una
// ficha). Aquí está lo puro: generar el HTML con los datos, leerlo (es un archivo de fuera: entrada NO fiable) y pasar sus datos a
// una ficha. Lo que toca disco, red o pantalla está en `plantillaIo.ts`.
import plantillaHtml from "./plantilla/plantilla.html?raw";
import type {
  Cuerpo,
  DefinicionParametro,
  DefinicionVariable,
  Entidad,
  GrupoVariables,
  TipoEntidad,
  ValorActual,
  ValorVariable
} from "./partida.svelte";
import { esDerivada } from "./variables";

export const FORMATO_PLANTILLA = "openroll-plantilla";
export const VERSION_PLANTILLA = 1;
/** Donde van los datos dentro del HTML. */
const MARCA_DATOS = "__DATOS_OPENROLL__";
/** Tope del archivo que se acepta (la foto va entera dentro). */
export const MAX_BYTES_PLANTILLA = 16 * 1024 * 1024;
/** Tope de la foto ya decodificada (el de una subida: `imagenes::MAX_BYTES_SUBIDA`). */
export const MAX_BYTES_FOTO = 8 * 1024 * 1024;
/** Formatos de foto que acepta la app (rasterizados: nada de SVG). */
export const MIME_FOTO: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/webp": "webp",
  "image/gif": "gif",
  "image/bmp": "bmp"
};
/** Tope de archivos de texto de una ficha y de caracteres de cada uno (`MAX_ARCHIVOS_TEXTO` y `MAX_CARACTERES_TEXTO` en Rust: el mismo para todo el programa). */
export const MAX_ARCHIVOS_TEXTO = 20;
export const MAX_CARACTERES_ARCHIVO = 100_000;
/** Caracteres del nombre de un archivo de texto, sin la extensión (`MAX_NOMBRE_ARCHIVO` en Rust). */
export const MAX_NOMBRE_ARCHIVO_TEXTO = 100;
const MAX_STEM_ARCHIVO = 100;
const RESERVADOS_WINDOWS = new Set([
  "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9",
  "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"
]);
const TIPOS: TipoEntidad[] = ["Enemigo", "NPC", "Jugador", "Objeto"];
const MAX_NOMBRE = 100;
const MAX_TEXTO = 2000;
const MAX_ELEMENTOS = 200;
const LIMITE_VARIABLE = 9999;
const MAX_PARAMETRO = 32767;
const MAX_CUERPO = 255;
const MAX_VISION = 10000;
/** Tope de un valor actual, en positivo y en negativo (`LIMITE_VALOR_ACTUAL` en Rust). */
const LIMITE_ACTUAL = 16000;

export interface EncuadrePlantilla {
  x: number;
  y: number;
  zoom: number;
}

export interface FotoPlantilla {
  mime: string;
  /** La imagen en base64 (sin el prefijo `data:`). */
  datos: string;
  encuadre: EncuadrePlantilla;
}

export interface FichaPlantilla {
  nombre: string;
  tipo: TipoEntidad;
  cuerpo: [number, number];
  vision: number;
  /** ¿Es una ficha única (una sola instancia en el mundo)? Solo la importa el host. Una plantilla de antes no lo trae. */
  unico?: boolean;
  /** `actual`: solo en una ficha única y en un parámetro con máximo (`valor` es el máximo): el valor actual, que puede pasar del máximo. */
  parametros: { id: number; clave: string; valor: number | string; publico?: boolean; actual?: number }[];
  variables: {
    id: number;
    nombre: string;
    valor: number;
    eleccion: number;
    eleccion_nombre: string;
    opcionales: number[];
    parametros_marcados: number[];
  }[];
}

/**
 * Un archivo de texto de la ficha dentro de la plantilla. `original` es el nombre que tenía en la ficha al exportar (null: se creó en la
 * plantilla) y `nombre` el de ahora: si son distintos, es un cambio de nombre y la app sabe qué archivo renombrar. El contenido va
 * dentro del HTML. Siempre `.txt`.
 */
export interface ArchivoPlantilla {
  original: string | null;
  nombre: string;
  contenido: string;
}

/** Lo que va dentro del HTML. */
export interface DatosPlantilla {
  formato: string;
  version: number;
  /** El conjunto de parámetros con el que se hizo (solo para avisar si al importar es otro). */
  conjunto: string;
  /** La ficha de la que salió (null: en blanco, o de una ficha nueva sin guardar). */
  id_ficha: number | null;
  /** La identidad de este ARCHIVO: un UUID que genera la propia plantilla la primera vez que se guarda (null hasta entonces, o en una
   *  plantilla en blanco recién exportada). La ficha que se crea con él lo guarda, así se reconoce si vuelve a importarse. */
  plantilla_id: string | null;
  definiciones: { parametros: DefinicionParametro[]; grupos_variables: GrupoVariables[]; variables: DefinicionVariable[] };
  ficha: FichaPlantilla | null;
  foto: FotoPlantilla | null;
  /** Los archivos de texto de la ficha. Una plantilla de antes de este apartado no lo trae: en ese caso no se tocan los de la ficha. */
  archivos: ArchivoPlantilla[];
}

/** Lo que se queda de una plantilla leída: sus datos y, de sus definiciones, solo los nombres (para emparejar por nombre). */
export interface PlantillaLeida {
  conjunto: string;
  id_ficha: number | null;
  plantilla_id: string | null;
  ficha: FichaPlantilla | null;
  foto: FotoPlantilla | null;
  /** `null` = la plantilla no lleva archivos de texto (no se toca ninguno). */
  archivos: ArchivoPlantilla[] | null;
  nombresVariables: Map<number, string>;
  nombresParametros: Map<number, string>;
}

// ------------------------------------------------------------------
// Exportar
// ------------------------------------------------------------------


const ladosDe = (c: Cuerpo): [number, number] => (typeof c === "object" ? [c.Grande[0] ?? 1, c.Grande[1] ?? 1] : [1, 1]);

/** Los datos de la plantilla: las definiciones de la partida y, si sale de una ficha, sus valores y su foto. */
export function datosDePlantilla(p: {
  definiciones: DefinicionParametro[];
  grupos: GrupoVariables[];
  variables: DefinicionVariable[];
  conjunto: string;
  entidad?: Entidad | null;
  idFicha?: number | null;
  foto?: FotoPlantilla | null;
  archivos?: ArchivoPlantilla[];
}): DatosPlantilla {
  const e = p.entidad ?? null;
  return {
    formato: FORMATO_PLANTILLA,
    version: VERSION_PLANTILLA,
    conjunto: p.conjunto,
    id_ficha: p.idFicha ?? null,
    // Desde una ficha que ya tiene identidad, la lleva; en blanco no: la plantilla se la pone al guardarse por primera vez
    plantilla_id: e?.plantilla_id ?? null,
    definiciones: { parametros: p.definiciones, grupos_variables: p.grupos, variables: p.variables },
    ficha: e
      ? {
          nombre: e.nombre,
          tipo: e.tipo,
          cuerpo: ladosDe(e.cuerpo),
          vision: e.vision ?? 0,
          unico: !!e.es_unico,
          parametros: (e.parametros ?? []).map((x) => ({
            id: x.id_definicion,
            clave: p.definiciones.find((d) => d.id === x.id_definicion)?.clave ?? "",
            valor: "Numero" in x.valor ? x.valor.Numero : x.valor.Texto,
            publico: x.publico,
            // El valor actual solo existe en una ficha única (la otra tiene varias instancias, cada una con el suyo)
            actual: e.es_unico && p.definiciones.find((d) => d.id === x.id_definicion)?.tiene_maximo ? e.actuales?.find((a) => a.id_definicion === x.id_definicion)?.valor : undefined
          })),
          variables: (e.variables ?? []).map((v) => {
            const d = p.variables.find((x) => x.id === v.id_definicion);
            return {
              id: v.id_definicion,
              nombre: d?.nombre ?? "",
              valor: v.valor,
              eleccion: v.eleccion ?? 0,
              eleccion_nombre: d?.opciones?.[v.eleccion ?? 0]?.nombre ?? "",
              opcionales: v.opcionales ?? [],
              parametros_marcados: v.parametros_marcados ?? []
            };
          })
        }
      : null,
    foto: p.foto ?? null,
    archivos: p.archivos ?? []
  };
}

/** El HTML de la plantilla con los datos dentro. `<` va escrito `<` (sigue siendo el mismo JSON), así ningún nombre puede
 *  cerrar el `<script>` donde van. */
export function generarPlantilla(datos: DatosPlantilla): string {
  const json = JSON.stringify(datos).replace(/</g, "\\u003c");
  return plantillaHtml.replace(MARCA_DATOS, () => json);
}

// ------------------------------------------------------------------
// Leer (un archivo de fuera: nada se da por bueno)
// ------------------------------------------------------------------

const esEntero = (n: unknown): n is number => typeof n === "number" && Number.isInteger(n);
const acotar = (n: number, min: number, max: number) => Math.min(max, Math.max(min, Math.round(n)));
const texto = (v: unknown, max: number, que: string): string => {
  if (typeof v !== "string") throw new Error(`La plantilla está dañada (${que})`);
  return v.slice(0, max);
};
const enteros = (v: unknown, que: string): number[] => {
  if (!Array.isArray(v) || v.length > MAX_ELEMENTOS || !v.every(esEntero)) throw new Error(`La plantilla está dañada (${que})`);
  return v as number[];
};
const lista = (v: unknown, que: string): unknown[] => {
  if (!Array.isArray(v) || v.length > MAX_ELEMENTOS) throw new Error(`La plantilla está dañada (${que})`);
  return v;
};
const objeto = (v: unknown, que: string): Record<string, unknown> => {
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new Error(`La plantilla está dañada (${que})`);
  return v as Record<string, unknown>;
};

/**
 * El nombre de un archivo de texto como se acepta: SIEMPRE `.txt`. Quita lo que no es un nombre (rutas, caracteres prohibidos, control
 * e invisibles), cambia cualquier otra extensión por `.txt` (o la añade) y esquiva los nombres reservados de Windows. Es la defensa de
 * la app por si alguien edita la plantilla a mano; la plantilla ya no deja escribir otra cosa.
 */
export function nombreTxt(nombre: string): string {
  let n = String(nombre)
    .replace(/[\u0000-\u001f\u007f\u200b-\u200f\u202a-\u202e\u2066-\u2069\ufeff]/g, "")
    .replace(/[\\/:*?"<>|]+/g, "_")
    .trim();
  const conTxt = /^(.*)\.txt$/i.exec(n);
  const conOtra = /^(.*\S)\.[^.\s]+$/.exec(n);
  n = conTxt ? conTxt[1] : conOtra ? conOtra[1] : n;
  n = n.replace(/[.\s]+$/, "").slice(0, MAX_STEM_ARCHIVO).replace(/[.\s]+$/, "");
  if (!n) n = "Documento";
  if (RESERVADOS_WINDOWS.has(n.toLowerCase())) n = `_${n}`;
  return `${n}.txt`;
}

/** Un nombre que no choque (sin mirar mayúsculas) con los `usados`: «Notas.txt» → «Notas (2).txt». */
function nombreLibre(nombre: string, usados: Set<string>): string {
  let candidato = nombre;
  for (let n = 2; usados.has(candidato.toLowerCase()); n++) candidato = nombre.replace(/\.txt$/i, ` (${n}).txt`);
  return candidato;
}

/** ¿Parece una plantilla de ficha? (por la extensión: el contenido lo comprueba `leerPlantilla`) */
export const esArchivoPlantilla = (nombre: string) => /\.html?$/i.test(nombre.trim());

/** Saca y comprueba los datos de una plantilla. Lanza un error con el motivo si no es una plantilla de OpenRoll o está dañada. */
export function leerPlantilla(html: string): PlantillaLeida {
  if (html.length > MAX_BYTES_PLANTILLA) throw new Error(`La plantilla no puede pasar de ${MAX_BYTES_PLANTILLA / (1024 * 1024)} MB`);
  // Solo se lee el bloque de datos: nada del archivo se ejecuta
  const m = /<script[^>]*\bid=["']datos-openroll["'][^>]*>([\s\S]*?)<\/script>/i.exec(html);
  if (!m) throw new Error("Ese archivo no es una plantilla de ficha de OpenRoll");
  let crudo: unknown;
  try {
    crudo = JSON.parse(m[1]);
  } catch {
    throw new Error("La plantilla está dañada");
  }
  const d = objeto(crudo, "datos");
  if (d.formato !== FORMATO_PLANTILLA) throw new Error("Ese archivo no es una plantilla de ficha de OpenRoll");
  if (!esEntero(d.version) || d.version < 1) throw new Error("La plantilla está dañada (versión)");
  if (d.version > VERSION_PLANTILLA) throw new Error("La plantilla es de una versión más nueva de OpenRoll: actualiza la app");
  const idFicha = d.id_ficha === null || d.id_ficha === undefined ? null : esEntero(d.id_ficha) ? d.id_ficha : null;
  const plantillaId = typeof d.plantilla_id === "string" && /^[A-Za-z0-9-]{8,64}$/.test(d.plantilla_id) ? d.plantilla_id : null;

  // De las definiciones con que se hizo solo interesan los nombres (para emparejar si el conjunto cambió)
  const nombresVariables = new Map<number, string>();
  const nombresParametros = new Map<number, string>();
  if (d.definiciones && typeof d.definiciones === "object") {
    const defs = d.definiciones as Record<string, unknown>;
    for (const v of Array.isArray(defs.variables) ? defs.variables.slice(0, MAX_ELEMENTOS) : []) {
      if (v && esEntero((v as { id: unknown }).id) && typeof (v as { nombre: unknown }).nombre === "string") {
        nombresVariables.set((v as { id: number }).id, (v as { nombre: string }).nombre.slice(0, MAX_NOMBRE));
      }
    }
    for (const p of Array.isArray(defs.parametros) ? defs.parametros.slice(0, MAX_ELEMENTOS) : []) {
      if (p && esEntero((p as { id: unknown }).id) && typeof (p as { clave: unknown }).clave === "string") {
        nombresParametros.set((p as { id: number }).id, (p as { clave: string }).clave.slice(0, MAX_NOMBRE));
      }
    }
  }

  return {
    conjunto: typeof d.conjunto === "string" ? d.conjunto.slice(0, MAX_NOMBRE) : "",
    id_ficha: idFicha,
    plantilla_id: plantillaId,
    ficha: d.ficha === null || d.ficha === undefined ? null : leerFicha(objeto(d.ficha, "ficha")),
    foto: d.foto === null || d.foto === undefined ? null : leerFoto(objeto(d.foto, "foto")),
    archivos: d.archivos === null || d.archivos === undefined ? null : leerArchivos(d.archivos),
    nombresVariables,
    nombresParametros
  };
}

function leerFicha(f: Record<string, unknown>): FichaPlantilla {
  const tipo = TIPOS.includes(f.tipo as TipoEntidad) ? (f.tipo as TipoEntidad) : "Jugador";
  const cuerpo = Array.isArray(f.cuerpo) && f.cuerpo.length === 2 && f.cuerpo.every(esEntero) ? (f.cuerpo as number[]) : [1, 1];
  return {
    nombre: texto(f.nombre ?? "", MAX_NOMBRE, "nombre").trim(),
    tipo,
    cuerpo: [acotar(cuerpo[0], 1, MAX_CUERPO), acotar(cuerpo[1], 1, MAX_CUERPO)],
    vision: esEntero(f.vision) ? acotar(f.vision, 0, MAX_VISION) : 0,
    unico: typeof f.unico === "boolean" ? f.unico : undefined,
    parametros: lista(f.parametros ?? [], "parámetros").map((x) => {
      const p = objeto(x, "parámetro");
      if (!esEntero(p.id)) throw new Error("La plantilla está dañada (parámetro)");
      const valor =
        typeof p.valor === "number" && Number.isFinite(p.valor)
          ? acotar(p.valor, 0, MAX_PARAMETRO)
          : texto(p.valor ?? "", MAX_TEXTO, "parámetro");
      const actual = typeof p.actual === "number" && Number.isFinite(p.actual) ? acotar(p.actual, -LIMITE_ACTUAL, LIMITE_ACTUAL) : undefined;
      return { id: p.id, clave: texto(p.clave ?? "", MAX_NOMBRE, "parámetro"), valor, publico: typeof p.publico === "boolean" ? p.publico : undefined, actual };
    }),
    variables: lista(f.variables ?? [], "variables").map((x) => {
      const v = objeto(x, "variable");
      if (!esEntero(v.id)) throw new Error("La plantilla está dañada (variable)");
      return {
        id: v.id,
        nombre: texto(v.nombre ?? "", MAX_NOMBRE, "variable"),
        valor: esEntero(v.valor) ? acotar(v.valor, -LIMITE_VARIABLE, LIMITE_VARIABLE) : 0,
        eleccion: esEntero(v.eleccion) ? acotar(v.eleccion, 0, 100) : 0,
        eleccion_nombre: texto(v.eleccion_nombre ?? "", MAX_NOMBRE, "variable"),
        opcionales: enteros(v.opcionales ?? [], "variable"),
        parametros_marcados: enteros(v.parametros_marcados ?? [], "variable")
      };
    })
  };
}

function leerArchivos(v: unknown): ArchivoPlantilla[] {
  if (!Array.isArray(v) || v.length > MAX_ARCHIVOS_TEXTO) throw new Error(`La plantilla no puede llevar más de ${MAX_ARCHIVOS_TEXTO} archivos de texto`);
  const usados = new Set<string>();
  const originales = new Set<string>();
  return v.map((x) => {
    const a = objeto(x, "archivo");
    const contenido = typeof a.contenido === "string" ? a.contenido : "";
    if (typeof a.nombre !== "string") throw new Error("La plantilla está dañada (archivo)");
    // Siempre .txt y sin repetirse (dos archivos no pueden llamarse igual)
    const nombre = nombreLibre(nombreTxt(a.nombre), usados);
    if (contenido.length > MAX_CARACTERES_ARCHIVO) throw new Error(`El archivo «${nombre}» de la plantilla pasa de ${MAX_CARACTERES_ARCHIVO / 1000} mil caracteres`);
    usados.add(nombre.toLowerCase());
    // El nombre que tenía en la ficha: sirve para saber a qué archivo se refiere; uno solo puede reclamarlo
    let original: string | null = typeof a.original === "string" && a.original.trim() ? nombreTxt(a.original) : null;
    if (original && originales.has(original.toLowerCase())) original = null;
    if (original) originales.add(original.toLowerCase());
    return { original, nombre, contenido };
  });
}

function leerFoto(f: Record<string, unknown>): FotoPlantilla {
  const mime = texto(f.mime, 40, "foto");
  const datos = texto(f.datos, Math.ceil((MAX_BYTES_FOTO * 4) / 3) + 8, "foto");
  if (datos.length * 0.75 > MAX_BYTES_FOTO + 4) throw new Error(`La foto de la plantilla no puede pasar de ${MAX_BYTES_FOTO / (1024 * 1024)} MB`);
  if (!/^[A-Za-z0-9+/=\s]*$/.test(datos)) throw new Error("La plantilla está dañada (foto)");
  const e = f.encuadre && typeof f.encuadre === "object" ? (f.encuadre as Record<string, unknown>) : {};
  const n = (v: unknown, def: number, min: number, max: number) => (typeof v === "number" && Number.isFinite(v) ? acotar(v, min, max) : def);
  return { mime, datos, encuadre: { x: n(e.x, 50, 0, 100), y: n(e.y, 50, 0, 100), zoom: n(e.zoom, 100, 100, 400) } };
}

// ------------------------------------------------------------------
// Aplicar a una ficha
// ------------------------------------------------------------------

/** La ficha de `fichas` a la que corresponde la plantilla: la que guarda su identidad (si hay exactamente una) o, si no, la del `id_ficha`
 *  con que se exportó. `undefined` si no es de ninguna (plantilla nueva, o de otra partida). */
export function fichaDePlantilla(p: Pick<PlantillaLeida, "plantilla_id" | "id_ficha">, fichas: readonly Entidad[]): Entidad | undefined {
  if (p.plantilla_id) {
    const iguales = fichas.filter((f) => f.plantilla_id === p.plantilla_id);
    if (iguales.length === 1) return iguales[0];
  }
  return p.id_ficha === null ? undefined : fichas.find((f) => f.id === p.id_ficha);
}

/** ¿Es la plantilla de ESTA ficha? (misma identidad, o la ficha de la que se exportó) */
export function esDeEstaFicha(p: Pick<PlantillaLeida, "plantilla_id" | "id_ficha">, ficha: Pick<Entidad, "id" | "plantilla_id">): boolean {
  return (!!p.plantilla_id && ficha.plantilla_id === p.plantilla_id) || p.id_ficha === ficha.id;
}

const clave = (s: string) => s.trim().replace(/\s+/g, " ").toLowerCase();

/** La definición que corresponde a un valor de la plantilla: la del mismo id Y nombre; si no, la del mismo nombre; y solo si la
 *  plantilla es del mismo conjunto, la del mismo id aunque se haya renombrado. */
function emparejar<T extends { id: number }>(defs: T[], nombre: (d: T) => string, id: number, nombrePlantilla: string, mismoConjunto: boolean): T | undefined {
  const k = clave(nombrePlantilla);
  return (
    defs.find((d) => d.id === id && clave(nombre(d)) === k) ??
    (k ? defs.find((d) => clave(nombre(d)) === k) : undefined) ??
    (mismoConjunto ? defs.find((d) => d.id === id) : undefined)
  );
}

/**
 * Los datos de la plantilla sobre una COPIA de `entidad` (ya normalizada: un parámetro y una variable por definición). Cambia nombre,
 * tipo, cuerpo, visión, los parámetros que se escriben a mano (no los calculados) con su «Hacer público» (solo en los de visibilidad variable) y las variables (valor de
 * las bases; rango, opcionales y parámetros marcados de las derivadas, solo los que lo sigan siendo) y, si es única (solo lo cambia el
 * host: `ctx.puedeUnico`), si lo es y el valor actual de sus parámetros con máximo. No toca zona, propietarios, adjuntos, pasos ni la
 * foto (la pone el editor). `avisos`: lo que no se pudo pasar.
 */
export function aplicarAFicha(
  entidad: Entidad,
  plantilla: PlantillaLeida,
  ctx: { definiciones: DefinicionParametro[]; variables: DefinicionVariable[]; conjunto: string; puedeUnico?: boolean }
): { entidad: Entidad; avisos: string[] } {
  const e: Entidad = structuredClone(entidad);
  const avisos: string[] = [];
  const f = plantilla.ficha;
  const mismoConjunto = !plantilla.conjunto || !ctx.conjunto || clave(plantilla.conjunto) === clave(ctx.conjunto);
  if (!mismoConjunto) {
    avisos.push(`La plantilla es del conjunto «${plantilla.conjunto}» y la partida usa «${ctx.conjunto}»: solo se ha copiado lo que coincide por nombre.`);
  }
  // La ficha pasa a ser la de este archivo (así se reconoce si vuelve a importarse)
  if (plantilla.plantilla_id) e.plantilla_id = plantilla.plantilla_id;
  if (!f) return { entidad: e, avisos };

  if (f.nombre) e.nombre = f.nombre;
  e.tipo = f.tipo;
  e.cuerpo = f.cuerpo[0] === 1 && f.cuerpo[1] === 1 ? "Pequenio" : { Grande: [f.cuerpo[0], f.cuerpo[1]] };
  e.vision = f.vision;
  // Si es única lo decide el host (un jugador no lo cambia): una plantilla de antes no lo dice y se deja como está
  if (ctx.puedeUnico !== false && typeof f.unico === "boolean") e.es_unico = f.unico;
  const actuales: ValorActual[] = [];

  const sinPareja: string[] = [];
  for (const p of f.parametros) {
    const d = emparejar(ctx.definiciones, (x) => x.clave, p.id, p.clave, mismoConjunto);
    if (!d) {
      if (p.clave) sinPareja.push(p.clave);
      continue;
    }
    const actual = e.parametros.find((x) => x.id_definicion === d.id);
    if (!actual) continue;
    // «Hacer público»: solo en los que cada ficha decide (los de visibilidad fija no cambian)
    if (d.visibilidad === "Variable" && typeof p.publico === "boolean") actual.publico = p.publico;
    // El valor actual de una única (puede pasar del máximo); lo que no traiga lo rellena Rust con el de su instancia o el máximo
    if (typeof p.actual === "number" && d.tiene_maximo && "Numero" in d.valor_por_defecto) actuales.push({ id_definicion: d.id, valor: p.actual });
    if (d.calculo) continue; // lo calcula Rust con las variables
    const esNumero = "Numero" in d.valor_por_defecto;
    if (esNumero && typeof p.valor === "number") actual.valor = { Numero: p.valor };
    else if (!esNumero && typeof p.valor === "string") actual.valor = { Texto: p.valor };
    else sinPareja.push(d.clave);
  }

  // Ids de la plantilla → definiciones de ahora, por nombre (las opcionales de una derivada apuntan a otras variables o parámetros)
  const variableDeAhora = (idPlantilla: number) => {
    const nombre = plantilla.nombresVariables.get(idPlantilla) ?? "";
    return emparejar(ctx.variables, (x) => x.nombre, idPlantilla, nombre, mismoConjunto);
  };
  const parametroDeAhora = (idPlantilla: number) => {
    const nombre = plantilla.nombresParametros.get(idPlantilla) ?? "";
    return emparejar(ctx.definiciones, (x) => x.clave, idPlantilla, nombre, mismoConjunto);
  };

  for (const v of f.variables) {
    const d = emparejar(ctx.variables, (x) => x.nombre, v.id, v.nombre, mismoConjunto);
    if (!d) {
      if (v.nombre) sinPareja.push(v.nombre);
      continue;
    }
    const actual: ValorVariable | undefined = e.variables.find((x) => x.id_definicion === d.id);
    if (!actual) continue;
    if (!esDerivada(d)) {
      actual.valor = v.valor;
      continue;
    }
    const opciones = d.opciones ?? [];
    const porNombre = v.eleccion_nombre ? opciones.findIndex((o) => clave(o.nombre) === clave(v.eleccion_nombre)) : -1;
    actual.eleccion = porNombre >= 0 ? porNombre : v.eleccion < opciones.length ? v.eleccion : 0;
    actual.opcionales = [
      ...new Set(v.opcionales.map((o) => variableDeAhora(o)?.id).filter((id): id is number => id !== undefined && d.opcionales.includes(id)))
    ];
    actual.parametros_marcados = [
      ...new Set(
        v.parametros_marcados
          .map((p) => parametroDeAhora(p)?.id)
          .filter((id): id is number => id !== undefined && (d.parametros_opcionales ?? []).includes(id))
      )
    ];
  }
  e.actuales = e.es_unico ? [...(entidad.actuales ?? []).filter((a) => !actuales.some((n) => n.id_definicion === a.id_definicion)), ...actuales] : [];
  if (sinPareja.length > 0) avisos.push(`No se han encontrado en esta partida: ${[...new Set(sinPareja)].join(", ")}.`);
  return { entidad: e, avisos };
}
