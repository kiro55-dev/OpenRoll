// src/lib/partida.svelte.ts
//
// Estado compartido de la partida en el front: zonas, mapas, mapa abierto, rejilla, banco de
// entidades e instancias. Los componentes lo importan y lo leen directamente
// (`partida.zonas`, `partida.mapaActual`...) y llaman a sus métodos para cambiarlo; ya no hay
// props ni callbacks (`onRecargarZonas`...) que atravesar de padre a hijo.
//
// Rust sigue siendo la fuente de verdad: cada método escribe en Rust y vuelve a leer.
import type { Encuadre } from "$lib/recorteFoto";
import { EVENTO_PARTIDA_CAMBIADA, invoke, listen } from "$lib/backend";
import type { PartesCambiadas } from "$lib/red/replica";
import { iniciarImagenes, importarImagen } from "$lib/imagenes";
import { miniaturas } from "$lib/miniaturas.svelte";
import { enviarAviso, avisoFoco } from "$lib/avisosTablero";
import { desplazarGrupo, encajar, ladosInstancia, rejillaDe } from "$lib/tablero";
import { ajustesVista } from "$lib/ajustesVista.svelte";
import { esJugador } from "$lib/modo";
import { tiradas } from "$lib/tiradas.svelte";
import { grupoParaEnter, ordenarPorGrupos, tieneElTurno } from "$lib/batalla";
import type { PlantillaLeida } from "$lib/plantillaFicha";

export interface Zona {
  id: number;
  nombre: string;
  orden: number;
}

export interface Mapa {
  id: number;
  id_zona: number;
  nombre: string;
  url_imagen_fondo: string | null; // ruta relativa devuelta por importar_imagen
  ancho: number;
  alto: number;
  rejilla: number;
  color_opacidad: number[]; // [r, g, b, a] con a en 0..255
  grosor: number;
  orden: number;
  /** Lo ven los jugadores de la sala (la casilla del mapa). Lo cambia solo `compartirMapa`. */
  compartido: boolean;
  /** La batalla en curso en este mapa (el parámetro «en batalla»), o nada. La cambian solo `Batalla`, `Terminar` y pasar el turno. */
  batalla?: Batalla | null;
}

/** Una batalla en un mapa: uno o varios grupos, cada uno con su turno y su contador de vueltas (ver `$lib/batalla`). */
export interface Batalla {
  grupos: GrupoBatalla[];
  siguiente_id: number;
}

export interface GrupoBatalla {
  id: number;
  nombre: string;
  /** Ids de las instancias que participan. */
  miembros: number[];
  /** La instancia que tiene el turno (`null` si el grupo no tiene a nadie). */
  turno: number | null;
  /** La vuelta en la que va (empieza en 1). */
  ronda: number;
}

export type TipoEntidad = "Enemigo" | "NPC" | "Jugador" | "Objeto";
export type Cuerpo = "Pequenio" | { Grande: number[] };

// ---- Parámetros (espejo de modelos.rs) ----
// Partida: DefinicionParametro (comunes a todas las fichas) · Ficha: Parametro (su valor)
// · Instancia: ValorActual (solo los que tienen máximo, p. ej. la vida)
export type ValorParametro = { Numero: number } | { Texto: string };
export type Visibilidad = "SiemprePublico" | "SiemprePrivado" | "Variable";
/** El array es RGBA */
export type Forma =
  | { Rombo: number[] }
  | { Circulo: number[] }
  | { Cuadrado: number[] }
  | { BarraVida: number[] }
  /** Solo con máximo: «actual/máx» en «Fichas activas», en ese color */
  | { Resumen: number[] };

export interface DefinicionParametro {
  id: number;
  clave: string;
  valor_por_defecto: ValorParametro;
  tiene_maximo: boolean;
  representar_mapa: Forma | null;
  visibilidad: Visibilidad;
  unidad: string | null;
  /** Solo con máximo: si el valor actual llega a 0 o menos, la ficha se ve atenuada (como la vida) */
  atenuar_si_negativo: boolean;
  /** Si lo hay, el valor NO se escribe: lo calcula Rust con las variables de la ficha (`CalculoParametro`). */
  calculo?: CalculoParametro | null;
}

/** `floor(suma · multiplicador / divisor) + constante`, con `suma` = el valor de cada variable (en una base, el valor escrito;
 *  en una derivada, su resultado). */
export interface CalculoParametro {
  variables: number[];
  multiplicador: number;
  divisor: number;
  constante: number;
}

/** Un rango de una derivada (Entrenado +2...): cada ficha elige uno y su bono se suma. */
export interface OpcionVariable {
  nombre: string;
  bonus: number;
}

export interface Parametro {
  id_definicion: number;
  valor: ValorParametro;
  publico: boolean;
}

export interface ValorActual {
  id_definicion: number;
  valor: number;
}

/** Variable de la partida (`DefinicionVariable` en Rust): Fuerza «f», neutro 10, escala 2. Ver $lib/variables. */
export interface DefinicionVariable {
  id: number;
  nombre: string;
  abreviatura: string;
  neutro: number;
  escala: number;
  /** Su grupo (`GrupoVariables.id`). */
  grupo: number;
  /** Solo derivadas: las variables base cuyos resultados se suman (al menos una). */
  bases: number[];
  /** Solo derivadas: variables base que cada ficha puede sumar o no (un check «+ com»). */
  opcionales: number[];
  /** Solo derivadas: `floor(suma · multiplicador / divisor) + constante (+ el bono del rango elegido)`. Sin tocar: 1, 1 y 0. */
  multiplicador?: number;
  divisor?: number;
  constante?: number;
  /** Solo derivadas: la lista de rangos (`ListaRangos.id`) de la que cada ficha elige uno (`ValorVariable.eleccion`) y cuyo bono se suma. */
  rangos?: number | null;
  /** Solo derivadas: fórmula individual (× ÷ +) de las entradas que quieran (base, opcional o parámetro); se aplica a lo que aporta cada una ANTES de sumarlas. */
  formulas_entradas?: FormulaEntrada[];
  /** Una copia de las opciones de su lista (la deja Rust al día: no se escribe a mano). */
  opciones?: OpcionVariable[];
  /** Solo derivadas: parámetros visuales (numéricos, escritos a mano) cuyo valor se suma tal cual. */
  parametros?: number[];
  /** Solo derivadas: parámetros que cada ficha puede sumar o no (un check «+ armadura»). */
  parametros_opcionales?: number[];
}

/** De dónde viene una entrada de una derivada: una variable (base u opcional) o un parámetro visual (sus ids comparten números). */
export type OrigenEntrada = { Variable: number } | { Parametro: number };

/** La fórmula de UNA entrada de una derivada: `floor(valor · multiplicador / divisor) + constante`, antes de sumarlas todas. */
export interface FormulaEntrada {
  origen: OrigenEntrada;
  multiplicador: number;
  divisor: number;
  constante: number;
}

/** Una lista de rangos con nombre («Competencia»: Sin entrenar +0, Entrenado +2...) que comparten las variables derivadas. */
export interface ListaRangos {
  id: number;
  nombre: string;
  opciones: OpcionVariable[];
}

/** Los dos tipos de grupo de variables (`TipoGrupoVariables` en Rust). */
export type TipoGrupoVariables = "Base" | "Derivadas";

/** Un grupo de variables («Características», «Habilidades»...). */
export interface GrupoVariables {
  id: number;
  nombre: string;
  tipo: TipoGrupoVariables;
}

/** Un conjunto de parámetros guardado en la partida, en resumen (`ResumenConjunto` en Rust). */
export interface ResumenConjunto {
  id: number;
  nombre: string;
  descripcion: string;
  /** La clave del conjunto de serie del que viene; `null` si es propio o importado. */
  origen: string | null;
  parametros: number;
  variables: number;
  grupos: number;
}

/** Un conjunto de la biblioteca del host (`ResumenBiblioteca` en Rust). `archivo` lo identifica. */
export interface ResumenBiblioteca {
  archivo: string;
  nombre: string;
  descripcion: string;
  parametros: number;
  variables: number;
}

/** Un conjunto que trae el programa (definido en código). `ya_anadido`: la partida ya tiene una copia. */
export interface ResumenDeSerie {
  clave: string;
  nombre: string;
  descripcion: string;
  ya_anadido: boolean;
}

/** Cómo se nombra el resultado de una comparación del chat (`EscalaResultado` en Rust; del conjunto activo). */
export interface EscalaResultado {
  nombre_exito: string;
  nombre_fallo: string;
  /** Solo con `<=`: niveles de acierto (`total <= objetivo · numerador / denominador`), del más estricto al más laxo. */
  tramos: TramoEscala[];
  /** Críticos y pifias: reglas sobre la cara de un dado suelto. La primera que casa gana. */
  naturales: ReglaNatural[];
}

export interface TramoEscala {
  nombre: string;
  numerador: number;
  denominador: number;
}

export type EfectoNatural = "SoloSiAcierta" | "SoloSiFalla" | "ForzarAcierto" | "ForzarFallo";

export interface ReglaNatural {
  nombre: string;
  /** Solo en un dado de estas caras; `null` = en cualquiera. */
  dado: number | null;
  desde: number;
  hasta: number;
  efecto: EfectoNatural;
  /** Solo si el objetivo es menor que esto; `null` = siempre. */
  umbral_menor_que: number | null;
}

/** Una escala de ejemplo (Cthulhu, D&D...) con la que rellenar el editor: se copia, la partida no depende de ella. */
export interface EjemploEscala {
  clave: string;
  nombre: string;
  descripcion: string;
  escala: EscalaResultado;
}

/** Un .txt con el que nace toda ficha nueva del conjunto activo (`ArchivoTextoDefecto` en Rust). `nombre` va sin «.txt». */
export interface ArchivoTextoDefecto {
  id: number;
  nombre: string;
  contenido: string;
}

export interface ListaConjuntos {
  activo: number;
  guardados: ResumenConjunto[];
  de_serie: ResumenDeSerie[];
}

/** Paso de − / + de un parámetro en una ficha (`PasoValor` en Rust). */
export interface PasoValor {
  id_definicion: number;
  paso: number;
}

/** Tope del paso (`PASO_VALOR_MAX` en Rust). */
export const PASO_VALOR_MAX = 9999;

/** Valor de una variable en una ficha. */
export interface ValorVariable {
  id_definicion: number;
  /** El valor escrito (solo en las bases: una derivada se calcula). */
  valor: number;
  /** Solo en una derivada: las opcionales que esta ficha tiene marcadas. */
  opcionales?: number[];
  /** Solo en una derivada con rangos: el que ha elegido esta ficha (su posición en `DefinicionVariable.opciones`). */
  eleccion?: number;
  /** Solo en una derivada: los parámetros opcionales que esta ficha tiene marcados. */
  parametros_marcados?: number[];
}

