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
use crate::tablero_general::modelos::{AsociacionMacro, Casilla, DestinoMacro, IdJugador, Macro, MacroConjunto, MacroFicha, ID_HOST};
use crate::tablero_general::partida::{registrar, registrar_tirada_en, AppState};
use interprete::{Mundo, MAX_ASOCIACIONES_POR_DESTINO};

/// Macros de un jugador (y del host, que no tiene por qué tener muchas más).
pub const MAX_MACROS_JUGADOR: usize = 20;
pub const MAX_MACROS_HOST: usize = 200;
/// Largo del nombre de una macro.
pub const MAX_NOMBRE: usize = 40;
/// Macros que puede traer un conjunto de parámetros.
pub const MAX_MACROS_CONJUNTO: usize = 200;
/// Macros que puede traer la plantilla de una ficha: las del personaje y todas las generales del conjunto.
pub const MAX_MACROS_PLANTILLA: usize = MAX_ASOCIACIONES_POR_DESTINO + MAX_MACROS_CONJUNTO;

// ------------------------------------------------------------------
// Conjuntos de parámetros: una macro es del conjunto que estaba activo al crearla
// ------------------------------------------------------------------

/// ¿Se puede usar ahora? Solo con su conjunto activo (usa sus nombres); una sin conjunto (de antes de esto) vale en cualquiera.
pub fn activa(est: &EstadoPartida, m: &Macro) -> bool {
    m.conjunto.is_none_or(|c| c == est.conjunto_activo)
}

/// ¿Pueden chocar dos macros por su nombre? Si las dos se pueden usar a la vez (mismo conjunto, o alguna sin conjunto).
fn mismo_ambito(a: Option<u16>, b: Option<u16>) -> bool {
    a.is_none() || b.is_none() || a == b
}

/// ¿Chocan por el nombre dos macros de estos personajes? Las de un mismo personaje, y una general con cualquiera (se nombran igual en `MACROS.x`).
fn mismo_personaje(a: Option<u16>, b: Option<u16>) -> bool {
    a.is_none() || b.is_none() || a == b
}

