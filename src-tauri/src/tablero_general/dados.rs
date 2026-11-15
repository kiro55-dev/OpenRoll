// src-tauri/src/tablero_general/dados.rs
use std::fmt::Write;
use tauri::State;
use crate::tablero_general::estado::{DeltaPartida, EstadoPartida};
use crate::tablero_general::jugadores::controla_instancia;
use crate::tablero_general::modelos::{IdJugador, ID_HOST};
use crate::tablero_general::partida::{procesar_delta, registrar_comando_personal, registrar_tirada, AppState};
use crate::tablero_general::variables;

pub struct Tirada {
    pub signo: i32,
    pub veces: i32,
    pub d: i32,
    pub bonificador: Option<i32>,
    /// Si el bonificador sale de una variable de la ficha (`2d6 + f`): su abreviatura, para el desglose («f (+1)»).
    pub variable: Option<String>,
    pub ventaja: bool,
    pub desventaja: bool,
    /// Multiplica la suma de ESTE término de dados (`2*3d20`). 1 = sin multiplicar.
    pub multiplicador: i32,
}

/// Lo que vale en una tirada la variable que nombra `texto` (por nombre o abreviatura): su abreviatura y su
/// resultado en la ficha con el foco (`id_instancia`). Un jugador solo puede usar las de sus fichas.
fn resolver_variable(
    est: &EstadoPartida,
    jugador: IdJugador,
    id_instancia: Option<u16>,
    texto: &str,
) -> Result<(String, i32), String> {
    let def = variables::buscar(&est.definiciones_variables, texto)
        .ok_or_else(|| format!("«{}» no es una variable", texto.trim()))?;
    let id = id_instancia.ok_or("Pon el foco en una ficha (solo una) para usar sus variables")?;
    let inst = est.instancias_activas.iter().find(|i| i.id_instancia == id).ok_or("Esa ficha ya no está activa")?;
    if !controla_instancia(est, jugador, id) {
        return Err("Solo puedes usar las variables de tus fichas".into());
    }
    let valor = inst.base.variables.iter().find(|v| v.id_definicion == def.id).map_or(def.neutro, |v| v.valor);
    Ok((def.abreviatura.clone(), variables::modificador(def, valor)))
}

// Resalta en negrita si es 1 (pifia) o el valor máximo de caras (crítico)
fn formatear_num(valor: i32, caras: i32) -> String {
    if valor == 1 || valor == caras {
        format!("<b>{valor}</b>")
    } else {
        format!("{valor}")
    }
}

/// Longitud máxima de un comando de dados. Cualquier tirada razonable cabe de sobra; cuando haya red, un
/// jugador no podrá mandar al host textos enormes que parsear.
pub const MAX_LONGITUD_COMANDO: usize = 200;

