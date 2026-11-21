// src-tauri/src/tablero_general/batalla.rs
//
// El modo batalla de un mapa (`Mapa.batalla`): uno o varios GRUPOS, cada uno con su turno y su contador de vueltas.
//  * Se empieza con el comando de chat `Batalla [tirada]` y se acaba con `Terminar` (solo el host). Con tirada, cada ficha tira con
//    SUS variables (como cualquier tirada del chat: los parámetros visuales no se tiran) y se reordenan de mayor a menor («Fichas activas»); sin ella no se reordena.
//  * El orden de los turnos de un grupo es el de `Instancia.orden`: el host puede reordenar a mano y el turno sigue a su instancia.
//    Al llegar al último se vuelve al primero y la vuelta (`ronda`) sube en uno.
//  * Pasa el turno el dueño de la ficha que lo tiene (el host, siempre).
//  * Las funciones de este archivo que no tocan `AppState` son puras (turnos, saneado) y las prueba `cargo test` sin Tauri.
//
// Mismo espejo en el front: src/lib/batalla.ts.
use crate::tablero_general::dados::{dividir_comparacion, parsear_comando, tirar_lado};
use crate::tablero_general::estado::{DeltaPartida, EstadoPartida};
use crate::tablero_general::jugadores::controla_instancia;
use crate::tablero_general::modelos::{IdJugador, ID_HOST};
use crate::tablero_general::modelos::{Batalla, GrupoBatalla};
use crate::tablero_general::partida::{registrar, registrar_tirada_en, AppState};
use crate::tablero_general::variables;
use serde::Deserialize;

/// Nombre del grupo único de una batalla empezada con el comando.
pub const NOMBRE_GRUPO_UNICO: &str = "Batalla";

// ------------------------------------------------------------------
// El comando de chat
// ------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum ComandoBatalla {
    /// `Batalla [tirada]`: lo que sigue a la palabra (puede estar vacío).
    Empezar(String),
    /// `Terminar`: lo que sigue a la palabra (debe estar vacío).
    Terminar(String),
}

/// ¿Es el comando una orden de batalla? La primera palabra entera (sin distinguir mayúsculas) es `batalla` o `terminar`.
pub fn interpretar_comando(comando: &str) -> Option<ComandoBatalla> {
    let t = comando.trim();
    let fin = t.find(char::is_whitespace).unwrap_or(t.len());
    let (palabra, resto) = t.split_at(fin);
    let resto = resto.trim().to_string();
    if palabra.eq_ignore_ascii_case("batalla") {
        Some(ComandoBatalla::Empezar(resto))
    } else if palabra.eq_ignore_ascii_case("terminar") {
        Some(ComandoBatalla::Terminar(resto))
    } else {
        None
    }
}

// ------------------------------------------------------------------
// Turnos (puro)
// ------------------------------------------------------------------

/// Los ids de los miembros de `grupo` que existen, en el orden de «Fichas activas» (el de los turnos).
pub fn orden_de_turno(est: &EstadoPartida, grupo: &GrupoBatalla) -> Vec<u16> {
    let mut v: Vec<(u16, u16)> = grupo
        .miembros
        .iter()
        .filter_map(|id| est.instancias_activas.iter().find(|i| i.id_instancia == *id).map(|i| (i.orden, i.id_instancia)))
        .collect();
    v.sort_unstable();
    v.into_iter().map(|(_, id)| id).collect()
}

/// A quién le toca después y en qué vuelta: el siguiente de la lista, y al volver al primero la vuelta sube. Sin turno aún, el
/// primero (vuelta 1). `None` si el grupo no tiene a nadie.
pub fn siguiente_turno(est: &EstadoPartida, grupo: &GrupoBatalla) -> Option<(u16, u32)> {
    let orden = orden_de_turno(est, grupo);
    let primero = *orden.first()?;
    let Some(pos) = grupo.turno.and_then(|t| orden.iter().position(|id| *id == t)) else {
        return Some((primero, grupo.ronda.max(1)));
    };
    match orden.get(pos + 1) {
        Some(id) => Some((*id, grupo.ronda)),
        None => Some((primero, grupo.ronda.saturating_add(1))),
    }
}

fn batalla_de<'a>(est: &'a EstadoPartida, id_mapa: u16) -> Result<&'a Batalla, String> {
    let mapa = est.mapas.iter().find(|m| m.id == id_mapa).ok_or("Ese mapa no existe")?;
    mapa.batalla.as_ref().ok_or_else(|| "Ese mapa no está en batalla".to_string())
}

/// El delta de pasar el turno del grupo `id_grupo` de la batalla del mapa. Ya resuelto: lleva el turno y la vuelta nuevos.
pub fn delta_de_pasar_turno(est: &EstadoPartida, id_mapa: u16, id_grupo: u16) -> Result<DeltaPartida, String> {
    let grupo = batalla_de(est, id_mapa)?.grupos.iter().find(|g| g.id == id_grupo).ok_or("Ese grupo ya no existe")?;
    let (turno, ronda) = siguiente_turno(est, grupo).ok_or("Ese grupo no tiene fichas")?;
    Ok(DeltaPartida::TurnoPasado { id_mapa, id_grupo, turno, ronda })
}

/// Pasar el turno como `jugador`: solo si controla la ficha que lo tiene (el host, siempre).
pub fn pasar_turno_de(est: &EstadoPartida, jugador: IdJugador, id_mapa: u16, id_grupo: u16) -> Result<DeltaPartida, String> {
    let batalla = batalla_de(est, id_mapa)?;
    let grupo = batalla.grupos.iter().find(|g| g.id == id_grupo).ok_or("Ese grupo ya no existe")?;
    if jugador != ID_HOST {
        match grupo.turno {
            Some(t) if controla_instancia(est, jugador, t) => {}
            _ => return Err("Solo puede pasar el turno quien controla la ficha que lo tiene".into()),
        }
    }
    delta_de_pasar_turno(est, id_mapa, id_grupo)
}

pub fn pasar_turno_en(state: &AppState, jugador: IdJugador, id_mapa: u16, id_grupo: u16) -> Result<(), String> {
    let mut est = state.lock();
    let delta = pasar_turno_de(&est, jugador, id_mapa, id_grupo)?;
    registrar(&mut est, &state.almacen, delta);
    Ok(())
}

// ------------------------------------------------------------------
// Saneado (se aplica dentro de `aplicar_delta`, así el log siempre da lo mismo)
// ------------------------------------------------------------------

