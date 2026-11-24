// src-tauri/src/tablero_general/macros/mod.rs
//
// Macros: pequeños programas que el host y los jugadores guardan en la partida, asocian a fichas que controlan y que se ejecutan al
// pulsar su botón (ONCLICK, en la caja «Parámetros») o solos cuando el turno de batalla llega a esa instancia (ONTURN).
// El lenguaje (palabras clave en inglés, sobre la sintaxis de los dados) está en lexico.rs / sintaxis.rs / interprete.rs y la
// referencia para usuarios en docs/macros.md.
//
// Cada efecto es un `DeltaPartida` normal registrado con `partida::registrar`, así que va al log y a la red como cualquier cambio.
// Las de turno se lanzan desde `partida::registrar` (no desde `aplicar_delta`, que se repite al cargar el log: si no, se volverían
// a ejecutar al abrir la partida).

pub mod interprete;
pub mod lexico;
pub mod sintaxis;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::tablero_general::almacen::Almacen;
use crate::tablero_general::escalas::Valoracion;
use crate::tablero_general::estado::{DeltaPartida, EstadoPartida};
use crate::tablero_general::jugadores::{controla_entidad, controla_instancia};
use crate::tablero_general::modelos::{AsociacionMacro, DestinoMacro, IdJugador, Macro, ID_HOST};
use crate::tablero_general::partida::{registrar, registrar_tirada_en, AppState};
use interprete::{Mundo, MAX_ASOCIACIONES_POR_DESTINO};

/// Macros de un jugador (y del host, que no tiene por qué tener muchas más).
pub const MAX_MACROS_JUGADOR: usize = 20;
pub const MAX_MACROS_HOST: usize = 200;
/// Largo del nombre de una macro.
pub const MAX_NOMBRE: usize = 40;

/// El mundo de verdad: la partida con el lock cogido. Cada delta se registra (log + red) en el momento.
pub struct MundoPartida<'a> {
    pub est: &'a mut EstadoPartida,
    pub almacen: &'a Almacen,
}

impl Mundo for MundoPartida<'_> {
    fn estado(&self) -> &EstadoPartida {
        self.est
    }
    fn aplicar(&mut self, delta: DeltaPartida) {
        registrar(self.est, self.almacen, delta);
    }
    fn tirar(&mut self, caras: i32) -> i32 {
        rand::random_range(1..=caras.max(1))
    }
    fn chat(&mut self, jugador: IdJugador, desglose: &str, total: i32, publica: bool, valoracion: Option<&Valoracion>, informativa: bool) {
        registrar_tirada_en(self.est, self.almacen, jugador, desglose, total, publica, valoracion, informativa);
    }
}

/// Ejecuta una asociación con el estado ya bloqueado, marcando que hay una macro en curso (lo que cambie no dispara las de turno).
fn ejecutar_con_lock(est: &mut EstadoPartida, almacen: &Almacen, id_asociacion: u32, id_instancia: u16, quien: IdJugador) -> Result<(), String> {
    let antes = est.macros_en_curso;
    est.macros_en_curso = true;
    let r = interprete::ejecutar(&mut MundoPartida { est, almacen }, id_asociacion, id_instancia, quien);
    est.macros_en_curso = antes;
    r
}

// ------------------------------------------------------------------
// Turnos: las ONTURN
// ------------------------------------------------------------------

/// Quién tiene el turno en cada grupo de batalla: `(mapa, grupo, turno, ronda)`.
pub type Turnos = Vec<(u16, u16, Option<u16>, u32)>;

pub fn turnos(est: &EstadoPartida) -> Turnos {
    est.mapas
        .iter()
        .filter_map(|m| m.batalla.as_ref().map(|b| (m.id, b)))
        .flat_map(|(id, b)| b.grupos.iter().map(move |g| (id, g.id, g.turno, g.ronda)))
        .collect()
}

