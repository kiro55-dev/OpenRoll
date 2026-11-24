<!-- src/routes/componentes/VentanaMacros.svelte -->
<!-- «Macros…» (⚙, host y jugadores): ventana flotante no modal con MIS macros. A la izquierda la lista (con su tipo: ONCLICK /
     ONTURN), a la derecha el editor: nombre, código (se comprueba en Rust mientras se escribe y el error dice la línea) y Guardar /
     Borrar. Abajo, plegada, la referencia del lenguaje. Asociarlas a una ficha se hace desde la caja «Parámetros» de la derecha. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { moverVentana } from "$lib/moverVentana";
  import { redimensionable } from "$lib/redimensionarVentana";
  import { esJugador } from "$lib/modo";
  import { untrack } from "svelte";
  import { macros, macrosEditables, type Comprobacion } from "$lib/macros.svelte";

  let { onCerrar }: { onCerrar: () => void } = $props();

  /** La ficha a la que se asocia (abierta desde su caja «Parámetros»), si la hay. */
  const entidad = $derived(macros.ventana?.entidad);

  let ventana = $state<HTMLDivElement | null>(null);
  let pos = $state({ x: 280, y: 90 });

  const EJEMPLO = "ONTURN\nDO CHANGE @vida -1d4 IF EXECUTE 1d20 <= .cont ELSE QUIT";

  const mias = $derived(macrosEditables(macros.lista, esJugador));
  let busqueda = $state("");
  const visibles = $derived(mias.filter((m) => m.nombre.toLowerCase().includes(busqueda.trim().toLowerCase())));
  /** La asociación de una macro con la ficha de la ventana (para la marca ✓ y el ✕). */
  const deLaFicha = (idMacro: number) =>
    entidad ? macros.asociaciones.find((a) => a.id_macro === idMacro && "Ficha" in a.destino && a.destino.Ficha === entidad.id) : undefined;
  /** La que se edita (0 = una nueva). */
  let idActual = $state(0);
  let nombre = $state("");
  let codigo = $state("");
  let comprobacion = $state<Comprobacion | null>(null);
  let errorCodigo = $state("");
  let error = $state("");
  let guardando = $state(false);
  let confirmarBorrado = $state(false);
  let ayudaAbierta = $state(false);

  const guardada = $derived(mias.find((m) => m.id === idActual));
  const cambiada = $derived(!guardada ? nombre.trim() !== "" || codigo.trim() !== "" : guardada.nombre !== nombre.trim() || guardada.codigo !== codigo);

  $effect(() => macros.iniciar());

  // Al abrirla (o volver a abrirla desde otra macro o ficha) carga la que se pidió
  $effect(() => {
    const v = macros.ventana;
    untrack(() => {
      busqueda = "";
      abrir(v?.idMacro ?? 0);
    });
  });

  function abrir(id: number) {
    const m = mias.find((x) => x.id === id);
    idActual = m?.id ?? 0;
    nombre = m?.nombre ?? "";
    codigo = m?.codigo ?? "";
    error = "";
    confirmarBorrado = false;
  }

  // Se comprueba el código en Rust al escribir (un poco después de la última tecla)
  let temporizador: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const texto = codigo;
    clearTimeout(temporizador);
    if (!texto.trim()) {
      comprobacion = null;
      errorCodigo = "";
      return;
    }
    temporizador = setTimeout(async () => {
      try {
        comprobacion = await macros.validar(texto);
        errorCodigo = "";
      } catch (err) {
        comprobacion = null;
        errorCodigo = String(err);
      }
    }, 300);
    return () => clearTimeout(temporizador);
  });

  /** Un clic en la lista: la abre en el editor y, si se abrió desde una ficha, se la asocia (si aún no la tiene). */
  async function elegir(id: number) {
    abrir(id);
    if (entidad && !deLaFicha(id)) {
      try {
        await macros.asociar(id, entidad.id);
      } catch (err) {
        error = String(err);
      }
    }
  }

  async function quitarDeLaFicha(e: MouseEvent, idAsociacion: number) {
    e.stopPropagation();
    try {
      await macros.desasociar(idAsociacion);
    } catch (err) {
      error = String(err);
    }
  }

  async function guardar() {
    if (guardando) return;
    guardando = true;
    error = "";
    try {
      const eraNueva = idActual === 0;
      const m = await macros.guardar({ id: idActual, nombre, codigo });
      // Una macro creada desde una ficha queda asociada a ella
      if (eraNueva && entidad) await macros.asociar(m.id, entidad.id);
      abrir(m.id);
    } catch (err) {
      error = String(err);
    } finally {
      guardando = false;
    }
  }

  async function borrar() {
    if (!guardada) return;
    try {
      await macros.borrar(guardada.id);
      abrir(0);
    } catch (err) {
      error = String(err);
    }
  }

  function alTeclear(e: KeyboardEvent) {
    if (e.key === "Escape") onCerrar();
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void guardar();
    }
  }
</script>

<div
  class="ventana-flotante"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  use:redimensionable={{ pos }}
  role="dialog"
  aria-label="Macros"
  tabindex="-1"
  onkeydown={alTeclear}
  bind:this={ventana}
