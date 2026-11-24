// src/lib/redimensionarVentana.ts
//
// Cambiar el tamaño de una ventana flotante cogiéndola de un lateral o una esquina, como una ventana de Windows. Es una acción de
// Svelte: `use:redimensionable={{ pos }}` (el mismo `pos` que mueve la ventana por su cabecera, `{ x, y }` reactivo).
//
// Pone ocho tiradores invisibles en los bordes (los de arriba y a la izquierda también mueven la ventana, para que la esquina
// opuesta se quede quieta). El tamaño va en el `style` del elemento con propiedades sueltas: la ventana debe poner su posición con
// `style:left` / `style:top` y no con un `style="..."` entero, que borraría lo que se pone aquí.
//
// Con un alto elegido a mano, lo que no cabe se desplaza por dentro: las partes de la ventana (menos su cabecera, la primera) pasan a
// poder encogerse y desplazarse, y la más alta se queda el espacio que sobre.

export interface OpcionesRedimension {
  pos: { x: number; y: number };
  minAncho?: number;
  minAlto?: number;
}

type Borde = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

const BORDES: Borde[] = ["n", "s", "e", "w", "ne", "nw", "se", "sw"];
const GROSOR = 6;
const ESQUINA = 14;
const MARGEN = 10;

const CURSOR: Record<Borde, string> = { n: "ns-resize", s: "ns-resize", e: "ew-resize", w: "ew-resize", ne: "nesw-resize", sw: "nesw-resize", nw: "nwse-resize", se: "nwse-resize" };

/** Dónde va cada tirador (px respecto a la ventana). */
function colocar(borde: Borde): Partial<CSSStyleDeclaration> {
  const lado = (borde.includes("n") ? { top: "0" } : borde.includes("s") ? { bottom: "0" } : { top: `${ESQUINA}px`, bottom: `${ESQUINA}px` }) as Partial<CSSStyleDeclaration>;
  const lateral = (borde.includes("w") ? { left: "0" } : borde.includes("e") ? { right: "0" } : { left: `${ESQUINA}px`, right: `${ESQUINA}px` }) as Partial<CSSStyleDeclaration>;
  const esquina = borde.length === 2;
  return {
    ...lado,
    ...lateral,
    width: esquina ? `${ESQUINA}px` : borde === "n" || borde === "s" ? "" : `${GROSOR}px`,
    height: esquina ? `${ESQUINA}px` : borde === "e" || borde === "w" ? "" : `${GROSOR}px`
  };
}

/** Que lo que no cabe se desplace dentro de la ventana en vez de cortarse (solo cuando se le ha dado un alto a mano). */
function repartir(nodo: HTMLElement) {
  const partes = [...nodo.children].filter((h): h is HTMLElement => h instanceof HTMLElement && !h.dataset.redim).slice(1);
  if (!partes.length) return;
  let alta = partes[0];
  for (const p of partes) {
    p.style.minHeight = "0";
    p.style.overflowY = "auto";
    if (p.scrollHeight > alta.scrollHeight) alta = p;
  }
  alta.style.flex = "1 1 auto";
  // Las listas con tope de alto propio (`max-height`) también crecen con la ventana
  for (const el of nodo.querySelectorAll<HTMLElement>("*")) {
    if (getComputedStyle(el).maxHeight !== "none" && el.scrollHeight > 0 && el.parentElement !== nodo) el.style.maxHeight = "none";
  }
}

export function redimensionable(nodo: HTMLElement, opciones: OpcionesRedimension) {
  const minAncho = opciones.minAncho ?? 220;
  const minAlto = opciones.minAlto ?? 120;
  const tiradores: HTMLElement[] = [];
  let preparada = false;

  for (const borde of BORDES) {
    const t = document.createElement("div");
    t.dataset.redim = borde;
    t.setAttribute("aria-hidden", "true");
    Object.assign(t.style, { position: "absolute", zIndex: "20", cursor: CURSOR[borde], touchAction: "none" }, colocar(borde));
    t.addEventListener("pointerdown", (e) => empezar(e, borde, t));
    nodo.appendChild(t);
    tiradores.push(t);
  }

  function empezar(e: PointerEvent, borde: Borde, t: HTMLElement) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const caja = nodo.getBoundingClientRect();
    const inicio = { x: e.clientX, y: e.clientY, izq: caja.left, arr: caja.top, ancho: caja.width, alto: caja.height };
    t.setPointerCapture(e.pointerId);
    if (!preparada) {
      preparada = true;
      // Lo que el CSS limitaba (alto máximo, ancho máximo) deja de limitar: ahora manda lo que elija el usuario
      nodo.style.maxHeight = "none";
      nodo.style.maxWidth = "none";
      nodo.style.height = `${caja.height}px`;
      repartir(nodo);
    }

    const mover = (ev: PointerEvent) => {
      const dx = ev.clientX - inicio.x;
      const dy = ev.clientY - inicio.y;
      let { izq, arr, ancho, alto } = inicio;
      if (borde.includes("e")) ancho = Math.min(Math.max(inicio.ancho + dx, minAncho), window.innerWidth - inicio.izq - MARGEN);
      if (borde.includes("s")) alto = Math.min(Math.max(inicio.alto + dy, minAlto), window.innerHeight - inicio.arr - MARGEN);
      if (borde.includes("w")) {
        ancho = Math.min(Math.max(inicio.ancho - dx, minAncho), inicio.izq + inicio.ancho - MARGEN);
        izq = inicio.izq + inicio.ancho - ancho;
      }
      if (borde.includes("n")) {
        alto = Math.min(Math.max(inicio.alto - dy, minAlto), inicio.arr + inicio.alto - MARGEN);
        arr = inicio.arr + inicio.alto - alto;
      }
      nodo.style.width = `${ancho}px`;
      nodo.style.height = `${alto}px`;
      opciones.pos.x = izq;
      opciones.pos.y = arr;
    };
    const soltar = () => {
      t.removeEventListener("pointermove", mover);
      t.removeEventListener("pointerup", soltar);
      t.removeEventListener("pointercancel", soltar);
    };
    t.addEventListener("pointermove", mover);
    t.addEventListener("pointerup", soltar);
    t.addEventListener("pointercancel", soltar);
  }

  return {
    update(nuevas: OpcionesRedimension) {
      opciones = nuevas;
    },
    destroy() {
      tiradores.forEach((t) => t.remove());
    }
  };
}
