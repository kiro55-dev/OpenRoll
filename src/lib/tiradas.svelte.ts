// src/lib/tiradas.svelte.ts
//
// Estado compartido de las tiradas de dados: el historial que se muestra, el texto del chat y
// todo lo que se hace con él (añadir/quitar dados desde la barra de dados, tirar, navegar por los
// comandos anteriores). La barra de dados, la lista de tiradas y la caja de texto lo comparten
// sin pasarse nada entre ellas.
import { invoke } from "$lib/backend";
import { esJugador } from "$lib/modo";
import { esComandoBatalla } from "$lib/batalla";

export interface Tirada {
  hora: string;
  /** Desglose ya formateado por Rust (puede llevar <b>negritas</b>), o un mensaje de error. */
  comando: string;
  /** `null` = la tirada no era válida (no hay total). */
  total: number | null;
  /** Quién tiró (su nombre), para el chat compartido: «Ana: 1d20 (7) = 7». Vacío si fue el host. */
  autor?: string;
  /** `false`: solo la ve el host (una tirada privada). */
  publica?: boolean;
  /** Solo en una comparación (`1d100 <= acro`): el resultado, con el nombre que le da la escala del conjunto. */
  veredicto?: string;
  /** Si la comparación acertó. */
  acierta?: boolean | null;
  /** El resultado salió de una regla del dado natural (crítico, pifia...). */
  especial?: boolean;
}

/** La clase de color de un veredicto: acierto o fallo, y dorado o rojo si es una regla del dado natural (crítico o pifia). */
export function claseVeredicto(acierta: boolean | null | undefined, especial: boolean | undefined): string {
  if (acierta === null || acierta === undefined) return "";
  if (especial) return acierta ? "critico" : "pifia";
  return acierta ? "exito" : "fallo";
}

const MAX_TIRADAS = 20;

const horaActual = () => new Date().toTimeString().slice(0, 5);

/**
 * Pega el bonificador de la barra de dados al comando: vacío o 0 no añade nada; con signo (`-2`, `- f`) se pega tal
 * cual y sin signo (`2`, `f`) se suma. Es texto del chat: Rust lo valida igual que si se escribiera allí.
 */
export function comandoConBonificador(comando: string, bonificador: number | string | null): string {
  const extra = String(bonificador ?? "").trim();
  if (!extra || Number(extra) === 0) return comando;
  const signo = extra[0];
  return signo === "+" || signo === "-" ? `${comando} ${signo} ${extra.slice(1).trim()}` : `${comando} + ${extra}`;
}

/** Las comparaciones del chat (`1d100 <= 45`, `1d20 > 12`, `vs` = `<=`). Espejo de `dados::dividir_comparacion` en Rust. */
export type Comparacion = "<" | "<=" | ">" | ">=";

/** Un comando partido por su comparación: la tirada (izquierda), el operador como se escribió, ya normalizado, y el objetivo. */
export interface ComandoComparado {
  izq: string;
  /** Tal cual está escrito (`vs` incluido). */
  texto: string;
  /** `vs` cuenta como `<=`. */
  op: Comparacion;
  der: string;
}

const RE_COMPARACION = /<=|>=|<|>|(?<![A-Za-z])vs(?![A-Za-z])/gi;

/**
 * Parte el comando por su comparación. `null` si no hay ninguna o si hay más de una (Rust da el error al tirar: aquí no se
 * toca nada de lo escrito).
 */
export function dividirComparacion(comando: string): ComandoComparado | null {
  const hallazgos = [...comando.matchAll(RE_COMPARACION)];
  if (hallazgos.length !== 1) return null;
  const h = hallazgos[0];
  const texto = h[0];
  const op = (texto.toLowerCase() === "vs" ? "<=" : texto) as Comparacion;
  return { izq: comando.slice(0, h.index), texto, op, der: comando.slice((h.index ?? 0) + texto.length) };
}

/** ¿Tiene más de una comparación? (entonces no se toca nada: está mal escrito y Rust lo dirá al tirar) */
const variasComparaciones = (comando: string) => [...comando.matchAll(RE_COMPARACION)].length > 1;

/** Aplica `f` solo a la tirada (el lado izquierdo) si el comando tiene una comparación; si no, a todo. */
function sobreLaTirada(comando: string, f: (tirada: string) => string): string {
  if (variasComparaciones(comando)) return comando;
  const c = dividirComparacion(comando);
  if (!c) return f(comando);
  const izq = f(c.izq).trim();
  if (!izq) return ""; // se quitó el último dado: sin tirada no queda comparación
  const der = c.der.trim();
  return der ? `${izq} ${c.texto} ${der}` : `${izq} ${c.texto} `;
}

