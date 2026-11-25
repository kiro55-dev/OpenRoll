// Pruebas del intérprete (con un mundo en memoria y dados fijos) y de los comandos (con un AppState de verdad).

use super::interprete::{ejecutar, Mundo};
use super::*;
use crate::tablero_general::almacen::pruebas::dir_temporal;
use crate::tablero_general::escalas::Valoracion;
use crate::tablero_general::estado::{self, DeltaPartida, EstadoPartida};
use crate::tablero_general::modelos::*;
use serde_json::json;

// ---------------- Un mundo de prueba ----------------

struct Prueba {
    est: EstadoPartida,
    caras: Vec<i32>,
    chat: Vec<(String, i32, bool)>,
}

impl Mundo for Prueba {
    fn estado(&self) -> &EstadoPartida {
        &self.est
    }
    fn aplicar(&mut self, delta: DeltaPartida) {
        self.est.aplicar_delta(&delta);
    }
    fn tirar(&mut self, caras: i32) -> i32 {
        if self.caras.is_empty() { 1 } else { self.caras.remove(0).min(caras) }
    }
    fn chat(&mut self, _jugador: IdJugador, desglose: &str, total: i32, publica: bool, _v: Option<&Valoracion>, _informativa: bool) {
        self.chat.push((desglose.to_string(), total, publica));
    }
}

fn ficha(id: u16, nombre: &str, propietarios: Vec<IdJugador>) -> Entidad {
    serde_json::from_value(json!({
        "id": id, "nombre": nombre, "tipo": "Enemigo", "zona_id": 1, "cuerpo": "Pequenio", "propietarios": propietarios,
        "parametros": [
            { "id_definicion": ID_VIDA, "valor": { "Numero": 20 }, "publico": true },
            { "id_definicion": ID_ARMADURA, "valor": { "Numero": 12 }, "publico": true },
            { "id_definicion": ID_VELOCIDAD, "valor": { "Numero": 6 }, "publico": true },
            { "id_definicion": ID_RANGO_ARMA, "valor": { "Numero": 2 }, "publico": true }
        ],
        "variables": [ { "id_definicion": 1, "valor": 14 }, { "id_definicion": 2, "valor": 14 } ]
    }))
    .unwrap()
}

fn instancia(id: u16, base: Entidad, x: u16, y: u16) -> Instancia {
    serde_json::from_value(json!({
        "id_instancia": id, "id_mapa": 1, "base": base, "activo": true, "orden": id,
        "valores_actuales": [ { "id_definicion": ID_VIDA, "valor": 20 } ],
        "coordenadas": { "Casilla": { "x": x, "y": y } }
    }))
    .unwrap()
}

fn variable(id: u16, nombre: &str, abreviatura: &str) -> DefinicionVariable {
    serde_json::from_value(json!({ "id": id, "nombre": nombre, "abreviatura": abreviatura, "neutro": 10, "escala": 2, "grupo": 1 })).unwrap()
}

fn partida() -> EstadoPartida {
    let mut est = EstadoPartida::default();
    est.definiciones_variables = vec![variable(1, "Constitucion", "cont"), variable(2, "Fuerza", "fue")];
    est.zonas = vec![Zona { id: 1, nombre: "Z".into(), orden: 0 }];
    est.mapas = vec![serde_json::from_value(json!({
        "id": 1, "id_zona": 1, "nombre": "1", "ancho": 1000, "alto": 1000, "rejilla": 50, "color_opacidad": [255, 255, 255, 76], "grosor": 1
    }))
    .unwrap()];
    est.banco_entidades = vec![ficha(1, "Mago", vec![1]), ficha(2, "Goblin", vec![]), ficha(3, "Lejano", vec![]), ficha(4, "Oculto", vec![])];
    let mut oculta = instancia(4, ficha(4, "Oculto", vec![]), 5, 6);
    oculta.oculto = true;
    est.instancias_activas = vec![
        instancia(1, ficha(1, "Mago", vec![1]), 5, 5),
        instancia(2, ficha(2, "Goblin", vec![]), 6, 5),
        instancia(3, ficha(3, "Lejano", vec![]), 9, 5),
        oculta,
    ];
    est
}

/// Guarda la macro y la asocia a la instancia `destino` (id de asociación = id de la macro).
fn con_macro(est: &mut EstadoPartida, id: u16, nombre: &str, codigo: &str, destino: u16) {
    let tipo = comprobar(codigo).unwrap();
    est.aplicar_delta(&DeltaPartida::MacroGuardada(Macro {
        id, nombre: nombre.into(), codigo: codigo.into(), autor: ID_HOST, al_pulsar: tipo.al_pulsar, en_turno: tipo.en_turno, conjunto: None, propietaria: None,
    }));
    est.aplicar_delta(&DeltaPartida::MacroAsociada(AsociacionMacro {
        id: u32::from(id), id_macro: id, destino: DestinoMacro::Instancia(destino), autor: ID_HOST,
    }));
}

fn vida(est: &EstadoPartida, id: u16) -> i16 {
    est.instancias_activas.iter().find(|i| i.id_instancia == id).unwrap().valores_actuales[0].valor
}

fn correr(p: &mut Prueba, asoc: u32, inst: u16) -> Result<(), String> {
    ejecutar(p, asoc, inst, ID_HOST, &[])
}

// ---------------- Intérprete ----------------