/// Deja cada batalla coherente con las instancias: un miembro que ya no existe o ya no está en ese mapa sale; si tenía el turno, pasa
/// al siguiente (sin sumar vuelta). Un grupo sin miembros se queda sin turno. Una instancia OCULTA no entra sola en la batalla, pero
/// el host puede meterla en un grupo y seguirá en él; cuando le llegue el turno (o se lo encuentre por quedarse el grupo sin
/// titular) se activa: deja de estar oculta.
pub fn sanear(est: &mut EstadoPartida) {
    let mut reveladas: Vec<u16> = Vec::new();
    for m in 0..est.mapas.len() {
        let Some(batalla) = est.mapas[m].batalla.clone() else { continue };
        let id_mapa = est.mapas[m].id;
        let vale = |est: &EstadoPartida, id: u16| est.instancias_activas.iter().any(|i| i.id_instancia == id && i.id_mapa == Some(id_mapa));
        let mut ya_en_un_grupo: Vec<u16> = Vec::new();
        let mut nueva = batalla.clone();
        for g in nueva.grupos.iter_mut() {
            let antes = g.clone();
            g.miembros.retain(|id| {
                let ok = vale(est, *id) && !ya_en_un_grupo.contains(id);
                if ok {
                    ya_en_un_grupo.push(*id);
                }
                ok
            });
            if g.turno.is_some_and(|t| !g.miembros.contains(&t)) {
                // Si el titular sigue existiendo (se ocultó o se fue de mapa) el turno pasa al siguiente de su sitio en la lista;
                // si ya no existe, al primero
                let orden_titular = antes.turno.and_then(|t| est.instancias_activas.iter().find(|i| i.id_instancia == t)).map(|i| i.orden);
                let ordenados = orden_de_turno(est, g);
                let siguiente = match orden_titular {
                    Some(o) => ordenados
                        .iter()
                        .copied()
                        .find(|id| est.instancias_activas.iter().any(|i| i.id_instancia == *id && i.orden > o))
                        .or(ordenados.first().copied()),
                    None => ordenados.first().copied(),
                };
                g.turno = siguiente;
            }
            if g.turno.is_none() {
                g.turno = orden_de_turno(est, g).first().copied();
            }
            if g.turno != antes.turno {
                reveladas.extend(g.turno); // el turno llegó a alguien nuevo: si estaba oculto, se activa
            }
        }
        if nueva != batalla {
            est.mapas[m].batalla = Some(nueva);
        }
    }
    activar(est, &reveladas);
}

/// Las instancias que ya tienen el turno dejan de estar ocultas (aparecen para los jugadores).
pub fn activar(est: &mut EstadoPartida, ids: &[u16]) {
    for i in est.instancias_activas.iter_mut().filter(|i| i.oculto && ids.contains(&i.id_instancia)) {
        i.oculto = false;
    }
}

/// Una instancia acaba de llegar (o dejó de estar oculta) a un mapa en batalla: si solo hay un grupo, entra en él; con varios se
/// queda en «No participa». `antes_visible_aqui`: ya estaba en ese mapa y a la vista (moverla dentro del mapa no la mete).
pub fn al_llegar(est: &mut EstadoPartida, id_instancia: u16, antes_visible_aqui: bool) {
    if antes_visible_aqui {
        return;
    }
    let Some(inst) = est.instancias_activas.iter().find(|i| i.id_instancia == id_instancia) else { return };
    let (Some(id_mapa), false) = (inst.id_mapa, inst.oculto) else { return };
    let Some(m) = est.mapas.iter().position(|m| m.id == id_mapa) else { return };
    let Some(batalla) = &est.mapas[m].batalla else { return };
    if batalla.grupos.len() != 1 || batalla.grupos[0].miembros.contains(&id_instancia) {
        return;
    }
    let mut nueva = batalla.clone();
    nueva.grupos[0].miembros.push(id_instancia);
    if nueva.grupos[0].turno.is_none() {
        nueva.grupos[0].turno = orden_de_turno(est, &nueva.grupos[0]).first().copied();
    }
    est.mapas[m].batalla = Some(nueva);
}

// ------------------------------------------------------------------
// Empezar y terminar
// ------------------------------------------------------------------

/// Lo que vale en la iniciativa de una ficha un término de letras: una variable SUYA, exactamente como en una tirada del chat
/// (`resolver_variable`, pero de cada ficha en vez de la que tiene el foco). Un parámetro visual no se tira: si depende de una
/// variable, se tira esa variable.
fn valor_de_termino(est: &EstadoPartida, id_instancia: u16, texto: &str) -> Result<(String, i32), String> {
    let inst = est.instancias_activas.iter().find(|i| i.id_instancia == id_instancia).ok_or("Esa ficha ya no está activa")?;
    let def = variables::buscar(&est.definiciones_variables, texto).ok_or_else(|| format!("«{}» no es una variable", texto.trim()))?;
    let nombre = if def.abreviatura.is_empty() { &def.nombre } else { &def.abreviatura };
    Ok((nombre.clone(), variables::resultado(&est.definiciones_variables, &inst.base.parametros, &inst.base.variables, def)))
}

/// Los ids de todas las instancias en su orden (el de «Fichas activas»).
fn ids_en_orden(est: &EstadoPartida) -> Vec<u16> {
    let mut todas: Vec<(u16, u16)> = est.instancias_activas.iter().map(|i| (i.orden, i.id_instancia)).collect();
    todas.sort_unstable();
    todas.into_iter().map(|(_, id)| id).collect()
}

/// Reparte los huecos que ocupaban los `miembros` en `ids` entre esos mismos miembros, ya en el orden `nuevo_orden` (el resto de
/// las instancias no se mueve de sitio).
fn recolocar(ids: &mut [u16], miembros: &[u16], nuevo_orden: &[u16]) {
    let mut nuevos = nuevo_orden.iter();
    for id in ids.iter_mut() {
        if miembros.contains(id) {
            if let Some(n) = nuevos.next() {
                *id = *n;
            }
        }
    }
}

/// Un grupo que pide la ventana de «Empezar batalla» (solo ahí se organizan los grupos: nunca por el chat).
#[derive(Debug, Clone, Deserialize)]
pub struct GrupoInicial {
    pub nombre: String,
    pub miembros: Vec<u16>,
}

pub const MAX_GRUPOS_BATALLA: usize = 10;
pub const MAX_NOMBRE_GRUPO_BATALLA: usize = 30;

/// Un nombre de grupo: 1-30 caracteres sin control ni invisibles, y distinto (sin mirar mayúsculas) de los `otros`.
pub fn validar_nombre_grupo(nombre: &str, otros: &[String]) -> Result<String, String> {
    let n = nombre.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() {
        return Err("El grupo necesita un nombre".into());
    }
    if n.chars().count() > MAX_NOMBRE_GRUPO_BATALLA {
        return Err(format!("El nombre del grupo no puede pasar de {MAX_NOMBRE_GRUPO_BATALLA} caracteres"));
    }
    if n.chars().any(|c| c.is_control() || crate::tablero_general::jugadores::caracter_enganoso(c)) {
        return Err("El nombre del grupo tiene caracteres no permitidos".into());
    }
    if otros.iter().any(|o| o.trim().to_lowercase() == n.to_lowercase()) {
        return Err(format!("Ya hay un grupo que se llama «{n}»"));
    }
    Ok(n)
}

