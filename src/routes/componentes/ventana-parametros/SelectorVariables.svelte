<!-- src/routes/componentes/ventana-parametros/SelectorVariables.svelte -->
<!-- Elegir varias variables BASE (de qué depende una derivada, o sus opcionales): un botón con las elegidas («des + fue ▾»)
     que abre una lista con checks. La lista va dentro de la fila (así elegir no cuenta como «salir de la fila») pero se
     dibuja fija sobre la ventana para que no la corte el scroll de la tabla. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import type { DefinicionParametro, DefinicionVariable } from "$lib/partida.svelte";
  import { etiquetaVariable } from "$lib/variables";

  let {
    opciones,
    elegidas = $bindable(),
    excluidas = [],
    parametros = [],
    parametrosElegidos = $bindable([]),
    parametrosExcluidos = [],
    vacio = "Elegir…",
    titulo,
    etiquetaAria
  }: {
    /** Las variables base que se pueden elegir. */
    opciones: DefinicionVariable[];
    elegidas: number[];
    /** Las que ya están elegidas en el otro selector de la fila: se ven pero no se pueden marcar. */
    excluidas?: number[];
    /** Los parámetros visuales que también se pueden elegir (su valor se suma tal cual), con los mismos permisos que las variables. */
    parametros?: DefinicionParametro[];
    parametrosElegidos?: number[];
    parametrosExcluidos?: number[];
    vacio?: string;
    titulo?: string;
    etiquetaAria: string;
  } = $props();

  let abierto = $state(false);
  let boton = $state<HTMLButtonElement | null>(null);
  let pos = $state({ x: 0, y: 0, ancho: 160 });

  const texto = $derived(
    [
      ...elegidas
        .map((id) => opciones.find((o) => o.id === id))
        .filter((o): o is DefinicionVariable => !!o)
        .map(etiquetaVariable),
      ...parametrosElegidos.map((id) => parametros.find((p) => p.id === id)?.clave).filter((c): c is string => !!c)
    ].join(" + ")
  );

  /**
   * Cierra la lista. Si un check de dentro tiene el foco, primero se devuelve al botón: si no, al desaparecer el check
   * el foco caería a la página, la fila creería que has salido de ella y guardaría el borrador a medias (y la fila
   * nueva sustituiría a la que estabas usando justo cuando ibas a pulsar otro campo).
   */
  function cerrar() {
    if (raiz?.querySelector(".lista")?.contains(document.activeElement)) boton?.focus();
    abierto = false;
  }

  function alternar() {
    if (abierto) return cerrar();
    const r = boton?.getBoundingClientRect();
    if (r) pos = { x: r.left, y: r.bottom + 2, ancho: Math.max(160, r.width) };
    abierto = true;
  }

  function marcar(id: number) {
    elegidas = elegidas.includes(id) ? elegidas.filter((x) => x !== id) : [...elegidas, id];
  }

  function marcarParametro(id: number) {
    parametrosElegidos = parametrosElegidos.includes(id) ? parametrosElegidos.filter((x) => x !== id) : [...parametrosElegidos, id];
  }

  // Pulsar fuera lo cierra; Escape también (sin llegar a la fila, que desharía lo escrito)
  let raiz = $state<HTMLSpanElement | null>(null);
  $effect(() => {
    if (!abierto) return;
    const fuera = (e: PointerEvent) => {
      if (!raiz?.contains(e.target as Node)) cerrar();
    };
    window.addEventListener("pointerdown", fuera, true);
    return () => window.removeEventListener("pointerdown", fuera, true);
  });
</script>

<span class="selector" bind:this={raiz}>
  <button
    type="button"
    class="boton"
    class:vacio={!texto}
    aria-label={etiquetaAria}
    aria-expanded={abierto}
    title={ayuda(titulo)}
    bind:this={boton}
    onclick={alternar}
    onkeydown={(e) => {
      if (e.key === "Escape" && abierto) {
        e.stopPropagation();
        cerrar();
      }
    }}
  >
    <span class="texto">{texto || vacio}</span>
    <span class="flecha">▾</span>
  </button>

  {#if abierto}
    <!-- Pulsar dentro (también sobre el texto, que no admite foco) no debe quitarle el foco al botón: si no, la fila cree que
         has salido, guarda el borrador a medias y la fila nueva sustituye a esta con la lista abierta. El clic sigue
         marcando la opción (un \`mousedown\` cancelado no cancela el clic del \`label\`). -->
    <div
      class="lista"
      style:left="{pos.x}px"
      style:top="{pos.y}px"
      style:min-width="{pos.ancho}px"
      role="listbox"
      tabindex="-1"
      aria-multiselectable="true"
      onmousedown={(e) => e.preventDefault()}
    >
      {#each opciones as o (o.id)}
        {@const bloqueada = excluidas.includes(o.id)}
        <label class="opcion" class:bloqueada>
          <input
            type="checkbox"
            checked={elegidas.includes(o.id)}
            disabled={bloqueada}
            onchange={() => marcar(o.id)}
            onkeydown={(e) => {
              if (e.key === "Escape") {
                e.stopPropagation();
                cerrar();
              }
            }}
          />
          <span>{o.nombre}</span>
          {#if o.abreviatura}<span class="abrev">{o.abreviatura}</span>{/if}
        </label>
      {:else}
        {#if parametros.length === 0}<span class="sin">No hay variables base: créalas en un grupo de bases.</span>{/if}
      {/each}
      {#if parametros.length > 0}
        <span class="seccion">Parámetros visuales (su valor)</span>
        {#each parametros as p (p.id)}
          {@const bloqueado = parametrosExcluidos.includes(p.id)}
          <label class="opcion" class:bloqueada={bloqueado}>
            <input
              type="checkbox"
              checked={parametrosElegidos.includes(p.id)}
              disabled={bloqueado}
              onchange={() => marcarParametro(p.id)}
              onkeydown={(e) => {
                if (e.key === "Escape") {
                  e.stopPropagation();
                  cerrar();
                }
              }}
            />
            <span>{p.clave}</span>
          </label>
        {/each}
      {/if}
    </div>
  {/if}
</span>

<style>
  .selector { position: relative; min-width: 0; display: flex; }
  .boton {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: 4px; padding: 3px 5px; border-radius: 4px; cursor: pointer;
    background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 11px; text-align: left;
  }
  .boton:hover { border-color: #52525b; }
  .boton.vacio .texto { color: #71717a; }
  .texto { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .flecha { color: #71717a; font-size: 9px; }
  .lista {
    position: fixed; z-index: 2000; max-height: 220px; overflow-y: auto; padding: 4px;
    background: #18181b; border: 1px solid #3f3f46; border-radius: 6px; box-shadow: 0 8px 20px rgba(0, 0, 0, 0.6);
    display: flex; flex-direction: column; gap: 1px;
  }
  .opcion { display: flex; align-items: center; gap: 6px; padding: 3px 6px; border-radius: 4px; cursor: pointer; font-size: 11px; color: #f4f4f5; }
  .opcion:hover { background: #27272a; }
  .opcion input { margin: 0; accent-color: #3b82f6; cursor: pointer; }
  .opcion.bloqueada { opacity: 0.4; cursor: default; }
  .abrev { color: #a1a1aa; margin-left: auto; }
  .seccion { color: #71717a; font-size: 9px; font-weight: 700; text-transform: uppercase; padding: 5px 6px 2px; }
  .sin { color: #71717a; font-size: 10px; padding: 4px 6px; }
</style>