/// ¿Merece la pena mirar los turnos antes de un delta? Solo si hay alguna macro de turno asociada y nada en curso.
pub fn vigilar_turnos(est: &EstadoPartida) -> bool {
    !est.macros_en_curso
        && est.asociaciones_macro.iter().any(|a| est.macros.iter().any(|m| m.id == a.id_macro && m.en_turno))
        && est.mapas.iter().any(|m| m.batalla.is_some())
}

/// Tras un delta: a cada instancia a la que le acaba de llegar el turno (otro dueño del turno, o la misma en una vuelta nueva) se
/// le ejecutan sus macros de turno, las de su ficha y las suyas, con la autoridad de quien las asoció.
pub fn tras_cambio_de_turno(est: &mut EstadoPartida, almacen: &Almacen, antes: &Turnos) {
    let ahora = turnos(est);
    let llegan: Vec<u16> = ahora
        .iter()
        .filter(|(m, g, t, r)| t.is_some() && !antes.iter().any(|(m2, g2, t2, r2)| m2 == m && g2 == g && t2 == t && r2 == r))
        .filter_map(|(_, _, t, _)| *t)
        .collect();
    for id in llegan {
        let Some(ficha) = est.instancias_activas.iter().find(|i| i.id_instancia == id).map(|i| i.base.id) else { continue };
        let tareas: Vec<(u32, IdJugador)> = est
            .asociaciones_macro
            .iter()
            .filter(|a| a.destino == DestinoMacro::Instancia(id) || a.destino == DestinoMacro::Ficha(ficha))
            .filter(|a| est.macros.iter().any(|m| m.id == a.id_macro && m.en_turno))
            .map(|a| (a.id, a.autor))
            .collect();
        for (asoc, autor) in tareas {
            // Un error ya sale en el chat; las demás siguen
            let _ = ejecutar_con_lock(est, almacen, asoc, id, autor);
        }
    }
}

// ------------------------------------------------------------------
// Lo que ve un jugador
// ------------------------------------------------------------------

/// ¿Controla `jugador` aquello a lo que está asociada la macro?
fn controla_destino(est: &EstadoPartida, jugador: IdJugador, destino: DestinoMacro) -> bool {
    match destino {
        DestinoMacro::Ficha(id) => controla_entidad(est, jugador, id),
        DestinoMacro::Instancia(id) => controla_instancia(est, jugador, id),
    }
}

/// Las asociaciones de las fichas e instancias que controla.
pub fn asociaciones_para(jugador: IdJugador, est: &EstadoPartida) -> Vec<AsociacionMacro> {
    est.asociaciones_macro.iter().filter(|a| controla_destino(est, jugador, a.destino)).cloned().collect()
}

/// Sus macros enteras y, de las demás, solo nombre y tipo de las que están asociadas a lo suyo (el código no sale).
pub fn macros_para(jugador: IdJugador, est: &EstadoPartida) -> Vec<Macro> {
    let asociadas: Vec<u16> = asociaciones_para(jugador, est).iter().map(|a| a.id_macro).collect();
    est.macros
        .iter()
        .filter(|m| m.autor == jugador || asociadas.contains(&m.id))
        .map(|m| if m.autor == jugador || jugador == ID_HOST { m.clone() } else { Macro { codigo: String::new(), ..m.clone() } })
        .collect()
}

// ------------------------------------------------------------------
// Comandos (cada uno con su `_en` para que la red use la misma regla)
// ------------------------------------------------------------------

/// Lo que manda el front al guardar una macro. `id` 0 = nueva.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DatosMacro {
    #[serde(default)]
    pub id: u16,
    pub nombre: String,
    pub codigo: String,
}

/// Cómo es una macro que se puede leer.
#[derive(Serialize, Debug, PartialEq)]
pub struct Comprobacion {
    pub al_pulsar: bool,
    pub en_turno: bool,
}

pub fn comprobar(codigo: &str) -> Result<Comprobacion, String> {
    let p = sintaxis::leer(codigo).map_err(|e| e.to_string())?;
    Ok(Comprobacion { al_pulsar: p.al_pulsar, en_turno: p.en_turno })
}