/**
 * Pone (o cambia, o quita) la comparación del comando: solo puede haber una. Sin comparación la añade al final (con el chat
 * vacío no hace nada: antes hace falta un dado); con otra distinta la SUSTITUYE conservando los dos lados; y pulsar la que ya
 * está la quita, junto con lo que hubiera a su derecha (un objetivo sin comparación no vale nada).
 */
export function ponerComparacion(comando: string, op: Comparacion): string {
  if (!comando.trim() || variasComparaciones(comando)) return comando;
  const c = dividirComparacion(comando);
  if (!c) return `${comando.trim()} ${op} `;
  if (c.op === op && c.texto === op) return c.izq.trim();
  const der = c.der.trim();
  return der ? `${c.izq.trim()} ${op} ${der}` : `${c.izq.trim()} ${op} `;
}

/**
 * El comando que sale al pulsar TIRAR en la barra de dados. Sin comparación: lo escrito más el bonificador, de siempre (el
 * objetivo no se usa). Con ella, el bonificador va en la tirada (el lado IZQUIERDO) y el objetivo de la barra ocupa el derecho,
 * pero solo si ese está vacío: lo escrito a mano manda. `1d20 >=` + bonificador 3 + objetivo 12 → `1d20 + 3 >= 12`.
 */
export function comandoParaTirar(comando: string, bonificador: number | string | null, objetivo: number | string | null): string {
  const c = variasComparaciones(comando) ? null : dividirComparacion(comando);
  if (!c) return comandoConBonificador(comando, bonificador);
  const der = c.der.trim() || String(objetivo ?? "").trim();
  return `${comandoConBonificador(c.izq.trim(), bonificador)} ${c.texto} ${der}`.trim();
}

/** Los términos de dados de un texto (`5d20`, `1d6adv`...), con su posición. Los límites evitan tocar nombres de variables. */
const RE_DADO = /(?<![A-Za-z0-9])(\d*)d(\d+)(adv|desv|dis)?(?![A-Za-z0-9])/gi;

/** El término de dados al que afecta Vent/Desv: el último de ese dado (`caras`) o, si no hay, el último de todos. */
function terminoDeDado(tirada: string, caras: string | null): RegExpMatchArray | null {
  const todos = [...tirada.matchAll(RE_DADO)];
  return (caras ? todos.filter((m) => m[2] === caras).at(-1) : undefined) ?? todos.at(-1) ?? null;
}

export type ModificadorDado = "adv" | "desv";

/** El modificador que tiene ahora el dado afectado (`dis` es otra forma de `desv`), o `null`. */
export function modificadorDelDado(comando: string, caras: string | null): ModificadorDado | null {
  const c = variasComparaciones(comando) ? null : dividirComparacion(comando);
  const m = terminoDeDado(c ? c.izq : comando, caras);
  const s = m?.[3]?.toLowerCase();
  return s === "adv" ? "adv" : s ? "desv" : null;
}

/**
 * Ventaja o desventaja sobre el ÚLTIMO dado puesto en el chat (`5d20` → `5d20adv`): sin modificador lo pone; el mismo otra vez
 * lo quita; el contrario lo sustituye. Sin dados, o con la comparación mal escrita, no hace nada. Con una comparación solo mira
 * la tirada (a su izquierda).
 */
export function aplicarModificadorAlDado(comando: string, modificador: ModificadorDado, caras: string | null = null): string {
  return sobreLaTirada(comando, (tirada) => {
    const m = terminoDeDado(tirada, caras);
    if (!m || m.index === undefined) return tirada;
    const actual = m[3]?.toLowerCase();
    const nuevo = (actual === "adv" ? "adv" : actual ? "desv" : null) === modificador ? "" : modificador;
    // Si ya hay otro grupo igual de ese dado (con el mismo modificador), el término se junta con él: 1d20adv + 1d20adv → 2d20adv
    return agruparDados(tirada.slice(0, m.index) + `${m[1]}d${m[2]}${nuevo}` + tirada.slice(m.index + m[0].length));
  });
}

/**
 * Junta los términos de dados iguales (mismo signo, mismo dado y mismo modificador) sumando sus cantidades: `1d20adv + 1d20adv` →
 * `2d20adv`. Solo lo hace si hay algo que juntar (si no, devuelve el texto tal cual); no toca términos con multiplicador
 * (`2*3d20`) ni los que no son dados.
 */