/// Solo acepta tiradas: números, `d`, `+`, `-`, `*` y `adv`/`desv`/`dis`, y variables (un término solo de
/// letras), que `resolver` convierte en (abreviatura, resultado). Cualquier otro texto es un error, así que nada
/// de lo escrito llega al desglose (que lo forma `ejecutar_tiradas` con números, palabras fijas y la abreviatura
/// que definió el host, que solo lleva letras).
fn parsear_comando(
    comando: &str,
    resolver: &mut dyn FnMut(&str) -> Result<(String, i32), String>,
) -> Result<Vec<Tirada>, String> {
    if comando.chars().count() > MAX_LONGITUD_COMANDO {
        return Err(format!("El comando no puede pasar de {MAX_LONGITUD_COMANDO} caracteres"));
    }
    let mut tiradas: Vec<Tirada> = Vec::new();
    let mut signo_actual = 1;
    let mut buffer = String::new();
    let mut terminos = Vec::new();
    let mut dados_totales: i32 = 0; // todos los términos juntos, no solo cada uno

    // 1. Separamos términos respetando '+' y '-'
    for c in comando.chars() {
        if c == '+' || c == '-' {
            let limpio = buffer.trim();
            if !limpio.is_empty() {
                terminos.push((signo_actual, limpio.to_string()));
                buffer.clear();
            }
            signo_actual = if c == '+' { 1 } else { -1 };
        } else {
            buffer.push(c);
        }
    }
    let limpio = buffer.trim();
    if !limpio.is_empty() {
        terminos.push((signo_actual, limpio.to_string()));
    }

    if terminos.is_empty() {
        return Err("Comando vacío".into());
    }

    // 2. Parseamos cada término
    for (signo, termino) in terminos {
        // Una variable de la ficha (`f`, `Fuerza`): un término de dados siempre lleva algún dígito
        if variables::parece_variable(&termino) {
            let (abreviatura, valor) = resolver(&termino)?;
            tiradas.push(Tirada {
                signo,
                veces: 0,
                d: 0,
                bonificador: Some(valor),
                variable: Some(abreviatura),
                ventaja: false,
                desventaja: false,
                multiplicador: 1,
            });
            continue;
        }

        // Multiplicador opcional delante de un término de dados: `2*3d20`
        let (multiplicador, termino) = match termino.split_once('*') {
            Some((m, resto)) => {
                let m: i32 = m.trim().parse().map_err(|_| "Multiplicador inválido")?;
                if m <= 0 {
                    return Err("El multiplicador debe ser mayor a 0".into());
                }
                if !resto.contains(|c| c == 'd' || c == 'D') {
                    return Err("El multiplicador solo se aplica a dados".into());
                }
                (m, resto.trim().to_string())
            }
            None => (1, termino),
        };

        if let Some(pos_d) = termino.find(|c| c == 'd' || c == 'D') {
            let (veces_str, resto_str) = termino.split_at(pos_d);
            let resto = resto_str[1..].trim();

            let veces: i32 = if veces_str.trim().is_empty() {
                1
            } else {
                veces_str.trim().parse().map_err(|_| "Cantidad inválida")?
            };

            // Separamos número de caras de modificadores (adv, desv, dis)
            let pos_mods = resto.find(|c: char| !c.is_ascii_digit()).unwrap_or(resto.len());
            let (caras_str, mods_str) = resto.split_at(pos_mods);

            let caras: i32 = caras_str.trim().parse().map_err(|_| "Caras inválidas")?;
            if veces <= 0 || caras <= 0 {
                return Err("Los dados y caras deben ser mayores a 0".into());
            }

            if veces > 999 || dados_totales + veces > 999 {
                return Err("No puedes hacer tiradas de más de 999 de golpe".into());
            }
            dados_totales += veces;

            let mods_lower = mods_str.to_lowercase();
            let ventaja = mods_lower.contains("adv");
            let desventaja = mods_lower.contains("desv") || mods_lower.contains("dis");

            let test_limpio = mods_lower.replace("adv", "").replace("desv", "").replace("dis", "");
            if !test_limpio.trim().is_empty() {
                return Err(format!("Modificador desconocido: '{mods_str}'"));
            }

            // Agrupamos dados idénticos del mismo tipo y signo
            if let Some(existente) = tiradas.iter_mut().find(|t| {
                t.bonificador.is_none()
                    && t.d == caras
                    && t.signo == signo
                    && t.ventaja == ventaja
                    && t.desventaja == desventaja
                    && t.multiplicador == multiplicador
            }) {
                existente.veces += veces;
            } else {
                tiradas.push(Tirada {
                    signo,
                    veces,
                    d: caras,
                    bonificador: None,
                    variable: None,
                    ventaja,
                    desventaja,
                    multiplicador,
                });
            }
        } else {
            let bonif: i32 = termino.parse().map_err(|_| "Número inválido")?;
            tiradas.push(Tirada {
                signo,
                veces: 0,
                d: 0,
                bonificador: Some(bonif),
                variable: None,
                ventaja: false,
                desventaja: false,
                multiplicador: 1,
            });
        }
    }

    Ok(tiradas)
}

