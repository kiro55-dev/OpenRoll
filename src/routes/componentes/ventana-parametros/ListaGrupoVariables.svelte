<!-- src/routes/componentes/ventana-parametros/ListaGrupoVariables.svelte -->
<!-- Las variables de UN grupo en la definición del host: su tabla (bases o derivadas), reordenable arrastrando, con
     borradores locales («+ Añadir variable» siempre vale; un borrador se crea al salir de su fila en cuanto tiene lo
     necesario, y si no se queda sin más y se pierde al cambiar de grupo o de pestaña). -->
<script lang="ts">
  import { flip } from "svelte/animate";
  import { partida, reordenarConOcultas, type DefinicionVariable, type GrupoVariables } from "$lib/partida.svelte";
  import { ListaOrdenable } from "$lib/ordenable.svelte";
  import ArrastreFlotante from "../comunes/ArrastreFlotante.svelte";
  import FilaVariable from "./FilaVariable.svelte";

  let { grupo, onError }: { grupo: GrupoVariables; onError: (mensaje: string) => void } = $props();

  type Fila = DefinicionVariable & { orden: number };

  const derivadas = $derived(grupo.tipo === "Derivadas");
  const suyas = $derived(partida.definicionesVariables.filter((d) => d.grupo === grupo.id));

  const lista = new ListaOrdenable<Fila>({
    selector: ".fila-variable",
    alReordenar: async (ids) => {
      try {
        // El orden es uno solo para todas las variables: las de este grupo se reparten por los huecos que ya ocupaban
        const todas = partida.definicionesVariables.map((d) => d.id);
        await partida.reordenarVariables(reordenarConOcultas(todas, ids));
        onError("");
      } catch (err) {
        onError(String(err)); // la lista vuelve sola al orden real al recargar
      }
    },
    alPulsar: () => {},
    // Soltar sobre la pestaña de otro grupo del mismo tipo (bases con bases, derivadas con derivadas) lo cambia de grupo
    selectorDestino: ".grupo[data-destino]",
    alSoltarEnDestino: async (item, destino) => {
      const id = Number(destino);
      if (id === grupo.id) return;
      try {
        await partida.moverVariableAGrupo(item.id, id);
        onError("");
      } catch (err) {
        onError(String(err));
      }
    }
  });

  $effect(() => lista.sincronizar(suyas.map((d, i) => ({ ...d, orden: i }))));

  // svelte-ignore state_referenced_locally
  let borradores = $state<number[]>(suyas.length === 0 ? [0] : []);
  // svelte-ignore state_referenced_locally
  let siguiente = suyas.length === 0 ? 1 : 0;

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
  {#if derivadas}
    Se calculan solas: suman el resultado de las variables base de las que dependen y, si la ficha las marca, el de sus
    opcionales. En el chat se usan como las demás (1d20 + acro).
  {:else}
    Cada ficha pone su valor y el resultado es (valor − neutro) / escala, redondeado hacia abajo. En el chat se usan con su
    nombre o su abreviatura (2d6 + f) y valen lo de la ficha con el foco.
  {/if}
</p>

<div class="cabecera-tabla" class:derivadas aria-hidden="true">
  {#if derivadas}
    <span></span><span>Nombre</span><span>Abrev.</span><span>Depende de</span><span>Opcionales</span><span></span>
  {:else}
    <span></span><span>Nombre</span><span>Abreviatura</span><span>Neutro</span><span>Escala</span><span></span>
  {/if}
</div>

<div class="lista">
  {#each lista.visuales as d, i (d.id)}
    <div animate:flip={{ duration: 150 }}>
      <FilaVariable
        definicion={d}
        grupo={grupo.id}
        tipo={grupo.tipo}
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
      grupo={grupo.id}
      tipo={grupo.tipo}
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
  .cabecera-tabla.derivadas { grid-template-columns: 14px 1fr 54px 1fr 1fr 46px; }
  /* Sin scroll propio: la lista crece y el que hace scroll es el cuerpo de toda la ventana de parámetros */
  .lista { display: flex; flex-direction: column; gap: 3px; }
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
