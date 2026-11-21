<!-- src/routes/componentes/panel-derecho/VentanaBatalla.svelte -->
<!-- Ventana flotante (no modal) para empezar una batalla en el mapa abierto: la tirada de iniciativa (opcional) con la que cada ficha
     se ordena en «Fichas activas». Todo lo que se pone aquí se manda al chat como el comando `Batalla <tirada>`, igual que si se
     escribiera a mano. Solo el host. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida } from "$lib/partida.svelte";
  import { tiradas } from "$lib/tiradas.svelte";
  import { moverVentana } from "$lib/moverVentana";

  let { onCerrar }: { onCerrar: () => void } = $props();

  let ventana = $state<HTMLDivElement | null>(null);
  let campo = $state<HTMLInputElement | null>(null);
  let pos = $state({ x: 420, y: 90 });
  let tirada = $state("");
  let empezando = $state(false);
  let aviso = $state("");

  /** Lo que se manda al chat: `Batalla` o `Batalla 1d20 + des`. */
  const comando = $derived(`Batalla ${tirada.trim()}`.trim());

  async function empezar() {
    if (empezando) return;
    empezando = true;
    aviso = "";
    try {
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
    else if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT") {
      e.preventDefault();
      void empezar();
    }
  }

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
      placeholder="p. ej. 1d20 + Destreza"
      bind:this={campo}
      title={ayuda("Cada ficha tira con SUS variables, como en cualquier tirada del chat. Se ordenan de mayor a menor; en un empate se respeta el orden que ya tenían. Vacío: no se reordenan")}
      bind:value={tirada}
    />
    <p class="ayuda">
      Participan las fichas puestas en este mapa que no están ocultas. Se manda al chat como «<b>{comando}</b>»; con «Terminar» acaba.
    </p>

    {#if aviso}<p class="aviso" role="alert">{aviso}</p>{/if}

    <div class="botones">
      <button type="button" class="btn-cancelar" onclick={onCerrar}>Cancelar</button>
      <button type="button" class="btn-empezar" onclick={empezar} disabled={empezando}>Empezar</button>
    </div>
  </div>
</div>

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
