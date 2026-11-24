// src/lib/macros.svelte.ts
//
// Las macros de la partida (las mías y las asociadas a mis fichas) y lo que se hace con ellas. Todo lo decide Rust
// (tablero_general/macros/): aquí solo se lee, se pide y se vuelve a leer, como el resto de la app.
// Un jugador recibe sus macros con código y las demás que estén asociadas a lo suyo SIN código (solo nombre y tipo).
import { EVENTO_PARTIDA_CAMBIADA, invoke, listen } from "$lib/backend";
import type { AsociacionMacro, Instancia, Macro } from "$lib/partida.svelte";
import { partida } from "$lib/partida.svelte";
import { tiradas } from "$lib/tiradas.svelte";
import { carpetaDePartida } from "$lib/imagenes";
import { esJugador } from "$lib/modo";

/** Lo que tiene una macro recién creada (Rust no admite una vacía: tiene que empezar con ONCLICK u ONTURN y hacer algo). */
export const CODIGO_NUEVA_MACRO = "ONCLICK\n# Escribe aquí lo que hace la macro\nEXECUTE 1d20";

/** El nombre de una macro nueva: «Macro 1», «Macro 2»... el primero que no tenga ya una de las `existentes`. */
export function nombreNuevaMacro(existentes: readonly string[]): string {
  const usados = new Set(existentes.map((n) => n.trim().toLowerCase().replace(/\s+/g, "")));
  let n = 1;
  while (usados.has(`macro${n}`)) n++;
  return `Macro ${n}`;
}

/** Cómo es una macro que se puede leer (lo que devuelve `validar_macro`). */
export interface Comprobacion {
  al_pulsar: boolean;
  en_turno: boolean;
}

/** Una macro asociada a una instancia: la asociación y su macro. */
export interface MacroDeFicha {
  asociacion: AsociacionMacro;
  macro: Macro;
}

/**
 * Las macros que tiene una instancia: las asociadas a su FICHA (todas sus instancias) y las asociadas a ELLA (las que le repartió
 * un SPREAD), en ese orden y sin repetir macro. Las que son ONCLICK (aunque también sean ONTURN) van como botón; las solo de turno,
 * como etiqueta (`boton`).
 */
export function macrosDeInstancia(
  inst: Pick<Instancia, "id_instancia" | "base">,
  asociaciones: readonly AsociacionMacro[],
  macros: readonly Macro[]
): (MacroDeFicha & { boton: boolean })[] {
  const vistas = new Set<number>();
  const resultado: (MacroDeFicha & { boton: boolean })[] = [];
  const deFicha = asociaciones.filter((a) => "Ficha" in a.destino && a.destino.Ficha === inst.base.id);
  const deInstancia = asociaciones.filter((a) => "Instancia" in a.destino && a.destino.Instancia === inst.id_instancia);
  for (const a of [...deFicha, ...deInstancia]) {
    const m = macros.find((x) => x.id === a.id_macro);
    if (!m || vistas.has(m.id)) continue;
    vistas.add(m.id);
    resultado.push({ asociacion: a, macro: m, boton: m.al_pulsar });
  }
  return resultado;
}

/** Las macros que puedo editar: el host, todas; un jugador, las suyas (las ajenas le llegan sin código). */
export function macrosEditables(macros: readonly Macro[], jugador: boolean): Macro[] {
  return jugador ? macros.filter((m) => m.codigo !== "") : [...macros];
}

/** La ficha a la que se está asociando desde el editor (ver `Macros.ventana`). */
export interface FichaDeMacro {
  id: number;
  nombre: string;
}

class Macros {
  /**
   * El texto que alguien ha escrito en una macro y todavía no ha aplicado con «Guardar», por id. Se conserva aunque cierre la ventana,
   * cambie de macro o cierre la aplicación (en este equipo, por partida), así una macro larga a medias sigue ahí al volver.
   */
  borradores = $state<Record<number, string>>({});
  #claveBorradores = "";