export function agruparDados(tirada: string): string {
  const trozos = tirada.split(/([+-])/);
  const terminos: { signo: string; texto: string; cantidad?: number; clave?: string }[] = [];
  let signo = "+";
  for (let k = 0; k < trozos.length; k++) {
    if (k % 2 === 1) {
      signo = trozos[k];
      continue;
    }
    const texto = trozos[k].trim();
    if (!texto && k === 0) continue; // un comando que empieza con «-»
    const m = texto.match(/^(\d*)d(\d+)(adv|desv|dis)?$/i);
    if (!m) {
      terminos.push({ signo, texto });
      continue;
    }
    const sufijo = m[3]?.toLowerCase() === "adv" ? "adv" : m[3] ? "desv" : "";
    terminos.push({ signo, texto, cantidad: m[1] ? parseInt(m[1]) : 1, clave: `${signo}d${m[2]}${sufijo}` });
  }
  const juntos: typeof terminos = [];
  let hubo = false;
  for (const t of terminos) {
    const igual = t.clave ? juntos.find((x) => x.clave === t.clave) : undefined;
    if (igual) {
      igual.cantidad = (igual.cantidad ?? 1) + (t.cantidad ?? 1);
      hubo = true;
    } else {
      juntos.push({ ...t });
    }
  }
  if (!hubo) return tirada;
  return juntos
    .map((t, k) => {
      const cuerpo = t.clave ? `${t.cantidad}${t.texto.replace(/^\d*/, "")}` : t.texto;
      return k === 0 ? (t.signo === "-" ? `- ${cuerpo}` : cuerpo) : `${t.signo === "-" ? "-" : "+"} ${cuerpo}`;
    })
    .join(" ")
    .trim();
}

/**
 * Añade (`esSuma`) o quita un dado al comando del chat, agrupando los iguales:
 * "1d20" + "1d6" -> "1d20 + 1d6"; otra vez "1d6" -> "1d20 + 2d6"; restando se va reduciendo
 * hasta desaparecer. Solo cuenta el término del mismo dado Y con el mismo modificador: pulsar d20 con un `1d20adv` en el chat añade
 * un `1d20` aparte (sin ventaja); luego Vent sobre ese último lo junta con el que ya existe. Con una comparación, solo toca la
 * tirada (a su izquierda).
 * Función pura (no toca el estado) para poder probarla.
 */
export function aplicarDadoAlComando(comando: string, fragmento: string, esSuma: boolean): string {
  return sobreLaTirada(comando, (tirada) => aplicarDadoALaTirada(tirada, fragmento, esSuma));
}

function aplicarDadoALaTirada(comando: string, fragmento: string, esSuma: boolean): string {
  const matchFrag = fragmento.match(/^(\d*)d(\d+)([!a-z]*)$/i);
  if (!matchFrag) return comando;

  const delta = (matchFrag[1] ? parseInt(matchFrag[1]) : 1) * (esSuma ? 1 : -1);
  const caras = matchFrag[2];
  const sufijos = matchFrag[3] || "";

  const sufijosEscapados = sufijos.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(`(^|[+-])\\s*(\\d*)d${caras}${sufijosEscapados}(?=\\s|[+-]|$)`, "i");
  const match = comando.match(regex);

  if (!match) {
    if (esSuma) {
      const texto = `${matchFrag[1] || 1}d${caras}${sufijos}`;
      return comando.trim() ? `${comando.trim()} + ${texto}` : texto;
    }
    const texto = `- ${matchFrag[1] || 1}d${caras}${sufijos}`;
    return comando.trim() ? `${comando.trim()} ${texto}` : texto;
  }

  const signoActual = match[1] === "-" ? -1 : 1;
  const vecesActuales = (match[2] ? parseInt(match[2]) : 1) * signoActual;
  const nuevaCantidad = vecesActuales + delta;

  let resultado: string;
  if (nuevaCantidad === 0) {
    resultado = comando.replace(regex, "");
  } else if (nuevaCantidad > 0) {
    const prefijo = match.index === 0 ? "" : "+ ";
    resultado = comando.replace(regex, `${prefijo}${nuevaCantidad}d${caras}${sufijos}`);
  } else {
    resultado = comando.replace(regex, `- ${Math.abs(nuevaCantidad)}d${caras}${sufijos}`);
  }

  return resultado
    .replace(/^\s*\+\s*/, "")
    .replace(/\s*[+-]\s*$/, "")
    .replace(/\s*\+\s*\+/g, " + ")
    .replace(/\s*\+\s*-\s*/g, " - ")
    .trim();
}

