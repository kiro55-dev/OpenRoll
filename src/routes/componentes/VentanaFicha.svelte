<!-- src/routes/componentes/VentanaFicha.svelte -->
<!-- Ventana flotante (no modal) para crear o editar una ficha. Aquí solo está la vista: la lógica
     (archivos de la sesión, textos, validación, guardar/cancelar/borrar) vive en $lib/editorFicha. -->
<script lang="ts" module>
  import { ayuda } from "$lib/ajustesVista.svelte";
  // Dónde estaba la última ventana: la que la sustituye (al abrir otra ficha) aparece en el mismo sitio
  let ultimaPosicion = { x: 260, y: 70 };
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { partida, type Entidad } from "$lib/partida.svelte";
  import { EditorFicha } from "$lib/editorFicha.svelte";
  import { esImagen } from "$lib/imagenes";
  import { alSoltarArchivos } from "$lib/soltarArchivos";
  import { moverVentana } from "$lib/moverVentana";
  import { esJugador } from "$lib/modo";
  import { LIMITE_VALOR_VARIABLE, etiquetaVariable, textoModificador } from "$lib/variables";
  import CajaFoto from "./ventana-ficha/CajaFoto.svelte";
  import CampoNumero from "./ventana-ficha/CampoNumero.svelte";
  import AdjuntosFicha from "./ventana-ficha/AdjuntosFicha.svelte";
  import { esArchivoPlantilla, type PlantillaLeida } from "$lib/plantillaFicha";
  import { exportarPlantilla, leerArchivoPlantilla } from "$lib/plantillaIo";

  let { entidad }: { entidad: Entidad } = $props();

  // +page monta la ventana de nuevo para cada ficha ({#key partida.sesionEditor}), así que basta con crearlo una vez.
  // svelte-ignore state_referenced_locally
  const editor = new EditorFicha(entidad, partida.editorEsBorrador);
  const ficha = editor.entidad;
  // Las definiciones no cambian mientras la ventana está abierta
  const campos = editor.campos;

  let ventana = $state<HTMLDivElement | null>(null);
  let pos = $state({ ...ultimaPosicion });
  $effect(() => {
    ultimaPosicion = { x: pos.x, y: pos.y };
  });

  function alTeclear(e: KeyboardEvent) {
    if (e.key === "Enter") {
      const objetivo = e.target as HTMLElement | null;
      if (objetivo?.tagName === "TEXTAREA") return;
      e.preventDefault();
      // En el valor de una variable, Enter no guarda: pasa a la fila siguiente y, en la última, suelta el foco
      if (objetivo instanceof HTMLInputElement && objetivo.type === "number" && objetivo.closest(".tabla-variables")) {
        const campos = [...(ventana?.querySelectorAll<HTMLInputElement>(".tabla-variables input[type='number']") ?? [])];
        const siguiente = campos[campos.indexOf(objetivo) + 1];
        if (siguiente) {
          siguiente.focus();
          siguiente.select();
        } else {
          objetivo.blur();
        }
        return;
      }
      editor.guardar();
    } else if (e.key === "Escape") {
      editor.cancelar();
    }
  }


  // ---- Plantilla de ficha (ver $lib/plantillaFicha): soltada en cualquier parte de la ventana ----
  /** Una plantilla de OTRA ficha esperando confirmación (se sobrescribirían los datos de esta). */
  let plantillaAjena = $state<PlantillaLeida | null>(null);
  let exportando = $state(false);

  /** La de esta misma ficha (o sobre una ficha nueva, que no tiene nada que perder) se aplica; la de otra pide confirmar. */
  function recibirPlantilla(p: PlantillaLeida) {
    if (editor.esBorrador || p.id_ficha === ficha.id) {
      plantillaAjena = null;
      void editor.aplicarPlantilla(p);
    } else {
      plantillaAjena = p;
    }
  }

  async function soltarPlantilla(origen: string | File): Promise<boolean> {
    try {
      recibirPlantilla(await leerArchivoPlantilla(origen));
      return true;
    } catch (err) {
      partida.errorEditor = err instanceof Error ? err.message : String(err);
      return false;
    }
  }

  // Un jugador suelta el archivo desde su navegador (en el host lo hace Tauri con rutas)
  function alSoltarEnElNavegador(e: DragEvent) {
    const archivo = e.dataTransfer?.files?.[0];
    if (!esJugador || !archivo || !esArchivoPlantilla(archivo.name)) return;
    e.preventDefault();
    e.stopPropagation();
    void soltarPlantilla(archivo);
  }

  async function exportar() {
    if (exportando) return;
    exportando = true;
    try {
      await exportarPlantilla($state.snapshot(ficha) as Entidad, editor.esBorrador ? null : ficha.id);
    } catch (err) {
      partida.errorEditor = `No se pudo exportar la plantilla: ${err instanceof Error ? err.message : String(err)}`;
    } finally {
      exportando = false;
    }
  }

  onMount(() => {
    ventana?.focus(); // para que Enter/Escape funcionen sin tocar la ventana con el ratón
    editor.abrirPestana(0);

    // Abierta desde una plantilla soltada en el banco: se rellena con ella
    const alAbrir = partida.plantillaAlAbrir;
    partida.plantillaAlAbrir = null;
    if (alAbrir) void editor.aplicarPlantilla(alAbrir);
    const dejarDeImportar = partida.registrarImportador(recibirPlantilla);

    // Abrir otra ficha con esta abierta la guarda primero (ver `partida.abrirEditorEntidad`)
    const dejarDeGuardar = partida.registrarEditor(() => editor.guardar());
    const dejarDeSoltar = alSoltarArchivos(async (rutas, el) => {
      // Una plantilla de ficha soltada en cualquier parte de la ventana; un .html que no lo sea, sobre los adjuntos, se adjunta
      const html = rutas.find(esArchivoPlantilla);
      if (html && el?.closest("[data-ventana-ficha]")) {
        if (await soltarPlantilla(html)) return;
        if (!el?.closest(".zona-archivos")) return;
        partida.errorEditor = "";
      }
      if (el?.closest(".caja-foto")) {
        const imagen = rutas.find((r) => esImagen(r, "ficha"));
        if (imagen) return editor.ponerFoto(imagen);
      } else if (el?.closest(".zona-archivos")) {
        return editor.importarArchivos(rutas);
      }
    });
    return () => {
      dejarDeGuardar();
      dejarDeSoltar();
      dejarDeImportar();
    };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="ventana-flotante"
  style="left: {pos.x}px; top: {pos.y}px;"
  role="dialog"
  aria-label="Ficha"
  data-ventana-ficha
  ondragover={(e) => esJugador && e.preventDefault()}
  ondrop={alSoltarEnElNavegador}
  tabindex="-1"
  onkeydown={alTeclear}
  bind:this={ventana}
>
  <div class="cabecera" role="presentation" onpointerdown={(e) => moverVentana(e, pos, ventana)}>
    <span class="icono-drag">⋮⋮</span>
    <span class="titulo">{editor.esBorrador ? "Nueva Ficha" : `Ficha: ${ficha.nombre}`}</span>
    <button type="button" class="btn-cerrar" aria-label="Cerrar" onclick={() => editor.cancelar()}>✕</button>
  </div>

  <div class="cuerpo-ficha">
    <div class="subpestanas" role="tablist" aria-label="Secciones de la ficha">
      <button type="button" role="tab" aria-selected={editor.seccion === "ficha"} class:activa={editor.seccion === "ficha"} onclick={() => (editor.seccion = "ficha")}>Ficha</button>
      <button type="button" role="tab" aria-selected={editor.seccion === "variables"} class:activa={editor.seccion === "variables"} onclick={() => (editor.seccion = "variables")}>Variables</button>
    </div>

    <!-- Las dos secciones siguen montadas (solo se oculta una): así no se pierde lo que se está escribiendo -->
    <div class="seccion" style:display={editor.seccion === "ficha" ? null : "none"}>
    <div class="fila-superior">
      <CajaFoto foto={ficha.foto_url} bind:encuadre={ficha.foto_encuadre} onCambio={() => editor.encuadreCambiado()} onElegir={(origen) => editor.ponerFoto(origen)} />

      <div class="columna-campos">
        <div class="fila">
          <label for="f-nombre">Nombre:</label>
          <input id="f-nombre" type="text" class="input-texto" maxlength="100" bind:value={ficha.nombre} />
        </div>

        <!-- Parámetros de la partida: los numéricos de dos en dos y, debajo de cada pareja, las
             casillas de los que cada ficha puede hacer públicos. Luego los de texto. -->
        {#each campos.numericos as par}
          <div class="fila separada">
            {#each par as d (d.id)}
              <CampoNumero
                etiqueta="{d.clave}{d.tiene_maximo ? ' max' : ''}:"
                sufijo={d.unidad ?? ""}
                soloLectura={editor.esCalculado(d.id)}
                titulo={editor.descripcionCalculo(d.id) ?? "Rueda del ratón para subir/bajar"}
                bind:value={() => editor.numero(d.id), (v) => editor.ponerNumero(d.id, v)}
              />
            {/each}
          </div>

          {#if par.some((d) => d.visibilidad === "Variable")}
            <div class="fila separada">
              {#each par.filter((d) => d.visibilidad === "Variable") as d (d.id)}
                <label class="check">
                  <span>Hacer pública la {d.clave.toLowerCase()}</span>
                  <input
                    type="checkbox"
                    bind:checked={() => editor.esPublico(d.id), (v) => editor.ponerPublico(d.id, v)}
                  />
                </label>
              {/each}
            </div>
          {/if}
        {/each}

        {#each campos.textos as d (d.id)}
          <div class="fila">
            <label for="f-param-{d.id}">{d.clave}:</label>
            <input
              id="f-param-{d.id}"
              type="text"
              class="input-texto"
              maxlength="2000"
              bind:value={() => editor.texto(d.id), (v) => editor.ponerTexto(d.id, v)}
            />
          </div>
        {/each}

        <div class="fila">
          <span class="etiqueta">Cuerpo:</span>
          <div class="cuerpo">
            <CampoNumero bind:value={editor.cuerpoX} min={1} max={255} estrecho titulo="Ancho en casillas (rueda del ratón para ajustar)" />
            <span class="separador-x">×</span>
            <CampoNumero bind:value={editor.cuerpoY} min={1} max={255} estrecho sufijo="m" titulo="Alto en casillas (rueda del ratón para ajustar)" />
          </div>
        </div>

        <div class="fila">
          <span class="etiqueta">Visión:</span>
          <CampoNumero bind:value={ficha.vision} min={0} max={10000} sufijo="m" titulo="Cuánto ve la ficha, en metros (por ahora no hace nada; rueda del ratón para ajustar)" />
        </div>

        <div class="fila">
          <label for="f-tipo">Tipo:</label>
          <select id="f-tipo" class="select" bind:value={ficha.tipo}>
            <option value="Enemigo">Enemigo</option>
            <option value="NPC">NPC</option>
            <option value="Jugador">Jugador</option>
            <option value="Objeto">Objeto</option>
          </select>

          {#if !esJugador}
            <label class="check" style="margin-left: 8px;">
              <input type="checkbox" bind:checked={ficha.es_unico} />
              <span title={ayuda("Si es único solo existirá una instancia en el mundo")}>Único</span>
            </label>
          {/if}
        </div>

        <!-- La zona, si es única y de quién es una ficha lo decide el host: el jugador no lo cambia -->
        {#if !esJugador}
          <div class="fila">
            <label for="f-zona">Zona asociada:</label>
            <select id="f-zona" class="select" bind:value={ficha.zona_id}>
              {#each partida.zonasOrdenadas as z (z.id)}
                <option value={z.id}>{z.nombre}</option>
              {/each}
            </select>
          </div>
        {/if}
      </div>
    </div>

    <AdjuntosFicha {editor} />
    </div>

    <!-- Variables: el host define cuáles hay (nombre, abreviatura, neutro y escala); aquí solo se pone el valor -->
    <div class="seccion seccion-variables" style:display={editor.seccion === "variables" ? null : "none"}>
      {#if editor.variables.length === 0}
        <p class="vacio">
          {esJugador ? "El host aún no ha definido variables." : "Aún no hay variables. Defínelas en ⚙ → «Editar parámetros de esta partida» → «Variables»."}
        </p>
      {:else}
        <!-- Arriba, los grupos como en su definición: «Todos» (por defecto) enseña una sección por grupo y cada grupo, solo el suyo -->
        {#if editor.gruposVariables.length > 1}
          <div class="grupos-ficha" role="tablist" aria-label="Grupos de variables">
            <button type="button" role="tab" class="grupo-ficha" class:activo={editor.grupoVistaEfectivo === null} aria-selected={editor.grupoVistaEfectivo === null} onclick={() => (editor.grupoVista = null)}>Todos</button>
            {#each editor.gruposVariables as grupo (grupo.id)}
              <button type="button" role="tab" class="grupo-ficha" class:activo={editor.grupoVistaEfectivo === grupo.id} aria-selected={editor.grupoVistaEfectivo === grupo.id} onclick={() => (editor.grupoVista = grupo.id)}>
                {grupo.nombre}{#if grupo.tipo === "Derivadas"}<span class="marca" aria-hidden="true">ƒ</span>{/if}
              </button>
            {/each}
          </div>
        {/if}

        <!-- Una sección por grupo: las bases con su valor y las derivadas ya calculadas, con los checks de sus opcionales -->
        {#each editor.gruposVariables as grupo (grupo.id)}
          {@const suyas = editor.variablesDe(grupo)}
          {#if editor.grupoVistaEfectivo !== null && editor.grupoVistaEfectivo === grupo.id && suyas.length === 0}
            <p class="vacio">Este grupo aún no tiene variables.</p>
          {/if}
          {#if suyas.length > 0 && (editor.grupoVistaEfectivo === null || editor.grupoVistaEfectivo === grupo.id)}
            {#if editor.grupoVistaEfectivo === null && editor.gruposVariables.length > 1}
              <h4 class="titulo-grupo">{grupo.nombre}</h4>
            {/if}
            <table class="tabla-variables">
              {#if grupo.tipo === "Base"}
                <thead>
                  <tr><th>Nombre</th><th>Abreviatura</th><th>Valor</th><th>Resultado</th></tr>
                </thead>
                <tbody>
                  {#each suyas as d (d.id)}
                    {@const resultado = editor.resultadoVariable(d)}
                    <tr>
                      <td class="nombre">{d.nombre}</td>
                      <td class="abreviatura">{d.abreviatura || "—"}</td>
                      <td>
                        <CampoNumero
                          min={-LIMITE_VALOR_VARIABLE}
                          max={LIMITE_VALOR_VARIABLE}
                          titulo="Neutro {d.neutro}, escala {d.escala} (rueda del ratón para ajustar)"
                          bind:value={() => editor.valorVariable(d.id), (v) => editor.ponerValorVariable(d.id, v)}
                        />
                      </td>
                      <td class="resultado" class:positivo={resultado !== null && resultado > 0} class:negativo={resultado !== null && resultado < 0}>
                        {resultado === null ? "—" : textoModificador(resultado)}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              {:else}
                <thead>
                  <tr><th>Nombre</th><th>Abreviatura</th><th>Opcionales</th><th>Resultado</th></tr>
                </thead>
                <tbody>
                  {#each suyas as d (d.id)}
                    {@const resultado = editor.resultadoVariable(d) ?? 0}
                    <tr>
                      <td class="nombre">{d.nombre} <span class="depende" title={ayuda("De qué variables base y parámetros depende")}>({editor.dependeDe(d)})</span></td>
                      <td class="abreviatura">{d.abreviatura || "—"}</td>
                      <td>
                       <div class="opcionales">
                        {#each d.opcionales as idOpcional (idOpcional)}
                          {@const opcional = editor.variables.find((v) => v.id === idOpcional)}
                          {#if opcional}
                            <!-- Check redondo: si está marcado, se suma «+ <abreviatura o nombre de la opcional>» -->
                            <label class="opcional" title={ayuda(`Sumar ${opcional.nombre} a ${d.nombre}`)}>
                              <input
                                type="checkbox"
                                class="redondo"
                                checked={editor.opcionalMarcada(d.id, idOpcional)}
                                onchange={() => editor.alternarOpcional(d.id, idOpcional)}
                              />
                              <span>+ {etiquetaVariable(opcional)}</span>
                            </label>
                          {/if}
                        {/each}
                        <!-- Parámetros visuales opcionales: si está marcado, su valor se suma tal cual -->
                        {#each editor.parametrosOpcionales(d) as p (p.id)}
                          <label class="opcional" title={ayuda(`Sumar ${p.clave} (su valor) a ${d.nombre}`)}>
                            <input
                              type="checkbox"
                              class="redondo"
                              checked={editor.parametroMarcado(d.id, p.id)}
                              onchange={() => editor.alternarParametro(d.id, p.id)}
                            />
                            <span>+ {p.clave}</span>
                          </label>
                        {/each}
                        <!-- Los rangos de la derivada (Sin entrenar +0, Entrenado +2...): cada ficha elige uno y su bono se suma -->
                        {#if (d.opciones?.length ?? 0) > 0}
                          <select
                            class="rango"
                            aria-label="Rango de {d.nombre}"
                            title={ayuda("El bono del rango elegido se suma al resultado")}
                            value={editor.eleccion(d.id)}
                            onchange={(e) => editor.elegirOpcion(d.id, Number(e.currentTarget.value))}
                          >
                            {#each d.opciones ?? [] as o, i (i)}
                              <option value={i}>{o.nombre} ({textoModificador(o.bonus)})</option>
                            {/each}
                          </select>
                        {:else if d.opcionales.length === 0 && (d.parametros_opcionales?.length ?? 0) === 0}
                          <span class="sin">—</span>
                        {/if}
                       </div>
                      </td>
                      <td class="resultado" class:positivo={resultado > 0} class:negativo={resultado < 0} title={editor.desgloseVariable(d)}>
                        {textoModificador(resultado)}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              {/if}
            </table>
          {/if}
        {/each}
        <p class="ayuda" title={ayuda("Si hay varias fichas con el foco, o ninguna, una tirada con variables da error")}>
          En el chat, con el foco en una instancia de esta ficha: «2d6 + {editor.variables[0].abreviatura || editor.variables[0].nombre}» suma su resultado.
        </p>
      {/if}
    </div>

    {#if plantillaAjena}
      <!-- Los datos son de OTRA ficha: se pide confirmación antes de pisar los de esta -->
      <div class="aviso-plantilla" role="alert">
        <span>Los datos a importar no son de la ficha actual. Si procedes se sobrescribirán sus datos y se perderán los anteriores.</span>
        <div class="botones-aviso">
          <button type="button" class="btn-cancelar" onclick={() => (plantillaAjena = null)}>Cancelar</button>
          <button
            type="button"
            class="btn-proceder"
            onclick={() => {
              const p = plantillaAjena;
              plantillaAjena = null;
              if (p) void editor.aplicarPlantilla(p);
            }}
          >Proceder</button>
        </div>
      </div>
    {/if}

    {#if editor.avisosPlantilla.length > 0}
      <div class="avisos-plantilla">
        {#each editor.avisosPlantilla as a}<p>{a}</p>{/each}
      </div>
    {/if}

    {#if editor.error}
      <p class="error" role="alert">{editor.error}</p>
    {/if}

    <div class="pie">
      {#if !editor.esBorrador && !esJugador}
        <button type="button" class="btn-borrar" onclick={() => editor.borrar()} disabled={editor.guardando}>Borrar ficha</button>
      {/if}
      <button
        type="button"
        class="btn-exportar"
        disabled={exportando}
        title={ayuda("Guarda una plantilla de esta ficha (un .html con sus datos y su foto) para rellenarla fuera de OpenRoll con cualquier navegador. Después se arrastra a OpenRoll: al banco o a esta ventana")}
        onclick={exportar}
      >Exportar plantilla</button>
      <div class="botones-derecha">
        <button type="button" class="btn-cancelar" onclick={() => editor.cancelar()}>Cancelar</button>
        <button type="button" class="btn-guardar" onclick={() => editor.guardar()} disabled={editor.guardando}>Guardar</button>
      </div>
    </div>
  </div>
</div>

<style>
  .ventana-flotante {
    position: fixed; z-index: 1000; width: 480px;
    display: flex; flex-direction: column; overflow: hidden;
    background: #18181b; border: 1px solid #3f3f46; border-radius: 8px;
    box-shadow: 0 14px 35px rgba(0, 0, 0, 0.75);
    user-select: none; font-size: 11px; color: #f4f4f5;
  }

  .cabecera {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px; cursor: grab;
    background: #27272a; border-bottom: 1px solid #3f3f46;
  }
  .cabecera:active { cursor: grabbing; }
  .icono-drag { color: #71717a; font-size: 12px; }
  .titulo { font-weight: bold; flex: 1; }
  .btn-cerrar { background: none; border: none; color: #a1a1aa; cursor: pointer; font-size: 13px; }
  .btn-cerrar:hover { color: white; }

  .cuerpo-ficha { padding: 10px; display: flex; flex-direction: column; gap: 8px; }
  .seccion { display: flex; flex-direction: column; gap: 8px; }
  /* Con muchas variables (las 18 habilidades de D&D...) la lista hace scroll y las pestañas y los botones se quedan a la vista */
  .seccion-variables { max-height: min(60vh, 520px); overflow-y: auto; padding-right: 4px; scrollbar-width: thin; scrollbar-color: #3f3f46 transparent; }
  .seccion-variables::-webkit-scrollbar { width: 4px; }
  .seccion-variables::-webkit-scrollbar-thumb { background: #3f3f46; border-radius: 4px; }

  .subpestanas { display: flex; gap: 2px; border-bottom: 1px solid #27272a; margin: -2px 0 0; }
  .subpestanas button {
    background: none; border: none; border-bottom: 2px solid transparent; color: #a1a1aa;
    padding: 3px 10px 4px; font-size: 11px; font-weight: 600; cursor: pointer;
  }
  .subpestanas button:hover { color: #f4f4f5; }
  .subpestanas button.activa { color: white; border-bottom-color: #3b82f6; }

  .titulo-grupo { margin: 6px 0 0; font-size: 11px; font-weight: 700; color: #d4d4d8; text-transform: uppercase; letter-spacing: 0.03em; }
  .titulo-grupo:first-child { margin-top: 0; }
  .tabla-variables td { vertical-align: middle; }
  .rango {
    background: #09090b; border: 1px solid #3f3f46; color: #e4e4e7; border-radius: 4px; padding: 1px 4px; font-size: 11px; max-width: 100%;
  }
  .opcionales { display: flex; flex-wrap: wrap; gap: 4px 10px; align-items: center; min-height: 18px; }
  .grupos-ficha { display: flex; flex-wrap: wrap; gap: 3px; }
  .grupo-ficha {
    background: #111113; border: 1px solid #27272a; color: #a1a1aa; border-radius: 4px; padding: 2px 8px;
    font-size: 11px; font-weight: 600; cursor: pointer; display: inline-flex; align-items: center; gap: 4px;
  }
  .grupo-ficha:hover { color: #f4f4f5; border-color: #3f3f46; }
  .grupo-ficha.activo { color: white; border-color: #3b82f6; background: rgba(59, 130, 246, 0.12); }
  .grupo-ficha .marca { color: #60a5fa; font-style: italic; font-size: 10px; }
  .tabla-variables .sin { color: #52525b; }
  .opcional { display: inline-flex; align-items: center; gap: 5px; cursor: pointer; color: #d4d4d8; font-size: 11px; line-height: 1; white-space: nowrap; }
  /* Check redondo */
  .redondo {
    appearance: none; -webkit-appearance: none; margin: 0; flex: none; position: relative; box-sizing: border-box;
    width: 14px; height: 14px; border-radius: 50%; border: 1.5px solid #71717a; background: #09090b; cursor: pointer;
  }
  /* El punto de dentro, en el centro exacto del círculo (inset + margin auto), sea cual sea el grosor del borde */
  .redondo::before {
    content: ""; position: absolute; inset: 0; margin: auto; width: 6px; height: 6px; border-radius: 50%;
    background: #3b82f6; transform: scale(0); transition: transform 0.1s;
  }
  .redondo:checked { border-color: #3b82f6; }
  .redondo:checked::before { transform: scale(1); }
  .tabla-variables { width: 100%; border-collapse: collapse; font-size: 11px; }
  .tabla-variables th { color: #71717a; font-size: 10px; font-weight: 600; text-align: left; padding: 2px 6px; border-bottom: 1px solid #27272a; }
  .tabla-variables td { padding: 3px 6px; border-bottom: 1px solid #1f1f23; }
  .tabla-variables .nombre { color: #f4f4f5; font-weight: 600; }
  .tabla-variables .abreviatura { color: #a1a1aa; }
  .tabla-variables .depende { color: #71717a; font-weight: 400; margin-left: 2px; }
  .tabla-variables .resultado { font-weight: bold; color: #d4d4d8; min-width: 40px; }
  .tabla-variables .resultado.positivo { color: #4ade80; }
  .tabla-variables .resultado.negativo { color: #f87171; }
  .vacio { margin: 0; color: #71717a; font-size: 11px; padding: 16px 4px; text-align: center; }
  .ayuda { margin: 0; color: #71717a; font-size: 10px; }
  .fila-superior { display: flex; gap: 10px; align-items: flex-start; }
  .columna-campos { flex: 1; display: flex; flex-direction: column; gap: 4px; }

  .fila { display: flex; align-items: center; gap: 6px; }
  .fila.separada { gap: 10px; }
  .fila label, .etiqueta { color: #a1a1aa; font-size: 10px; display: flex; align-items: center; gap: 4px; }

  .check { cursor: pointer; }
  .check input { cursor: pointer; accent-color: #3b82f6; margin: 0; }

  .cuerpo { display: flex; align-items: center; gap: 4px; }
  .separador-x { color: #71717a; font-weight: bold; font-size: 11px; }

  .input-texto {
    flex: 1; padding: 3px 6px; border-radius: 4px;
    background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 11px;
  }
  .select {
    padding: 2px 4px; border-radius: 4px;
    background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 10px;
  }

  .error { margin: 0 0 6px; color: #f87171; font-size: 12px; }

  .pie { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
  .btn-exportar {
    background: transparent; border: 1px solid #3f3f46; color: #d4d4d8; padding: 5px 10px; border-radius: 4px;
    font-size: 11px; font-weight: 600; cursor: pointer;
  }
  .btn-exportar:hover { border-color: #52525b; color: white; }
  .btn-exportar:disabled { opacity: 0.6; cursor: default; }
  .aviso-plantilla {
    display: flex; flex-direction: column; gap: 6px; margin-bottom: 6px; padding: 7px 9px; border-radius: 5px;
    background: rgba(127, 29, 29, 0.35); border: 1px solid #7f1d1d; color: #fca5a5; font-size: 12px; font-weight: 600;
  }
  .botones-aviso { display: flex; justify-content: flex-end; gap: 6px; }
  .btn-proceder { background: #b91c1c; color: white; border: none; padding: 5px 12px; border-radius: 4px; font-size: 11px; font-weight: 700; cursor: pointer; }
  .btn-proceder:hover { background: #dc2626; }
  .avisos-plantilla { margin-bottom: 6px; color: #fbbf24; font-size: 11px; }
  .avisos-plantilla p { margin: 0; }
  .botones-derecha { display: flex; gap: 6px; margin-left: auto; }
  .btn-cancelar, .btn-guardar, .btn-borrar {
    border: none; padding: 5px 12px; border-radius: 4px; font-size: 11px; font-weight: 600; cursor: pointer;
  }
  .btn-cancelar { background: #27272a; color: #d4d4d8; }
  .btn-guardar { background: #2563eb; color: white; }
  .btn-borrar { background: #7f1d1d; color: #fca5a5; }
  .btn-cancelar:hover { background: #3f3f46; }
  .btn-guardar:hover { background: #1d4ed8; }
  .btn-borrar:hover { background: #991b1b; }
</style>