/// Devuelve el desglose (sin el total) y el total. Se calcula en i64 y se comprueba al final que cabe
/// en i32: con dados de muchas caras el total se desbordaba (panic en debug, número basura en release).
fn ejecutar_tiradas(tiradas: &[Tirada]) -> Result<(String, i32), String> {
    let mut resultado = String::new();
    let mut total: i64 = 0;
    let mut primero = true;

    for t in tiradas {
        if primero {
            if t.signo < 0 {
                resultado.push_str("- ");
            }
        } else {
            if t.signo < 0 {
                resultado.push_str(" - ");
            } else {
                resultado.push_str(" + ");
            }
        }
        primero = false;

        // Bonificadores numéricos directos
        if let Some(bonificador) = t.bonificador {
            match &t.variable {
                Some(abreviatura) => {
                    let _ = write!(resultado, "{abreviatura} ({bonificador:+})");
                }
                None => {
                    let _ = write!(resultado, "{bonificador}");
                }
            }
            total += i64::from(t.signo) * i64::from(bonificador);
            continue;
        }

        // Encabezado del dado (ej: 1d20, 1d20adv, 1d20desv)
        let mod_tipo = if t.ventaja {
            "adv"
        } else if t.desventaja {
            "desv"
        } else {
            ""
        };
        if t.multiplicador != 1 {
            let _ = write!(resultado, "{}*", t.multiplicador);
        }
        let _ = write!(resultado, "{}d{}{} (", t.veces, t.d, mod_tipo);

        let mut subtotal_dados: i64 = 0;

        for roll_idx in 0..t.veces {
            if roll_idx > 0 {
                resultado.push_str(", ");
            }

            if t.ventaja || t.desventaja {
                // Tiramos 2 dados directamente sin alocar memoria
                let r1 = rand::random_range(1..=t.d);
                let r2 = rand::random_range(1..=t.d);

                let (ganador, perdedor) = if t.ventaja {
                    if r1 >= r2 { (r1, r2) } else { (r2, r1) }
                } else {
                    if r1 <= r2 { (r1, r2) } else { (r2, r1) }
                };

                subtotal_dados += i64::from(ganador);

                // El ganador se muestra en negrita <b>
                if t.veces == 1 {
                    let _ = write!(resultado, "<b>{ganador}</b>, {perdedor}");
                } else {
                    let _ = write!(resultado, "[<b>{ganador}</b>, {perdedor}]");
                }
            } else {
                // Tirada normal directa
                let r = rand::random_range(1..=t.d);
                subtotal_dados += i64::from(r);
                let _ = write!(resultado, "{}", formatear_num(r, t.d));
            }
        }
        resultado.push(')');

        let multiplicado = subtotal_dados
            .checked_mul(i64::from(t.multiplicador))
            .ok_or("El resultado es demasiado grande")?;
        total = total
            .checked_add(i64::from(t.signo) * multiplicado)
            .ok_or("El resultado es demasiado grande")?;
    }

    let total = i32::try_from(total).map_err(|_| "El resultado es demasiado grande".to_string())?;
    Ok((resultado, total))
}

#[tauri::command]
pub fn tirar_dado(
    comando: String,
    publica: Option<bool>,
    id_instancia: Option<u16>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    tirar_dado_de(&state, ID_HOST, &comando, publica.unwrap_or(true), id_instancia)
}

