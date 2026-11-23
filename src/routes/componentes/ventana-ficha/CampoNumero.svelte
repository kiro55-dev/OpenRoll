<!-- src/routes/componentes/ventana-ficha/CampoNumero.svelte -->
<!-- Campo numérico que también se ajusta con la rueda del ratón, pero SOLO cuando tiene el foco (has hecho clic dentro): así, hacer scroll por
     la ventana pasando por encima de los campos no cambia ningún valor sin querer. -->
<script lang="ts">
  import { ajustesVista, ayuda } from "$lib/ajustesVista.svelte";
  let {
    value = $bindable(),
    etiqueta = "",
    sufijo = "",
    min = 0,
    max = 32767,
    estrecho = false,
    ancho = 0,
    soloLectura = false,
    titulo = "Haz clic en el campo y usa la rueda del ratón para subir/bajar"
  }: {
    value: number;
    etiqueta?: string;
    sufijo?: string;
    min?: number;
    max?: number;
    /** Más estrecho y centrado (para el cuerpo X × Y) */
    estrecho?: boolean;
    /** Ancho del campo en px (0 = el de siempre). */
    ancho?: number;
    /** Se ve el valor pero no se escribe (un parámetro que se calcula con las variables). */
    soloLectura?: boolean;
    titulo?: string;
  } = $props();

  function alRueda(e: WheelEvent) {
    // Sin el foco, la rueda hace scroll como en el resto de la ventana (salvo que el usuario lo haya pedido en su configuración)
    if (!ajustesVista.ruedaSinFoco && document.activeElement !== e.currentTarget) return;
    e.preventDefault();
    if (soloLectura) return;
    const paso = e.deltaY < 0 ? 1 : -1;
    value = Math.min(max, Math.max(min, Math.round(Number(value) || 0) + paso));
  }
</script>

<label title={ayuda(titulo)}>
  {etiqueta}
  <input type="number" class:estrecho class:calculado={soloLectura} style:width={ancho ? `${ancho}px` : null} {min} {max} readonly={soloLectura} bind:value onwheel={alRueda} />
  {#if sufijo}<span class="sufijo">{sufijo}</span>{/if}
</label>

<style>
  label { color: #a1a1aa; font-size: 10px; display: flex; align-items: center; gap: 4px; }
  input {
    width: 44px; background: #09090b; border: 1px solid #3f3f46; color: white;
    padding: 2px 4px; border-radius: 4px; font-size: 11px;
  }
  input.estrecho { width: 34px; text-align: center; }
  input.calculado { color: #93c5fd; background: #111113; cursor: default; }
  .sufijo { color: #71717a; font-size: 10px; }
</style>