fn validar_nombre(nombre: &str) -> Result<String, String> {
    let n = nombre.split_whitespace().collect::<Vec<_>>().join(" ");
    let largo = n.chars().count();
    if largo == 0 || largo > MAX_NOMBRE {
        return Err(format!("El nombre de una macro va de 1 a {MAX_NOMBRE} caracteres"));
    }
    if !n.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '_') {
        return Err("El nombre de una macro solo lleva letras, cifras y espacios (así se puede nombrar en MACROS.nombre)".into());
    }
    Ok(n)
}

#[tauri::command]
pub fn validar_macro(codigo: String) -> Result<Comprobacion, String> {
    comprobar(&codigo)
}

#[tauri::command]
pub fn obtener_macros(state: State<'_, AppState>) -> Vec<Macro> {
    state.lock().macros.clone()
}

#[tauri::command]
pub fn obtener_asociaciones_macro(state: State<'_, AppState>) -> Vec<AsociacionMacro> {
    state.lock().asociaciones_macro.clone()
}

#[tauri::command]
pub fn guardar_macro(datos: DatosMacro, state: State<'_, AppState>) -> Result<Macro, String> {
    guardar_macro_en(&state, ID_HOST, datos)
}

/// Crea (`id` 0) o edita una macro. Un jugador solo edita las suyas; el host, cualquiera (conserva su autor).
pub fn guardar_macro_en(state: &AppState, jugador: IdJugador, datos: DatosMacro) -> Result<Macro, String> {
    let nombre = validar_nombre(&datos.nombre)?;
    let tipo = comprobar(&datos.codigo)?;
    let mut est = state.lock();
    let autor = if datos.id == 0 {
        let cuantas = est.macros.iter().filter(|m| m.autor == jugador).count();
        let tope = if jugador == ID_HOST { MAX_MACROS_HOST } else { MAX_MACROS_JUGADOR };
        if cuantas >= tope {
            return Err(format!("No puedes tener más de {tope} macros"));
        }
        jugador
    } else {
        let m = est.macros.iter().find(|m| m.id == datos.id).ok_or("Esa macro no existe")?;
        if jugador != ID_HOST && m.autor != jugador {
            return Err("Solo puedes cambiar tus macros".into());
        }
        m.autor
    };
    let junto = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    if est.macros.iter().any(|m| m.autor == autor && m.id != datos.id && junto(&m.nombre) == junto(&nombre)) {
        return Err(format!("Ya hay una macro que se llama «{nombre}»"));
    }
    let id = if datos.id == 0 { est.macros.iter().map(|m| m.id).max().unwrap_or(0).checked_add(1).ok_or("No caben más macros")? } else { datos.id };
    let m = Macro { id, nombre, codigo: datos.codigo, autor, al_pulsar: tipo.al_pulsar, en_turno: tipo.en_turno };
    if est.macros.iter().any(|x| *x == m) {
        return Ok(m); // sin cambios, sin delta
    }
    registrar(&mut est, &state.almacen, DeltaPartida::MacroGuardada(m.clone()));
    Ok(m)
}

#[tauri::command]
pub fn borrar_macro(id: u16, state: State<'_, AppState>) -> Result<(), String> {
    borrar_macro_en(&state, ID_HOST, id)
}

pub fn borrar_macro_en(state: &AppState, jugador: IdJugador, id: u16) -> Result<(), String> {
    let mut est = state.lock();
    let m = est.macros.iter().find(|m| m.id == id).ok_or("Esa macro no existe")?;
    if jugador != ID_HOST && m.autor != jugador {
        return Err("Solo puedes borrar tus macros".into());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::MacroBorrada(id));
    Ok(())
}

#[tauri::command]
pub fn asociar_macro(id_macro: u16, id_entidad: u16, state: State<'_, AppState>) -> Result<u32, String> {
    asociar_macro_en(&state, ID_HOST, id_macro, id_entidad)
}