/// La tirada de `jugador` (el host o un jugador de la sala, que la pide por la red). Rust la procesa y la
/// registra con el nombre de quien tira; el chat de tiradas llega así a todos. Un `clean` no es una tirada
/// (`limpiar_chat` es solo del host).
/// `publica`: si los jugadores ven la tirada (solo el host puede tirar en privado).
/// `id_instancia`: la ficha con el foco, de la que salen las variables (`2d6 + f`); solo se mira si hay alguna.
pub fn tirar_dado_de(
    state: &AppState,
    jugador: IdJugador,
    comando: &str,
    publica: bool,
    id_instancia: Option<u16>,
) -> Result<String, String> {
    if jugador != ID_HOST && es_comando_limpiar(comando) {
        return Err("Solo el host puede limpiar el chat".into());
    }
    let tiradas = parsear_comando(comando, &mut |texto| resolver_variable(&state.lock(), jugador, id_instancia, texto))?;
    let (desglose, total) = ejecutar_tiradas(&tiradas)?;

    // 1. Registra en el estado de la partida / deltas (con el total tal cual, sin releerlo de un texto)
    registrar_tirada(state, jugador, &desglose, total, publica || jugador != ID_HOST);

    // 2. El historial personal (↑ ↓) es de quien tira: se guarda en el disco del host, uno por jugador (así lo
    //    recupera al reconectar; se borra al quitarlo de la partida)
    state.almacen.anadir_historial_de(jugador, comando);

    // El front espera "desglose = total"
    Ok(format!("{desglose} = {total}"))
}

/// `clean` o `c` (sin nada más, da igual mayúsculas) es la orden de vaciar el chat de tiradas.
/// Mezclada con otras cosas (`c + 1d6`) no lo es, y como tirada tampoco es válida.
pub fn es_comando_limpiar(comando: &str) -> bool {
    let c = comando.trim();
    c.eq_ignore_ascii_case("clean") || c.eq_ignore_ascii_case("c")
}