/** Ids de los parámetros de serie (fijos en Rust: `ID_VIDA`...). */
export const ID_PARAMETRO = { vida: 1, armadura: 2, velocidad: 3, rangoArma: 4 } as const;

/** Valor numérico de un parámetro de una ficha (0 si no lo tiene o es de texto). */
export function numeroDe(entidad: Entidad, idDefinicion: number): number {
  const v = entidad.parametros.find((p) => p.id_definicion === idDefinicion)?.valor;
  return v && "Numero" in v ? v.Numero : 0;
}

/**
 * ¿La instancia trae ese parámetro? Rust no manda a la pantalla los privados (ver `visibilidad.rs`):
 * un parámetro que falta no se enseña, ni su valor, ni su resumen, ni lo que se dibuja con él.
 */
export function veParametro(instancia: Instancia, idDefinicion: number): boolean {
  return instancia.base.parametros.some((p) => p.id_definicion === idDefinicion);
}

/** Valor actual de un parámetro con máximo en una instancia (si no lo lleva, el de la ficha). */
export function valorActual(instancia: Instancia, idDefinicion: number): number {
  return (
    instancia.valores_actuales.find((v) => v.id_definicion === idDefinicion)?.valor ??
    numeroDe(instancia.base, idDefinicion)
  );
}

export interface Entidad {
  id: number;
  nombre: string;
  tipo: TipoEntidad;
  /** Uno por definición, en el orden de las definiciones */
  parametros: Parametro[];
  /** Una por variable de la partida, en su orden. Solo llegan al host y a los propietarios (a los demás, vacía). */
  variables: ValorVariable[];
  /** Lo que suman o restan los botones − / + de cada valor (vida...) de esta ficha; solo los que no son 1. Lo guarda Rust. */
  pasos_valor?: PasoValor[];
  cuerpo: Cuerpo;
  /** Visión nocturna: cuánto ve la ficha en la oscuridad, en metros (dato de serie de la ficha, como el cuerpo; 0 = sin definir). Por ahora no hace nada. */
  vision: number;
  zona_id: number;
  foto_url: string | null;
  /** Miniatura propia de la ficha (la que encuadró el usuario); la pone Rust, nunca el front. Sin ella se usa la automática de la foto. */
  miniatura_url: string | null;
  /** Encuadre de la foto en la miniatura (zoom 100-400 %, x e y 0-100): se guarda para poder seguir moviéndola al reabrir la ficha. */
  foto_encuadre: Encuadre;
  archivos_adjuntos: (string | null)[];
  es_unico: boolean;
  /** Solo en una ficha ÚNICA: el valor actual de cada parámetro con máximo (la vida actual...), que se escribe junto al máximo. La
   *  mantiene Rust al día con el de su instancia; en las demás fichas está vacía. */
  actuales?: ValorActual[];
  /** La identidad del archivo de plantilla con el que se rellenó la ficha (un UUID); sirve para reconocerla si se vuelve a importar. */
  plantilla_id?: string | null;
  /** Ids de los jugadores que controlan la ficha (`IdJugador` en Rust), sin el host, que lo puede todo. Vacío = solo el host. */
  propietarios: number[];
}

// Casilla del tablero: (0,0) arriba a la izquierda. La lógica está en $lib/tablero.ts (y en tablero.rs).
export interface Casilla {
  x: number;
  y: number;
}

// En una ficha de varias casillas es la casilla CENTRAL
export type Coordenada = "NoPuesto" | { Casilla: Casilla };

/** Dónde poner una instancia: una casilla de un mapa (null = activa, fuera del tablero). */
export interface Destino {
  id_mapa: number;
  casilla: Casilla;
}

export interface Instancia {
  id_instancia: number;
  id_mapa: number | null;
  base: Entidad;
  valores_actuales: ValorActual[];
  coordenadas: Coordenada;
  activo?: boolean;
  /** Cuartos de vuelta en el sentido del reloj (0..3); con uno impar, ancho y alto se intercambian */
  rotacion: number;
  /** La imagen se dibuja en espejo (izquierda-derecha, en el eje de la propia ficha, antes de girar). Solo aspecto. */
  espejo?: boolean;
  /** Oculta: solo la ve el host (al 50 % en el tablero, ojo cerrado en «Fichas activas»); a los jugadores no les llega. */
  oculto?: boolean;
  /** Posición en «Fichas activas» */
  orden: number;
  /** Altura en el tablero (la mayor, encima). La pone Rust al poner o mover la ficha. */
  capa?: number;
  /** Rust la marca cuando está gris pero su parámetro no es público (no llega su valor, solo esta señal) */
  atenuada?: boolean;
}

export interface Rejilla {
  visible: boolean;
  tamano: number;
  color: string; // "#rrggbb"
  grosor: number;
  opacidad: number; // 0..1
}

export const LIMITES_REJILLA = { tamanoMin: 5, tamanoMax: 300, grosorMin: 0, grosorMax: 10 } as const;

const REJILLA_DEFECTO: Rejilla = { visible: true, tamano: 50, color: "#ffffff", grosor: 1, opacidad: 0.3 };

// Los cambios de rejilla se agrupan: se guardan 5 s después del último, o antes si hace falta.
const ESPERA_GUARDADO_MS = 5000;
/** Los cambios de valores actuales (vida...) se mandan 1 s después del último. */
const ESPERA_VALORES_MS = 1000;
/** Tope de un valor actual en positivo y negativo (igual que `LIMITE_VALOR_ACTUAL` en Rust). */
export const LIMITE_VALOR_ACTUAL = 16000;
const claveValor = (idInstancia: number, idDefinicion: number) => `${idInstancia}:${idDefinicion}`;

const porOrden = (a: { orden?: number }, b: { orden?: number }) => (a.orden ?? 0) - (b.orden ?? 0);

export function hexARgba(hex: string, opacidad: number): number[] {
  const limpio = hex.replace("#", "");
  const completo = limpio.length === 3 ? limpio.split("").map((x) => x + x).join("") : limpio;
  const n = parseInt(completo, 16) || 0;
  const a = Math.round(Math.min(1, Math.max(0, opacidad)) * 255);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, a];
}

export const rgbAHex = (r: number, g: number, b: number) =>
  "#" + [r, g, b].map((v) => Number(v).toString(16).padStart(2, "0")).join("");

/** ¿Está la instancia puesta en el tablero del mapa `idMapa`? (las activas sin colocar no lo están) */
export function estaEnMapa(instancia: Instancia, idMapa: number | null | undefined): boolean {
  return idMapa != null && instancia.id_mapa === idMapa && typeof instancia.coordenadas === "object";
}

/**
 * Orden completo de «Fichas activas» tras reordenar solo las que se ven (con el filtro de mapa puesto):
 * las visibles se reparten, en su orden nuevo, por los huecos que ya ocupaban, y las ocultas no se mueven.
 * `visibles` es la lista nueva de las que se ven (las mismas que había, en otro orden).
 */
export function reordenarConOcultas(todos: readonly number[], visibles: readonly number[]): number[] {
  const esVisible = new Set(visibles);
  let k = 0;
  return todos.map((id) => (esVisible.has(id) ? visibles[k++] : id));
}

/** Elemento que queda al borrar `id` de una lista ordenada: el siguiente, o si no el anterior. */
function vecinoTrasBorrar<T extends { id: number }>(ordenada: T[], id: number): T | null {
  const i = ordenada.findIndex((x) => x.id === id);
  if (i === -1) return null;
  return ordenada[i + 1] ?? ordenada[i - 1] ?? null;
}

class Partida {
  zonas = $state<Zona[]>([]);
  /** Mapas de la zona seleccionada. */
  mapas = $state<Mapa[]>([]);
  banco = $state<Entidad[]>([]);
  instancias = $state<Instancia[]>([]);
  /** Parámetros comunes a todas las fichas (vida, armadura...). */
  definiciones = $state<DefinicionParametro[]>([]);
  /** Variables de las fichas (Fuerza «f»...), usables en el chat de dados. */
  definicionesVariables = $state<DefinicionVariable[]>([]);
  /** Sus grupos (bases o derivadas), en su orden. */
  gruposVariables = $state<GrupoVariables[]>([]);
  /** Los conjuntos de parámetros de la partida y los de serie (solo el host: el jugador no los ve ni los cambia). */
  conjuntos = $state<ListaConjuntos>({ activo: 0, guardados: [], de_serie: [] });
  /** Las listas de rangos del conjunto activo (solo el host las lee: el jugador recibe en cada variable la copia de sus opciones). */
  listasRangos = $state<ListaRangos[]>([]);
  /** La escala de resultados del conjunto activo (`null` = solo «Éxito» o «Fallo»). Solo la lee el host: el jugador recibe ya el veredicto. */
  escala = $state<EscalaResultado | null>(null);
  /** Los .txt por defecto de las fichas nuevas del conjunto activo, en su orden. Solo los lee el host (el jugador recibe sus fichas ya hechas). */
  archivosTextoDefecto = $state<ArchivoTextoDefecto[]>([]);
  /** Escalas de ejemplo (Cthulhu, D&D...) con las que rellenar el editor. */
  ejemplosEscala = $state<EjemploEscala[]>([]);

  /** «Fichas activas» de arriba abajo. */
  instanciasOrdenadas = $derived([...this.instancias].sort((a, b) => a.orden - b.orden || a.id_instancia - b.id_instancia));
  /** Las que enseña «Fichas activas»: todas, o con el filtro «Este mapa» (ver `ajustesVista`) solo las puestas
   *  en el tablero del mapa abierto. En un mapa en batalla el filtro es obligatorio (el botón se bloquea). A un jugador solo le llega lo que puede ver, así que el filtro es el mismo. */
  instanciasVisibles = $derived.by(() => {
    const batalla = this.mapaActual?.batalla;
    if (!ajustesVista.soloMapaActual && !batalla) return this.instanciasOrdenadas;
    const delMapa = this.instanciasOrdenadas.filter((i) => estaEnMapa(i, this.mapaActual?.id));
    // En batalla, en el orden de las cajas (grupo a grupo): así las posiciones de la lista que se reordena coinciden con lo que se ve
    return batalla ? ordenarPorGrupos(delMapa, batalla) : delMapa;
  });

  /** Fichas con el foco (varias con Ctrl, como en el banco). Solo se guardan los ids: se actualizan solas al recargar
   *  las instancias y desaparecen si Rust las elimina (p. ej. al borrar su zona). */
  idsSeleccionados = $state<number[]>([]);
  seleccionadas = $derived(this.instancias.filter((i) => this.idsSeleccionados.includes(i.id_instancia)));
  /** La ficha con el foco si hay exactamente una (sus parámetros se ven y se pueden cambiar). */
  instanciaSeleccionada = $derived(this.seleccionadas.length === 1 ? this.seleccionadas[0] : null);

