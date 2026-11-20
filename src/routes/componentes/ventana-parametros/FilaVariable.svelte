<!-- src/routes/componentes/ventana-parametros/FilaVariable.svelte -->
<!-- Una variable de la partida en la tabla del host, editable en línea. En un grupo de BASES: nombre, abreviatura, neutro y
     escala. En uno de DERIVADAS: nombre, abreviatura, de qué bases depende (se suman) y sus opcionales (cada ficha las marca
     o no). Con el botón «ƒ» una derivada también tiene una fórmula (× multiplicador ÷ divisor + constante, redondeando hacia abajo)
     y RANGOS (Sin entrenar +0, Entrenado +2...: cada ficha elige uno y su bono se suma). NO se guarda sola: con cambios sin guardar la fila lleva una franja ROJA a la izquierda y, ya guardada, VERDE (ver `edicionVariables`);
     Enter guarda esta fila, Ctrl+S o el botón «Guardar cambios» de la ventana guardan todas; Escape deshace lo escrito. Sin `definicion` es una
     fila nueva (borrador): se crea en Rust al guardarla (con nombre y, si es derivada, al menos una base o un parámetro). Borrar una base
     que usan derivadas pide confirmar y dice qué pasará (en cascada). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type DefinicionVariable, type TipoGrupoVariables } from "$lib/partida.svelte";
  import { edicionVariables as edicion } from "$lib/edicionVariables.svelte";
  import {
    LIMITE_VALOR_VARIABLE,
    MAX_ABREVIATURA_VARIABLE,
    MAX_ESCALA_VARIABLE,
    MAX_FACTOR_FORMULA,
    MAX_NOMBRE_VARIABLE,
    VARIABLE_NUEVA,
    avisoDeBorrar,
    esDerivada,
    parametroComoFuente
  } from "$lib/variables";
  import SelectorVariables from "./SelectorVariables.svelte";

  let {
    definicion = null,
    grupo,
    tipo,
    indice = -1,
    ejemplo = false,
    arrastrando = false,
    onIniciarArrastre,
    onError,
    onCreada,
    onDescartar
  }: {
    definicion?: DefinicionVariable | null;
    /** El grupo de la fila (el de un borrador al crearse). */
    grupo: number;
    tipo: TipoGrupoVariables;
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

  const derivada = $derived(tipo === "Derivadas");
  /** Las variables base de la partida: de ellas puede depender una derivada. */
  const bases = $derived(partida.definicionesVariables.filter((d) => !esDerivada(d) && partida.gruposVariables.find((g) => g.id === d.grupo)?.tipo === "Base"));

  /** Los parámetros visuales que pueden ser fuente: numéricos y escritos a mano (uno calculado ya usa variables). */
  const fuentes = $derived(partida.definiciones.filter(parametroComoFuente));

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
  // svelte-ignore state_referenced_locally
  let dependeDe = $state<number[]>([...(definicion?.bases ?? [])]);
  // svelte-ignore state_referenced_locally
  let opcionales = $state<number[]>([...(definicion?.opcionales ?? [])]);
  // svelte-ignore state_referenced_locally
  let parametrosFijos = $state<number[]>([...(definicion?.parametros ?? [])]);
  // svelte-ignore state_referenced_locally
  let parametrosOpcionales = $state<number[]>([...(definicion?.parametros_opcionales ?? [])]);
  // svelte-ignore state_referenced_locally
  let multiplicador = $state<number | null>(definicion?.multiplicador ?? 1);
  // svelte-ignore state_referenced_locally
  let divisor = $state<number | null>(definicion?.divisor ?? 1);
  // svelte-ignore state_referenced_locally
  let constante = $state<number | null>(definicion?.constante ?? 0);
  // svelte-ignore state_referenced_locally
  let rangos = $state<number | null>(definicion?.rangos ?? null);
  /** El panel de la fórmula y los rangos: se abre solo si la variable ya tiene alguno. */
  // svelte-ignore state_referenced_locally
  let avanzado = $state(tieneAvanzado(definicion));
  let guardando = false;
  /** El borrador ya se creó en Rust: esta fila está a punto de sustituirse por la guardada y no debe crear otra. */
  let creada = $state(false);
  /**
   * El borrador ya creado cuya variable real YA aparece en la lista: hasta que `onCreada` lo quita pasa un momento (se vuelven a
   * leer las fichas tras recargar las definiciones) en el que se veían las dos filas a la vez, y al desaparecer el borrador
   * todo daba un salto. Se esconde en cuanto la real está, así que la una sustituye a la otra sin que se vea.
   */
  const yaEnLista = $derived(
    creada &&
      !definicion &&
      partida.definicionesVariables.some((d) => d.grupo === grupo && d.nombre.trim().toLowerCase() === nombre.trim().toLowerCase())
  );
  /** El aviso de borrar en cascada, mientras se espera la confirmación. */
  let confirmando = $state<string | null>(null);

  function tieneAvanzado(d: DefinicionVariable | null) {
    return !!d && ((d.multiplicador ?? 1) !== 1 || (d.divisor ?? 1) !== 1 || (d.constante ?? 0) !== 0 || d.rangos != null);
  }

  /** Vuelve a lo que tiene Rust (tras guardar, o al deshacer con Escape). */
  function deshacer() {
    if (!definicion) return;
    nombre = definicion.nombre;
    abreviatura = definicion.abreviatura;
    neutro = definicion.neutro;
    escala = definicion.escala;
    dependeDe = [...definicion.bases];
    opcionales = [...definicion.opcionales];
    parametrosFijos = [...(definicion.parametros ?? [])];
    parametrosOpcionales = [...(definicion.parametros_opcionales ?? [])];
    multiplicador = definicion.multiplicador ?? 1;
    divisor = definicion.divisor ?? 1;
    constante = definicion.constante ?? 0;
    rangos = definicion.rangos ?? null;
  }

  // Si la lista se relee (al guardar esta u otra), se toma lo de Rust, salvo en una fila con cambios sin guardar
  $effect(() => {
    if (!definicion) return;
    const d = definicion;
    void [d.nombre, d.abreviatura, d.neutro, d.escala, d.bases, d.opcionales, d.parametros, d.parametros_opcionales, d.multiplicador, d.divisor, d.constante, d.rangos];
    if (sucia) return;
    deshacer();
  });

  onMount(() => {
    if (!definicion) campoNombre?.focus();
    // Se apunta en el registro de ediciones (se quita sola al desmontarse)
    return edicion.registrar(clave, {
      guardar: () => guardar(),
      descartar: () => (definicion ? deshacer() : onDescartar?.())
    });
  });
  $effect(() => edicion.marcar(clave, sucia));

  const iguales = (a: number[], b: number[]) => a.length === b.length && a.every((x, i) => x === b[i]);

  /** Lo escrito en la fila, listo para mandarlo a Rust. */
  function leerDatos() {
    return {
      nombre: nombre.trim(),
      abreviatura: abreviatura.trim(),
      neutro: derivada ? 0 : Number(neutro),
      escala: derivada ? 1 : Number(escala),
      grupo: definicion?.grupo ?? grupo,
      bases: derivada ? [...dependeDe] : [],
      opcionales: derivada ? [...opcionales] : [],
      parametros: derivada ? [...parametrosFijos] : [],
      parametros_opcionales: derivada ? [...parametrosOpcionales] : [],
      multiplicador: derivada ? Number(multiplicador ?? 0) : 1,
      divisor: derivada ? Number(divisor ?? 0) : 1,
      constante: derivada ? Number(constante ?? 0) : 0,
      rangos: derivada ? rangos : null
    };
  }

  /** ¿Es igual a lo que tiene Rust? */
  function igualA(datos: ReturnType<typeof leerDatos>, d: DefinicionVariable): boolean {
    return (
      datos.nombre === d.nombre &&
      datos.abreviatura === d.abreviatura &&
      datos.neutro === d.neutro &&
      datos.escala === d.escala &&
      iguales(datos.bases, d.bases) &&
      iguales(datos.opcionales, d.opcionales) &&
      iguales(datos.parametros, d.parametros ?? []) &&
      iguales(datos.parametros_opcionales, d.parametros_opcionales ?? []) &&
      datos.multiplicador === (d.multiplicador ?? 1) &&
      datos.divisor === (d.divisor ?? 1) &&
      datos.constante === (d.constante ?? 0) &&
      datos.rangos === (d.rangos ?? null)
    );
  }

  const clave = Symbol("fila-variable");
  /** Id único del desplegable de rangos: ahora pueden estar montados a la vez varios borradores. */
  // svelte-ignore state_referenced_locally
  const idLista = `lista-${definicion?.id ?? "nueva"}-${Math.random().toString(36).slice(2, 8)}`;

  /**
   * Cambios sin guardar. En una variable que ya existe: lo escrito difiere de lo guardado. En un borrador: tiene algo escrito
   * (uno vacío no cuenta, ni avisa al cerrar). Un borrador ya creado en Rust deja de contar.
   */
  const sucia = $derived(
    creada
      ? false
      : definicion
        ? !igualA(leerDatos(), definicion)
        : nombre.trim() !== "" || abreviatura.trim() !== "" || dependeDe.length + opcionales.length + parametrosFijos.length + parametrosOpcionales.length > 0
  );
  /** Guardada en esta sesión y sin tocar desde entonces. */
  const guardada = $derived(!!definicion && !sucia && edicion.verdes.has(definicion.id));

  /** Guarda la fila. `true` = queda guardada (o no había nada que guardar); `false` = no se pudo (el error ya está a la vista). */
  async function guardar(conEnter = false): Promise<boolean> {
    if (guardando || creada) return true;
    const datos = leerDatos();
    // Un borrador vacío no se guarda ni da error
    if (!definicion && !sucia) return true;
    if (!definicion && !datos.nombre) {
      onError("La variable necesita un nombre");
      return false;
    }
    if (derivada && datos.bases.length === 0 && datos.parametros.length === 0) {
      onError("Una variable derivada depende de al menos una variable base o un parámetro");
      return false;
    }
    if (!derivada && (neutro === null || escala === null || !Number.isInteger(datos.neutro) || !Number.isInteger(datos.escala))) {
      onError("El valor neutro y la escala deben ser números enteros");
      return false;
    }
    if (definicion && igualA(datos, definicion)) return true;
    guardando = true;
    try {
      if (definicion) {
        await partida.modificarVariable({ ...datos, id: definicion.id });
        edicion.marcarVerde(definicion.id);
      } else {
        // Se marca ANTES de esperar: nada (Enter, Ctrl+S...) puede crearla por segunda vez
        creada = true;
        try {
          const nueva = await partida.crearVariable(datos);
          edicion.marcarVerde(nueva.id); // antes de quitar el borrador: la fila real nace ya en verde
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

  async function borrar(confirmado = false) {
    if (!definicion) return onDescartar?.();
    // Si la usan derivadas, primero se avisa de lo que pasará
    const aviso = confirmado ? null : avisoDeBorrar(partida.definicionesVariables, [definicion.id]);
    if (aviso) return void (confirmando = aviso);
    confirmando = null;
    try {
      await partida.borrarVariable(definicion.id);
      edicion.quitarVerde(definicion.id);
      onError("");
    } catch (err) {
      onError(String(err));
    }
  }

  function alTeclear(e: KeyboardEvent) {
    // Ni Enter ni Escape llegan a la ventana (que guardaría el formulario de parámetros o se cerraría)
    if (e.key === "Enter") {
      if ((e.target as HTMLElement).tagName === "BUTTON") return; // un botón de la fila hace lo suyo
      e.preventDefault();
      e.stopPropagation();
      void guardar(true);
    } else if (e.key === "Escape") {
      e.stopPropagation();
      if (confirmando) return void (confirmando = null);
      if (!definicion) return onDescartar?.();
      deshacer();
      onError("");
      (document.activeElement as HTMLElement | null)?.blur();
    }
  }

  function alPulsar(e: PointerEvent) {
    // Desde un campo, un botón o un selector no se arrastra (se escribe o se pulsa)
    if ((e.target as HTMLElement).closest("input, button, label, .selector, .avanzado")) return;
    onIniciarArrastre?.(e);
  }
</script>

<!-- Las teclas y el foco son de sus campos (Enter guarda, Escape deshace); la fila solo los recoge -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="fila-variable"
  class:nueva={!definicion}
  class:sustituida={yaEnLista}
  class:sucia
  class:guardada
  class:fantasma-origen={arrastrando}
  data-indice={definicion ? indice : undefined}
  role="group"
  aria-label={definicion ? `Variable ${definicion.nombre}` : "Variable nueva"}
  bind:this={fila}
  onpointerdown={alPulsar}
  onkeydown={alTeclear}
>
  <div class="campos" class:derivada>
    <span class="asa" title={definicion ? ayuda("Arrastra para cambiar el orden") : undefined}>{definicion ? "⋮⋮" : "+"}</span>
    <input
      type="text"
      class="campo"
      maxlength={MAX_NOMBRE_VARIABLE}
      placeholder={ejemplo ? (derivada ? "Acrobacias" : "Fuerza") : ""}
      aria-label="Nombre"
      bind:value={nombre}
      bind:this={campoNombre}
    />
    <input
      type="text"
      class="campo"
      maxlength={MAX_ABREVIATURA_VARIABLE}
      placeholder={ejemplo ? (derivada ? "acro" : "f") : ""}
      aria-label="Abreviatura"
      title={ayuda("Opcional. Lo que se escribe en el chat: 2d6 + f (sin ella, se usa el nombre). Solo letras")}
      bind:value={abreviatura}
    />
    {#if derivada}
      <SelectorVariables
        opciones={bases}
        bind:elegidas={dependeDe}
        excluidas={opcionales}
        parametros={fuentes}
        bind:parametrosElegidos={parametrosFijos}
        parametrosExcluidos={parametrosOpcionales}
        vacio="Elegir…"
        etiquetaAria="De qué variables base y parámetros depende"
        titulo="Variables base (su resultado) y parámetros visuales (su valor) que se suman. Al menos uno"
      />
      <SelectorVariables
        opciones={bases}
        bind:elegidas={opcionales}
        excluidas={dependeDe}
        parametros={fuentes}
        bind:parametrosElegidos={parametrosOpcionales}
        parametrosExcluidos={parametrosFijos}
        vacio="Ninguna"
        etiquetaAria="Variables y parámetros opcionales"
        titulo="Cada ficha decide si suma estos (un check «+ com» en la ficha)"
      />
    {:else}
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
    {/if}
    <button type="button" class="btn-borrar" onclick={() => borrar()}>{definicion ? "Borrar" : "Quitar"}</button>
  </div>

  {#if derivada}
    <!-- Fórmula y rangos: se abren con «ƒ» y quedan dentro de la fila (así tocarlos no cuenta como salir de ella) -->
    <div class="barra-avanzado">
      <button
        type="button"
        class="btn-avanzado"
        class:con-datos={tieneAvanzado(definicion) || rangos !== null || multiplicador !== 1 || divisor !== 1 || constante !== 0}
        aria-expanded={avanzado}
        title={ayuda("Fórmula (× ÷ +) y rangos de esta variable")}
        onclick={() => (avanzado = !avanzado)}
      >ƒ Fórmula y rangos {avanzado ? "▴" : "▾"}</button>
    </div>
    {#if avanzado}
      <div class="avanzado">
        <div class="formula">
          <span class="texto">Resultado = (suma de las variables)</span>
          <label>× <input type="number" class="campo numero" min="1" max={MAX_FACTOR_FORMULA} aria-label="Multiplicador" bind:value={multiplicador} /></label>
          <label>÷ <input type="number" class="campo numero" min="1" max={MAX_FACTOR_FORMULA} aria-label="Divisor" title={ayuda("Redondea hacia abajo")} bind:value={divisor} /></label>
          <label>+ <input type="number" class="campo numero" min={-LIMITE_VALOR_VARIABLE} max={LIMITE_VALOR_VARIABLE} aria-label="Constante" bind:value={constante} /></label>
        </div>
        <div class="rangos">
          <label class="texto" for={idLista} title={ayuda("Cada ficha elige uno de los rangos de la lista y su bono se suma al resultado. Las listas se definen una vez, abajo, y las comparten varias variables")}>Rangos:</label>
          <select id={idLista} class="campo" aria-label="Lista de rangos" bind:value={rangos}>
            <option value={null}>Sin rangos</option>
            {#each partida.listasRangos as l (l.id)}<option value={l.id}>{l.nombre}</option>{/each}
          </select>
        </div>
      </div>
    {/if}
  {/if}

  {#if confirmando}
    <div class="confirmar" role="alert">
      <span>{confirmando}</span>
      <button type="button" class="si" onclick={() => borrar(true)}>Borrar</button>
      <button type="button" class="no" onclick={() => (confirmando = null)}>Cancelar</button>
    </div>
  {/if}
</div>

<style>
  .fila-variable {
    position: relative; display: flex; flex-direction: column; gap: 4px;
    /* 8 px a la izquierda (no 6): un poco de aire entre la franja de estado y el asa ⋮⋮ */
    padding: 4px 6px 4px 8px; background: #111113; border: 1px solid #27272a; border-radius: 5px; cursor: grab;
  }
  /* Franja de estado, como en Notepad++: roja = cambios sin guardar, verde = guardada, nada = sin tocar */
  .fila-variable::before {
    content: ""; position: absolute; left: -1px; top: -1px; bottom: -1px; width: 3px;
    border-radius: 5px 0 0 5px; background: transparent; pointer-events: none;
  }
  .fila-variable.guardada::before { background: #22c55e; }
  .fila-variable.sucia::before { background: #ef4444; }
  .barra-avanzado { display: flex; }
  .btn-avanzado {
    background: none; border: none; color: #71717a; cursor: pointer; font-size: 10px; padding: 0 2px; font-weight: 600;
  }
  .btn-avanzado:hover, .btn-avanzado.con-datos { color: #93c5fd; }
  .avanzado { display: flex; flex-direction: column; gap: 5px; padding: 5px 6px; background: #0c0c0e; border-radius: 4px; cursor: default; }
  .formula, .rangos { display: flex; flex-wrap: wrap; align-items: center; gap: 5px 8px; }
  .formula label { display: flex; align-items: center; gap: 3px; color: #a1a1aa; font-size: 11px; }
  .formula .numero { width: 48px; }
  .texto { color: #a1a1aa; font-size: 10px; }
  .campos { display: grid; grid-template-columns: 14px 1fr 64px 50px 44px 46px; gap: 4px; align-items: center; }
  .campos.derivada { grid-template-columns: 14px 1fr 54px 1fr 1fr 46px; }
  .fila-variable:hover { border-color: #3f3f46; }
  .fila-variable.nueva { border-style: dashed; cursor: default; }
  .fila-variable.sustituida { display: none; }
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
  .confirmar {
    display: flex; align-items: center; gap: 6px; flex-wrap: wrap; cursor: default;
    padding: 5px 7px; border-radius: 4px; background: rgba(127, 29, 29, 0.35); border: 1px solid #7f1d1d;
    color: #fecaca; font-size: 10px;
  }
  .confirmar span { flex: 1; min-width: 140px; }
  .confirmar button { border: none; border-radius: 3px; padding: 3px 8px; font-size: 10px; font-weight: 700; cursor: pointer; }
  .confirmar .si { background: #b91c1c; color: white; }
  .confirmar .si:hover { background: #dc2626; }
  .confirmar .no { background: #27272a; color: #d4d4d8; }
</style>
