<!-- src/routes/componentes/ventana-parametros/ConjuntosParametros.svelte -->
<!-- El conjunto de parámetros que usa la partida (parámetros visuales + variables), arriba de la ventana de parámetros: un
     selector para cambiar de conjunto y «Gestionar…» para importar, exportar, renombrar y borrar los que guarda. Un conjunto
     define el rol que se juega (D&D…): la partida guarda varios y usa uno. Cada conjunto guarda lo que tenían las fichas cuando lo
     dejaste y lo recupera al volver; nada se comparte de uno a otro. Cambiar de conjunto cambia los valores de TODAS las fichas,
     así que SIEMPRE pide confirmar y lo explica. -->
<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type ResumenConjunto } from "$lib/partida.svelte";

  let { onError, alCambiar }: { onError: (mensaje: string) => void; alCambiar: () => void } = $props();

  const activo = $derived(partida.conjuntos.guardados.find((c) => c.id === partida.conjuntos.activo));
  const deSerieNuevos = $derived(partida.conjuntos.de_serie.filter((s) => !s.ya_anadido));

  let gestionando = $state(false);
  let ocupado = $state(false);
  /** Lo último que se hizo bien (importado, exportado...), para decírselo. */
  let aviso = $state("");

  // ---- Cambiar de conjunto (con aviso) ----
  type Destino = { tipo: "guardado"; id: number; nombre: string } | { tipo: "serie"; clave: string; nombre: string };
  let confirmando = $state<Destino | null>(null);

  function pedirCambio(valor: string) {
    aviso = "";
    if (valor.startsWith("serie:")) {
      const s = partida.conjuntos.de_serie.find((x) => x.clave === valor.slice(6));
      if (s) confirmando = { tipo: "serie", clave: s.clave, nombre: s.nombre };
    } else {
      const c = partida.conjuntos.guardados.find((x) => x.id === Number(valor));
      if (c && c.id !== partida.conjuntos.activo) confirmando = { tipo: "guardado", id: c.id, nombre: c.nombre };
    }
  }

  async function cambiar() {
    const d = confirmando;
    if (!d || ocupado || partida.hayFichaNuevaAbierta) return;
    ocupado = true;
    try {
      const id = d.tipo === "serie" ? await partida.anadirConjuntoDeSerie(d.clave) : d.id;
      await partida.usarConjunto(id);
      confirmando = null;
      onError("");
      alCambiar();
      aviso = `Ahora usas «${d.nombre}».`;
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      ocupado = false;
    }
  }

  // ---- Importar y exportar ----
  async function importar() {
    if (ocupado) return;
    const ruta = await open({ multiple: false, directory: false, filters: [{ name: "Conjunto de parámetros", extensions: ["json"] }] });
    if (typeof ruta !== "string") return;
    ocupado = true;
    aviso = "";
    try {
      const id = await partida.importarConjunto(ruta);
      onError("");
      aviso = `Importado «${partida.conjuntos.guardados.find((c) => c.id === id)?.nombre ?? "conjunto"}». Todavía no lo usas: elígelo arriba cuando quieras.`;
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err)); // el motivo concreto: no es un conjunto, es de otra versión, algo no vale...
    } finally {
      ocupado = false;
    }
  }

  const nombreDeArchivo = (nombre: string) => nombre.replace(/[\\/:*?"<>|]+/g, "_").trim() || "conjunto";

  async function exportar(c: ResumenConjunto) {
    if (ocupado) return;
    const ruta = await save({
      defaultPath: `${nombreDeArchivo(c.nombre)}.openroll.json`,
      filters: [{ name: "Conjunto de parámetros", extensions: ["json"] }]
    });
    if (typeof ruta !== "string") return;
    ocupado = true;
    aviso = "";
    try {
      await partida.exportarConjunto(c.id, ruta);
      onError("");
      aviso = `Exportado «${c.nombre}».`;
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      ocupado = false;
    }
  }

  async function anadirDeSerie(clave: string) {
    if (ocupado) return;
    ocupado = true;
    aviso = "";
    try {
      await partida.anadirConjuntoDeSerie(clave);
      onError("");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      ocupado = false;
    }
  }

  // ---- Renombrar (doble clic) y borrar ----
  let renombrando = $state<{ id: number; texto: string } | null>(null);
  let borrandoId = $state<number | null>(null);

  async function confirmarRenombrar() {
    const r = renombrando;
    renombrando = null; // antes de esperar: el blur que provoca quitar el campo no vuelve a entrar
    if (!r) return;
    const actual = partida.conjuntos.guardados.find((c) => c.id === r.id);
    if (!actual || !r.texto.trim() || r.texto.trim() === actual.nombre) return;
    try {
      await partida.renombrarConjunto(r.id, r.texto);
      onError("");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  async function borrar(id: number) {
    borrandoId = null;
    try {
      await partida.borrarConjunto(id);
      onError("");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  function enfocar(el: HTMLInputElement) {
    el.focus();
    el.select();
  }
</script>

<div class="barra">
  <label class="etiqueta" for="conjunto-activo">Conjunto:</label>
  <select
    id="conjunto-activo"
    class="select"
    title={ayuda("El conjunto de parámetros y variables que usa esta partida (el rol que se juega). Cambiarlo reinicia los valores de las fichas")}
    value={String(partida.conjuntos.activo)}
    onchange={(e) => {
      const v = e.currentTarget.value;
      e.currentTarget.value = String(partida.conjuntos.activo); // hasta confirmar, se sigue en el mismo
      pedirCambio(v);
    }}
  >
    {#each partida.conjuntos.guardados as c (c.id)}
      <option value={String(c.id)}>{c.nombre}{c.id === partida.conjuntos.activo ? " (en uso)" : ""}</option>
    {/each}
    {#if deSerieNuevos.length > 0}
      <optgroup label="Disponibles en OpenRoll">
        {#each deSerieNuevos as s (s.clave)}
          <option value={"serie:" + s.clave}>{s.nombre}</option>
        {/each}
      </optgroup>
    {/if}
  </select>
  <button type="button" class="gestionar" class:abierto={gestionando} aria-expanded={gestionando} onclick={() => (gestionando = !gestionando)}>
    Gestionar…
  </button>
</div>

{#if confirmando}
  <div
    class="confirmar"
    role="alertdialog"
    aria-label="Cambiar de conjunto"
    tabindex="-1"
    onkeydown={(e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        confirmando = null;
      }
    }}
  >
    <p>
      Vas a usar «{confirmando.nombre}». <strong>Los parámetros y variables específicos de TODAS las fichas cambiarán</strong>: sus
      valores, la variación de − y + y el valor actual de los parámetros con máximo serán los que tenían con ese conjunto (si ya lo
      usaste antes) o los de por defecto (si es la primera vez).
      {#if activo}Lo que tienen ahora se guarda en «{activo.nombre}»: si vuelves a él, lo recuperas.{/if}
      Los conjuntos son independientes: no se comparte ningún valor entre ellos.
      {#if partida.entidadEnEdicion && !partida.editorEsBorrador}La ficha que tienes abierta se cerrará sin guardar.{/if}
    </p>
    {#if partida.hayFichaNuevaAbierta}
      <p class="bloqueo">Cierra antes la ficha nueva que tienes abierta (guárdala o cancélala).</p>
    {/if}
    <div class="botones">
      <button type="button" class="no" onclick={() => (confirmando = null)}>Cancelar</button>
      <button type="button" class="si" disabled={ocupado || partida.hayFichaNuevaAbierta} onclick={cambiar}>Cambiar de conjunto</button>
    </div>
  </div>
{/if}

{#if aviso}
  <p class="aviso" role="status">{aviso}</p>
{/if}

{#if gestionando}
  <div class="gestor">
    <div class="fila-importar">
      <button type="button" class="importar" disabled={ocupado} onclick={importar} title={ayuda("Trae un conjunto de otro archivo. No sustituye lo que tienes: se añade a la lista")}>
        Importar desde archivo…
      </button>
      <span class="nota">Se añade a la lista; no borra nada.</span>
    </div>

    <div class="lista">
      {#each partida.conjuntos.guardados as c (c.id)}
        {@const enUso = c.id === partida.conjuntos.activo}
        <div class="conjunto" class:en-uso={enUso}>
          <div class="textos">
            {#if renombrando?.id === c.id}
              <input
                class="renombrar"
                maxlength="60"
                aria-label="Nombre del conjunto"
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
              <span class="nombre" role="presentation" title={ayuda("Doble clic para cambiarle el nombre")} ondblclick={() => (renombrando = { id: c.id, texto: c.nombre })}>
                {c.nombre}
                {#if enUso}<span class="marca">en uso</span>{/if}
                {#if c.origen}<span class="origen" title={ayuda("Viene de un conjunto que trae el programa")}>de serie</span>{/if}
              </span>
            {/if}
            <span class="resumen">
              {c.parametros} parámetro{c.parametros === 1 ? "" : "s"} · {c.variables} variable{c.variables === 1 ? "" : "s"}{c.descripcion ? " · " + c.descripcion : ""}
            </span>
          </div>
          <div class="acciones">
            {#if !enUso}
              <button type="button" onclick={() => pedirCambio(String(c.id))}>Usar</button>
            {/if}
            <button type="button" disabled={ocupado} onclick={() => exportar(c)}>Exportar</button>
            {#if !enUso}
              {#if borrandoId === c.id}
                <button type="button" class="peligro" onclick={() => borrar(c.id)}>¿Borrar?</button>
                <button type="button" onclick={() => (borrandoId = null)}>No</button>
              {:else}
                <button type="button" class="peligro" onclick={() => (borrandoId = c.id)}>Borrar</button>
              {/if}
            {/if}
          </div>
        </div>
      {/each}

      {#if deSerieNuevos.length > 0}
        <span class="subtitulo">Disponibles en OpenRoll</span>
        {#each deSerieNuevos as s (s.clave)}
          <div class="conjunto">
            <div class="textos">
              <span class="nombre">{s.nombre}</span>
              <span class="resumen">{s.descripcion}</span>
            </div>
            <div class="acciones">
              <button type="button" onclick={() => pedirCambio("serie:" + s.clave)}>Usar</button>
              <button type="button" disabled={ocupado} onclick={() => anadirDeSerie(s.clave)} title={ayuda("Lo añade a la lista sin usarlo")}>Añadir</button>
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .barra { display: flex; align-items: center; gap: 6px; }
  .etiqueta { color: #a1a1aa; font-size: 10px; font-weight: 600; }
  .select {
    flex: 1; min-width: 0; padding: 3px 5px; border-radius: 4px; font-size: 11px;
    background: #09090b; border: 1px solid #3f3f46; color: white;
  }
  .gestionar {
    background: #27272a; border: 1px solid #3f3f46; color: #d4d4d8; border-radius: 4px; padding: 3px 9px;
    font-size: 11px; font-weight: 600; cursor: pointer;
  }
  .gestionar:hover, .gestionar.abierto { border-color: #3b82f6; color: white; }

  .confirmar {
    display: flex; flex-direction: column; gap: 6px; padding: 8px 10px; border-radius: 5px;
    background: rgba(120, 80, 10, 0.3); border: 1px solid #854d0e; color: #fde68a; font-size: 11px;
  }
  .confirmar p { margin: 0; line-height: 1.4; }
  .confirmar strong { color: #fff; }
  .confirmar .bloqueo { color: #fca5a5; font-weight: 700; }
  .botones { display: flex; justify-content: flex-end; gap: 6px; }
  .botones button { border: none; border-radius: 4px; padding: 4px 12px; font-size: 11px; font-weight: 700; cursor: pointer; }
  .botones .si { background: #2563eb; color: white; }
  .botones .si:hover:not(:disabled) { background: #1d4ed8; }
  .botones .si:disabled { opacity: 0.4; cursor: default; }
  .botones .no { background: #27272a; color: #d4d4d8; }

  .aviso { margin: 0; color: #86efac; font-size: 10px; }

  .gestor { display: flex; flex-direction: column; gap: 6px; padding: 8px; background: #111113; border: 1px solid #27272a; border-radius: 6px; }
  .fila-importar { display: flex; align-items: center; gap: 8px; }
  .importar {
    background: #2563eb; border: none; color: white; border-radius: 4px; padding: 4px 10px; font-size: 11px; font-weight: 600; cursor: pointer;
  }
  .importar:hover:not(:disabled) { background: #1d4ed8; }
  .importar:disabled { opacity: 0.5; cursor: default; }
  .nota { color: #71717a; font-size: 10px; }
  .lista { display: flex; flex-direction: column; gap: 4px; max-height: 240px; overflow-y: auto; }
  .subtitulo { margin-top: 4px; color: #71717a; font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.03em; }
  .conjunto {
    display: flex; align-items: center; gap: 8px; padding: 5px 8px; background: #18181b; border: 1px solid #27272a; border-radius: 5px;
  }
  .conjunto.en-uso { border-color: #3b82f6; }
  .textos { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .nombre { font-weight: 600; font-size: 11px; cursor: text; }
  .marca { margin-left: 4px; color: #60a5fa; font-size: 9px; font-weight: 700; text-transform: uppercase; }
  .origen { margin-left: 4px; color: #a1a1aa; font-size: 9px; }
  .resumen { color: #71717a; font-size: 9px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .renombrar { width: 100%; padding: 2px 5px; border-radius: 4px; font-size: 11px; background: #09090b; border: 1px solid #3b82f6; color: white; box-sizing: border-box; }
  .acciones { display: flex; gap: 4px; flex-shrink: 0; }
  .acciones button {
    background: #27272a; border: 1px solid #3f3f46; color: #d4d4d8; border-radius: 4px; padding: 2px 8px; font-size: 10px; font-weight: 600; cursor: pointer;
  }
  .acciones button:hover:not(:disabled) { border-color: #3b82f6; color: white; }
  .acciones button:disabled { opacity: 0.5; cursor: default; }
  .acciones .peligro { color: #f87171; }
  .acciones .peligro:hover:not(:disabled) { border-color: #ef4444; color: #fca5a5; }
</style>
