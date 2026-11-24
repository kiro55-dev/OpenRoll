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
        id, nombre: nombre.into(), codigo: codigo.into(), autor: ID_HOST, al_pulsar: tipo.al_pulsar, en_turno: tipo.en_turno,
    }));
    est.aplicar_delta(&DeltaPartida::MacroAsociada(AsociacionMacro {
        id: u32::from(id), id_macro: id, destino: DestinoMacro::Instancia(destino), autor: ID_HOST, atributos: vec![],
    }));
}

fn vida(est: &EstadoPartida, id: u16) -> i16 {
    est.instancias_activas.iter().find(|i| i.id_instancia == id).unwrap().valores_actuales[0].valor
}

fn correr(p: &mut Prueba, asoc: u32, inst: u16) -> Result<(), String> {
    ejecutar(p, asoc, inst, ID_HOST)
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
fn save_guarda_atributos_que_duran_entre_ejecuciones() {
    let mut p = Prueba { est: partida(), caras: vec![4, 5], chat: vec![] };
    con_macro(&mut p.est, 1, "Contar", "ONCLICK\nDO SAVE veces 0 IF 1 = 2\nSAVE dano 2d6 + .fue\nEXECUTEP dano", 1);
    correr(&mut p, 1, 1).unwrap();
    assert_eq!(p.chat, vec![("Contar: dano (+11)".into(), 11, false)]);
    let a = &p.est.asociaciones_macro[0];
    assert_eq!(a.atributos, vec![AtributoMacro { nombre: "dano".into(), valor: 11 }]);

    con_macro(&mut p.est, 2, "Leer", "ONCLICK\nEXECUTE veces", 1);
    assert!(correr(&mut p, 2, 1).unwrap_err().contains("no existe"));
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

fn datos(nombre: &str, codigo: &str) -> DatosMacro {
    DatosMacro { id: 0, nombre: nombre.into(), codigo: codigo.into() }
}

#[test]
fn guardar_valida_el_codigo_y_saca_el_tipo_de_la_cabecera() {
    let state = estado_real("macros_guardar");
    let m = guardar_macro_en(&state, 1, datos("Golpe", "ONTURN ONCLICK\nEXECUTE 1d6")).unwrap();
    assert!(m.al_pulsar && m.en_turno && m.autor == 1);
    assert!(guardar_macro_en(&state, 1, datos("Mal", "EXECUTE 1")).unwrap_err().contains("línea"));
    assert!(guardar_macro_en(&state, 1, datos("golpe", "ONCLICK\nEXECUTE 1")).unwrap_err().contains("Ya hay"));
    assert!(guardar_macro_en(&state, 1, datos("Raro<script>", "ONCLICK\nEXECUTE 1")).is_err());
    // Otro jugador no la cambia ni la borra; el host sí, y conserva el autor
    assert!(guardar_macro_en(&state, 2, DatosMacro { id: m.id, ..datos("X", "ONCLICK\nEXECUTE 1") }).is_err());
    assert!(borrar_macro_en(&state, 2, m.id).is_err());
    let editada = guardar_macro_en(&state, ID_HOST, DatosMacro { id: m.id, ..datos("Golpe", "ONCLICK\nEXECUTE 2") }).unwrap();
    assert!(editada.al_pulsar && !editada.en_turno && editada.autor == 1);
}

#[test]
fn un_jugador_asocia_sus_macros_a_sus_fichas_y_solo_ejecuta_las_de_pulsar() {
    let state = estado_real("macros_permisos");
    let suya = guardar_macro_en(&state, 1, datos("Golpe", "ONCLICK\nCHANGE @vida -1")).unwrap();
    let turno = guardar_macro_en(&state, 1, datos("Veneno", "ONTURN\nCHANGE @vida -1")).unwrap();
    let del_host = guardar_macro_en(&state, ID_HOST, datos("Secreta", "ONCLICK\nCHANGE @vida -1")).unwrap();
    assert!(asociar_macro_en(&state, 1, suya.id, 2).unwrap_err().contains("tus fichas"), "la ficha 2 no es suya");
    assert!(asociar_macro_en(&state, 1, del_host.id, 1).unwrap_err().contains("tus macros"));
    let a = asociar_macro_en(&state, 1, suya.id, 1).unwrap();
    assert_eq!(asociar_macro_en(&state, 1, suya.id, 1).unwrap(), a, "dos veces es la misma");
    let b = asociar_macro_en(&state, 1, turno.id, 1).unwrap();
    assert!(ejecutar_macro_en(&state, 2, a, 1).is_err(), "otro jugador no la pulsa");
    assert!(ejecutar_macro_en(&state, 1, b, 1).unwrap_err().contains("ONCLICK"));
    assert!(ejecutar_macro_en(&state, 1, a, 2).unwrap_err().contains("no está asociada"));
    ejecutar_macro_en(&state, 1, a, 1).unwrap();
    assert_eq!(vida(&state.lock(), 1), 19);
    // El host asocia la suya a la ficha del jugador: el jugador la ve, pero sin código
    asociar_macro_en(&state, ID_HOST, del_host.id, 1).unwrap();
    let est = state.lock();
    let vistas = macros_para(1, &est);
    assert_eq!(vistas.len(), 3);
    assert!(vistas.iter().find(|m| m.id == del_host.id).unwrap().codigo.is_empty());
    assert!(!vistas.iter().find(|m| m.id == suya.id).unwrap().codigo.is_empty());
    assert!(macros_para(2, &est).is_empty());
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
    let suya = guardar_macro_en(&state, 1, datos("Secreto", "ONCLICK\nEXECUTEP 1d6")).unwrap();
    let ajena = guardar_macro_en(&state, ID_HOST, datos("Del host", "ONCLICK\nEXECUTE 1")).unwrap();
    let pet = |v: serde_json::Value| decodificar(v).unwrap();
    let est = state.lock();
    let ok = |p: &Peticion| permitida(&est, 1, p, &HashSet::new()).is_ok();
    assert!(ok(&pet(json!({ "cmd": "validar_macro", "codigo": "ONCLICK\nEXECUTE 1" }))));
    assert!(ok(&pet(json!({ "cmd": "guardar_macro", "datos": { "nombre": "Nueva", "codigo": "ONCLICK\nEXECUTE 1" } }))));
    assert!(!ok(&pet(json!({ "cmd": "guardar_macro", "datos": { "id": ajena.id, "nombre": "X", "codigo": "ONCLICK\nEXECUTE 1" } }))));
    assert!(!ok(&pet(json!({ "cmd": "borrar_macro", "id": ajena.id }))));
    assert!(ok(&pet(json!({ "cmd": "asociar_macro", "idMacro": suya.id, "idEntidad": 1 }))));
    assert!(!ok(&pet(json!({ "cmd": "asociar_macro", "idMacro": suya.id, "idEntidad": 2 }))));
    assert!(!ok(&pet(json!({ "cmd": "asociar_macro", "idMacro": ajena.id, "idEntidad": 1 }))));
    assert!(!ok(&pet(json!({ "cmd": "ejecutar_macro", "idAsociacion": 1, "idInstancia": 2 }))));
    drop(est);

    let a = asociar_macro_en(&state, 1, suya.id, 1).unwrap();
    ejecutar_macro_en(&state, 1, a, 1).unwrap();
    let est = state.lock();
    let privada = |j| vista_para(j, &est).historial_tiradas.iter().any(|t| t.comando.starts_with("Secreto:") && !t.publica);
    assert!(privada(1), "quien la tira la ve");
    assert!(!privada(2), "los demás no");
    let v = vista_para(1, &est);
    assert_eq!(v.asociaciones_macro.len(), 1);
    assert_eq!(v.macros.len(), 1, "la del host no está asociada a nada suyo");
    assert!(vista_para(2, &est).macros.is_empty());
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
