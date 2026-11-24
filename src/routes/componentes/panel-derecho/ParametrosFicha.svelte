<!-- src/routes/componentes/panel-derecho/ParametrosFicha.svelte -->
<!-- Los parámetros de la instancia seleccionada, tal y como estén definidos en la partida (en su
     orden). Los que tienen máximo muestran «actual / máx» con − y + para cambiar el actual. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, numeroDe, veParametro, PASO_VALOR_MAX, type Cuerpo } from "$lib/partida.svelte";
  import { arrastreTirada } from "$lib/arrastreTirada.svelte";
  import { parametroOculto } from "$lib/fichasTablero";
  import { macros, macrosDeInstancia, macrosEditables } from "$lib/macros.svelte";
  import { esJugador } from "$lib/modo";

  const textoCuerpo = (c: Cuerpo) => (typeof c === "object" ? `${c.Grande[0] ?? 1}×${c.Grande[1] ?? 1}` : "1×1");

  const ficha = $derived(partida.instanciaSeleccionada);

  // Las macros de la ficha: las ONCLICK como botón, las de turno como etiqueta (ver macros.svelte.ts)
  $effect(() => macros.iniciar());
  // El host no recibe avisos de lo que hace él mismo: al releer sus fichas (un turno que pasa, una macro...) relee también las macros
  $effect(() => {
    void partida.instancias;
    if (!esJugador) void macros.recargar();
  });
  const asociadas = $derived(ficha ? macrosDeInstancia(ficha, macros.asociaciones, macros.lista) : []);
  /** Abre el editor de macros para esta ficha (con `idMacro`, ya con esa macro: clic derecho sobre ella). */
  function abrirMacros(idMacro?: number) {
    if (!ficha) return;
    // Una macro ajena (sin código) no se puede editar: se abre el editor solo con la ficha
    const editable = idMacro !== undefined && macrosEditables(macros.lista, esJugador).some((m) => m.id === idMacro);
    macros.abrirVentana({ entidad: { id: ficha.base.id, nombre: ficha.base.nombre }, idMacro: editable ? idMacro : undefined });
  }

  // El parámetro cuyo paso (lo que suman o restan − y +) se está editando, con lo escrito
  // El paso es de la ENTIDAD (lo guarda Rust): lo comparten sus instancias y lo conserva una que se quite y se vuelva a poner
  let editandoPaso = $state<{ id: number; texto: number | null } | null>(null);
  const pasoDe = (id: number) => (ficha ? partida.pasoDe(ficha.base, id) : 1);

  function empezarPaso(e: MouseEvent, id: number, paso: number) {
    e.preventDefault();
    editandoPaso = { id, texto: paso };
  }

  function confirmarPaso() {
    const e = editandoPaso;
    editandoPaso = null; // antes de guardar: el blur que provoca quitar el campo no vuelve a entrar aquí
    if (e && e.texto !== null && ficha && partida.controla(ficha)) void partida.ponerPaso(ficha.base.id, e.id, e.texto);
  }

  /** Al aparecer el campo, ya con el foco y lo escrito seleccionado. */
  function enfocar(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  const parametros = $derived(
    ficha
      ? // Solo los que Rust manda: lo privado no se enseña
        partida.definiciones.filter((d) => veParametro(ficha, d.id) && !parametroOculto(ficha.base.tipo, d)).map((d) => {
          const v = ficha.base.parametros.find((p) => p.id_definicion === d.id)?.valor;
          const unidad = d.unidad ? ` ${d.unidad}` : "";
          const texto = !v ? "—" : "Numero" in v ? `${v.Numero}${unidad}` : v.Texto || "—";
          return { id: d.id, clave: d.clave, tieneMaximo: d.tiene_maximo, texto, unidad };
        })
      : []
  );
</script>

<div class="caja" data-conserva-foco>
  <span class="titulo-mini">PARÁMETROS{ficha ? ` · ${ficha.base.nombre}` : ""}</span>
  {#if partida.idsSeleccionados.length > 1}
    <div class="sin-seleccion">{partida.seleccionadas.length} fichas con el foco</div>
  {:else if ficha}
    <div class="caja-parametros">
      {#each parametros as p (p.id)}
        {#if p.tieneMaximo}
          <!-- La rueda también lo cambia, pero SOLO sobre el número y los botones (no sobre toda la fila): así hacer scroll por la caja
               no cambia la vida sin querer. Se manda a Rust agrupado (ver partida.cambiarValorActual) -->
          <div
            class="param ancho"
            class:destino-tirada={arrastreTirada.esDestino(ficha.id_instancia, p.id)}
            data-valor="{ficha.id_instancia}:{p.id}"
            role="group"
          >
            <span class="label">{p.clave}</span>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="control"
              title={ayuda(partida.controla(ficha) ? "Rueda (con el ratón sobre el número o los botones) o − / + para cambiarlo" : undefined)}
              onwheel={(e) => {
                if (!partida.controla(ficha)) return; // sin control, la rueda hace scroll como siempre
                e.preventDefault();
                partida.cambiarValorActual(ficha.id_instancia, p.id, e.deltaY < 0 ? 1 : -1);
              }}
            >
              {#if editandoPaso?.id === p.id}
                <!-- Clic derecho en − o +: cuánto suman o restan (se recuerda en este equipo, por parámetro) -->
                <label class="editar-paso">
                  <span>±</span>
                  <input
                    type="number"
                    min="1"
                    max={PASO_VALOR_MAX}
                    aria-label="Cuánto suman o restan − y + en {p.clave}"
                    bind:value={editandoPaso.texto}
                    use:enfocar
                    onkeydown={(e) => {
                      e.stopPropagation(); // Enter y Escape son de este campo, no de lo de fuera
                      if (e.key === "Enter") confirmarPaso();
                      else if (e.key === "Escape") editandoPaso = null;
                    }}
                    onblur={confirmarPaso}
                  />
                </label>
              {:else}
                {@const paso = pasoDe(p.id)}
                {#if partida.controla(ficha)}
                  <button
                    type="button"
                    aria-label="Restar {paso} a {p.clave}"
                    title={ayuda("Click derecho: cambiar la variación")}
                    onclick={() => partida.cambiarValorActual(ficha.id_instancia, p.id, -paso)}
                    oncontextmenu={(e) => empezarPaso(e, p.id, paso)}>−{paso}</button
                  >
                {/if}
                <span class="valor">{partida.valorMostrado(ficha, p.id)} / {numeroDe(ficha.base, p.id)}{p.unidad}</span>
                {#if partida.controla(ficha)}
                  <button
                    type="button"
                    aria-label="Sumar {paso} a {p.clave}"
                    title={ayuda("Click derecho: cambiar la variación")}
                    onclick={() => partida.cambiarValorActual(ficha.id_instancia, p.id, paso)}
                    oncontextmenu={(e) => empezarPaso(e, p.id, paso)}>+{paso}</button
                  >
                {/if}
              {/if}
            </div>
          </div>
        {:else}
          <div class="param">
            <span class="label">{p.clave}</span>
            <span class="valor">{p.texto}</span>
          </div>
        {/if}
      {/each}
      <div class="param">
        <span class="label">Cuerpo</span>
        <span class="valor">{textoCuerpo(ficha.base.cuerpo)}</span>
      </div>
      <div class="param">
        <span class="label">Visión nocturna</span>
        <span class="valor">{ficha.base.vision ? `${ficha.base.vision} m` : "—"}</span>
      </div>
    </div>
    {#if asociadas.length || partida.controla(ficha)}
      <div class="macros">
        {#each asociadas as a (a.asociacion.id)}
          <span class="macro">
            {#if a.boton}
              <button
                type="button"
                class="btn-macro"
                disabled={!partida.controla(ficha) || macros.ejecutando.includes(a.asociacion.id)}
                title={ayuda(a.macro.en_turno ? "Pulsa para ejecutarla (también va sola al empezar su turno)" : "Pulsa para ejecutarla")}
                onclick={() => macros.ejecutar(a.asociacion.id, ficha.id_instancia)}
                oncontextmenu={(e) => {
                  e.preventDefault();
                  abrirMacros(a.macro.id);
                }}
              >
                {a.macro.nombre}
              </button>
            {:else}
              <span
                class="etiqueta"
                role="presentation"
                title={ayuda("Va sola al empezar su turno en batalla. Clic derecho: editarla")}
                oncontextmenu={(e) => {
                  e.preventDefault();
                  abrirMacros(a.macro.id);
                }}>⟳ {a.macro.nombre}</span>
            {/if}
            {#if partida.controla(ficha)}
              <button type="button" class="quitar" aria-label="Quitar la macro de la ficha" title="Quitar de la ficha" onclick={() => macros.desasociar(a.asociacion.id)}>✕</button>
            {/if}
          </span>
        {/each}
        {#if partida.controla(ficha)}
          <button
            type="button"
            class="anadir"
            title={ayuda("Abre el editor de macros para esta ficha: pulsa una para asociársela, busca por nombre o crea una nueva")}
            onclick={() => abrirMacros()}>+ Macro</button>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="sin-seleccion">Pulsa una ficha activa para ver sus parámetros</div>
  {/if}
</div>

<style>
  .caja {
    flex: 1;
    min-height: 0;
    background: #1b1b1f;
    border: 1px solid #27272a;
    border-radius: 6px;
    padding: 6px;
    display: flex;
    flex-direction: column;
  }
  .titulo-mini {
    font-size: 10px; font-weight: bold; color: #71717a; margin-bottom: 3px; display: block;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .caja-parametros { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; overflow-y: auto; min-height: 0; }
  .param { background: #27272a; padding: 5px; border-radius: 4px; min-width: 0; }
  .param.ancho { grid-column: span 2; display: flex; justify-content: space-between; align-items: center; }
  .param.destino-tirada { background: rgba(250, 204, 21, 0.25); outline: 1px solid #facc15; }
  .label { color: #a1a1aa; font-size: 10px; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .valor { font-size: 13px; font-weight: bold; color: #f4f4f5; overflow-wrap: anywhere; }
  .control { display: flex; align-items: center; gap: 6px; }
  .control button {
    background: #3f3f46; color: white; border: none; min-width: 22px; height: 22px; padding: 0 4px; border-radius: 3px; cursor: pointer; font-weight: bold;
  }
  .editar-paso { display: flex; align-items: center; gap: 3px; color: #a1a1aa; font-size: 12px; }
  .editar-paso input {
    width: 58px; background: #09090b; border: 1px solid #3b82f6; color: white; padding: 2px 4px; border-radius: 3px; font-size: 12px;
  }
  .control button:hover { background: #52525b; }
  .sin-seleccion { color: #71717a; text-align: center; margin: auto 0; font-size: 11px; }
  .macros { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 5px; flex: none; }
  .macro { display: inline-flex; align-items: stretch; border-radius: 4px; overflow: hidden; border: 1px solid #3f3f46; }
  .btn-macro { background: #1e3a8a; color: #dbeafe; border: none; padding: 3px 8px; font-size: 11px; font-weight: bold; cursor: pointer; }
  .btn-macro:hover:not(:disabled) { background: #1d4ed8; }
  .btn-macro:disabled { opacity: 0.55; cursor: default; }
  .etiqueta { background: #27272a; color: #fde68a; padding: 3px 7px; font-size: 11px; }
  .quitar { background: #27272a; color: #a1a1aa; border: none; border-left: 1px solid #3f3f46; padding: 0 5px; cursor: pointer; font-size: 10px; }
  .quitar:hover { color: #f87171; }
  .anadir { background: #18181b; color: #a1a1aa; border: 1px dashed #3f3f46; border-radius: 4px; font-size: 11px; padding: 2px 4px; cursor: pointer; }
</style>
