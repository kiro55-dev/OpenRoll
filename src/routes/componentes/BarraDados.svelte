<!-- src/routes/componentes/BarraDados.svelte -->
<!-- La barra de dados. Los botones de dado suman o restan en el chat (rueda: cantidad). «Vent» y «Desv» son botones que se aplican al
     ÚLTIMO dado que has puesto en el chat (5d20 → 5d20adv; otra vez lo quita) y quedan marcados mientras valga. «Comparar» tiene
     dos botones, `<=` y `>=`, que escriben la comparación en el chat tal cual (solo hay una: la nueva sustituye a la anterior), y un
     campo «Objetivo» que se añade al tirar. El bonificador va en la parte de los dados (la izquierda de la comparación). Toda la
     lógica del texto está en `tiradas.svelte.ts` (funciones puras y con pruebas); aquí solo se pinta. -->
<script>
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { tiradas } from "$lib/tiradas.svelte";
  let { onSumarAlChat, onRestarDelChat, onLanzarTirada } = $props();

  // Un número o texto del chat (f, -fuerza...): se pega a la tirada al tirar
  let bonificador = $state("");
  // Con qué se compara la tirada (un número o una variable: 12, acro...): se pega a la derecha de la comparación al tirar
  let objetivo = $state("");

  let dados = $state([
    { tipo: "d4", cantidad: 1, color: "#22c55e" },
    { tipo: "d6", cantidad: 1, color: "#06b6d4" },
    { tipo: "d8", cantidad: 1, color: "#8b5cf6" },
    { tipo: "d10", cantidad: 1, color: "#ec4899" },
    { tipo: "d12", cantidad: 1, color: "#f97316" },
    { tipo: "d20", cantidad: 1, color: "#eab308" }
  ]);

  function rueda(e, d) {
    e.preventDefault();
    if (e.deltaY < 0) d.cantidad++;
    else if (d.cantidad > 1) d.cantidad--;
  }

  // Sin modificador: Vent y Desv se aplican después, al último dado del chat
  const formatearDado = (d) => `${d.cantidad}${d.tipo}`;

  const lanzar = () => onLanzarTirada(bonificador, objetivo);
</script>

