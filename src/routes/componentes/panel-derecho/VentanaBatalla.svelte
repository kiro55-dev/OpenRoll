<!-- src/routes/componentes/panel-derecho/VentanaBatalla.svelte -->
<!-- Ventana flotante (no modal) para empezar una batalla en el mapa abierto: la tirada de iniciativa (opcional) con la que cada ficha
     se ordena en «Fichas activas». La tirada se manda al chat como el comando `Batalla <tirada>`, igual que si se escribiera a mano.
     Con «Organizar por grupos» (opcional) se reparten las fichas en grupos con su turno y su contador; eso NO pasa por el chat. Solo
     el host. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { estaEnMapa, partida } from "$lib/partida.svelte";
  import { invoke } from "$lib/backend";
  import { tiradas } from "$lib/tiradas.svelte";
  import { moverVentana } from "$lib/moverVentana";
  import { arrastre } from "$lib/arrastreFicha.svelte";

  let { onCerrar }: { onCerrar: () => void } = $props();

  let ventana = $state<HTMLDivElement | null>(null);
  let campo = $state<HTMLInputElement | null>(null);
  let pos = $state({ x: 420, y: 90 });
  let tirada = $state("");
  let empezando = $state(false);
  let aviso = $state("");

  // ---- Organizar por grupos (por defecto no se ve: una batalla sin esto es de un solo grupo) ----
  let porGrupos = $state(false);
  let siguienteClave = 2;
  let grupos = $state<{ clave: number; nombre: string }[]>([{ clave: 1, nombre: "Grupo 1" }]);
  /** A qué grupo (su clave) va cada ficha; 0 = no participa. Sin entrada, al primero. */
  let asignacion = $state<Record<number, number>>({});
  const MAX_GRUPOS = 10;
  /** Las fichas que pueden participar: las puestas en este mapa y a la vista, en su orden. */
  const fichas = $derived(partida.instanciasOrdenadas.filter((i) => estaEnMapa(i, partida.mapaActual?.id) && !i.oculto));
  const grupoDeFicha = (id: number) => asignacion[id] ?? grupos[0]?.clave ?? 0;

  /** Las fichas elegidas (Ctrl+clic) para llevarlas juntas, y el arrastre en marcha. */
  let seleccion = $state<number[]>([]);
  let arrastrando = $state<{ ids: number[]; x: number; y: number; sobre: number | null } | null>(null);

  function cajaEn(x: number, y: number): number | null {
    const caja = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-caja-conf]");
    return caja?.dataset.cajaConf === undefined ? null : Number(caja.dataset.cajaConf);
  }

  /** Pulsar una ficha: Ctrl la añade o la quita de la elección; sin Ctrl, se elige ella sola (si no estaba ya) y, al arrastrarla,
   *  se llevan todas las elegidas a la caja donde se suelte. */
  function pulsarFicha(e: PointerEvent, id: number) {
    if (e.button !== 0) return;
    if (e.ctrlKey || e.metaKey) {
      seleccion = seleccion.includes(id) ? seleccion.filter((x) => x !== id) : [...seleccion, id];
      return;
    }
    if (!seleccion.includes(id)) seleccion = [id];
    const ids = [...seleccion];
    const inicio = { x: e.clientX, y: e.clientY };
    let empezado = false;
    const mover = (ev: PointerEvent) => {
      if (!empezado) {
        if (Math.hypot(ev.clientX - inicio.x, ev.clientY - inicio.y) <= 5) return;
        empezado = true;
      }
      arrastrando = { ids, x: ev.clientX, y: ev.clientY, sobre: cajaEn(ev.clientX, ev.clientY) };
    };
    const quitar = () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", cancelar);
    };
    const soltar = () => {
      quitar();
      const sobre = arrastrando?.sobre ?? null;
      arrastrando = null;
      if (empezado && sobre !== null) {
        for (const x of ids) asignacion[x] = sobre; // todas de golpe
        seleccion = [];
      }
    };
    const cancelar = () => {
      quitar();
      arrastrando = null;
    };
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", cancelar);
  }

  function anadirGrupo() {
    if (grupos.length >= MAX_GRUPOS) return;
    grupos.push({ clave: siguienteClave, nombre: `Grupo ${siguienteClave}` });
    siguienteClave++;
  }

  function quitarGrupo(clave: number) {
    if (grupos.length <= 1) return;
    grupos = grupos.filter((g) => g.clave !== clave);
    for (const id of Object.keys(asignacion).map(Number)) if (asignacion[id] === clave) delete asignacion[id]; // vuelven al primero
    seleccion = [];
  }

  /** Lo que se manda al chat: `Batalla` o `Batalla 1d20 + des`. */
  const comando = $derived(`Batalla ${tirada.trim()}`.trim());

  async function empezar() {
    if (empezando) return;
    empezando = true;
    aviso = "";
    try {
      if (porGrupos) {
        // El reparto va directo a Rust; el chat solo recibe la orden y su resultado
        const reparto = grupos.map((g) => ({ nombre: g.nombre, miembros: fichas.filter((f) => grupoDeFicha(f.id_instancia) === g.clave).map((f) => f.id_instancia) }));
        try {
          await invoke("empezar_batalla", { comando, grupos: reparto });
          await tiradas.cargar();
          await tiradas.alCambiarBatalla();
          onCerrar();
        } catch (err) {
          aviso = typeof err === "string" ? err : "No se ha podido empezar la batalla";
        }
        return;
      }
      await tiradas.tirar(comando);
      if (partida.batallaActual) onCerrar();
      else aviso = "No se ha podido empezar: el motivo está en el chat."; // Rust lo dice ahí («tirada no válida: …»)
    } finally {
      empezando = false;
    }
  }

  function alTeclear(e: KeyboardEvent) {
    e.stopPropagation(); // Enter y Escape son de esta ventana, no del tablero
    if (e.key === "Escape") onCerrar();
    else if (e.key === "Enter" && (e.target as HTMLElement).id === "b-tirada") {
      e.preventDefault();
      void empezar();
    }
  }

  // Una instancia de «Fichas activas» soltada sobre un grupo de esta ventana (o varias, las del foco): se asignan de golpe, y si ya
  // estaban en otro grupo, cambian
  $effect(() => {
    const alAsignar = (e: Event) => {
      const { ids, clave } = (e as CustomEvent<{ ids: number[]; clave: number }>).detail;
      if (clave !== 0 && !grupos.some((g) => g.clave === clave)) return;
      for (const id of ids) if (fichas.some((f) => f.id_instancia === id)) asignacion[id] = clave; // solo las que pueden participar
    };
    window.addEventListener("batalla-asignar", alAsignar);
    return () => window.removeEventListener("batalla-asignar", alAsignar);
  });

  // Al abrirla, el foco va directo al campo de la tirada (así se escribe y Enter empieza)
  $effect(() => campo?.focus());
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="ventana-flotante"
  style="left: {pos.x}px; top: {pos.y}px;"
  role="dialog"
  aria-label="Empezar batalla"
  tabindex="-1"
  data-conserva-foco
  onkeydown={alTeclear}
  bind:this={ventana}
