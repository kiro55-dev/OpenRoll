<!-- src/routes/componentes/ventana-parametros/FilaArchivoTexto.svelte -->
<!-- Un archivo de texto POR DEFECTO del conjunto (el .txt con el que nace toda ficha nueva): su nombre (la extensión es siempre .txt) y,
     si se despliega, el texto inicial. Funciona como una fila de variables: NO se guarda sola; con cambios sin guardar lleva una franja
     ROJA a la izquierda y, ya guardada, VERDE (ver `edicionVariables`). Enter en el nombre guarda esta fila, Ctrl+S o «Guardar cambios»
     guardan todas, Escape deshace. Sin `archivo` es una fila nueva (borrador): se crea en Rust al guardarla. «Borrar» y arrastrar para
     reordenar se aplican al momento. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type ArchivoTextoDefecto } from "$lib/partida.svelte";
  import { edicionVariables as edicion } from "$lib/edicionVariables.svelte";
  import { MAX_CARACTERES_ARCHIVO, MAX_NOMBRE_ARCHIVO_TEXTO } from "$lib/plantillaFicha";

  let {
    archivo = null,
    indice = -1,
    ejemplo = false,
    arrastrando = false,
    onIniciarArrastre,
    onError,
    onCreada,
    onDescartar
  }: {
    archivo?: ArchivoTextoDefecto | null;
    indice?: number;
    /** Solo la primera fila de la tabla enseña un ejemplo como placeholder. */
    ejemplo?: boolean;
    /** Es la que se está arrastrando (se ve tenue en su sitio). */
    arrastrando?: boolean;
    onIniciarArrastre?: (e: PointerEvent) => void;
    onError: (mensaje: string) => void;
    /** Fila nueva: se creó en Rust. `conEnter`: el usuario quiere seguir añadiendo. */
    onCreada?: (conEnter: boolean) => void;
    /** Fila nueva: se quita sin crear nada. */
    onDescartar?: () => void;
  } = $props();

  let campoNombre = $state<HTMLInputElement | null>(null);

  // svelte-ignore state_referenced_locally
  let nombre = $state(archivo?.nombre ?? "");
  // svelte-ignore state_referenced_locally
  let contenido = $state(archivo?.contenido ?? "");
  /** El texto inicial se ve desplegado: solo si ya tiene algo o el usuario lo abre. */
  // svelte-ignore state_referenced_locally
  let abierto = $state(!!archivo?.contenido);
  let guardando = false;
  /** El borrador ya se creó en Rust: esta fila está a punto de sustituirse por la guardada y no debe crear otra. */
  let creada = $state(false);
  const clave = Symbol("fila-archivo-texto");

  /** La extensión es siempre .txt: el nombre no lleva puntos ni caracteres de ruta (Rust lo comprueba igual). */
  const limpiar = (t: string) => t.replace(/[.\\/:*?"<>|\u0000-\u001f]/g, "");

  /** Vuelve a lo que tiene Rust (tras guardar, o al deshacer con Escape). */
  function deshacer() {
    if (!archivo) return;
    nombre = archivo.nombre;
    contenido = archivo.contenido;
  }

  // Si la lista se relee (al guardar esta u otra, o al cambiar de conjunto), se toma lo de Rust, salvo en una fila con cambios sin guardar
  $effect(() => {
    if (!archivo) return;
    void [archivo.nombre, archivo.contenido];
    if (sucia) return;
    deshacer();
  });

  onMount(() => {
    if (!archivo) campoNombre?.focus();
    return edicion.registrar(clave, {
      guardar: () => guardar(),
      descartar: () => (archivo ? deshacer() : onDescartar?.())
    });
  });
  $effect(() => edicion.marcar(clave, sucia));

  /**
   * Cambios sin guardar. En un archivo que ya existe: lo escrito difiere de lo guardado. En un borrador: tiene algo escrito (uno vacío
   * no cuenta, ni avisa al cerrar). Un borrador ya creado en Rust deja de contar.
   */
  const sucia = $derived(
    creada ? false : archivo ? nombre.trim() !== archivo.nombre || contenido !== archivo.contenido : nombre.trim() !== "" || contenido !== ""
  );
  const guardada = $derived(!!archivo && !sucia && edicion.verdes.has(`txt:${archivo.id}`));

  /** Guarda la fila. `true` = queda guardada (o no había nada que guardar); `false` = no se pudo (el error ya está a la vista). */
  async function guardar(conEnter = false): Promise<boolean> {
    if (guardando || creada) return true;
    if (!archivo && !sucia) return true; // un borrador vacío no se guarda ni da error
    const datos = { nombre: nombre.trim(), contenido };
    if (!datos.nombre) {
      onError("El archivo de texto necesita un nombre");
      return false;
    }
    if (archivo && datos.nombre === archivo.nombre && datos.contenido === archivo.contenido) return true;
    guardando = true;
    try {
      if (archivo) {
        await partida.guardarArchivoTextoDefecto({ id: archivo.id, ...datos });
        edicion.marcarVerde(`txt:${archivo.id}`);
      } else {
        // Se marca ANTES de esperar: nada (Enter, Ctrl+S...) puede crearla por segunda vez
        creada = true;
        try {
          const nueva = await partida.guardarArchivoTextoDefecto({ id: 0, ...datos });
          edicion.marcarVerde(`txt:${nueva.id}`); // antes de quitar el borrador: la fila real nace ya en verde
        } catch (err) {
          creada = false;
          throw err;
        }
        onCreada?.(conEnter);
      }
      onError("");
      return true;
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err)); // lo escrito se queda, para corregirlo
      return false;
    } finally {
      guardando = false;
    }
  }

  async function borrar() {
    if (!archivo) return onDescartar?.();
    try {
      await partida.borrarArchivoTextoDefecto(archivo.id);
      edicion.quitarVerde(`txt:${archivo.id}`);
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  function alTeclear(e: KeyboardEvent) {
    const objetivo = e.target as HTMLElement;
    // Ni Enter ni Escape llegan a la ventana (que guardaría el formulario de parámetros o se cerraría)
    if (e.key === "Enter") {
      if (objetivo.tagName === "BUTTON" || objetivo.tagName === "TEXTAREA") return; // un botón hace lo suyo; en el texto es un salto de línea
      e.preventDefault();
      e.stopPropagation();
      void guardar(true);
    } else if (e.key === "Escape") {
      e.stopPropagation();
      if (!archivo) return onDescartar?.();
      deshacer();
      onError("");
      (document.activeElement as HTMLElement | null)?.blur();
    }
  }

  function alPulsar(e: PointerEvent) {
    // Desde un campo o un botón no se arrastra (se escribe o se pulsa)
    if ((e.target as HTMLElement).closest("input, button, textarea, label")) return;
    onIniciarArrastre?.(e);
  }
</script>

<!-- Las teclas y el foco son de sus campos (Enter guarda, Escape deshace); la fila solo los recoge -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="fila-archivo-texto"
  class:nueva={!archivo}
  class:sucia
  class:guardada
  class:fantasma-origen={arrastrando}
  data-indice={archivo ? indice : undefined}
  role="group"
  aria-label={archivo ? `Archivo ${archivo.nombre}.txt` : "Archivo de texto nuevo"}
  onpointerdown={alPulsar}
  onkeydown={alTeclear}
>
  <div class="campos">
    <span class="asa" title={archivo ? ayuda("Arrastra para cambiar el orden") : undefined}>{archivo ? "⋮⋮" : "+"}</span>
    <div class="nombre">
      <input
        type="text"
        class="campo"
        maxlength={MAX_NOMBRE_ARCHIVO_TEXTO}
        placeholder={ejemplo ? "Historia" : ""}
        aria-label="Nombre del archivo"
        value={nombre}
        oninput={(e) => {
          nombre = limpiar(e.currentTarget.value);
          e.currentTarget.value = nombre; // nada de puntos: la extensión es siempre .txt
        }}
        bind:this={campoNombre}
      />
      <span class="extension">.txt</span>
    </div>
    <button
      type="button"
      class="btn-texto"
      class:con-texto={contenido !== ""}
      aria-expanded={abierto}
      title={ayuda("El texto con el que nace este archivo en cada ficha nueva (opcional)")}
      onclick={() => (abierto = !abierto)}
    >Texto {abierto ? "▴" : "▾"}</button>
    <button type="button" class="btn-borrar" onclick={borrar}>{archivo ? "Borrar" : "Quitar"}</button>
  </div>

  {#if abierto}
    <textarea
      class="texto-inicial"
      maxlength={MAX_CARACTERES_ARCHIVO}
      spellcheck="false"
      placeholder="Texto inicial (opcional)"
      aria-label="Texto inicial del archivo"
      bind:value={contenido}
    ></textarea>
  {/if}
</div>

<style>
  .fila-archivo-texto {
    position: relative; display: flex; flex-direction: column; gap: 4px;
    /* 8 px a la izquierda (no 6): un poco de aire entre la franja de estado y el asa ⋮⋮ */
    padding: 4px 6px 4px 8px; background: #111113; border: 1px solid #27272a; border-radius: 5px; cursor: grab;
  }
  /* Franja de estado, como en las variables: roja = cambios sin guardar, verde = guardada, nada = sin tocar */
  .fila-archivo-texto::before {
    content: ""; position: absolute; left: -1px; top: -1px; bottom: -1px; width: 3px;
    border-radius: 5px 0 0 5px; background: transparent; pointer-events: none;
  }
  .fila-archivo-texto.guardada::before { background: #22c55e; }
  .fila-archivo-texto.sucia::before { background: #ef4444; }
  .fila-archivo-texto:hover { border-color: #3f3f46; }
  .fila-archivo-texto.nueva { border-style: dashed; cursor: default; }
  .fila-archivo-texto.fantasma-origen { opacity: 0.3; }
  .campos { display: grid; grid-template-columns: 14px 1fr 62px 46px; gap: 4px; align-items: center; }
  .asa { color: #71717a; font-size: 12px; text-align: center; }
  .nombre { display: flex; align-items: center; gap: 4px; min-width: 0; }
  .nombre .campo { flex: 1; }
  .extension { color: #71717a; font-size: 11px; }
  .campo {
    min-width: 0; padding: 3px 5px; border-radius: 4px; cursor: text;
    background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 11px;
  }
  .btn-texto { background: none; border: none; color: #71717a; cursor: pointer; font-size: 10px; font-weight: 600; padding: 2px; }
  .btn-texto:hover, .btn-texto.con-texto { color: #93c5fd; }
  .btn-borrar { background: none; border: none; color: #f87171; cursor: pointer; font-size: 11px; font-weight: 600; padding: 2px; }
  .btn-borrar:hover { color: #ef4444; text-decoration: underline; }
  .texto-inicial {
    width: 100%; box-sizing: border-box; min-height: 90px; resize: vertical; cursor: text;
    padding: 5px 6px; border-radius: 4px; background: #09090b; border: 1px solid #3f3f46; color: white;
    font: 11px/1.45 ui-monospace, "Cascadia Mono", Consolas, monospace;
  }
  .texto-inicial:focus { outline: none; border-color: #3b82f6; }
</style>