<!-- data-conserva-foco: pulsar aquí no quita el foco a la ficha (si no, no se podría tirar su bonificador) -->
<div class="barra-dados" data-conserva-foco>
  <div class="zona-dados">
    {#each dados as d}
      <button
        class="boton-dado"
        style="border-color: {d.color};"
        onwheel={(e) => rueda(e, d)}
        onclick={() => onSumarAlChat(formatearDado(d))}
        oncontextmenu={(e) => { e.preventDefault(); onRestarDelChat(formatearDado(d)); }}
        title={ayuda("Rueda: cantidad\nClic: sumar\nClic derecho: restar")}
      >
        <span class="cant">{d.cantidad}x</span>
        <span class="label">{d.tipo}</span>
      </button>
    {/each}
  </div>

  <!-- VENTAJA / DESVENTAJA: dos botones, uno encima del otro, que se aplican al último dado del chat -->
  <div
    class="grupo"
    role="group"
    aria-label="Ventaja o desventaja del último dado"
    title={ayuda("Se aplican al último dado que has puesto en el chat (por ejemplo 5d20 → 5d20adv). Pulsar otra vez lo quita; si ya hay otro grupo igual del mismo dado, se juntan")}
  >
    <button
      type="button"
      class="btn-mod"
      class:activo={tiradas.modificadorActivo === "adv"}
      aria-pressed={tiradas.modificadorActivo === "adv"}
      title={ayuda("Ventaja: tira 2 veces cada dado del último grupo y se queda con el mayor")}
      onclick={() => tiradas.alternarModificador("adv")}
    >Vent</button>
    <button
      type="button"
      class="btn-mod"
      class:activo={tiradas.modificadorActivo === "desv"}
      aria-pressed={tiradas.modificadorActivo === "desv"}
      title={ayuda("Desventaja: tira 2 veces cada dado del último grupo y se queda con el menor")}
      onclick={() => tiradas.alternarModificador("desv")}
    >Desv</button>
  </div>

  <!-- COMPARAR: los dos botones encima del objetivo; la comparación se escribe en el chat tal cual y el objetivo se añade al tirar -->
  <div
    class="grupo"
    role="group"
    aria-label="Comparar la tirada con un objetivo"
    title={ayuda("Compara la tirada con un objetivo: 1d20 >= 12 es «igualar o superar 12», 1d100 <= 45 es «igualar o quedar por debajo de 45». También se puede escribir a mano en el chat (<, >, <=, >= o vs). El host puede darle nombre al resultado en Parámetros > Resultados")}
  >
    <div class="fila-grupo">
      <button
        type="button"
        class="btn-mod"
        class:activo={tiradas.comparacionActiva === "<="}
        aria-pressed={tiradas.comparacionActiva === "<="}
        title={ayuda("Escribe «<=» en el chat: acierta si la tirada iguala o queda por debajo del objetivo (Cthulhu). Pulsa otra vez para quitarlo")}
        onclick={() => tiradas.alternarComparacion("<=")}
      >&lt;=</button>
      <button
        type="button"
        class="btn-mod"
        class:activo={tiradas.comparacionActiva === ">="}
        aria-pressed={tiradas.comparacionActiva === ">="}
        title={ayuda("Escribe «>=» en el chat: acierta si la tirada iguala o supera el objetivo (una CD de D&D). Pulsa otra vez para quitarlo")}
        onclick={() => tiradas.alternarComparacion(">=")}
      >&gt;=</button>
    </div>
    <input
      type="text"
      class="objetivo"
      placeholder="Objetivo"
      aria-label="Objetivo de la comparación"
      title={ayuda("Con qué se compara: un número o una variable (12, acro...). Se añade a la derecha de la comparación al tirar; si ya has escrito uno en el chat, vale ese")}
      bind:value={objetivo}
      onkeydown={(e) => e.key === "Enter" && lanzar()}
    />
  </div>

  <div class="bonif">
    <span>Bonf:</span>
    <input
      type="text"
      placeholder=""
      title={ayuda("Un número o una variable (f, -fuerza...): se suma a la tirada (a la izquierda de la comparación) como si lo escribieras en el chat")}
      bind:value={bonificador}
      onkeydown={(e) => e.key === "Enter" && lanzar()}
    />
  </div>

  <button class="btn-tirar" onclick={lanzar}>
    TIRAR
  </button>
</div>

<style>
  .barra-dados {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    background: #18181b;
    border: 1px solid #3f3f46;
    padding: 6px 12px;
    border-radius: 8px;
  }
  .zona-dados { display: flex; gap: 5px; }
  .boton-dado {
    background: #27272a;
    color: white;
    border: 2px solid;
    padding: 3px 6px;
    border-radius: 5px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    font-size: 11px;
    font-weight: bold;
    user-select: none;
  }
  .cant { font-size: 9px; color: #a1a1aa; }

  /* Un grupo de botones, uno encima del otro (como eran los checks de antes) */
  .grupo {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 2px;
    border-left: 1px solid #3f3f46;
    padding-left: 8px;
  }
  .fila-grupo { display: flex; align-items: center; gap: 4px; }
  .fila-grupo .btn-mod { flex: 1; }
  .btn-mod {
    background: #27272a;
    color: #e4e4e7;
    border: 1px solid #3f3f46;
    border-radius: 4px;
    padding: 0 7px; /* poco alto: el borde pegado al texto */
    font-size: 11px;
    line-height: 1.35;
    font-weight: bold;
    cursor: pointer;
    user-select: none;
  }
  .btn-mod:hover { border-color: #71717a; }
  /* Marcado: el modificador o la comparación está puesto ahora en el chat */
  .btn-mod.activo { background: #eab308; border-color: #eab308; color: #18181b; }
  .objetivo {
    width: 68px;
    box-sizing: border-box;
    background: #09090b;
    border: 1px solid #3f3f46;
    color: white;
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 11px;
  }

  .bonif { display: flex; align-items: center; gap: 4px; font-size: 11px; border-left: 1px solid #3f3f46; padding-left: 8px; }
  /* Un tercio de lo que era (64px): basta para un número o una letra */
  .bonif input { width: 45px; box-sizing: border-box; background: #09090b; border: 1px solid #3f3f46; color: white; padding: 4px 2px; border-radius: 3px; text-align: center; }
  .btn-tirar {
    background: #00A2E8; color: white; border: none; font-weight: bold; padding: 6px 14px; border-radius: 4px; cursor: pointer;
  }
</style>
