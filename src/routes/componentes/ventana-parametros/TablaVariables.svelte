<!-- src/routes/componentes/ventana-parametros/TablaVariables.svelte -->
<!-- Pestaña «Variables» de la ventana de parámetros: las variables de la partida (Fuerza «f»...), que cada
     ficha rellena con su valor y se usan en el chat de dados (`2d6 + f`). Formulario rápido: se edita en
     la propia tabla, se reordena arrastrando y «+» añade una con neutro 10 y escala 2. -->
<script lang="ts">
  import { flip } from "svelte/animate";
  import { partida, type DefinicionVariable } from "$lib/partida.svelte";
  import { ListaOrdenable } from "$lib/ordenable.svelte";
  import ArrastreFlotante from "../comunes/ArrastreFlotante.svelte";
  import FilaVariable from "./FilaVariable.svelte";

  let { onError }: { onError: (mensaje: string) => void } = $props();

  type Fila = DefinicionVariable & { orden: number };

  const lista = new ListaOrdenable<Fila>({
    selector: ".fila-variable",
    alReordenar: async (ids) => {
      try {
        await partida.reordenarVariables(ids);
        onError("");
      } catch (err) {
        onError(String(err)); // la lista vuelve sola al orden real al recargar
      }
    },
    alPulsar: () => {}
  });

  $effect(() => lista.sincronizar(partida.definicionesVariables.map((d, i) => ({ ...d, orden: i }))));

  // Filas nuevas a medias (borradores). Viven aquí, no en Rust: «Añadir» siempre vale, un borrador con nombre se
  // crea al salir de su fila y uno sin nombre se queda sin más (y se pierde al cerrar la pestaña).
  let borradores = $state<number[]>(partida.definicionesVariables.length === 0 ? [0] : []);
  let siguiente = partida.definicionesVariables.length === 0 ? 1 : 0;

  function anadir() {
    borradores.push(siguiente++);
  }

  /** El borrador ya es una variable de Rust (aparece en la lista): se quita; con Enter se deja otro vacío listo. */
  function alCrear(clave: number, conEnter: boolean) {
    borradores = borradores.filter((c) => c !== clave);
    if (conEnter) anadir();
  }

  const descartar = (clave: number) => (borradores = borradores.filter((c) => c !== clave));
</script>

<p class="ayuda">
  Cada ficha pone su valor y el resultado es (valor − neutro) / escala, redondeado hacia abajo. En el chat se usan con su
  nombre o su abreviatura (2d6 + f) y valen lo de la ficha con el foco.
</p>

<div class="cabecera-tabla" aria-hidden="true">
  <span></span><span>Nombre</span><span>Abreviatura</span><span>Neutro</span><span>Escala</span><span></span>
</div>

<div class="lista">
  {#each lista.visuales as d, i (d.id)}
    <div animate:flip={{ duration: 150 }}>
      <FilaVariable
        definicion={d}
        indice={i}
        ejemplo={i === 0}
        arrastrando={lista.activo && lista.item?.id === d.id}
        onIniciarArrastre={(e) => lista.iniciar(d, i, e)}
        {onError}
      />
    </div>
  {/each}
  {#each borradores as clave, k (clave)}
    <FilaVariable
      ejemplo={lista.visuales.length === 0 && k === 0}
      {onError}
      onCreada={(conEnter) => alCrear(clave, conEnter)}
      onDescartar={() => descartar(clave)}
    />
  {/each}
</div>

<button type="button" class="btn-crear" onclick={anadir}>+ Añadir variable</button>

<ArrastreFlotante {lista} recto>
  {#if lista.item}
    <div class="flotante">
      <span class="asa">⋮⋮</span>
      <span class="nombre">{lista.item.nombre}</span>
      <span class="abreviatura">{lista.item.abreviatura}</span>
    </div>
  {/if}
</ArrastreFlotante>

<style>
  .ayuda { margin: 0; color: #a1a1aa; font-size: 10px; }
  .cabecera-tabla {
    display: grid; grid-template-columns: 14px 1fr 64px 50px 44px 46px; gap: 4px;
    padding: 0 7px; color: #71717a; font-size: 10px; font-weight: 600;
  }
  .lista { display: flex; flex-direction: column; gap: 3px; max-height: 280px; overflow-y: auto; }
  .btn-crear {
    padding: 6px; border-radius: 5px; cursor: pointer; font-size: 11px; font-weight: 600;
    background: transparent; border: 1px dashed #52525b; color: #a1a1aa;
  }
  .btn-crear:hover { border-color: #3b82f6; color: white; }
  .flotante {
    height: 100%; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 4px 8px;
    background: #27272a; border: 1px solid #3f3f46; border-radius: 5px; font-size: 11px; color: #f4f4f5;
  }
  .flotante .asa { color: #71717a; }
  .flotante .nombre { font-weight: 600; }
  .flotante .abreviatura { color: #a1a1aa; }
</style>