#[test]
fn el_veneno_quita_vida_si_falla_la_tirada_y_se_va_si_la_supera() {
    let codigo = "ONTURN\nDO CHANGE @vida -1d4 IF EXECUTE 1d20 >= .cont ELSE QUIT";
    let mut p = Prueba { est: partida(), caras: vec![1, 3], chat: vec![] };
    con_macro(&mut p.est, 1, "Envenenado", codigo, 1);
    // 1d20 = 1 >= cont (+2)? no: va al ELSE y se quita
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat[0].0, "Envenenado: 1d20 (<b>1</b>) >= cont (+2)");
    assert!(p.chat[0].2, "EXECUTE es pública");
    assert!(p.est.asociaciones_macro.is_empty(), "QUIT la desasocia");
    assert_eq!(vida(&p.est, 1), 20);

    con_macro(&mut p.est, 1, "Envenenado", codigo, 1);
    p.caras = vec![15, 3];
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(vida(&p.est, 1), 17, "con el signo delante se suma: -1d4 = -3");
    assert_eq!(p.est.asociaciones_macro.len(), 1);
}

#[test]
fn change_sin_signo_pone_el_valor_y_solo_vale_con_maximo() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    con_macro(&mut p.est, 1, "Curar", "ONCLICK\nCHANGE @vida 5\nCHANGE @vida +2", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(vida(&p.est, 1), 7);
    assert!(p.chat.is_empty(), "sin EXECUTE no sale nada en el chat");

    con_macro(&mut p.est, 2, "Mal", "ONCLICK\nCHANGE @armadura 3", 1);
    let e = correr(&mut p, 2, 1).unwrap_err();
    assert!(e.contains("máximo"), "{e}");
    assert!(p.chat[0].0.starts_with("Mal: error:") && !p.chat[0].2, "el error sale en privado");
}

#[test]
fn save_guarda_atributos_solo_durante_la_ejecucion() {
    let mut p = Prueba { est: partida(), caras: vec![4, 5], chat: vec![] };
    con_macro(&mut p.est, 1, "Contar", "ONCLICK\nDO SAVE veces 0 IF 1 = 2\nSAVE dano 2d6 + .fue\nEXECUTEP dano", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat, vec![("Contar: dano (+11)".into(), 11, false)]);
    // Otra ejecución con otros dados: el atributo se vuelve a calcular, no arrastra nada de la anterior
    p.caras = vec![1, 1];
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat[1], ("Contar: dano (+4)".into(), 4, false));

    // Leer un atributo que la macro no guarda ya no pasa ni la comprobación del código
    assert!(comprobar("ONCLICK\nEXECUTE veces").unwrap_err().contains("ningún SAVE"));
}

#[test]
fn turn_fuera_de_batalla_es_un_error_y_en_batalla_es_la_ronda() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    con_macro(&mut p.est, 1, "Ronda", "ONCLICK\nEXECUTE TURN + 1", 1);
    assert!(correr(&mut p, 1, 1).unwrap_err().contains("batalla"));
    p.est.mapas[0].batalla = Some(Batalla {
        grupos: vec![GrupoBatalla { id: 1, nombre: "B".into(), miembros: vec![1, 2], turno: Some(1), ronda: 3 }],
        siguiente_id: 2,
    });
    p.chat.clear();
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat[0], ("Ronda: turno (3) + 1".into(), 4, true));
}

