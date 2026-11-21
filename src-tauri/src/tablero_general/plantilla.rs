// src-tauri/src/tablero_general/plantilla.rs
//
// La plantilla de ficha: un HTML autocontenido que la app (o el navegador de un jugador) genera con los datos dentro, que se
// rellena FUERA de la app con cualquier navegador y que, guardado, vuelve a arrastrarse a la app. Todo lo que es la plantilla
// (generarla, leerla, aplicarla a una ficha) vive en el front (`src/lib/plantillaFicha.ts`); aquí solo está lo que el host
// necesita del disco: escribir el archivo exportado, leer el que se suelta y meter en `imagenes/` la foto que trae dentro.
// Solo del host: un jugador hace lo mismo en su navegador (descarga, `File.text()` y la subida de imágenes de siempre).
use std::path::{Path, PathBuf};

use base64::Engine;
use tauri::State;

use crate::tablero_general::imagenes;
use crate::tablero_general::partida::AppState;

/// Tope del archivo que se exporta (la foto va entera dentro, en base64).
pub const MAX_BYTES_EXPORTAR: usize = 24 * 1024 * 1024;
/// Tope del archivo que se lee (comprobado ANTES de leerlo: es un archivo de fuera).
pub const MAX_BYTES_LEER: u64 = 16 * 1024 * 1024;

/// La ruta donde se escribe la plantilla: absoluta y con extensión `.html` (se le pone si no la tiene).
fn destino_html(ruta: &str) -> Result<PathBuf, String> {
    let destino = Path::new(ruta);
    if !destino.is_absolute() {
        return Err("La ruta de destino no es válida".into());
    }
    let html = destino.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
    Ok(if html { destino.to_path_buf() } else { destino.with_extension("html") })
}

/// Escribe la plantilla generada en `ruta` (atómico: tmp + rename).
pub fn guardar_plantilla_en(ruta: &str, contenido: &str) -> Result<(), String> {
    if contenido.len() > MAX_BYTES_EXPORTAR {
        return Err(format!("La plantilla no puede pasar de {} MB", MAX_BYTES_EXPORTAR / (1024 * 1024)));
    }
    let destino = destino_html(ruta)?;
    let tmp = destino.with_extension("html.tmp");
    std::fs::write(&tmp, contenido).map_err(|e| format!("No se pudo escribir el archivo: {e}"))?;
    std::fs::rename(&tmp, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("No se pudo escribir el archivo: {e}")
    })
}

/// Lee una plantilla soltada sobre la app: solo `.html`/`.htm` y con el tamaño comprobado antes de leerla. Devuelve el texto tal
/// cual: lo interpreta (como entrada NO fiable) `leerPlantilla` en el front, y lo que se guarde pasa luego por `validar_entidad`.
pub fn leer_plantilla_en(ruta: &str) -> Result<String, String> {
    let ruta = Path::new(ruta);
    if !ruta.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")) {
        return Err("No es una plantilla de ficha (.html)".into());
    }
    let tam = std::fs::metadata(ruta).map_err(|e| format!("No se pudo leer el archivo: {e}"))?.len();
    if tam > MAX_BYTES_LEER {
        return Err(format!("El archivo no puede pasar de {} MB", MAX_BYTES_LEER / (1024 * 1024)));
    }
    std::fs::read_to_string(ruta).map_err(|e| format!("No se pudo leer el archivo: {e}"))
}

/// La foto que viene dentro de una plantilla (base64) a `imagenes/`, con las MISMAS comprobaciones que una imagen que sube un
/// jugador (`imagenes::importar_bytes`: formatos rasterizados, contenido de verdad, topes de tamaño). Devuelve la ruta relativa.
pub fn importar_imagen_base64_en(dir_datos: &Path, datos: &str, extension: &str) -> Result<String, String> {
    // Antes de decodificar: el base64 de 8 MiB son ~11 MB de texto
    if datos.len() > imagenes::MAX_BYTES_SUBIDA / 3 * 4 + 8 {
        return Err(format!("La imagen no puede pasar de {} MB", imagenes::MAX_BYTES_SUBIDA / (1024 * 1024)));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(datos.trim())
        .map_err(|_| "La foto de la plantilla está dañada".to_string())?;
    imagenes::importar_bytes(dir_datos, &bytes, extension)
}

#[tauri::command]
pub fn guardar_plantilla(ruta: String, contenido: String) -> Result<(), String> {
    guardar_plantilla_en(&ruta, &contenido)
}

#[tauri::command]
pub fn leer_plantilla(ruta: String) -> Result<String, String> {
    leer_plantilla_en(&ruta)
}

#[tauri::command]
pub fn importar_imagen_base64(datos: String, extension: String, state: State<'_, AppState>) -> Result<String, String> {
    importar_imagen_base64_en(&state.almacen.dir(), &datos, &extension)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tablero_general::almacen::pruebas::dir_temporal;

    #[test]
    fn la_plantilla_se_guarda_como_html_y_solo_en_una_ruta_absoluta() {
        let dir = dir_temporal("plantilla_guardar");
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("Orco - plantilla");
        guardar_plantilla_en(ruta.to_str().unwrap(), "<!doctype html>hola").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("Orco - plantilla.html")).unwrap(), "<!doctype html>hola", "se le pone .html");
        assert!(guardar_plantilla_en("relativa.html", "x").is_err());
        let enorme = "x".repeat(MAX_BYTES_EXPORTAR + 1);
        assert!(guardar_plantilla_en(dir.join("grande.html").to_str().unwrap(), &enorme).is_err());
    }

    #[test]
    fn solo_se_lee_un_html_y_no_demasiado_grande() {
        let dir = dir_temporal("plantilla_leer");
        std::fs::create_dir_all(&dir).unwrap();
        let html = dir.join("ficha.html");
        std::fs::write(&html, "<html>datos</html>").unwrap();
        assert_eq!(leer_plantilla_en(html.to_str().unwrap()).unwrap(), "<html>datos</html>");
        let json = dir.join("ficha.json");
        std::fs::write(&json, "{}").unwrap();
        assert!(leer_plantilla_en(json.to_str().unwrap()).is_err(), "otra extensión");
        let grande = dir.join("grande.htm");
        std::fs::write(&grande, vec![b'x'; (MAX_BYTES_LEER + 1) as usize]).unwrap();
        assert!(leer_plantilla_en(grande.to_str().unwrap()).unwrap_err().contains("MB"));
        assert!(leer_plantilla_en(dir.join("no_existe.html").to_str().unwrap()).is_err());
    }

    #[test]
    fn la_foto_de_la_plantilla_se_comprueba_como_una_subida() {
        let dir = dir_temporal("plantilla_foto");
        std::fs::create_dir_all(&dir).unwrap();
        // Un PNG de 1×1 de verdad
        let mut png = Vec::new();
        image::RgbaImage::new(1, 1).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let ruta = importar_imagen_base64_en(&dir, &b64, "png").unwrap();
        assert!(ruta.starts_with("imagenes/") && ruta.ends_with(".png"), "{ruta}");
        assert!(importar_imagen_base64_en(&dir, "esto no es base64!!", "png").is_err());
        assert!(importar_imagen_base64_en(&dir, &b64, "svg").is_err(), "nada de SVG");
        let texto = base64::engine::general_purpose::STANDARD.encode(b"no soy una imagen");
        assert!(importar_imagen_base64_en(&dir, &texto, "png").is_err(), "el contenido tiene que ser una imagen");
    }
}