  /** Cambios de valores actuales aún sin mandar a Rust, por "idInstancia:idDefinicion" (ver `cambiarValorActual`). */
  /** Fichas que otro jugador tiene con el foco (llega por red como aviso, no se guarda). Se dibujan
   *  con otro contorno; no cambian el foco propio. */
  idsFocoAjeno = $state<number[]>([]);
  /** Último foco propio avisado a los demás, para no repetirlo. */
  #focoAvisado: number[] = [];
  /** La última instancia que tuvo el foco ELLA SOLA: de ella salen las variables de una tirada cuando ahora no hay foco. */
  #ultimaConFoco: number | null = null;

  /**
   * La instancia de la que salen las variables de una tirada (`2d6 + f`), por orden de prioridad:
   *  1. La que tiene el foco, si es UNA sola y sea de quien sea: si un jugador tiene seleccionada una ficha que no es suya,
   *     la tirada con variables va contra ella y el host la rechaza («tirada no válida»).
   *  2. Sin ninguna enfocada, la última que tuvo el foco, mientras siga existiendo. Esa solo se recuerda si la controlas
   *     (el host, todas; un jugador, las suyas): mirar la de otro no la cambia.
   *  3. Si un jugador solo tiene UNA ficha asignada, una instancia de ella (las variables son de la ficha, vale cualquiera).
   * `null` si no hay ninguna.
   */
  get fichaParaVariables(): number | null {
    const enfocada = this.instanciaSeleccionada;
    if (enfocada) return enfocada.id_instancia;
    const ultima = this.instancias.find((i) => i.id_instancia === this.#ultimaConFoco);
    if (ultima && this.controla(ultima)) return ultima.id_instancia;
    if (esJugador && this.banco.length === 1) {
      return this.instanciasOrdenadas.find((i) => i.base.id === this.banco[0].id)?.id_instancia ?? null;
    }
    return null;
  }

  valoresPendientes = $state<Record<string, number>>({});
  /** Valores ya mandados cuya confirmación aún no ha vuelto (por red, un jugador espera al host): por "idInstancia:idDefinicion",
   *  el valor que se espera (absoluto) y cuántos envíos hay en camino. Mientras esté, se ve ESTE valor y no el que
   *  haya en `instancias` (el viejo, o uno intermedio), así no parpadea entre el antiguo y el nuevo. */
  valoresEnVuelo = $state<Record<string, { valor: number; envios: number }>>({});
  /** Lo mismo para la variación de − y + (`ponerPaso`): la que se espera de cada "idEntidad:idDefinicion" hasta que se confirme. */
  pasosEnVuelo = $state<Record<string, { paso: number; envios: number }>>({});
  #temporizadorValores: ReturnType<typeof setTimeout> | null = null;

  /** Aviso de una línea (algo que falló sin que haya una ventana donde mostrarlo); desaparece solo. */
  avisos = $state("");
  #temporizadorAviso: ReturnType<typeof setTimeout> | null = null;
  avisar(texto: string) {
    this.avisos = texto;
    if (this.#temporizadorAviso) clearTimeout(this.#temporizadorAviso);
    this.#temporizadorAviso = setTimeout(() => (this.avisos = ""), 6000);
  }

  /** `null` = no hay ninguna zona (todavía no se ha creado, o se borró la última). */
  zonaId = $state<number | null>(null);
  mapaActual = $state<Mapa | null>(null);
  /** La batalla del mapa abierto (`null` = no está en batalla). */
  batallaActual = $derived(this.mapaActual?.batalla ?? null);
  rejilla = $state<Rejilla>({ ...REJILLA_DEFECTO });

  zonasOrdenadas = $derived([...this.zonas].sort(porOrden));
  mapasOrdenados = $derived([...this.mapas].sort(porOrden));

  #temporizador: ReturnType<typeof setTimeout> | null = null;
  #pendiente = false;

  // ------------------------------------------------------------------
  // Carga
  // ------------------------------------------------------------------

  async iniciar() {
    this.#escucharCambios();
    await iniciarImagenes(); // hace falta antes de mostrar ningún mapa con imagen
    await this.recargarDefiniciones();
    await this.recargarZonas();
    if (esJugador) {
      // El jugador empieza en el mapa que tiene abierto el host (si lo comparte)
      await this.#seguirAlHost();
      if (this.zonaId !== null) {
        await this.recargarMapas();
        await this.recargarEntidades();
        return;
      }
    }
    await this.seleccionarZona(this.zonasOrdenadas[0]?.id ?? null);
    await this.recargarEntidades();
  }

  /** Último mapa del host que el jugador pasó a ver (para saber cuándo CAMBIA de verdad). */
  #mapaDelHostSeguido: number | null = null;

  /**
   * Solo jugador: el mapa que ve lo decide el host. Si el host abrió otro mapa (compartido), su lista salta a la
   * zona de ese mapa; si no cambió, ver otra zona en la lista no le mueve el mapa.
   */
  async #seguirAlHost() {
    const visible = await invoke<Mapa | null>("obtener_mapa_visible");
    if (!visible) {
      this.#mapaDelHostSeguido = null;
      return;
    }
    if (visible.id === this.#mapaDelHostSeguido) return;
    this.#mapaDelHostSeguido = visible.id;
    if (visible.id_zona !== this.zonaId && this.zonas.some((z) => z.id === visible.id_zona)) {
      this.#desenfocar();
      this.zonaId = visible.id_zona;
    }
  }

  #escuchando = false;

  /**
   * La partida cambió por algo que no hizo esta pantalla: en el jugador, lo que le envía el host; en el host
   * (fase 2), lo que hagan los jugadores. Se relee lo que cambió, como tras cualquier acción propia.
   */
  #escucharCambios() {
    if (this.#escuchando) return;
    this.#escuchando = true;
    listen<Partial<PartesCambiadas>>(EVENTO_PARTIDA_CAMBIADA, (e) => void this.alCambiarPartida(e.payload)).catch((err) =>
      console.warn("partida-cambiada no disponible:", err)
    );
  }

  /** Relee lo que cambió. Si la zona o el mapa abiertos dejan de existir (o de verse), pasa al primero. */
  async alCambiarPartida(partes: Partial<PartesCambiadas> = { zonas: true, mapas: true, entidades: true, definiciones: true, tiradas: true }) {
    if (partes.tiradas) await tiradas.cargar(); // el chat es de todos: también llega lo que tiran los demás
    if (partes.definiciones) await this.recargarDefiniciones();
    if (partes.zonas || partes.mapas) {
      await this.recargarZonas();
      if (esJugador) await this.#seguirAlHost();
      if (this.zonaId === null || !this.zonas.some((z) => z.id === this.zonaId)) {
        await this.seleccionarZona(this.zonasOrdenadas[0]?.id ?? null);
      } else {
        await this.recargarMapas();
      }
    }
    if (partes.entidades) await this.recargarEntidades();
  }

  // ------------------------------------------------------------------
  // Partidas (una carpeta cada una)
  // ------------------------------------------------------------------

  /** Partida abierta ("" hasta que se lee) y todas las que hay. */
  partidas = $state<{ actual: string; lista: string[] }>({ actual: "", lista: [] });

  async recargarPartidas() {
    const { actual, partidas } = await invoke<{ actual: string; partidas: string[] }>("obtener_partidas");
    this.partidas = { actual, lista: partidas };
  }

  /** Guarda la partida abierta y abre otra. Los errores de Rust (no se pudo guardar...) se propagan,
   *  y en ese caso nada cambia. Quien llama debe recargar también las tiradas (`tiradas.cargar()`). */
  cambiarPartida(nombre: string) {
    return this.#alternar("abrir_partida", { nombre });
  }

  /**
   * Borra PARA SIEMPRE una partida. Si es la abierta, Rust pasa antes a otra (o a una nueva vacía si era la única) y aquí
   * se vuelve a leer todo, como al cambiar de partida (quien llama debe recargar también las tiradas). Si es otra, solo
   * se recarga la lista.
   */
  async borrarPartida(nombre: string) {
    await this.recargarPartidas().catch(() => {}); // cuál es la abierta, de verdad (no lo que se recuerde)
    if (nombre === this.partidas.actual) return this.#alternar("borrar_partida", { nombre });
    try {
      await invoke("borrar_partida", { nombre });
    } finally {
      await this.recargarPartidas().catch(() => {});
    }
  }

  /**
   * Cambia el nombre de una partida. Si es OTRA, solo se vuelve a leer la lista. Si es la abierta, la ruta de su carpeta
   * (la de sus imágenes, `obtener_dir_datos`) y las miniaturas guardadas cambian: se vuelve a leer todo como al cambiar de
   * partida (quien llama debe recargar también las tiradas). Rust la rechaza con la sala abierta.
   */
  async renombrarPartida(nombre: string, nuevo: string) {
    await this.recargarPartidas().catch(() => {}); // cuál es la abierta, de verdad
    if (nombre === this.partidas.actual) return this.#alternar("renombrar_partida", { nombre, nuevo });
    try {
      await invoke("renombrar_partida", { nombre, nuevo });
    } finally {
      await this.recargarPartidas().catch(() => {});
    }
  }

  /** Crea una partida vacía (con los parámetros de serie) y pasa a ella. */
  crearPartida(nombre: string) {
    return this.#alternar("crear_partida", { nombre });
  }

  async #alternar(cmd: string, args: Record<string, unknown>) {
    // Un cambio de rejilla o de valores pendiente se escribiría en la partida nueva
    await this.guardarRejilla();
    await this.guardarValoresPendientes();
    try {
      await invoke(cmd, args);
    } finally {
      // Si falló, la lista puede haber cambiado (p. ej. se creó la carpeta pero no se pudo abrir)
      await this.recargarPartidas().catch(() => {});
    }

    // Nada de lo de la partida anterior debe quedar a la vista, ni siquiera un instante
    this.zonas = [];
    this.mapas = [];
    this.banco = [];
    this.instancias = [];
    this.zonaId = null;
    this.mapaActual = null;
    this.idsSeleccionados = [];
    this.#ultimaConFoco = null;
    this.rejilla = { ...REJILLA_DEFECTO };
    miniaturas.limpiar(); // sus URLs llevan dentro la carpeta de la partida anterior
    await this.iniciar();
  }

  async recargarZonas() {
    this.zonas = await invoke<Zona[]>("obtener_zonas");
  }

  /**
   * Vuelve a leer los mapas de la zona seleccionada y abre uno.
   * `preferido`: id del mapa a abrir (por defecto, el que ya estaba abierto). Si no existe, el primero.
   */
  async recargarMapas(preferido?: number | null) {
    await this.guardarRejilla(); // que no se pierdan cambios de rejilla en curso al refrescar
    if (esJugador) {
      // La lista es de la zona que mira (sus miniaturas); el mapa abierto lo decide el host
      this.mapas = this.zonaId === null ? [] : await invoke<Mapa[]>("obtener_mapas_zona", { zonaId: this.zonaId });
      this.#abrirMapa(await invoke<Mapa | null>("obtener_mapa_visible"));
      return;
    }
    const quiero = preferido === undefined ? (this.mapaActual?.id ?? null) : preferido;

    if (this.zonaId === null) {
      this.mapas = [];
      this.#abrirMapa(null);
      return;
    }
    this.mapas = await invoke<Mapa[]>("obtener_mapas_zona", { zonaId: this.zonaId });
    const ordenados = this.mapasOrdenados;
    this.#abrirMapa(ordenados.find((m) => m.id === quiero) ?? ordenados[0] ?? null);
  }

  // ------------------------------------------------------------------
  // Parámetros de la partida (comunes a todas las fichas)
  // ------------------------------------------------------------------

  /** Los parámetros y las variables (con sus grupos) de la partida. */
  async recargarDefiniciones() {
    const [definiciones, variables, grupos] = await Promise.all([
      invoke<DefinicionParametro[]>("obtener_definiciones_parametros"),
      invoke<DefinicionVariable[]>("obtener_definiciones_variables"),
      invoke<GrupoVariables[]>("obtener_grupos_variables")
    ]);
    this.definiciones = definiciones;
    this.definicionesVariables = variables ?? [];
    this.gruposVariables = grupos ?? [];
    if (!esJugador) {
      this.conjuntos = await invoke<ListaConjuntos>("obtener_conjuntos_parametros");
      this.escala = (await invoke<EscalaResultado | null>("obtener_escala_resultado")) ?? null;
      this.listasRangos = (await invoke<ListaRangos[]>("obtener_listas_rangos")) ?? [];
      this.archivosTextoDefecto = (await invoke<ArchivoTextoDefecto[]>("obtener_archivos_texto_defecto")) ?? [];
    }
  }

  /**
   * Guarda la lista entera de .txt por defecto del conjunto activo (Rust la valida y da el motivo si falla, y asigna el id a los
   * archivos nuevos, que van con id 0). Devuelve lo guardado. Pase lo que pase se vuelve a leer lo de Rust.
   */
  async guardarArchivosTextoDefecto(lista: ArchivoTextoDefecto[]): Promise<ArchivoTextoDefecto[]> {
    try {
      return (await invoke<ArchivoTextoDefecto[]>("guardar_archivos_texto_defecto", { archivos: lista })) ?? [];
    } finally {
      this.archivosTextoDefecto = (await invoke<ArchivoTextoDefecto[]>("obtener_archivos_texto_defecto")) ?? [];
    }
  }

  /**
   * Guarda UN archivo por defecto sin tocar lo que otras filas tengan escrito sin guardar: lo guardado, con este cambiado (id > 0) o
   * añadido al final (id 0). Devuelve el archivo guardado, con su id.
   */
  async guardarArchivoTextoDefecto(archivo: ArchivoTextoDefecto): Promise<ArchivoTextoDefecto> {
    const actuales = $state.snapshot(this.archivosTextoDefecto) as ArchivoTextoDefecto[];
    const lista = archivo.id === 0 ? [...actuales, archivo] : actuales.map((a) => (a.id === archivo.id ? archivo : a));
    const guardada = await this.guardarArchivosTextoDefecto(lista);
    return archivo.id === 0 ? guardada[guardada.length - 1] : (guardada.find((a) => a.id === archivo.id) as ArchivoTextoDefecto);
  }

  /** Nuevo orden de los .txt por defecto (los ids, todos). */
  reordenarArchivosTextoDefecto(ids: number[]) {
    const actuales = $state.snapshot(this.archivosTextoDefecto) as ArchivoTextoDefecto[];
    const lista = ids.map((id) => actuales.find((a) => a.id === id)).filter((a): a is ArchivoTextoDefecto => !!a);
    return this.guardarArchivosTextoDefecto(lista.length === actuales.length ? lista : actuales);
  }

  borrarArchivoTextoDefecto(id: number) {
    const actuales = $state.snapshot(this.archivosTextoDefecto) as ArchivoTextoDefecto[];
    return this.guardarArchivosTextoDefecto(actuales.filter((a) => a.id !== id));
  }

  /** Crea una lista de rangos. Cambiar una lista cambia las variables que la usan, y con ellas las fichas: se vuelve a leer todo. */
  async crearListaRangos(lista: Omit<ListaRangos, "id">): Promise<ListaRangos> {
    let creada = { ...lista, id: 0 } as ListaRangos;
    await this.#cambiarDefiniciones("crear_lista_rangos", { lista: { ...lista, id: 0 } }, (r) => (creada = r as ListaRangos));
    return creada;
  }

  modificarListaRangos(lista: ListaRangos) {
    return this.#cambiarDefiniciones("modificar_lista_rangos", { lista });
  }

  /** Borra una lista: las variables que la usaban se quedan sin rangos. */
  borrarListaRangos(id: number) {
    return this.#cambiarDefiniciones("borrar_lista_rangos", { id });
  }

  /** Guarda (o quita, con `null`) la escala de resultados del conjunto activo. Rust la valida entera y da el motivo si falla. */
  async guardarEscala(escala: EscalaResultado | null) {
    try {
      await invoke("guardar_escala_resultado", { escala });
    } finally {
      this.escala = (await invoke<EscalaResultado | null>("obtener_escala_resultado")) ?? null;
    }
  }

  /** Lee las escalas de ejemplo (una vez). */
  async cargarEjemplosEscala() {
    if (this.ejemplosEscala.length === 0) this.ejemplosEscala = (await invoke<EjemploEscala[]>("obtener_ejemplos_escala")) ?? [];
  }

  /** Crear, borrar o reordenar un parámetro cambia también las fichas: se vuelve a leer todo.
   *  Los errores de Rust (nombre repetido...) se propagan para que los muestre quien llama. */
  async #cambiarDefiniciones(cmd: string, args: Record<string, unknown>, alResponder?: (r: unknown) => void) {
    try {
      const respuesta = await invoke(cmd, args);
      alResponder?.(respuesta);
    } finally {
      await Promise.all([this.recargarDefiniciones(), this.recargarEntidades()]);
    }
  }

