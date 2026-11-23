// src/lib/arrastreSO.ts
//
// Resaltado de las zonas donde se puede SOLTAR un archivo que se arrastra desde el sistema operativo (el explorador de archivos). En cuanto el
// archivo entra en el programa se marcan en azul las zonas donde ese tipo de archivo vale, y la que está bajo el cursor se ve un poco más clara.
// El azul es un VELO aparte (un div fijo con pointer-events: none, por encima de todo) y no un estilo de la propia zona: así lo tapan ni las
// imágenes ni los botones de dentro, y no cambia nada del diseño. Esto solo PINTA: quien suelta es cada componente, como siempre (alSoltarArchivos en la app del host,
// ondrop en el navegador de un jugador), y sus criterios son los mismos que los predicados de aquí.
//
//  * App del host (Tauri): onDragDropEvent da las RUTAS ya al entrar (con su extensión) y la posición.
//  * Navegador de un jugador: dragenter/dragover solo dan el MIME de cada archivo (nunca el nombre). Un MIME vacío o desconocido cuenta como
//    «puede valer»: no se esconde una zona válida por no saber el tipo.
//
// La clasificación y las zonas son funciones puras con pruebas (tests/arrastreSO.test.ts); lo que toca el DOM es una capa fina al final.
import { EXTENSIONES_IMAGEN, type UsoImagen } from "$lib/imagenes";
import { esJugador } from "$lib/modo";

// ------------------------------------------------------------------
// Qué archivo se arrastra
// ------------------------------------------------------------------

/** Lo que se sabe de un archivo mientras se arrastra: su extensión (app del host) o su MIME (navegador); a veces ninguna de las dos. */
export interface ArchivoArrastrado {
  ext: string | null;
  mime: string | null;
}

/** De una ruta del disco (app del host). Una carpeta o un nombre sin extensión no tiene extensión. */
export function deRuta(ruta: string): ArchivoArrastrado {
  const ext = /\.([^./\\]+)$/.exec(ruta.trim())?.[1].toLowerCase() ?? null;
  return { ext, mime: null };
}

/** De un MIME (navegador). Vacío = no se sabe. */
export function deMime(mime: string): ArchivoArrastrado {
  const limpio = mime.trim().toLowerCase();
  return { ext: null, mime: limpio || null };
}

/** ¿No hay forma de saber qué es? (sin extensión ni MIME) */
export const desconocido = (a: ArchivoArrastrado) => !a.ext && !a.mime;

const MIME_IMAGEN_NO_MEDIBLE = ["image/svg+xml", "image/avif"];

/** ¿Es una imagen que vale para ese uso? Un mapa no admite SVG ni AVIF (su tamaño define las casillas). */
export function esImagenArrastrada(a: ArchivoArrastrado, uso: UsoImagen): boolean {
  if (a.ext) return EXTENSIONES_IMAGEN[uso].includes(a.ext);
  if (a.mime?.startsWith("image/")) return uso === "ficha" || !MIME_IMAGEN_NO_MEDIBLE.includes(a.mime);
  return false;
}

/** ¿Es un .html (una posible plantilla de ficha)? No se sabe si lo es de verdad hasta soltarlo. */
export function esPlantillaArrastrada(a: ArchivoArrastrado): boolean {
  return a.ext ? a.ext === "html" || a.ext === "htm" : a.mime === "text/html";
}

/** ¿Es un archivo de texto (.txt)? */
export function esTextoArrastrado(a: ArchivoArrastrado): boolean {
  return a.ext ? a.ext === "txt" : a.mime === "text/plain";
}

/** Cualquier archivo con nombre (no una carpeta): lo que admite adjuntar el host. Una carpeta llega sin extensión ni MIME. */
export const esCualquierArchivo = (a: ArchivoArrastrado) => !!a.ext || !!a.mime;

// ------------------------------------------------------------------
// Zonas
// ------------------------------------------------------------------

