// Apuntar el AREA de una macro con el ratón (apuntarArea.svelte.ts), su geometría (extremosArea) y el flujo de ejecutar una macro.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { extremosArea } from "$lib/tablero";
import { confirmarCentro, type AreaResuelta } from "$lib/apuntarArea.svelte";

describe("geometría de un AREA", () => {
  it("las líneas de la rejilla alrededor de la casilla, con cada lado distinto (como tablero::extremos_area)", () => {
    // Centro (10,10): 1 arriba, 3 a la derecha, 1 abajo, 3 a la izquierda
    expect(extremosArea({ x: 10, y: 10 }, [1, 3, 1, 3])).toEqual({ izquierda: 7, derecha: 14, arriba: 9, abajo: 12, cx: 10.5, cy: 10.5 });
    // Sin lados: la casilla sola
    expect(extremosArea({ x: 2, y: 3 }, [0, 0, 0, 0])).toEqual({ izquierda: 2, derecha: 3, arriba: 3, abajo: 4, cx: 2.5, cy: 3.5 });
  });
});

const rombo: AreaResuelta = { forma: "Rombo", lados: [1, 1, 1, 1] };
const circulo: AreaResuelta = { forma: "Circulo", lados: [0, 2, 0, 2] };

describe("apuntar las áreas una detrás de otra", () => {
  it("cada clic fija el centro de la siguiente; al fijar la última termina", () => {
    const a = { areas: [rombo, circulo], centros: [], idMapa: 1 };
    const uno = confirmarCentro(a, { x: 3, y: 4 });
    expect(uno).toEqual({ centros: [{ x: 3, y: 4 }], terminado: false });
    expect(confirmarCentro({ ...a, centros: uno.centros }, { x: 5, y: 5 }).terminado).toBe(true);
  });

  it("el store devuelve los centros al terminar, null al cancelar, y una petición nueva cancela la anterior", async () => {
    vi.resetModules();
    const { apuntarArea } = await import("$lib/apuntarArea.svelte");
    const p = apuntarArea.pedir([rombo, circulo], 1);
    expect(apuntarArea.areaActual).toEqual(rombo);
    apuntarArea.confirmar({ x: 1, y: 1 });
    expect(apuntarArea.areaActual).toEqual(circulo);
    apuntarArea.confirmar({ x: 2, y: 2 });
    expect(await p).toEqual([{ x: 1, y: 1 }, { x: 2, y: 2 }]);
    expect(apuntarArea.actual).toBeNull();

    const cancelada = apuntarArea.pedir([rombo], 1);
    apuntarArea.cancelar();
    expect(await cancelada).toBeNull();

    const vieja = apuntarArea.pedir([rombo], 1);
    const nueva = apuntarArea.pedir([circulo], 1);
    expect(await vieja).toBeNull();
    apuntarArea.confirmar({ x: 9, y: 9 });
    expect(await nueva).toEqual([{ x: 9, y: 9 }]);
    expect(await apuntarArea.pedir([], 1)).toEqual([]);
  });
});

describe("ejecutar una macro", () => {
  let llamadas: { cmd: string; args: Record<string, unknown> }[] = [];
  let areas: AreaResuelta[] = [];

  beforeEach(() => {
    llamadas = [];
    areas = [];
    vi.resetModules();
    vi.doMock("@tauri-apps/api/core", () => ({
      convertFileSrc: (s: string) => s,
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        llamadas.push({ cmd, args });
        if (cmd === "preparar_macro") return areas;
        if (cmd === "obtener_dir_datos") return "";
        return [];
      }
    }));
    vi.doMock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
  });

  const cargar = async () => {
    const { macros } = await import("$lib/macros.svelte");
    const { partida } = await import("$lib/partida.svelte");
    const { apuntarArea } = await import("$lib/apuntarArea.svelte");
    // Lo que la partida carga sola al importarse (vacío con el backend falso) termina antes de que el test ponga sus datos
    await new Promise((r) => setTimeout(r, 20));
    return { macros, partida, apuntarArea };
  };
  const ejecutadas = () => llamadas.filter((l) => l.cmd === "ejecutar_macro");

  it("sin AREA se ejecuta directamente", async () => {
    const { macros } = await cargar();
    await macros.ejecutar(1, 7);
    expect(ejecutadas()).toEqual([{ cmd: "ejecutar_macro", args: { idAsociacion: 1, idInstancia: 7, centros: [] } }]);
  });

  it("con AREA espera a que se apunte; si se cancela no se ejecuta nada", async () => {
    areas = [rombo];
    const { macros, partida, apuntarArea } = await cargar();
    partida.instancias = [{ id_instancia: 7, id_mapa: 3 } as never];
    partida.mapaActual = { id: 3 } as never;

    const ejecutando = macros.ejecutar(1, 7);
    await vi.waitFor(() => expect(apuntarArea.actual).not.toBeNull());
    apuntarArea.confirmar({ x: 4, y: 2 });
    await ejecutando;
    expect(ejecutadas()[0].args).toEqual({ idAsociacion: 1, idInstancia: 7, centros: [{ x: 4, y: 2 }] });

    // Al acabar, la partida se vuelve a leer del backend falso (vacío): se vuelven a poner la ficha y el mapa
    partida.instancias = [{ id_instancia: 7, id_mapa: 3 } as never];
    partida.mapaActual = { id: 3 } as never;
    const cancelada = macros.ejecutar(1, 7);
    await vi.waitFor(() => expect(apuntarArea.actual).not.toBeNull());
    apuntarArea.cancelar();
    await cancelada;
    expect(ejecutadas()).toHaveLength(1);
  });

  it("con AREA y la ficha en otro mapa, avisa y no apunta", async () => {
    areas = [rombo];
    const { macros, partida, apuntarArea } = await cargar();
    partida.instancias = [{ id_instancia: 7, id_mapa: 3 } as never];
    partida.mapaActual = { id: 9 } as never;
    await macros.ejecutar(1, 7);
    expect(apuntarArea.actual).toBeNull();
    expect(ejecutadas()).toHaveLength(0);
    expect(partida.avisos).toContain("mapa");
  });
});