#[test]
fn el_ataque_en_area_llega_solo_a_quien_esta_en_la_zona_cada_uno_con_su_tirada() {
    let codigo = "ONCLICK\nSPREAD CHANGE ENTITIES@vida -(2d8 + .fue) TO ENTITIES@armadura <= 1d20 + .fue AND ENTITIES IN @rangoarma";
    // Goblin (orden 2): d20 = 15 → 17 >= 12 y está a 1 casilla (círculo de 2): 2d8 = 3 + 4 → -(7 + 2).
    // Lejano (orden 3): d20 = 15, pero a 4 casillas queda fuera. El oculto nunca cuenta.
    let mut p = Prueba { est: partida(), caras: vec![15, 3, 4, 15], chat: vec![] };
    con_macro(&mut p.est, 1, "Bola", codigo, 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!((vida(&p.est, 1), vida(&p.est, 2), vida(&p.est, 3), vida(&p.est, 4)), (20, 11, 20, 20));
    assert!(p.chat.is_empty());
}

#[test]
fn un_bloque_en_spread_ensena_lo_que_quita_a_cada_uno() {
    // Cada entidad tira su propio d20 (4 al goblin, 9 al lejano), lo guarda, se lo quita y lo enseña
    let codigo = "ONCLICK\nSPREAD [\n  SAVE dano 1d20\n  CHANGE ENTITIES@vida -dano\n  EXECUTE -dano\n] TO ENTITIES@armadura >= 0";
    let mut p = Prueba { est: partida(), caras: vec![4, 9], chat: vec![] };
    con_macro(&mut p.est, 1, "Rafaga", codigo, 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!((vida(&p.est, 2), vida(&p.est, 3)), (16, 11));
    assert_eq!(p.chat, vec![("Rafaga → Goblin: - dano (+4)".into(), -4, true), ("Rafaga → Lejano: - dano (+9)".into(), -9, true)]);
}

#[test]
fn spread_sin_entidades_en_la_condicion_vale_para_todas_y_execute_dice_a_quien() {
    let mut p = Prueba { est: partida(), caras: vec![6, 2, 5], chat: vec![] };
    con_macro(&mut p.est, 1, "Grito", "ONCLICK\nSPREAD EXECUTE 1d6 TO 1d20 >= 5", 1);
    correr(&mut p, 1, 1).unwrap();
    // 1d20 = 6 (una vez para todas) y luego 1d6 por cada una (Goblin 2, Lejano 5)
    assert_eq!(p.chat, vec![("Grito → Goblin: 1d6 (2)".into(), 2, true), ("Grito → Lejano: 1d6 (5)".into(), 5, true)]);
}

#[test]
fn spread_macros_asocia_a_cada_instancia_sin_repetir() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    con_macro(&mut p.est, 1, "Envenenado", "ONTURN\nCHANGE @vida -1", 9); // asociada a algo que no existe: solo se usa su nombre
    con_macro(&mut p.est, 2, "Toque", "ONCLICK\nSPREAD MACROS.envenenado TO DISTANCE <= 1", 1);
    correr(&mut p, 2, 1).unwrap();
    correr(&mut p, 2, 1).unwrap();
    let del_goblin: Vec<_> = p.est.asociaciones_macro.iter().filter(|a| a.destino == DestinoMacro::Instancia(2)).collect();
    assert_eq!(del_goblin.len(), 1, "una sola vez");
    assert_eq!(del_goblin[0].id_macro, 1);
    assert!(!p.est.asociaciones_macro.iter().any(|a| a.destino == DestinoMacro::Instancia(3)), "el lejano está a 3 casillas");
    assert!(!p.est.asociaciones_macro.iter().any(|a| a.destino == DestinoMacro::Instancia(4)), "el oculto no cuenta");
}

#[test]
fn una_comparacion_usa_la_escala_y_su_dado_natural() {
    let mut p = Prueba { est: partida(), caras: vec![20], chat: vec![] };
    p.est.escala_resultado = Some(crate::tablero_general::escalas::ejemplos().into_iter().find(|e| e.clave == "dnd").unwrap().escala);
    // 20 natural: la escala de D&D lo da por acertado aunque 20 < 99
    con_macro(&mut p.est, 1, "Ataque", "ONCLICK\nDO CHANGE @vida -1 IF EXECUTE 1d20 >= 99", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(vida(&p.est, 1), 19);
}

#[test]
fn los_topes_paran_una_macro_que_no_acaba() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    con_macro(&mut p.est, 1, "Muchos", &format!("ONCLICK\n{}", "EXECUTE 500d6\n".repeat(3)), 1);
    assert!(correr(&mut p, 1, 1).unwrap_err().contains("dados"));
}

#[test]
fn execute_de_una_comparacion_la_ensena_con_su_resultado() {
    let mut p = Prueba { est: partida(), caras: vec![7], chat: vec![] };
    con_macro(&mut p.est, 1, "Salvar", "ONCLICK\nEXECUTEP 1d20 + .fue >= 12", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat, vec![("Salvar: 1d20 (7) + fue (+2) >= 12".into(), 9, false)]);
}

#[test]
fn ifnot_aplica_el_cuerpo_cuando_la_condicion_falla() {
    // Salvación: si NO se supera (1d20 + cont < 14) pierde vida; si se supera, la macro se quita
    let codigo = "ONTURN\nDO CHANGE @vida -1d4 IFNOT EXECUTE 1d20 + .cont >= 14\nELSE QUIT";
    let mut p = Prueba { est: partida(), caras: vec![3, 2], chat: vec![] };
    con_macro(&mut p.est, 1, "Veneno", codigo, 1);
    correr(&mut p, 1, 1).unwrap(); // 3 + 2 = 5 < 14: no la supera, pierde 1d4 = 2
    assert_eq!(vida(&p.est, 1), 18);
    assert_eq!(p.est.asociaciones_macro.len(), 1);
    p.caras = vec![15];
    correr(&mut p, 1, 1).unwrap(); // 15 + 2 >= 14: la supera, ELSE QUIT
    assert!(p.est.asociaciones_macro.is_empty());
}

#[test]
fn quit_para_la_macro() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    con_macro(&mut p.est, 1, "Una vez", "ONCLICK\nEXECUTE 1\nQUIT\nEXECUTE 2", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat.len(), 1);
}

// ---------------- Comandos, permisos y turnos (con un AppState de verdad) ----------------

fn estado_real(nombre: &str) -> AppState {
    let dir = dir_temporal(nombre);
    std::fs::create_dir_all(&dir).unwrap();
    let almacen = Almacen::abrir(&dir).unwrap();
    // En el disco empieza igual que en memoria, para poder volver a cargarla
    estado::compactar(&almacen, &partida()).unwrap();
    AppState::nuevo(partida(), almacen, dir)
}

/// Una macro GENERAL (del conjunto): solo la crea el host.
fn datos(nombre: &str, codigo: &str) -> DatosMacro {
    DatosMacro { id: 0, nombre: nombre.into(), codigo: codigo.into(), id_entidad: None }
}

/// Una macro de un PERSONAJE (la que se crea desde su caja «Parámetros»).
fn de_ficha(nombre: &str, codigo: &str, ficha: u16) -> DatosMacro {
    DatosMacro { id_entidad: Some(ficha), ..datos(nombre, codigo) }
}