/** Una zona donde se puede soltar: acepta mira todos los archivos que se arrastran (vale si al menos uno le sirve). */
export interface ZonaSoltar<N = Element> {
  nodo: N;
  acepta: (archivos: ArchivoArrastrado[]) => boolean;
}

/** ¿Es válida esta zona para lo que se arrastra? Si algún archivo es desconocido, puede valer: se marca. */
export function zonaValida<N>(zona: ZonaSoltar<N>, archivos: ArchivoArrastrado[]): boolean {
  return archivos.length > 0 && (archivos.some(desconocido) || zona.acepta(archivos));
}

/**
 * La zona válida más INTERNA bajo el cursor: de las que contienen el nodo (contiene(zona, nodo)), la que está contenida en todas las demás.
 * null si no hay ninguna.
 */
export function zonaEncima<N>(
  zonas: ZonaSoltar<N>[],
  archivos: ArchivoArrastrado[],
  nodo: N | null,
  contiene: (zona: N, nodo: N) => boolean
): ZonaSoltar<N> | null {
  if (!nodo) return null;
  const candidatas = zonas.filter((z) => zonaValida(z, archivos) && contiene(z.nodo, nodo));
  // La más interna es la que no contiene a ninguna otra candidata
  return candidatas.find((z) => !candidatas.some((o) => o !== z && o.nodo !== z.nodo && contiene(z.nodo, o.nodo))) ?? null;
}

// ------------------------------------------------------------------
// DOM (capa fina: sin lógica propia)
// ------------------------------------------------------------------

const zonas = new Set<ZonaSoltar>();
let archivos: ArchivoArrastrado[] = [];
let activo = false;
let encima: ZonaSoltar | null = null;

const contiene = (zona: Element, nodo: Element) => zona.contains(nodo);

/** Lado mínimo (px) de una zona para marcarla: una más pequeña no se ve ni se puede acertar con el ratón. */
const ZONA_MINIMA_PX = 24;

/** Un velo por cada zona válida, colocado sobre su rectángulo. */
const velos = new Map<ZonaSoltar, HTMLDivElement>();

function quitarVelo(z: ZonaSoltar) {
  velos.get(z)?.remove();
  velos.delete(z);
}

/** Pone o quita el velo de cada zona según lo que se arrastra y dónde está el cursor. */
function pintar(bajoElCursor: Element | null) {
  encima = activo ? zonaEncima([...zonas], archivos, bajoElCursor, contiene) : null;
  for (const z of zonas) {
    if (!activo || !zonaValida(z, archivos)) {
      quitarVelo(z);
      continue;
    }
    // Una zona colapsada (sin alto o ancho: un hueco que se ve como una línea) no se puede usar para soltar, así que no se marca
    const r = z.nodo.getBoundingClientRect();
    if (r.width < ZONA_MINIMA_PX || r.height < ZONA_MINIMA_PX) {
      quitarVelo(z);
      continue;
    }
    let velo = velos.get(z);
    if (!velo) {
      velo = document.createElement("div");
      velo.className = "so-velo";
      velo.setAttribute("aria-hidden", "true");
      document.body.appendChild(velo);
      velos.set(z, velo);
    }
    velo.style.left = r.left + "px";
    velo.style.top = r.top + "px";
    velo.style.width = r.width + "px";
    velo.style.height = r.height + "px";
    velo.style.borderRadius = getComputedStyle(z.nodo).borderRadius;
    velo.classList.toggle("so-velo-encima", z === encima);
  }
}

function empezar(nuevos: ArchivoArrastrado[], bajoElCursor: Element | null) {
  archivos = nuevos;
  activo = nuevos.length > 0;
  pintar(bajoElCursor);
}

function terminar() {
  activo = false;
  archivos = [];
  pintar(null);
}

