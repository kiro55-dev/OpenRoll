<!-- src/routes/componentes/ventana-parametros/TablaArchivosTexto.svelte -->
<!-- Pestaña «TXT por defecto» de la ventana de parámetros: los archivos de texto (.txt) con los que nace toda ficha NUEVA del
     conjunto en uso (p. ej. «Historia», «Notas», con un texto inicial opcional). Las fichas que ya existen no cambian, y la plantilla
     HTML en blanco los lleva dentro. Filas como las de las variables (franja roja/verde, Enter, Ctrl+S, «Guardar cambios»), reordenables
     arrastrando; «+ Añadir archivo de texto» añade un borrador local que se crea en Rust al guardarlo. -->
<script lang="ts">
  import { flip } from "svelte/animate";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type ArchivoTextoDefecto } from "$lib/partida.svelte";
  import { ListaOrdenable } from "$lib/ordenable.svelte";
  import { edicionVariables as edicion } from "$lib/edicionVariables.svelte";
  import { MAX_ARCHIVOS_TEXTO } from "$lib/plantillaFicha";
  import ArrastreFlotante from "../comunes/ArrastreFlotante.svelte";
  import FilaArchivoTexto from "./FilaArchivoTexto.svelte";

  let { onError }: { onError: (mensaje: string) => void } = $props();

  type Fila = ArchivoTextoDefecto & { orden: number };

  const lista = new ListaOrdenable<Fila>({
    selector: ".fila-archivo-texto",
    alReordenar: async (ids) => {
      try {
        await partida.reordenarArchivosTextoDefecto(ids);
        onError("");
      } catch (err) {
        onError(String(err)); // la lista vuelve sola al orden real al recargar
      }
    },
    alPulsar: () => {}
  });

  $effect(() => lista.sincronizar(partida.archivosTextoDefecto.map((a, i) => ({ ...a, orden: i }))));

  // svelte-ignore state_referenced_locally
  let borradores = $state<number[]>(partida.archivosTextoDefecto.length === 0 ? [0] : []);
  // svelte-ignore state_referenced_locally
  let siguiente = partida.archivosTextoDefecto.length === 0 ? 1 : 0;

  /** Con los 20 archivos hechos (o en camino) no se añaden más: Rust no los aceptaría. */
  const lleno = $derived(partida.archivosTextoDefecto.length + borradores.length >= MAX_ARCHIVOS_TEXTO);

  function anadir() {
    if (lleno) return;
    borradores.push(siguiente++);
  }

  /** El borrador ya es un archivo de Rust (aparece en la lista): se quita; con Enter se deja otro vacío listo. */
  function alCrear(clave: number, conEnter: boolean) {
    borradores = borradores.filter((c) => c !== clave);
    if (conEnter) anadir();
  }

  const descartar = (clave: number) => (borradores = borradores.filter((c) => c !== clave));

  // ---- Guardar lo pendiente de todas las filas (el mismo botón que Ctrl+S) ----
  let guardandoTodo = $state(false);

  async function guardarTodo() {
    if (guardandoTodo) return;
    guardandoTodo = true;
    try {
      await edicion.guardarTodo();
    } finally {
      guardandoTodo = false;
    }
  }
</script>

<p class="ayuda">
  Cada ficha NUEVA nace con estos archivos de texto (con su texto inicial, si lo tiene). Las fichas que ya existen no cambian. La
  plantilla en blanco que se exporta también los lleva. Hasta {MAX_ARCHIVOS_TEXTO} archivos.
</p>

<div class="lista">
  {#each lista.visuales as a, i (a.id)}
    <div animate:flip={{ duration: 150 }}>
      <FilaArchivoTexto
        archivo={a}
        indice={i}
        ejemplo={i === 0}
        arrastrando={lista.activo && lista.item?.id === a.id}
        onIniciarArrastre={(e) => lista.iniciar(a, i, e)}
        {onError}
      />
    </div>
  {/each}
  {#each borradores as clave, k (clave)}
    <FilaArchivoTexto
      ejemplo={lista.visuales.length === 0 && k === 0}
      {onError}
      onCreada={(conEnter) => alCrear(clave, conEnter)}
      onDescartar={() => descartar(clave)}
    />
  {/each}
</div>

<button type="button" class="btn-crear" disabled={lleno} onclick={anadir}>+ Añadir archivo de texto</button>

<div class="pie">
  <!-- Con cambios sin guardar en cualquier fila (de aquí o de las variables): el botón verde -->
  {#if edicion.hayCambios}
    <button type="button" class="btn-guardar-cambios" title={ayuda("Atajo: Ctrl + S")} onclick={guardarTodo} disabled={guardandoTodo}>Guardar cambios</button>
  {/if}
</div>

<ArrastreFlotante {lista} recto>
  {#if lista.item}
    <div class="flotante">
      <span class="asa">⋮⋮</span>
      <span class="nombre">{lista.item.nombre}.txt</span>
    </div>
  {/if}
</ArrastreFlotante>

<style>
  .ayuda { margin: 0; color: #a1a1aa; font-size: 10px; }
  /* Sin scroll propio: la lista crece y el que hace scroll es el cuerpo de toda la ventana de parámetros */
  .lista { display: flex; flex-direction: column; gap: 3px; }
  .btn-crear {
    padding: 6px; border-radius: 5px; cursor: pointer; font-size: 11px; font-weight: 600;
    background: transparent; border: 1px dashed #52525b; color: #a1a1aa;
  }
  .btn-crear:hover:not(:disabled) { border-color: #3b82f6; color: white; }
  .btn-crear:disabled { opacity: 0.5; cursor: default; }
  .pie { display: flex; align-items: center; gap: 8px; }
  .pie:empty { display: none; }
  .btn-guardar-cambios {
    border: none; padding: 5px 12px; border-radius: 4px; font-size: 11px; font-weight: 600; cursor: pointer;
    background: #16a34a; color: white;
  }
  .btn-guardar-cambios:hover { background: #15803d; }
  .btn-guardar-cambios:disabled { opacity: 0.6; cursor: default; }
  .flotante {
    height: 100%; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 4px 8px;
    background: #27272a; border: 1px solid #3f3f46; border-radius: 5px; font-size: 11px; color: #f4f4f5;
  }
  .flotante .asa { color: #71717a; }
  .flotante .nombre { font-weight: 600; }
</style>