/// Vacía el historial de tiradas de la partida (delta `HistorialTiradasLimpiado`). El comando queda en
/// el historial personal (↑ ↓) como cualquier otro.
#[tauri::command]
pub fn limpiar_chat(comando: String, state: State<'_, AppState>) -> Result<(), String> {
    if !es_comando_limpiar(&comando) {
        return Err("No es la orden de limpiar el chat".into());
    }
    procesar_delta(&state, DeltaPartida::HistorialTiradasLimpiado);
    registrar_comando_personal(&state, comando.trim());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tablero_general::modelos::*;
    use crate::tablero_general::variables::pruebas::var;

    /// Sin variables: un término de letras es un error, como antes.
    fn sin_variables(texto: &str) -> Result<(String, i32), String> {
        Err(format!("«{texto}» no es una variable"))
    }

    fn parsear_comando(cmd: &str) -> Result<Vec<Tirada>, String> {
        super::parsear_comando(cmd, &mut sin_variables)
    }

    fn tirar(cmd: &str) -> Result<(String, i32), String> {
        ejecutar_tiradas(&parsear_comando(cmd)?)
    }

    /// Partida con Fuerza (f, 10/2) y Cordura (cor, 7/3), y la instancia 1 (ficha de Ana, jugador 3) con
    /// Fuerza 12 (+1) y Cordura 9 (0); la instancia 2 es solo del host.
    fn partida_con_variables() -> EstadoPartida {
        let mut e = EstadoPartida::default();
        e.definiciones_variables = vec![var(1, "Fuerza", "f", 10, 2), var(2, "Cordura", "cor", 7, 3)];
        let ficha = |id: u16, propietarios: Vec<u16>, fuerza: i16| Entidad {
            id, nombre: format!("F{id}"), tipo: TipoEntidad::Jugador, parametros: vec![],
            variables: vec![ValorVariable { id_definicion: 1, valor: fuerza }, ValorVariable { id_definicion: 2, valor: 9 }],
            cuerpo: Cuerpo::Pequenio, zona_id: 1, foto_url: None, miniatura_url: None, foto_encuadre: Default::default(),
            archivos_adjuntos: vec![], es_unico: false, propietarios,
        };
        let inst = |id: u16, base: Entidad| Instancia {
            id_instancia: id, id_mapa: None, base, valores_actuales: vec![], coordenadas: Coordenada::NoPuesto,
            activo: true, rotacion: 0, espejo: false, orden: 0, capa: 0, atenuada: false,
        };
        e.jugadores = vec![Jugador { id: 3, nombre: "Ana".into() }];
        e.instancias_activas = vec![inst(1, ficha(1, vec![3], 12)), inst(2, ficha(2, vec![], 4))];
        e
    }

    fn tirar_con(e: &EstadoPartida, jugador: IdJugador, foco: Option<u16>, cmd: &str) -> Result<(String, i32), String> {
        ejecutar_tiradas(&super::parsear_comando(cmd, &mut |t| resolver_variable(e, jugador, foco, t))?)
    }

    #[test]
    fn las_variables_de_la_ficha_con_el_foco_suman_su_resultado() {
        let e = partida_con_variables();
        let (d, total) = tirar_con(&e, ID_HOST, Some(1), "2d1 + f").unwrap();
        assert_eq!(total, 3);
        assert_eq!(d, "2d1 (<b>1</b>, <b>1</b>) + f (+1)");
        // Por nombre, en mayúsculas, restando y la de resultado 0
        assert_eq!(tirar_con(&e, ID_HOST, Some(1), "1d1 + FUERZA - cor").unwrap(), ("1d1 (<b>1</b>) + f (+1) - cor (+0)".into(), 2));
        assert_eq!(tirar_con(&e, ID_HOST, Some(1), "f").unwrap().1, 1);
        // La otra ficha: Fuerza 4 → -3
        let (d, total) = tirar_con(&e, ID_HOST, Some(2), "10 + f").unwrap();
        assert_eq!((d.as_str(), total), ("10 + f (-3)", 7));
        // Sin variables el foco no importa
        assert_eq!(tirar_con(&e, ID_HOST, None, "1d1 + 2").unwrap().1, 3);
    }

    #[test]
    fn las_variables_necesitan_foco_existir_y_ser_de_quien_tira() {
        let e = partida_con_variables();
        assert!(tirar_con(&e, ID_HOST, None, "1d20 + f").unwrap_err().contains("foco"));
        assert!(tirar_con(&e, ID_HOST, Some(9), "1d20 + f").unwrap_err().contains("ya no está activa"));
        assert!(tirar_con(&e, ID_HOST, Some(1), "1d20 + fu").unwrap_err().contains("no es una variable"));
        assert_eq!(tirar_con(&e, 3, Some(1), "1d1 + f").unwrap().1, 2, "Ana usa las de su ficha");
        assert!(tirar_con(&e, 3, Some(2), "1d1 + f").unwrap_err().contains("tus fichas"), "pero no las de otra");
        assert!(tirar_con(&e, ID_HOST, Some(1), "2*f").is_err(), "el multiplicador solo vale para dados");
        // Lo escrito no llega al desglose: solo la abreviatura que definió el host
        for malo in ["f<b>", "<script>", "f'", "f;"] {
            assert!(tirar_con(&e, ID_HOST, Some(1), malo).is_err(), "{malo}");
        }
    }

    #[test]
    fn clean_o_c_solos_limpian_el_chat_y_mezclados_no_valen() {
        for cmd in ["c", "C", "clean", " Clean ", "CLEAN"] {
            assert!(es_comando_limpiar(cmd), "{cmd}");
        }
        for cmd in ["c + 1d6", "clean 1d6", "1d6 + c", "cc", "cleanup", ""] {
            assert!(!es_comando_limpiar(cmd), "{cmd}");
            assert!(tirar(cmd).is_err(), "{cmd} tampoco es una tirada");
        }

        let mut e = crate::tablero_general::estado::EstadoPartida::default();
        let t = crate::tablero_general::modelos::Tiradas { total: 3, comando: "1d6 (3)".into(), hora: "10:00".into(), jugador: 0, autor: String::new(), publica: true };
        e.aplicar_delta(&DeltaPartida::TiradaRealizada(t));
        e.aplicar_delta(&DeltaPartida::HistorialTiradasLimpiado);
        assert!(e.historial_tiradas.is_empty());
    }

    #[test]
    fn solo_se_aceptan_tiradas_y_nada_de_lo_escrito_llega_al_desglose() {
        for malo in [
            "<script>alert(1)</script>",
            "1d20 <img src=x onerror=alert(1)>",
            "1d6<b>",
            "hola que tal",
            "1d20adv\"'",
            "2*hola",
        ] {
            assert!(tirar(malo).is_err(), "{malo}");
        }
        // El desglose solo lleva números, palabras fijas y <b>
        let (desglose, _) = tirar("2*3d20adv + 1d4 - 2").unwrap();
        let sin_negritas = desglose.replace("<b>", "").replace("</b>", "");
        assert!(!sin_negritas.contains('<') && !sin_negritas.contains('>'), "{desglose}");

        // Un comando demasiado largo ni se parsea
        let largo = vec!["1"; MAX_LONGITUD_COMANDO / 2 + 1].join("+");
        assert!(largo.len() > MAX_LONGITUD_COMANDO);
        assert!(tirar(&largo).unwrap_err().contains("no puede pasar"));
        assert!(tirar(&vec!["1"; 50].join("+")).is_ok());
    }

    #[test]
    fn un_dado_enorme_da_su_total_real() {
        let (desglose, total) = tirar("1d5346534").unwrap();
        assert!((1..=5_346_534).contains(&total));
        assert_eq!(desglose.replace("<b>", "").replace("</b>", ""), format!("1d5346534 ({total})"));
    }

    #[test]
    fn un_total_que_no_cabe_en_i32_es_un_error_y_no_un_numero_basura() {
        // 999 dados de ~2.000 millones de caras: casi seguro supera i32::MAX (antes: overflow)
        assert_eq!(tirar("999d2000000000").unwrap_err(), "El resultado es demasiado grande");
        assert!(tirar("2000000000 + 2000000000").is_err(), "los bonificadores también suman en i64");
    }

    #[test]
    fn totales_deterministas() {
        assert_eq!(tirar("3d1 + 4").unwrap().1, 7);
        assert_eq!(tirar("3d1 - 4").unwrap().1, -1);
        assert_eq!(tirar("5").unwrap().1, 5);
        assert_eq!(tirar("- 2d1").unwrap().1, -2);
    }

    #[test]
    fn el_limite_de_999_dados_cuenta_todos_los_terminos_y_los_agrupados() {
        assert!(parsear_comando("999d6").is_ok());
        assert!(parsear_comando("1000d6").is_err());
        assert!(parsear_comando("500d6 + 499d8").is_ok());
        assert!(parsear_comando("500d6 + 500d8").is_err());
        assert!(parsear_comando("600d6 + 600d6").is_err(), "iguales: se agruparían en 1200");
    }

    #[test]
    fn el_multiplicador_afecta_solo_a_su_termino_de_dados() {
        assert_eq!(tirar("2*3d1").unwrap().1, 6);
        assert_eq!(tirar("2*3d1 + 4").unwrap().1, 10);
        assert_eq!(tirar("2*3d1 + 2d1").unwrap().1, 8);
        assert_eq!(tirar("10 - 2*3d1").unwrap().1, 4);
        let (d, _) = tirar("2*3d1").unwrap();
        assert!(d.starts_with("2*3d1 ("));
        // con distinto multiplicador no se agrupan
        assert_eq!(parsear_comando("2*1d6 + 3*1d6").unwrap().len(), 2);
    }

    #[test]
    fn el_multiplicador_invalido_es_un_error() {
        for malo in ["0*1d6", "x*1d6", "2*5", "2*", "*1d6"] {
            assert!(parsear_comando(malo).is_err(), "{malo:?}");
        }
        assert!(tirar("2000000000*999d2000000000").is_err());
    }

    #[test]
    fn ventaja_y_desventaja_siguen_funcionando() {
        let (d, t) = tirar("1d1adv").unwrap();
        assert_eq!(t, 1);
        assert!(d.contains("<b>1</b>"));
        assert_eq!(tirar("2d1desv").unwrap().1, 2);
    }

    #[test]
    fn los_errores_de_sintaxis_se_siguen_detectando() {
        for malo in ["", "patata", "1d0", "0d6", "1d6xyz"] {
            assert!(parsear_comando(malo).is_err(), "{malo:?}");
        }
    }
}