/**
 * El desglose listo para `{@html}`: escapa TODO y solo vuelve a permitir `<b>` y `</b>` (las negritas que
 * pone Rust en el 1 y en la cara máxima). Rust ya solo genera números y palabras fijas, así que es una
 * defensa de más: aunque algún día llegara por red un desglose manipulado, no se ejecutaría nada.
 */
export function desgloseSeguro(desglose: string): string {
  return desglose
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/&lt;(\/?)b&gt;/g, "<$1b>");
}

/**
 * `clean` o `c` (solos, da igual mayúsculas) vacían el chat. Mezclados con otra cosas (`c + 1d6`) no
 * lo son: van a Rust como tirada y no es válida. Igual que `es_comando_limpiar` de Rust.
 */
export function esComandoLimpiar(comando: string): boolean {
  return /^(clean|c)$/i.test(comando.trim());
}

class Tiradas {
  /** Las últimas tiradas, la más reciente primero. */
  historial = $state<Tirada[]>([]);
  /** Lo que hay escrito en la caja de comandos. */
  comando = $state("");
  /** Solo el host: si las tiradas que hace las ven los jugadores (casilla de la barra de dados). */
  publica = $state(true);
  /** Las caras del último dado pulsado en la barra (`"20"`): es al que se aplican Vent y Desv. Se limpia al tirar. */
  #ultimoDado: string | null = null;

  /** La instancia con el foco (una sola), de la que salen las variables de una tirada (`2d6 + f`). La pone
   *  `partida.svelte.ts` (que ya importa este store: así no se importan el uno al otro). */
  fichaConFoco: () => number | null = () => null;
  /** Lo que hay que releer tras `Empezar` o `Terminar` (lo pone `partida.svelte.ts`, por lo mismo). */
  alCambiarBatalla: () => Promise<void> = async () => {};

  // Navegación por los comandos anteriores con ↑ ↓, como en una terminal.
  #anteriores: string[] = [];
  #indice = -1; // -1 = no se está navegando
  #borrador = ""; // lo que se estaba escribiendo antes de empezar a navegar

  #enfocar: (() => void) | null = null;

  /** Carga el historial guardado en Rust. */
  async cargar() {
    // El chat es de todos: cada tirada lleva el nombre de quien la hizo (`autor`, vacío si fue el host)
    const guardadas = await invoke<{ hora: string; comando: string; total: number; autor?: string; publica?: boolean; veredicto?: string; acierta?: boolean | null; especial?: boolean }[]>("obtener_historial_tiradas");
    this.historial = guardadas.map((t) => ({ hora: t.hora, comando: t.comando, total: t.total, autor: t.autor ?? "", publica: t.publica ?? true, veredicto: t.veredicto ?? "", acierta: t.acierta ?? null, especial: t.especial ?? false }));
    this.#salirDelHistorial(); // los comandos anteriores (↑ ↓) también son de la partida abierta
  }

  // ------------------------------------------------------------------
  // Composición del comando (barra de dados)
  // ------------------------------------------------------------------

  sumarDado(fragmento: string) {
    this.#ultimoDado = fragmento.match(/d(\d+)/i)?.[1] ?? this.#ultimoDado;
    this.comando = aplicarDadoAlComando(this.comando, fragmento, true);
    this.#enfocar?.();
  }

  restarDado(fragmento: string) {
    this.#ultimoDado = fragmento.match(/d(\d+)/i)?.[1] ?? this.#ultimoDado;
    this.comando = aplicarDadoAlComando(this.comando, fragmento, false);
    this.#enfocar?.();
  }