  /** Se llama al abrir la ventana: carga los borradores de esta partida (si cambió de partida, los de la otra ya no valen). */
  cargarBorradores() {
    const clave = `openroll.macros.borradores:${esJugador ? "jugador" : carpetaDePartida()}`;
    if (clave === this.#claveBorradores) return;
    this.#claveBorradores = clave;
    try {
      this.borradores = JSON.parse(localStorage.getItem(clave) ?? "{}") ?? {};
    } catch {
      this.borradores = {};
    }
  }

  #guardarBorradores() {
    try {
      if (this.#claveBorradores) localStorage.setItem(this.#claveBorradores, JSON.stringify(this.borradores));
    } catch {
      // sin almacenamiento: los borradores viven solo mientras la aplicación está abierta
    }
  }

  /** Apunta el texto escrito; `undefined` (o igual a lo guardado) lo quita. */
  ponerBorrador(id: number, texto: string | undefined) {
    if (this.borradores[id] === texto) return;
    const nuevo = { ...this.borradores };
    if (texto === undefined) delete nuevo[id];
    else nuevo[id] = texto;
    this.borradores = nuevo;
    this.#guardarBorradores();
  }

  /**
   * La ventana de macros, si está abierta. Con `entidad` se abrió desde la caja «Parámetros» de una ficha: ahí pulsar una macro la
   * asocia a esa ficha y la que se cree nueva se le asocia sola. Con `idMacro`, abre ya esa macro (clic derecho sobre ella).
   * `version` cambia en cada apertura para que una ventana ya abierta cargue lo nuevo.
   */
  ventana = $state<{ entidad?: FichaDeMacro; idMacro?: number; version: number } | null>(null);
  #aperturas = 0;

  abrirVentana(opciones: { entidad?: FichaDeMacro; idMacro?: number } = {}) {
    this.ventana = { ...opciones, version: ++this.#aperturas };
  }

  lista = $state<Macro[]>([]);
  asociaciones = $state<AsociacionMacro[]>([]);
  /** Asociaciones que se están ejecutando ahora (su botón se ve ocupado). */
  ejecutando = $state<number[]>([]);
  #escuchando = false;

  async recargar() {
    try {
      const [lista, asociaciones] = await Promise.all([
        invoke<Macro[]>("obtener_macros"),
        invoke<AsociacionMacro[]>("obtener_asociaciones_macro")
      ]);
      this.lista = lista;
      this.asociaciones = asociaciones;
    } catch (err) {
      console.error("No se pudieron leer las macros:", err);
    }
  }

  /** Se lee una vez al empezar y cada vez que cambie algo (otra persona, un turno que dispara una macro...). */
  iniciar() {
    if (this.#escuchando) return;
    this.#escuchando = true;
    void this.recargar();
    listen<{ macros?: boolean; entidades?: boolean } | null>(EVENTO_PARTIDA_CAMBIADA, (e) => {
      if (!e.payload || e.payload.macros || e.payload.entidades) void this.recargar();
    }).catch((err) => console.error("No se pudo escuchar los cambios de macros:", err));
  }

  validar(codigo: string): Promise<Comprobacion> {
    return invoke<Comprobacion>("validar_macro", { codigo });
  }

  async guardar(datos: { id: number; nombre: string; codigo: string }): Promise<Macro> {
    const m = await invoke<Macro>("guardar_macro", { datos });
    await this.recargar();
    return m;
  }

  async borrar(id: number) {
    await invoke("borrar_macro", { id });
    this.ponerBorrador(id, undefined);
    await this.recargar();
  }

  async asociar(idMacro: number, idEntidad: number) {
    await invoke("asociar_macro", { idMacro, idEntidad });
    await this.recargar();
  }

  async desasociar(idAsociacion: number) {
    await invoke("desasociar_macro", { idAsociacion });
    await this.recargar();
  }

  /** El botón de una macro ONCLICK. Lo que haga (vida, chat, otras macros) se vuelve a leer al acabar; un error ya sale en el chat. */
  async ejecutar(idAsociacion: number, idInstancia: number) {
    if (this.ejecutando.includes(idAsociacion)) return;
    this.ejecutando = [...this.ejecutando, idAsociacion];
    try {
      await invoke("ejecutar_macro", { idAsociacion, idInstancia });
    } catch (err) {
      console.error("La macro falló:", err);
    } finally {
      this.ejecutando = this.ejecutando.filter((i) => i !== idAsociacion);
      await Promise.all([this.recargar(), partida.recargarEntidades(), tiradas.cargar()]);
    }
  }
}

export const macros = new Macros();
