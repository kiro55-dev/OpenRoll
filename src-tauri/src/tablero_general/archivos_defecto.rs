// src-tauri/src/tablero_general/archivos_defecto.rs
//
// Los ARCHIVOS DE TEXTO POR DEFECTO de un conjunto de parámetros: los .txt con los que nace toda ficha NUEVA (p. ej. «Historia»,
// «Notas», con un texto inicial opcional). Son parte del conjunto (`ConjuntoParametros.archivos_texto`, copia viva
// `EstadoPartida.archivos_texto_defecto`), así que se intercambian al cambiar de conjunto y viajan en el archivo exportado. La ficha
// los recibe al iniciar su borrador (`partida::iniciar_borrador_entidad_en` los crea en su carpeta); las fichas que ya existen no
// se tocan, y la plantilla HTML en blanco los lleva dentro.
//
// Este módulo es puro (sin Tauri) salvo los comandos del final, que son solo del host (nada en `PETICIONES`).
use tauri::State;

use crate::tablero_general::archivos::{nombre_valido, MAX_ARCHIVOS_TEXTO, MAX_CARACTERES_TEXTO};
use crate::tablero_general::estado::DeltaPartida;
use crate::tablero_general::jugadores::caracter_enganoso;
use crate::tablero_general::modelos::ArchivoTextoDefecto;
use crate::tablero_general::partida::{registrar, AppState};

pub const MAX_NOMBRE_ARCHIVO: usize = 100;
/// Lo que no puede llevar el nombre (sin la extensión, que es siempre .txt): nada de puntos ni de caracteres de ruta.
const PROHIBIDOS: [char; 10] = ['.', '\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Comprueba la lista entera con las reglas de siempre y la devuelve limpia, con ids únicos (los de id 0 reciben el siguiente libre).
pub fn validar(lista: Vec<ArchivoTextoDefecto>) -> Result<Vec<ArchivoTextoDefecto>, String> {
    if lista.len() > MAX_ARCHIVOS_TEXTO {
        return Err(format!("No puede haber más de {MAX_ARCHIVOS_TEXTO} archivos de texto por defecto"));
    }
    let mut limpia: Vec<ArchivoTextoDefecto> = Vec::with_capacity(lista.len());
    for a in lista {
        let nombre = a.nombre.trim().to_string();
        if nombre.is_empty() {
            return Err("Todos los archivos de texto necesitan un nombre".into());
        }
        if nombre.chars().count() > MAX_NOMBRE_ARCHIVO {
            return Err(format!("El nombre «{nombre}» pasa de {MAX_NOMBRE_ARCHIVO} caracteres"));
        }
        if nombre.chars().any(|c| caracter_enganoso(c) || PROHIBIDOS.contains(&c)) {
            return Err(format!("El nombre «{nombre}» tiene caracteres que no se pueden usar (ni puntos: la extensión es siempre .txt)"));
        }
        nombre_valido(&format!("{nombre}.txt"))?;
        if limpia.iter().any(|o| o.nombre.to_lowercase() == nombre.to_lowercase()) {
            return Err(format!("Hay dos archivos llamados «{nombre}»"));
        }
        if a.contenido.chars().count() > MAX_CARACTERES_TEXTO {
            return Err(format!("El texto de «{nombre}» pasa de {MAX_CARACTERES_TEXTO} caracteres"));
        }
        if a.id != 0 && limpia.iter().any(|o| o.id == a.id) {
            return Err(format!("El archivo «{nombre}» repite el id {}", a.id));
        }
        limpia.push(ArchivoTextoDefecto { id: a.id, nombre, contenido: a.contenido });
    }
    let mut siguiente = limpia.iter().map(|a| a.id).max().unwrap_or(0);
    for a in limpia.iter_mut().filter(|a| a.id == 0) {
        siguiente += 1;
        a.id = siguiente;
    }
    Ok(limpia)
}

#[tauri::command]
pub fn obtener_archivos_texto_defecto(state: State<'_, AppState>) -> Vec<ArchivoTextoDefecto> {
    state.lock().archivos_texto_defecto.clone()
}

/// Guarda la lista del conjunto activo (entera). Se valida entera; sin cambios no se registra nada. Devuelve la lista guardada, con
/// los ids que Rust ha dado a los archivos nuevos.
#[tauri::command]
pub fn guardar_archivos_texto_defecto(archivos: Vec<ArchivoTextoDefecto>, state: State<'_, AppState>) -> Result<Vec<ArchivoTextoDefecto>, String> {
    guardar_archivos_texto_defecto_en(archivos, &state)
}

pub fn guardar_archivos_texto_defecto_en(archivos: Vec<ArchivoTextoDefecto>, state: &AppState) -> Result<Vec<ArchivoTextoDefecto>, String> {
    let lista = validar(archivos)?;
    let mut est = state.lock();
    if est.archivos_texto_defecto != lista {
        registrar(&mut est, &state.almacen, DeltaPartida::ArchivosTextoDefectoCambiados(lista.clone()));
    }
    Ok(lista)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(id: u16, nombre: &str, contenido: &str) -> ArchivoTextoDefecto {
        ArchivoTextoDefecto { id, nombre: nombre.into(), contenido: contenido.into() }
    }

    #[test]
    fn asigna_ids_a_los_nuevos_sin_tocar_los_que_ya_tienen() {
        let l = validar(vec![a(0, "Historia", ""), a(5, "Notas", "x"), a(0, "Aliados", "")]).unwrap();
        assert_eq!(l.iter().map(|x| x.id).collect::<Vec<_>>(), vec![6, 5, 7]);
        assert_eq!(l[1].contenido, "x");
    }

    #[test]
    fn rechaza_nombres_malos() {
        for malo in ["", "   ", "a.b", "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "con", "a\u{202e}b", "a\nb"] {
            assert!(validar(vec![a(0, malo, "")]).is_err(), "{malo:?}");
        }
        assert!(validar(vec![a(0, &"x".repeat(MAX_NOMBRE_ARCHIVO + 1), "")]).is_err());
        assert!(validar(vec![a(0, &"x".repeat(MAX_NOMBRE_ARCHIVO), "")]).is_ok());
    }

    #[test]
    fn nombres_e_ids_no_se_repiten() {
        assert!(validar(vec![a(0, "Notas", ""), a(0, "notas", "")]).is_err(), "sin mirar mayúsculas");
        assert!(validar(vec![a(3, "A", ""), a(3, "B", "")]).is_err());
        assert_eq!(validar(vec![a(0, "  Notas  ", "")]).unwrap()[0].nombre, "Notas");
    }

    #[test]
    fn respeta_los_topes_de_archivos_y_de_texto() {
        let veinte: Vec<_> = (0..MAX_ARCHIVOS_TEXTO).map(|i| a(0, &format!("A{i}"), "")).collect();
        assert!(validar(veinte.clone()).is_ok());
        let mut veintiuno = veinte;
        veintiuno.push(a(0, "Uno más", ""));
        assert!(validar(veintiuno).is_err());
        assert!(validar(vec![a(0, "Largo", &"é".repeat(MAX_CARACTERES_TEXTO))]).is_ok(), "se cuentan caracteres, no bytes");
        assert!(validar(vec![a(0, "Largo", &"é".repeat(MAX_CARACTERES_TEXTO + 1))]).is_err());
    }
}
