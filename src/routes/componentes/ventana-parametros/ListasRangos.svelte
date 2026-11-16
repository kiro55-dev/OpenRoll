<!-- src/routes/componentes/ventana-parametros/ListasRangos.svelte -->
<!-- Las listas de rangos del conjunto (al final de la pestaña «Variables»): se definen UNA vez y las variables derivadas las eligen en su «Fórmula y
     rangos». Cada ficha elige un rango por variable y su bono se suma al resultado (la competencia de Pathfinder: Sin entrenar … Legendario). -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida } from "$lib/partida.svelte";
  import FilaListaRangos from "./FilaListaRangos.svelte";

  let { onError }: { onError: (mensaje: string) => void } = $props();

  /** Una lista nueva, a medias (se crea en Rust al guardarla). */
  let creando = $state(false);
  // svelte-ignore state_referenced_locally
  let abierto = $state(partida.listasRangos.length > 0);
</script>

<div class="listas">
  <button
    type="button"
    class="titulo"
    aria-expanded={abierto}
    title={ayuda("Rangos que elige cada ficha en una variable derivada (Sin entrenar +0, Entrenado +2...). Se definen una vez y las comparten varias variables")}
    onclick={() => (abierto = !abierto)}
  >
    Listas de rangos ({partida.listasRangos.length}) {abierto ? "▴" : "▾"}
  </button>

  {#if abierto}
    {#each partida.listasRangos as l (l.id)}
      <FilaListaRangos lista={l} {onError} />
    {/each}
    {#if creando}
      <FilaListaRangos {onError} onCreada={() => (creando = false)} onDescartar={() => (creando = false)} />
    {:else}
      <button type="button" class="btn-nueva" onclick={() => (creando = true)}>+ Nueva lista de rangos</button>
    {/if}
    <p class="ayuda">Elige una lista en «ƒ Fórmula y rangos» de una variable derivada. Si cambias la lista, cambian todas las variables que la usan.</p>
  {/if}
</div>

<style>
  .listas { display: flex; flex-direction: column; gap: 5px; border-top: 1px solid #27272a; padding-top: 6px; margin-top: 4px; }
  .titulo { align-self: flex-start; background: none; border: none; color: #a1a1aa; font-size: 11px; font-weight: 700; cursor: pointer; padding: 0; }
  .titulo:hover { color: white; }
  .btn-nueva { align-self: flex-start; background: none; border: 1px dashed #3f3f46; color: #a1a1aa; border-radius: 4px; font-size: 11px; padding: 3px 10px; cursor: pointer; }
  .btn-nueva:hover { color: white; border-color: #71717a; }
  .ayuda { margin: 0; color: #71717a; font-size: 10px; line-height: 1.4; }
</style>