  crearDefinicion(definicion: Omit<DefinicionParametro, "id">) {
    return this.#cambiarDefiniciones("crear_definicion_parametro", { definicion: { ...definicion, id: 0 } });
  }

  /** Cambia la configuración de un parámetro (no su tipo). Las fichas conservan sus valores. */
  modificarDefinicion(definicion: DefinicionParametro) {
    return this.#cambiarDefiniciones("modificar_definicion_parametro", { definicion });
  }

  /** Desaparece de la lista y del valor de todas las fichas. */
  borrarDefinicion(id: number) {
    return this.#cambiarDefiniciones("borrar_definicion_parametro", { id });
  }

  /** `ids` = todos los parámetros en el orden nuevo (el que seguirán las fichas). */
  reordenarDefiniciones(ids: number[]) {
    return this.#cambiarDefiniciones("reordenar_definiciones_parametros", { ids });
  }

  // ------------------------------------------------------------------
  // Variables de la partida (Fuerza «f»...): las define el host
  // ------------------------------------------------------------------

  /** Devuelve la variable creada (con el id que le puso Rust). */
  async crearVariable(definicion: Omit<DefinicionVariable, "id">): Promise<DefinicionVariable> {
    let creada: DefinicionVariable | null = null;
    await this.#cambiarDefiniciones("crear_definicion_variable", { definicion: { ...definicion, id: 0 } }, (r) => (creada = r as DefinicionVariable));
    return creada!;
  }

  /** Cambia nombre, abreviatura, neutro o escala. Las fichas conservan sus valores. */
  modificarVariable(definicion: DefinicionVariable) {
    return this.#cambiarDefiniciones("modificar_definicion_variable", { definicion });
  }

  /** Pasa la variable a otro grupo del mismo tipo (Rust rechaza bases ↔ derivadas). */
  moverVariableAGrupo(id: number, grupo: number) {
    return this.#cambiarDefiniciones("mover_variable_a_grupo", { id, grupo });
  }

  /** Desaparece de la lista y de todas las fichas, en cascada (ver `efectoDeBorrar` en $lib/variables). */
  borrarVariable(id: number) {
    return this.#cambiarDefiniciones("borrar_definicion_variable", { id });
  }

  // ------------------------------------------------------------------
  // Conjuntos de parámetros: varios guardados, uno en uso, exportar e importar (solo el host)
  // ------------------------------------------------------------------

  /** ¿Hay una ficha NUEVA abierta en el editor? Entonces no se puede cambiar de conjunto (su borrador es del anterior). */
  get hayFichaNuevaAbierta(): boolean {
    return !!this.entidadEnEdicion && this.editorEsBorrador;
  }

