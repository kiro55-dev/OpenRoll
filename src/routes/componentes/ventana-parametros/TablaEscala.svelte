<!-- src/routes/componentes/ventana-parametros/TablaEscala.svelte -->
<!-- Pestaña «Resultados» de la ventana de parámetros: cómo se NOMBRA el resultado de una comparación del chat (`1d100 <= 45`,
     `1d20 >= 12`, `1d100 vs acro`). El chat solo compara; esto es de cada rol (va en el conjunto de parámetros). Sin escala, el
     resultado es solo «Éxito» o «Fallo». Con ella: niveles de acierto al tirar por debajo (extremo 1/5, difícil 1/2...) y reglas
     sobre la cara de un dado suelto (crítico, pifia...). Se edita aquí y se guarda entera con «Guardar»; Rust la valida y da el
     motivo si algo no vale. -->
<script lang="ts">
  import { ayuda } from "$lib/ajustesVista.svelte";
  import { partida, type EfectoNatural, type EscalaResultado } from "$lib/partida.svelte";

  let { onError }: { onError: (mensaje: string) => void } = $props();

  const EFECTOS: { valor: EfectoNatural; texto: string; ayuda: string }[] = [
    { valor: "SoloSiAcierta", texto: "Solo si acierta", ayuda: "Se nombra solo si la comparación acierta (el crítico de Cthulhu)" },
    { valor: "SoloSiFalla", texto: "Solo si falla", ayuda: "Se nombra solo si la comparación falla (la pifia de Cthulhu)" },
    { valor: "ForzarAcierto", texto: "Acierta siempre", ayuda: "Acierta aunque el total no llegue (el 20 de D&D)" },
    { valor: "ForzarFallo", texto: "Falla siempre", ayuda: "Falla aunque el total llegue (el 1 de D&D)" }
  ];

  interface FilaTramo { nombre: string; numerador: number | null; denominador: number | null }
  interface FilaRegla { nombre: string; dado: number | null; desde: number | null; hasta: number | null; efecto: EfectoNatural; umbral: number | null }

  let usar = $state(false);
  let nombreExito = $state("Éxito");
  let nombreFallo = $state("Fallo");
  let tramos = $state<FilaTramo[]>([]);
  let reglas = $state<FilaRegla[]>([]);
  /** Hay algo escrito que aún no está guardado. */
  let tocado = $state(false);
  let guardando = $state(false);
  let aviso = $state("");

  function cargar(e: EscalaResultado | null) {
    usar = e !== null;
    nombreExito = e?.nombre_exito ?? "Éxito";
    nombreFallo = e?.nombre_fallo ?? "Fallo";
    tramos = (e?.tramos ?? []).map((t) => ({ nombre: t.nombre, numerador: t.numerador, denominador: t.denominador }));
    reglas = (e?.naturales ?? []).map((r) => ({ nombre: r.nombre, dado: r.dado, desde: r.desde, hasta: r.hasta, efecto: r.efecto, umbral: r.umbral_menor_que }));
    tocado = false;
  }

  // La de Rust (al abrir, al cambiar de conjunto, tras guardar): se toma salvo que haya algo escrito a medias
  $effect(() => {
    const e = partida.escala;
    if (!tocado) cargar(e);
  });
  $effect(() => {
    void partida.cargarEjemplosEscala();
  });

  const marcar = () => {
    tocado = true;
    aviso = "";
  };

  function aEscala(): EscalaResultado {
    const num = (v: number | null) => Number(v ?? 0);
    return {
      nombre_exito: nombreExito.trim(),
      nombre_fallo: nombreFallo.trim(),
      tramos: tramos.map((t) => ({ nombre: t.nombre.trim(), numerador: num(t.numerador), denominador: num(t.denominador) })),
      naturales: reglas.map((r) => ({
        nombre: r.nombre.trim(),
        dado: r.dado === null || (r.dado as unknown) === "" ? null : Number(r.dado),
        desde: num(r.desde),
        hasta: num(r.hasta),
        efecto: r.efecto,
        umbral_menor_que: r.umbral === null || (r.umbral as unknown) === "" ? null : Number(r.umbral)
      }))
    };
  }

  async function guardar() {
    if (guardando) return;
    guardando = true;
    aviso = "";
    try {
      await partida.guardarEscala(usar ? aEscala() : null);
      onError("");
      tocado = false;
      cargar(partida.escala);
      aviso = usar ? "Escala guardada." : "Sin escala: las comparaciones dan solo «Éxito» o «Fallo».";
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err)); // lo escrito se queda, para corregirlo
    } finally {
      guardando = false;
    }
  }

  function rellenarCon(clave: string) {
    const ejemplo = partida.ejemplosEscala.find((e) => e.clave === clave);
    if (!ejemplo) return;
    cargar(ejemplo.escala);
    usar = true;
    tocado = true;
    aviso = `Escala de «${ejemplo.nombre}» cargada: pulsa «Guardar» para usarla.`;
  }

  function descartar() {
    cargar(partida.escala);
    aviso = "";
    onError("");
  }
