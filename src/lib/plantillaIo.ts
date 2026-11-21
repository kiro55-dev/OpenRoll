// src/lib/plantillaIo.ts
//
// La plantilla de ficha (ver `plantillaFicha.ts`) contra el disco y la red: exportarla (el host la guarda donde elija; un jugador la
// descarga), leer la que se suelta sobre la app (el host recibe una ruta; un jugador, un archivo de su navegador) y meter en la
// partida la foto que trae (el host con `importar_imagen_base64`; un jugador la sube como cualquier foto).
import { save } from "@tauri-apps/plugin-dialog";
import { invoke } from "$lib/backend";
import { esJugador } from "$lib/modo";
import { importarImagen, urlDeImagen } from "$lib/imagenes";
import { partida, type Entidad } from "$lib/partida.svelte";
import { encuadreCentrado } from "$lib/recorteFoto";
import { MIME_FOTO, datosDePlantilla, generarPlantilla, leerPlantilla, type ArchivoPlantilla, type FotoPlantilla, type PlantillaLeida } from "$lib/plantillaFicha";

/** El nombre del conjunto de parámetros en uso (un jugador no lo sabe: los conjuntos son del host). */
export function conjuntoActual(): string {
  if (esJugador) return "";
  return partida.conjuntos.guardados.find((c) => c.id === partida.conjuntos.activo)?.nombre ?? "";
}

const MIME_POR_EXTENSION: Record<string, string> = {
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp", gif: "image/gif", bmp: "image/bmp",
  svg: "image/svg+xml", avif: "image/avif"
};

function base64DeBlob(blob: Blob): Promise<string> {
  return new Promise((ok, mal) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).slice(String(r.result).indexOf(",") + 1));
    r.onerror = () => mal(r.error);
    r.readAsDataURL(blob);
  });
}

/** La foto de la ficha, ENTERA, para meterla en la plantilla (con su encuadre). Si no se puede leer, la plantilla sale sin foto. */
async function fotoDe(entidad: Entidad): Promise<FotoPlantilla | null> {
  if (!entidad.foto_url) return null;
  try {
    const respuesta = await fetch(urlDeImagen(entidad.foto_url));
    if (!respuesta.ok) return null;
    const blob = await respuesta.blob();
    const ext = /\.([a-z0-9]+)$/i.exec(entidad.foto_url)?.[1].toLowerCase() ?? "";
    const mime = blob.type && blob.type.startsWith("image/") ? blob.type : (MIME_POR_EXTENSION[ext] ?? "image/png");
    return { mime, datos: await base64DeBlob(blob), encuadre: { ...(entidad.foto_encuadre ?? encuadreCentrado()) } };
  } catch {
    return null;
  }
}

const nombreDeArchivo = (nombre: string) => nombre.replace(/[\\/:*?"<>|]+/g, "_").trim() || "ficha";

/**
 * Exporta la plantilla: en blanco (`entidad` null, con el conjunto en uso) o con los datos de una ficha (lo que tiene ahora en el
 * editor, sin guardar incluido) y su foto. `idFicha`: la ficha de la que sale (null si es nueva sin guardar). `true` si se guardó.
 */
export async function exportarPlantilla(entidad: Entidad | null, idFicha: number | null = null, archivos: ArchivoPlantilla[] = []): Promise<boolean> {
  const datos = datosDePlantilla({
    definiciones: partida.definiciones,
    grupos: partida.gruposVariables,
    variables: partida.definicionesVariables,
    conjunto: conjuntoActual(),
    entidad,
    idFicha,
    foto: entidad ? await fotoDe(entidad) : null,
    // En blanco, la plantilla trae los .txt con los que nace toda ficha del conjunto; de una ficha, los suyos
    archivos: entidad ? archivos : partida.archivosTextoDefecto.map((a) => ({ original: null, nombre: `${a.nombre}.txt`, contenido: a.contenido }))
  });
  const html = generarPlantilla(datos);
  const nombre = `${nombreDeArchivo(entidad ? entidad.nombre : datos.conjunto || "Ficha")}.html`;
  if (esJugador) {
    // En el navegador: se descarga
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([html], { type: "text/html" }));
    a.download = nombre;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
    return true;
  }
  const ruta = await save({ defaultPath: nombre, filters: [{ name: "Plantilla de ficha", extensions: ["html"] }] });
  if (typeof ruta !== "string") return false;
  await invoke("guardar_plantilla", { ruta, contenido: html });
  return true;
}

/** Lee una plantilla soltada sobre la app: una ruta (host) o un archivo del navegador (jugador). Lanza el motivo si no vale. */
export async function leerArchivoPlantilla(origen: string | File): Promise<PlantillaLeida> {
  const html = typeof origen === "string" ? await invoke<string>("leer_plantilla", { ruta: origen }) : await origen.text();
  return leerPlantilla(html);
}

/** Mete en la partida la foto que trae la plantilla (con las mismas comprobaciones que cualquier foto). Devuelve su ruta. */
export async function importarFotoPlantilla(foto: FotoPlantilla): Promise<string> {
  const ext = MIME_FOTO[foto.mime];
  if (!ext) throw new Error("La foto de la plantilla no está en un formato que admita OpenRoll (png, jpg, webp, gif o bmp)");
  if (!esJugador) return invoke<string>("importar_imagen_base64", { datos: foto.datos, extension: ext });
  const bytes = Uint8Array.from(atob(foto.datos), (c) => c.charCodeAt(0));
  return importarImagen(new File([bytes], `foto.${ext}`, { type: foto.mime }), "ficha");
}
