// src/lib/arrastreGrupoBatalla.svelte.ts
//
// Arrastrar un grupo entero de la batalla por su cabecera y soltarlo sobre otro: sus fichas pasan a él (que conserva su turno y su
// contador) y el grupo desaparece. Con eventos de puntero, como el resto de arrastres. Solo el host.
import { partida } from "$lib/partida.svelte";
import { esJugador } from "$lib/modo";

/** Píxeles que hay que mover el ratón para que una pulsación sea un arrastre y no un clic. */
const UMBRAL_PX = 5;

export interface ArrastreDeGrupo {
  id: number;
  nombre: string;
  x: number;
  y: number;
  /** El grupo sobre el que está ahora (nunca el mismo ni «No participa»), o null. */
  sobre: number | null;
}

/** Id de la caja (grupo) bajo un punto, o null. 0 es «No participa». */
export function cajaEn(x: number, y: number): number | null {
  const caja = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-grupo-batalla]");
  return caja?.dataset.grupoBatalla === undefined ? null : Number(caja.dataset.grupoBatalla);
}

class ArrastreGrupoBatalla {
  actual = $state<ArrastreDeGrupo | null>(null);

  /** En el `pointerdown` de la cabecera de un grupo. */
  preparar(e: PointerEvent, grupo: { id: number; nombre: string }) {
    if (esJugador || e.button !== 0 || this.actual) return;
    if ((e.target as HTMLElement).closest("button, input")) return;
    const inicio = { x: e.clientX, y: e.clientY };
    let empezado = false;

    const mover = (ev: PointerEvent) => {
      if (!empezado) {
        if (Math.hypot(ev.clientX - inicio.x, ev.clientY - inicio.y) <= UMBRAL_PX) return;
        empezado = true;
      }
      const caja = cajaEn(ev.clientX, ev.clientY);
      this.actual = { id: grupo.id, nombre: grupo.nombre, x: ev.clientX, y: ev.clientY, sobre: caja !== null && caja !== 0 && caja !== grupo.id ? caja : null };
    };
    const quitar = () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", cancelar);
      window.removeEventListener("keydown", cancelar);
    };
    const soltar = () => {
      quitar();
      const sobre = this.actual?.sobre ?? null;
      this.actual = null;
      if (empezado && sobre !== null) void partida.fusionarGruposBatalla(grupo.id, sobre);
    };
    const cancelar = (ev: Event) => {
      if (ev instanceof KeyboardEvent && ev.key !== "Escape") return;
      quitar();
      this.actual = null;
    };
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", cancelar);
    window.addEventListener("keydown", cancelar);
  }
}

export const arrastreGrupoBatalla = new ArrastreGrupoBatalla();