</script>

<div class="escala">
  <p class="ayuda">
    En el chat, <code>1d100 &lt;= 45</code> (por debajo), <code>1d20 &gt;= 12</code> (superar) y <code>vs</code> (igual que <code>&lt;=</code>) comparan
    una tirada con un objetivo. Aquí eliges cómo se llama el resultado.
  </p>

  <label class="check" title={ayuda("Sin escala, el resultado de una comparación es solo «Éxito» o «Fallo»")}>
    <input type="checkbox" bind:checked={usar} onchange={marcar} />
    <span>Nombrar el resultado de las comparaciones (niveles, críticos, pifias…)</span>
  </label>

  {#if partida.ejemplosEscala.length > 0}
    <div class="ejemplos">
      <span class="texto">Empezar desde un ejemplo:</span>
      {#each partida.ejemplosEscala as e (e.clave)}
        <button type="button" class="btn-ejemplo" title={ayuda(e.descripcion)} onclick={() => rellenarCon(e.clave)}>{e.nombre}</button>
      {/each}
    </div>
  {/if}

  {#if usar}
    <div class="fila">
      <label for="esc-exito">Acertar se llama:</label>
      <input id="esc-exito" type="text" class="campo" maxlength="30" bind:value={nombreExito} oninput={marcar} />
      <label for="esc-fallo">Fallar se llama:</label>
      <input id="esc-fallo" type="text" class="campo" maxlength="30" bind:value={nombreFallo} oninput={marcar} />
    </div>

    <h4 class="titulo" title={ayuda("Solo al tirar por debajo (<=): el acierto se llama así si el total no pasa de esa parte del objetivo. Del más estricto al más laxo")}>Niveles de acierto (con &lt;=)</h4>
    {#each tramos as t, i (i)}
      <div class="fila-tramo">
        <input type="text" class="campo" maxlength="30" placeholder="Éxito difícil" aria-label="Nombre del nivel" bind:value={t.nombre} oninput={marcar} />
        <span class="texto">hasta</span>
        <input type="number" class="campo numero" min="1" max="100" aria-label="Numerador" bind:value={t.numerador} oninput={marcar} />
        <span class="texto">/</span>
        <input type="number" class="campo numero" min="1" max="100" aria-label="Denominador" bind:value={t.denominador} oninput={marcar} />
        <span class="texto">del objetivo</span>
        <button type="button" class="btn-quitar" aria-label="Quitar el nivel" onclick={() => { tramos = tramos.filter((_, j) => j !== i); marcar(); }}>✕</button>
      </div>
    {/each}
    {#if tramos.length < 6}
      <button type="button" class="btn-mas" onclick={() => { tramos = [...tramos, { nombre: "", numerador: 1, denominador: 2 }]; marcar(); }}>+ Añadir nivel</button>
    {/if}

    <h4 class="titulo" title={ayuda("Se miran cuando la tirada es UN solo dado normal (sin ventaja ni multiplicador). La primera regla que casa gana")}>Críticos y pifias (cara del dado)</h4>
    {#if reglas.length > 0}
      <div class="cab-regla">
        <span>Nombre</span><span title={ayuda("Solo en un dado de estas caras (vacío = en cualquiera)")}>Dado</span><span>Desde</span><span>Hasta</span><span>Cuándo</span>
        <span title={ayuda("Solo si el objetivo es menor que esto (vacío = siempre)")}>Objetivo &lt;</span><span></span>
      </div>
    {/if}
    {#each reglas as r, i (i)}
      <div class="fila-regla">
        <input type="text" class="campo" maxlength="30" placeholder="Pifia" aria-label="Nombre de la regla" bind:value={r.nombre} oninput={marcar} />
        <input type="number" class="campo numero" min="2" max="1000" placeholder="d100" aria-label="Caras del dado" bind:value={r.dado} oninput={marcar} />
        <input type="number" class="campo numero" min="1" max="1000" aria-label="Desde la cara" bind:value={r.desde} oninput={marcar} />
        <input type="number" class="campo numero" min="1" max="1000" aria-label="Hasta la cara" bind:value={r.hasta} oninput={marcar} />
        <select class="campo" aria-label="Cuándo se aplica" title={ayuda(EFECTOS.find((x) => x.valor === r.efecto)?.ayuda)} bind:value={r.efecto} onchange={marcar}>
          {#each EFECTOS as x (x.valor)}<option value={x.valor}>{x.texto}</option>{/each}
        </select>
        <input type="number" class="campo numero" min="-9999" max="9999" placeholder="—" aria-label="Objetivo menor que" bind:value={r.umbral} oninput={marcar} />
        <button type="button" class="btn-quitar" aria-label="Quitar la regla" onclick={() => { reglas = reglas.filter((_, j) => j !== i); marcar(); }}>✕</button>
      </div>
    {/each}
    {#if reglas.length < 10}
      <button type="button" class="btn-mas" onclick={() => { reglas = [...reglas, { nombre: "", dado: null, desde: 1, hasta: 1, efecto: "SoloSiFalla", umbral: null }]; marcar(); }}>+ Añadir regla</button>
    {/if}
  {/if}

  <div class="pie">
    {#if aviso}<span class="aviso">{aviso}</span>{/if}
    {#if tocado}<button type="button" class="btn-cancelar" onclick={descartar}>Descartar</button>{/if}
    <button type="button" class="btn-guardar" disabled={guardando || !tocado} onclick={guardar}>Guardar</button>
  </div>
</div>

<style>
  .escala { display: flex; flex-direction: column; gap: 6px; }
  .ayuda { margin: 0; color: #71717a; font-size: 10px; line-height: 1.4; }
  .ayuda code { background: #18181b; padding: 0 3px; border-radius: 3px; color: #d4d4d8; }
  .check { display: flex; align-items: center; gap: 6px; color: #d4d4d8; font-size: 11px; cursor: pointer; }
  .check input { accent-color: #3b82f6; margin: 0; }
  .ejemplos { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .texto { color: #a1a1aa; font-size: 10px; }
  .btn-ejemplo {
    background: #18181b; border: 1px solid #3f3f46; color: #d4d4d8; border-radius: 4px; padding: 2px 8px; font-size: 10px; cursor: pointer;
  }
  .btn-ejemplo:hover { border-color: #71717a; color: white; }
  .fila { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; font-size: 11px; color: #a1a1aa; }
  .titulo { margin: 4px 0 0; color: #a1a1aa; font-size: 11px; font-weight: 700; }
  .campo {
    min-width: 0; padding: 3px 5px; border-radius: 4px; background: #09090b; border: 1px solid #3f3f46; color: white; font-size: 11px;
  }
  .campo.numero { width: 52px; text-align: center; padding: 3px 2px; }
  .fila-tramo { display: flex; align-items: center; gap: 5px; }
  .fila-tramo .campo:first-child { width: 150px; }
  .cab-regla, .fila-regla {
    display: grid; grid-template-columns: 1fr 54px 52px 52px 118px 62px 20px; gap: 4px; align-items: center;
  }
  .cab-regla { color: #71717a; font-size: 9px; }
  .btn-quitar { background: none; border: none; color: #f87171; cursor: pointer; font-size: 11px; padding: 2px; }
  .btn-quitar:hover { color: #ef4444; }
  .btn-mas { align-self: flex-start; background: none; border: 1px dashed #3f3f46; color: #a1a1aa; border-radius: 4px; font-size: 10px; padding: 2px 8px; cursor: pointer; }
  .btn-mas:hover { color: white; border-color: #71717a; }
  .pie { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 4px; }
  .aviso { flex: 1; color: #86efac; font-size: 10px; }
  .btn-guardar { background: #2563eb; border: none; color: white; border-radius: 4px; padding: 4px 12px; font-size: 11px; font-weight: 600; cursor: pointer; }
  .btn-guardar:hover:not(:disabled) { background: #1d4ed8; }
  .btn-guardar:disabled { opacity: 0.5; cursor: default; }
  .btn-cancelar { background: #27272a; border: none; color: #d4d4d8; border-radius: 4px; padding: 4px 10px; font-size: 11px; cursor: pointer; }
</style>