>
  <div class="cabecera" role="presentation" onpointerdown={(e) => moverVentana(e, pos, ventana)}>
    <span class="icono-drag">⋮⋮</span>
    <span class="titulo">Empezar batalla</span>
    <button type="button" class="btn-cerrar" aria-label="Cerrar" onclick={onCerrar}>✕</button>
  </div>

  <div class="cuerpo">
    <label for="b-tirada">Tirada para ordenar las fichas (opcional):</label>
    <input
      id="b-tirada"
      type="text"
      class="input-texto"
      maxlength="120"
      placeholder="p. ej. 1d20 + Destreza o 1d20 + @velocidad"
      bind:this={campo}
      title={ayuda("Cada ficha tira con SUS variables y parámetros visuales (con @: 1d20 + @velocidad), como en cualquier tirada del chat. Se ordenan de mayor a menor; en un empate se respeta el orden que ya tenían. Vacío: no se reordenan")}
      bind:value={tirada}
    />
    <p class="ayuda">
      Participan las fichas puestas en este mapa que no están ocultas. Se manda al chat como «<b>{comando}</b>»; con «Terminar» acaba.
    </p>

    <label class="check" title={ayuda("Reparte las fichas en grupos: cada uno con su turno y su contador de vueltas, y cada uno se ordena por su cuenta con la tirada. Sin esto, la batalla es de un solo grupo")}>
      <input type="checkbox" bind:checked={porGrupos} />
      <span>Organizar por grupos</span>
    </label>

    {#if porGrupos}
      <!-- Cajas: una por grupo y «No participa». Se arrastran fichas (una, o varias con Ctrl+clic) de una caja a otra y se asignan de golpe -->
      <div class="config-grupos">
        <p class="ayuda">Arrastra fichas a un grupo para asignarlas de golpe: desde «Instancias» (las del foco van juntas) o entre los grupos de aquí (Ctrl+clic para varias). Si ya estaban en otro grupo, cambian.</p>
        {#each grupos as g (g.clave)}
          {@render caja(g.clave, g)}
        {/each}
        {#if grupos.length < MAX_GRUPOS}
          <button type="button" class="anadir" onclick={anadirGrupo}>+ Añadir grupo</button>
        {/if}
        {@render caja(0, null)}
      </div>
    {/if}

    {#if aviso}<p class="aviso" role="alert">{aviso}</p>{/if}

    <div class="botones">
      <button type="button" class="btn-cancelar" onclick={onCerrar}>Cancelar</button>
      <button type="button" class="btn-empezar" onclick={empezar} disabled={empezando}>Empezar</button>
    </div>
  </div>
</div>

{#snippet caja(clave: number, g: { clave: number; nombre: string } | null)}
  <div class="caja" class:fuera={!g} class:sobre={arrastrando?.sobre === clave || arrastre.actual?.cajaConf === clave} data-caja-conf={clave}>
    <div class="cab-caja">
      {#if g}
        <input type="text" class="input-texto nombre-grupo" maxlength="30" aria-label="Nombre del grupo" bind:value={g.nombre} />
        {#if grupos.length > 1}
          <button type="button" class="quitar" aria-label="Quitar el grupo" title={ayuda("Quitar el grupo: sus fichas pasan al primero")} onclick={() => quitarGrupo(g.clave)}>✕</button>
        {/if}
      {:else}
        <span class="nombre-fuera">No participa</span>
      {/if}
    </div>
    <div class="fichas-caja">
      {#each fichas.filter((f) => grupoDeFicha(f.id_instancia) === clave) as f (f.id_instancia)}
        <div
          class="ficha-conf"
          class:elegida={seleccion.includes(f.id_instancia)}
          role="presentation"
          onpointerdown={(e) => pulsarFicha(e, f.id_instancia)}
        >{f.base.nombre}</div>
      {:else}
        <span class="vacia">Suelta aquí las fichas</span>
      {/each}
    </div>
  </div>
{/snippet}

{#if arrastrando}
  <div class="flotante" style="left: {arrastrando.x + 12}px; top: {arrastrando.y + 12}px;">{arrastrando.ids.length === 1 ? "1 ficha" : `${arrastrando.ids.length} fichas`}</div>
{/if}

<style>
  .ventana-flotante {
    position: fixed; z-index: 1000; width: 340px;
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
  .cuerpo { padding: 10px; display: flex; flex-direction: column; gap: 8px; }
  .input-texto {
    padding: 4px 6px; border-radius: 4px; font-size: 12px; user-select: text;
    background: #09090b; border: 1px solid #3f3f46; color: white;
  }
  .input-texto:focus { outline: none; border-color: #3b82f6; }
  .check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
  .check input { accent-color: #3b82f6; cursor: pointer; }
  .config-grupos { display: flex; flex-direction: column; gap: 5px; max-height: 280px; overflow-y: auto; scrollbar-width: thin; scrollbar-color: #3f3f46 transparent; }
  .quitar { background: none; border: none; color: #71717a; cursor: pointer; font-size: 12px; }
  .quitar:hover { color: #f87171; }
  .anadir {
    align-self: flex-start; background: transparent; border: 1px dashed #52525b; color: #a1a1aa; border-radius: 4px;
    padding: 2px 8px; font-size: 10px; font-weight: 600; cursor: pointer;
  }
  .anadir:hover { border-color: #3b82f6; color: white; }
  .caja { display: flex; flex-direction: column; gap: 4px; padding: 4px; border-radius: 6px; border: 1px solid #1e3a8a; background: rgba(59, 130, 246, 0.05); }
  .caja.fuera { border-style: dashed; border-color: #3f3f46; background: rgba(255, 255, 255, 0.02); }
  .caja.sobre { border-color: #60a5fa; background: rgba(96, 165, 250, 0.16); }
  .cab-caja { display: flex; align-items: center; gap: 4px; }
  .nombre-grupo { flex: 1; min-width: 0; font-size: 11px; font-weight: bold; }
  .nombre-fuera { font-size: 11px; font-weight: bold; color: #71717a; padding: 2px 4px; }
  .fichas-caja { display: flex; flex-wrap: wrap; gap: 4px; min-height: 22px; }
  .ficha-conf {
    padding: 2px 8px; border-radius: 4px; font-size: 11px; cursor: grab; background: #27272a; border: 1px solid #3f3f46; touch-action: none;
  }
  .ficha-conf:hover { border-color: #52525b; }
  .ficha-conf.elegida { border-color: #3b82f6; background: rgba(59, 130, 246, 0.18); }
  .vacia { color: #52525b; font-size: 10px; padding: 3px 2px; }
  .flotante {
    position: fixed; z-index: 9999; pointer-events: none; padding: 3px 8px; border-radius: 4px; font-size: 11px; font-weight: bold;
    background: #1e3a8a; color: #dbeafe; border: 1px solid #3b82f6; box-shadow: 0 6px 16px rgba(0, 0, 0, 0.6);
  }
  .ayuda { margin: 0; color: #a1a1aa; font-size: 10px; }
  .ayuda b { color: #e4e4e7; user-select: text; }
  .aviso { margin: 0; color: #f87171; font-size: 11px; }
  .botones { display: flex; justify-content: flex-end; gap: 6px; }
  .btn-cancelar, .btn-empezar {
    border: none; padding: 5px 12px; border-radius: 4px; font-size: 11px; font-weight: 600; cursor: pointer;
  }
  .btn-cancelar { background: #27272a; color: #d4d4d8; }
  .btn-cancelar:hover { background: #3f3f46; }
  .btn-empezar { background: #16a34a; color: white; }
  .btn-empezar:hover { background: #15803d; }
  .btn-empezar:disabled { opacity: 0.6; cursor: default; }
</style>