  /**
   * Pasa a usar el conjunto `id`. Las fichas pasan a los valores que tenían con ese conjunto (o a los de por defecto si es la
   * primera vez) y lo de ahora se guarda en el que se deja: la pantalla avisa ANTES. Lo que estaba pendiente de mandar (cambios del
   * valor actual de un parámetro con máximo...) era del conjunto anterior y se descarta, y la ficha abierta en el editor se cierra
   * sin guardar.
   */
  async usarConjunto(id: number) {
    if (this.#temporizadorValores) clearTimeout(this.#temporizadorValores);
    this.#temporizadorValores = null;
    this.valoresPendientes = {};
    this.valoresEnVuelo = {};
    this.pasosEnVuelo = {};
    if (this.entidadEnEdicion && !this.editorEsBorrador) this.cerrarEditorEntidad();
    await this.#cambiarDefiniciones("usar_conjunto", { id });
  }

  /** Copia un conjunto de serie a la partida (sin usarlo) y devuelve su id. */
  async anadirConjuntoDeSerie(clave: string): Promise<number> {
    let id = 0;
    await this.#cambiarDefiniciones("anadir_conjunto_de_serie", { clave }, (r) => (id = r as number));
    return id;
  }

  /**
   * Importa el conjunto que lleva dentro la plantilla de ficha `ruta` (un .html exportado por OpenRoll, rellena o no: solo se lee el
   * conjunto) sin usarlo, y devuelve su id. Rust lo valida entero y da el motivo si falla.
   */
  async importarConjunto(ruta: string): Promise<number> {
    let id = 0;
    await this.#cambiarDefiniciones("importar_conjunto", { ruta }, (r) => (id = r as number));
    return id;
  }

  /** Los conjuntos de la biblioteca del host (compartida entre todas sus partidas). */
  listarBiblioteca() {
    return invoke<ResumenBiblioteca[]>("listar_biblioteca_conjuntos");
  }

  /** Guarda (o actualiza, si ya hay uno con ese nombre) el conjunto `id` en la biblioteca. */
  guardarEnBiblioteca(id: number) {
    return invoke<string>("guardar_en_biblioteca", { id });
  }

  /** Copia un conjunto de la biblioteca a esta partida (sin usarlo) y devuelve su id. */
  async anadirDeBiblioteca(archivo: string): Promise<number> {
    let id = 0;
    await this.#cambiarDefiniciones("anadir_de_biblioteca", { archivo }, (r) => (id = r as number));
    return id;
  }

  borrarDeBiblioteca(archivo: string) {
    return invoke("borrar_de_biblioteca", { archivo });
  }

  renombrarConjunto(id: number, nombre: string) {
    return this.#cambiarDefiniciones("renombrar_conjunto", { id, nombre });
  }

  /** Borra un conjunto guardado (no el que se usa). No toca las fichas. */
  borrarConjunto(id: number) {
    return this.#cambiarDefiniciones("borrar_conjunto", { id });
  }

  /** Crea un grupo de variables (bases o derivadas) y lo devuelve con el id que le puso Rust. */
  async crearGrupoVariables(nombre: string, tipo: TipoGrupoVariables): Promise<GrupoVariables> {
    let creado: GrupoVariables | null = null;
    await this.#cambiarDefiniciones("crear_grupo_variables", { nombre, tipo }, (r) => (creado = r as GrupoVariables));
    return creado!;
  }

  renombrarGrupoVariables(id: number, nombre: string) {
    return this.#cambiarDefiniciones("renombrar_grupo_variables", { id, nombre });
  }

  /** Borra el grupo y sus variables, en cascada. */
  borrarGrupoVariables(id: number) {
    return this.#cambiarDefiniciones("borrar_grupo_variables", { id });
  }

  /** `ids` = todas las variables en el orden nuevo (el que seguirán las fichas). */
  reordenarVariables(ids: number[]) {
    return this.#cambiarDefiniciones("reordenar_definiciones_variables", { ids });
  }

  /** Rust también cambia el banco y las instancias al borrar zonas o mapas: se vuelven a leer. */
  async recargarEntidades() {
    const [banco, instancias] = await Promise.all([
      invoke<Entidad[]>("obtener_banco_entidades"),
      invoke<Instancia[]>("obtener_instancias_activas")
    ]);
    this.banco = banco;
    this.instancias = instancias;
    // Una instancia que llega, sale o se oculta cambia la batalla (la sanea Rust): se vuelve a leer si hay alguna
    if (!esJugador && this.mapas.some((m) => m.batalla)) await this.recargarBatallas();
    // Las seleccionadas que ya no existen (borradas, o se fue su zona) dejan de estarlo
    const ids = this.idsBancoSeleccionados.filter((id) => banco.some((e) => e.id === id));
    if (ids.length !== this.idsBancoSeleccionados.length) this.idsBancoSeleccionados = ids;
  }

  // ------------------------------------------------------------------
  // Batalla: turnos por mapa (Rust decide; aquí solo se lee y se pide)
  // ------------------------------------------------------------------

  /** Vuelve a leer solo la batalla de los mapas de la zona (sin reabrir el mapa ni mover la vista). */
  async recargarBatallas() {
    if (this.zonaId === null) return;
    const mapas = (await invoke<Mapa[]>("obtener_mapas_zona", { zonaId: this.zonaId })) ?? [];
    for (const m of mapas) {
      const local = this.mapas.find((x) => x.id === m.id);
      if (local) local.batalla = m.batalla ?? null;
    }
    const actual = mapas.find((m) => m.id === this.mapaActual?.id);
    if (this.mapaActual && actual) this.mapaActual.batalla = actual.batalla ?? null;
  }

  /** Una acción sobre los grupos de la batalla del mapa abierto (solo el host); después se vuelve a leer la batalla. */
  async #grupoBatalla(cmd: string, args: Record<string, unknown>) {
    const idMapa = this.mapaActual?.id;
    if (idMapa === undefined || esJugador) return;
    try {
      await invoke(cmd, { idMapa, ...args });
    } catch (err) {
      this.avisar(String(err));
    } finally {
      await this.recargarBatallas();
    }
  }

  /** Pasa estas instancias a un grupo (`0` = «No participa»). */
  moverAGrupoBatalla(ids: number[], idGrupo: number) {
    return this.#grupoBatalla("mover_a_grupo_batalla", { ids, idGrupo });
  }

  /** Mete el grupo `origen` entero en `destino` (que conserva su turno y su contador). */
  fusionarGruposBatalla(origen: number, destino: number) {
    return this.#grupoBatalla("fusionar_grupos_batalla", { origen, destino });
  }

  crearGrupoBatalla() {
    return this.#grupoBatalla("crear_grupo_batalla", { nombre: null });
  }

  renombrarGrupoBatalla(idGrupo: number, nombre: string) {
    return this.#grupoBatalla("renombrar_grupo_batalla", { idGrupo, nombre });
  }

  borrarGrupoBatalla(idGrupo: number) {
    return this.#grupoBatalla("borrar_grupo_batalla", { idGrupo });
  }

  /** ¿Tiene esta instancia el turno? */
  tieneElTurno(instancia: Instancia): boolean {
    return tieneElTurno(this.batallaActual, instancia.id_instancia);
  }

  /** ¿Puede quien mira pasar el turno de este grupo? (el host, siempre; un jugador, si la ficha que lo tiene es suya) */
  puedePasarTurno(grupo: GrupoBatalla): boolean {
    const i = this.instancias.find((x) => x.id_instancia === grupo.turno);
    return !!i && (!esJugador || (!i.oculto && this.controla(i)));
  }

  /** Pasa el turno del grupo al siguiente (Rust vuelve a comprobar quién puede). */
  async pasarTurno(idGrupo: number) {
    const idMapa = this.mapaActual?.id;
    if (idMapa === undefined) return;
    try {
      await invoke("pasar_turno", { idMapa, idGrupo });
      await this.recargarBatallas();
    } catch (err) {
      this.avisar(String(err));
    }
  }

  /** Enter en el tablero: pasa el turno del grupo que toca (ver `grupoParaEnter`). `true` si pasó alguno. */
  pasarTurnoConEnter(): boolean {
    const grupo = grupoParaEnter({
      batalla: this.batallaActual,
      esJugador,
      controla: (id) => {
        const i = this.instancias.find((x) => x.id_instancia === id);
        return !!i && (!esJugador || (!i.oculto && this.controla(i)));
      },
      enFoco: this.instanciaSeleccionada?.id_instancia ?? null
    });
    if (!grupo) return false;
    void this.pasarTurno(grupo.id);
    return true;
  }

  // ------------------------------------------------------------------
  // Selección múltiple en el banco (Ctrl+clic). Solo de esta pantalla: no se guarda ni se avisa.
  // ------------------------------------------------------------------

  /** Fichas del banco seleccionadas, en el orden en que se eligieron. */
  idsBancoSeleccionados = $state<number[]>([]);

  alternarSeleccionBanco(id: number) {
    const ids = this.idsBancoSeleccionados;
    this.idsBancoSeleccionados = ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
  }

  limpiarSeleccionBanco() {
    if (this.idsBancoSeleccionados.length > 0) this.idsBancoSeleccionados = [];
  }

  /** Manda a la papelera varias fichas de una vez (un Ctrl+Z las devuelve todas). */
  async borrarEntidades(ids: number[]) {
    if (ids.length === 0) return;
    try {
      await invoke("borrar_entidades", { ids });
    } catch (err) {
      console.error("borrar_entidades:", err);
    }
    await this.recargarEntidades();
    if (this.entidadEnEdicion && ids.includes(this.entidadEnEdicion.id)) this.cerrarEditorEntidad();
    this.idsBancoSeleccionados = this.idsBancoSeleccionados.filter((id) => !ids.includes(id));
  }

  /**
   * Ctrl+Z: devuelve lo último que se borró (fichas, un mapa o una zona). Las creaciones no se
   * deshacen. Devuelve false si la papelera estaba vacía.
   */
  async restaurarPapelera(): Promise<boolean> {
    let r: { zonas: number[]; mapas: number[]; entidades: number[] } | null;
    try {
      r = await invoke("restaurar_papelera");
    } catch (err) {
      console.error("restaurar_papelera:", err);
      return false;
    }
    if (!r) return false;
    await this.recargarZonas();
    if (this.zonaId === null || r.zonas.length > 0) {
      // Se abre la zona recuperada (o, si no había ninguna abierta, la primera)
      await this.seleccionarZona(r.zonas[0] ?? this.zonasOrdenadas[0]?.id ?? null);
    }
    await this.recargarMapas(this.mapaActual === null ? (r.mapas[0] ?? undefined) : undefined);
    await this.recargarEntidades();
    return true;
  }

  // ------------------------------------------------------------------
  // Selección
  // ------------------------------------------------------------------

  async seleccionarZona(id: number | null) {
    if (id === this.zonaId) return;
    this.#desenfocar();
    await this.guardarRejilla(); // cambios pendientes del mapa que se abandona
    this.zonaId = id;
    await this.recargarMapas(null);
  }

  async seleccionarMapa(mapa: Mapa | null) {
    if (esJugador) return; // el jugador no elige mapa: ve el que tiene abierto el host
    if (!mapa || mapa.id === this.mapaActual?.id) return;
    this.#desenfocar();
    await this.guardarRejilla();
    this.#abrirMapa(mapa);
  }

  // ------------------------------------------------------------------
  // Fichas (instancias activas)
  // ------------------------------------------------------------------

  /**
   * Da el foco a una ficha (`null` = a ninguna). Con `anadir` (Ctrl) la añade o la quita del
   * conjunto en vez de sustituirlo. Los valores cambiados de la que pierde el foco se guardan ya.
   */
  seleccionarInstancia(instancia: Instancia | null, anadir = false) {
    const ids = this.idsSeleccionados;
    let nuevos: number[];
    if (!instancia) nuevos = [];
    else if (!anadir) nuevos = [instancia.id_instancia];
    else if (ids.includes(instancia.id_instancia)) nuevos = ids.filter((id) => id !== instancia.id_instancia);
    else nuevos = [...ids, instancia.id_instancia];

    if (nuevos.length === ids.length && nuevos.every((id, i) => id === ids[i])) return;
    this.#cambiarFoco(nuevos);
  }

  /**
   * Da el foco a varias fichas a la vez (el recuadro de Ctrl+arrastrar en el tablero). Con `anadir` se
   * suman a las que ya lo tenían; si no, las sustituyen.
   */
  seleccionarInstancias(ids: readonly number[], anadir = false) {
    const actuales = this.idsSeleccionados;
    const nuevos = anadir ? [...actuales, ...ids.filter((id) => !actuales.includes(id))] : [...new Set(ids)];
    if (nuevos.length === actuales.length && nuevos.every((id, i) => id === actuales[i])) return;
    this.#cambiarFoco(nuevos);
  }