/// Asocia una macro a una FICHA (todas sus instancias). Solo a una ficha que controla, y solo una macro suya (el host, cualquiera).
pub fn asociar_macro_en(state: &AppState, jugador: IdJugador, id_macro: u16, id_entidad: u16) -> Result<u32, String> {
    let mut est = state.lock();
    if !est.banco_entidades.iter().any(|e| e.id == id_entidad) {
        return Err("Esa ficha no existe".into());
    }
    if !controla_entidad(&est, jugador, id_entidad) {
        return Err("Solo puedes asociar macros a tus fichas".into());
    }
    let m = est.macros.iter().find(|m| m.id == id_macro).ok_or("Esa macro no existe")?;
    if jugador != ID_HOST && m.autor != jugador {
        return Err("Solo puedes asociar tus macros".into());
    }
    let destino = DestinoMacro::Ficha(id_entidad);
    let ya: Vec<&AsociacionMacro> = est.asociaciones_macro.iter().filter(|a| a.destino == destino).collect();
    if let Some(a) = ya.iter().find(|a| a.id_macro == id_macro) {
        return Ok(a.id);
    }
    if ya.len() >= MAX_ASOCIACIONES_POR_DESTINO {
        return Err(format!("Una ficha no puede tener más de {MAX_ASOCIACIONES_POR_DESTINO} macros"));
    }
    let id = est.asociaciones_macro.iter().map(|a| a.id).max().unwrap_or(0) + 1;
    registrar(&mut est, &state.almacen, DeltaPartida::MacroAsociada(AsociacionMacro { id, id_macro, destino, autor: jugador, atributos: Vec::new() }));
    Ok(id)
}

#[tauri::command]
pub fn desasociar_macro(id_asociacion: u32, state: State<'_, AppState>) -> Result<(), String> {
    desasociar_macro_en(&state, ID_HOST, id_asociacion)
}

pub fn desasociar_macro_en(state: &AppState, jugador: IdJugador, id_asociacion: u32) -> Result<(), String> {
    let mut est = state.lock();
    let a = est.asociaciones_macro.iter().find(|a| a.id == id_asociacion).ok_or("Esa macro ya no está asociada")?;
    if !controla_destino(&est, jugador, a.destino) {
        return Err("Solo puedes quitar macros de tus fichas".into());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::MacroDesasociada(id_asociacion));
    Ok(())
}

#[tauri::command]
pub fn ejecutar_macro(id_asociacion: u32, id_instancia: u16, state: State<'_, AppState>) -> Result<(), String> {
    ejecutar_macro_en(&state, ID_HOST, id_asociacion, id_instancia)
}

/// El botón de una macro ONCLICK en la caja «Parámetros»: solo sobre una instancia que controla y a la que está asociada.
pub fn ejecutar_macro_en(state: &AppState, jugador: IdJugador, id_asociacion: u32, id_instancia: u16) -> Result<(), String> {
    let mut est = state.lock();
    let a = est.asociaciones_macro.iter().find(|a| a.id == id_asociacion).ok_or("Esa macro ya no está asociada")?;
    let inst = est.instancias_activas.iter().find(|i| i.id_instancia == id_instancia).ok_or("Esa ficha ya no está activa")?;
    let suya = match a.destino {
        DestinoMacro::Ficha(id) => inst.base.id == id,
        DestinoMacro::Instancia(id) => id == id_instancia,
    };
    if !suya {
        return Err("Esa macro no está asociada a esta ficha".into());
    }
    if !controla_instancia(&est, jugador, id_instancia) {
        return Err("Solo puedes usar las macros de tus fichas".into());
    }
    if !est.macros.iter().any(|m| m.id == a.id_macro && m.al_pulsar) {
        return Err("Esta macro no es ONCLICK: va sola con el turno".into());
    }
    ejecutar_con_lock(&mut est, &state.almacen, id_asociacion, id_instancia, jugador)
}

#[cfg(test)]
mod tests;