/**
 * Acción de Svelte: marca un elemento como zona donde se puede soltar un archivo del sistema. acepta es el MISMO criterio que usa su drop:
 * use:zonaSoltar={{ acepta: (a) => a.some(esPlantillaArrastrada) }}
 */
export function zonaSoltar(nodo: Element, opciones: { acepta: ZonaSoltar["acepta"] }) {
  const zona: ZonaSoltar = { nodo, acepta: opciones.acepta };
  zonas.add(zona);
  if (activo) pintar(null);
  return {
    update(nuevas: { acepta: ZonaSoltar["acepta"] }) {
      zona.acepta = nuevas.acepta;
    },
    destroy() {
      zonas.delete(zona);
      quitarVelo(zona);
    }
  };
}

/**
 * Empieza a escuchar el arrastre de archivos del sistema (llamar UNA vez al arrancar; devuelve cómo dejar de escuchar). En la app del host
 * lo avisa Tauri, con las rutas; en el navegador de un jugador, los eventos HTML5, solo con el MIME.
 */
export function iniciarArrastreSO(): () => void {
  return esJugador ? escucharNavegador() : escucharTauri();
}

function escucharTauri(): () => void {
  let quitar: (() => void) | null = null;
  let terminado = false;
  const bajo = (p: { x: number; y: number }) => {
    // Tauri da la posición en píxeles físicos; elementFromPoint los quiere en CSS (igual que alSoltarArchivos)
    const escala = window.devicePixelRatio || 1;
    return document.elementFromPoint(p.x / escala, p.y / escala);
  };
  void import("@tauri-apps/api/webview")
    .then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter") empezar((p.paths ?? []).map(deRuta), bajo(p.position));
        else if (p.type === "over") pintar(bajo(p.position));
        else terminar(); // drop o leave
      })
    )
    .then((fn) => (terminado ? fn() : (quitar = fn)))
    .catch((err) => console.warn("El resaltado al arrastrar archivos no está disponible:", err));
  return () => {
    terminado = true;
    quitar?.();
    terminar();
  };
}

function escucharNavegador(): () => void {
  // dragenter y dragleave se repiten con cada elemento hijo: se cuentan para saber cuándo sale de verdad de la página
  let dentro = 0;
  const esDeArchivos = (e: DragEvent) => !!e.dataTransfer?.types?.includes("Files");
  const tipos = (e: DragEvent) => [...(e.dataTransfer?.items ?? [])].filter((i) => i.kind === "file").map((i) => deMime(i.type));
  const bajo = (e: DragEvent) => document.elementFromPoint(e.clientX, e.clientY);

  const alEntrar = (e: DragEvent) => {
    if (!esDeArchivos(e)) return;
    dentro++;
    empezar(tipos(e), bajo(e));
  };
  const alPasar = (e: DragEvent) => {
    if (!esDeArchivos(e)) return;
    if (!activo) empezar(tipos(e), bajo(e));
    else pintar(bajo(e));
    // Soltar un archivo donde nada lo recoge haría que el navegador lo abra y deje la partida: se cancela salvo en una zona que lo acepta
    if (!encima) e.preventDefault();
  };
  const alSalir = (e: DragEvent) => {
    if (!esDeArchivos(e)) return;
    dentro = Math.max(0, dentro - 1);
    if (dentro === 0) terminar();
  };
  const alSoltar = (e: DragEvent) => {
    if (esDeArchivos(e) && !encima) e.preventDefault();
    dentro = 0;
    terminar();
  };
  window.addEventListener("dragenter", alEntrar);
  window.addEventListener("dragover", alPasar);
  window.addEventListener("dragleave", alSalir);
  window.addEventListener("drop", alSoltar);
  return () => {
    window.removeEventListener("dragenter", alEntrar);
    window.removeEventListener("dragover", alPasar);
    window.removeEventListener("dragleave", alSalir);
    window.removeEventListener("drop", alSoltar);
    terminar();
  };
}
