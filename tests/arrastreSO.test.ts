// Resaltado de las zonas donde se puede soltar un archivo del sistema (src/lib/arrastreSO.ts): clasificación por extensión (app del host)
// y por MIME (navegador de un jugador), cada criterio de zona y cuál es la zona «encima» entre zonas anidadas. Lo que toca el DOM es una capa fina.
import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/modo", () => ({ esJugador: false }));
vi.mock("$lib/red/conexion.svelte", () => ({ conexion: null }));

import {
  deMime,
  deRuta,
  desconocido,
  esCualquierArchivo,
  esImagenArrastrada,
  esPlantillaArrastrada,
  esTextoArrastrado,
  zonaEncima,
  zonaValida,
  type ArchivoArrastrado,
  type ZonaSoltar
} from "$lib/arrastreSO";

describe("clasificación del archivo que se arrastra", () => {
  it("por ruta (app del host): extensión en minúsculas; sin extensión o una carpeta no tiene", () => {
    expect(deRuta("C:\\Users\\Ana\\Fotos\\Mapa.PNG")).toEqual({ ext: "png", mime: null });
    expect(deRuta("/home/ana/ficha.final.html")).toEqual({ ext: "html", mime: null });
    expect(deRuta("C:\\Users\\Ana\\Documentos")).toEqual({ ext: null, mime: null });
    expect(deRuta("C:\\Users\\Ana\\Documentos\\")).toEqual({ ext: null, mime: null });
  });

  it("por MIME (navegador): en minúsculas; vacío = no se sabe", () => {
    expect(deMime(" IMAGE/PNG ")).toEqual({ ext: null, mime: "image/png" });
    expect(deMime("")).toEqual({ ext: null, mime: null });
    expect(desconocido(deMime(""))).toBe(true);
    expect(desconocido(deRuta("C:\\carpeta"))).toBe(true);
    expect(desconocido(deRuta("a.txt"))).toBe(false);
    expect(desconocido(deMime("text/plain"))).toBe(false);
  });

  it("imágenes: una ficha admite SVG y AVIF, un mapa no; por extensión y por MIME", () => {
    for (const ext of ["png", "jpg", "jpeg", "webp", "gif", "bmp"]) {
      expect(esImagenArrastrada(deRuta(`a.${ext}`), "mapa")).toBe(true);
      expect(esImagenArrastrada(deRuta(`a.${ext}`), "ficha")).toBe(true);
    }
    expect(esImagenArrastrada(deRuta("a.svg"), "ficha")).toBe(true);
    expect(esImagenArrastrada(deRuta("a.svg"), "mapa")).toBe(false);
    expect(esImagenArrastrada(deRuta("a.avif"), "mapa")).toBe(false);
    expect(esImagenArrastrada(deRuta("a.pdf"), "ficha")).toBe(false);
    expect(esImagenArrastrada(deMime("image/jpeg"), "mapa")).toBe(true);
    expect(esImagenArrastrada(deMime("image/svg+xml"), "mapa")).toBe(false);
    expect(esImagenArrastrada(deMime("image/svg+xml"), "ficha")).toBe(true);
    expect(esImagenArrastrada(deMime("image/avif"), "mapa")).toBe(false);
    expect(esImagenArrastrada(deMime("text/html"), "ficha")).toBe(false);
    expect(esImagenArrastrada(deMime(""), "ficha")).toBe(false);
  });

  it("plantillas (.html/.htm), textos (.txt) y cualquier archivo", () => {
    expect(esPlantillaArrastrada(deRuta("ficha.HTML"))).toBe(true);
    expect(esPlantillaArrastrada(deRuta("ficha.htm"))).toBe(true);
    expect(esPlantillaArrastrada(deRuta("ficha.html.txt"))).toBe(false);
    expect(esPlantillaArrastrada(deMime("text/html"))).toBe(true);
    expect(esPlantillaArrastrada(deMime("text/plain"))).toBe(false);
    expect(esTextoArrastrado(deRuta("notas.txt"))).toBe(true);
    expect(esTextoArrastrado(deMime("text/plain"))).toBe(true);
    expect(esTextoArrastrado(deRuta("notas.md"))).toBe(false);
    expect(esCualquierArchivo(deRuta("a.pdf"))).toBe(true);
    expect(esCualquierArchivo(deMime("application/pdf"))).toBe(true);
    expect(esCualquierArchivo(deRuta("C:\\carpeta"))).toBe(false);
  });
});