#[test]
fn guardar_valida_el_codigo_y_saca_el_tipo_de_la_cabecera() {
    let state = estado_real("macros_guardar");
    let m = guardar_macro_en(&state, 1, de_ficha("Golpe", "ONTURN ONCLICK\nEXECUTE 1d6", 1)).unwrap();
    assert!(m.al_pulsar && m.en_turno && m.autor == 1);
    assert!(guardar_macro_en(&state, 1, de_ficha("Mal", "EXECUTE 1", 1)).unwrap_err().contains("línea"));
    assert!(guardar_macro_en(&state, 1, de_ficha("golpe", "ONCLICK\nEXECUTE 1", 1)).unwrap_err().contains("Ya hay"));
    assert!(guardar_macro_en(&state, 1, de_ficha("Raro<script>", "ONCLICK\nEXECUTE 1", 1)).is_err());
    // Un jugador no crea generales ni macros de una ficha que no es suya
    assert!(guardar_macro_en(&state, 1, datos("General", "ONCLICK\nEXECUTE 1")).unwrap_err().contains("host"));
    assert!(guardar_macro_en(&state, 1, de_ficha("Ajena", "ONCLICK\nEXECUTE 1", 2)).is_err());
    assert_eq!(m.propietaria, Some(1));
    // Otro jugador no la cambia ni la borra; el host sí, y conserva el autor
    assert!(guardar_macro_en(&state, 2, DatosMacro { id: m.id, ..datos("X", "ONCLICK\nEXECUTE 1") }).is_err());
    assert!(borrar_macro_en(&state, 2, m.id).is_err());
    let editada = guardar_macro_en(&state, ID_HOST, DatosMacro { id: m.id, ..datos("Golpe", "ONCLICK\nEXECUTE 2") }).unwrap();
    assert!(editada.al_pulsar && !editada.en_turno && editada.autor == 1);
}

#[test]
fn un_jugador_asocia_sus_macros_a_sus_fichas_y_solo_ejecuta_las_de_pulsar() {
    let state = estado_real("macros_permisos");
    let suya = guardar_macro_en(&state, 1, de_ficha("Golpe", "ONCLICK\nCHANGE @vida -1", 1)).unwrap();
    let turno = guardar_macro_en(&state, 1, de_ficha("Veneno", "ONTURN\nCHANGE @vida -1", 1)).unwrap();
    let del_host = guardar_macro_en(&state, ID_HOST, datos("Secreta", "ONCLICK\nCHANGE @vida -1")).unwrap();
    assert!(asociar_macro_en(&state, 1, suya.id, 2).unwrap_err().contains("tus fichas"), "la ficha 2 no es suya");
    let a = asociar_macro_en(&state, 1, suya.id, 1).unwrap();
    assert_eq!(asociar_macro_en(&state, 1, suya.id, 1).unwrap(), a, "dos veces es la misma (nació asociada a su personaje)");
    let b = asociar_macro_en(&state, 1, turno.id, 1).unwrap();
    // Una macro de un personaje solo se asocia a él (ni siquiera el host la pone en otro)
    assert!(asociar_macro_en(&state, ID_HOST, suya.id, 2).unwrap_err().contains("otro personaje"));
    assert!(ejecutar_macro_en(&state, 2, a, 1, vec![]).is_err(), "otro jugador no la pulsa");
    assert!(ejecutar_macro_en(&state, 1, b, 1, vec![]).unwrap_err().contains("ONCLICK"));
    assert!(ejecutar_macro_en(&state, 1, a, 2, vec![]).unwrap_err().contains("no está asociada"));
    ejecutar_macro_en(&state, 1, a, 1, vec![]).unwrap();
    assert_eq!(vida(&state.lock(), 1), 19);
    // Una general (del host) la puede poner cualquiera en sus fichas: la ve, pero sin código
    asociar_macro_en(&state, 1, del_host.id, 1).unwrap();
    let est = state.lock();
    let vistas = macros_para(1, &est);
    assert_eq!(vistas.len(), 3);
    assert!(vistas.iter().find(|m| m.id == del_host.id).unwrap().codigo.is_empty());
    assert!(!vistas.iter().find(|m| m.id == suya.id).unwrap().codigo.is_empty());
    // Quien no controla nada la ve solo la general (sin código), no las de otros personajes
    let de_2 = macros_para(2, &est);
    assert_eq!(de_2.iter().map(|m| m.id).collect::<Vec<_>>(), vec![del_host.id]);
    assert!(de_2[0].codigo.is_empty());
    assert_eq!(asociaciones_para(2, &est).len(), 0);
    drop(est);
    assert!(desasociar_macro_en(&state, 2, a).is_err());
    desasociar_macro_en(&state, 1, a).unwrap();
}

