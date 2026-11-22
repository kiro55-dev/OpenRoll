// src/lib/fusionFicha.ts
//
// La ventana de una ficha trabaja sobre una COPIA. Si mientras está abierta alguien cambia la ficha desde fuera (la vida con la rueda,
// el jugador dueño que edita su ficha, un arrastre de tirada...), la ventana no debe pisar ese cambio al guardar. Aquí se hace la fusión
// a tres bandas: lo que la ventana NO ha tocado desde que lo vio (`original`) se toma de fuera (`externa`, lo que hay ahora en Rust); lo
// que sí ha tocado se queda como lo dejó el usuario. Las listas que van una por definición (parámetros, variables, valores actuales) se
// fusionan elemento a elemento, así que tocar la Fuerza no pisa un cambio de fuera en la Destreza.
//
// Función pura (sin estado ni Svelte), con pruebas en tests/fusionFicha.test.ts.
import type { Entidad } from "./partida.svelte";

/** Listas con un elemento por definición (`id_definicion`): se fusionan elemento a elemento. */
const POR_DEFINICION = new Set(["parametros", "variables", "actuales"]);
/** Lo que solo cambia Rust (la miniatura propia, el paso de − / +): siempre lo de fuera. */
const DE_RUST = new Set(["miniatura_url", "pasos_valor"]);

/** ¿Son iguales dos valores (objetos y listas incluidos), sin importar el orden de las claves de un objeto? */
export function iguales(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  // `undefined` y `null` cuentan igual: un campo que falta en una copia no es un cambio
  if (a == null || b == null) return a == null && b == null;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    const lb = b as unknown[];
    return a.length === lb.length && a.every((x, i) => iguales(x, lb[i]));
  }
  const oa = a as Record<string, unknown>;
  const ob = b as Record<string, unknown>;
  const claves = new Set([...Object.keys(oa), ...Object.keys(ob)]);
  for (const k of claves) if (!iguales(oa[k], ob[k])) return false;
  return true;
}

type ConDefinicion = { id_definicion: number };

function fusionarLista<T extends ConDefinicion>(original: T[], editada: T[], externa: T[]): T[] {
  const de = (lista: T[], id: number) => lista.find((v) => v.id_definicion === id);
  // El orden es el de fuera (el de las definiciones); un elemento tocado en la ventana se queda como lo dejó el usuario
  const resultado = externa.map((x) => {
    const e = de(editada, x.id_definicion);
    return e && !iguales(e, de(original, x.id_definicion)) ? e : x;
  });
  // Lo que la ventana tocó y fuera ya no tiene (no debería pasar: las definiciones son las mismas) se conserva; Rust lo normaliza
  for (const e of editada) {
    if (!de(externa, e.id_definicion) && !iguales(e, de(original, e.id_definicion))) resultado.push(e);
  }
  return resultado;
}

/**
 * Fusión a tres bandas de una ficha: `original` es como estaba cuando la ventana la vio por última vez, `editada` como la tiene la
 * ventana ahora y `externa` como está ahora fuera. Lo que la ventana no tocó, de fuera; lo que tocó, de la ventana.
 */
export function fusionarFicha(original: Entidad, editada: Entidad, externa: Entidad): Entidad {
  const o = original as unknown as Record<string, unknown>;
  const e = editada as unknown as Record<string, unknown>;
  const x = externa as unknown as Record<string, unknown>;
  const resultado: Record<string, unknown> = { ...x };
  for (const k of new Set([...Object.keys(o), ...Object.keys(e), ...Object.keys(x)])) {
    if (DE_RUST.has(k)) continue;
    if (POR_DEFINICION.has(k)) {
      resultado[k] = fusionarLista((o[k] ?? []) as ConDefinicion[], (e[k] ?? []) as ConDefinicion[], (x[k] ?? []) as ConDefinicion[]);
      continue;
    }
    resultado[k] = iguales(e[k], o[k]) ? x[k] : e[k];
  }
  return resultado as unknown as Entidad;
}

/** Los valores actuales (de una ficha única) que la ventana ha cambiado desde `original`: los únicos que se mandan al guardar. */
export function actualesTocados(original: Entidad, editada: Entidad): NonNullable<Entidad["actuales"]> {
  return (editada.actuales ?? []).filter(
    (a) => !iguales(a, (original.actuales ?? []).find((o) => o.id_definicion === a.id_definicion))
  );
}
