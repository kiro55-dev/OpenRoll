// src/lib/editorFicha.svelte.ts
//
// Lógica de la ventana de edición de una ficha (VentanaFicha.svelte), separada de la vista para
// poder probarla sin abrir la app.
//
// Archivos "de la sesión": todo lo que se crea o importa mientras la ventana está abierta (la foto
// con la que nace un borrador, los .txt nuevos, los adjuntos arrastrados, las copias que deja un
// renombrado...). Al cancelar se descartan; al guardar se descarta lo que al final no se usó.
// Rust nunca borra lo que la partida guardada referencia, así que pasarle de más no es peligroso.
import { invoke } from "$lib/backend";
import { actualesTocados, fusionarFicha, iguales as fusionIgual } from "$lib/fusionFicha";
import { LIMITE_VALOR_ACTUAL, partida, type DefinicionParametro, type DefinicionVariable, type Entidad, type GrupoVariables, type Parametro } from "$lib/partida.svelte";
import { esJugador } from "$lib/modo";
import {
  LIMITE_VALOR_VARIABLE,
  calcularParametro,
  desgloseResultado,
  esDerivada,
  etiquetaVariable,
  modificadorVariable,
  normalizarVariables,
  resultadoVariable as resultadoDe,
  textoCalculo,
  textoModificador
} from "$lib/variables";
import { importarImagen, urlDeImagen } from "$lib/imagenes";
import { cargarImagen, encuadreCentrado, miniaturaWebp, type Encuadre } from "$lib/recorteFoto";
import { aplicarAFicha, MAX_ARCHIVOS_TEXTO, type ArchivoPlantilla, type PlantillaLeida } from "$lib/plantillaFicha";
import { conjuntoActual, importarFotoPlantilla } from "$lib/plantillaIo";

interface ArchivoAdjunto {
  ruta: string;
  entidad_archivo: Entidad;
}

const MAX_I16 = 32767;
const MAX_CUERPO = 255;
const MAX_VISION = 10000;

export const nombreArchivo = (ruta: string) => ruta.split(/[/\\]/).pop() ?? ruta;
export const esTexto = (ruta: string | null | undefined) => !!ruta && ruta.toLowerCase().endsWith(".txt");

/** Entero dentro de [min, max]; lo que no es un número cuenta como `min`. */
export function acotar(v: unknown, min: number, max: number): number {
  const n = Math.round(Number(v));
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : min;
}

/** Número entero dentro del rango de i16 que usa Rust, o un error con el motivo. */
function entero(v: unknown, nombre: string, min = 0): number {
  const n = typeof v === "number" ? v : Number(v);
  if (v === null || v === "" || !Number.isInteger(n)) throw new Error(`${nombre}: debe ser un número entero`);
  if (n < min) throw new Error(`${nombre}: no puede ser menor que ${min}`);
  if (n > MAX_I16) throw new Error(`${nombre}: no puede pasar de ${MAX_I16}`);
  return n;
}

const esNumerico = (d: DefinicionParametro) => "Numero" in d.valor_por_defecto;

/**
 * Igual que `normalizar_parametros` en Rust: un parámetro por definición y en su orden, con el
 * valor por defecto en los que falten, y `publico` según la visibilidad si esta es fija.
 */
export function normalizarParametros(defs: DefinicionParametro[], parametros: Parametro[]): Parametro[] {
  return defs.map((d) => {
    const p = parametros.find((x) => x.id_definicion === d.id) ?? {
      id_definicion: d.id,
      valor: structuredClone(d.valor_por_defecto),
      publico: false
    };
    if (d.visibilidad === "SiemprePublico") p.publico = true;
    else if (d.visibilidad === "SiemprePrivado") p.publico = false;
    return p;
  });
}

export class EditorFicha {
  /** La ficha que se edita (en la app, el mismo objeto reactivo que `partida.entidadEnEdicion`). */
  readonly entidad: Entidad;
  readonly esBorrador: boolean;
  readonly definiciones: DefinicionParametro[];
  /** Las variables de la partida (pestaña «Variables»), en su orden. */
  readonly variables: DefinicionVariable[];
  /** Sus grupos: cada uno es una sección de la pestaña «Variables». */
  readonly gruposVariables: GrupoVariables[];

  /** Subpestaña de la ventana: «Ficha» (todo, como la plantilla), «Parámetros visuales y archivos» o «Variables». */
  seccion = $state<"ficha" | "parametros" | "variables">("ficha");
  /** Qué grupo de variables se ve en la pestaña «Variables»: `null` = «Todos» (todas las secciones, una tras otra). */
  grupoVista = $state<number | null>(null);

  /** El grupo que se ve de verdad: el elegido si sigue existiendo y, si no, «Todos». */
  get grupoVistaEfectivo(): number | null {
    return this.gruposVariables.some((g) => g.id === this.grupoVista) ? this.grupoVista : null;
  }

