<!-- src/routes/componentes/ventana-parametros/ConjuntosParametros.svelte -->
<!-- El conjunto de parámetros que usa la partida (parámetros visuales + variables), arriba de la ventana de parámetros: un
     selector para cambiar de conjunto y «Gestionar…» para importar (desde una plantilla de ficha, también arrastrándola a la ventana),
     exportar la plantilla, renombrar y borrar los que guarda. Un conjunto
     define el rol que se juega (D&D…): la partida guarda varios y usa uno. Cada conjunto guarda lo que tenían las fichas cuando lo
     dejaste y lo recupera al volver; nada se comparte de uno a otro. Como no se pierde nada, cambiar de conjunto no pide confirmar:
     debajo del selector hay un texto informativo. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type ResumenBiblioteca, type ResumenConjunto } from "$lib/partida.svelte";
  import { esArchivoPlantilla } from "$lib/plantillaFicha";
  import { exportarPlantilla } from "$lib/plantillaIo";
  import { alSoltarArchivos } from "$lib/soltarArchivos";
  import { edicionVariables } from "$lib/edicionVariables.svelte";

  let { onError, alCambiar }: { onError: (mensaje: string) => void; alCambiar: () => void } = $props();

  const activo = $derived(partida.conjuntos.guardados.find((c) => c.id === partida.conjuntos.activo));
  const deSerieNuevos = $derived(partida.conjuntos.de_serie.filter((s) => !s.ya_anadido));

  let gestionando = $state(false);
  let ocupado = $state(false);
  /** Lo último que se hizo bien (importado, exportado...), para decírselo. */
  let aviso = $state("");

  // ---- Cambiar de conjunto (directo: lo que tenían las fichas se guarda en el que se deja y vuelve al regresar) ----
  async function pedirCambio(valor: string) {
    if (ocupado || partida.hayFichaNuevaAbierta) return;
    aviso = "";
    // Las filas con cambios sin guardar dejarían de corresponder a nada en el otro conjunto
    if (edicionVariables.hayCambios) {
      onError("Guarda o descarta los cambios sin guardar de las variables antes de cambiar de conjunto");
      return;
    }
    let nombre = "";
    let clave: string | null = null;
    let id = 0;
    if (valor.startsWith("serie:")) {
      const s = partida.conjuntos.de_serie.find((x) => x.clave === valor.slice(6));
      if (!s) return;
      clave = s.clave;
      nombre = s.nombre;
    } else {
      const c = partida.conjuntos.guardados.find((x) => x.id === Number(valor));
      if (!c || c.id === partida.conjuntos.activo) return;
      id = c.id;
      nombre = c.nombre;
    }
    ocupado = true;
    try {
      await partida.usarConjunto(clave ? await partida.anadirConjuntoDeSerie(clave) : id);
      onError("");
      alCambiar();
      aviso = `Ahora usas «${nombre}».`;
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      ocupado = false;
    }
  }

  // ---- Importar (desde una plantilla de ficha) y exportar (la plantilla) ----
  /** Importa el conjunto que lleva la plantilla `ruta` (rellena o no: de ella solo se lee el conjunto). */
  async function importarDe(ruta: string) {
    if (ocupado) return;
    ocupado = true;
    aviso = "";
    try {
      const id = await partida.importarConjunto(ruta);
      onError("");
      aviso = `Importado «${partida.conjuntos.guardados.find((c) => c.id === id)?.nombre ?? "conjunto"}». Todavía no lo usas: elígelo arriba cuando quieras.`;
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err)); // el motivo concreto: no es una plantilla, no lleva conjunto, algo no vale...
    } finally {
      ocupado = false;
    }
  }

  async function importar() {
    if (ocupado) return;
    const ruta = await open({ multiple: false, directory: false, filters: [{ name: "Plantilla de ficha de OpenRoll", extensions: ["html", "htm"] }] });
    if (typeof ruta === "string") await importarDe(ruta);
  }

  // Arrastrar la plantilla a cualquier parte de la ventana de parámetros hace lo mismo que el botón
  onMount(() =>
    alSoltarArchivos(async (rutas, el) => {
      if (!el?.closest("[data-ventana-parametros]")) return;
      const html = rutas.find(esArchivoPlantilla);
      if (html) await importarDe(html);
    })
  );

  async function exportarPlantillaEnBlanco() {
    if (ocupado) return;
    ocupado = true;
    aviso = "";
    try {
      if (await exportarPlantilla(null)) aviso = "Plantilla exportada.";
      onError("");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      ocupado = false;
    }
  }

  // ---- Biblioteca: conjuntos propios compartidos entre todas las partidas de este equipo ----
  let biblioteca = $state<ResumenBiblioteca[]>([]);
  let borrandoArchivo = $state<string | null>(null);

  async function cargarBiblioteca() {
    try {
      biblioteca = await partida.listarBiblioteca();
    } catch {
      biblioteca = [];
    }
  }

  $effect(() => {
    if (gestionando) void cargarBiblioteca();
  });

  async function accionBiblioteca(hacer: () => Promise<string>) {
    if (ocupado) return;
    ocupado = true;
    aviso = "";
    try {
      aviso = await hacer();
      onError("");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      ocupado = false;
      await cargarBiblioteca();
    }
  }

  const aBiblioteca = (c: ResumenConjunto) =>
    accionBiblioteca(async () => {
      await partida.guardarEnBiblioteca(c.id);
      return `«${c.nombre}» guardado en tu biblioteca: lo tendrás en todas tus partidas.`;
    });

  const deBiblioteca = (b: ResumenBiblioteca) =>
    accionBiblioteca(async () => {
      await partida.anadirDeBiblioteca(b.archivo);
      return `«${b.nombre}» añadido a esta partida. Todavía no lo usas: elígelo arriba cuando quieras.`;
    });

  async function borrarDeBiblioteca(archivo: string) {
    borrandoArchivo = null;
    await accionBiblioteca(async () => {
      await partida.borrarDeBiblioteca(archivo);
      return "Quitado de la biblioteca. Las partidas que ya lo tengan lo conservan.";
    });
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
    disabled={ocupado || partida.hayFichaNuevaAbierta}
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

<p class="info">
  {#if partida.hayFichaNuevaAbierta}
    Cierra antes la ficha nueva que tienes abierta (guárdala o cancélala) para poder cambiar de conjunto.
  {:else}
    Los parámetros y variables de las fichas cambiarán. Los valores que puedan contener no se pierden: puedes volver a poner el
    conjunto que tenías y se recupera.
  {/if}
</p>

{#if aviso}
  <p class="aviso" role="status">{aviso}</p>
{/if}

{#if gestionando}
  <div class="gestor">
    <!-- La plantilla de ficha en blanco (un .html para rellenar fuera de OpenRoll) con los parámetros y variables en uso -->
    <div class="fila-importar">
      <button
        type="button"
        class="importar"
        disabled={ocupado}
        onclick={exportarPlantillaEnBlanco}
        title={ayuda("Guarda una plantilla de ficha en blanco (un .html) con todo el conjunto en uso. Se rellena con cualquier navegador, sin OpenRoll, y después se arrastra al banco de fichas; la misma plantilla sirve para llevar el conjunto a otra partida")}
      >
        Exportar plantilla de los parámetros actuales
      </button>
    </div>
    <div class="fila-importar">
      <button type="button" class="importar" disabled={ocupado} onclick={importar} title={ayuda("Trae el conjunto que lleva dentro una plantilla de ficha (rellena o en blanco: del personaje no se coge nada). No sustituye lo que tienes: se añade a la lista")}>
        Importar desde plantilla…
      </button>
      <span class="nota">O arrastra la plantilla a esta ventana. Se añade a la lista; no borra nada.</span>
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
            <button type="button" disabled={ocupado} onclick={() => aBiblioteca(c)} title={ayuda("Lo guarda en tu biblioteca para usarlo en cualquier partida. Si ya hay uno con ese nombre, lo actualiza")}>A mi biblioteca</button>
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

      {#if biblioteca.length > 0}
        <span class="subtitulo">Mi biblioteca (en todas mis partidas)</span>
        {#each biblioteca as b (b.archivo)}
          <div class="conjunto">
            <div class="textos">
              <span class="nombre">{b.nombre}</span>
              <span class="resumen">
                {b.parametros} parámetro{b.parametros === 1 ? "" : "s"} · {b.variables} variable{b.variables === 1 ? "" : "s"}{b.descripcion ? " · " + b.descripcion : ""}
              </span>
            </div>
            <div class="acciones">
              <button type="button" disabled={ocupado} onclick={() => deBiblioteca(b)} title={ayuda("Lo copia a esta partida sin usarlo")}>Añadir</button>
              {#if borrandoArchivo === b.archivo}
                <button type="button" class="peligro" onclick={() => borrarDeBiblioteca(b.archivo)}>¿Quitar?</button>
                <button type="button" onclick={() => (borrandoArchivo = null)}>No</button>
              {:else}
                <button type="button" class="peligro" onclick={() => (borrandoArchivo = b.archivo)} title={ayuda("Solo lo quita de la biblioteca; las partidas que ya lo tengan lo conservan")}>Quitar</button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}

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

  .info { margin: 0; color: #71717a; font-size: 10px; line-height: 1.4; }

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
