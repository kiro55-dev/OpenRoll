// src/lib/batalla.ts
//
// El modo batalla de un mapa: grupos con su turno y su contador de vueltas. Espejo de src-tauri/src/tablero_general/batalla.rs (Rust
// es quien decide y guarda; esto solo lee lo que llega en `Mapa.batalla` y calcula lo mismo para pintarlo).
import type { Batalla, GrupoBatalla, Instancia } from "./partida.svelte";

/** ¿Es una orden de batalla escrita en el chat? (`Batalla [tirada]` o `Terminar`; la primera palabra entera, sin mayúsculas). Igual que
 *  `batalla::interpretar_comando` de Rust. */
export function esComandoBatalla(comando: string): boolean {
  return /^(batalla|terminar)(\s|$)/i.test(comando.trim());
}

/** Los miembros de un grupo que existen, en el orden de «Fichas activas» (el de los turnos). */
export function ordenDeTurno(instancias: readonly Instancia[], grupo: GrupoBatalla): number[] {
  return instancias
    .filter((i) => grupo.miembros.includes(i.id_instancia))
    .sort((a, b) => a.orden - b.orden || a.id_instancia - b.id_instancia)
    .map((i) => i.id_instancia);
}

/** A quién le toca después y en qué vuelta: el siguiente de la lista y, al volver al primero, la vuelta sube. `null` sin miembros. */
export function siguienteTurno(instancias: readonly Instancia[], grupo: GrupoBatalla): { turno: number; ronda: number } | null {
  const orden = ordenDeTurno(instancias, grupo);
  if (orden.length === 0) return null;
  const pos = grupo.turno === null || grupo.turno === undefined ? -1 : orden.indexOf(grupo.turno);
  if (pos < 0) return { turno: orden[0], ronda: Math.max(1, grupo.ronda) };
  return pos + 1 < orden.length ? { turno: orden[pos + 1], ronda: grupo.ronda } : { turno: orden[0], ronda: grupo.ronda + 1 };
}

/** El grupo en el que participa una instancia (`null` = no participa). */
export function grupoDe(batalla: Batalla | null | undefined, idInstancia: number): GrupoBatalla | null {
  return batalla?.grupos.find((g) => g.miembros.includes(idInstancia)) ?? null;
}

/** ¿Tiene esta instancia el turno de algún grupo? */
export function tieneElTurno(batalla: Batalla | null | undefined, idInstancia: number): boolean {
  return !!batalla?.grupos.some((g) => g.turno === idInstancia);
}

/**
 * Qué grupo avanza con Enter. Un jugador, el grupo cuyo turno tiene una ficha suya; el host, el del instancia con el foco o, sin
 * foco, el primero. `null` si no hay ninguno que se pueda pasar.
 */
export function grupoParaEnter(p: {
  batalla: Batalla | null | undefined;
  esJugador: boolean;
  /** ¿Controla quien pulsa esta instancia? (el host, todas; un jugador, las suyas) */
  controla: (idInstancia: number) => boolean;
  /** La instancia con el foco, si hay exactamente una. */
  enFoco: number | null;
}): GrupoBatalla | null {
  const { batalla } = p;
  if (!batalla) return null;
  const pasable = (g: GrupoBatalla) => g.turno !== null && g.turno !== undefined && p.controla(g.turno);
  if (p.esJugador) return batalla.grupos.find(pasable) ?? null;
  const delFoco = p.enFoco === null ? null : grupoDe(batalla, p.enFoco);
  return (delFoco && pasable(delFoco) ? delFoco : batalla.grupos.find(pasable)) ?? null;
}

/** Una caja de «Fichas activas» en una batalla: un grupo con sus fichas, o (`grupo` null) las que no participan. */
export interface SeccionBatalla {
  grupo: GrupoBatalla | null;
  fichas: Instancia[];
}

/** Reparte `instancias` (ya en su orden) en las cajas de la batalla: cada grupo con sus miembros y, al final, las que no participan. */
export function seccionesDeBatalla(batalla: Batalla, instancias: readonly Instancia[]): SeccionBatalla[] {
  const secciones: SeccionBatalla[] = batalla.grupos.map((g) => ({ grupo: g, fichas: instancias.filter((i) => g.miembros.includes(i.id_instancia)) }));
  secciones.push({ grupo: null, fichas: instancias.filter((i) => !grupoDe(batalla, i.id_instancia)) });
  return secciones;
}

/** Las mismas instancias en el orden en que se ven con las cajas (grupo a grupo y al final las que no participan). Es el orden de
 *  la lista que se reordena arrastrando: sus posiciones tienen que coincidir con lo que se ve. */
export function ordenarPorGrupos(instancias: readonly Instancia[], batalla: Batalla): Instancia[] {
  return seccionesDeBatalla(batalla, instancias).flatMap((s) => s.fichas);
}

/** ¿Se dibujan cajas? Con varios grupos, o si alguna ficha a la vista no participa; con un solo grupo que las tiene a todas, la lista normal. */
export function usaCuadros(batalla: Batalla | null | undefined, instancias: readonly Instancia[]): boolean {
  return !!batalla && (batalla.grupos.length > 1 || instancias.some((i) => !grupoDe(batalla, i.id_instancia)));
}