#[test]
fn por_la_red_un_jugador_solo_toca_lo_suyo_y_su_executep_solo_lo_ve_el_y_el_host() {
    use crate::red::peticiones::{decodificar, permitida, Peticion};
    use crate::red::vista::vista_para;
    use std::collections::HashSet;
    let state = estado_real("macros_red");
    let suya = guardar_macro_en(&state, 1, de_ficha("Secreto", "ONCLICK\nEXECUTEP 1d6", 1)).unwrap();
    let ajena = guardar_macro_en(&state, ID_HOST, datos("Del host", "ONCLICK\nEXECUTE 1")).unwrap();
    let pet = |v: serde_json::Value| decodificar(v).unwrap();
    let est = state.lock();
    let ok = |p: &Peticion| permitida(&est, 1, p, &HashSet::new()).is_ok();
    assert!(ok(&pet(json!({ "cmd": "validar_macro", "codigo": "ONCLICK\nEXECUTE 1" }))));
    // Crear: de uno de sus personajes sí; general (sin personaje) o de un personaje ajeno, no
    assert!(ok(&pet(json!({ "cmd": "guardar_macro", "datos": { "nombre": "Nueva", "codigo": "ONCLICK\nEXECUTE 1", "idEntidad": 1 } }))));
    assert!(!ok(&pet(json!({ "cmd": "guardar_macro", "datos": { "nombre": "Nueva", "codigo": "ONCLICK\nEXECUTE 1" } }))));
    assert!(!ok(&pet(json!({ "cmd": "guardar_macro", "datos": { "nombre": "Nueva", "codigo": "ONCLICK\nEXECUTE 1", "idEntidad": 2 } }))));
    assert!(!ok(&pet(json!({ "cmd": "guardar_macro", "datos": { "id": ajena.id, "nombre": "X", "codigo": "ONCLICK\nEXECUTE 1" } }))));
    assert!(!ok(&pet(json!({ "cmd": "borrar_macro", "id": ajena.id }))));
    assert!(ok(&pet(json!({ "cmd": "asociar_macro", "idMacro": suya.id, "idEntidad": 1 }))));
    assert!(!ok(&pet(json!({ "cmd": "asociar_macro", "idMacro": suya.id, "idEntidad": 2 }))));
    assert!(ok(&pet(json!({ "cmd": "asociar_macro", "idMacro": ajena.id, "idEntidad": 1 }))), "una general sí");
    assert!(!ok(&pet(json!({ "cmd": "ejecutar_macro", "idAsociacion": 1, "idInstancia": 2 }))));
    drop(est);

    let a = asociar_macro_en(&state, 1, suya.id, 1).unwrap();
    ejecutar_macro_en(&state, 1, a, 1, vec![]).unwrap();
    let est = state.lock();
    let privada = |j| vista_para(j, &est).historial_tiradas.iter().any(|t| t.comando.starts_with("Secreto:") && !t.publica);
    assert!(privada(1), "quien la tira la ve");
    assert!(!privada(2), "los demás no");
    let v = vista_para(1, &est);
    assert_eq!(v.asociaciones_macro.len(), 1);
    assert_eq!(v.macros.len(), 2, "la suya y la general del host (esta, sin código)");
    assert!(v.macros.iter().find(|m| m.id == ajena.id).unwrap().codigo.is_empty());
    assert_eq!(vista_para(2, &est).macros.iter().map(|m| m.id).collect::<Vec<_>>(), vec![ajena.id], "ni la de otro personaje");
}

#[test]
fn las_de_turno_se_ejecutan_al_pasar_el_turno_y_no_al_recargar() {
    let state = estado_real("macros_turno");
    let m = guardar_macro_en(&state, ID_HOST, datos("Quemadura", "ONTURN\nCHANGE @vida -2\nEXECUTE 0")).unwrap();
    asociar_macro_en(&state, ID_HOST, m.id, 2).unwrap(); // a la ficha del goblin
    crate::tablero_general::partida::procesar_delta(
        &state,
        DeltaPartida::BatallaIniciada {
            id_mapa: 1,
            batalla: Batalla {
                grupos: vec![GrupoBatalla { id: 1, nombre: "B".into(), miembros: vec![1, 2], turno: Some(1), ronda: 1 }],
                siguiente_id: 2,
            },
        },
    );
    assert_eq!(vida(&state.lock(), 2), 20, "el turno es del mago");
    crate::tablero_general::batalla::pasar_turno_en(&state, ID_HOST, 1, 1).unwrap();
    assert_eq!(vida(&state.lock(), 2), 18, "le llega el turno al goblin");
    let lineas = state.lock().historial_tiradas.len();
    assert_eq!(lineas, 1);
    // Al volver a cargar la partida desde el disco no se repite nada
    let (cargada, _) = estado::cargar(&state.almacen).unwrap();
    assert_eq!(vida(&cargada, 2), 18);
    assert_eq!(cargada.historial_tiradas.len(), 1);
}


// ---------------- Macros por conjunto de parámetros ----------------

#[test]
fn una_macro_es_del_conjunto_activo_y_solo_vale_con_el() {
    use crate::tablero_general::conjuntos::{anadir_conjunto_de_serie_en, borrar_conjunto_en, usar_conjunto_en};
    let state = estado_real("macros_conjunto");
    let base = state.lock().conjunto_activo;
    let m = guardar_macro_en(&state, ID_HOST, datos("Curar", "ONCLICK\nCHANGE @vida +1")).unwrap();
    assert_eq!(m.conjunto, Some(base), "nace en el conjunto activo");
    let a = asociar_macro_en(&state, ID_HOST, m.id, 1).unwrap();

    // Añadir Pathfinder crea sus macros de serie, del host y de ese conjunto (y el conjunto guardado no las repite)
    let pf = anadir_conjunto_de_serie_en("pathfinder2e", &state).unwrap();
    {
        let est = state.lock();
        let suyas: Vec<&Macro> = est.macros.iter().filter(|x| x.conjunto == Some(pf)).collect();
        assert!(suyas.len() >= 3 && suyas.iter().all(|x| x.autor == ID_HOST));
        assert!(est.conjuntos_parametros.iter().find(|c| c.id == pf).unwrap().contenido.macros.is_empty());
    }

    // Con Pathfinder en uso, la macro del otro conjunto ni se ve ni se ejecuta (pero sigue ahí)
    usar_conjunto_en(pf, &state).unwrap();
    assert!(ejecutar_macro_en(&state, ID_HOST, a, 1, vec![]).unwrap_err().contains("otro conjunto"));
    assert!(asociar_macro_en(&state, ID_HOST, m.id, 2).is_err());
    {
        let est = state.lock();
        assert!(!macros_para(1, &est).iter().any(|x| x.id == m.id));
        assert!(est.macros.iter().any(|x| x.id == m.id));
    }
    // Se puede repetir el nombre de una macro de otro conjunto
    assert!(guardar_macro_en(&state, ID_HOST, datos("Curar", "ONCLICK\nEXECUTE 1")).is_ok());

    // Al volver, vuelve a funcionar; y borrar Pathfinder se lleva sus macros
    usar_conjunto_en(base, &state).unwrap();
    ejecutar_macro_en(&state, ID_HOST, a, 1, vec![]).unwrap();
    borrar_conjunto_en(pf, &state).unwrap();
    assert!(!state.lock().macros.iter().any(|x| x.conjunto == Some(pf)));
}

