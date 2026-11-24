<!-- src/routes/componentes/PanelIzquierdo.svelte -->
<!-- Solo la estructura del panel: cada parte vive en su componente y lee el estado de `partida`. -->
<script lang="ts">
  import { partida } from "$lib/partida.svelte";
  import { ajustesVista } from "$lib/ajustesVista.svelte";
  import ListaZonas from "./panel-izquierdo/ListaZonas.svelte";
  import ListaMapas from "./panel-izquierdo/ListaMapas.svelte";
  import BancoEntidades from "./panel-izquierdo/BancoEntidades.svelte";
  import BotonAjustes from "./panel-izquierdo/BotonAjustes.svelte";
  import VentanaParametros from "./VentanaParametros.svelte";
  import VentanaPartidas from "./VentanaPartidas.svelte";
  import VentanaSala from "./VentanaSala.svelte";
  import VentanaColorFlecha from "./VentanaColorFlecha.svelte";
  import VentanaMacros from "./VentanaMacros.svelte";
  import { macros } from "$lib/macros.svelte";
  import { sala } from "$lib/sala.svelte";
  import { esJugador } from "$lib/modo";

  let parametrosAbierto = $state(false);
  let partidasAbierto = $state(false);
  let colorAbierto = $state(false);

  // Más opciones de configuración se añaden aquí
  const opcionesHost = $derived([
    {
      texto: "Partidas…",
      accion: () => (partidasAbierto = true),
      // Una ficha nueva abierta tiene su carpeta en esta partida: no puede acompañarte a otra. Y los
      // jugadores de la sala están en esta partida
      deshabilitada: sala.enMarcha
        ? "Cierra la sala antes de cambiar de partida"
        : partida.entidadEnEdicion
          ? "Cierra la ficha que tienes abierta"
          : undefined
    },
    {
      texto: "Configuración de sala…",
      accion: () => (sala.ventanaAbierta = true)
    },
    {
      texto: "Editar parámetros de esta partida",
      accion: () => (parametrosAbierto = true),
      // Una ficha abierta ya copió los parámetros: se cambian cuando está cerrada
      deshabilitada: partida.entidadEnEdicion ? "Cierra la ficha que tienes abierta" : undefined
    },
    {
      texto: ajustesVista.edicionRejilla ? "Ocultar edición de la rejilla" : "Mostrar edición de la rejilla",
      accion: () => {
        // Un cambio de rejilla a medias se guarda ya, no al cabo de 5 s con la barra oculta
        if (ajustesVista.edicionRejilla) void partida.guardarRejilla();
        ajustesVista.alternarEdicionRejilla();
      }
    }
  ]);

  // Lo que cada uno elige para sí: también lo tienen los jugadores
  const opcionTutoriales = $derived({
    texto: ajustesVista.tutoriales ? "Quitar tutoriales" : "Mostrar tutoriales",
    accion: () => ajustesVista.alternarTutoriales()
  });
  // Mis preferencias (color de mi flecha y de mi ping, la rueda en los campos de una ficha...): cada uno las suyas (host y jugadores)
  const opcionColor = {
    texto: "Configuración de usuario…",
    accion: () => (colorAbierto = true)
  };
  // Las macros: el host y cada jugador escriben las suyas
  const opcionMacros = { texto: "Macros…", accion: () => macros.abrirVentana() };
  const opciones = $derived(
    esJugador ? [opcionMacros, opcionColor, opcionTutoriales] : [...opcionesHost, opcionMacros, opcionColor, opcionTutoriales]
  );
</script>

<aside class="panel-izquierdo">
  <div class="bloque-arriba">
    <div class="subseccion"><ListaZonas /></div>
    <div class="subseccion"><ListaMapas /></div>
  </div>

  <div class="bloque-abajo">
    <BancoEntidades />
  </div>

  <div class="pie">
    <BotonAjustes {opciones} />
  </div>
</aside>

{#if parametrosAbierto}
  <VentanaParametros onCerrar={() => (parametrosAbierto = false)} />
{/if}

{#if partidasAbierto}
  <VentanaPartidas onCerrar={() => (partidasAbierto = false)} />
{/if}

{#if sala.ventanaAbierta}
  <VentanaSala />
{/if}

{#if colorAbierto}
  <VentanaColorFlecha onCerrar={() => (colorAbierto = false)} />
{/if}

{#if macros.ventana}
  <VentanaMacros onCerrar={() => (macros.ventana = null)} />
{/if}

<style>
  .panel-izquierdo {
    width: clamp(180px, 14vw, 225px);
    height: 100%;
    background: #141416;
    border-right: 1px solid #27272a;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    padding: 6px;
    gap: 6px;
    user-select: none;
    font-size: 11px;
  }
  .bloque-arriba { height: 44%; display: flex; gap: 4px; border-bottom: 1px solid #27272a; padding-bottom: 4px; }
  .subseccion { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .bloque-abajo { flex: 1; display: flex; flex-direction: column; overflow: hidden; gap: 5px; }
  .pie { display: flex; justify-content: flex-start; border-top: 1px solid #27272a; padding-top: 4px; }
</style>
