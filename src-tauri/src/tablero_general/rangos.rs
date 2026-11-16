// src-tauri/src/tablero_general/rangos.rs
//
// LISTAS DE RANGOS: «Competencia» = Sin entrenar +0, Entrenado +2, Experto +4, Maestro +6, Legendario +8. Se definen UNA vez (en el conjunto de
// parámetros) y las variables derivadas la usan por su id (`DefinicionVariable.rangos`): cada ficha elige un rango por variable y su bono se
// suma al resultado. Cambiar la lista cambia todas las variables que la usan.
//
// Para que la ficha, el chat y los jugadores no tengan que buscar la lista, cada variable lleva una COPIA de las opciones de la suya
// (`DefinicionVariable.opciones`) que `normalizar_rangos` deja al día tras cada cambio. Nunca se escribe a mano.
//
// Este módulo es puro salvo los comandos del final, que son solo del host (nada en `PETICIONES`).
use tauri::State;

use crate::tablero_general::estado::DeltaPartida;
use crate::tablero_general::jugadores::caracter_enganoso;
use crate::tablero_general::modelos::{DefinicionVariable, ListaRangos, OpcionVariable};
use crate::tablero_general::partida::{registrar, AppState};
use crate::tablero_general::variables::{self, LIMITE_VALOR_VARIABLE, MAX_NOMBRE_OPCION, MAX_OPCIONES_VARIABLE};

/// Listas que puede tener un conjunto y largo de su nombre.
pub const MAX_LISTAS_RANGOS: usize = 10;
pub const MAX_NOMBRE_LISTA: usize = 30;