/// `Batalla [tirada]` en el mapa que tiene abierto el host (un solo grupo con todos). `comando` es lo escrito (para el historial) y
/// `tirada` lo que sigue a la palabra. Devuelve la línea que se ve en el chat.
pub fn empezar_en(state: &AppState, comando: &str, tirada: &str) -> Result<String, String> {
    empezar_con_grupos(state, comando, tirada, None)
}

/// Como `empezar_en`, y con `grupos` (de la ventana) la batalla se reparte en ellos: cada uno con su turno y su contador, y cada
/// uno se ordena por su cuenta con la tirada. Las fichas que no estén en ningún grupo no participan. El chat solo recibe la orden.
pub fn empezar_con_grupos(state: &AppState, comando: &str, tirada: &str, grupos: Option<Vec<GrupoInicial>>) -> Result<String, String> {
    let mut est = state.lock();
    let id_mapa = est.mapa_del_host.abierto.ok_or("Abre primero el mapa donde será la batalla")?;
    if est.mapas.iter().find(|m| m.id == id_mapa).ok_or("Ese mapa no existe")?.batalla.is_some() {
        return Err("Ese mapa ya está en batalla: termínala antes con «Terminar»".into());
    }
    // Las que pueden participar: las del mapa que no están ocultas, en su orden
    let en_orden = ids_en_orden(&est);
    let a_la_vista: Vec<u16> = en_orden
        .iter()
        .copied()
        .filter(|id| est.instancias_activas.iter().any(|i| i.id_instancia == *id && i.id_mapa == Some(id_mapa) && !i.oculto))
        .collect();
    // Los grupos pedidos (o uno solo con todas)
    let pedidos: Vec<(String, Vec<u16>)> = match grupos {
        None => vec![(NOMBRE_GRUPO_UNICO.to_string(), a_la_vista.clone())],
        Some(gs) => {
            if gs.is_empty() || gs.len() > MAX_GRUPOS_BATALLA {
                return Err(format!("Una batalla lleva de 1 a {MAX_GRUPOS_BATALLA} grupos"));
            }
            let mut nombres: Vec<String> = Vec::new();
            let mut vistos: Vec<u16> = Vec::new();
            let mut out = Vec::new();
            for g in gs {
                let nombre = validar_nombre_grupo(&g.nombre, &nombres)?;
                nombres.push(nombre.clone());
                for id in &g.miembros {
                    if !a_la_vista.contains(id) {
                        return Err("Una de las fichas del grupo ya no está a la vista en este mapa".into());
                    }
                    if vistos.contains(id) {
                        return Err("Una ficha no puede estar en dos grupos".into());
                    }
                    vistos.push(*id);
                }
                // Dentro de cada grupo, en el orden de «Fichas activas»
                let mut miembros = g.miembros;
                miembros.sort_by_key(|id| en_orden.iter().position(|x| x == id));
                out.push((nombre, miembros));
            }
            out
        }
    };
    let participantes: Vec<u16> = pedidos.iter().flat_map(|(_, m)| m.iter().copied()).collect();
    if participantes.is_empty() {
        return Err("No hay fichas a la vista en este mapa".into());
    }

    // La tirada de cada ficha (si la hay) y, con ella, el orden de cada grupo: de mayor a menor, en un empate como estaban
    let nombre_de = |est: &EstadoPartida, id: u16| est.instancias_activas.iter().find(|i| i.id_instancia == id).map(|i| i.base.nombre.clone()).unwrap_or_default();
    let mut grupos_ordenados: Vec<(String, Vec<u16>)> = pedidos.clone();
    let mut resumen: Vec<String> = Vec::new();
    let mut ids_nuevo_orden = en_orden.clone();
    if !tirada.trim().is_empty() {
        if dividir_comparacion(tirada)?.1.is_some() {
            return Err("La tirada de iniciativa no admite comparaciones".into());
        }
        let mut totales: Vec<(u16, i32)> = Vec::new();
        for &id in &participantes {
            let mut resolver = |texto: &str| valor_de_termino(&est, id, texto);
            totales.push((id, tirar_lado(&parsear_comando(tirada, &mut resolver)?)?.total));
        }
        let total_de = |id: u16| totales.iter().find(|(i, _)| *i == id).map_or(0, |(_, t)| *t);
        for (_, miembros) in grupos_ordenados.iter_mut() {
            let antes = miembros.clone();
            miembros.sort_by(|a, b| total_de(*b).cmp(&total_de(*a))); // estable: el empate respeta el orden anterior
            recolocar(&mut ids_nuevo_orden, &antes, miembros);
            resumen.extend(miembros.iter().map(|id| format!("{} {}", nombre_de(&est, *id), total_de(*id))));
        }
    } else {
        resumen = participantes.iter().map(|id| nombre_de(&est, *id)).collect();
    }

    let batalla = Batalla {
        siguiente_id: grupos_ordenados.len() as u16 + 1,
        grupos: grupos_ordenados
            .into_iter()
            .enumerate()
            .map(|(i, (nombre, miembros))| GrupoBatalla { id: i as u16 + 1, nombre, turno: miembros.first().copied(), miembros, ronda: 1 })
            .collect(),
    };
    if !tirada.trim().is_empty() {
        registrar(&mut est, &state.almacen, DeltaPartida::InstanciasReordenadas(ids_nuevo_orden));
    }
    registrar(&mut est, &state.almacen, DeltaPartida::BatallaIniciada { id_mapa, batalla });
    // En el chat sale el comando equivalente tal cual (`Batalla 1d20 + velocidad`) y, entre paréntesis, el resultado: las fichas (con su tirada si la hubo)
    let linea = format!("{comando} ({})", resumen.join(" · "));
    registrar_tirada_en(&mut est, &state.almacen, ID_HOST, &linea, participantes.len() as i32, true, None, true);
    drop(est);
    state.almacen.anadir_historial(comando);
    Ok(linea)
}

// ------------------------------------------------------------------
// Grupos (solo el host, desde «Fichas activas»; nunca por el chat)
// ------------------------------------------------------------------

/// Cambia los grupos de la batalla del mapa con `cambio` (que recibe una copia) y lo registra entero. `cambio` devuelve error si lo
/// pedido no vale.
fn cambiar_grupos(state: &AppState, id_mapa: u16, cambio: impl FnOnce(&EstadoPartida, &mut Batalla) -> Result<(), String>) -> Result<(), String> {
    let mut est = state.lock();
    let mut batalla = batalla_de(&est, id_mapa)?.clone();
    cambio(&est, &mut batalla)?;
    if batalla == *batalla_de(&est, id_mapa)? {
        return Ok(());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::GruposBatallaCambiados { id_mapa, grupos: batalla.grupos, siguiente_id: batalla.siguiente_id });
    Ok(())
}

