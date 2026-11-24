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
  import { CODIGO_NUEVA_MACRO, macros, macrosEditables, nombreNuevaMacro, type Comprobacion } from "$lib/macros.svelte";
  import { partida } from "$lib/partida.svelte";

  let { onCerrar }: { onCerrar: () => void } = $props();

  /** La ficha a la que se asocia (abierta desde su caja «Parámetros»), si la hay. */
  const entidad = $derived(macros.ventana?.entidad);

  let ventana = $state<HTMLDivElement | null>(null);
  let pos = $state({ x: 280, y: 90 });

  const EJEMPLO = "ONTURN\nDO CHANGE @vida -1d4 IFNOT EXECUTE 1d20 + .con >= 14\nELSE QUIT";

  const mias = $derived(macrosEditables(macros.lista, esJugador));
  let busqueda = $state("");
  const visibles = $derived(mias.filter((m) => m.nombre.toLowerCase().includes(busqueda.trim().toLowerCase())));
  /** La asociación de una macro con la ficha de la ventana (para la marca ✓ y el ✕). */
  const deLaFicha = (idMacro: number) =>
    entidad ? macros.asociaciones.find((a) => a.id_macro === idMacro && "Ficha" in a.destino && a.destino.Ficha === entidad.id) : undefined;
  /** La que se edita (0 = ninguna: primero se crea con «+ Nueva macro»). */
  let idActual = $state(0);
  let nombre = $state("");
  let codigo = $state("");
  let nombreAplicado = $state("");
  let comprobacion = $state<Comprobacion | null>(null);
  let errorCodigo = $state("");
  let error = $state("");
  let guardando = $state(false);
  let confirmarBorrado = $state(false);
  let ayudaAbierta = $state(false);

  const guardada = $derived(mias.find((m) => m.id === idActual));
  /** El texto escrito difiere del aplicado: queda guardado como borrador hasta pulsar «Guardar». */
  const sinAplicar = $derived(!!guardada && guardada.codigo !== codigo);

  $effect(() => macros.iniciar());
  macros.cargarBorradores();

  // Todo lo que se escribe se guarda como borrador al momento (aunque no se pulse «Guardar»)
  $effect(() => {
    const texto = codigo;
    const id = idActual;
    untrack(() => {
      const g = mias.find((m) => m.id === id);
      if (g) macros.ponerBorrador(id, g.codigo === texto ? undefined : texto);
    });
  });

  // Con la ventana abierta, pulsar UNA ficha (en «Fichas activas» o en el tablero) la pone como la ficha de la ventana. Solo cuando
  // la selección cambia (no al abrirla, ni al deseleccionar: la ventana se queda con la última) y solo si se controla.
  let ultimaSeleccion: number | null = untrack(() => partida.instanciaSeleccionada?.id_instancia ?? null);
  $effect(() => {
    const sel = partida.instanciaSeleccionada;
    const id = sel?.id_instancia ?? null;
    untrack(() => {
      if (id === ultimaSeleccion) return;
      ultimaSeleccion = id;
      const v = macros.ventana;
      if (!sel || !v) return;
      const entidad = partida.controla(sel) ? { id: sel.base.id, nombre: sel.base.nombre } : undefined;
      if (v.entidad?.id !== entidad?.id) macros.ventana = { ...v, entidad };
    });
  });

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
    nombreAplicado = nombre;
    // Si había texto a medias, se retoma donde se dejó
    codigo = m ? (macros.borradores[m.id] ?? m.codigo) : "";
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

  /** La casilla ✓ de una macro: la asocia a la ficha de la ventana o, si ya la tenía, se la quita. Pulsar el nombre solo la abre. */
  async function alternarAsociacion(id: number) {
    if (!entidad) return;
    const aso = deLaFicha(id);
    error = "";
    try {
      if (aso) await macros.desasociar(aso.id);
      else await macros.asociar(id, entidad.id);
    } catch (err) {
      error = String(err);
    }
  }

  /** «+ Nueva macro»: se crea ya, con un nombre («Macro 1»...) y un código mínimo, y se abre para editarla. Desde una ficha, queda asociada a ella. */
  async function crear() {
    if (guardando) return;
    guardando = true;
    error = "";
    try {
      const m = await macros.guardar({ id: 0, nombre: nombreNuevaMacro(mias.map((x) => x.nombre)), codigo: CODIGO_NUEVA_MACRO });
      if (entidad) await macros.asociar(m.id, entidad.id);
      busqueda = "";
      abrir(m.id);
    } catch (err) {
      error = String(err);
    } finally {
      guardando = false;
    }
  }

  /** El nombre se aplica al salir del campo (o con Enter): no hace falta «Guardar». Un nombre que no vale vuelve al anterior. */
  async function aplicarNombre() {
    const g = guardada;
    const nuevo = nombre.trim();
    if (!g || nuevo === g.nombre) {
      if (g) nombre = g.nombre;
      return;
    }
    error = "";
    try {
      await macros.guardar({ id: g.id, nombre: nuevo, codigo: g.codigo });
      nombreAplicado = nuevo;
    } catch (err) {
      error = String(err);
      nombre = g.nombre;
    }
  }

  /** «Guardar» aplica el texto de la macro (se comprueba en Rust: si tiene un error no se aplica y el borrador sigue ahí). */
  async function guardar() {
    const g = guardada;
    if (guardando || !g || !sinAplicar) return;
    guardando = true;
    error = "";
    try {
      await macros.guardar({ id: g.id, nombre: g.nombre, codigo });
      macros.ponerBorrador(g.id, undefined);
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
  data-conserva-foco
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
      <button type="button" class="item nueva" disabled={guardando} onclick={crear}>+ Nueva macro</button>
      {#each visibles as m (m.id)}
        {@const aso = deLaFicha(m.id)}
        <div class="fila" class:activa={idActual === m.id} class:asociada={!!aso}>
          {#if entidad}
            <!-- Asociar a la ficha es esta casilla, aparte: pulsar el nombre solo abre la macro para editarla -->
            <button
              type="button"
              class="marca"
              class:puesta={!!aso}
              aria-pressed={!!aso}
              aria-label={aso ? `Quitar «${m.nombre}» de ${entidad.nombre}` : `Asociar «${m.nombre}» a ${entidad.nombre}`}
              title={ayuda(aso ? `Asociada a ${entidad.nombre}. Pulsa para quitársela` : `Pulsa para asociarla a ${entidad.nombre}`)}
              onclick={() => alternarAsociacion(m.id)}
            >{aso ? "✓" : ""}</button>
          {/if}
          <button type="button" class="item" title={ayuda("Pulsa para abrirla y editarla")} onclick={() => abrir(m.id)}>
            <span class="nombre">{m.nombre}</span>
            <span class="tipos">
              {#if m.al_pulsar}<span class="tipo">CLICK</span>{/if}
              {#if m.en_turno}<span class="tipo turno">TURNO</span>{/if}
            </span>
          </button>
        </div>
      {:else}
        <span class="vacio">{mias.length ? "Ninguna coincide" : "Aún no hay ninguna"}</span>
      {/each}
    </div>

    {#if !guardada}
      <div class="editor sin-macro">
        <p>{mias.length ? "Elige una macro de la lista para editarla" : "Aún no tienes ninguna macro"}, o crea una con «+ Nueva macro».</p>
        <p class="ejemplo">Por ejemplo:</p>
        <pre>{EJEMPLO}</pre>
      </div>
    {:else}
    <div class="editor">
      <label class="campo">
        <span>Nombre</span>
        <input
          type="text"
          maxlength="40"
          bind:value={nombre}
          title={ayuda("El nombre se aplica al salir del campo o con Enter")}
          onblur={aplicarNombre}
          onkeydown={(e) => {
            if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
            if (e.key === "Escape") {
              e.stopPropagation();
              nombre = nombreAplicado;
              (e.currentTarget as HTMLInputElement).blur();
            }
          }}
        />
      </label>
      <textarea
        class="codigo"
        spellcheck="false"
        bind:value={codigo}
        placeholder={EJEMPLO}
        title={ayuda("Empieza con ONCLICK (botón en la caja «Parámetros»), ONTURN (sola cuando le llega el turno) o los dos. Lo que escribes se conserva aunque no pulses Guardar")}
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
        {#if sinAplicar}<span class="aviso"> · sin aplicar (queda guardado como borrador)</span>{/if}
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
        <button type="button" class="btn guardar" disabled={guardando || !sinAplicar || !!errorCodigo} title={ayuda("Aplica el texto a la macro (Atajo: Ctrl + S)")} onclick={guardar}>
          Guardar
        </button>
      </div>
    </div>
    {/if}
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
        <li><code>DO … IF … ELSE …</code> · <code>DO … IFNOT … ELSE …</code> (el cuerpo es para cuando NO se cumple) · <code>QUIT</code>: quita la macro de la ficha.</li>
        <li><code>SPREAD acción TO condición</code>: a cada entidad del mapa que cumpla (<code>ENTITIES@armadura</code>, <code>ENTITIES.fue</code>, <code>ENTITIES IN @rangoarma</code>, <code>DISTANCE</code>). Acción: <code>EXECUTE</code>, <code>CHANGE ENTITIES@vida …</code>, <code>MACROS.veneno, MACROS.lento</code> o un bloque <code>[ … ]</code> que se hace en cada una.</li>
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
  .item.nueva { border-style: dashed; color: #a1a1aa; }
  .fila { display: flex; align-items: stretch; gap: 3px; }
  .fila .item { flex: 1; min-width: 0; }
  .fila.activa .item { border-color: #3b82f6; background: #2a2a32; }
  .fila.asociada .item { border-color: #16a34a; }
  .fila.activa.asociada .item { border-color: #3b82f6; }
  .marca {
    flex: none; width: 20px; background: #1f1f23; border: 1px solid #3f3f46; border-radius: 4px; color: #4ade80; font-size: 12px; cursor: pointer; padding: 0;
  }
  .marca:hover { border-color: #60a5fa; }
  .marca.puesta { background: #14532d; border-color: #16a34a; }
  .item:disabled { opacity: 0.5; cursor: default; }
  .sin-macro { color: #a1a1aa; font-size: 12px; }
  .sin-macro p { margin: 0 0 6px; }
  .sin-macro pre { margin: 0; background: #09090b; border-radius: 4px; padding: 6px; color: #93c5fd; font-family: ui-monospace, Consolas, monospace; white-space: pre-wrap; }
  .aviso { color: #fbbf24; }
  .buscar { background: #09090b; border: 1px solid #3f3f46; color: white; padding: 3px 5px; border-radius: 4px; font-size: 11px; }
  .buscar:focus { outline: none; border-color: #60a5fa; }
  .vacio { color: #71717a; font-size: 11px; padding: 4px; }

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