>
  <div class="cabecera" role="presentation" onpointerdown={(e) => moverVentana(e, pos, ventana)}>
    <span class="icono-drag">⋮⋮</span>
    <span class="titulo">{entidad ? `Macros de ${entidad.nombre}` : "Macros"}</span>
    <button type="button" class="btn-cerrar" aria-label="Cerrar" onclick={onCerrar}>✕</button>
  </div>

  <div class="cuerpo">
    <div class="lista">
      <input type="text" class="buscar" placeholder="Buscar macro..." bind:value={busqueda} />
      <button type="button" class="item nueva" class:activa={idActual === 0} onclick={() => abrir(0)}>+ Nueva macro</button>
      {#each visibles as m (m.id)}
        {@const aso = deLaFicha(m.id)}
        <button
          type="button"
          class="item"
          class:activa={idActual === m.id}
          class:asociada={!!aso}
          title={ayuda(entidad ? (aso ? `Asociada a ${entidad.nombre}` : `Pulsa para asociarla a ${entidad.nombre} y abrirla`) : "Pulsa para abrirla")}
          onclick={() => elegir(m.id)}
        >
          <span class="nombre">{aso ? "✓ " : ""}{m.nombre}</span>
          <span class="tipos">
            {#if m.al_pulsar}<span class="tipo">CLICK</span>{/if}
            {#if m.en_turno}<span class="tipo turno">TURNO</span>{/if}
            {#if aso}
              <span class="quitar" role="button" tabindex="-1" aria-label="Quitar de la ficha" title="Quitar de {entidad?.nombre}" onclick={(e) => quitarDeLaFicha(e, aso.id)} onkeydown={() => {}}>✕</span>
            {/if}
          </span>
        </button>
      {:else}
        <span class="vacio">{mias.length ? "Ninguna coincide" : "Aún no hay ninguna"}</span>
      {/each}
    </div>

    <div class="editor">
      <label class="campo">
        <span>Nombre</span>
        <input type="text" maxlength="40" bind:value={nombre} placeholder="Envenenado" />
      </label>
      <textarea
        class="codigo"
        spellcheck="false"
        bind:value={codigo}
        placeholder={EJEMPLO}
        title={ayuda("Empieza con ONCLICK (botón en la caja «Parámetros»), ONTURN (sola cuando le llega el turno) o los dos")}
      ></textarea>
      <div class="estado">
        {#if errorCodigo}
          <span class="mal">{errorCodigo}</span>
        {:else if comprobacion}
          <span class="bien">
            Correcta ·
            {comprobacion.al_pulsar && comprobacion.en_turno
              ? "botón y al empezar su turno"
              : comprobacion.al_pulsar
                ? "botón en la caja «Parámetros»"
                : "sola al empezar su turno"}
          </span>
        {/if}
      </div>
      {#if error}<p class="mal">{error}</p>{/if}
      <div class="botones">
        {#if guardada}
          {#if confirmarBorrado}
            <span class="mal">¿Borrarla? Se quita de todas las fichas.</span>
            <button type="button" class="btn peligro" onclick={borrar}>Borrar</button>
            <button type="button" class="btn" onclick={() => (confirmarBorrado = false)}>Cancelar</button>
          {:else}
            <button type="button" class="btn" onclick={() => (confirmarBorrado = true)}>Borrar</button>
          {/if}
        {/if}
        <button type="button" class="btn guardar" disabled={guardando || !cambiada || !!errorCodigo} title={ayuda("Atajo: Ctrl + S")} onclick={guardar}>
          Guardar
        </button>
      </div>
    </div>
  </div>

  <details class="referencia" bind:open={ayudaAbierta}>
    <summary>Cómo se escriben</summary>
    <div class="texto">
      <p><b>Cabecera</b> (primera línea): <code>ONCLICK</code> (botón), <code>ONTURN</code> (sola al llegarle el turno en batalla) o las dos.</p>
      <p><b>Argumentos</b>: números, dados (<code>2d6</code>, <code>1d20adv</code>), <code>.variable</code>, <code>@parámetro</code> (<code>@vida.max</code> el máximo), un atributo guardado con SAVE, <code>TURN</code> (la vuelta), con <code>+ - ( )</code>. Los nombres van juntos: <code>@rangoarma</code>.</p>
      <p><b>Condiciones</b>: <code>&lt; &lt;= = != &gt;= &gt;</code> con <code>AND</code> / <code>OR</code>; con <code>EXECUTE</code> delante sale en el chat.</p>
      <p><b>Sentencias</b> (una por línea, o <code>[ … ]</code> para varias):</p>
      <ul>
        <li><code>EXECUTE x</code> / <code>EXECUTEP x</code>: al chat (pública / privada).</li>
        <li><code>SAVE nombre x</code>: guarda un atributo.</li>
        <li><code>CHANGE @vida -1d4</code>: con signo suma; sin signo pone el valor (solo parámetros con máximo).</li>
        <li><code>DO … IF … ELSE …</code> · <code>QUIT</code>: quita la macro de la ficha.</li>
        <li><code>SPREAD acción TO condición</code>: a cada entidad del mapa que cumpla (<code>ENTITIES@armadura</code>, <code>ENTITIES.fue</code>, <code>ENTITIES IN @rangoarma</code>, <code>DISTANCE</code>). Acción: <code>EXECUTE</code>, <code>CHANGE ENTITIES@vida …</code> o <code>MACROS.veneno, MACROS.lento</code>.</li>
      </ul>
      <pre>ONCLICK
SPREAD CHANGE ENTITIES@vida -(2d8 + .fue) TO ENTITIES@armadura &lt;= 1d20 + .des AND ENTITIES IN @rangoarma</pre>
    </div>
  </details>
</div>

<style>
  .ventana-flotante {
    position: fixed; z-index: 1000; width: 560px; max-height: 80vh;
    display: flex; flex-direction: column; overflow: hidden;
    background: #18181b; border: 1px solid #3f3f46; border-radius: 8px;
    box-shadow: 0 14px 35px rgba(0, 0, 0, 0.75);
    font-size: 12px; color: #f4f4f5;
  }
  .cabecera {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px; cursor: grab; user-select: none;
    background: #27272a; border-bottom: 1px solid #3f3f46; font-size: 11px;
  }
  .cabecera:active { cursor: grabbing; }
  .icono-drag { color: #71717a; font-size: 12px; }
  .titulo { font-weight: bold; flex: 1; }
  .btn-cerrar { background: none; border: none; color: #a1a1aa; cursor: pointer; font-size: 13px; }
  .btn-cerrar:hover { color: white; }

  .cuerpo { display: flex; gap: 10px; padding: 10px; min-height: 0; }
  .lista { width: 160px; flex: none; display: flex; flex-direction: column; gap: 3px; overflow-y: auto; max-height: 320px; }
  .item {
    display: flex; align-items: center; justify-content: space-between; gap: 4px; text-align: left;
    background: #1f1f23; border: 1px solid #3f3f46; border-radius: 4px; color: inherit; font: inherit; padding: 4px 6px; cursor: pointer;
  }
  .item:hover { background: #27272a; }
  .item.activa { border-color: #3b82f6; background: #2a2a32; }
  .item.nueva { border-style: dashed; color: #a1a1aa; }
  .item.asociada { border-color: #16a34a; }
  .buscar { background: #09090b; border: 1px solid #3f3f46; color: white; padding: 3px 5px; border-radius: 4px; font-size: 11px; }
  .buscar:focus { outline: none; border-color: #60a5fa; }
  .vacio { color: #71717a; font-size: 11px; padding: 4px; }
  .quitar { color: #a1a1aa; padding: 0 2px; cursor: pointer; font-size: 10px; }
  .quitar:hover { color: #f87171; }
  .nombre { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tipos { display: flex; gap: 2px; flex: none; }
  .tipo { font-size: 8px; padding: 1px 3px; border-radius: 3px; background: #1e3a8a; color: #bfdbfe; }
  .tipo.turno { background: #713f12; color: #fde68a; }

  .editor { flex: 1; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .campo { display: flex; align-items: center; gap: 6px; }
  .campo input {
    flex: 1; background: #09090b; border: 1px solid #3f3f46; border-radius: 4px; color: white; padding: 4px 6px; font: inherit;
  }
  .codigo {
    height: 210px; resize: vertical; background: #09090b; border: 1px solid #3f3f46; border-radius: 4px; color: #e4e4e7;
    padding: 6px; font-family: ui-monospace, Consolas, monospace; font-size: 12px; line-height: 1.4; white-space: pre; tab-size: 2;
  }
  .codigo:focus, .campo input:focus { outline: none; border-color: #60a5fa; }
  .estado { min-height: 16px; font-size: 11px; }
  .bien { color: #4ade80; }
  .mal { color: #f87171; font-size: 11px; margin: 0; }
  .botones { display: flex; align-items: center; justify-content: flex-end; gap: 6px; flex-wrap: wrap; }
  .btn { background: #27272a; border: 1px solid #3f3f46; border-radius: 4px; color: #f4f4f5; cursor: pointer; padding: 4px 12px; font-size: 11px; }
  .btn:hover:not(:disabled) { background: #3f3f46; }
  .btn:disabled { opacity: 0.45; cursor: default; }
  .btn.guardar { background: #166534; border-color: #16a34a; }
  .btn.peligro { background: #7f1d1d; border-color: #dc2626; }

  .referencia { border-top: 1px solid #27272a; padding: 6px 10px; overflow-y: auto; }
  .referencia summary { cursor: pointer; color: #a1a1aa; font-size: 11px; user-select: none; }
  .texto { font-size: 11px; color: #d4d4d8; line-height: 1.45; }
  .texto p { margin: 6px 0; }
  .texto ul { margin: 0; padding-left: 16px; }
  .texto code, .texto pre { font-family: ui-monospace, Consolas, monospace; color: #93c5fd; }
  .texto pre { background: #09090b; padding: 6px; border-radius: 4px; white-space: pre-wrap; margin: 6px 0 0; }
</style>