fn limpio(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn nombre_valido(n: &str, largo: usize, que: &str) -> Result<(), String> {
    if n.is_empty() || n.chars().count() > largo || n.chars().any(|c| c.is_control() || caracter_enganoso(c)) {
        return Err(format!("{que}: el nombre debe tener entre 1 y {largo} caracteres, sin símbolos raros"));
    }
    Ok(())
}

/// Comprueba una lista contra las DEMÁS (al crear, todas; al cambiarla, todas menos ella) y la devuelve con los nombres limpios. Entrada no
/// confiable (puede venir de un archivo importado).
pub fn validar_lista(otras: &[ListaRangos], l: ListaRangos) -> Result<ListaRangos, String> {
    let nombre = limpio(&l.nombre);
    nombre_valido(&nombre, MAX_NOMBRE_LISTA, "La lista de rangos")?;
    if otras.iter().any(|o| o.nombre.to_lowercase() == nombre.to_lowercase()) {
        return Err(format!("Ya hay una lista de rangos que se llama «{nombre}»"));
    }
    if otras.len() >= MAX_LISTAS_RANGOS {
        return Err(format!("No puede haber más de {MAX_LISTAS_RANGOS} listas de rangos"));
    }
    if l.opciones.is_empty() || l.opciones.len() > MAX_OPCIONES_VARIABLE {
        return Err(format!("«{nombre}»: una lista tiene de 1 a {MAX_OPCIONES_VARIABLE} rangos"));
    }
    let mut opciones: Vec<OpcionVariable> = Vec::new();
    for o in l.opciones {
        let n = limpio(&o.nombre);
        nombre_valido(&n, MAX_NOMBRE_OPCION, &format!("«{nombre}», un rango"))?;
        if opciones.iter().any(|p| p.nombre.to_lowercase() == n.to_lowercase()) {
            return Err(format!("«{nombre}»: el rango «{n}» está repetido"));
        }
        if o.bonus.abs() > LIMITE_VALOR_VARIABLE {
            return Err(format!("«{nombre}»: el bono de un rango debe estar entre -{LIMITE_VALOR_VARIABLE} y {LIMITE_VALOR_VARIABLE}"));
        }
        opciones.push(OpcionVariable { nombre: n, bonus: o.bonus });
    }
    Ok(ListaRangos { id: l.id, nombre, opciones })
}

/// Deja las variables acordes a las listas: una base no lleva rangos; una derivada con lista recibe una copia de sus opciones; si su lista ya
/// no existe se queda sin rangos; y una derivada con opciones propias y sin lista (una partida o un archivo de antes de las listas) pasa a una
/// lista (la que ya tenga esas mismas opciones o una nueva «Rangos N»), de modo que no se pierde nada.
pub fn normalizar_rangos(listas: &mut Vec<ListaRangos>, defs: &mut [DefinicionVariable]) {
    for d in defs.iter_mut() {
        if !variables::es_derivada(d) {
            d.rangos = None;
            d.opciones.clear();
            continue;
        }
        if d.rangos.is_none() && !d.opciones.is_empty() {
            d.rangos = lista_para(listas, std::mem::take(&mut d.opciones));
        }
        match d.rangos.and_then(|id| listas.iter().find(|l| l.id == id)) {
            Some(l) => d.opciones = l.opciones.clone(),
            None => {
                d.rangos = None;
                d.opciones.clear();
            }
        }
    }
}

/// La lista que tiene exactamente esas opciones o, si no hay, una nueva. `None` si no cabe otra.
fn lista_para(listas: &mut Vec<ListaRangos>, opciones: Vec<OpcionVariable>) -> Option<u16> {
    if let Some(l) = listas.iter().find(|l| l.opciones == opciones) {
        return Some(l.id);
    }
    if listas.len() >= MAX_LISTAS_RANGOS {
        return None;
    }
    let id = listas.iter().map(|l| l.id).max().unwrap_or(0) + 1;
    let nombre = (listas.len() + 1..)
        .map(|n| format!("Rangos {n}"))
        .find(|c| !listas.iter().any(|l| l.nombre.to_lowercase() == c.to_lowercase()))
        .expect("siempre hay un nombre libre");
    listas.push(ListaRangos { id, nombre, opciones });
    Some(id)
}

// ------------------------------------------------------------------
// Comandos (solo del host)
// ------------------------------------------------------------------

#[tauri::command]
pub fn obtener_listas_rangos(state: State<'_, AppState>) -> Vec<ListaRangos> {
    state.lock().listas_rangos.clone()
}

/// Crea una lista (el id lo pone Rust). Guardar dos veces lo mismo (un doble clic) devuelve la que ya hay, sin error.
#[tauri::command]
pub fn crear_lista_rangos(lista: ListaRangos, state: State<'_, AppState>) -> Result<ListaRangos, String> {
    crear_lista_rangos_en(lista, &state)
}

pub fn crear_lista_rangos_en(lista: ListaRangos, state: &AppState) -> Result<ListaRangos, String> {
    let mut est = state.lock();
    let id = est.listas_rangos.iter().map(|l| l.id).max().unwrap_or(0) + 1;
    let nueva = validar_lista(&[], ListaRangos { id, ..lista })?;
    if let Some(ya) = est.listas_rangos.iter().find(|l| l.nombre.to_lowercase() == nueva.nombre.to_lowercase() && l.opciones == nueva.opciones) {
        return Ok(ya.clone());
    }
    let nueva = validar_lista(&est.listas_rangos, nueva)?;
    registrar(&mut est, &state.almacen, DeltaPartida::ListaRangosGuardada(nueva.clone()));
    Ok(nueva)
}

/// Cambia el nombre o los rangos de una lista: todas las variables que la usan los reciben. Las elecciones de las fichas que ya no existan
/// (si se quitan rangos) vuelven al primero.
#[tauri::command]
pub fn modificar_lista_rangos(lista: ListaRangos, state: State<'_, AppState>) -> Result<(), String> {
    modificar_lista_rangos_en(lista, &state)
}

pub fn modificar_lista_rangos_en(lista: ListaRangos, state: &AppState) -> Result<(), String> {
    let mut est = state.lock();
    let Some(actual) = est.listas_rangos.iter().find(|l| l.id == lista.id) else { return Err("Esa lista de rangos ya no existe".into()) };
    let otras: Vec<ListaRangos> = est.listas_rangos.iter().filter(|l| l.id != lista.id).cloned().collect();
    let nueva = validar_lista(&otras, lista)?;
    if *actual == nueva {
        return Ok(());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::ListaRangosGuardada(nueva));
    Ok(())
}

/// Borra una lista: las variables que la usaban se quedan sin rangos (las elecciones de las fichas se pierden de vista, no su resto).
#[tauri::command]
pub fn borrar_lista_rangos(id: u16, state: State<'_, AppState>) -> Result<(), String> {
    borrar_lista_rangos_en(id, &state)
}

pub fn borrar_lista_rangos_en(id: u16, state: &AppState) -> Result<(), String> {
    let mut est = state.lock();
    if !est.listas_rangos.iter().any(|l| l.id == id) {
        return Err("Esa lista de rangos ya no existe".into());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::ListaRangosBorrada(id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tablero_general::variables::pruebas::{derivada, var};

    fn rango(n: &str, b: i16) -> OpcionVariable {
        OpcionVariable { nombre: n.into(), bonus: b }
    }
    fn competencia() -> ListaRangos {
        ListaRangos { id: 1, nombre: "Competencia".into(), opciones: vec![rango("Sin entrenar", 0), rango("Entrenado", 2), rango("Experto", 4)] }
    }

    #[test]
    fn validacion_de_las_listas() {
        let ok = validar_lista(&[], competencia()).unwrap();
        assert_eq!(ok, competencia());
        // Los nombres se limpian
        let l = ListaRangos { nombre: "  Mi   lista ".into(), opciones: vec![rango(" Alto  nivel ", 3)], ..competencia() };
        let l = validar_lista(&[], l).unwrap();
        assert_eq!((l.nombre.as_str(), l.opciones[0].nombre.as_str()), ("Mi lista", "Alto nivel"));
        let mal = |f: &dyn Fn(&mut ListaRangos)| {
            let mut l = competencia();
            f(&mut l);
            validar_lista(&[], l)
        };
        assert!(mal(&|l| l.nombre = "  ".into()).is_err());
        assert!(mal(&|l| l.nombre = "x".repeat(31)).is_err());
        assert!(mal(&|l| l.nombre = "a\u{202e}b".into()).is_err());
        assert!(mal(&|l| l.opciones.clear()).is_err(), "una lista sin rangos no sirve");
        assert!(mal(&|l| l.opciones = (0..13).map(|i| rango(&format!("R{i}"), i)).collect()).is_err());
        assert!(mal(&|l| l.opciones[0].nombre = "".into()).is_err());
        assert!(mal(&|l| l.opciones[1].nombre = " sin ENTRENAR ".into()).is_err(), "repetido sin mayúsculas");
        assert!(mal(&|l| l.opciones[1].bonus = 10_000).is_err());
        // Contra las demás: nombre repetido y tope
        assert!(validar_lista(&[competencia()], ListaRangos { id: 2, nombre: "competencia".into(), ..competencia() }).is_err());
        let muchas: Vec<_> = (0..MAX_LISTAS_RANGOS as u16).map(|i| ListaRangos { id: i, nombre: format!("L{i}"), ..competencia() }).collect();
        assert!(validar_lista(&muchas, ListaRangos { id: 99, nombre: "Otra".into(), ..competencia() }).unwrap_err().contains("más de"));
    }

    #[test]
    fn las_variables_reciben_una_copia_de_su_lista_y_una_lista_que_falta_se_quita() {
        let mut listas = vec![competencia()];
        let mut defs = vec![
            var(1, "Destreza", "des", 10, 2),
            DefinicionVariable { rangos: Some(1), ..derivada(2, "Acrobacias", "acro", &[1], &[]) },
            DefinicionVariable { rangos: Some(1), ..derivada(3, "Sigilo", "sig", &[1], &[]) },
            DefinicionVariable { rangos: Some(9), opciones: vec![rango("Viejo", 1)], ..derivada(4, "Atletismo", "atl", &[1], &[]) },
            derivada(5, "Otra", "otra", &[1], &[]),
        ];
        // Una base con restos no los conserva
        defs[0].rangos = Some(1);
        defs[0].opciones = vec![rango("X", 1)];
        normalizar_rangos(&mut listas, &mut defs);
        assert_eq!((defs[0].rangos, defs[0].opciones.len()), (None, 0));
        assert_eq!(defs[1].opciones, competencia().opciones);
        assert_eq!(defs[2].opciones, competencia().opciones);
        assert_eq!((defs[3].rangos, defs[3].opciones.len()), (None, 0), "su lista no existe");
        assert_eq!((defs[4].rangos, defs[4].opciones.len()), (None, 0), "sin lista, sin rangos");
        assert_eq!(listas.len(), 1);
        // Cambiar la lista cambia a todas
        listas[0].opciones.push(rango("Maestro", 6));
        normalizar_rangos(&mut listas, &mut defs);
        assert_eq!(defs[1].opciones.len(), 4);
        assert_eq!(defs[2].opciones.len(), 4);
    }

    #[test]
    fn las_opciones_propias_de_antes_pasan_a_una_lista_sin_repetir() {
        let mut listas = vec![];
        let tres = vec![rango("A", 0), rango("B", 1)];
        let mut defs = vec![
            var(1, "Destreza", "des", 10, 2),
            DefinicionVariable { opciones: tres.clone(), ..derivada(2, "Acrobacias", "acro", &[1], &[]) },
            DefinicionVariable { opciones: tres.clone(), ..derivada(3, "Sigilo", "sig", &[1], &[]) },
            DefinicionVariable { opciones: vec![rango("Z", 5)], ..derivada(4, "Atletismo", "atl", &[1], &[]) },
        ];
        normalizar_rangos(&mut listas, &mut defs);
        // Dos listas: las dos con las mismas opciones comparten una
        assert_eq!(listas.len(), 2);
        assert_eq!((defs[1].rangos, defs[2].rangos), (Some(1), Some(1)));
        assert_eq!(defs[3].rangos, Some(2));
        assert_eq!(listas[0].nombre, "Rangos 1");
        assert_eq!(listas[1].nombre, "Rangos 2");
        assert_eq!(defs[1].opciones, tres, "no se pierde ningún rango");
        // Volver a normalizar no crea más
        normalizar_rangos(&mut listas, &mut defs);
        assert_eq!(listas.len(), 2);
    }
}
