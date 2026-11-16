<!-- src/routes/componentes/ventana-parametros/FilaListaRangos.svelte -->
<!-- Una lista de rangos («Competencia»: Sin entrenar +0, Entrenado +2, Experto +4...): se edita aquí y la comparten todas las variables derivadas
     que la elijan en su «Fórmula y rangos». Se guarda entera con «Guardar» (Rust la valida y da el motivo si algo no vale); cambiarla cambia
     todas las variables que la usan. Sin `lista` es una nueva (borrador): se crea en Rust al guardar. Borrar una que usan variables pide
     confirmar y dice cuáles (se quedarán sin rangos). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type ListaRangos } from "$lib/partida.svelte";
  import { LIMITE_VALOR_VARIABLE, MAX_NOMBRE_OPCION, MAX_OPCIONES_VARIABLE, etiquetaVariable } from "$lib/variables";

  let {
    lista = null,
    onError,
    onCreada,
    onDescartar
  }: {
    lista?: ListaRangos | null;
    onError: (mensaje: string) => void;
    /** Borrador: se creó en Rust (la lista nueva ya está en `partida.listasRangos`). */
    onCreada?: () => void;
    /** Borrador: se quita sin crear nada. */
    onDescartar?: () => void;
  } = $props();

  interface FilaRango { nombre: string; bonus: number | null }

  let campoNombre = $state<HTMLInputElement | null>(null);
  // svelte-ignore state_referenced_locally
  let nombre = $state(lista?.nombre ?? "");
  // svelte-ignore state_referenced_locally
  let opciones = $state<FilaRango[]>((lista?.opciones ?? [{ nombre: "", bonus: 0 }]).map((o) => ({ ...o })));
  /** Hay algo escrito que aún no está guardado. */
  // svelte-ignore state_referenced_locally
  let tocado = $state(!lista);
  let guardando = $state(false);
  let confirmando = $state(false);

  /** Las variables que usan esta lista (para el aviso de borrar y para saber cuántas cambian al guardar). */
  const usadas = $derived(lista ? partida.definicionesVariables.filter((d) => d.rangos === lista.id) : []);

  function cargar() {
    nombre = lista?.nombre ?? "";
    opciones = (lista?.opciones ?? [{ nombre: "", bonus: 0 }]).map((o) => ({ ...o }));
    tocado = !lista;
  }

  // Lo de Rust (al abrir, al cambiar de conjunto, tras guardar) se toma salvo que haya algo escrito a medias
  $effect(() => {
    void lista;
    if (!tocado) cargar();
  });

  onMount(() => {
    if (!lista) campoNombre?.focus();
  });

  const marcar = () => (tocado = true);

  async function guardar() {
    if (guardando) return;
    // Un rango recién añadido y sin escribir no cuenta
    const datos = {
      nombre: nombre.trim(),
      opciones: opciones.filter((o) => o.nombre.trim() !== "").map((o) => ({ nombre: o.nombre.trim(), bonus: Number(o.bonus ?? 0) }))
    };
    guardando = true;
    try {
      if (lista) await partida.modificarListaRangos({ ...datos, id: lista.id });
      else await partida.crearListaRangos(datos);
      onError("");
      tocado = false;
      if (!lista) onCreada?.();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err)); // lo escrito se queda, para corregirlo
    } finally {
      guardando = false;
    }
  }

  async function borrar() {
    if (!lista) return onDescartar?.();
    confirmando = false;
    try {
      await partida.borrarListaRangos(lista.id);
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  function alTeclear(e: KeyboardEvent) {
    // Enter y Escape son de esta lista, no de la ventana (que guardaría un parámetro o se cerraría)
    if (e.key === "Enter") {
      if ((e.target as HTMLElement).tagName === "BUTTON") return;
      e.preventDefault();
      e.stopPropagation();
      void guardar();
    } else if (e.key === "Escape") {
      e.stopPropagation();
      if (confirmando) return void (confirmando = false);
      if (!lista) return onDescartar?.();
      cargar();
      onError("");
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="lista" class:nueva={!lista} role="group" aria-label={lista ? `Lista de rangos ${lista.nombre}` : "Lista de rangos nueva"} onkeydown={alTeclear}>
  <div class="cabecera">
    <input
      type="text"
      class="campo nombre"
      maxlength="30"
      placeholder="Competencia"
      aria-label="Nombre de la lista"
      bind:value={nombre}
      bind:this={campoNombre}
      oninput={marcar}
    />
    {#if lista}
      <span class="usos" title={ayuda(usadas.map((d) => etiquetaVariable(d)).join(", ") || "Ninguna variable la usa todavía")}>
        {usadas.length === 0 ? "sin usar" : `la usan ${usadas.length}`}
      </span>
    {/if}
  </div>

  {#each opciones as o, i (i)}
    <div class="rango">
      <input type="text" class="campo" maxlength={MAX_NOMBRE_OPCION} placeholder="Entrenado" aria-label="Nombre del rango" bind:value={o.nombre} oninput={marcar} />
      <input
        type="number"
        class="campo numero"
        min={-LIMITE_VALOR_VARIABLE}
        max={LIMITE_VALOR_VARIABLE}
        aria-label="Bono del rango"
        title={ayuda("Lo que suma este rango al resultado de la variable")}
        bind:value={o.bonus}
        oninput={marcar}
      />
      <button type="button" class="btn-quitar" aria-label="Quitar el rango" onclick={() => { opciones = opciones.filter((_, j) => j !== i); marcar(); }}>✕</button>
    </div>
  {/each}
  {#if opciones.length < MAX_OPCIONES_VARIABLE}
    <button type="button" class="btn-mas" onclick={() => { opciones = [...opciones, { nombre: "", bonus: 0 }]; marcar(); }}>+ Añadir rango</button>
  {/if}

  {#if confirmando}
    <div class="confirmar" role="alert">
      <span>
        {#if usadas.length > 0}La usan {usadas.map((d) => etiquetaVariable(d)).join(", ")}: se quedarán sin rangos.{:else}Se borrará la lista.{/if}
      </span>
      <button type="button" class="si" onclick={borrar}>Borrar</button>
      <button type="button" class="no" onclick={() => (confirmando = false)}>Cancelar</button>
    </div>
  {:else}
    <div class="pie">
      <button type="button" class="btn-borrar" onclick={() => (lista ? (confirmando = true) : onDescartar?.())}>{lista ? "Borrar" : "Quitar"}</button>
      {#if tocado && lista}<button type="button" class="btn-cancelar" onclick={() => { cargar(); onError(""); }}>Descartar</button>{/if}
      <button type="button" class="btn-guardar" disabled={guardando || !tocado} onclick={guardar}>{lista ? "Guardar" : "Crear"}</button>
    </div>
  {/if}
</div>

<style>
  .lista { display: flex; flex-direction: column; gap: 4px; padding: 6px 8px; background: #111113; border: 1px solid #27272a; border-radius: 5px; }
  .lista.nueva { border-style: dashed; }
  .cabecera { display: flex; align-items: center; gap: 8px; }
  .usos { color: #71717a; font-size: 10px; cursor: help; }
  .campo { min-width: 0; padding: 3px 5px; border-radius: 4px; background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 11px; }
  .campo.nombre { width: 160px; font-weight: 700; }
  .campo.numero { width: 56px; text-align: center; padding: 3px 2px; }
  .rango { display: grid; grid-template-columns: 1fr 60px 22px; gap: 4px; align-items: center; }
  .btn-quitar { background: none; border: none; color: #f87171; cursor: pointer; font-size: 11px; padding: 2px; }
  .btn-quitar:hover { color: #ef4444; }
  .btn-mas { align-self: flex-start; background: none; border: 1px dashed #3f3f46; color: #a1a1aa; border-radius: 4px; font-size: 10px; padding: 2px 8px; cursor: pointer; }
  .btn-mas:hover { color: white; border-color: #71717a; }
  .pie { display: flex; align-items: center; justify-content: flex-end; gap: 6px; margin-top: 2px; }
  .btn-borrar { margin-right: auto; background: none; border: none; color: #f87171; cursor: pointer; font-size: 11px; font-weight: 600; padding: 2px; }
  .btn-borrar:hover { color: #ef4444; text-decoration: underline; }
  .btn-guardar { background: #2563eb; border: none; color: white; border-radius: 4px; padding: 3px 10px; font-size: 11px; font-weight: 600; cursor: pointer; }
  .btn-guardar:hover:not(:disabled) { background: #1d4ed8; }
  .btn-guardar:disabled { opacity: 0.5; cursor: default; }
  .btn-cancelar { background: #27272a; border: none; color: #d4d4d8; border-radius: 4px; padding: 3px 8px; font-size: 11px; cursor: pointer; }
  .confirmar {
    display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 5px 7px; border-radius: 4px;
    background: rgba(127, 29, 29, 0.35); border: 1px solid #7f1d1d; color: #fecaca; font-size: 10px;
  }
  .confirmar span { flex: 1; min-width: 140px; }
  .confirmar button { border: none; border-radius: 3px; padding: 3px 8px; font-size: 10px; font-weight: 700; cursor: pointer; }
  .confirmar .si { background: #b91c1c; color: white; }
  .confirmar .no { background: #27272a; color: #d4d4d8; }
</style>
