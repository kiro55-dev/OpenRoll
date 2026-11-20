// El modo batalla en el front (src/lib/batalla.ts): lo mismo que calcula batalla.rs, para pintar el turno y decidir el Enter.
import { describe, expect, it } from "vitest";
import type { Batalla, GrupoBatalla, Instancia } from "$lib/partida.svelte";
import { esComandoBatalla, grupoDe, grupoParaEnter, ordenDeTurno, siguienteTurno, tieneElTurno } from "$lib/batalla";

const inst = (id: number, orden: number) => ({ id_instancia: id, orden }) as unknown as Instancia;
const grupo = (id: number, miembros: number[], turno: number | null, ronda = 1): GrupoBatalla => ({ id, nombre: `G${id}`, miembros, turno, ronda });
const instancias = [inst(1, 0), inst(2, 1), inst(3, 2), inst(4, 3)];

describe("las órdenes de batalla del chat", () => {
  it("Empezar y Terminar, con o sin texto detrás y sin mirar mayúsculas", () => {
    expect(esComandoBatalla("Empezar")).toBe(true);
    expect(esComandoBatalla("  empezar 1d20 + des")).toBe(true);
    expect(esComandoBatalla("TERMINAR")).toBe(true);
    expect(esComandoBatalla("empezarya")).toBe(false);
    expect(esComandoBatalla("1d20")).toBe(false);
    expect(esComandoBatalla("")).toBe(false);
  });
});

describe("los turnos (igual que batalla.rs)", () => {
  it("el orden de turno es el de «Fichas activas», solo con los miembros", () => {
    expect(ordenDeTurno(instancias, grupo(1, [3, 1, 4], 1))).toEqual([1, 3, 4]);
    // El host reordena a mano
    const reordenadas = [inst(3, 0), inst(1, 1), inst(2, 2), inst(4, 3)];
    expect(ordenDeTurno(reordenadas, grupo(1, [3, 1, 4], 1))).toEqual([3, 1, 4]);
  });

  it("el turno pasa al siguiente y, al volver al primero, sube la vuelta", () => {
    expect(siguienteTurno(instancias, grupo(1, [1, 2, 3], 1))).toEqual({ turno: 2, ronda: 1 });
    expect(siguienteTurno(instancias, grupo(1, [1, 2, 3], 3))).toEqual({ turno: 1, ronda: 2 });
    expect(siguienteTurno(instancias, grupo(1, [2], 2, 4))).toEqual({ turno: 2, ronda: 5 }); // uno solo: cada paso es una vuelta
    expect(siguienteTurno(instancias, grupo(1, [3, 1], null))).toEqual({ turno: 1, ronda: 1 }); // sin turno: el primero
    expect(siguienteTurno(instancias, grupo(1, [], null))).toBeNull();
  });

  it("quién tiene el turno y en qué grupo participa cada una", () => {
    const b: Batalla = { grupos: [grupo(1, [1, 2], 2), grupo(2, [3], 3)], siguiente_id: 3 };
    expect(tieneElTurno(b, 2)).toBe(true);
    expect(tieneElTurno(b, 1)).toBe(false);
    expect(tieneElTurno(null, 1)).toBe(false);
    expect(grupoDe(b, 3)?.id).toBe(2);
    expect(grupoDe(b, 4)).toBeNull(); // no participa
  });
});

describe("qué grupo avanza con Enter", () => {
  const b: Batalla = { grupos: [grupo(1, [1, 2], 1), grupo(2, [3, 4], 4)], siguiente_id: 3 };
  const todas = () => true;

  it("el host: el grupo de la ficha con el foco; sin foco, el primero", () => {
    expect(grupoParaEnter({ batalla: b, esJugador: false, controla: todas, enFoco: 3 })?.id).toBe(2);
    expect(grupoParaEnter({ batalla: b, esJugador: false, controla: todas, enFoco: null })?.id).toBe(1);
    // El foco en una que no participa: el primero
    expect(grupoParaEnter({ batalla: b, esJugador: false, controla: todas, enFoco: 9 })?.id).toBe(1);
  });

  it("un jugador: el grupo cuyo turno tiene una ficha suya", () => {
    expect(grupoParaEnter({ batalla: b, esJugador: true, controla: (id) => id === 4, enFoco: null })?.id).toBe(2);
    expect(grupoParaEnter({ batalla: b, esJugador: true, controla: (id) => id === 2, enFoco: null })).toBeNull(); // no es su turno
  });

  it("sin batalla o sin grupos pasables no hace nada", () => {
    expect(grupoParaEnter({ batalla: null, esJugador: false, controla: todas, enFoco: null })).toBeNull();
    expect(grupoParaEnter({ batalla: { grupos: [grupo(1, [], null)], siguiente_id: 2 }, esJugador: false, controla: todas, enFoco: null })).toBeNull();
  });
});
