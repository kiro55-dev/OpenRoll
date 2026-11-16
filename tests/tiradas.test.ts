import { beforeEach, describe, expect, it, vi } from "vitest";

type Args = Record<string, any>;

// ---- Referencia: el código ORIGINAL de +page.svelte (actualizarDadoEnChat), tal cual, sobre una variable ----
function referencia(comandoChat: string, fragmento: string, esSuma: boolean): string {
  const matchFrag = fragmento.match(/^(\d*)d(\d+)([!a-z]*)$/i);
  if (!matchFrag) return comandoChat;

  const delta = (matchFrag[1] ? parseInt(matchFrag[1]) : 1) * (esSuma ? 1 : -1);
  const caras = matchFrag[2];
  const sufijos = matchFrag[3] || "";

  const sufijosEscapados = sufijos.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(`(^|[+-])\\s*(\\d*)d${caras}${sufijosEscapados}(?=\\s|[+-]|$)`, "i");
  const match = comandoChat.match(regex);

  if (match) {
    const signoActual = match[1] === "-" ? -1 : 1;
    const vecesActuales = (match[2] ? parseInt(match[2]) : 1) * signoActual;
    const nuevaCantidad = vecesActuales + delta;

    if (nuevaCantidad === 0) {
      comandoChat = comandoChat.replace(regex, "");
    } else if (nuevaCantidad > 0) {
      const prefijo = match.index === 0 ? "" : "+ ";
      comandoChat = comandoChat.replace(regex, `${prefijo}${nuevaCantidad}d${caras}${sufijos}`);
    } else {
      const prefijo = "- ";
      comandoChat = comandoChat.replace(regex, `${prefijo}${Math.abs(nuevaCantidad)}d${caras}${sufijos}`);
    }

    comandoChat = comandoChat
      .replace(/^\s*\+\s*/, "")
      .replace(/\s*[+-]\s*$/, "")
      .replace(/\s*\+\s*\+/g, " + ")
      .replace(/\s*\+\s*-\s*/g, " - ")
      .trim();
  } else {
    if (esSuma) {
      const texto = `${matchFrag[1] || 1}d${caras}${sufijos}`;
      comandoChat = comandoChat.trim() ? `${comandoChat.trim()} + ${texto}` : texto;
    } else {
      const texto = `- ${matchFrag[1] || 1}d${caras}${sufijos}`;
      comandoChat = comandoChat.trim() ? `${comandoChat.trim()} ${texto}` : texto;
    }
  }
  return comandoChat;
}

// ---- Backend falso ----
let tirarRespuesta: (cmd: string) => string;
let personales: string[];
let llamadas: { cmd: string; args: Args }[];
/** El chat de la partida (lo que guarda Rust): la más reciente primero, con el nombre de quien tiró. */
let chat: { hora: string; comando: string; total: number; autor: string }[];

async function nuevoStore(comoJugador = false) {
  vi.resetModules();
  llamadas = [];
  personales = [];
  chat = [{ hora: "10:00", comando: "1d20 (15)", total: 15, autor: "" }];
  tirarRespuesta = (c) => `${c} (<b>7</b>) = 7`;
  vi.doMock("$lib/modo", () => ({ esJugador: comoJugador }));
  vi.doMock("@tauri-apps/api/core", () => ({
    convertFileSrc: (p: string) => p,
    invoke: async (cmd: string, args: Args = {}) => {
      llamadas.push({ cmd, args });
      if (cmd === "tirar_dado") {
        const r = tirarRespuesta(args.comando);
        if (r === "ERROR") throw "comando inválido";
        // Rust la registra en el chat de la partida (20 como mucho), con quien la hace
        const [comando, total] = r.split(" = ");
        chat.unshift({ hora: "10:01", comando, total: Number(total), autor: comoJugador ? "Ana" : "" });
        if (chat.length > 20) chat.pop();
        return r;
      }
      if (cmd === "limpiar_chat") {
        if (comoJugador) throw "Esto solo lo puede hacer el host";
        if (!/^(clean|c)$/i.test(String(args.comando).trim())) throw "no es la orden de limpiar";
        chat = [];
        return;
      }
      if (cmd === "obtener_historial_personal") return [...personales];
      if (cmd === "obtener_historial_tiradas") return structuredClone(chat);
      throw new Error("comando desconocido " + cmd);
    }
  }));
  // Un jugador no habla con Tauri: su `invoke` es el de la conexión. Aquí, el mismo backend falso (el del host)
  if (comoJugador) {
    vi.doMock("$lib/backend", async () => ({ invoke: (await import("@tauri-apps/api/core")).invoke }));
  } else {
    vi.doUnmock("$lib/backend"); // que un test de jugador no deje su mock a los demás
  }
  return await import("$lib/tiradas.svelte");
}

