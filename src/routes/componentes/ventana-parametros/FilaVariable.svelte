<!-- src/routes/componentes/ventana-parametros/FilaVariable.svelte -->
<!-- Una variable de la partida en la tabla del host (nombre, abreviatura, neutro, escala), editable en línea.
     Se guarda al salir de la fila o con Enter, si cambió algo; Escape deshace lo escrito. Sin `definicion` es
     la fila nueva: se crea en Rust en cuanto tiene nombre y abreviatura. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type DefinicionVariable } from "$lib/partida.svelte";
  import {
    LIMITE_VALOR_VARIABLE,
    MAX_ABREVIATURA_VARIABLE,
    MAX_ESCALA_VARIABLE,
    MAX_NOMBRE_VARIABLE,
    VARIABLE_NUEVA
  } from "$lib/variables";

  let {
    definicion = null,
    indice = -1,
    ejemplo = false,
    arrastrando = false,
    onIniciarArrastre,
    onError,
    onCreada,
    onDescartar
  }: {
    definicion?: DefinicionVariable | null;
    indice?: number;
    /** Solo la primera fila de la tabla enseña un ejemplo («Fuerza», «f») como placeholder. */
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

  let fila = $state<HTMLDivElement | null>(null);
  let campoNombre = $state<HTMLInputElement | null>(null);

  // svelte-ignore state_referenced_locally
  let nombre = $state(definicion?.nombre ?? "");
  // svelte-ignore state_referenced_locally
  let abreviatura = $state(definicion?.abreviatura ?? "");
  // svelte-ignore state_referenced_locally
  let neutro = $state<number | null>(definicion?.neutro ?? VARIABLE_NUEVA.neutro);
  // svelte-ignore state_referenced_locally
  let escala = $state<number | null>(definicion?.escala ?? VARIABLE_NUEVA.escala);
  let guardando = false;
  /** El borrador ya se creó en Rust: esta fila está a punto de sustituirse por la guardada y no debe crear otra. */
  let creada = false;

  /** Vuelve a lo que tiene Rust (tras guardar, o al deshacer con Escape). */
  function deshacer() {
    if (!definicion) return;
    nombre = definicion.nombre;
    abreviatura = definicion.abreviatura;
    neutro = definicion.neutro;
    escala = definicion.escala;
  }

  // Si la lista se relee (al guardar esta u otra), se toma lo de Rust, salvo en la fila donde se está escribiendo
  $effect(() => {
    if (!definicion) return;
    const { nombre: n, abreviatura: a, neutro: ne, escala: es } = definicion;
    if (fila?.contains(document.activeElement)) return;
    nombre = n;
    abreviatura = a;
    neutro = ne;
    escala = es;
  });

  onMount(() => {
    if (!definicion) campoNombre?.focus();
  });

  async function guardar(conEnter = false) {
    if (guardando || creada) return;
    const datos = { nombre: nombre.trim(), abreviatura: abreviatura.trim(), neutro: Number(neutro), escala: Number(escala) };
    // Un borrador sin nombre se queda como está, sin error y sin guardarse (la abreviatura es opcional)
    if (!definicion && !datos.nombre) return;
    if (neutro === null || escala === null || !Number.isInteger(datos.neutro) || !Number.isInteger(datos.escala)) {
      onError("El valor neutro y la escala deben ser números enteros");
      return;
    }
    if (
      definicion &&
      datos.nombre === definicion.nombre &&
      datos.abreviatura === definicion.abreviatura &&
      datos.neutro === definicion.neutro &&
      datos.escala === definicion.escala
    ) {
      return;
    }
    guardando = true;
    try {
      if (definicion) await partida.modificarVariable({ ...datos, id: definicion.id });
      else {
        // Se marca ANTES de esperar: nada (otro foco perdido, Enter...) puede crearla por segunda vez
        creada = true;
        try {
          await partida.crearVariable(datos);
        } catch (err) {
          creada = false;
          throw err;
        }
        onCreada?.(conEnter);
      }
      onError("");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err)); // lo escrito se queda, para corregirlo
    } finally {
      guardando = false;
    }
  }

  async function borrar() {
    if (!definicion) return onDescartar?.();
    try {
      await partida.borrarVariable(definicion.id);
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  function alTeclear(e: KeyboardEvent) {
    // Ni Enter ni Escape llegan a la ventana (que guardaría el formulario de parámetros o se cerraría)
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      void guardar(true);
    } else if (e.key === "Escape") {
      e.stopPropagation();
      if (!definicion) return onDescartar?.();
      deshacer();
      onError("");
      (document.activeElement as HTMLElement | null)?.blur();
    }
  }

  function alSalir(e: FocusEvent) {
    if (fila?.contains(e.relatedTarget as Node | null)) return;
    // `relatedTarget` puede venir vacío aunque el foco pase a otro campo de la fila: se mira dónde queda el foco
    // cuando termina el cambio, y si la fila ya no está en la página (se está quitando) no se guarda nada
    setTimeout(() => {
      if (!fila?.isConnected || fila.contains(document.activeElement)) return;
      void guardar();
    }, 0);
  }

  function alPulsar(e: PointerEvent) {
    // Desde un campo o un botón no se arrastra (se escribe o se pulsa)
    if ((e.target as HTMLElement).closest("input, button")) return;
    onIniciarArrastre?.(e);
  }
</script>

<!-- Las teclas y el foco son de sus campos (Enter guarda, Escape deshace); la fila solo los recoge -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="fila-variable"
  class:nueva={!definicion}
  class:fantasma-origen={arrastrando}
  data-indice={definicion ? indice : undefined}
  role="group"
  aria-label={definicion ? `Variable ${definicion.nombre}` : "Variable nueva"}
  bind:this={fila}
  onpointerdown={alPulsar}
  onkeydown={alTeclear}
  onfocusout={alSalir}
>
  <span class="asa" title={definicion ? ayuda("Arrastra para cambiar el orden") : undefined}>{definicion ? "⋮⋮" : "+"}</span>
  <input
    type="text"
    class="campo"
    maxlength={MAX_NOMBRE_VARIABLE}
    placeholder={ejemplo ? "Fuerza" : ""}
    aria-label="Nombre"
    bind:value={nombre}
    bind:this={campoNombre}
  />
  <input
    type="text"
    class="campo"
    maxlength={MAX_ABREVIATURA_VARIABLE}
    placeholder={ejemplo ? "f" : ""}
    aria-label="Abreviatura"
    title={ayuda("Opcional. Lo que se escribe en el chat: 2d6 + f (sin ella, se usa el nombre). Solo letras")}
    bind:value={abreviatura}
  />
  <input
    type="number"
    class="campo numero"
    min={-LIMITE_VALOR_VARIABLE}
    max={LIMITE_VALOR_VARIABLE}
    aria-label="Valor neutro"
    title={ayuda("Con este valor el resultado es 0")}
    bind:value={neutro}
  />
  <input
    type="number"
    class="campo numero"
    min="1"
    max={MAX_ESCALA_VARIABLE}
    aria-label="Escala"
    title={ayuda("Cada tantos puntos por encima del neutro suma 1 (y por debajo resta 1)")}
    bind:value={escala}
  />
  <button type="button" class="btn-borrar" onclick={borrar}>{definicion ? "Borrar" : "Quitar"}</button>
</div>

<style>
  .fila-variable {
    display: grid; grid-template-columns: 14px 1fr 64px 50px 44px 46px; gap: 4px; align-items: center;
    padding: 4px 6px; background: #111113; border: 1px solid #27272a; border-radius: 5px; cursor: grab;
  }
  .fila-variable:hover { border-color: #3f3f46; }
  .fila-variable.nueva { border-style: dashed; cursor: default; }
  .fila-variable.fantasma-origen { opacity: 0.3; }
  .asa { color: #71717a; font-size: 12px; text-align: center; }
  .campo {
    min-width: 0; padding: 3px 5px; border-radius: 4px; cursor: text;
    background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 11px;
  }
  .campo.numero { text-align: center; padding: 3px 2px; }
  .btn-borrar {
    background: none; border: none; color: #f87171; cursor: pointer; font-size: 11px; font-weight: 600; padding: 2px;
  }
  .btn-borrar:hover { color: #ef4444; text-decoration: underline; }
</style>