  /** Botones Vent / Desv: ponen, cambian o quitan el modificador del último dado puesto en el chat. */
  alternarModificador(modificador: ModificadorDado) {
    this.comando = aplicarModificadorAlDado(this.comando, modificador, this.#ultimoDado);
    this.#enfocar?.();
  }

  /** El modificador que tiene ahora el dado al que afectan Vent y Desv (para marcar el botón). */
  get modificadorActivo(): ModificadorDado | null {
    return modificadorDelDado(this.comando, this.#ultimoDado);
  }

  /** Botones `<=` y `>=`: escriben la comparación en el chat (solo hay una: la nueva sustituye a la anterior; la misma la quita). */
  alternarComparacion(op: Comparacion) {
    this.comando = ponerComparacion(this.comando, op);
    this.#enfocar?.();
  }

  /** La comparación que hay escrita en el chat (para marcar su botón), o `null`. */
  get comparacionActiva(): Comparacion | null {
    return dividirComparacion(this.comando)?.op ?? null;
  }

  /** La caja de texto se registra aquí para que otros puedan pedirle el foco. Devuelve cómo darse de baja. */
  registrarFoco(enfocar: () => void): () => void {
    this.#enfocar = enfocar;
    return () => {
      if (this.#enfocar === enfocar) this.#enfocar = null;
    };
  }

  // ------------------------------------------------------------------
  // Tirar
  // ------------------------------------------------------------------

  /** Intro en la caja de texto: tira lo escrito y la vacía enseguida. */
  async enviar() {
    const cmd = this.comando.trim();
    if (!cmd) return;
    this.comando = "";
    this.#ultimoDado = null;
    this.#salirDelHistorial();
    await this.tirar(cmd);
  }

  /** Botón TIRAR de la barra de dados: lo escrito más el bonificador (un número o texto del chat: `f`, `-fuerza`, `2 + f`) y,
   *  si hay una comparación, el objetivo de la barra (ver `comandoParaTirar`). */
  async tirarConBonificador(bonificador: number | string | null, objetivo: number | string | null = null) {
    if (!this.comando.trim()) return;
    const final = comandoParaTirar(this.comando, bonificador, objetivo);
    this.comando = "";
    this.#ultimoDado = null;
    this.#salirDelHistorial();
    await this.tirar(final);
  }

  async tirar(comando: string) {
    const cmd = comando.trim();
    if (!cmd) return;

    if (esComandoLimpiar(cmd)) {
      try {
        await invoke("limpiar_chat", { comando: cmd });
        this.historial = [];
      } catch (err) {
        console.error("limpiar_chat:", err);
        // Solo el host limpia el chat de todos
        if (esJugador) this.#anadir({ hora: horaActual(), comando: "Solo el host puede limpiar el chat", total: null });
      }
      return;
    }

    try {
      // Las variables (`2d6 + f`) salen de la ficha con el foco: Rust solo la mira si el comando usa alguna.
      // Solo el host puede tirar en privado (las de los jugadores son siempre públicas)
      const idInstancia = this.fichaConFoco();
      await invoke<string>("tirar_dado", esJugador ? { comando: cmd, idInstancia } : { comando: cmd, publica: this.publica, idInstancia });
      // La tirada ya está en el chat de la partida (con el nombre de quien tira): se vuelve a leer, así sale
      // igual para todos y no se duplica cuando llega también como cambio
      await this.cargar();
      if (esComandoBatalla(cmd)) await this.alCambiarBatalla();
    } catch (err) {
      // El motivo, si Rust lo da (sin foco, una variable que no existe...). Solo lo ve quien tira.
      const motivo = typeof err === "string" && err.trim() ? `: ${err}` : "";
      this.#anadir({ hora: horaActual(), comando: `tirada no válida${motivo}`, total: null });
    }
  }

  #anadir(tirada: Tirada) {
    this.historial.unshift(tirada);
    if (this.historial.length > MAX_TIRADAS) this.historial.pop();
  }

  // ------------------------------------------------------------------
  // Historial de comandos (↑ ↓)
  // ------------------------------------------------------------------

  get navegandoHistorial() {
    return this.#indice !== -1;
  }

  /** ↑: pone en la caja el comando anterior. Devuelve `true` si cambió el texto. */
  async historialAnterior(): Promise<boolean> {
    if (this.#indice === -1) {
      this.#anteriores = await invoke<string[]>("obtener_historial_personal");
      if (this.#anteriores.length === 0) return false;
      this.#borrador = this.comando; // se guarda lo que estaba escribiendo
      this.#indice = this.#anteriores.length;
    }
    if (this.#indice <= 0) return false;

    this.#indice--;
    this.comando = this.#anteriores[this.#indice];
    return true;
  }

  /** ↓: avanza hacia los más recientes; al pasar el último, devuelve lo que se estaba escribiendo. */
  historialSiguiente(): boolean {
    if (this.#indice === -1) return false;
    if (this.#indice >= this.#anteriores.length) return false;

    this.#indice++;
    if (this.#indice === this.#anteriores.length) {
      this.comando = this.#borrador;
      this.#indice = -1;
    } else {
      this.comando = this.#anteriores[this.#indice];
    }
    return true;
  }

  #salirDelHistorial() {
    this.#indice = -1;
    this.#borrador = "";
  }
}

export const tiradas = new Tiradas();