fn junto(t: &str) -> String {
    t.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

/// Las macros de un conjunto (las de serie, las de una plantilla) con las reglas de siempre: nombre, código y sin nombres repetidos.
pub fn validar_de_conjunto(lista: Vec<MacroConjunto>) -> Result<Vec<MacroConjunto>, String> {
    if lista.len() > MAX_MACROS_CONJUNTO {
        return Err(format!("Un conjunto no puede traer más de {MAX_MACROS_CONJUNTO} macros"));
    }
    let mut buenas: Vec<MacroConjunto> = Vec::with_capacity(lista.len());
    for m in lista {
        let nombre = validar_nombre(&m.nombre)?;
        comprobar(&m.codigo).map_err(|e| format!("Macro «{nombre}»: {e}"))?;
        if buenas.iter().any(|b| junto(&b.nombre) == junto(&nombre)) {
            return Err(format!("El conjunto trae dos macros que se llaman «{nombre}»"));
        }
        buenas.push(MacroConjunto { nombre, codigo: m.codigo });
    }
    Ok(buenas)
}

/// Las macros (del host) que nacen con un conjunto `conjunto`, con ids libres a partir de las que ya hay. Las que no se pueden leer se
/// saltan (un conjunto ya validado no trae ninguna así).
pub fn macros_nuevas(est: &EstadoPartida, conjunto: u16, lista: Vec<MacroConjunto>) -> Vec<Macro> {
    let mut id = est.macros.iter().map(|m| m.id).max().unwrap_or(0);
    lista
        .into_iter()
        .filter_map(|m| {
            let tipo = comprobar(&m.codigo).ok()?;
            id = id.checked_add(1)?;
            Some(Macro { id, nombre: m.nombre, codigo: m.codigo, autor: ID_HOST, al_pulsar: tipo.al_pulsar, en_turno: tipo.en_turno, conjunto: Some(conjunto), propietaria: None })
        })
        .collect()
}

/// Al añadir un conjunto a la partida, sus macros pasan a ser macros del host de ese conjunto (una `MacroGuardada` cada una).
pub fn crear_de_conjunto(est: &mut EstadoPartida, almacen: &Almacen, conjunto: u16, lista: Vec<MacroConjunto>) {
    for m in macros_nuevas(est, conjunto, lista) {
        registrar(est, almacen, DeltaPartida::MacroGuardada(m));
    }
}

/// Las macros del host del conjunto activo, como datos (para exportar el conjunto entero en la plantilla de los parámetros).
pub fn del_conjunto_activo(est: &EstadoPartida) -> Vec<MacroConjunto> {
    est.macros
        .iter()
        .filter(|m| m.es_general() && activa(est, m))
        .map(|m| MacroConjunto { nombre: m.nombre.clone(), codigo: m.codigo.clone() })
        .collect()
}

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
fn ejecutar_con_lock(est: &mut EstadoPartida, almacen: &Almacen, id_asociacion: u32, id_instancia: u16, quien: IdJugador, centros: &[Casilla]) -> Result<(), String> {
    let antes = est.macros_en_curso;
    est.macros_en_curso = true;
    let r = interprete::ejecutar(&mut MundoPartida { est, almacen }, id_asociacion, id_instancia, quien, centros);
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
        && est.asociaciones_macro.iter().any(|a| est.macros.iter().any(|m| m.id == a.id_macro && m.en_turno && activa(est, m)))
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
            .filter(|a| est.macros.iter().any(|m| m.id == a.id_macro && m.en_turno && activa(est, m)))
            .map(|a| (a.id, a.autor))
            .collect();
        for (asoc, autor) in tareas {
            // Un error ya sale en el chat; las demás siguen
            let _ = ejecutar_con_lock(est, almacen, asoc, id, autor, &[]);
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
    est.asociaciones_macro
        .iter()
        .filter(|a| controla_destino(est, jugador, a.destino) && est.macros.iter().any(|m| m.id == a.id_macro && activa(est, m)))
        .cloned()
        .collect()
}

/// Sus macros enteras y, de las demás, solo nombre y tipo de las generales y de las que están asociadas a lo suyo (el código no sale).
pub fn macros_para(jugador: IdJugador, est: &EstadoPartida) -> Vec<Macro> {
    let asociadas: Vec<u16> = asociaciones_para(jugador, est).iter().map(|a| a.id_macro).collect();
    est.macros
        .iter()
        .filter(|m| activa(est, m) && (m.autor == jugador || m.es_general() || asociadas.contains(&m.id)))
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
    /// Al CREAR: el personaje (ficha) del que es la macro; `None` = general (del conjunto: solo el host).
    #[serde(default, rename = "idEntidad", alias = "id_entidad")]
    pub id_entidad: Option<u16>,
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

pub(crate) fn validar_nombre(nombre: &str) -> Result<String, String> {
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
    // Solo las del conjunto que se está usando: las de otros conjuntos se quedan guardadas pero no se ven
    let est = state.lock();
    est.macros.iter().filter(|m| activa(&est, m)).cloned().collect()
}

#[tauri::command]
pub fn obtener_asociaciones_macro(state: State<'_, AppState>) -> Vec<AsociacionMacro> {
    state.lock().asociaciones_macro.clone()
}

#[tauri::command]
pub fn guardar_macro(datos: DatosMacro, state: State<'_, AppState>) -> Result<Macro, String> {
    guardar_macro_en(&state, ID_HOST, datos)
}

/// Crea (`id` 0) o edita una macro. Con `id_entidad` es de ESE personaje (una ficha que controla quien la crea) y queda asociada a él; sin
/// él es general, del conjunto, y solo la crea el host. Un jugador solo edita las suyas; el host, cualquiera (conserva su autor).
pub fn guardar_macro_en(state: &AppState, jugador: IdJugador, datos: DatosMacro) -> Result<Macro, String> {
    let nombre = validar_nombre(&datos.nombre)?;
    let tipo = comprobar(&datos.codigo)?;
    let mut est = state.lock();
    // Una macro nueva es del conjunto activo y, si se crea desde una ficha, de ese personaje; al editarla conserva ambos
    let (autor, conjunto, propietaria) = if datos.id == 0 {
        let cuantas = est.macros.iter().filter(|m| m.autor == jugador).count();
        let tope = if jugador == ID_HOST { MAX_MACROS_HOST } else { MAX_MACROS_JUGADOR };
        if cuantas >= tope {
            return Err(format!("No puedes tener más de {tope} macros"));
        }
        match datos.id_entidad {
            Some(e) => {
                if !est.banco_entidades.iter().any(|x| x.id == e) {
                    return Err("Esa ficha no existe".into());
                }
                if !controla_entidad(&est, jugador, e) {
                    return Err("Solo puedes crear macros para tus fichas".into());
                }
            }
            None if jugador != ID_HOST => {
                return Err("Solo el host crea macros generales: las tuyas se crean desde una de tus fichas (caja «Parámetros» → + Macro)".into());
            }
            None => {}
        }
        (jugador, Some(est.conjunto_activo), datos.id_entidad)
    } else {
        let m = est.macros.iter().find(|m| m.id == datos.id).ok_or("Esa macro no existe")?;
        if jugador != ID_HOST && m.autor != jugador {
            return Err("Solo puedes cambiar tus macros".into());
        }
        (m.autor, m.conjunto, m.propietaria)
    };
    if est.macros.iter().any(|m| m.id != datos.id && mismo_ambito(m.conjunto, conjunto) && mismo_personaje(m.propietaria, propietaria) && junto(&m.nombre) == junto(&nombre)) {
        return Err(format!("Ya hay una macro que se llama «{nombre}»"));
    }
    let id = if datos.id == 0 { est.macros.iter().map(|m| m.id).max().unwrap_or(0).checked_add(1).ok_or("No caben más macros")? } else { datos.id };
    let m = Macro { id, nombre, codigo: datos.codigo, autor, al_pulsar: tipo.al_pulsar, en_turno: tipo.en_turno, conjunto, propietaria };
    if est.macros.iter().any(|x| *x == m) {
        return Ok(m); // sin cambios, sin delta
    }
    registrar(&mut est, &state.almacen, DeltaPartida::MacroGuardada(m.clone()));
    // Una macro de un personaje nace asociada a él
    if let (0, Some(e)) = (datos.id, propietaria) {
        asociar_en(&mut est, &state.almacen, id, e, autor);
    }
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
    // Las de un personaje solo se asocian a él; las generales las puede poner cualquiera en sus fichas; las demás, solo su autor
    if m.propietaria.is_some_and(|p| p != id_entidad) {
        return Err("Esa macro es de otro personaje".into());
    }
    if jugador != ID_HOST && m.autor != jugador && !m.es_general() {
        return Err("Solo puedes asociar tus macros o las generales".into());
    }
    if !activa(&est, m) {
        return Err("Esa macro es de otro conjunto de parámetros".into());
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
    registrar(&mut est, &state.almacen, DeltaPartida::MacroAsociada(AsociacionMacro { id, id_macro, destino, autor: jugador }));
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

/// Centros de `AREA` que admite una ejecución, y hasta dónde puede llegar cada coordenada.
pub const MAX_CENTROS: usize = 16;
pub const MAX_COORDENADA: u16 = 10_000;

/// Antes de ejecutar una macro ONCLICK: los `AREA` que hay que apuntar en el tablero (vacío si no tiene ninguno), con sus lados ya
/// calculados con la ficha, para la vista previa. Los mismos permisos que ejecutarla.
#[tauri::command]
pub fn preparar_macro(id_asociacion: u32, id_instancia: u16, state: State<'_, AppState>) -> Result<Vec<interprete::AreaResuelta>, String> {
    preparar_macro_en(&state, ID_HOST, id_asociacion, id_instancia)
}

pub fn preparar_macro_en(state: &AppState, jugador: IdJugador, id_asociacion: u32, id_instancia: u16) -> Result<Vec<interprete::AreaResuelta>, String> {
    let est = state.lock();
    comprobar_ejecutable(&est, jugador, id_asociacion, id_instancia)?;
    interprete::areas_de(&est, id_asociacion, id_instancia)
}

#[tauri::command]
pub fn ejecutar_macro(id_asociacion: u32, id_instancia: u16, centros: Option<Vec<Casilla>>, state: State<'_, AppState>) -> Result<(), String> {
    ejecutar_macro_en(&state, ID_HOST, id_asociacion, id_instancia, centros.unwrap_or_default())
}

/// El botón de una macro ONCLICK en la caja «Parámetros»: solo sobre una instancia que controla y a la que está asociada. `centros`: dónde
/// apuntó el usuario cada `AREA` de la macro, en orden.
pub fn ejecutar_macro_en(state: &AppState, jugador: IdJugador, id_asociacion: u32, id_instancia: u16, centros: Vec<Casilla>) -> Result<(), String> {
    if centros.len() > MAX_CENTROS || centros.iter().any(|c| c.x > MAX_COORDENADA || c.y > MAX_COORDENADA) {
        return Err("Demasiadas áreas, o fuera del tablero".into());
    }
    let mut est = state.lock();
    comprobar_ejecutable(&est, jugador, id_asociacion, id_instancia)?;
    ejecutar_con_lock(&mut est, &state.almacen, id_asociacion, id_instancia, jugador, &centros)
}

/// ¿Puede `jugador` ejecutar con el botón la asociación sobre esa instancia? (que la controle, que esté asociada a ella, ONCLICK y activa)
fn comprobar_ejecutable(est: &EstadoPartida, jugador: IdJugador, id_asociacion: u32, id_instancia: u16) -> Result<(), String> {
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
    let m = est.macros.iter().find(|m| m.id == a.id_macro).ok_or("Esa macro ya no existe")?;
    if !activa(est, m) {
        return Err("Esa macro es de otro conjunto de parámetros".into());
    }
    if !m.al_pulsar {
        return Err("Esta macro no es ONCLICK: va sola con el turno".into());
    }
    Ok(())
}

/// Asocia una macro a una ficha (con el estado ya bloqueado, sin repetir). Devuelve si quedó asociada.
fn asociar_en(est: &mut EstadoPartida, almacen: &Almacen, id_macro: u16, id_entidad: u16, autor: IdJugador) -> bool {
    let destino = DestinoMacro::Ficha(id_entidad);
    let ya: Vec<&AsociacionMacro> = est.asociaciones_macro.iter().filter(|a| a.destino == destino).collect();
    if ya.iter().any(|a| a.id_macro == id_macro) {
        return true;
    }
    if ya.len() >= MAX_ASOCIACIONES_POR_DESTINO {
        return false;
    }
    let id = est.asociaciones_macro.iter().map(|a| a.id).max().unwrap_or(0) + 1;
    registrar(est, almacen, DeltaPartida::MacroAsociada(AsociacionMacro { id, id_macro, destino, autor }));
    true
}

/// Las macros que trae la plantilla de una ficha (ver plantillaFicha.ts), ya en la partida y asociadas a la ficha:
///  * las del PERSONAJE (`general = false`): si la ficha ya tiene una con ese nombre se usa (no se pisa su código); si no, se crea como suya;
///  * las GENERALES (todas las del conjunto de quien exportó): si el conjunto ya tiene una con ese nombre se usa esa; si no, el host la
///    crea general (con el código que traiga) aunque no esté asociada, para que el conjunto las tenga. Un jugador, que no puede crear
///    generales, solo la crea (como macro de su personaje) si venía asociada; las demás las salta sin avisar. Solo se asocian las que
///    el personaje tenía puestas (`asociada`).
/// Las que no valen (nombre, código, topes) se saltan: devuelve un aviso por cada una.
#[tauri::command]
pub fn importar_macros_ficha(id_entidad: u16, macros: Vec<MacroFicha>, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    importar_macros_ficha_en(&state, ID_HOST, id_entidad, macros)
}

pub fn importar_macros_ficha_en(state: &AppState, jugador: IdJugador, id_entidad: u16, macros: Vec<MacroFicha>) -> Result<Vec<String>, String> {
    if macros.len() > MAX_MACROS_PLANTILLA {
        return Err(format!("Una plantilla no puede traer más de {MAX_MACROS_PLANTILLA} macros"));
    }
    let mut est = state.lock();
    if !est.banco_entidades.iter().any(|e| e.id == id_entidad) {
        return Err("Esa ficha no existe".into());
    }
    if !controla_entidad(&est, jugador, id_entidad) {
        return Err("Solo puedes asociar macros a tus fichas".into());
    }
    let mut avisos = Vec::new();
    for m in macros {
        let nombre = match validar_nombre(&m.nombre) {
            Ok(n) => n,
            Err(e) => {
                avisos.push(format!("Macro sin importar: {e}"));
                continue;
            }
        };
        let junto_nombre = junto(&nombre);
        // La que ya existe: la general del conjunto, o la de este personaje
        let existente = est
            .macros
            .iter()
            .find(|x| {
                activa(&est, x) && junto(&x.nombre) == junto_nombre && if m.general { x.es_general() } else { x.propietaria == Some(id_entidad) }
            })
            .map(|x| x.id);
        let id_macro = match existente {
            Some(id) => id,
            None => {
                // Una general que el personaje no tenía puesta y que un jugador no puede crear: no hace falta nada
                if m.general && !m.asociada && jugador != ID_HOST {
                    continue;
                }
                if m.codigo.trim().is_empty() {
                    if m.asociada {
                        avisos.push(format!("La macro general «{nombre}» no existe en este conjunto y la plantilla no trae su código"));
                    }
                    continue;
                }
                let tipo = match comprobar(&m.codigo) {
                    Ok(t) => t,
                    Err(e) => {
                        avisos.push(format!("La macro «{nombre}» no se ha importado: {e}"));
                        continue;
                    }
                };
                let tope = if jugador == ID_HOST { MAX_MACROS_HOST } else { MAX_MACROS_JUGADOR };
                if est.macros.iter().filter(|x| x.autor == jugador).count() >= tope {
                    avisos.push(format!("La macro «{nombre}» no se ha importado: no puedes tener más de {tope}"));
                    continue;
                }
                // Un nombre ya usado por otra macro que chocaría (por ejemplo una general al importar una del personaje) no se pisa
                let propietaria = if m.general && jugador == ID_HOST { None } else { Some(id_entidad) };
                let conjunto = Some(est.conjunto_activo);
                if est.macros.iter().any(|x| mismo_ambito(x.conjunto, conjunto) && mismo_personaje(x.propietaria, propietaria) && junto(&x.nombre) == junto_nombre) {
                    avisos.push(format!("La macro «{nombre}» no se ha importado: ya hay otra con ese nombre"));
                    continue;
                }
                if m.general && jugador != ID_HOST {
                    avisos.push(format!("«{nombre}» era una macro general: como no eres el host se ha creado como macro de este personaje"));
                }
                let Some(id) = est.macros.iter().map(|x| x.id).max().unwrap_or(0).checked_add(1) else { break };
                let nueva = Macro { id, nombre: nombre.clone(), codigo: m.codigo, autor: jugador, al_pulsar: tipo.al_pulsar, en_turno: tipo.en_turno, conjunto, propietaria };
                registrar(&mut est, &state.almacen, DeltaPartida::MacroGuardada(nueva));
                id
            }
        };
        // Las del personaje siempre; las generales, solo las que traía puestas
        if (!m.general || m.asociada) && !asociar_en(&mut est, &state.almacen, id_macro, id_entidad, jugador) {
            avisos.push(format!("La macro «{nombre}» no se ha asociado: la ficha ya tiene {MAX_ASOCIACIONES_POR_DESTINO}"));
        }
    }
    Ok(avisos)
}

#[cfg(test)]
mod tests;