  /** true mientras se guarda o se borra: los botones se desactivan y no se puede cancelar. */
  guardando = $state(false);
  #error = $state("");

  /** Índice de la pestaña de adjuntos abierta. */
  pestana = $state(0);
  /** Contenido de los .txt ya leídos, por ruta. Sin entrada = todavía no se ha leído. */
  textos = $state<Record<string, string>>({});
  #fallidos = $state<string[]>([]);
  /** Solo estos .txt se escriben al guardar. */
  #modificados = new Set<string>();
  #sesion: string[] = [];

  // El encuadre de la foto (`entidad.foto_encuadre`, lo mueve CajaFoto) se guarda con la ficha. Si se tocó
  // o cambió la foto, al guardar se recorta aquí y se manda a Rust la miniatura ya hecha.
  #encuadreTocado = false;
  #fotoCambiada = false;

  /** CajaFoto avisa cuando el usuario mueve o acerca la foto. */
  encuadreCambiado() {
    this.#encuadreTocado = true;
  }

  constructor(
    entidad: Entidad,
    esBorrador: boolean,
    definiciones: DefinicionParametro[] = partida.definiciones,
    variables: DefinicionVariable[] = partida.definicionesVariables ?? [],
    gruposVariables: GrupoVariables[] = partida.gruposVariables ?? []
  ) {
    this.entidad = entidad;
    this.esBorrador = esBorrador;
    this.definiciones = definiciones;
    this.variables = variables;
    this.gruposVariables = gruposVariables;
    entidad.foto_encuadre ??= encuadreCentrado(); // fichas de antes del encuadre
    // Sin definiciones (no se pudieron leer) no se toca nada, para no perder los valores de la ficha
    if (definiciones.length > 0) entidad.parametros = normalizarParametros(definiciones, entidad.parametros ?? []);
    // Las variables, igual (si no hay ninguna definida, las de la ficha se dejan como estén: Rust las ajusta)
    entidad.variables ??= [];
    if (variables.length > 0) entidad.variables = normalizarVariables(variables, entidad.variables);
    // La foto con la que nace un borrador (ficha creada arrastrando una imagen) es de esta sesión
    if (esBorrador && entidad.foto_url) this.#sesion.push(entidad.foto_url);
    this.#original = structuredClone($state.snapshot(entidad) as Entidad);
  }

  /**
   * Cómo estaba la ficha la última vez que la ventana la vio (al abrirla o al recibir un cambio de fuera). Lo que difiera de esto es lo
   * que ha tocado el usuario en la ventana; lo demás sigue a lo que haya fuera (ver `sincronizar`).
   */
  #original: Entidad;

  /**
   * Lleva a la ventana lo que ha cambiado FUERA de ella mientras está abierta (la vida con la rueda, el jugador dueño que edita su ficha...):
   * lo que el usuario no ha tocado aquí pasa a valer lo de fuera, y lo que sí ha tocado se queda. Así, al guardar, la ventana nunca pisa un
   * cambio ajeno con lo que vio al abrirla. Un borrador (ficha nueva) no tiene nada fuera.
   */
  sincronizar(externa: Entidad) {
    if (this.esBorrador || this.guardando || externa.id !== this.entidad.id) return;
    const fuera = structuredClone($state.snapshot(externa) as Entidad);
    fuera.foto_encuadre ??= encuadreCentrado();
    if (this.definiciones.length > 0) fuera.parametros = normalizarParametros(this.definiciones, fuera.parametros ?? []);
    fuera.variables ??= [];
    if (this.variables.length > 0) fuera.variables = normalizarVariables(this.variables, fuera.variables);
    const fusion = fusionarFicha(this.#original, $state.snapshot(this.entidad) as Entidad, fuera) as unknown as Record<string, unknown>;
    const destino = this.entidad as unknown as Record<string, unknown>;
    for (const k of Object.keys(fusion)) {
      if (!fusionIgual(destino[k], fusion[k])) destino[k] = fusion[k];
    }
    this.#original = fuera;
  }

  // ------------------------------------------------------------------
  // Parámetros (vida, armadura... los define la partida)
  // ------------------------------------------------------------------

  /** Cómo los pinta la ventana: los numéricos de dos en dos y los de texto de uno en uno. */
  get campos(): { numericos: DefinicionParametro[][]; textos: DefinicionParametro[] } {
    const nums = this.definiciones.filter(esNumerico);
    const numericos: DefinicionParametro[][] = [];
    for (let i = 0; i < nums.length; i += 2) numericos.push(nums.slice(i, i + 2));
    return { numericos, textos: this.definiciones.filter((d) => !esNumerico(d)) };
  }

  /** Los parámetros de la ficha tal como están escritos: lo que suma una derivada que usa parámetros. */
  get #fuentes() {
    return { definiciones: this.definiciones, valores: this.entidad.parametros };
  }