#[test]
fn la_plantilla_de_los_parametros_lleva_las_macros_del_conjunto_y_al_importarla_vuelven() {
    let state = estado_real("macros_plantilla_conjunto");
    guardar_macro_en(&state, ID_HOST, datos("Del host", "ONCLICK\nEXECUTE 1")).unwrap();
    guardar_macro_en(&state, 1, de_ficha("De un personaje", "ONCLICK\nEXECUTE 2", 1)).unwrap();
    let est = state.lock();
    let llevadas = del_conjunto_activo(&est);
    assert_eq!(llevadas.iter().map(|m| m.nombre.as_str()).collect::<Vec<_>>(), vec!["Del host"], "las de un personaje no salen con el conjunto");
    drop(est);
    // Un conjunto con macros que no valen se rechaza entero
    assert!(validar_de_conjunto(vec![MacroConjunto { nombre: "Mala".into(), codigo: "EXECUTE 1".into() }]).is_err());
    let dos = |n: &str| MacroConjunto { nombre: n.into(), codigo: "ONCLICK\nEXECUTE 1".into() };
    assert!(validar_de_conjunto(vec![dos("A b"), dos("ab")]).unwrap_err().contains("dos macros"));
    assert_eq!(validar_de_conjunto(llevadas).unwrap().len(), 1);
}

#[test]
fn importar_las_macros_de_una_ficha_las_crea_o_reutiliza_y_las_asocia() {
    let state = estado_real("macros_importar_ficha");
    let golpe = |codigo: &str| MacroFicha { nombre: "Golpe".into(), codigo: codigo.into(), general: false, asociada: true };
    let mala = MacroFicha { nombre: "Rota".into(), codigo: "EXECUTE 1".into(), general: false, asociada: true };
    let avisos = importar_macros_ficha_en(&state, 1, 1, vec![golpe("ONCLICK\nEXECUTE 1d6"), mala.clone()]).unwrap();
    assert_eq!(avisos.len(), 1, "la que no se puede leer se salta: {avisos:?}");
    let id = {
        let est = state.lock();
        let m = est.macros.iter().find(|m| m.nombre == "Golpe").unwrap();
        assert_eq!((m.autor, m.conjunto), (1, Some(est.conjunto_activo)));
        assert!(est.asociaciones_macro.iter().any(|a| a.id_macro == m.id && a.destino == DestinoMacro::Ficha(1)));
        m.id
    };
    // Otra vez (con otro código): se reutiliza la que ya tenía, sin pisarla ni repetir la asociación
    importar_macros_ficha_en(&state, 1, 1, vec![golpe("ONCLICK\nEXECUTE 2")]).unwrap();
    let est = state.lock();
    assert_eq!(est.macros.iter().filter(|m| m.nombre == "Golpe").count(), 1);
    assert_eq!(est.macros.iter().find(|m| m.id == id).unwrap().codigo, "ONCLICK\nEXECUTE 1d6");
    assert_eq!(est.asociaciones_macro.iter().filter(|a| a.id_macro == id).count(), 1);
    drop(est);
    // A una ficha que no es suya, no
    assert!(importar_macros_ficha_en(&state, 1, 2, vec![golpe("ONCLICK\nEXECUTE 1")]).is_err());
}

#[test]
fn las_macros_de_serie_se_leen_y_usan_nombres_de_su_conjunto() {
    use super::lexico::{trocear, Clave, Tok};
    let sin = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    for i in crate::tablero_general::conjuntos::integrados() {
        let c = &i.contenido;
        assert_eq!(validar_de_conjunto(c.macros.clone()).unwrap().len(), c.macros.len(), "{}", i.clave);
        for m in &c.macros {
            let mut anterior: Option<Tok> = None;
            for p in trocear(&m.codigo).unwrap() {
                match &p.tok {
                    Tok::Var(n) if anterior != Some(Tok::Clave(Clave::Macros)) => assert!(
                        c.variables.iter().any(|v| sin(&v.nombre) == sin(n) || (!v.abreviatura.is_empty() && sin(&v.abreviatura) == sin(n))),
                        "{}: «{}» usa la variable .{n}, que no está en el conjunto",
                        i.clave,
                        m.nombre
                    ),
                    Tok::Param { nombre, .. } => assert!(
                        c.parametros.iter().any(|d| sin(&d.clave) == sin(nombre)),
                        "{}: «{}» usa el parámetro @{nombre}, que no está en el conjunto",
                        i.clave,
                        m.nombre
                    ),
                    _ => {}
                }
                anterior = Some(p.tok.clone());
            }
        }
    }
}


// ---------------- Macros de un personaje y generales ----------------