/// Un grupo nuevo, vacío. Sin nombre (`None`) es «Grupo N».
pub fn crear_grupo_en(state: &AppState, id_mapa: u16, nombre: Option<String>) -> Result<(), String> {
    cambiar_grupos(state, id_mapa, |_, b| {
        if b.grupos.len() >= MAX_GRUPOS_BATALLA {
            return Err(format!("No puede haber más de {MAX_GRUPOS_BATALLA} grupos"));
        }
        let nombres: Vec<String> = b.grupos.iter().map(|g| g.nombre.clone()).collect();
        let nombre = match nombre.filter(|n| !n.trim().is_empty()) {
            Some(n) => validar_nombre_grupo(&n, &nombres)?,
            None => {
                let mut n = b.grupos.len() + 1;
                while nombres.iter().any(|x| x.to_lowercase() == format!("grupo {n}")) {
                    n += 1;
                }
                format!("Grupo {n}")
            }
        };
        let id = b.siguiente_id.max(b.grupos.iter().map(|g| g.id).max().unwrap_or(0) + 1);
        b.grupos.push(GrupoBatalla { id, nombre, miembros: vec![], turno: None, ronda: 1 });
        b.siguiente_id = id + 1;
        Ok(())
    })
}

pub fn renombrar_grupo_en(state: &AppState, id_mapa: u16, id_grupo: u16, nombre: &str) -> Result<(), String> {
    cambiar_grupos(state, id_mapa, |_, b| {
        let otros: Vec<String> = b.grupos.iter().filter(|g| g.id != id_grupo).map(|g| g.nombre.clone()).collect();
        let g = b.grupos.iter_mut().find(|g| g.id == id_grupo).ok_or("Ese grupo ya no existe")?;
        g.nombre = validar_nombre_grupo(nombre, &otros)?;
        Ok(())
    })
}

/// Borra un grupo: sus fichas dejan de participar. La batalla se queda siempre con al menos un grupo (para acabarla está «Terminar»).
pub fn borrar_grupo_en(state: &AppState, id_mapa: u16, id_grupo: u16) -> Result<(), String> {
    cambiar_grupos(state, id_mapa, |_, b| {
        if b.grupos.len() <= 1 {
            return Err("La batalla necesita al menos un grupo".into());
        }
        let antes = b.grupos.len();
        b.grupos.retain(|g| g.id != id_grupo);
        if b.grupos.len() == antes {
            return Err("Ese grupo ya no existe".into());
        }
        Ok(())
    })
}

/// Pasa instancias a un grupo (`id_grupo` 0 = «No participa»). Solo valen las que están en el mapa de la batalla (también las ocultas). El grupo
/// que las recibe conserva SU turno; si el que tenía el turno se va, pasa al siguiente (lo hace `sanear`).
pub fn mover_a_grupo_en(state: &AppState, id_mapa: u16, ids: &[u16], id_grupo: u16) -> Result<(), String> {
    if ids.len() > 200 {
        return Err("Demasiadas fichas a la vez".into());
    }
    cambiar_grupos(state, id_mapa, |est, b| {
        if (1..ids.len()).any(|i| ids[..i].contains(&ids[i])) {
            return Err("Una ficha no puede ir dos veces".into());
        }
        for id in ids {
            // Una oculta sí puede ir a un grupo (el host la arrastra): se activará cuando le toque el turno
            if !est.instancias_activas.iter().any(|i| i.id_instancia == *id && i.id_mapa == Some(id_mapa)) {
                return Err("Una de las fichas no está en este mapa".into());
            }
        }
        if id_grupo != 0 && !b.grupos.iter().any(|g| g.id == id_grupo) {
            return Err("Ese grupo ya no existe".into());
        }
        for g in b.grupos.iter_mut() {
            g.miembros.retain(|m| !ids.contains(m));
        }
        if let Some(g) = b.grupos.iter_mut().find(|g| g.id == id_grupo) {
            g.miembros.extend(ids.iter().copied());
        }
        Ok(())
    })
}

/// Mete el grupo `origen` entero en `destino`: sus fichas pasan a él (que conserva su turno y su contador) y `origen` desaparece.
pub fn fusionar_grupos_en(state: &AppState, id_mapa: u16, origen: u16, destino: u16) -> Result<(), String> {
    cambiar_grupos(state, id_mapa, |_, b| {
        if origen == destino {
            return Ok(());
        }
        let desde = b.grupos.iter().find(|g| g.id == origen).ok_or("Ese grupo ya no existe")?.miembros.clone();
        let a = b.grupos.iter_mut().find(|g| g.id == destino).ok_or("Ese grupo ya no existe")?;
        a.miembros.extend(desde);
        b.grupos.retain(|g| g.id != origen);
        Ok(())
    })
}

/// `Terminar`: acaba la batalla del mapa abierto (todos sus grupos) y se vuelve al modo normal.
pub fn terminar_en(state: &AppState, comando: &str) -> Result<String, String> {
    let mut est = state.lock();
    let id_mapa = est.mapa_del_host.abierto.ok_or("No hay ningún mapa abierto")?;
    let batalla = batalla_de(&est, id_mapa).map_err(|_| "El mapa abierto no está en batalla".to_string())?;
    let vueltas = batalla.grupos.iter().map(|g| g.ronda).max().unwrap_or(1);
    registrar(&mut est, &state.almacen, DeltaPartida::BatallaTerminada { id_mapa });
    let linea = format!("{comando} (Batalla terminada en {vueltas} {})", if vueltas == 1 { "turno" } else { "turnos" });
    registrar_tirada_en(&mut est, &state.almacen, ID_HOST, &linea, vueltas as i32, true, None, true);
    drop(est);
    state.almacen.anadir_historial(comando);
    Ok(linea)
}

/// Resuelve una orden de batalla escrita en el chat. Solo el host.
pub fn ejecutar_comando(state: &AppState, jugador: IdJugador, comando: &str, orden: ComandoBatalla) -> Result<String, String> {
    if jugador != ID_HOST {
        return Err("Solo el host puede empezar o terminar una batalla".into());
    }
    match orden {
        ComandoBatalla::Empezar(tirada) => empezar_en(state, comando.trim(), &tirada),
        ComandoBatalla::Terminar(resto) if resto.is_empty() => terminar_en(state, comando.trim()),
        ComandoBatalla::Terminar(_) => Err("«Terminar» no lleva nada más".into()),
    }
}