describe("desgloseSeguro (lo que se pinta con {@html})", () => {
  it("conserva las negritas de Rust", async () => {
    const { desgloseSeguro } = await nuevoStore();
    expect(desgloseSeguro("2d20adv ([<b>20</b>, 3], [7, <b>1</b>]) + 3")).toBe("2d20adv ([<b>20</b>, 3], [7, <b>1</b>]) + 3");
  });

  it("escapa cualquier otra etiqueta, atributo o comilla", async () => {
    const { desgloseSeguro } = await nuevoStore();
    expect(desgloseSeguro("<script>alert(1)</script>")).toBe("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(desgloseSeguro('<img src=x onerror="alert(1)">')).toBe("&lt;img src=x onerror=&quot;alert(1)&quot;&gt;");
    expect(desgloseSeguro("<b onclick='x'>5</b>")).toBe("&lt;b onclick=&#39;x&#39;&gt;5</b>");
    expect(desgloseSeguro("<B>5</B> & <bold>")).toBe("&lt;B&gt;5&lt;/B&gt; &amp; &lt;bold&gt;");
  });
});

describe("aplicarDadoAlComando (barra de dados -> texto del chat)", () => {
  it("agrupa los dados iguales, suma, resta y los quita al llegar a cero", async () => {
    const { aplicarDadoAlComando: f } = await nuevoStore();
    expect(f("", "1d20", true)).toBe("1d20");
    expect(f("1d20", "1d20", true)).toBe("2d20");
    expect(f("1d20", "1d6", true)).toBe("1d20 + 1d6");
    expect(f("1d20 + 1d6", "1d6", true)).toBe("1d20 + 2d6");
    expect(f("2d20", "1d20", false)).toBe("1d20");
    expect(f("1d20", "1d20", false)).toBe("");
    expect(f("1d20 + 1d6", "1d20", false)).toBe("1d6");
    expect(f("", "1d8", false)).toBe("- 1d8");
    expect(f("1d20", "3d6adv", true)).toBe("1d20 + 3d6adv");
    expect(f("1d20", "esto no es un dado", true)).toBe("1d20");
  });

  it("se comporta exactamente igual que el código original (500 secuencias al azar)", async () => {
    const { aplicarDadoAlComando: f } = await nuevoStore();
    let semilla = 12345;
    const azar = (n: number) => ((semilla = (semilla * 1103515245 + 12345) & 0x7fffffff) % n);
    const dados = ["1d4", "1d6", "2d6", "1d8", "1d10", "1d12", "1d20", "3d20", "1d20adv", "1d20desv", "2d8adv", "d6", "xd6", ""];

    for (let k = 0; k < 500; k++) {
      let a = "";
      let b = "";
      for (let paso = 0; paso < 12; paso++) {
        const d = dados[azar(dados.length)];
        const suma = azar(2) === 0;
        a = f(a, d, suma);
        b = referencia(b, d, suma);
        expect(a).toBe(b);
      }
    }
  });
});

describe("tirar", () => {
  beforeEach(() => vi.useRealTimers());

  it("añade la tirada al principio con su total como número", async () => {
    const { tiradas } = await nuevoStore();
    await tiradas.cargar();
    await tiradas.tirar("1d20");
    expect(tiradas.historial[0]).toMatchObject({ comando: "1d20 (<b>7</b>)", total: 7 });
    expect(tiradas.historial[1]).toMatchObject({ hora: "10:00", total: 15 }); // la que venía de Rust
  });

  it("el chat es de todos: la tirada lleva el nombre de quien la hizo y las de los demás llegan al recargar", async () => {
    const { tiradas } = await nuevoStore(true); // un jugador (se llama Ana)
    chat.unshift({ hora: "10:02", comando: "1d8 (<b>8</b>)", total: 8, autor: "Luis" }); // la ha tirado otro
    await tiradas.tirar("1d20");
    expect(tiradas.historial.map((t) => [t.autor, t.total])).toEqual([["Ana", 7], ["Luis", 8], ["", 15]]);
    // Una sola vez: no se duplica porque ahora llegue también como cambio
    await tiradas.cargar();
    expect(tiradas.historial).toHaveLength(3);
  });

  it("un jugador no puede limpiar el chat de todos: sale un aviso y el chat sigue", async () => {
    const { tiradas } = await nuevoStore(true);
    await tiradas.cargar();
    await tiradas.tirar("clean");
    expect(tiradas.historial[0]).toMatchObject({ comando: "Solo el host puede limpiar el chat", total: null });
    expect(chat).toHaveLength(1);
  });

  it("un comando inválido deja una línea de error (total null) en vez de romper", async () => {
    const { tiradas } = await nuevoStore();
    tirarRespuesta = () => "ERROR";
    await tiradas.tirar("patata");
    // Con el motivo que da Rust, que solo ve quien tira
    expect(tiradas.historial[0]).toMatchObject({ comando: "tirada no válida: comando inválido", total: null });
  });

  it("manda la ficha con el foco (de ella salen las variables, 2d6 + f)", async () => {
    const { tiradas } = await nuevoStore();
    await tiradas.tirar("2d6 + f");
    expect(llamadas.find((l) => l.cmd === "tirar_dado")?.args).toMatchObject({ comando: "2d6 + f", idInstancia: null });
    tiradas.fichaConFoco = () => 7;
    await tiradas.tirar("1d20 + f");
    expect(llamadas.filter((l) => l.cmd === "tirar_dado")[1].args).toMatchObject({ idInstancia: 7, publica: true });
    // Rust la rechaza sin foco: se ve por qué
    tirarRespuesta = () => "ERROR";
    await tiradas.tirar("1d20 + f");
    expect(tiradas.historial[0].comando).toBe("tirada no válida: comando inválido");
  });

  it("el jugador también manda su ficha con el foco", async () => {
    const { tiradas } = await nuevoStore(true);
    tiradas.fichaConFoco = () => 3;
    await tiradas.tirar("1d20 + f");
    expect(llamadas.find((l) => l.cmd === "tirar_dado")?.args).toEqual({ comando: "1d20 + f", idInstancia: 3 });
  });

  it("un total 0 real se muestra como total, no como error", async () => {
    const { tiradas } = await nuevoStore();
    tirarRespuesta = () => "1d4 (<b>2</b>) - 2 = 0";
    await tiradas.tirar("1d4 - 2");
    expect(tiradas.historial[0].total).toBe(0);
  });

  it("guarda como máximo 20 tiradas", async () => {
    const { tiradas } = await nuevoStore();
    for (let i = 0; i < 25; i++) await tiradas.tirar(`1d${i + 2}`);
    expect(tiradas.historial).toHaveLength(20);
    expect(tiradas.historial[0].comando).toContain("1d26");
  });

  it("clean o c, solos, vacían el chat en Rust y en pantalla; mezclados son una tirada no válida", async () => {
    const { tiradas, esComandoLimpiar } = await nuevoStore();
    for (const c of ["c", "C", "clean", " Clean "]) expect(esComandoLimpiar(c)).toBe(true);
    for (const c of ["c + 1d6", "clean 1d6", "cc", "1d6"]) expect(esComandoLimpiar(c)).toBe(false);

    await tiradas.cargar();
    await tiradas.tirar("1d20");
    expect(tiradas.historial).toHaveLength(2);
    await tiradas.tirar(" CLEAN ");
    expect(tiradas.historial).toEqual([]);
    expect(llamadas.filter((l) => l.cmd === "limpiar_chat")).toHaveLength(1);
    expect(llamadas.some((l) => l.cmd === "tirar_dado" && /clean/i.test(l.args.comando))).toBe(false);

    // Mezclado: va a Rust como tirada, que lo rechaza
    tirarRespuesta = () => "ERROR";
    await tiradas.tirar("c + 1d6");
    expect(tiradas.historial[0]).toMatchObject({ comando: "tirada no válida: comando inválido", total: null });
    expect(llamadas.filter((l) => l.cmd === "limpiar_chat")).toHaveLength(1);
  });

  it("ignora comandos vacíos", async () => {
    const { tiradas } = await nuevoStore();
    await tiradas.tirar("   ");
    expect(llamadas).toHaveLength(0);
  });
});

describe("enviar y botón TIRAR", () => {
  it("Intro vacía la caja enseguida y tira lo escrito", async () => {
    const { tiradas } = await nuevoStore();
    tiradas.comando = "  2d6 + 3 ";
    const promesa = tiradas.enviar();
    expect(tiradas.comando).toBe(""); // antes de que Rust responda
    await promesa;
    expect(llamadas.find((l) => l.cmd === "tirar_dado")?.args.comando).toBe("2d6 + 3");
  });

  it("TIRAR suma o resta el bonificador y vacía la caja", async () => {
    const { tiradas } = await nuevoStore();
    tiradas.comando = "1d20";
    await tiradas.tirarConBonificador(5);
    tiradas.comando = "1d20";
    await tiradas.tirarConBonificador(-2);
    tiradas.comando = "1d20";
    await tiradas.tirarConBonificador(0);
    tiradas.comando = "1d20";
    await tiradas.tirarConBonificador(null); // campo vacío: cuenta como 0, no "- 0"

    const enviados = llamadas.filter((l) => l.cmd === "tirar_dado").map((l) => l.args.comando);
    expect(enviados).toEqual(["1d20 + 5", "1d20 - 2", "1d20", "1d20"]);
    expect(tiradas.comando).toBe("");
  });

  it("el bonificador también puede ser texto del chat (variables)", async () => {
    const { tiradas, comandoConBonificador } = await nuevoStore();
    const casos: [string | number | null, string][] = [
      ["f", "1d20 + f"], ["-f", "1d20 - f"], ["- fuerza", "1d20 - fuerza"], ["+ f", "1d20 + f"],
      ["  2 ", "1d20 + 2"], ["-3", "1d20 - 3"], ["f + 2", "1d20 + f + 2"], ["", "1d20"], ["0", "1d20"], [null, "1d20"], [0, "1d20"]
    ];
    for (const [bonif, esperado] of casos) expect(comandoConBonificador("1d20", bonif), String(bonif)).toBe(esperado);

    tiradas.comando = "1d20";
    await tiradas.tirarConBonificador("-f");
    expect(llamadas.find((l) => l.cmd === "tirar_dado")?.args.comando).toBe("1d20 - f");
  });

  it("TIRAR con la caja vacía no hace nada", async () => {
    const { tiradas } = await nuevoStore();
    await tiradas.tirarConBonificador(3);
    expect(llamadas).toHaveLength(0);
  });
});

describe("foco desde la barra de dados", () => {
  it("sumar o restar un dado pide el foco a la caja registrada; al darse de baja ya no", async () => {
    const { tiradas } = await nuevoStore();
    const enfocar = vi.fn();
    const baja = tiradas.registrarFoco(enfocar);

    tiradas.sumarDado("1d20");
    tiradas.restarDado("1d20");
    expect(enfocar).toHaveBeenCalledTimes(2);
    expect(tiradas.comando).toBe("");

    baja();
    tiradas.sumarDado("1d6");
    expect(enfocar).toHaveBeenCalledTimes(2);
    expect(tiradas.comando).toBe("1d6");
  });

  it("una baja tardía no desregistra a la caja nueva", async () => {
    const { tiradas } = await nuevoStore();
    const a = vi.fn();
    const b = vi.fn();
    const bajaA = tiradas.registrarFoco(a);
    tiradas.registrarFoco(b);
    bajaA();
    tiradas.sumarDado("1d6");
    expect(b).toHaveBeenCalledTimes(1);
  });
});

describe("historial de comandos con ↑ y ↓", () => {
  it("sube por los anteriores, baja, y al pasar el último recupera lo que se estaba escribiendo", async () => {
    const { tiradas } = await nuevoStore();
    personales = ["1d20", "2d6", "3d4"];
    tiradas.comando = "borrador";

    expect(await tiradas.historialAnterior()).toBe(true);
    expect(tiradas.comando).toBe("3d4");
    expect(tiradas.navegandoHistorial).toBe(true);
    expect(await tiradas.historialAnterior()).toBe(true);
    expect(tiradas.comando).toBe("2d6");
    expect(await tiradas.historialAnterior()).toBe(true);
    expect(tiradas.comando).toBe("1d20");
    expect(await tiradas.historialAnterior()).toBe(false); // ya es el más antiguo
    expect(tiradas.comando).toBe("1d20");

    expect(tiradas.historialSiguiente()).toBe(true);
    expect(tiradas.comando).toBe("2d6");
    expect(tiradas.historialSiguiente()).toBe(true);
    expect(tiradas.comando).toBe("3d4");
    expect(tiradas.historialSiguiente()).toBe(true);
    expect(tiradas.comando).toBe("borrador");
    expect(tiradas.navegandoHistorial).toBe(false);
    expect(tiradas.historialSiguiente()).toBe(false);
  });

  it("sin comandos guardados, ↑ no cambia nada ni entra en modo historial", async () => {
    const { tiradas } = await nuevoStore();
    tiradas.comando = "algo";
    expect(await tiradas.historialAnterior()).toBe(false);
    expect(tiradas.comando).toBe("algo");
    expect(tiradas.navegandoHistorial).toBe(false);
  });

  it("enviar sale del modo historial", async () => {
    const { tiradas } = await nuevoStore();
    personales = ["1d20"];
    await tiradas.historialAnterior();
    await tiradas.enviar();
    expect(tiradas.navegandoHistorial).toBe(false);
    expect(tiradas.comando).toBe("");
  });
});

describe("el veredicto de una comparación (`1d100 <= 45`)", () => {
  it("se pinta según haya acertado y si salió de una regla del dado natural", async () => {
    const { claseVeredicto } = await nuevoStore();
    expect(claseVeredicto(true, false)).toBe("exito");
    expect(claseVeredicto(false, false)).toBe("fallo");
    expect(claseVeredicto(true, true)).toBe("critico");
    expect(claseVeredicto(false, true)).toBe("pifia");
    // Una tirada normal (sin comparación) no se pinta
    expect(claseVeredicto(null, false)).toBe("");
    expect(claseVeredicto(undefined, undefined)).toBe("");
  });

  it("el historial trae el nombre y si acertó; el de una tirada normal queda vacío", async () => {
    const { tiradas } = await nuevoStore();
    chat = [
      { hora: "10:00", comando: "1d100 (30) <= acro (60)", total: 30, autor: "", veredicto: "Éxito difícil", acierta: true, especial: false },
      { hora: "10:01", comando: "1d6 (3)", total: 3, autor: "" }
    ] as any;
    await tiradas.cargar();
    expect(tiradas.historial[0]).toMatchObject({ veredicto: "Éxito difícil", acierta: true, especial: false });
    expect(tiradas.historial[1]).toMatchObject({ veredicto: "", acierta: null, especial: false });
  });
});

describe("barra de dados: ventaja y desventaja sobre el último dado", () => {
  it("un dado sin modificador va aparte del que ya lo tiene, y Vent sobre ese último los junta", async () => {
    const { aplicarDadoAlComando: f, aplicarModificadorAlDado: m } = await nuevoStore();
    // Con un 1d20adv en el chat, otro d20 es un grupo aparte, sin ventaja
    expect(f("1d20adv", "1d20", true)).toBe("1d20adv + 1d20");
    expect(f("1d20adv + 1d20", "1d20", true)).toBe("1d20adv + 2d20");
    expect(f("1d20adv + 2d20", "1d20", false)).toBe("1d20adv + 1d20");
    expect(f("1d20adv", "1d20desv", true)).toBe("1d20adv + 1d20desv");
    // Vent sobre el último grupo (el que no la tiene) lo junta con el que ya existe
    expect(m("1d20adv + 1d20", "adv", "20")).toBe("2d20adv");
    expect(m("1d20adv + 2d20", "adv", "20")).toBe("3d20adv");
    expect(m("2d20desv + 1d20desv + 1d6", "desv", "20")).toBe("2d20desv + 1d20 + 1d6"); // quitarla del último no junta con el desv
    // Quitar el modificador a un grupo puede juntarlo con el que no lo tiene
    expect(m("1d20 + 1d20adv", "adv", "20")).toBe("2d20");
    // Los términos que no son dados se conservan, también los negativos
    expect(m("1d20adv + 1d20 + 3 - f", "adv", "20")).toBe("2d20adv + 3 - f");
    expect(m("- 1d6adv - 1d6", "adv", "6")).toBe("- 2d6adv");
    expect(m("1d20adv + 1d20 - 1d20", "adv", "20")).toBe("1d20adv + 1d20 - 1d20adv");
  });

  it("Vent y Desv se ponen, se cambian y se quitan en el último dado", async () => {
    const { aplicarModificadorAlDado: f } = await nuevoStore();
    expect(f("5d20", "adv", "20")).toBe("5d20adv");
    expect(f("5d20adv", "adv", "20")).toBe("5d20"); // el mismo otra vez lo quita
    expect(f("5d20adv", "desv", "20")).toBe("5d20desv"); // el contrario lo sustituye
    expect(f("5d20desv", "adv", "20")).toBe("5d20adv");
    expect(f("5d20dis", "desv", "20")).toBe("5d20"); // `dis` es otra forma de desv
    // Solo el dado afectado: el último de ese tipo, y sin tipo, el último del texto
    expect(f("1d20 + 1d6", "adv", "20")).toBe("1d20adv + 1d6");
    expect(f("1d20 + 1d6", "adv", "6")).toBe("1d20 + 1d6adv");
    expect(f("1d20 + 1d6", "adv", null)).toBe("1d20 + 1d6adv");
    expect(f("1d20 + 1d6", "adv", "12")).toBe("1d20 + 1d6adv"); // ese dado ya no está: el último
    expect(f("2*3d20 + 5", "desv", null)).toBe("2*3d20desv + 5");
    // Sin dados, o con algo que se le parece, no hace nada
    expect(f("", "adv", null)).toBe("");
    expect(f("5 + f", "adv", null)).toBe("5 + f");
    expect(f("1d20adx", "adv", null)).toBe("1d20adx");
  });

  it("con una comparación solo mira la tirada, no el objetivo", async () => {
    const { aplicarModificadorAlDado: f, modificadorDelDado: m, aplicarDadoAlComando: g } = await nuevoStore();
    expect(f("1d20 >= 1d20", "adv", "20")).toBe("1d20adv >= 1d20");
    expect(f("1d20 >=", "adv", null)).toBe("1d20adv >= ");
    expect(m("1d20desv >= 1d20adv", "20")).toBe("desv");
    expect(m("1d20", null)).toBeNull();
    expect(m("1d20adv", null)).toBe("adv");
    // Un dado nuevo entra en la tirada, no detrás del operador
    expect(g("1d20 >= ", "1d6", true)).toBe("1d20 + 1d6 >= ");
    expect(g("1d20 >= 12", "1d20", true)).toBe("2d20 >= 12");
    // Quitar el último dado deja el chat vacío: una comparación sin tirada no sirve
    expect(g("1d20 >= 12", "1d20", false)).toBe("");
    // Mal escrito (dos comparaciones): no se toca
    expect(f("1d20 >= 5 <= 6", "adv", null)).toBe("1d20 >= 5 <= 6");
    expect(g("1d20 >= 5 <= 6", "1d6", true)).toBe("1d20 >= 5 <= 6");
  });
});

describe("barra de dados: comparar", () => {
  it("dividirComparacion reconoce los cuatro operadores y vs (igual que <=)", async () => {
    const { dividirComparacion: d } = await nuevoStore();
    expect(d("1d20 >= 12")).toEqual({ izq: "1d20 ", texto: ">=", op: ">=", der: " 12" });
    expect(d("1d20>12")?.op).toBe(">");
    expect(d("1d100 < 45")?.op).toBe("<");
    expect(d("1d100 <= acro")?.der).toBe(" acro");
    expect(d("1d100 VS acro")).toEqual({ izq: "1d100 ", texto: "VS", op: "<=", der: " acro" });
    expect(d("1d20 + avs")).toBeNull(); // vs pegado a letras es un nombre
    expect(d("1d20")).toBeNull();
    expect(d("1d20 <= 3 >= 1")).toBeNull(); // dos: no se toca
  });

  it("pulsar <= o >= la escribe, la otra la sustituye y la misma la quita", async () => {
    const { ponerComparacion: p } = await nuevoStore();
    expect(p("5d20adv", ">=")).toBe("5d20adv >= ");
    expect(p("5d20adv >= ", "<=")).toBe("5d20adv <= ");
    expect(p("5d20adv >= 12", "<=")).toBe("5d20adv <= 12"); // conserva el objetivo escrito
    expect(p("5d20adv >= 12", ">=")).toBe("5d20adv"); // la misma otra vez: fuera, y su objetivo con ella
    expect(p("1d20 > 12", ">=")).toBe("1d20 >= 12"); // un estricto escrito a mano se cambia
    expect(p("1d100 vs 45", "<=")).toBe("1d100 <= 45");
    expect(p("1d100 vs 45", ">=")).toBe("1d100 >= 45");
    expect(p("", ">=")).toBe(""); // antes hace falta un dado
    expect(p("1d20 >= 5 <= 6", "<=")).toBe("1d20 >= 5 <= 6");
  });

  it("al tirar, el bonificador va a la tirada (izquierda) y el objetivo de la barra a la derecha solo si está vacía", async () => {
    const { comandoParaTirar: c } = await nuevoStore();
    expect(c("1d20 >= ", 3, 12)).toBe("1d20 + 3 >= 12");
    expect(c("1d20 >=", "-f", "")).toBe("1d20 - f >=");
    expect(c("1d20 >= 15", 3, 12)).toBe("1d20 + 3 >= 15"); // lo escrito a mano gana
    expect(c("1d100 vs ", "", "acro")).toBe("1d100 vs acro");
    expect(c("1d20 <", 0, 5)).toBe("1d20 < 5");
    // Sin comparación, el objetivo no se usa
    expect(c("1d20", 3, 12)).toBe("1d20 + 3");
    expect(c("2d6", "", 12)).toBe("2d6");
  });

  it("el store lleva el último dado, marca los botones y manda el comando entero a Rust", async () => {
    const { tiradas } = await nuevoStore();
    expect(tiradas.modificadorActivo).toBeNull();
    for (let i = 0; i < 5; i++) tiradas.sumarDado("1d20");
    tiradas.sumarDado("1d6");
    expect(tiradas.comando).toBe("5d20 + 1d6");
    tiradas.sumarDado("1d20"); // el último pulsado vuelve a ser el d20, aunque el d6 esté detrás
    tiradas.alternarModificador("adv");
    expect(tiradas.comando).toBe("6d20adv + 1d6");
    tiradas.sumarDado("1d20"); // otro d20: va aparte, sin ventaja
    expect(tiradas.comando).toBe("6d20adv + 1d6 + 1d20");
    expect(tiradas.modificadorActivo).toBeNull(); // el último grupo de d20 no la tiene
    tiradas.alternarModificador("adv"); // ahora sí: se junta con el que ya existe
    expect(tiradas.comando).toBe("7d20adv + 1d6");
    expect(tiradas.modificadorActivo).toBe("adv");
    tiradas.alternarModificador("desv");
    expect(tiradas.modificadorActivo).toBe("desv");
    tiradas.alternarComparacion(">=");
    expect(tiradas.comando).toBe("7d20desv + 1d6 >= ");
    expect(tiradas.comparacionActiva).toBe(">=");
    tiradas.alternarComparacion("<=");
    expect(tiradas.comparacionActiva).toBe("<=");

    llamadas.length = 0;
    await tiradas.tirarConBonificador("3", "12");
    expect(llamadas.find((l) => l.cmd === "tirar_dado")?.args).toMatchObject({ comando: "7d20desv + 1d6 + 3 <= 12" });
    // Tras tirar, el chat y el dado afectado se limpian
    expect(tiradas.comando).toBe("");
    tiradas.sumarDado("1d8");
    tiradas.alternarModificador("adv");
    expect(tiradas.comando).toBe("1d8adv");
    tiradas.comando = "1d20 vs 12"; // escrito a mano también se marca
    expect(tiradas.comparacionActiva).toBe("<=");
  });
});