#[test]
fn una_macro_de_un_personaje_no_sale_con_el_conjunto_y_se_va_con_el_personaje() {
    let state = estado_real("macros_personaje");
    let propia = guardar_macro_en(&state, ID_HOST, de_ficha("Solo del mago", "ONCLICK\nEXECUTE 1", 1)).unwrap();
    let general = guardar_macro_en(&state, ID_HOST, datos("Para todos", "ONCLICK\nEXECUTE 2")).unwrap();
    assert_eq!((propia.propietaria, general.propietaria), (Some(1), None));
    {
        let est = state.lock();
        // Nace asociada a su personaje; la general, no
        assert!(est.asociaciones_macro.iter().any(|a| a.id_macro == propia.id && a.destino == DestinoMacro::Ficha(1)));
        assert!(!est.asociaciones_macro.iter().any(|a| a.id_macro == general.id));
        // Solo la general sale con el conjunto
        assert_eq!(del_conjunto_activo(&est).iter().map(|m| m.nombre.as_str()).collect::<Vec<_>>(), vec!["Para todos"]);
    }
    // El mismo nombre en dos personajes distintos vale; en el mismo personaje, o contra una general, no
    assert!(guardar_macro_en(&state, ID_HOST, de_ficha("Solo del mago", "ONCLICK\nEXECUTE 1", 2)).is_ok());
    assert!(guardar_macro_en(&state, ID_HOST, de_ficha("Solo del mago", "ONCLICK\nEXECUTE 1", 1)).is_err());
    assert!(guardar_macro_en(&state, ID_HOST, de_ficha("Para todos", "ONCLICK\nEXECUTE 1", 2)).is_err());
    // Al vaciar la papelera sin su personaje, sus macros se van con él
    {
        let mut est = state.lock();
        est.banco_entidades.retain(|e| e.id != 1);
        est.instancias_activas.retain(|i| i.base.id != 1);
    }
    crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::PapeleraVaciada);
    let est = state.lock();
    assert!(!est.macros.iter().any(|m| m.id == propia.id), "la del personaje borrado desaparece");
    assert!(est.macros.iter().any(|m| m.id == general.id), "la general se queda");
}

#[test]
fn spread_macros_solo_alcanza_las_generales_y_las_del_personaje_que_la_lanza() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    let mut suelta = |est: &mut EstadoPartida, id: u16, nombre: &str, propietaria: Option<u16>| {
        est.aplicar_delta(&DeltaPartida::MacroGuardada(Macro {
            id, nombre: nombre.into(), codigo: "ONTURN\nCHANGE @vida -1".into(), autor: ID_HOST, al_pulsar: false, en_turno: true, conjunto: None, propietaria,
        }));
    };
    suelta(&mut p.est, 1, "Veneno", None);
    suelta(&mut p.est, 2, "Tuya", Some(1));
    suelta(&mut p.est, 3, "Ajena", Some(2));
    con_macro(&mut p.est, 4, "Lanzar", "ONCLICK\nSPREAD MACROS.veneno, MACROS.tuya TO DISTANCE <= 1", 1);
    correr(&mut p, 4, 1).unwrap();
    let del_goblin: Vec<u16> = p.est.asociaciones_macro.iter().filter(|a| a.destino == DestinoMacro::Instancia(2)).map(|a| a.id_macro).collect();
    assert_eq!(del_goblin, vec![1, 2], "la general y la del personaje que la lanza (su ficha es la 1)");
    // (id 9: los ids 5 y 6 ya son asociaciones que acaba de repartir la anterior)
    con_macro(&mut p.est, 9, "Lanzar ajena", "ONCLICK\nSPREAD MACROS.ajena TO DISTANCE <= 1", 1);
    assert!(correr(&mut p, 9, 1).is_err(), "la de otro personaje no se puede repartir");
}

#[test]
fn importar_generales_y_propias_las_asocia_y_un_jugador_no_crea_generales() {
    let state = estado_real("macros_importar_general");
    let general = guardar_macro_en(&state, ID_HOST, datos("Atacar", "ONCLICK\nEXECUTE 1d20")).unwrap();
    let ficha = |nombre: &str, codigo: &str, general: bool| MacroFicha { nombre: nombre.into(), codigo: codigo.into(), general, asociada: true };

    // La general que ya existe se usa (aunque la plantilla no traiga su código, como la de un jugador) y se asocia
    let avisos = importar_macros_ficha_en(&state, 1, 1, vec![ficha("Atacar", "", true), ficha("Mía", "ONCLICK\nEXECUTE 3", false)]).unwrap();
    assert!(avisos.is_empty(), "{avisos:?}");
    {
        let est = state.lock();
        assert!(est.asociaciones_macro.iter().any(|a| a.id_macro == general.id && a.destino == DestinoMacro::Ficha(1)));
        let mia = est.macros.iter().find(|m| m.nombre == "Mía").unwrap();
        assert_eq!((mia.autor, mia.propietaria), (1, Some(1)));
    }
    // Una general que no existe: sin código no se puede; con código, un jugador la crea como de su personaje (con aviso)...
    let avisos = importar_macros_ficha_en(&state, 1, 1, vec![ficha("Nueva general", "", true)]).unwrap();
    assert!(avisos[0].contains("no trae su código"));
    let avisos = importar_macros_ficha_en(&state, 1, 1, vec![ficha("Nueva general", "ONCLICK\nEXECUTE 4", true)]).unwrap();
    assert!(avisos[0].contains("no eres el host"));
    assert_eq!(state.lock().macros.iter().find(|m| m.nombre == "Nueva general").unwrap().propietaria, Some(1));
    // ...y el host sí la crea general
    importar_macros_ficha_en(&state, ID_HOST, 2, vec![ficha("General del host", "ONCLICK\nEXECUTE 5", true)]).unwrap();
    assert!(state.lock().macros.iter().find(|m| m.nombre == "General del host").unwrap().es_general());

    // Las generales que el personaje NO tenía puestas viajan igual: el host las crea (el conjunto las tiene) pero no se asocian;
    // un jugador, que no puede crear generales, las salta sin más
    let suelta = MacroFicha { asociada: false, ..ficha("Suelta", "ONCLICK\nEXECUTE 6", true) };
    importar_macros_ficha_en(&state, 1, 1, vec![suelta.clone()]).unwrap();
    assert!(!state.lock().macros.iter().any(|m| m.nombre == "Suelta"), "un jugador no crea generales");
    importar_macros_ficha_en(&state, ID_HOST, 2, vec![suelta]).unwrap();
    let est = state.lock();
    let id = est.macros.iter().find(|m| m.nombre == "Suelta").unwrap();
    assert!(id.es_general());
    assert!(!est.asociaciones_macro.iter().any(|a| a.id_macro == id.id), "existe en el conjunto pero no está puesta");
}

