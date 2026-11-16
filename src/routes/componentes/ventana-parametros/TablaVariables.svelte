<!-- src/routes/componentes/ventana-parametros/TablaVariables.svelte -->
<!-- Pestaña «Variables» de la ventana de parámetros: los GRUPOS de variables, una subpestaña cada uno. Un grupo es de
     bases (Fuerza, Destreza... cada ficha pone su valor) o de derivadas (Acrobacias = Destreza + Competencia si la ficha la
     marca). Doble clic en una subpestaña la renombra; «+» crea un grupo nuevo eligiendo su tipo. Las variables de cada
     grupo se editan en `ListaGrupoVariables`. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type GrupoVariables, type TipoGrupoVariables } from "$lib/partida.svelte";
  import { MAX_NOMBRE_GRUPO, avisoDeBorrar } from "$lib/variables";
  import ListaGrupoVariables from "./ListaGrupoVariables.svelte";
  import ListasRangos from "./ListasRangos.svelte";

  let { onError }: { onError: (mensaje: string) => void } = $props();

  /** El grupo abierto (el primero si el que había ya no existe). */
  let idActivo = $state<number | null>(partida.gruposVariables[0]?.id ?? null);
  const activo = $derived(partida.gruposVariables.find((g) => g.id === idActivo) ?? partida.gruposVariables[0] ?? null);

  // ---- Renombrar (doble clic) ----
  let renombrando = $state<{ id: number; texto: string } | null>(null);

  function empezarRenombrar(g: GrupoVariables) {
    renombrando = { id: g.id, texto: g.nombre };
  }

  async function confirmarRenombrar() {
    const r = renombrando;
    renombrando = null; // antes de esperar: el blur que provoca quitar el campo no vuelve a entrar
    if (!r) return;
    const nombre = r.texto.trim();
    const actual = partida.gruposVariables.find((g) => g.id === r.id);
    if (!actual || !nombre || nombre === actual.nombre) return;
    try {
      await partida.renombrarGrupoVariables(r.id, nombre);
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  /** Al aparecer el campo, ya con el foco y el nombre seleccionado. */
  function enfocar(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  // ---- Crear (pestaña «+») ----
  let eligiendoTipo = $state(false);

  /** «Grupo N» con el primer N libre. */
  function nombreNuevo(): string {
    const usados = new Set(partida.gruposVariables.map((g) => g.nombre.toLowerCase()));
    let n = partida.gruposVariables.length + 1;
    while (usados.has(`grupo ${n}`)) n++;
    return `Grupo ${n}`;
  }

  async function crear(tipo: TipoGrupoVariables) {
    eligiendoTipo = false;
    try {
      const g = await partida.crearGrupoVariables(nombreNuevo(), tipo);
      idActivo = g.id;
      empezarRenombrar(g); // lo normal es ponerle nombre enseguida
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  // ---- Borrar el grupo abierto (con confirmación) ----
  let confirmandoBorrar = $state(false);
  const avisoBorrarGrupo = $derived.by(() => {
    if (!activo) return "";
    const ids = partida.definicionesVariables.filter((d) => d.grupo === activo.id).map((d) => d.id);
    const cascada = avisoDeBorrar(partida.definicionesVariables, ids);
    const cuantas = ids.length === 0 ? "No tiene variables." : `Se borrarán sus ${ids.length} variable${ids.length > 1 ? "s" : ""} de todas las fichas.`;
    return `¿Borrar el grupo «${activo.nombre}»? ${cuantas}${cascada ? ` ${cascada}` : ""}`;
  });

  async function borrarGrupo() {
    const g = activo;
    confirmandoBorrar = false;
    if (!g) return;
    try {
      await partida.borrarGrupoVariables(g.id);
      idActivo = partida.gruposVariables[0]?.id ?? null;
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  function cambiarA(g: GrupoVariables) {
    onError(""); // un aviso de otro grupo no se queda
    confirmandoBorrar = false;
    eligiendoTipo = false;
    idActivo = g.id;
  }
</script>

<div class="grupos" role="tablist" aria-label="Grupos de variables">
  {#each partida.gruposVariables as g (g.id)}
    {#if renombrando?.id === g.id}
      <input
        class="renombrar"
        maxlength={MAX_NOMBRE_GRUPO}
        aria-label="Nombre del grupo"
        bind:value={renombrando.texto}
        use:enfocar
        onkeydown={(e) => {
          e.stopPropagation(); // Enter y Escape son de este campo, no de la ventana
          if (e.key === "Enter") void confirmarRenombrar();
          else if (e.key === "Escape") renombrando = null;
        }}
        onblur={() => void confirmarRenombrar()}
      />
    {:else}
      <button
        type="button"
        role="tab"
        class="grupo"
        class:activo={activo?.id === g.id}
        aria-selected={activo?.id === g.id}
        title={ayuda(`${g.tipo === "Base" ? "Variables base" : "Variables derivadas"}. Doble clic para cambiarle el nombre`)}
        onclick={() => cambiarA(g)}
        ondblclick={() => empezarRenombrar(g)}
      >
        {g.nombre}
        {#if g.tipo === "Derivadas"}<span class="marca" aria-hidden="true">ƒ</span>{/if}
      </button>
    {/if}
  {/each}
  <span class="nuevo">
    <button type="button" class="grupo mas" aria-label="Nuevo grupo de variables" title={ayuda("Nuevo grupo de variables")} onclick={() => (eligiendoTipo = !eligiendoTipo)}>+</button>
    {#if eligiendoTipo}
      <span class="tipos" role="menu">
        <button type="button" role="menuitem" onclick={() => crear("Base")} title={ayuda("Cada ficha pone su valor (Fuerza, Destreza…)")}>Bases</button>
        <button type="button" role="menuitem" onclick={() => crear("Derivadas")} title={ayuda("Se calculan sumando bases (Acrobacias = Destreza…)")}>Derivadas</button>
      </span>
    {/if}
  </span>
</div>

{#if activo}
  {#key activo.id}
    <ListaGrupoVariables grupo={activo} {onError} />
  {/key}

  <div class="pie-grupo">
    {#if confirmandoBorrar}
      <div class="confirmar" role="alert">
        <span>{avisoBorrarGrupo}</span>
        <button type="button" class="si" onclick={borrarGrupo}>Borrar grupo</button>
        <button type="button" class="no" onclick={() => (confirmandoBorrar = false)}>Cancelar</button>
      </div>
    {:else}
      <button type="button" class="btn-borrar-grupo" onclick={() => (confirmandoBorrar = true)}>Borrar grupo</button>
    {/if}
  </div>
{:else}
  <p class="vacio">No hay grupos de variables. Crea uno con «+».</p>
{/if}

<!-- Las listas de rangos que eligen las derivadas (una vez definidas, las comparten varias variables) -->
<ListasRangos {onError} />

<style>
  .grupos { display: flex; flex-wrap: wrap; gap: 3px; align-items: center; }
  .grupo {
    background: #111113; border: 1px solid #27272a; color: #a1a1aa; border-radius: 4px; padding: 2px 8px;
    font-size: 11px; font-weight: 600; cursor: pointer; display: inline-flex; align-items: center; gap: 4px;
  }
  .grupo:hover { color: #f4f4f5; border-color: #3f3f46; }
  .grupo.activo { color: white; border-color: #3b82f6; background: rgba(59, 130, 246, 0.12); }
  .grupo.mas { padding: 2px 7px; }
  .marca { color: #60a5fa; font-style: italic; font-size: 10px; }
  .renombrar {
    width: 120px; padding: 2px 6px; border-radius: 4px; font-size: 11px;
    background: #09090b; border: 1px solid #3b82f6; color: white;
  }
  .nuevo { position: relative; display: inline-flex; gap: 3px; }
  .tipos { display: inline-flex; gap: 3px; }
  .tipos button {
    background: #27272a; border: 1px solid #3f3f46; color: #f4f4f5; border-radius: 4px; padding: 2px 8px; font-size: 11px; cursor: pointer;
  }
  .tipos button:hover { border-color: #3b82f6; }
  .pie-grupo { display: flex; justify-content: flex-end; }
  .btn-borrar-grupo { background: none; border: none; color: #f87171; font-size: 10px; font-weight: 600; cursor: pointer; padding: 2px; }
  .btn-borrar-grupo:hover { text-decoration: underline; }
  .confirmar {
    flex: 1; display: flex; align-items: center; gap: 6px; flex-wrap: wrap;
    padding: 5px 7px; border-radius: 4px; background: rgba(127, 29, 29, 0.35); border: 1px solid #7f1d1d;
    color: #fecaca; font-size: 10px;
  }
  .confirmar span { flex: 1; min-width: 160px; }
  .confirmar button { border: none; border-radius: 3px; padding: 3px 8px; font-size: 10px; font-weight: 700; cursor: pointer; }
  .confirmar .si { background: #b91c1c; color: white; }
  .confirmar .si:hover { background: #dc2626; }
  .confirmar .no { background: #27272a; color: #d4d4d8; }
  .vacio { margin: 0; color: #71717a; font-size: 11px; text-align: center; padding: 10px; }
</style>