/// Empieza la batalla desde la ventana: el comando equivalente (`Batalla 1d20 + des`) y, si se organizó por grupos, el reparto (que
/// no pasa por el chat).
#[tauri::command]
pub fn empezar_batalla(comando: String, grupos: Option<Vec<GrupoInicial>>, state: tauri::State<'_, AppState>) -> Result<String, String> {
    match interpretar_comando(&comando) {
        Some(ComandoBatalla::Empezar(tirada)) => empezar_con_grupos(&state, comando.trim(), &tirada, grupos),
        _ => Err("No es la orden de empezar una batalla".into()),
    }
}

#[tauri::command]
pub fn crear_grupo_batalla(id_mapa: u16, nombre: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    crear_grupo_en(&state, id_mapa, nombre)
}

#[tauri::command]
pub fn renombrar_grupo_batalla(id_mapa: u16, id_grupo: u16, nombre: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    renombrar_grupo_en(&state, id_mapa, id_grupo, &nombre)
}

#[tauri::command]
pub fn borrar_grupo_batalla(id_mapa: u16, id_grupo: u16, state: tauri::State<'_, AppState>) -> Result<(), String> {
    borrar_grupo_en(&state, id_mapa, id_grupo)
}

#[tauri::command]
pub fn mover_a_grupo_batalla(id_mapa: u16, ids: Vec<u16>, id_grupo: u16, state: tauri::State<'_, AppState>) -> Result<(), String> {
    mover_a_grupo_en(&state, id_mapa, &ids, id_grupo)
}

#[tauri::command]
pub fn fusionar_grupos_batalla(id_mapa: u16, origen: u16, destino: u16, state: tauri::State<'_, AppState>) -> Result<(), String> {
    fusionar_grupos_en(&state, id_mapa, origen, destino)
}

#[tauri::command]
pub fn terminar_batalla(state: tauri::State<'_, AppState>) -> Result<String, String> {
    terminar_en(&state, "Terminar")
}