// ---------------- Textos y AREA ----------------

#[test]
fn se_comparan_textos_con_el_nombre_el_tipo_y_parametros_de_texto() {
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    // Solo a quien no se llama «Lejano» y es Enemigo (todas lo son): el goblin
    con_macro(&mut p.est, 1, "Filtro", "ONCLICK\nSPREAD CHANGE ENTITIES@vida -5 TO ENTITIES@nombre != \"lejano\" AND ENTITIES@tipo = \" Enemigo \"", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!((vida(&p.est, 2), vida(&p.est, 3)), (15, 20), "sin distinguir mayúsculas ni espacios de los extremos");
    // Con EXECUTE sale en el chat con su resultado
    con_macro(&mut p.est, 2, "Quien", "ONCLICK\nDO QUIT IF EXECUTE @nombre = \"Mago\"", 1);
    correr(&mut p, 2, 1).unwrap();
    assert_eq!(p.chat.last().unwrap().0, "Quien: nombre (Mago) = \"Mago\"");
    // Un parámetro numérico no se compara con un texto
    con_macro(&mut p.est, 3, "Mal", "ONCLICK\nDO QUIT IF @vida = \"20\"", 1);
    assert!(correr(&mut p, 3, 1).unwrap_err().contains("número"));
}

#[test]
fn un_area_se_aplica_alrededor_del_centro_que_se_apunta() {
    // Rombo de 2 a izquierda y derecha y 0 arriba y abajo, centrado en (8,5): casillas 6..10 de la fila 5
    let codigo = "ONCLICK\nSPREAD CHANGE ENTITIES@vida -3 TO ENTITIES IN AREA:rombo(2, 0)";
    let mut p = Prueba { est: partida(), caras: vec![], chat: vec![] };
    con_macro(&mut p.est, 1, "Explosion", codigo, 1);
    // Sin centro, no se puede
    assert!(correr(&mut p, 1, 1).unwrap_err().contains("Apunta"));
    ejecutar(&mut p, 1, 1, ID_HOST, &[Casilla { x: 8, y: 5 }]).unwrap();
    // El goblin (6,5) y el lejano (9,5) caen dentro; el oculto (5,6) no cuenta nunca
    assert_eq!((vida(&p.est, 2), vida(&p.est, 3), vida(&p.est, 4)), (17, 17, 20));

    // Lo que necesita el front para la vista previa: la forma y los lados [arriba, derecha, abajo, izquierda]
    let areas = super::interprete::areas_de(&p.est, 1, 1).unwrap();
    assert_eq!(areas, vec![super::interprete::AreaResuelta { forma: crate::tablero_general::tablero::FormaZona::Rombo, lados: [0, 2, 0, 2] }]);
    // Con variables y parámetros de la ficha (fue de 14 es +2; vida 20): 4 lados
    con_macro(&mut p.est, 2, "Cuatro", "ONCLICK\nSPREAD EXECUTE 1 TO ENTITIES IN AREA:cuadrado(.fue, 1, @vida - 19, 200)", 1);
    let areas = super::interprete::areas_de(&p.est, 2, 1).unwrap();
    assert_eq!(areas[0].lados, [2, 1, 1, 100], "cada lado como mucho 100");
}

#[test]
fn preparar_dice_que_areas_apuntar_y_ejecutar_usa_los_centros() {
    let state = estado_real("macros_area_real");
    let m = guardar_macro_en(&state, ID_HOST, de_ficha("Bola", "ONCLICK\nSPREAD CHANGE ENTITIES@vida -1 TO ENTITIES IN AREA:circulo(1)", 1)).unwrap();
    let a = state.lock().asociaciones_macro.iter().find(|x| x.id_macro == m.id).unwrap().id;
    let areas = preparar_macro_en(&state, 1, a, 1).unwrap();
    assert_eq!(areas.len(), 1);
    assert!(preparar_macro_en(&state, 2, a, 1).is_err(), "otro jugador no");
    ejecutar_macro_en(&state, 1, a, 1, vec![Casilla { x: 6, y: 5 }]).unwrap();
    assert_eq!(vida(&state.lock(), 2), 19);
    assert!(ejecutar_macro_en(&state, 1, a, 1, vec![Casilla { x: 60_000, y: 5 }]).is_err(), "fuera de rango");
}
