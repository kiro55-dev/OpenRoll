<!-- src/routes/componentes/panel-izquierdo/BancoEntidades.svelte -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { onMount } from "svelte";
  import { invoke } from "$lib/backend";
  import { partida, type Entidad } from "$lib/partida.svelte";
  import { miniaturas } from "$lib/miniaturas.svelte";
  import { esImagen, importarImagen } from "$lib/imagenes";
  import { alSoltarArchivos } from "$lib/soltarArchivos";
  import { arrastre } from "$lib/arrastreFicha.svelte";
  import { arrastreJugador } from "$lib/arrastreJugador.svelte";
  import { esJugador } from "$lib/modo";
  import { esArchivoPlantilla, fichaDePlantilla, type PlantillaLeida } from "$lib/plantillaFicha";
  import { leerArchivoPlantilla } from "$lib/plantillaIo";

  // Soltar aquí una instancia la elimina: se marca el banco mientras pasa por encima
  const recibeInstancia = $derived(arrastre.actual?.zona === "banco" && "instancia" in arrastre.actual.origen);

  // Filtros
  let busqueda = $state("");
  let filtroZona = $state<"todos" | "actual">("todos");
  let filtroTipo = $state<"todos" | "Enemigo" | "NPC" | "Jugador" | "Objeto">("todos");

  // Control de qué filas están visibles en el scroll para descargar imágenes
  let visibles = $state<Record<number, boolean>>({});

  // Filtrado combinado: Nombre + Filtro de Tipo + Filtro de Zona original
  let entidadesFiltradas = $derived(
    partida.banco.filter((e) => {
      const coincideNombre = e.nombre.toLowerCase().includes(busqueda.toLowerCase());
      const coincideZona = filtroZona === "todos" || e.zona_id === partida.zonaId;
      const coincideTipo = filtroTipo === "todos" || e.tipo === filtroTipo;
      return coincideNombre && coincideZona && coincideTipo;
    })
  );

  // Pide la miniatura de las fichas visibles (también si su foto cambia al guardar).
  // Las filas que salen de pantalla dejan de pintar su <img>, así que el webview libera esos píxeles.
  $effect(() => {
    for (const e of entidadesFiltradas) {
      if (visibles[e.id] && e.foto_url) miniaturas.asegurarFicha(e);
    }
  });

  // IntersectionObserver: desmonta la imagen si la fila sale del scroll
  function vigilarVisibilidad(node: HTMLElement, id: number) {
    const root = node.closest(".sub-entidades");
    const observer = new IntersectionObserver(
      ([entry]) => {
        visibles[id] = entry.isIntersecting;
        if (entry.isIntersecting) {
          const ent = partida.banco.find((b) => b.id === id);
          if (ent?.foto_url) {
            miniaturas.asegurarFicha(ent);
          }
        }
      },
      { root, rootMargin: "40px" }
    );

    observer.observe(node);

    return {
      destroy() {
        delete visibles[id];
        observer.disconnect();
      }
    };
  }

  // Selección múltiple (Ctrl+clic). Arrastrar una seleccionada arrastra todas; Supr las manda a la papelera.
  const seleccion = $derived(partida.idsBancoSeleccionados);

  /** Lo que se arrastra desde esta fila: toda la selección si la fila está en ella, si no solo la fila. */
  function grupoDe(item: Entidad): Entidad[] {
    if (seleccion.length < 2 || !seleccion.includes(item.id)) return [item];
    return seleccion.flatMap((id) => partida.banco.filter((b) => b.id === id));
  }

  function alPulsarFila(e: PointerEvent, item: Entidad) {
    if (e.ctrlKey || e.metaKey) return; // Ctrl+clic solo selecciona (lo hace el clic)
    const grupo = grupoDe(item);
    arrastre.preparar(e, grupo.length > 1 ? { entidades: grupo } : { entidad: item });
  }

  // Con otra ficha abierta en el editor, esa se guarda y la ventana pasa a esta (ver `abrirEditorEntidad`)
  function alHacerClicFila(e: MouseEvent, item: Entidad) {
    if (arrastre.esClicDeArrastre()) return;
    if (e.ctrlKey || e.metaKey) return partida.alternarSeleccionBanco(item.id);
    partida.limpiarSeleccionBanco();
    void partida.abrirEditorEntidad(item);
  }

  // ---- Plantilla de ficha soltada en el banco (ver $lib/plantillaFicha) ----
  /** Una plantilla de una ficha que YA está en el banco: se pregunta si va a esa o a una nueva. */
  let plantillaDeFicha = $state<{ plantilla: PlantillaLeida; ficha: Entidad } | null>(null);

  async function soltarPlantilla(origen: string | File) {
    try {
      const plantilla = await leerArchivoPlantilla(origen);
      // La ficha de la plantilla: la que guarda su identidad (ya se importó antes) o aquella de la que se exportó
      const existente = fichaDePlantilla(plantilla, partida.banco);
      if (existente) plantillaDeFicha = { plantilla, ficha: existente };
      else await partida.abrirConPlantilla(plantilla, null);
    } catch (err) {
      partida.avisar(err instanceof Error ? err.message : String(err));
    }
  }

  function decidirPlantilla(destino: "nueva" | "existente") {
    const d = plantillaDeFicha;
    plantillaDeFicha = null;
    if (!d) return;
    // Una ficha NUEVA con los datos de una plantilla que ya es de otra: no hereda la identidad (no habría dos fichas con la misma)
    if (destino === "nueva") void partida.abrirConPlantilla({ ...d.plantilla, plantilla_id: null, id_ficha: null }, null);
    else void partida.abrirConPlantilla(d.plantilla, d.ficha);
  }

  // Drag & drop de imagen desde el explorador del SO: ficha nueva con esa foto (la abierta, si hay, se guarda)
  onMount(() =>
    alSoltarArchivos(async ([ruta], el) => {
      if (esArchivoPlantilla(ruta) && el?.closest(".banco")) return soltarPlantilla(ruta);
      if (!esImagen(ruta, "ficha") || !el?.closest(".banco")) return;
      const fotoUrl = await importarImagen(ruta, "ficha");
      if (!(await partida.abrirEditorEntidad(null, fotoUrl))) {
        // No se abrió el borrador (no se pudo guardar la abierta, o crearlo falló): la imagen no la usa nadie
        invoke("descartar_archivos_borrador", { rutas: [fotoUrl] }).catch(() => {});
      }
    })
  );

  // Un jugador arrastra la imagen desde su navegador (no hay rutas del disco): se sube al host y se abre la ficha nueva
  function alSoltarEnElNavegador(e: DragEvent) {
    if (!esJugador) return;
    e.preventDefault();
    const archivo = e.dataTransfer?.files?.[0];
    if (archivo && esArchivoPlantilla(archivo.name)) void soltarPlantilla(archivo);
    else if (archivo) void ficharDesdeArchivo(archivo);
  }

  async function ficharDesdeArchivo(archivo: File) {
    try {
      const fotoUrl = await importarImagen(archivo, "ficha");
      if (!(await partida.abrirEditorEntidad(null, fotoUrl))) {
        invoke("descartar_archivos_borrador", { rutas: [fotoUrl] }).catch(() => {});
      }
    } catch (err) {
      partida.avisar(`No se pudo usar la imagen: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="banco"
  data-destino="banco"
  class:destino={recibeInstancia}
  ondragover={(e) => esJugador && e.preventDefault()}
  ondrop={alSoltarEnElNavegador}
>
  <!-- Zona de búsqueda y filtros -->
  <div class="sub-busqueda">
    <div class="fila-cabecera">
      <span class="titulo-mini">BANCO DE FICHAS</span>
      <button type="button" class="btn-anadir" onclick={() => void partida.abrirEditorEntidad(null)}>
        + Añadir Ficha
      </button>
    </div>

    <input type="text" placeholder="Buscar ficha..." bind:value={busqueda} class="input-buscar" />

    <!-- Filtros rápidos unificados en dos filas -->
    <div class="filtros-rapidos">
      <!-- Fila superior: Todas las zonas vs Zona actual -->
      <div class="fila-superior">
        <button
          type="button"
          class:activo={filtroZona === "todos"}
          onclick={() => (filtroZona = "todos")}
        >
          Todas
        </button>
        <button
          type="button"
          class:activo={filtroZona === "actual"}
          onclick={() => (filtroZona = "actual")}
        >
          Zona actual
        </button>
      </div>

      <!-- Fila inferior: Todos / Enemigos / NPCs -->
      <div class="fila-inferior">
        <button
          type="button"
          class:activo={filtroTipo === "todos"}
          onclick={() => (filtroTipo = "todos")}
        >
          Todos
        </button>
        <button
          type="button"
          class:activo={filtroTipo === "Enemigo"}
          onclick={() => (filtroTipo = "Enemigo")}
        >
          Enemigos
        </button>
        <button
          type="button"
          class:activo={filtroTipo === "NPC"}
          onclick={() => (filtroTipo = "NPC")}
        >
          NPCs
        </button>
        <button
          type="button"
          class:activo={filtroTipo === "Jugador"}
          onclick={() => (filtroTipo = "Jugador")}
        >
          Jugadores
        </button>
        <button
          type="button"
          class:activo={filtroTipo === "Objeto"}
          onclick={() => (filtroTipo = "Objeto")}
        >
          Objetos
        </button>
      </div>
    </div>
  </div>

  <!-- Contenedor con scroll para las fichas -->
  <div class="sub-entidades">
    {#each entidadesFiltradas as item (item.id)}
      {@const estaEnPantalla = visibles[item.id] ?? false}
      {@const mini = estaEnPantalla && item.foto_url ? miniaturas.deFicha(item) : ""}

      <button
        type="button"
        class="fila-entidad"
        class:seleccionada={seleccion.includes(item.id)}
        class:recibe-jugador={arrastreJugador.actual?.idEntidad === item.id}
        data-id-entidad={item.id}
        use:vigilarVisibilidad={item.id}
        onpointerdown={(e) => alPulsarFila(e, item)}
        onclick={(e) => alHacerClicFila(e, item)}
        title={ayuda("Clic: editar ficha\nCtrl+clic: seleccionar varias\nSupr: borrar las seleccionadas\nArrastrar: al tablero o a «Fichas activas»\nArrastrar con click derecho: La instancia nace oculta")}
      >
        {#if item.tipo === "NPC"}
          <div class="contenedor-rombo">
            {#if estaEnPantalla && mini}
              <img src={mini} alt="" class="img-rombo" draggable="false" />
            {/if}
          </div>
        {:else if item.tipo === "Enemigo"}
          <div class="contenedor-cuadrado">
            {#if estaEnPantalla && mini}
              <img src={mini} alt="" class="img-cuadrado" draggable="false" />
            {/if}
          </div>
        {:else if item.tipo === "Objeto"}
          <div class="contenedor-hexagono">
            <div class="interior-hexagono">
              {#if estaEnPantalla && mini}
                <img src={mini} alt="" class="img-hexagono" draggable="false" />
              {/if}
            </div>
          </div>
        {:else}
          <div class="contenedor-circulo">
            {#if estaEnPantalla && mini}
              <img src={mini} alt="" class="img-circulo" draggable="false" />
            {/if}
          </div>
        {/if}

        <span class="texto-recortado">{item.nombre}</span>
      </button>
    {/each}

    {#if entidadesFiltradas.length === 0}
      <span class="sin-fichas">No hay fichas</span>
    {/if}
  </div>
  {#if plantillaDeFicha}
    <!-- La plantilla trae el id de una ficha que ya está en el banco: ¿a esa o a una nueva? -->
    <div class="aviso-plantilla" role="dialog" aria-label="Importar plantilla">
      <p>Estos datos corresponden con la ficha «{plantillaDeFicha.ficha.nombre}». ¿Guardar los cambios sobre esa ficha o crear una nueva?</p>
      <div class="botones-aviso">
        <button type="button" class="btn-aviso" onclick={() => (plantillaDeFicha = null)}>Cancelar</button>
        <button type="button" class="btn-aviso" onclick={() => decidirPlantilla("nueva")}>Nueva ficha</button>
        <button type="button" class="btn-aviso principal" onclick={() => decidirPlantilla("existente")}>Ficha existente</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .aviso-plantilla {
    position: absolute; left: 8px; right: 8px; bottom: 8px; z-index: 20; display: flex; flex-direction: column; gap: 8px;
    padding: 10px; border-radius: 6px; background: #18181b; border: 1px solid #3f3f46; box-shadow: 0 10px 25px rgba(0, 0, 0, 0.6);
    color: #e4e4e7; font-size: 12px;
  }
  .aviso-plantilla p { margin: 0; }
  .botones-aviso { display: flex; justify-content: flex-end; gap: 6px; flex-wrap: wrap; }
  .btn-aviso { background: #27272a; color: #e4e4e7; border: 1px solid #3f3f46; border-radius: 4px; padding: 4px 10px; font-size: 11px; font-weight: 600; cursor: pointer; }
  .btn-aviso:hover { border-color: #52525b; }
  .btn-aviso.principal { background: #2563eb; border-color: #2563eb; color: white; }
  .btn-aviso.principal:hover { background: #1d4ed8; }
  .banco {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
    overflow: hidden;
  }
  .banco.destino { outline: 1px dashed #ef4444; outline-offset: -1px; background: rgba(239, 68, 68, 0.06); }
  .sub-busqueda { display: flex; flex-direction: column; gap: 4px; }
  .fila-cabecera { display: flex; justify-content: space-between; align-items: center; }
  .titulo-mini { font-size: 10px; font-weight: bold; color: #71717a; }
  .btn-anadir {
    background: #27272a;
    border: 1px dashed #52525b;
    color: #60a5fa;
    border-radius: 3px;
    font-size: 10px;
    font-weight: bold;
    padding: 2px 6px;
    cursor: pointer;
  }
  .btn-anadir:hover { background: #3f3f46; color: white; }

  .input-buscar {
    background: #09090b;
    border: 1px solid #3f3f46;
    color: white;
    padding: 3px 5px;
    border-radius: 4px;
    font-size: 11px;
  }

  /* Reset completo para los botones de filtro rápido */
  .filtros-rapidos {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
  }

  .fila-superior,
  .fila-inferior {
    display: flex;
    gap: 3px;
    width: 100%;
  }
  /* Estilo base compartido para todos los botones de ambas filas */
  .filtros-rapidos button {
    flex: 1;
    background: #1c1c1f;
    border: 1px solid #27272a;
    color: #a1a1aa;
    font-size: 10px;
    padding: 3px 0;
    border-radius: 3px;
    cursor: pointer;
    text-align: center;
    line-height: normal;
  }

  .filtros-rapidos button:hover {
    background: #27272a;
    color: #f4f4f5;
  }

  /* Estado seleccionado (activo) */
  .filtros-rapidos button.activo {
    background: #27272a;
    color: #ffffff;
    border-color: #3f3f46;
    font-weight: bold;
  }

  .sub-entidades {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-right: 2px;
  }

  /* Fila de ficha en el banco */
  .fila-entidad {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #1f1f23;
    padding: 4px 6px;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    width: 100%;
    color: #f4f4f5;
    font: inherit;
    text-align: left;
    height: 32px;
    box-sizing: border-box;
    flex-shrink: 0;
  }
  .fila-entidad:hover {
    background: #27272a;
    border-color: #3f3f46;
  }
  .fila-entidad.seleccionada {
    background: #1e2a3d;
    border-color: #3b82f6;
  }
  /* Se está soltando encima un jugador (pasará a controlar esta ficha) */
  .fila-entidad.recibe-jugador { border-color: #22c55e; background: rgba(34, 197, 94, 0.12); }

  /* Rombo azul para NPC */
  .contenedor-rombo {
    width: 18px;
    height: 18px;
    transform: rotate(45deg);
    overflow: hidden;
    border: 2px solid #3b82f6;
    border-radius: 2px;
    background: #1e3a8a;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-left: 2px;
    margin-right: 2px;
  }
  .img-rombo {
    width: 142%;
    height: 142%;
    object-fit: cover;
    transform: rotate(-45deg);
    pointer-events: none;
  }

  /* Cuadrado rojo para Enemigos */
  .contenedor-cuadrado {
    width: 18px;
    height: 18px;
    border: 2px solid #ef4444;
    border-radius: 3px;
    overflow: hidden;
    background: #7f1d1d;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-left: 2px;
    margin-right: 2px;
  }
  .img-cuadrado {
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }
  /* Hexágono gris para Objetos: el borde es la capa de fuera, que asoma 2px alrededor de la de dentro */
  .contenedor-hexagono {
    width: 20px;
    height: 20px;
    padding: 2px;
    box-sizing: border-box;
    background: #6d6b65;
    clip-path: polygon(50% 0, 100% 25%, 100% 75%, 50% 100%, 0 75%, 0 25%);
    flex-shrink: 0;
    margin-left: 1px;
    margin-right: 1px;
  }
  .interior-hexagono {
    width: 100%;
    height: 100%;
    background: #3f3f46;
    clip-path: polygon(50% 0, 100% 25%, 100% 75%, 50% 100%, 0 75%, 0 25%);
    overflow: hidden;
  }
  .img-hexagono {
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
    display: block;
  }
  .contenedor-circulo {
    width: 18px;
    height: 18px;
    border: 2px solid #078c09;
    border-radius: 50%;
    overflow: hidden;
    background: #23e612;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-left: 2px;
    margin-right: 2px;
  }
  .img-circulo {
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }
  .texto-recortado {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
  }
  .sin-fichas {
    color: #52525b;
    font-size: 10px;
    text-align: center;
    margin-top: 10px;
  }
</style>