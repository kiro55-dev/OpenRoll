<!-- src/routes/componentes/comunes/TextoConvirtiendo.svelte -->
<!-- «Convirtiendo imagen...» con los tres puntos apareciendo y desapareciendo, como una pantalla de carga. -->
<script lang="ts">
  import { TEXTO_CONVIRTIENDO } from "$lib/conversionImagen.svelte";
</script>

<span role="status" aria-label={TEXTO_CONVIRTIENDO}>{TEXTO_CONVIRTIENDO}<span class="puntos" aria-hidden="true"><i>.</i><i>.</i><i>.</i></span></span>

<style>
  .puntos i { font-style: normal; animation: punto 1.2s infinite both; }
  .puntos i:nth-child(2) { animation-delay: 0.2s; }
  .puntos i:nth-child(3) { animation-delay: 0.4s; }
  @keyframes punto {
    0%, 20% { opacity: 0; }
    40%, 80% { opacity: 1; }
    100% { opacity: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .puntos i { animation: none; }
  }
</style>