  // Cambia el foco propio, guarda lo pendiente de las fichas que lo pierden y avisa a los demás.
  #cambiarFoco(nuevos: number[]) {
    this.idsSeleccionados = nuevos;
    // Solo se recuerda una ficha que controlas: tocar la de otro no cambia de qué ficha salen tus variables
    if (nuevos.length === 1) {
      const instancia = this.instancias.find((i) => i.id_instancia === nuevos[0]);
      if (instancia && this.controla(instancia)) this.#ultimaConFoco = nuevos[0];
    }
    void this.guardarValoresPendientes();
    // El foco es solo de este momento: se avisa (por red, cuando la haya) pero no se guarda en disco
    const igual = nuevos.length === this.#focoAvisado.length && nuevos.every((id, i) => id === this.#focoAvisado[i]);
    if (igual) return;
    this.#focoAvisado = [...nuevos];
    enviarAviso(avisoFoco(nuevos));
  }

  /** Lo que otro jugador tiene con el foco ahora (sustituye a lo anterior). */
  recibirFocoAjeno(ids: number[]) {
    this.idsFocoAjeno = [...ids];
  }

  estaSeleccionada(idInstancia: number) {
    return this.idsSeleccionados.includes(idInstancia);
  }

  /**
   * Con una sola ficha con foco, lo pasa a la anterior (-1) o la siguiente (+1) de «Fichas activas» (de las
   * que se ven, con el filtro de mapa puesto). Es circular: tras la última va la primera y antes de la
   * primera, la última. Si la que tiene el foco está oculta por el filtro, empieza por los extremos.
   */
  seleccionarVecina(paso: -1 | 1): boolean {
    const actual = this.instanciaSeleccionada;
    if (!actual || this.idsSeleccionados.length !== 1) return false;
    const lista = this.instanciasVisibles;
    const i = lista.findIndex((x) => x.id_instancia === actual.id_instancia);
    if (i === -1) {
      if (lista.length === 0) return false;
      this.seleccionarInstancia(lista[paso === 1 ? 0 : lista.length - 1]);
      return true;
    }
    if (lista.length < 2) return false;
    this.seleccionarInstancia(lista[(i + paso + lista.length) % lista.length]);
    return true;
  }

  // ------------------------------------------------------------------
  // Valores actuales (vida...): se agrupan antes de mandarlos
  // ------------------------------------------------------------------

  /**
   * La ficha se ve atenuada (gris) si algún parámetro marcado con «atenuar_si_negativo» (la vida de
   * serie lo está) tiene su valor actual en 0 o menos. Lo decide la definición, no ningún id fijo.
   */
  estaCaida(instancia: Instancia): boolean {
    if (instancia.atenuada) return true;
    return this.definiciones.some(
      (d) =>
        d.atenuar_si_negativo &&
        d.tiene_maximo &&
        instancia.valores_actuales.some((v) => v.id_definicion === d.id) &&
        this.valorMostrado(instancia, d.id) <= 0
    );
  }

  /** Valor actual que se ve: el de Rust más lo que aún no se ha mandado. */
  valorMostrado(instancia: Instancia, idDefinicion: number): number {
    return this.#base(instancia, idDefinicion) + (this.valoresPendientes[claveValor(instancia.id_instancia, idDefinicion)] ?? 0);
  }

  /** Sobre lo que se suman los pasos aún sin mandar: lo que se espera de lo ya mandado o, si no hay nada en camino, lo de Rust. */
  #base(instancia: Instancia, idDefinicion: number): number {
    return this.valoresEnVuelo[claveValor(instancia.id_instancia, idDefinicion)]?.valor ?? valorActual(instancia, idDefinicion);
  }

  /**
   * Suma `delta` a un valor con máximo (p. ej. la vida). Puede pasar del máximo (vida extra) o bajar
   * de 0 (vida negativa). Se ve al momento pero
   * no se manda a Rust hasta 1 s después del último cambio (o antes si la ficha pierde el foco):
   * 50 pasos de rueda son UN delta de 50, no 50 deltas.
   */
  cambiarValorActual(idInstancia: number, idDefinicion: number, delta: number) {
    const instancia = this.instancias.find((i) => i.id_instancia === idInstancia);
    if (!instancia || delta === 0 || !this.controla(instancia)) return;
    const clave = claveValor(idInstancia, idDefinicion);
    const actual = this.#base(instancia, idDefinicion);
    // El máximo es solo el valor por defecto: se puede pasar de él o bajar de 0 (como en Rust)
    const nuevo = Math.min(LIMITE_VALOR_ACTUAL, Math.max(-LIMITE_VALOR_ACTUAL, this.valorMostrado(instancia, idDefinicion) + delta));
    if (nuevo === actual) delete this.valoresPendientes[clave];
    else this.valoresPendientes[clave] = nuevo - actual;

    if (this.#temporizadorValores) clearTimeout(this.#temporizadorValores);
    this.#temporizadorValores = setTimeout(() => void this.guardarValoresPendientes(), ESPERA_VALORES_MS);
  }

  /** Manda ya los valores pendientes: uno por cada valor cambiado, con la suma de todos sus pasos. */
  async guardarValoresPendientes() {
    if (this.#temporizadorValores) clearTimeout(this.#temporizadorValores);
    this.#temporizadorValores = null;
    const enviados = Object.entries(this.valoresPendientes).filter(([, delta]) => delta !== 0);
    if (enviados.length === 0) return;

    // Lo que se va a mandar sale de «pendiente» y pasa a «en camino» YA, antes de esperar: así la pantalla enseña
    // el valor que se espera hasta que Rust (o el host, por red) lo confirme, y lo que se cambie mientras tanto
    // se suma encima en vez de pisarse
    for (const [clave, delta] of enviados) {
      const [idInstancia, idDefinicion] = clave.split(":").map(Number);
      const instancia = this.instancias.find((i) => i.id_instancia === idInstancia);
      const base = this.valoresEnVuelo[clave]?.valor ?? (instancia ? valorActual(instancia, idDefinicion) : 0);
      this.valoresEnVuelo[clave] = { valor: base + delta, envios: (this.valoresEnVuelo[clave]?.envios ?? 0) + 1 };
      const resto = (this.valoresPendientes[clave] ?? 0) - delta;
      if (resto === 0) delete this.valoresPendientes[clave];
      else this.valoresPendientes[clave] = resto;
    }

    try {
      for (const [clave, delta] of enviados) {
        const [idInstancia, idDefinicion] = clave.split(":").map(Number);
        try {
          await invoke("modificar_valor_actual", { idInstancia, idDefinicion, delta });
        } catch (err) {
          console.error("modificar_valor_actual:", err);
        }
      }
      this.instancias = await invoke<Instancia[]>("obtener_instancias_activas");
    } finally {
      // Confirmado (o fallido: entonces se ve lo que de verdad hay): se deja de esperar, salvo que haya otro envío en camino
      for (const [clave] of enviados) {
        const e = this.valoresEnVuelo[clave];
        if (!e) continue;
        if (e.envios <= 1) delete this.valoresEnVuelo[clave];
        else e.envios--;
      }
    }
  }

  // ------------------------------------------------------------------
  // Instancias
  // ------------------------------------------------------------------

  /** Gira la ficha un cuarto de vuelta en el sentido del reloj (Rust la mete en el mapa si no cabe). */
  async rotarInstancia(idInstancia: number) {
    await this.#cambiarInstancias("rotar_instancia", { idInstancia });
  }

  /** Pone o quita el espejo de la imagen de la ficha (Ctrl + clic derecho en el tablero). */
  async espejarInstancia(idInstancia: number) {
    // Se ve al momento; Rust lo confirma al releer
    this.instancias = this.instancias.map((i) => (i.id_instancia === idInstancia ? { ...i, espejo: !i.espejo } : i));
    await this.#cambiarInstancias("espejar_instancia", { idInstancia });
  }

  /** Nuevo orden de «Fichas activas» (todos los ids, de arriba abajo). */
  async reordenarInstancias(ids: number[]) {
    // Se ve al momento; Rust lo confirma al releer
    this.instancias = this.instancias.map((i) => ({ ...i, orden: Math.max(0, ids.indexOf(i.id_instancia)) }));
    await this.#cambiarInstancias("reordenar_instancias", { ids });
  }

  /**
   * Instancia una ficha del banco en una casilla (`destino`) o solo como activa (null). Una ficha
   * única que ya tenía instancia la reutiliza (Rust decide). Soltada en «Fichas activas» queda
   * seleccionada; en el tablero no (así no se dibujan sus formas al soltarla).
   */
  async instanciar(idEntidad: number, destino: Destino | null, oculto = false) {
    const instancia = await this.#cambiarInstancias<Instancia>("instanciar_entidad", { idEntidad, destino, oculto });
    if (instancia && destino === null) this.seleccionarInstancia(instancia);
  }

  /**
   * Instancia varias fichas del banco de golpe (selección múltiple). En el tablero la primera cae en
   * `destino` y las demás en cuadrícula a su lado (Rust); con null, todas activas sin colocar.
   * Como con una sola: en «Fichas activas» quedan todas con el foco; en el tablero, ninguna.
   */
  async instanciarVarias(idsEntidades: number[], destino: Destino | null, oculto = false) {
    const creadas = await this.#cambiarInstancias<Instancia[]>("instanciar_entidades", { ids: idsEntidades, destino, oculto });
    if (creadas?.length && destino === null) this.#cambiarFoco(creadas.map((i) => i.id_instancia));
  }

  /** Cuánto suman o restan − y + del valor `idDefinicion` de esa ficha (1 si no se ha cambiado). Es de la ENTIDAD. Mientras el
   *  host no confirme un cambio (un jugador con latencia), se ve el que se acaba de poner y no el de Rust. */
  pasoDe(entidad: Entidad, idDefinicion: number): number {
    return this.pasosEnVuelo[`${entidad.id}:${idDefinicion}`]?.paso ?? entidad.pasos_valor?.find((p) => p.id_definicion === idDefinicion)?.paso ?? 1;
  }

  /**
   * Cambia el paso de − y + de ese valor en la ficha `idEntidad`, que comparten todas sus instancias. Lo guarda Rust
   * (el host, o un jugador de una ficha suya). Se ve al momento y se sigue viendo hasta que Rust lo confirme y se relea
   * (`pasosEnVuelo`), aunque en medio llegue una lectura vieja. Devuelve el que queda.
   */
  async ponerPaso(idEntidad: number, idDefinicion: number, paso: number): Promise<number> {
    const n = Math.trunc(Number(paso));
    if (!Number.isFinite(n) || n < 1) return 1;
    const valido = Math.min(n, PASO_VALOR_MAX);
    const clave = `${idEntidad}:${idDefinicion}`;
    this.pasosEnVuelo[clave] = { paso: valido, envios: (this.pasosEnVuelo[clave]?.envios ?? 0) + 1 };
    try {
      await invoke("poner_paso_valor", { idEntidad, idDefinicion, paso: valido });
    } catch (err) {
      console.error("poner_paso_valor:", err);
    } finally {
      // Primero se relee y luego se deja de esperar: así no hay un instante con el valor viejo. Si hay otro envío en camino,
      // sigue valiendo el último que se puso (cada uno lo sobrescribe)
      await this.recargarEntidades().catch(() => {});
      const e = this.pasosEnVuelo[clave];
      if (e) {
        if (e.envios <= 1) delete this.pasosEnVuelo[clave];
        else e.envios--;
      }
    }
    return valido;
  }

  /** Oculta o muestra una instancia (solo el host): oculta, a los jugadores no les llega nada de ella. */
  async ocultarInstancia(idInstancia: number, oculto: boolean) {
    // Se ve al momento; Rust lo confirma al releer
    this.instancias = this.instancias.map((i) => (i.id_instancia === idInstancia ? { ...i, oculto } : i));
    await this.#cambiarInstancias("ocultar_instancia", { idInstancia, oculto });
  }

  /** Pone una instancia en otra casilla (la casilla final la encaja Rust). */
  async moverInstancia(idInstancia: number, destino: Destino) {
    const rejilla = this.#rejillaDelMapa(destino.id_mapa);
    const inst = this.instancias.find((i) => i.id_instancia === idInstancia);
    if (rejilla && inst) {
      this.#adelantarPosiciones(destino.id_mapa, new Map([[idInstancia, encajar(rejilla, destino.casilla, ladosInstancia(inst))]]));
    }
    await this.#cambiarInstancias("mover_instancia", { idInstancia, destino });
  }

  /**
   * Mueve juntas varias fichas del mapa `idMapa` `(dx, dy)` casillas, conservando sus distancias
   * (las seleccionadas que se arrastran a la vez). Rust las recorta en el borde del mapa.
   */
  async moverInstancias(ids: number[], idMapa: number, dx: number, dy: number) {
    const rejilla = this.#rejillaDelMapa(idMapa);
    const grupo = ids.flatMap((id) => {
      const i = this.instancias.find((x) => x.id_instancia === id);
      return i && typeof i.coordenadas === "object" ? [{ id, casilla: i.coordenadas.Casilla, lados: ladosInstancia(i) }] : [];
    });
    if (rejilla && grupo.length === ids.length) {
      const destinos = desplazarGrupo(rejilla, grupo, dx, dy);
      this.#adelantarPosiciones(idMapa, new Map(grupo.map((f, k) => [f.id, destinos[k]])));
    }
    await this.#cambiarInstancias("mover_instancias", { ids, idMapa, dx, dy });
  }

  /** La saca del tablero; sigue en «Fichas activas». */
  async quitarDelTablero(idInstancia: number) {
    // Se quita ya del tablero (si no, al soltarla se vería un instante donde estaba)
    this.instancias = this.instancias.map((i) =>
      i.id_instancia === idInstancia ? { ...i, id_mapa: null, coordenadas: "NoPuesto" as const } : i
    );
    await this.#cambiarInstancias("mover_instancia", { idInstancia, destino: null });
  }

  /** Rejilla de un mapa con su tamaño guardado: la misma con la que calcula Rust. */
  #rejillaDelMapa(idMapa: number) {
    const mapa = this.mapas.find((m) => m.id === idMapa) ?? (this.mapaActual?.id === idMapa ? this.mapaActual : null);
    return mapa ? rejillaDe(mapa.ancho, mapa.alto, mapa.rejilla) : null;
  }

  /**
   * Pone ya las fichas donde caerán (las mismas cuentas que Rust) y encima de las demás, sin esperar a
   * que Rust conteste: si no, al soltarlas se verían un instante en su casilla de antes. Lo que diga
   * Rust al releer manda.
   */
  #adelantarPosiciones(idMapa: number, casillas: Map<number, Casilla>) {
    let capa = Math.max(-1, ...this.instancias.map((i) => i.capa ?? 0)) + 1;
    // Las del grupo suben en el orden en que estaban apiladas, como en Rust
    const capas = new Map(
      this.instancias
        .filter((i) => casillas.has(i.id_instancia))
        .sort((a, b) => (a.capa ?? 0) - (b.capa ?? 0))
        .map((i) => [i.id_instancia, capa++])
    );
    this.instancias = this.instancias.map((i) => {
      const casilla = casillas.get(i.id_instancia);
      return casilla ? { ...i, id_mapa: idMapa, coordenadas: { Casilla: casilla }, capa: capas.get(i.id_instancia) } : i;
    });
  }

  /** Saca varias del tablero (siguen en «Fichas activas»). */
  async quitarVariasDelTablero(ids: number[]) {
    for (const idInstancia of ids) await this.quitarDelTablero(idInstancia);
  }

  /** Varias dejan de estar activas (Supr, o arrastrarlas juntas al banco). */
  async borrarInstancias(ids: number[]) {
    for (const id of ids) await this.borrarInstancia(id);
  }

  /** Deja de estar activa. */
  async borrarInstancia(idInstancia: number) {
    // Lo pendiente de una ficha que desaparece ya no hace falta
    for (const clave of Object.keys(this.valoresPendientes)) {
      if (clave.startsWith(`${idInstancia}:`)) delete this.valoresPendientes[clave];
    }
    for (const clave of Object.keys(this.valoresEnVuelo)) {
      if (clave.startsWith(`${idInstancia}:`)) delete this.valoresEnVuelo[clave];
    }
    await this.#cambiarInstancias("borrar_instancia", { idInstancia });
    if (this.idsSeleccionados.includes(idInstancia)) {
      this.#cambiarFoco(this.idsSeleccionados.filter((id) => id !== idInstancia));
    }
  }

  // Llama a Rust y vuelve a leer las instancias. Un error (instancia o mapa que ya no existe) se
  // muestra en consola: el arrastre ya terminó y no hay dónde enseñarlo.
  async #cambiarInstancias<T = void>(comando: string, args: Record<string, unknown>): Promise<T | undefined> {
    try {
      return await invoke<T>(comando, args);
    } catch (err) {
      console.error(`${comando}:`, err);
      return undefined;
    } finally {
      this.instancias = await invoke<Instancia[]>("obtener_instancias_activas");
    }
  }

  // ------------------------------------------------------------------
  // Zonas
  // ------------------------------------------------------------------

  async crearZona() {
    const zona = await invoke<Zona>("crear_zona");
    await this.recargarZonas();
    await this.seleccionarZona(zona.id);
  }

  async renombrarZona(zona: Zona, nombre: string) {
    await invoke("modificar_zona", { zona: { ...zona, nombre } });
    await this.recargarZonas();
  }

  /** `ids` en el orden nuevo. Solo se escriben las zonas cuya posición ha cambiado. */
  async reordenarZonas(ids: number[]) {
    for (let i = 0; i < ids.length; i++) {
      const zona = this.zonas.find((z) => z.id === ids[i]);
      if (zona && zona.orden !== i) {
        await invoke("modificar_zona", { zona: { ...zona, orden: i } });
      }
    }
    await this.recargarZonas();
  }

  async borrarZona(zona: Zona) {
    const eraLaActual = zona.id === this.zonaId;
    const siguiente = vecinoTrasBorrar(this.zonasOrdenadas, zona.id);

    // Los cambios de rejilla pendientes serían de un mapa que va a desaparecer: se descartan.
    if (eraLaActual) this.#descartarRejillaPendiente();

    await invoke("eliminar_zona", { zona });
    await this.recargarZonas();
    await this.recargarEntidades(); // Rust quita/desactiva fichas y entidades de esa zona

    if (eraLaActual) {
      // Pasamos a la zona vecina (o a ninguna si era la última): sin esto la pantalla seguía
      // mostrando los mapas de una zona que ya no existe.
      await this.seleccionarZona(siguiente?.id ?? null);
    }
  }

  // ------------------------------------------------------------------
  // Mapas
  // ------------------------------------------------------------------

  /** `rutaOrigen`: imagen del sistema para el fondo (opcional). Se copia a la carpeta de datos. */
  async crearMapa(rutaOrigen: string | null = null) {
    if (this.zonaId === null) return;

    let referencia: string | null = null;
    if (rutaOrigen) {
      try {
        referencia = await importarImagen(rutaOrigen, "mapa");
      } catch (err) {
        console.error("No se pudo importar la imagen:", err);
        return;
      }
    }

    const nuevo = await invoke<Mapa>("crear_mapa", { idZona: this.zonaId, urlImagenFondo: referencia });
    await this.recargarMapas(nuevo.id); // abre el mapa nuevo
  }

  /** La casilla del mapa: lo ven (o dejan de ver) los jugadores de la sala, al momento. */
  async compartirMapa(mapa: Mapa, compartido: boolean) {
    await invoke("compartir_mapa", { id: mapa.id, compartido });
    await this.recargarMapas();
  }

  async renombrarMapa(mapa: Mapa, nombre: string) {
    await this.#editarMapa(mapa.id, { nombre });
  }

  async cambiarImagenMapa(mapa: Mapa, rutaOrigen: string) {
    let referencia: string;
    try {
      referencia = await importarImagen(rutaOrigen, "mapa");
    } catch (err) {
      console.error("No se pudo importar la imagen:", err);
      return;
    }
    await this.#editarMapa(mapa.id, { url_imagen_fondo: referencia });
  }

  async reordenarMapas(ids: number[]) {
    await this.guardarRejilla();
    for (let i = 0; i < ids.length; i++) {
      const mapa = this.mapas.find((m) => m.id === ids[i]);
      if (mapa && mapa.orden !== i) {
        await invoke("modificar_mapa", { mapa: { ...mapa, orden: i } });
      }
    }
    await this.recargarMapas();
  }

  async borrarMapa(mapa: Mapa) {
    const eraElActual = mapa.id === this.mapaActual?.id;
    const siguiente = vecinoTrasBorrar(this.mapasOrdenados, mapa.id);

    if (eraElActual) this.#descartarRejillaPendiente();

    await invoke("borrar_mapa", { mapa });
    await this.recargarMapas(eraElActual ? (siguiente?.id ?? null) : undefined);
    await this.recargarEntidades(); // las fichas que estaban en ese mapa cambian
  }

  /**
   * Cambia campos de un mapa. Primero guarda la rejilla pendiente y parte de la copia MÁS
   * RECIENTE del mapa, para no pisar con datos viejos lo que acaba de guardarse.
   */
  async #editarMapa(id: number, cambios: Partial<Mapa>) {
    await this.guardarRejilla();
    const fresco = this.mapas.find((m) => m.id === id);
    if (!fresco) return;
    await invoke("modificar_mapa", { mapa: { ...fresco, ...cambios } });
    await this.recargarMapas();
  }

  // ------------------------------------------------------------------
  // Rejilla del mapa abierto (la barra de herramientas llama a estos dos)
  // ------------------------------------------------------------------

  marcarRejillaModificada() {
    if (!this.mapaActual) return;
    this.#pendiente = true;
    if (this.#temporizador) clearTimeout(this.#temporizador);
    this.#temporizador = setTimeout(() => void this.guardarRejilla(), ESPERA_GUARDADO_MS);
  }

  /** Guarda ya los cambios de rejilla pendientes (si los hay). */
  async guardarRejilla() {
    const base = this.mapaActual;
    if (!this.#pendiente || !base?.id) return;
    this.#descartarRejillaPendiente();

    const { tamanoMin, tamanoMax, grosorMin, grosorMax } = LIMITES_REJILLA;
    const tamano = Math.round(Number(this.rejilla.tamano));
    const grosor = Math.round(Number(this.rejilla.grosor));

    // Un campo vacío o a medias no debe llegar a Rust como 0: se conserva el valor anterior.
    const mapaModificado: Mapa = {
      ...base,
      rejilla: tamano >= tamanoMin && tamano <= tamanoMax ? tamano : base.rejilla,
      grosor: grosor >= grosorMin && grosor <= grosorMax ? grosor : base.grosor,
      color_opacidad: hexARgba(this.rejilla.color, this.rejilla.opacidad)
    };

    try {
      await invoke("modificar_mapa", { mapa: mapaModificado });
    } catch (err) {
      console.error("Error al persistir cambios de rejilla en Rust:", err);
      return;
    }

    if (this.mapaActual?.id === mapaModificado.id) this.mapaActual = mapaModificado;
    const i = this.mapas.findIndex((m) => m.id === mapaModificado.id);
    if (i !== -1) this.mapas[i] = mapaModificado;
  }

  // ------------------------------------------------------------------
  // Internos
  // ------------------------------------------------------------------

  #abrirMapa(mapa: Mapa | null) {
    this.mapaActual = mapa;
    // Los jugadores ven el mapa que el host tiene abierto (si lo comparte): se le dice a Rust. Un fallo no importa.
    if (!esJugador) void Promise.resolve(invoke("abrir_mapa", { id: mapa?.id ?? null })).catch(() => {});
    if (!mapa) return;

    this.rejilla.tamano = mapa.rejilla;
    this.rejilla.grosor = mapa.grosor;
    if (Array.isArray(mapa.color_opacidad) && mapa.color_opacidad.length >= 4) {
      const [r, g, b, a] = mapa.color_opacidad;
      this.rejilla.color = rgbAHex(r, g, b);
      this.rejilla.opacidad = Number(a) / 255;
    }
  }

  #descartarRejillaPendiente() {
    if (this.#temporizador) clearTimeout(this.#temporizador);
    this.#temporizador = null;
    this.#pendiente = false;
  }

  /** Quita el foco de un input en edición para que dispare su `change` antes de guardar. */
  #desenfocar() {
    if (typeof document !== "undefined" && document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }

  /** Ficha que se está editando en la ventana flotante (null = ventana cerrada) */
  entidadEnEdicion = $state<Entidad | null>(null);

  // ------------------------------------------------------------------
  // Ventana flotante y persistencia de Entidades
  // ------------------------------------------------------------------

  /** true mientras se edita una ficha NUEVA: ya tiene su id real (reservado en Rust) pero aún no está guardada */
  editorEsBorrador = $state(false);
  /** Mensaje de error de la última operación del editor (se muestra en la ventana) */
  errorEditor = $state("");

  /**
   * ¿Puede esta pantalla cambiar la ficha? El host, todas; un jugador, las suyas (su banco solo trae las que le
   * ha dado el host). El host lo vuelve a comprobar con cada acción: esto solo evita ofrecer lo que va a rechazar.
   */
  controla(instancia: Instancia): boolean {
    return !esJugador || this.banco.some((e) => e.id === instancia.base.id);
  }

  /** Sube en cada apertura del editor: la vista lo usa de `{#key}` para crear una ventana nueva aunque pase
   *  directamente de una ficha a otra. */
  sesionEditor = $state(0);

  /** Cómo guardar la ventana abierta (la registra VentanaFicha): devuelve true si se guardó y se cerró. */
  #guardarEditor: (() => Promise<boolean>) | null = null;

  /** La ventana de la ficha dice cómo guardarla. Devuelve cómo dejar de hacerlo. */
  /** La plantilla que hay que aplicar a la ficha que se va a abrir (la recoge la ventana al montarse). */
  plantillaAlAbrir: PlantillaLeida | null = null;
  #importador: ((p: PlantillaLeida) => void) | null = null;

  /** La ventana de la ficha dice cómo recibir una plantilla (para cuando ya está abierta la ficha a la que va). */
  registrarImportador(importar: (p: PlantillaLeida) => void) {
    this.#importador = importar;
    return () => {
      if (this.#importador === importar) this.#importador = null;
    };
  }

  /** Abre la ficha (`entidad`) o una nueva (null) con los datos de una plantilla (sin guardar: se guarda con «Guardar»). */
  async abrirConPlantilla(plantilla: PlantillaLeida, entidad: Entidad | null) {
    const abierta = this.entidadEnEdicion;
    if (entidad && abierta && !this.editorEsBorrador && abierta.id === entidad.id) {
      this.#importador?.(plantilla);
      return;
    }
    this.plantillaAlAbrir = plantilla;
    if (!(await this.abrirEditorEntidad(entidad))) this.plantillaAlAbrir = null;
  }

  registrarEditor(guardar: () => Promise<boolean>) {
    this.#guardarEditor = guardar;
    return () => {
      if (this.#guardarEditor === guardar) this.#guardarEditor = null;
    };
  }

  /**
   * Abre la ventana de una ficha. Con `entidad` edita una existente (sobre una copia);
   * sin ella crea un borrador: Rust reserva el id real en ese instante, así los archivos
   * que se adjunten ya viven en su carpeta definitiva (archivos/entidad_<id>).
   *
   * Si ya hay otra ficha abierta, primero se GUARDA (como al pulsar Guardar) y la ventana pasa a la nueva.
   * Si no se puede guardar (falta el nombre, Rust la rechaza...) la abierta se queda con el motivo a la vista
   * y no se abre nada. Pedir la que ya está abierta no hace nada. Devuelve true si se abrió la pedida.
   */
  async abrirEditorEntidad(entidad?: Entidad | null, fotoUrl: string | null = null): Promise<boolean> {
    if (this.#abriendo) return false;
    const abierta = this.entidadEnEdicion;
    if (abierta && entidad && !this.editorEsBorrador && abierta.id === entidad.id) return false;
    this.#abriendo = true;
    try {
      if (abierta && !(await this.#guardarEditor?.())) return false;
      this.errorEditor = "";
      if (entidad) {
        // Clonamos para no mutar el estado en vivo hasta pulsar Guardar. Se parte de la del banco por si
        // es la ficha que se acaba de guardar
        const actual = this.banco.find((e) => e.id === entidad.id) ?? entidad;
        this.entidadEnEdicion = structuredClone($state.snapshot(actual));
        this.editorEsBorrador = false;
      } else {
        const zonaDefecto = this.zonaId ?? this.zonasOrdenadas[0]?.id ?? 1;
        const borrador = await invoke<Entidad>("iniciar_borrador_entidad", {
          zonaId: zonaDefecto,
          fotoUrl
        });
        this.entidadEnEdicion = borrador;
        this.editorEsBorrador = true;
      }
      this.sesionEditor++;
      return true;
    } catch (err) {
      console.error("No se pudo abrir la ficha:", err);
      this.avisar(`No se pudo abrir la ficha: ${err instanceof Error ? err.message : String(err)}`);
      return false;
    } finally {
      this.#abriendo = false;
    }
  }
  #abriendo = false;

  /** Cierra la ventana sin guardar. `rutas` = archivos/imágenes que se crearon en esta sesión. */
  async cancelarEditorEntidad(rutas: string[] = []) {
    const e = this.entidadEnEdicion;
    if (!e) return;
    const eraBorrador = this.editorEsBorrador;
    this.entidadEnEdicion = null;
    this.editorEsBorrador = false;
    this.errorEditor = "";
    try {
      if (eraBorrador) {
        await invoke("cancelar_borrador_entidad", { id: e.id, rutas });
      } else if (rutas.length > 0) {
        await invoke("descartar_archivos_borrador", { rutas });
      }
    } catch (err) {
      console.error("Error al descartar el borrador:", err);
    }
  }

  /** Cierra la ventana tras guardar (no descarta nada) */
  cerrarEditorEntidad() {
    this.entidadEnEdicion = null;
    this.editorEsBorrador = false;
    this.errorEditor = "";
  }

  /**
   * La ficha `id` tal como está ahora fuera de la ventana (lo último leído de Rust): la del banco y, si es única y tiene instancia, con
   * los valores actuales de esa instancia (los que cambian la rueda, − / + y las tiradas arrastradas; Rust los mantiene iguales).
   */
  fichaActual(id: number): Entidad | null {
    const ficha = this.banco.find((e) => e.id === id);
    if (!ficha) return null;
    const instancia = ficha.es_unico ? this.instancias.find((i) => i.base.id === id) : undefined;
    if (!instancia) return ficha;
    return { ...ficha, actuales: instancia.valores_actuales.map((v) => ({ id_definicion: v.id_definicion, valor: v.valor })) };
  }

  /** Guarda la ficha. Devuelve true si Rust la aceptó; si no, deja el motivo en `errorEditor`. */
  async guardarEntidad(entidad: Entidad): Promise<boolean> {
    this.errorEditor = "";
    try {
      const plano = $state.snapshot(entidad) as Entidad;
      if (this.editorEsBorrador) {
        await invoke<Entidad>("crear_entidad", { entidad: plano });
      } else {
        await invoke("modificar_entidad", { entidad: plano });
      }
      await this.recargarEntidades();
      this.cerrarEditorEntidad();
      return true;
    } catch (err) {
      this.errorEditor = String(err);
      return false;
    }
  }

  async borrarEntidad(entidad: Entidad) {
    try {
      await invoke("borrar_entidad", { entidad: $state.snapshot(entidad) });
      await this.recargarEntidades();
      if (this.entidadEnEdicion?.id === entidad.id) {
        this.cerrarEditorEntidad();
      }
    } catch (err) {
      this.errorEditor = String(err);
    }
  }

}

export const partida = new Partida();

// Las variables de una tirada (`2d6 + f`) salen de la ficha con el foco o, sin él, de la última que lo tuvo
tiradas.fichaConFoco = () => partida.fichaParaVariables;
// `Batalla` y `Terminar` cambian mapas e instancias (el orden de «Fichas activas»): se vuelve a leer lo que cambió
tiradas.alCambiarBatalla = async () => {
  await partida.recargarEntidades();
  await partida.recargarBatallas();
};