// Zonas de prueba (los «nodos» son rutas de un árbol: «a/b» está dentro de «a»)
const zona = (nodo: string, acepta: ZonaSoltar<string>["acepta"]): ZonaSoltar<string> => ({ nodo, acepta });
const contiene = (zona: string, nodo: string) => nodo === zona || nodo.startsWith(zona + "/");
const html: ArchivoArrastrado[] = [deRuta("ficha.html")];
const png: ArchivoArrastrado[] = [deRuta("mapa.png")];

describe("zonas válidas", () => {
  const banco = zona("banco", (a) => a.some((x) => esPlantillaArrastrada(x) || esImagenArrastrada(x, "ficha")));
  const foto = zona("ventana/foto", (a) => a.some((x) => esImagenArrastrada(x, "ficha")));
  const ventana = zona("ventana", (a) => a.some(esPlantillaArrastrada));
  const mapas = zona("mapas", (a) => a.some((x) => esImagenArrastrada(x, "mapa")));

  it("un .html marca el banco y la ventana, no la foto ni los mapas; una imagen, el banco, la foto y los mapas", () => {
    const todas = [banco, foto, ventana, mapas];
    expect(todas.filter((z) => zonaValida(z, html)).map((z) => z.nodo)).toEqual(["banco", "ventana"]);
    expect(todas.filter((z) => zonaValida(z, png)).map((z) => z.nodo)).toEqual(["banco", "ventana/foto", "mapas"]);
    expect(todas.filter((z) => zonaValida(z, [deRuta("datos.pdf")])).map((z) => z.nodo)).toEqual([]);
  });

  it("sin archivos no hay zonas válidas, y si algún archivo es desconocido puede valer en todas", () => {
    expect(zonaValida(banco, [])).toBe(false);
    expect(zonaValida(mapas, [deMime("")])).toBe(true);
    expect(zonaValida(mapas, [deRuta("a.pdf"), deMime("")])).toBe(true);
  });

  it("con varios archivos vale si al menos uno le sirve", () => {
    expect(zonaValida(mapas, [deRuta("a.pdf"), deRuta("b.png")])).toBe(true);
    expect(zonaValida(mapas, [deRuta("a.pdf"), deRuta("b.html")])).toBe(false);
  });

  it("la zona «encima» es la válida más interna bajo el cursor", () => {
    const zonas = [banco, foto, ventana, mapas];
    // Una imagen sobre la foto: la foto (dentro de la ventana, que con una imagen no es válida)
    expect(zonaEncima(zonas, png, "ventana/foto/x", contiene)?.nodo).toBe("ventana/foto");
    // Un .html sobre la foto: la foto no vale, pero la ventana que la contiene sí
    expect(zonaEncima(zonas, html, "ventana/foto/x", contiene)?.nodo).toBe("ventana");
    // Fuera de toda zona, o sin nodo, ninguna
    expect(zonaEncima(zonas, png, "tablero", contiene)).toBeNull();
    expect(zonaEncima(zonas, png, null, contiene)).toBeNull();
    // Una zona no válida para el archivo no es «encima» aunque el cursor esté dentro
    expect(zonaEncima(zonas, [deRuta("a.pdf")], "banco/ficha", contiene)).toBeNull();
  });

  it("una tarjeta de mapa dentro de la lista: la tarjeta es la «encima» y la lista, la válida", () => {
    const lista = zona("mapas", (a) => a.some((x) => esImagenArrastrada(x, "mapa")));
    const tarjeta = zona("mapas/tarjeta1", (a) => a.some((x) => esImagenArrastrada(x, "mapa")));
    expect(zonaEncima([lista, tarjeta], png, "mapas/tarjeta1/img", contiene)?.nodo).toBe("mapas/tarjeta1");
    expect(zonaEncima([tarjeta, lista], png, "mapas/tarjeta1/img", contiene)?.nodo).toBe("mapas/tarjeta1");
    expect(zonaEncima([lista, tarjeta], png, "mapas/hueco", contiene)?.nodo).toBe("mapas");
  });
});