  #parametro(id: number): Parametro | undefined {
    return this.entidad.parametros.find((p) => p.id_definicion === id);
  }

  numero(id: number): number {
    // Un parámetro calculado no se escribe: sale de las variables de la ficha, y se ve cambiar al tocarlas
    const calculo = this.definiciones.find((d) => d.id === id)?.calculo;
    if (calculo) return calcularParametro(calculo, this.variables, this.entidad.variables, this.#fuentes);
    const v = this.#parametro(id)?.valor;
    return v && "Numero" in v ? v.Numero : 0;
  }
  /** ¿Se calcula con las variables (y por tanto no se puede escribir)? */
  esCalculado(id: number): boolean {
    return !!this.definiciones.find((d) => d.id === id)?.calculo;
  }
  /** Cómo se calcula, para el título del campo: «Se calcula con (con + tam) ÷10». `undefined` si se escribe a mano. */
  descripcionCalculo(id: number): string | undefined {
    const calculo = this.definiciones.find((d) => d.id === id)?.calculo;
    return calculo ? `Se calcula con las variables: ${textoCalculo(calculo, this.variables)}` : undefined;
  }
  /** Guarda lo que haya en el campo tal cual (aunque esté vacío): `guardar` lo valida. */
  ponerNumero(id: number, n: number) {
    const p = this.#parametro(id);
    if (p) p.valor = { Numero: n };
  }

  /**
   * ¿Se enseñan el valor actual Y el máximo? Solo en una ficha única y en los parámetros numéricos con máximo: una ficha que no es
   * única tiene varias instancias, cada una con su valor actual, y aquí solo se escribe el máximo.
   */
  muestraActual(d: DefinicionParametro): boolean {
    return !!this.entidad.es_unico && d.tiene_maximo && esNumerico(d);
  }
  /** El valor actual de una ficha única: el escrito y, si aún no hay, el de su instancia (si la tiene) o el máximo. */
  actual(id: number): number {
    const escrito = this.entidad.actuales?.find((a) => a.id_definicion === id);
    if (escrito) return escrito.valor;
    const deInstancia = partida.instancias.find((i) => i.base.id === this.entidad.id)?.valores_actuales.find((v) => v.id_definicion === id);
    return deInstancia ? deInstancia.valor : this.numero(id);
  }
  /** Guarda lo que haya en el campo tal cual (aunque esté vacío): `guardar` lo valida. */
  ponerActual(id: number, n: number) {
    this.entidad.actuales ??= [];
    const a = this.entidad.actuales.find((x) => x.id_definicion === id);
    if (a) a.valor = n;
    else this.entidad.actuales.push({ id_definicion: id, valor: n });
  }

  texto(id: number): string {
    const v = this.#parametro(id)?.valor;
    return v && "Texto" in v ? v.Texto : "";
  }
  ponerTexto(id: number, t: string) {
    const p = this.#parametro(id);
    if (p) p.valor = { Texto: t };
  }

  esPublico(id: number): boolean {
    return this.#parametro(id)?.publico ?? false;
  }
  ponerPublico(id: number, publico: boolean) {
    const p = this.#parametro(id);
    if (p) p.publico = publico;
  }

  // ------------------------------------------------------------------
  // Variables (Fuerza «f»...: las define el host; la ficha solo pone su valor)
  // ------------------------------------------------------------------

  valorVariable(id: number): number {
    const v = this.entidad.variables.find((x) => x.id_definicion === id);
    return v ? v.valor : 0; // tal cual (un campo vacío deja null): `guardar` lo valida
  }
  /** Guarda lo que haya en el campo tal cual (aunque esté vacío): `guardar` lo valida. */
  ponerValorVariable(id: number, n: number) {
    const v = this.entidad.variables.find((x) => x.id_definicion === id);
    if (v) v.valor = n;
  }
  /** El resultado que vale en el chat con lo escrito (`null` si el campo de una base no es un número). Una derivada se
   *  recalcula sola al cambiar sus bases o sus checks. */
  resultadoVariable(d: DefinicionVariable): number | null {
    if (esDerivada(d)) return resultadoDe(this.variables, this.entidad.variables, d, this.#fuentes);
    const valor: unknown = this.valorVariable(d.id); // un campo vacío deja null
    return typeof valor === "number" && Number.isFinite(valor) ? modificadorVariable(d, valor) : null;
  }

  /** De qué bases depende una derivada, como se enseña junto a su nombre: «des» o «des + fue» (la abreviatura de cada una o,
   *  sin ella, su nombre). Vacío en una base. */
  dependeDe(d: DefinicionVariable): string {
    const variables = d.bases.map((id) => this.variables.find((v) => v.id === id)).flatMap((b) => (b ? [etiquetaVariable(b)] : []));
    const parametros = (d.parametros ?? []).flatMap((id) => this.definiciones.find((p) => p.id === id)?.clave ?? []);
    return [...variables, ...parametros].join(" + ");
  }

  /** De dónde sale el resultado de una derivada, para el `title`: «des (+3) + com (+2)». */
  desgloseVariable(d: DefinicionVariable): string {
    return desgloseResultado(this.variables, this.entidad.variables, d, this.#fuentes);
  }

  /** El rango que esta ficha ha elegido en una derivada con rangos (su posición en `opciones`). */
  eleccion(idDerivada: number): number {
    return this.entidad.variables.find((x) => x.id_definicion === idDerivada)?.eleccion ?? 0;
  }
  elegirOpcion(idDerivada: number, indice: number) {
    const v = this.entidad.variables.find((x) => x.id_definicion === idDerivada);
    if (v) v.eleccion = indice;
  }

  /** Las variables de un grupo, en su orden. */
  variablesDe(grupo: GrupoVariables): DefinicionVariable[] {
    return this.variables.filter((d) => d.grupo === grupo.id);
  }

  /** ¿Tiene esta ficha marcada la opcional `idOpcional` en la derivada `idDerivada`? */
  opcionalMarcada(idDerivada: number, idOpcional: number): boolean {
    return this.entidad.variables.find((x) => x.id_definicion === idDerivada)?.opcionales?.includes(idOpcional) ?? false;
  }

  /** Marca o desmarca una opcional («+ com») de una derivada en esta ficha. */
  alternarOpcional(idDerivada: number, idOpcional: number) {
    const v = this.entidad.variables.find((x) => x.id_definicion === idDerivada);
    if (!v) return;
    const actuales = v.opcionales ?? [];
    v.opcionales = actuales.includes(idOpcional) ? actuales.filter((o) => o !== idOpcional) : [...actuales, idOpcional];
  }

  /** ¿Tiene esta ficha marcado el parámetro opcional `idParametro` en la derivada `idDerivada`? */
  parametroMarcado(idDerivada: number, idParametro: number): boolean {
    return this.entidad.variables.find((x) => x.id_definicion === idDerivada)?.parametros_marcados?.includes(idParametro) ?? false;
  }

  /** Marca o desmarca un parámetro opcional («+ armadura») de una derivada en esta ficha. */
  alternarParametro(idDerivada: number, idParametro: number) {
    const v = this.entidad.variables.find((x) => x.id_definicion === idDerivada);
    if (!v) return;
    const actuales = v.parametros_marcados ?? [];
    v.parametros_marcados = actuales.includes(idParametro) ? actuales.filter((p) => p !== idParametro) : [...actuales, idParametro];
  }

  /** Los parámetros opcionales de una derivada con su nombre, para pintar sus checks. */
  parametrosOpcionales(d: DefinicionVariable): DefinicionParametro[] {
    return (d.parametros_opcionales ?? []).flatMap((id) => this.definiciones.find((p) => p.id === id) ?? []);
  }

  /** Último error de la ventana (validación, archivos) o de Rust al guardar o borrar. */
  get error() {
    return this.#error || partida.errorEditor;
  }

  get adjuntos() {
    return this.entidad.archivos_adjuntos;
  }

  get rutaActiva(): string | null {
    return this.adjuntos[this.pestana] ?? null;
  }

  lecturaFallida(ruta: string) {
    return this.#fallidos.includes(ruta);
  }

  #plano(): Entidad {
    return $state.snapshot(this.entidad) as Entidad;
  }

  // ------------------------------------------------------------------
  // Cuerpo: 1×1 se guarda como "Pequenio", lo demás como { Grande: [x, y] }
  // ------------------------------------------------------------------

  get cuerpoX(): number {
    const c = this.entidad.cuerpo;
    return typeof c === "object" ? (c.Grande[0] ?? 1) : 1;
  }
  set cuerpoX(v: number) {
    this.#ponerCuerpo(v, this.cuerpoY);
  }

  get cuerpoY(): number {
    const c = this.entidad.cuerpo;
    return typeof c === "object" ? (c.Grande[1] ?? 1) : 1;
  }
  set cuerpoY(v: number) {
    this.#ponerCuerpo(this.cuerpoX, v);
  }

  #ponerCuerpo(x: unknown, y: unknown) {
    const ax = acotar(x, 1, MAX_CUERPO);
    const ay = acotar(y, 1, MAX_CUERPO);
    this.entidad.cuerpo = ax === 1 && ay === 1 ? "Pequenio" : { Grande: [ax, ay] };
  }

  // ------------------------------------------------------------------
  // Adjuntos
  // ------------------------------------------------------------------

  /** Abre una pestaña y, si es un .txt que aún no se ha leído, lo lee. */
  async abrirPestana(i: number) {
    if (this.adjuntos.length === 0) return;
    this.pestana = i;
    const ruta = this.adjuntos[i];
    if (!ruta || !esTexto(ruta) || ruta in this.textos) return;

    this.#fallidos = this.#fallidos.filter((r) => r !== ruta);
    try {
      const contenido = await invoke<string>("leer_archivo_texto", { rutaRelativa: ruta });
      // Si entretanto se quitó o se renombró, no se deja una entrada huérfana
      if (this.adjuntos.includes(ruta)) this.textos[ruta] ??= contenido;
    } catch (err) {
      this.#fallidos.push(ruta);
      this.#error = `No se pudo leer «${nombreArchivo(ruta)}»: ${err}`;
    }
  }

  /** Texto escrito en el .txt abierto. Se ignora si aún no se ha leído (no se pisa el archivo). */
  escribir(texto: string) {
    const ruta = this.rutaActiva;
    if (!ruta || !(ruta in this.textos)) return;
    this.textos[ruta] = texto;
    this.#modificados.add(ruta);
  }

  #anadir(ruta: string) {
    this.adjuntos.push(ruta);
    this.#sesion.push(ruta);
  }

  async crearTexto() {
    if (this.#rutasTexto().length >= MAX_ARCHIVOS_TEXTO) {
      this.#error = `Una ficha no puede tener más de ${MAX_ARCHIVOS_TEXTO} archivos de texto`;
      return;
    }
    try {
      const { ruta } = await invoke<ArchivoAdjunto>("crear_documento_texto", { entidad: this.#plano() });
      this.#anadir(ruta);
      this.textos[ruta] = ""; // ya existe vacío en disco: no hace falta leerlo
      this.pestana = this.adjuntos.length - 1;
    } catch (err) {
      this.#error = `No se pudo crear el documento: ${err}`;
    }
  }

  /** Copia archivos del sistema a la carpeta de la ficha y abre el último. */
  async importarArchivos(origenes: string[]) {
    const antes = this.adjuntos.length;
    for (const origen of origenes) {
      try {
        const { ruta } = await invoke<ArchivoAdjunto>("importar_archivo", {
          adjunto: { ruta: "", entidad_archivo: this.#plano() },
          origen
        });
        this.#anadir(ruta);
      } catch (err) {
        this.#error = `No se pudo importar «${origen}»: ${err}`;
      }
    }
    if (this.adjuntos.length > antes) await this.abrirPestana(this.adjuntos.length - 1);
  }

  async renombrar(i: number, nombre: string) {
    const vieja = this.adjuntos[i];
    nombre = nombre.trim();
    if (!vieja || !nombre || nombre === nombreArchivo(vieja)) return;

    try {
      const { ruta } = await invoke<ArchivoAdjunto>("renombrar_archivo_adjunto", {
        adjunto: { ruta: vieja, entidad_archivo: this.#plano() },
        nuevoNombre: nombre
      });

      const k = this.adjuntos.indexOf(vieja);
      if (k >= 0) this.adjuntos[k] = ruta;

      // Si el original es de la ficha ya guardada, Rust lo COPIÓ (no se pierde si se cancela):
      // la copia es un archivo nuevo de la sesión. Si era de la sesión, se movió: se sustituye.
      const s = this.#sesion.indexOf(vieja);
      if (s >= 0) this.#sesion[s] = ruta;
      else this.#sesion.push(ruta);

      if (vieja in this.textos) {
        this.textos[ruta] = this.textos[vieja];
        delete this.textos[vieja];
      }
      if (this.#modificados.delete(vieja)) this.#modificados.add(ruta);
    } catch (err) {
      this.#error = `No se pudo renombrar: ${err}`;
    }
  }

  /** Quita un adjunto de la ficha (el archivo se libera al guardar, o nunca si se cancela). */
  quitar(i: number) {
    const [ruta] = this.adjuntos.splice(i, 1);
    if (ruta) {
      delete this.textos[ruta];
      this.#modificados.delete(ruta);
    }
    // Si se quita una pestaña anterior a la activa, la activa sigue siendo el mismo archivo
    if (i < this.pestana) this.pestana -= 1;
    this.pestana = Math.max(0, Math.min(this.pestana, this.adjuntos.length - 1));
    void this.abrirPestana(this.pestana);
  }

  async abrirEnSistema(ruta: string) {
    try {
      await invoke("abrir_archivo_en_sistema", { rutaRelativa: ruta });
    } catch (err) {
      this.#error = String(err);
    }
  }

  // ------------------------------------------------------------------
  // Plantilla de ficha (ver `plantillaFicha.ts`): rellena la ventana; no se guarda hasta pulsar «Guardar»
  // ------------------------------------------------------------------

  /** Lo que no se pudo pasar de la última plantilla aplicada (se ve en la ventana). */
  avisosPlantilla = $state<string[]>([]);
  /** Se ve unos segundos tras importar una plantilla («Datos importados»). */
  datosImportados = $state(false);
  #relojImportado: ReturnType<typeof setTimeout> | null = null;

  async aplicarPlantilla(plantilla: PlantillaLeida) {
    const { entidad, avisos } = aplicarAFicha($state.snapshot(this.entidad) as Entidad, plantilla, {
      definiciones: this.definiciones,
      variables: this.variables,
      conjunto: conjuntoActual(),
      puedeUnico: !esJugador // si una ficha es única lo decide el host: un jugador no lo cambia con una plantilla
    });
    const e = this.entidad;
    e.nombre = entidad.nombre;
    e.tipo = entidad.tipo;
    e.cuerpo = entidad.cuerpo;
    e.vision = entidad.vision;
    e.es_unico = entidad.es_unico;
    e.actuales = entidad.actuales ?? [];
    e.plantilla_id = entidad.plantilla_id ?? null;
    e.parametros = entidad.parametros;
    e.variables = entidad.variables;
    // La foto (entera) con su encuadre; sin foto en la plantilla se queda la que hubiera
    if (plantilla.foto) {
      try {
        const ruta = await importarFotoPlantilla(plantilla.foto);
        if (ruta !== e.foto_url) this.#fotoCambiada = true;
        e.foto_url = ruta;
        e.foto_encuadre = { ...plantilla.foto.encuadre };
        this.#encuadreTocado = true; // la miniatura se rehace al guardar
        if (!this.#sesion.includes(ruta)) this.#sesion.push(ruta);
      } catch (err) {
        avisos.push(`La foto no se ha podido usar: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
    // Los archivos de texto: la plantilla manda (se renombran, se editan, se crean y los que ya no están dejan de existir)
    if (plantilla.ficha && plantilla.archivos) await this.#aplicarArchivos(plantilla.archivos, avisos);
    this.avisosPlantilla = avisos;
    this.datosImportados = true;
    if (this.#relojImportado) clearTimeout(this.#relojImportado);
    this.#relojImportado = setTimeout(() => (this.datosImportados = false), 4000);
  }

  /** La ficha tal como la ve la ventana, para exportarla: una única lleva su valor actual (el escrito o el de su instancia). */
  entidadParaPlantilla(): Entidad {
    const e = $state.snapshot(this.entidad) as Entidad;
    e.actuales = e.es_unico
      ? this.definiciones.filter((d) => this.muestraActual(d)).map((d) => ({ id_definicion: d.id, valor: this.actual(d.id) }))
      : [];
    return e;
  }

  /** Las rutas de los .txt adjuntos (los demás tipos de archivo no cuentan). */
  #rutasTexto(): string[] {
    return this.adjuntos.filter((r): r is string => !!r && esTexto(r));
  }

  /** Los .txt de la ficha tal como están ahora (lo escrito sin guardar incluido), para exportarlos en la plantilla. */
  async archivosParaPlantilla(): Promise<ArchivoPlantilla[]> {
    const lista: ArchivoPlantilla[] = [];
    for (const ruta of this.#rutasTexto()) {
      let contenido: string | undefined = this.textos[ruta];
      if (contenido === undefined) {
        try {
          contenido = await invoke<string>("leer_archivo_texto", { rutaRelativa: ruta });
        } catch {
          contenido = ""; // no se pudo leer: sale vacío (al importar solo se cambiaría si el usuario lo edita)
        }
      }
      lista.push({ original: nombreArchivo(ruta), nombre: nombreArchivo(ruta), contenido });
    }
    return lista;
  }

  /**
   * Pasa los archivos de texto de una plantilla a la ficha (sin guardar: se escriben con «Guardar»). Cada uno se une, por el nombre
   * que tenía al exportar (`original`), con el .txt de la ficha que se llama así: si el nombre cambió, ese archivo se RENOMBRA; si el
   * contenido cambió, se reescribe; los que no existían se crean; y los .txt de la ficha que ya no están en la plantilla dejan de
   * existir (es el cambio que el usuario espera). Los archivos que no son .txt no se tocan.
   */
  async #aplicarArchivos(archivos: ArchivoPlantilla[], avisos: string[]) {
    const clave = (n: string) => n.toLowerCase();
    const actuales = this.#rutasTexto();
    const porNombre = new Map(actuales.map((r) => [clave(nombreArchivo(r)), r]));
    const reclamados = new Set<string>();
    // Primero se une cada archivo por el nombre que tenía al exportar; los que no se encuentran así (la ficha ya los tiene con su
    // nombre nuevo, por una importación anterior de este mismo archivo) se buscan por el nombre de ahora
    const rutas = new Map<ArchivoPlantilla, string>();
    for (const archivo of archivos) {
      const ruta = archivo.original ? porNombre.get(clave(archivo.original)) : undefined;
      if (ruta && !reclamados.has(ruta)) {
        reclamados.add(ruta);
        rutas.set(archivo, ruta);
      }
    }
    for (const archivo of archivos) {
      if (rutas.has(archivo)) continue;
      const ruta = porNombre.get(clave(archivo.nombre));
      if (ruta && !reclamados.has(ruta)) {
        reclamados.add(ruta);
        rutas.set(archivo, ruta);
      }
    }
    // Los cambios de nombre de los que ya existen van ANTES de crear los nuevos (así un nombre que quedó libre se puede reutilizar)
    const trabajos: { ruta: string | null; archivo: ArchivoPlantilla }[] = [
      ...archivos.filter((a) => rutas.has(a)).map((archivo) => ({ ruta: rutas.get(archivo) as string, archivo })),
      ...archivos.filter((a) => !rutas.has(a)).map((archivo) => ({ ruta: null, archivo }))
    ];

    // Los que ya no están: dejan de ser de la ficha (el archivo se libera al guardar)
    for (const ruta of actuales) {
      if (reclamados.has(ruta)) continue;
      const i = this.adjuntos.indexOf(ruta);
      if (i >= 0) this.quitar(i);
    }

    for (const { ruta: existente, archivo } of trabajos) {
      let ruta: string;
      if (existente !== null) {
        ruta = existente;
      } else {
        // Uno nuevo: se crea (Documento_N.txt) y se le pone su nombre
        const antes = this.adjuntos.length;
        this.#error = "";
        await this.crearTexto();
        if (this.adjuntos.length === antes) {
          avisos.push(`No se pudo crear «${archivo.nombre}»${this.#error ? `: ${this.#error}` : ""}.`);
          this.#error = "";
          continue;
        }
        ruta = this.adjuntos[this.adjuntos.length - 1] as string;
      }
      // Su nombre: si cambió, se renombra ESE archivo (si ya hay otro con el nombre, se le añade un número)
      if (nombreArchivo(ruta) !== archivo.nombre) {
        const stem = archivo.nombre.replace(/\.txt$/i, "");
        let hecho = false;
        for (let n = 1; n <= 9 && !hecho; n++) {
          const nombre = n === 1 ? archivo.nombre : `${stem} (${n}).txt`;
          const i = this.adjuntos.indexOf(ruta);
          this.#error = "";
          await this.renombrar(i, nombre);
          const ahora = this.adjuntos[i];
          hecho = !!ahora && nombreArchivo(ahora) === nombre;
          if (hecho && ahora) ruta = ahora;
        }
        this.#error = "";
        if (!hecho) avisos.push(`No se pudo renombrar «${nombreArchivo(ruta)}» a «${archivo.nombre}».`);
      }
      // Su contenido: solo se reescribe si cambió
      let actual: string | undefined = this.textos[ruta];
      if (actual === undefined) {
        try {
          actual = await invoke<string>("leer_archivo_texto", { rutaRelativa: ruta });
          this.textos[ruta] = actual;
        } catch {
          actual = undefined;
        }
      }
      if (actual !== archivo.contenido) {
        this.textos[ruta] = archivo.contenido;
        this.#modificados.add(ruta);
      }
    }
    this.pestana = Math.max(0, Math.min(this.pestana, this.adjuntos.length - 1));
    if (this.adjuntos.length > 0) await this.abrirPestana(this.pestana);
  }

  /** `origen`: una ruta del disco (host) o el archivo que eligió el jugador en su navegador (se sube al host). */
  async ponerFoto(origen: string | File) {
    try {
      const nueva = await importarImagen(origen, "ficha");
      if (nueva !== this.entidad.foto_url) {
        // Otra foto: se empieza centrada y hay que rehacer la miniatura (la propia era de la foto anterior)
        this.entidad.foto_encuadre = encuadreCentrado();
        this.#fotoCambiada = true;
      }
      this.entidad.foto_url = nueva;
      if (!this.#sesion.includes(nueva)) this.#sesion.push(nueva);
    } catch (err) {
      this.#error = `No se pudo importar la imagen: ${err}`;
    }
  }

  // ------------------------------------------------------------------
  // Guardar / cancelar / borrar
  // ------------------------------------------------------------------

  #validar() {
    const e = this.entidad;
    if (!e.nombre?.trim()) throw new Error("La ficha necesita un nombre");
    for (const d of this.definiciones.filter(esNumerico)) {
      this.ponerNumero(d.id, entero(this.numero(d.id), d.clave));
    }
    // Las derivadas no tienen valor propio: se calculan
    for (const d of this.variables.filter((v) => !esDerivada(v))) {
      const n = entero(this.valorVariable(d.id), d.nombre, -LIMITE_VALOR_VARIABLE);
      if (n > LIMITE_VALOR_VARIABLE) throw new Error(`${d.nombre}: no puede pasar de ${LIMITE_VALOR_VARIABLE}`);
      this.ponerValorVariable(d.id, n);
    }
    this.#ponerCuerpo(this.cuerpoX, this.cuerpoY);
    this.entidad.vision = acotar(this.entidad.vision ?? 0, 0, MAX_VISION);
    // Los valores actuales solo los tiene una ficha única, y de los parámetros con máximo; uno vacío o roto se quita (Rust lo rellena)
    this.entidad.actuales = !this.entidad.es_unico
      ? []
      : (this.entidad.actuales ?? [])
          .filter((a) => this.definiciones.some((d) => d.id === a.id_definicion && this.muestraActual(d)) && Number.isFinite(a.valor))
          .map((a) => ({ id_definicion: a.id_definicion, valor: acotar(a.valor, -LIMITE_VALOR_ACTUAL, LIMITE_VALOR_ACTUAL) }));
  }

  async #descartarSesion() {
    if (this.#sesion.length === 0) return;
    try {
      await invoke("descartar_archivos_borrador", { rutas: [...this.#sesion] });
    } catch (err) {
      console.error("Error al descartar archivos de la sesión:", err);
    }
  }

  /** Recorta la foto con el encuadre elegido y manda la miniatura a Rust. Un fallo aquí no impide guardar la ficha. */
  async #guardarMiniatura() {
    const foto = this.entidad.foto_url;
    if (!foto || !(this.#encuadreTocado || this.#fotoCambiada)) return;
    try {
      const datos = await miniaturaWebp(await cargarImagen(urlDeImagen(foto)), $state.snapshot(this.entidad.foto_encuadre) as Encuadre);
      await invoke("guardar_miniatura_ficha", { id: this.entidad.id, datos });
      await partida.recargarEntidades(); // la ficha ya tiene la ruta de la miniatura nueva
      this.#encuadreTocado = this.#fotoCambiada = false;
    } catch (err) {
      console.warn("No se pudo guardar la miniatura de la ficha:", err);
      partida.avisar(`La ficha se guardó, pero no se pudo actualizar su miniatura: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /**
   * Valida, escribe los .txt modificados y guarda la ficha. Si algo falla la ventana sigue
   * abierta con el motivo a la vista y no se descarta nada. Devuelve true si se guardó.
   */
  async guardar(): Promise<boolean> {
    if (this.guardando) return false;
    this.guardando = true;
    this.#error = "";
    try {
      // Antes de nada, lo último de fuera: los pasos de rueda pendientes se mandan y la ventana recoge lo que haya cambiado
      if (!this.esBorrador) {
        await partida.guardarValoresPendientes();
        const fuera = partida.fichaActual(this.entidad.id);
        this.guardando = false;
        if (fuera) this.sincronizar(fuera);
        this.guardando = true;
      }
      this.#validar();

      for (const ruta of this.#modificados) {
        await invoke("guardar_archivo_texto", {
          adjunto: { ruta, entidad_archivo: this.#plano() },
          contenido: this.textos[ruta]
        });
        this.#modificados.delete(ruta);
      }

      // De una ficha única solo se mandan los valores actuales que se han cambiado AQUÍ: los demás los pone Rust con los de su
      // instancia (los últimos, aunque alguien los haya cambiado hace un instante), así guardar nunca devuelve la vida a un valor viejo
      const enviar = $state.snapshot(this.entidad) as Entidad;
      if (!this.esBorrador) enviar.actuales = actualesTocados(this.#original, enviar);
      if (!(await partida.guardarEntidad(enviar))) return false;
      await this.#guardarMiniatura();
      // Fotos/archivos de la sesión que al final no se usaron (cambió de foto, quitó un adjunto...)
      await this.#descartarSesion();
      return true;
    } catch (err) {
      this.#error = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      this.guardando = false;
    }
  }

  /** Cierra sin guardar. Rust descarta lo creado en la sesión (y, si era un borrador, su carpeta y su id). */
  async cancelar() {
    if (this.guardando) return;
    await partida.cancelarEditorEntidad([...this.#sesion]);
  }

  async borrar() {
    if (this.guardando) return;
    this.guardando = true;
    try {
      await partida.borrarEntidad(this.entidad);
      // Solo si se borró de verdad (si falla, la ventana sigue abierta con el error)
      if (!partida.entidadEnEdicion) await this.#descartarSesion();
    } finally {
      this.guardando = false;
    }
  }
}
