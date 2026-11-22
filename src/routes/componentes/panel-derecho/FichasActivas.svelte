<!-- src/routes/componentes/panel-derecho/FichasActivas.svelte -->
<!-- Las instancias activas, en su orden. Como las filas del banco pero más grandes y con la misma
     miniatura de la ficha, en la forma de su tipo (cuadrado rojo, rombo azul, círculo verde). A la
     derecha, el «actual/máx» de los parámetros con forma «Resumen».
     - Pulsar = foco (con Ctrl, añadirla o quitarla del foco). Pulsar el hueco vacío = quitar el foco.
     - Arrastrar dentro de la lista = reordenar; al tablero = colocarla; al banco = eliminarla.
     - Doble clic = la foto de la ficha a resolución completa, en una ventanita junto al puntero (se cierra pulsando fuera).
     - Clic derecho = menú con «Eliminar instancia».
     Soltar aquí una ficha del banco la activa sin colocar; una del tablero, la quita de él. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, numeroDe, veParametro, type Instancia } from "$lib/partida.svelte";
  import { ajustesVista } from "$lib/ajustesVista.svelte";
  import { esJugador } from "$lib/modo";
  import { arrastreJugador } from "$lib/arrastreJugador.svelte";
  import { arrastre, moverEnLista } from "$lib/arrastreFicha.svelte";
  import { arrastreTirada } from "$lib/arrastreTirada.svelte";
  import { miniaturas } from "$lib/miniaturas.svelte";
  import { visorImagen } from "$lib/visorImagen.svelte";
  import { tiradas } from "$lib/tiradas.svelte";
  import { grupoDe, seccionesDeBatalla, usaCuadros } from "$lib/batalla";
  import { arrastreGrupoBatalla } from "$lib/arrastreGrupoBatalla.svelte";
  import MenuContextual from "../comunes/MenuContextual.svelte";
  import VentanaBatalla from "./VentanaBatalla.svelte";

  // La miniatura de cada ficha activa (la misma que usan el banco, el arrastre y el tablero)
  $effect(() => {
    for (const i of partida.instanciasVisibles) miniaturas.asegurarFicha(i.base);
  });

  const FORMA_TIPO = { Enemigo: "cuadrado", NPC: "rombo", Jugador: "circulo", Objeto: "hexagono" } as const;

  // Parámetros que se resumen en la tarjeta, con su color
  const resumidos = $derived(
    partida.definiciones.flatMap((d) =>
      d.tiene_maximo && d.representar_mapa && "Resumen" in d.representar_mapa
        ? [{ id: d.id, clave: d.clave, color: `rgb(${d.representar_mapa.Resumen.slice(0, 3).join(",")})` }]
        : []
    )
  );

  // Mientras se arrastra una tarjeta por la lista, la lista ya se ve con el orden nuevo
  const idArrastrada = $derived.by(() => {
    const a = arrastre.actual;
    return a && "instancia" in a.origen && a.origen.desde === "activas" && a.zona === "activas" ? a.origen.instancia.id_instancia : null;
  });
  // Con «Este mapa» solo las puestas en el tablero del mapa abierto (`data-indice` es la posición en ESTA lista)
  const visuales = $derived.by(() => {
    const lista = partida.instanciasVisibles;
    if (idArrastrada === null || arrastre.actual?.indice == null) return lista;
    // En una batalla con cajas, llevarla sobre OTRA caja no la reordena (la cambiará de grupo al soltarla)
    const caja = arrastre.actual.grupoBatalla;
    if (partida.batallaActual && caja != null && caja !== (grupoDe(partida.batallaActual, idArrastrada)?.id ?? 0)) return lista;
    const ids = moverEnLista(lista.map((i) => i.id_instancia), idArrastrada, arrastre.actual.indice);
    return ids.map((id) => lista.find((i) => i.id_instancia === id)!);
  });

  let menu = $state<{ x: number; y: number; instancia: Instancia } | null>(null);

  // Batalla: en un mapa en batalla el filtro «Este mapa» es obligatorio, y el host puede empezarla o terminarla desde aquí
  const batalla = $derived(partida.batallaActual);
  let ventanaBatalla = $state(false);
  // Con varios grupos (o fichas que no participan) la lista se parte en cajas, una por grupo y una última «No participa»
  const cuadros = $derived(usaCuadros(batalla, visuales));
  const secciones = $derived(batalla && cuadros ? seccionesDeBatalla(batalla, visuales) : []);
  /** El grupo que se está renombrando (doble clic en su nombre). */
  let renombrando = $state<{ id: number; texto: string } | null>(null);

  function confirmarRenombrar() {
    const r = renombrando;
    renombrando = null; // antes de esperar: el blur que provoca quitar el campo no vuelve a entrar
    if (r && r.texto.trim()) void partida.renombrarGrupoBatalla(r.id, r.texto);
  }

  function enfocar(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  function dondeEsta(i: Instancia): string {
    if (typeof i.coordenadas !== "object" || i.id_mapa === null) return "Sin colocar";
    if (i.id_mapa === partida.mapaActual?.id) return "En este mapa";
    const mapa = partida.mapas.find((m) => m.id === i.id_mapa);
    return mapa ? `En el mapa ${mapa.nombre}` : "En otro mapa";
  }

  function alPulsar(e: MouseEvent, i: Instancia) {
    if (arrastre.esClicDeArrastre()) return;
    partida.seleccionarInstancia(i, e.ctrlKey || e.metaKey); // Ctrl: varias, como en el banco y el tablero
  }

  // Pulsar el hueco vacío de la lista (no una tarjeta) quita el foco
  function alPulsarFondo(e: PointerEvent) {
    if (e.button === 0 && !(e.target as Element).closest(".tarjeta")) partida.seleccionarInstancia(null);
  }
</script>

<div class="cabecera">
  <span class="titulo-mini">INSTANCIAS</span>
  <div class="acciones-cabecera">
  {#if !esJugador}
    {#if batalla}
      <button
        type="button"
        class="filtro batalla"
        data-conserva-foco
        title={ayuda("Termina la batalla de este mapa (todos sus grupos) y vuelve al modo normal. Es lo mismo que escribir «Terminar» en el chat")}
        onclick={() => void tiradas.tirar("Terminar")}
      >
        Terminar
      </button>
    {:else}
      <button
        type="button"
        class="filtro batalla"
        data-conserva-foco
        title={ayuda("Empieza una batalla en el mapa abierto: turnos con contador de vueltas y, si quieres, una tirada de iniciativa que ordena las fichas")}
        onclick={() => (ventanaBatalla = true)}
      >
        Empezar batalla
      </button>
    {/if}
  {/if}
  <button
    type="button"
    class="filtro"
    class:activo={ajustesVista.soloMapaActual || !!batalla}
    aria-pressed={ajustesVista.soloMapaActual || !!batalla}
    disabled={esJugador || !!batalla}
    data-conserva-foco
    title={ayuda(esJugador
      ? "Solo ves las fichas puestas en el mapa abierto"
      : batalla
      ? "Este mapa está en batalla: solo se ven las fichas puestas en él"
      : ajustesVista.soloMapaActual
      ? "Solo las fichas puestas en el mapa abierto. Pulsa para ver todas"
      : "Pulsa para ver solo las fichas puestas en el mapa abierto")}
    onclick={() => ajustesVista.alternarSoloMapaActual()}
  >
    Este mapa
  </button>
  </div>
</div>
{#if batalla && !esJugador}
  <div class="barra-grupos" data-conserva-foco>
    <button
      type="button"
      class="filtro batalla"
      title={ayuda("Un grupo nuevo, vacío, con su propio turno y su contador de vueltas. Arrastra fichas (o un grupo entero) a él")}
      onclick={() => void partida.crearGrupoBatalla()}
    >+ Grupo</button>
  </div>
{/if}
{#if batalla && !cuadros}
  <!-- Un renglón por grupo: la vuelta en la que va y, si la ficha que tiene el turno es tuya, pasarlo -->
  <div class="franja-batalla" data-conserva-foco>
    {#each batalla.grupos as g (g.id)}
      <div class="renglon-batalla">
        <span class="etiqueta-batalla">⚔ {batalla.grupos.length > 1 ? g.nombre : "Batalla"} · Turno {g.ronda}</span>
        {#if partida.puedePasarTurno(g)}
          <button
            type="button"
            class="pasar-turno"
            title={ayuda("Atajo: Enter sin tener nada en foco")}
            onclick={() => void partida.pasarTurno(g.id)}
          >Pasar turno ›</button>
        {/if}
      </div>
    {/each}
  </div>
{/if}
<div
  class="scroll-fichas"
  data-destino="activas"
  data-conserva-foco
  class:destino={arrastre.actual?.zona === "activas" && idArrastrada === null}
  role="list"
  onpointerdown={alPulsarFondo}
>
  {#snippet tarjeta(ficha: Instancia, i: number)}
    {@const forma = FORMA_TIPO[ficha.base.tipo] ?? "cuadrado"}
    <button
      type="button"
      class="tarjeta"
      class:seleccionado={partida.estaSeleccionada(ficha.id_instancia)}
      class:caida={partida.estaCaida(ficha)}
      class:oculta={ficha.oculto}
      class:foco-ajeno={partida.idsFocoAjeno.includes(ficha.id_instancia) && !partida.estaSeleccionada(ficha.id_instancia)}
      class:arrastrada={idArrastrada === ficha.id_instancia}
      class:recibe-jugador={arrastreJugador.actual?.idEntidad === ficha.base.id}
      class:turno={partida.tieneElTurno(ficha)}
      data-indice={i}
      data-id-entidad={ficha.base.id}
      onpointerdown={(e) => arrastre.preparar(e, { instancia: ficha, desde: "activas" })}
      onclick={(e) => alPulsar(e, ficha)}
      ondblclick={(e) => visorImagen.abrir(ficha.base.foto_url, e.clientX, e.clientY)}
      oncontextmenu={(e) => {
        e.preventDefault();
        if (partida.controla(ficha)) menu = { x: e.clientX, y: e.clientY, instancia: ficha };
      }}
      title={ayuda("Clic: foco\nCtrl+clic: foco en varias\nSuprimir: Instancias con foco se eliminan\nDoble clic: ver la foto\nArrastrar: reordenar o llevar al tablero\nClic derecho: eliminar")}
    >
      <div class="foto {forma}">
        {#if forma === "hexagono"}
          <!-- Hexágono: la capa de fuera es el borde gris y la de dentro lleva la foto -->
          <div class="interior-hexagono">
            {#if ficha.base.foto_url}
              {@const mini = miniaturas.deFicha(ficha.base)}
              {#if mini}
                <img src={mini} alt="" draggable="false" />
              {/if}
            {:else}
              <span class="inicial">{ficha.base.nombre.charAt(0).toUpperCase()}</span>
            {/if}
          </div>
        {:else if ficha.base.foto_url}
          {@const mini = miniaturas.deFicha(ficha.base)}
          {#if mini}
            <img src={mini} alt="" draggable="false" />
          {/if}
        {:else}
          <span class="inicial">{ficha.base.nombre.charAt(0).toUpperCase()}</span>
        {/if}
      </div>

      <div class="texto">
        <span class="nombre">{ficha.base.nombre}</span>
        <span class="donde">{dondeEsta(ficha)}</span>
      </div>

      <!-- Solo el host: ojo abierto = la ven los jugadores; cerrado = oculta (solo la ve el host). Pulsarlo alterna. -->
      {#if !esJugador}
        <span
          class="ojo"
          class:cerrado={ficha.oculto}
          role="button"
          tabindex="-1"
          aria-label={ficha.oculto ? "Mostrar a los jugadores" : "Ocultar a los jugadores"}
          aria-pressed={!!ficha.oculto}
          title={ayuda(ficha.oculto ? "Oculta: solo la ves tú. Pulsa para mostrarla a los jugadores" : "Visible para los jugadores. Pulsa para ocultarla")}
          onpointerdown={(e) => e.stopPropagation()}
          onclick={(e) => {
            e.stopPropagation();
            void partida.ocultarInstancia(ficha.id_instancia, !ficha.oculto);
          }}
          onkeydown={(e) => e.stopPropagation()}
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            {#if ficha.oculto}
              <!-- Ojo cerrado: párpado y pestañas -->
              <path d="M3 10c2.2 3.2 5.2 5 9 5s6.8-1.8 9-5" />
              <path d="M5 13l-2 3M9.5 15.2L8.5 18.5M14.5 15.2l1 3.3M19 13l2 3" />
            {:else}
              <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" />
              <circle cx="12" cy="12" r="3" />
            {/if}
          </svg>
        </span>
      {/if}

      {#if resumidos.length}
        <div class="resumen">
          {#each resumidos.filter((r) => veParametro(ficha, r.id)) as r (r.id)}
            <!-- Una tirada soltada aquí cambia este valor (ver $lib/arrastreTirada) -->
            <span
              style:color={r.color}
              title={ayuda(r.clave)}
              data-valor="{ficha.id_instancia}:{r.id}"
              class:destino-tirada={arrastreTirada.esDestino(ficha.id_instancia, r.id)}
            >{partida.valorMostrado(ficha, r.id)}/{numeroDe(ficha.base, r.id)}</span>
          {/each}
        </div>
      {/if}

          </button>
  {/snippet}

  {#if cuadros}
    {#each secciones as sec (sec.grupo?.id ?? 0)}
      {@const g = sec.grupo}
      {@const id = g?.id ?? 0}
      <div
        class="cuadro"
        class:fuera={!g}
        class:sobre={arrastre.actual?.grupoBatalla === id || arrastreGrupoBatalla.actual?.sobre === id}
        class:fantasma={arrastreGrupoBatalla.actual?.id === id}
        data-grupo-batalla={id}
      >
        {#if g}
          <!-- La cabecera arrastra el grupo entero sobre otro (el host) -->
          <div class="cab-cuadro" role="presentation" onpointerdown={(e) => arrastreGrupoBatalla.preparar(e, g)}>
            {#if !esJugador}<span class="asa-cuadro">⋮⋮</span>{/if}
            {#if renombrando?.id === g.id}
              <input
                class="renombrar-cuadro"
                maxlength="30"
                aria-label="Nombre del grupo"
                bind:value={renombrando.texto}
                use:enfocar
                onkeydown={(e) => {
                  e.stopPropagation();
                  if (e.key === "Enter") confirmarRenombrar();
                  else if (e.key === "Escape") renombrando = null;
                }}
                onblur={confirmarRenombrar}
              />
            {:else}
              <span
                class="nombre-cuadro"
                role="presentation"
                title={ayuda(esJugador ? undefined : "Doble clic: cambiar el nombre. Arrastra la cabecera sobre otro grupo para meterlo en él")}
                ondblclick={() => !esJugador && (renombrando = { id: g.id, texto: g.nombre })}
              >{g.nombre} · Turno {g.ronda}</span>
            {/if}
            {#if partida.puedePasarTurno(g)}
              <button type="button" class="pasar-turno" title={ayuda("Pasa el turno a la ficha siguiente de este grupo (atajo: Enter sin tener nada en foco)")} onclick={() => void partida.pasarTurno(g.id)}>Pasar turno ›</button>
            {/if}
            {#if !esJugador && batalla && batalla.grupos.length > 1}
              <button type="button" class="quitar-cuadro" aria-label="Borrar el grupo" title={ayuda("Borra el grupo: sus fichas dejan de participar")} onclick={() => void partida.borrarGrupoBatalla(g.id)}>✕</button>
            {/if}
          </div>
        {:else}
          <div class="cab-cuadro fuera-cab"><span class="nombre-cuadro">No participa</span></div>
        {/if}
        {#each sec.fichas as ficha (ficha.id_instancia)}
          {@render tarjeta(ficha, visuales.indexOf(ficha))}
        {:else}
          <span class="vacio-cuadro">{g ? "Arrastra aquí las fichas de este grupo" : "Las fichas que no participan están aquí"}</span>
        {/each}
      </div>
    {/each}
  {:else}
    {#each visuales as ficha, i (ficha.id_instancia)}
      {@render tarjeta(ficha, i)}
    {/each}
  {/if}

  {#if partida.instancias.length === 0}
    <span class="vacio">Arrastra aquí o al tablero una ficha del banco</span>
  {:else if visuales.length === 0}
    <span class="vacio">Ninguna ficha puesta en este mapa</span>
  {/if}
</div>

{#if arrastreGrupoBatalla.actual}
  <div class="grupo-flotante" style="left: {arrastreGrupoBatalla.actual.x + 12}px; top: {arrastreGrupoBatalla.actual.y + 12}px;">⚔ {arrastreGrupoBatalla.actual.nombre}</div>
{/if}

{#if ventanaBatalla && !esJugador}
  <VentanaBatalla onCerrar={() => (ventanaBatalla = false)} />
{/if}

{#if menu}
  {@const elegida = menu.instancia}
  <div data-conserva-foco>
    <MenuContextual
      x={menu.x}
      y={menu.y}
      opciones={[{ texto: "Eliminar instancia", peligro: true, accion: () => void partida.borrarInstancia(elegida.id_instancia) }]}
      onCerrar={() => (menu = null)}
    />
  </div>
{/if}

<style>
  .cabecera { display: flex; justify-content: space-between; align-items: center; margin-bottom: 3px; }
  .titulo-mini { font-size: 10px; font-weight: bold; color: #71717a; display: block; }
  /* Mismo aspecto que los filtros rápidos del banco */
  .filtro {
    background: #1c1c1f;
    border: 1px solid #27272a;
    color: #a1a1aa;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    cursor: pointer;
    line-height: normal;
  }
  .filtro:hover:not(:disabled) { background: #27272a; color: #f4f4f5; }
  .filtro:disabled { cursor: default; }
  .acciones-cabecera { display: flex; gap: 4px; align-items: center; }
  .filtro.batalla { color: #60a5fa; border-color: #1e3a8a; }
  .filtro.batalla:hover:not(:disabled) { background: rgba(59, 130, 246, 0.15); color: #bfdbfe; }
  /* La batalla: renglón por grupo (vuelta y pasar el turno) y la ficha que tiene el turno, remarcada a la derecha */
  .franja-batalla { display: flex; flex-direction: column; gap: 2px; margin-bottom: 4px; }
  .renglon-batalla {
    display: flex; align-items: center; justify-content: space-between; gap: 6px;
    padding: 3px 8px; border-radius: 4px; background: rgba(59, 130, 246, 0.1); border: 1px solid #1e3a8a;
  }
  .etiqueta-batalla { font-size: 11px; font-weight: bold; color: #93c5fd; }
  .pasar-turno {
    background: #2563eb; border: none; color: white; border-radius: 3px; padding: 2px 8px;
    font-size: 10px; font-weight: 700; cursor: pointer;
  }
  .pasar-turno:hover { background: #3b82f6; }
  /* La ficha con el turno: relleno azulado y la barra ancha a su derecha (sin remarcar el borde) */
  /* Las cajas de la batalla: un recuadro por grupo que engloba a sus fichas y otro, discontinuo, para las que no participan */
  .barra-grupos { display: flex; justify-content: flex-end; margin-bottom: 3px; }
  .cuadro {
    display: flex; flex-direction: column; gap: 4px; flex-shrink: 0;
    padding: 4px; border-radius: 6px; border: 1px solid #1e3a8a; background: rgba(59, 130, 246, 0.05);
  }
  .cuadro.fuera { border-style: dashed; border-color: #3f3f46; background: rgba(255, 255, 255, 0.02); }
  .cuadro.sobre { border-color: #60a5fa; background: rgba(96, 165, 250, 0.14); }
  .cuadro.fantasma { opacity: 0.4; }
  .cab-cuadro { display: flex; align-items: center; gap: 6px; padding: 1px 2px; cursor: grab; }
  .cab-cuadro.fuera-cab { cursor: default; }
  .asa-cuadro { color: #52525b; font-size: 11px; }
  .nombre-cuadro { flex: 1; font-size: 11px; font-weight: bold; color: #93c5fd; user-select: none; }
  .fuera-cab .nombre-cuadro { color: #71717a; }
  .renombrar-cuadro {
    flex: 1; min-width: 0; padding: 1px 4px; border-radius: 3px; font-size: 11px;
    background: #09090b; border: 1px solid #3b82f6; color: white;
  }
  .quitar-cuadro { background: none; border: none; color: #71717a; cursor: pointer; font-size: 11px; padding: 0 2px; }
  .quitar-cuadro:hover { color: #f87171; }
  .vacio-cuadro { color: #52525b; font-size: 10px; text-align: center; padding: 6px 0; }
  .grupo-flotante {
    position: fixed; z-index: 9999; pointer-events: none; padding: 3px 8px; border-radius: 4px; font-size: 11px; font-weight: bold;
    background: #1e3a8a; color: #dbeafe; border: 1px solid #3b82f6; box-shadow: 0 6px 16px rgba(0, 0, 0, 0.6);
  }
  .tarjeta.turno { background: rgba(59, 130, 246, 0.14); box-shadow: inset -4px 0 0 #3b82f6; }
  .filtro.activo { background: #27272a; color: #ffffff; border-color: #3f3f46; font-weight: bold; }
  .scroll-fichas {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border: 1px dashed transparent;
    border-radius: 4px;
  }
  .scroll-fichas.destino { border-color: #60a5fa; background: rgba(96, 165, 250, 0.06); }

  .tarjeta {
    display: flex;
    align-items: center;
    gap: 10px;
    background: #1f1f23;
    padding: 6px 8px;
    border-radius: 5px;
    cursor: pointer;
    border: 1px solid transparent;
    width: 100%;
    color: #f4f4f5;
    font: inherit;
    text-align: left;
    flex-shrink: 0;
    touch-action: none;
  }
  .tarjeta:hover { background: #27272a; border-color: #3f3f46; }
  .tarjeta:focus-visible { outline: none; } /* el estado lo marca solo .seleccionado */
  .tarjeta.seleccionado { border-color: #3b82f6; }
  /* Sin vida (0 o negativa): en gris. Los textos de vida se siguen leyendo */
  .tarjeta.caida .foto, .tarjeta.caida .texto { opacity: 0.4; filter: grayscale(100%); }
  /* El foco de otro jugador (llega por red) */
  .tarjeta.foco-ajeno { border-color: #a855f7; border-style: dotted; }
  /* El hueco donde caerá la tarjeta que se está reordenando */
  .tarjeta.arrastrada { opacity: 0.35; border-style: dashed; border-color: #60a5fa; }
  /* Se está soltando encima un jugador (pasará a controlar esta ficha) */
  .tarjeta.recibe-jugador { border-color: #22c55e; background: rgba(34, 197, 94, 0.12); }

  /* Foto dentro de la forma de su tipo (como en el banco, más grande) */
  .foto {
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 2px solid;
    box-sizing: border-box;
  }
  .foto img { width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
  .inicial { font-weight: bold; font-size: 15px; }
  .foto.cuadrado { border-color: #ef4444; background: #7f1d1d; border-radius: 4px; }
  .foto.circulo { border-color: #22c55e; background: #14532d; border-radius: 50%; }
  .foto.rombo {
    width: 30px;
    height: 30px;
    margin: 4px;
    transform: rotate(45deg);
    border-color: #3b82f6;
    background: #1e3a8a;
    border-radius: 3px;
  }
  .foto.rombo img, .foto.rombo .inicial { transform: rotate(-45deg) scale(1.42); }
  /* Hexágono gris (Objeto): el borde es el fondo de la capa de fuera, recortada igual que la de dentro */
  .foto.hexagono {
    padding: 2px;
    border: none;
    background: #6d6b65;
    clip-path: polygon(50% 0, 100% 25%, 100% 75%, 50% 100%, 0 75%, 0 25%);
  }
  .interior-hexagono {
    width: 100%;
    height: 100%;
    background: #3f3f46;
    clip-path: polygon(50% 0, 100% 25%, 100% 75%, 50% 100%, 0 75%, 0 25%);
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .texto { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .nombre { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: bold; }
  .donde { font-size: 10px; color: #71717a; }

  /* Una oculta se ve algo apagada (el host la ve al 50 % también en el tablero) */
  .tarjeta.oculta .foto, .tarjeta.oculta .texto { opacity: 0.55; }
  .ojo {
    flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    width: 24px; height: 24px; border-radius: 4px; color: #a1a1aa; cursor: pointer;
  }
  .ojo:hover { background: #3f3f46; color: #f4f4f5; }
  .ojo.cerrado { color: #f59e0b; }

  .resumen { display: flex; flex-direction: column; align-items: flex-end; gap: 1px; font-size: 12px; font-weight: bold; }
  .resumen span { padding: 0 3px; border-radius: 3px; }
  .resumen .destino-tirada { background: rgba(250, 204, 21, 0.25); outline: 1px solid #facc15; }
  .vacio { color: #52525b; font-size: 10px; text-align: center; margin-top: 10px; }
</style>