/// Pasa el turno del grupo (el host siempre puede; un jugador lo pide por la red con la misma acción `pasar_turno`).
#[tauri::command]
pub fn pasar_turno(id_mapa: u16, id_grupo: u16, state: tauri::State<'_, AppState>) -> Result<(), String> {
    pasar_turno_en(&state, ID_HOST, id_mapa, id_grupo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tablero_general::estado::EstadoPartida;
    use crate::tablero_general::modelos::*;

    #[test]
    fn interpreta_empezar_y_terminar() {
        assert_eq!(interpretar_comando("Batalla"), Some(ComandoBatalla::Empezar(String::new())));
        assert_eq!(interpretar_comando("  batalla 1d20 + des "), Some(ComandoBatalla::Empezar("1d20 + des".into())));
        assert_eq!(interpretar_comando("BATALLA\t1d20"), Some(ComandoBatalla::Empezar("1d20".into())));
        assert_eq!(interpretar_comando("terminar"), Some(ComandoBatalla::Terminar(String::new())));
        assert_eq!(interpretar_comando("Terminar ya"), Some(ComandoBatalla::Terminar("ya".into())));
        assert_eq!(interpretar_comando("batallaya"), None, "palabra entera");
        assert_eq!(interpretar_comando("1d20"), None);
        assert_eq!(interpretar_comando(""), None);
    }

    fn inst(id: u16, orden: u16, mapa: Option<u16>) -> Instancia {
        let mut i = crate::tablero_general::estado::tests::instancia(id, 10);
        i.orden = orden;
        i.id_mapa = mapa;
        i
    }

    fn estado(miembros: &[u16], turno: Option<u16>) -> EstadoPartida {
        let mut e = EstadoPartida::default();
        e.mapas.push(crate::tablero_general::estado::tests::mapa(1, 1));
        for (i, id) in [1u16, 2, 3, 4].iter().enumerate() {
            e.instancias_activas.push(inst(*id, i as u16, Some(1)));
        }
        e.mapas[0].batalla = Some(Batalla {
            grupos: vec![GrupoBatalla { id: 1, nombre: "B".into(), miembros: miembros.to_vec(), turno, ronda: 1 }],
            siguiente_id: 2,
        });
        e
    }

    fn grupo(e: &EstadoPartida) -> &GrupoBatalla {
        &e.mapas[0].batalla.as_ref().unwrap().grupos[0]
    }

    #[test]
    fn el_turno_da_la_vuelta_y_cuenta_las_vueltas() {
        let e = estado(&[1, 2, 3], Some(1));
        let g = grupo(&e);
        assert_eq!(siguiente_turno(&e, g), Some((2, 1)));
        let mut e = e;
        e.aplicar_delta(&DeltaPartida::TurnoPasado { id_mapa: 1, id_grupo: 1, turno: 3, ronda: 1 });
        assert_eq!(siguiente_turno(&e, grupo(&e)), Some((1, 2)), "tras el último vuelve al primero y sube la vuelta");
        // Un solo miembro: cada paso es una vuelta
        let solo = estado(&[2], Some(2));
        assert_eq!(siguiente_turno(&solo, grupo(&solo)), Some((2, 2)));
        // Sin turno todavía: el primero, vuelta 1
        let sin = estado(&[3, 1], None);
        assert_eq!(siguiente_turno(&sin, grupo(&sin)), Some((1, 1)));
        // Sin nadie
        let vacio = estado(&[], None);
        assert_eq!(siguiente_turno(&vacio, grupo(&vacio)), None);
    }

    #[test]
    fn el_orden_de_los_turnos_sigue_al_orden_de_fichas_activas() {
        let mut e = estado(&[1, 2, 3], Some(1));
        assert_eq!(orden_de_turno(&e, grupo(&e)), vec![1, 2, 3]);
        // El host reordena a mano: 3, 1, 2
        e.aplicar_delta(&DeltaPartida::InstanciasReordenadas(vec![3, 1, 2, 4]));
        assert_eq!(orden_de_turno(&e, grupo(&e)), vec![3, 1, 2]);
        assert_eq!(siguiente_turno(&e, grupo(&e)), Some((2, 1)), "el turno sigue siendo de la 1; la siguiente es la 2");
    }

    #[test]
    fn pasar_el_turno_lo_decide_quien_controla_la_ficha_que_lo_tiene() {
        let mut e = estado(&[1, 2], Some(1));
        e.instancias_activas[0].base.propietarios = vec![7];
        assert!(pasar_turno_de(&e, ID_HOST, 1, 1).is_ok());
        assert!(pasar_turno_de(&e, 7, 1, 1).is_ok(), "su dueño");
        assert!(pasar_turno_de(&e, 8, 1, 1).is_err(), "otro jugador");
        e.instancias_activas[0].oculto = true;
        assert!(pasar_turno_de(&e, 7, 1, 1).is_err(), "una oculta no se controla");
        assert!(pasar_turno_de(&e, ID_HOST, 1, 9).is_err(), "grupo inexistente");
        assert!(pasar_turno_de(&e, ID_HOST, 5, 1).is_err(), "mapa inexistente");
    }

    #[test]
    fn el_saneado_quita_a_quien_sale_y_pasa_el_turno_sin_sumar_vuelta() {
        let mut e = estado(&[1, 2, 3], Some(2));
        // El que tiene el turno (2) se va a otro mapa: pasa a la 3, misma vuelta
        let mut i2 = e.instancias_activas[1].clone();
        i2.id_mapa = Some(5);
        e.aplicar_delta(&DeltaPartida::InstanciaModificada(i2));
        assert_eq!(grupo(&e).miembros, vec![1, 3]);
        assert_eq!((grupo(&e).turno, grupo(&e).ronda), (Some(3), 1));
        // Una oculta SE QUEDA en el grupo (el host puede meterla); solo se activa cuando le llega el turno
        let mut i3 = e.instancias_activas[2].clone();
        i3.oculto = true;
        e.aplicar_delta(&DeltaPartida::InstanciaModificada(i3));
        assert_eq!(grupo(&e).miembros, vec![1, 3]);
        // La 1 se borra: el turno (que era de la 3, oculta) se queda donde estaba
        e.aplicar_delta(&DeltaPartida::InstanciaBorrada(1));
        assert_eq!(grupo(&e).miembros, vec![3]);
        // La 3 se borra: el grupo se queda sin nadie ni turno
        e.aplicar_delta(&DeltaPartida::InstanciaBorrada(3));
        assert!(grupo(&e).miembros.is_empty());
        assert_eq!(grupo(&e).turno, None);
    }

    #[test]
    fn una_instancia_que_llega_entra_si_solo_hay_un_grupo() {
        let mut e = estado(&[1, 2], Some(1));
        // La 3 ya estaba en el mapa pero no participaba: moverla dentro del mapa no la mete
        let mut i3 = e.instancias_activas[2].clone();
        i3.coordenadas = Coordenada::Casilla(Casilla { x: 4, y: 4 });
        e.aplicar_delta(&DeltaPartida::InstanciaModificada(i3));
        assert_eq!(grupo(&e).miembros, vec![1, 2]);
        // Una nueva sí entra
        e.aplicar_delta(&DeltaPartida::InstanciaCreada(inst(9, 9, Some(1))));
        assert_eq!(grupo(&e).miembros, vec![1, 2, 9]);
        // Una creada oculta, o en otro mapa, no
        let mut oculta = inst(10, 10, Some(1));
        oculta.oculto = true;
        e.aplicar_delta(&DeltaPartida::InstanciaCreada(oculta));
        e.aplicar_delta(&DeltaPartida::InstanciaCreada(inst(11, 11, Some(2))));
        assert_eq!(grupo(&e).miembros, vec![1, 2, 9]);
        // Dejar de estar oculta es llegar
        let mut visible = e.instancias_activas.iter().find(|i| i.id_instancia == 10).unwrap().clone();
        visible.oculto = false;
        e.aplicar_delta(&DeltaPartida::InstanciaModificada(visible));
        assert_eq!(grupo(&e).miembros, vec![1, 2, 9, 10]);
    }

    #[test]
    fn con_varios_grupos_la_que_llega_no_participa() {
        let mut e = estado(&[1], Some(1));
        e.mapas[0].batalla.as_mut().unwrap().grupos.push(GrupoBatalla { id: 2, nombre: "Otro".into(), miembros: vec![2], turno: Some(2), ronda: 1 });
        e.aplicar_delta(&DeltaPartida::InstanciaCreada(inst(9, 9, Some(1))));
        let b = e.mapas[0].batalla.as_ref().unwrap();
        assert!(b.grupos.iter().all(|g| !g.miembros.contains(&9)));
    }

    #[test]
    fn terminar_quita_la_batalla_y_no_hace_falta_nada_mas() {
        let mut e = estado(&[1, 2], Some(1));
        e.aplicar_delta(&DeltaPartida::BatallaTerminada { id_mapa: 1 });
        assert!(e.mapas[0].batalla.is_none());
    }

    #[test]
    fn recolocar_reparte_los_huecos_de_los_miembros() {
        // Participan 1, 3 y 4 (la 2 no): salen 4, 1, 3 por los huecos de 1, 3 y 4
        let mut ids = vec![1u16, 2, 3, 4];
        recolocar(&mut ids, &[1, 3, 4], &[4, 1, 3]);
        assert_eq!(ids, vec![4, 2, 1, 3]);
    }

    // ---- Empezar y terminar con un estado de verdad ----

    use crate::tablero_general::almacen::{pruebas::dir_temporal, Almacen};

    /// Cuatro instancias en el mapa 1 (la 3 oculta) y una en otro mapa; el host tiene abierto el 1. Velocidad 10 en todas.
    fn partida(nombre: &str) -> AppState {
        let raiz = dir_temporal(nombre);
        std::fs::create_dir_all(&raiz).unwrap();
        let almacen = Almacen::abrir(raiz.join("p")).unwrap();
        let mut e = EstadoPartida::default();
        e.mapas.push(crate::tablero_general::estado::tests::mapa(1, 1));
        e.mapas.push(crate::tablero_general::estado::tests::mapa(2, 1));
        for (id, mapa, oculto) in [(1u16, 1u16, false), (2, 1, false), (3, 1, true), (4, 1, false), (5, 2, false)] {
            let mut i = inst(id, id - 1, Some(mapa));
            i.oculto = oculto;
            i.base.nombre = format!("F{id}");
            e.instancias_activas.push(i);
        }
        e.mapa_del_host.abierto = Some(1);
        AppState::nuevo(e, almacen, raiz)
    }

    fn miembros(state: &AppState) -> Vec<u16> {
        state.lock().mapas[0].batalla.as_ref().unwrap().grupos[0].miembros.clone()
    }

    #[test]
    fn empezar_sin_tirada_no_reordena_y_deja_el_turno_en_el_primero() {
        let s = partida("batalla_sin_tirada");
        let linea = ejecutar_comando(&s, ID_HOST, "Batalla", ComandoBatalla::Empezar(String::new())).unwrap();
        assert_eq!(linea, "Batalla (F1 · F2 · F4)");
        assert_eq!(miembros(&s), vec![1, 2, 4], "la oculta no participa; la de otro mapa tampoco");
        let est = s.lock();
        let g = &est.mapas[0].batalla.as_ref().unwrap().grupos[0];
        assert_eq!((g.turno, g.ronda), (Some(1), 1));
        assert!(est.mapas[1].batalla.is_none());
        // Una línea en el chat
        assert!(est.historial_tiradas[0].informativa);
        drop(est);
        // Empezar otra vez en el mismo mapa no se puede
        assert!(empezar_en(&s, "Batalla", "").is_err());
    }

    #[test]
    fn empezar_con_tirada_ordena_de_mayor_a_menor_con_las_variables_de_cada_ficha() {
        use crate::tablero_general::variables::pruebas::var;
        let s = partida("batalla_con_tirada");
        {
            let mut est = s.lock();
            // Destreza (neutro 0, escala 1: su resultado es su valor) = 6 + id: la 1 tiene 7, la 2 tiene 8, la 4 tiene 10; un dado de
            // 1 cara no varía, así que el resultado es seguro
            est.definiciones_variables = vec![var(1, "Destreza", "des", 0, 1)];
            for i in est.instancias_activas.iter_mut() {
                i.base.variables = vec![ValorVariable { id_definicion: 1, valor: 6 + i.id_instancia as i16, opcionales: vec![], eleccion: 0, parametros_marcados: vec![] }];
            }
        }
        let linea = empezar_en(&s, "Batalla 1d1 + des", "1d1 + des").unwrap();
        assert_eq!(linea, "Batalla 1d1 + des (F4 11 · F2 9 · F1 8)");
        assert_eq!(miembros(&s), vec![4, 2, 1], "de mayor a menor");
        let est = s.lock();
        assert_eq!(est.mapas[0].batalla.as_ref().unwrap().grupos[0].turno, Some(4));
        // «Fichas activas» se reordenó: 4, 2, 1 por los huecos de 1, 2 y 4 (la oculta 3 y la de otro mapa siguen donde estaban)
        let mut ordenadas: Vec<(u16, u16)> = est.instancias_activas.iter().map(|i| (i.orden, i.id_instancia)).collect();
        ordenadas.sort_unstable();
        assert_eq!(ordenadas.iter().map(|(_, id)| *id).collect::<Vec<_>>(), vec![4, 2, 3, 1, 5]);
    }

    #[test]
    fn un_parametro_visual_no_se_tira_ni_en_la_iniciativa() {
        let s = partida("batalla_parametro");
        s.lock().definiciones_parametros = crate::tablero_general::modelos::parametros_por_defecto();
        let e = empezar_en(&s, "Batalla Velocidad", "Velocidad").unwrap_err();
        assert!(e.contains("no es una variable"), "{e}");
        assert!(s.lock().mapas[0].batalla.is_none());
    }

    #[test]
    fn en_un_empate_se_respeta_el_orden_que_ya_tenian() {
        let s = partida("batalla_empate");
        empezar_en(&s, "Batalla 1d1", "1d1").unwrap();
        assert_eq!(miembros(&s), vec![1, 2, 4]);
    }

    #[test]
    fn empezar_rechaza_lo_que_no_vale() {
        let s = partida("batalla_errores");
        assert!(empezar_en(&s, "Batalla 1d20 <= 5", "1d20 <= 5").unwrap_err().contains("comparaciones"));
        assert!(empezar_en(&s, "Batalla Inventada", "Inventada").is_err(), "no es una variable");
        assert!(s.lock().mapas[0].batalla.is_none(), "un error no deja nada a medias");
        s.lock().mapa_del_host.abierto = None;
        assert!(empezar_en(&s, "Batalla", "").is_err(), "sin mapa abierto");
        // Un jugador no puede
        let e = ejecutar_comando(&s, 3, "Batalla", ComandoBatalla::Empezar(String::new())).unwrap_err();
        assert!(e.contains("Solo el host"), "{e}");
    }

    #[test]
    fn terminar_quita_la_batalla_y_avisa_en_el_chat() {
        let s = partida("batalla_terminar");
        assert!(terminar_en(&s, "Terminar").is_err(), "sin batalla");
        empezar_en(&s, "Batalla", "").unwrap();
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap();
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap();
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap(); // vuelve al primero: turno 2
        let linea = ejecutar_comando(&s, ID_HOST, "Terminar", ComandoBatalla::Terminar(String::new())).unwrap();
        assert_eq!(linea, "Terminar (Batalla terminada en 2 turnos)");
        assert!(s.lock().mapas[0].batalla.is_none());
        assert!(ejecutar_comando(&s, ID_HOST, "Terminar ya", ComandoBatalla::Terminar("ya".into())).is_err());
    }
    // ---- Grupos (fase 2) ----

    fn gi(nombre: &str, miembros: &[u16]) -> GrupoInicial {
        GrupoInicial { nombre: nombre.into(), miembros: miembros.to_vec() }
    }

    fn grupos_de(state: &AppState) -> Vec<(u16, Vec<u16>, Option<u16>, u32)> {
        state.lock().mapas[0].batalla.as_ref().unwrap().grupos.iter().map(|g| (g.id, g.miembros.clone(), g.turno, g.ronda)).collect()
    }

    #[test]
    fn empezar_por_grupos_cada_uno_con_su_turno_y_lo_que_no_esta_en_ninguno_no_participa() {
        let s = partida("batalla_grupos");
        // Participan la 1 y la 2 (Héroes) y la 4 (Orcos); no hay quien se quede fuera salvo la oculta y la de otro mapa
        let linea = empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("Héroes", &[2, 1]), gi("Orcos", &[4])])).unwrap();
        assert_eq!(linea, "Batalla (F1 · F2 · F4)", "los grupos no salen en el chat");
        assert_eq!(grupos_de(&s), vec![(1, vec![1, 2], Some(1), 1), (2, vec![4], Some(4), 1)]);
        // Una ficha que no entra en ningún grupo no participa
        let s = partida("batalla_grupos_fuera");
        empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("Solo", &[2])])).unwrap();
        assert_eq!(grupos_de(&s), vec![(1, vec![2], Some(2), 1)]);
    }

    #[test]
    fn cada_grupo_se_ordena_por_su_cuenta_con_la_tirada() {
        use crate::tablero_general::variables::pruebas::var;
        let s = partida("batalla_grupos_tirada");
        {
            let mut est = s.lock();
            est.definiciones_variables = vec![var(1, "Destreza", "des", 0, 1)];
            for i in est.instancias_activas.iter_mut() {
                i.base.variables = vec![ValorVariable { id_definicion: 1, valor: 10 - i.id_instancia as i16, opcionales: vec![], eleccion: 0, parametros_marcados: vec![] }];
            }
        }
        // Des = 10 - id: la 1 tiene 9, la 2 tiene 8, la 4 tiene 6. En «A» (1, 4) y en «B» (2): A queda 1, 4 y B queda 2
        let linea = empezar_con_grupos(&s, "Batalla 1d1 + des", "1d1 + des", Some(vec![gi("A", &[4, 1]), gi("B", &[2])])).unwrap();
        assert_eq!(linea, "Batalla 1d1 + des (F1 10 · F4 7 · F2 9)");
        assert_eq!(grupos_de(&s), vec![(1, vec![1, 4], Some(1), 1), (2, vec![2], Some(2), 1)]);
    }

    #[test]
    fn los_grupos_pedidos_se_validan() {
        let s = partida("batalla_grupos_validar");
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![])).is_err());
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[1]), gi("a", &[2])])).is_err(), "nombre repetido");
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[1]), gi("B", &[1])])).is_err(), "en dos grupos");
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[3])])).is_err(), "una oculta no participa");
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[5])])).is_err(), "una de otro mapa");
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[])])).is_err(), "sin nadie");
        assert!(empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("  ", &[1])])).is_err(), "sin nombre");
        assert!(s.lock().mapas[0].batalla.is_none(), "un error no deja nada a medias");
    }

    #[test]
    fn mover_entre_grupos_y_hacia_no_participa() {
        let s = partida("batalla_mover");
        empezar_en(&s, "Batalla", "").unwrap(); // un grupo con 1, 2 y 4; turno de la 1
        assert!(crear_grupo_en(&s, 1, Some("Batalla".into())).is_err(), "nombre repetido");
        crear_grupo_en(&s, 1, None).unwrap();
        assert_eq!(s.lock().mapas[0].batalla.as_ref().unwrap().grupos[1].nombre, "Grupo 2");
        // La 4 pasa al grupo nuevo: tiene su propio turno (ella) y el primero sigue como estaba
        mover_a_grupo_en(&s, 1, &[4], 2).unwrap();
        assert_eq!(grupos_de(&s), vec![(1, vec![1, 2], Some(1), 1), (2, vec![4], Some(4), 1)]);
        // La que tiene el turno en el primero deja de participar: el turno pasa a la siguiente, misma vuelta
        mover_a_grupo_en(&s, 1, &[1], 0).unwrap();
        assert_eq!(grupos_de(&s), vec![(1, vec![2], Some(2), 1), (2, vec![4], Some(4), 1)]);
        // Una de otro mapa o un grupo que no existe, no (una oculta sí vale: se activa cuando le toca)
        assert!(mover_a_grupo_en(&s, 1, &[5], 1).is_err(), "una de otro mapa");
        assert!(mover_a_grupo_en(&s, 1, &[2], 9).is_err());
        assert!(mover_a_grupo_en(&s, 1, &[2, 2], 2).is_err());
        // Volver a participar: entra con la que tenía el turno intacto
        mover_a_grupo_en(&s, 1, &[1], 1).unwrap();
        assert_eq!(grupos_de(&s)[0], (1, vec![2, 1], Some(2), 1));
    }

    #[test]
    fn fusionar_conserva_el_turno_y_la_vuelta_del_destino() {
        let s = partida("batalla_fusionar");
        empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[1, 2]), gi("B", &[4])])).unwrap();
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap(); // A: turno de la 2
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap(); // A: vuelta 2, turno de la 1
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap(); // A: turno de la 2
        assert_eq!(grupos_de(&s)[0], (1, vec![1, 2], Some(2), 2));
        // B entra en A: A sigue con su turno y su vuelta, no el de la recién llegada
        fusionar_grupos_en(&s, 1, 2, 1).unwrap();
        assert_eq!(grupos_de(&s), vec![(1, vec![1, 2, 4], Some(2), 2)]);
        assert!(fusionar_grupos_en(&s, 1, 2, 1).is_err(), "ya no existe");
        // Cada grupo lleva su contador aparte: pasar el turno de uno no toca al otro
        let s = partida("batalla_contadores");
        empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[1]), gi("B", &[2, 4])])).unwrap();
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap();
        assert_eq!(grupos_de(&s), vec![(1, vec![1], Some(1), 2), (2, vec![2, 4], Some(2), 1)]);
    }

    #[test]
    fn borrar_un_grupo_deja_a_sus_fichas_sin_participar_y_no_se_borra_el_ultimo() {
        let s = partida("batalla_borrar_grupo");
        empezar_con_grupos(&s, "Batalla", "", Some(vec![gi("A", &[1]), gi("B", &[2])])).unwrap();
        borrar_grupo_en(&s, 1, 2).unwrap();
        assert_eq!(grupos_de(&s), vec![(1, vec![1], Some(1), 1)]);
        assert!(borrar_grupo_en(&s, 1, 1).is_err(), "el último, no");
        assert!(borrar_grupo_en(&s, 1, 7).is_err());
        renombrar_grupo_en(&s, 1, 1, "  Los   buenos ").unwrap();
        assert_eq!(s.lock().mapas[0].batalla.as_ref().unwrap().grupos[0].nombre, "Los buenos");
        assert!(renombrar_grupo_en(&s, 1, 1, "").is_err());
    }
    #[test]
    fn una_oculta_metida_en_un_grupo_se_activa_cuando_le_toca() {
        let s = partida("batalla_oculta");
        empezar_en(&s, "Batalla", "").unwrap(); // participan la 1, la 2 y la 4; la 3 está oculta y no entra
        assert_eq!(miembros(&s), vec![1, 2, 4]);
        // El host la mete en el grupo: se queda oculta mientras no le toque
        mover_a_grupo_en(&s, 1, &[3], 1).unwrap();
        assert_eq!(miembros(&s), vec![1, 2, 4, 3]);
        assert!(s.lock().instancias_activas.iter().find(|i| i.id_instancia == 3).unwrap().oculto);
        // Se pasa el turno hasta ella (el orden de turnos es el de «Fichas activas»: 1, 2, 3, 4)
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap(); // 2
        assert!(s.lock().instancias_activas.iter().find(|i| i.id_instancia == 3).unwrap().oculto);
        pasar_turno_en(&s, ID_HOST, 1, 1).unwrap(); // 3: se activa
        let est = s.lock();
        assert_eq!(est.mapas[0].batalla.as_ref().unwrap().grupos[0].turno, Some(3));
        assert!(!est.instancias_activas.iter().find(|i| i.id_instancia == 3).unwrap().oculto, "ya no está oculta");
    }

    #[test]
    fn un_jugador_no_recibe_las_ocultas_de_la_batalla() {
        use crate::tablero_general::visibilidad::mapas_para;
        let mut e = estado(&[1, 2, 3], Some(3));
        e.instancias_activas[2].oculto = true; // la 3, que además tiene el turno
        e.mapas[0].compartido = true;
        let vista = mapas_para(7, &e.mapas, &e.instancias_activas);
        let g = &vista[0].batalla.as_ref().unwrap().grupos[0];
        assert_eq!(g.miembros, vec![1, 2]);
        assert_eq!(g.turno, None);
        // El host lo ve todo
        assert_eq!(mapas_para(ID_HOST, &e.mapas, &e.instancias_activas)[0].batalla.as_ref().unwrap().grupos[0].miembros, vec![1, 2, 3]);
    }
}
