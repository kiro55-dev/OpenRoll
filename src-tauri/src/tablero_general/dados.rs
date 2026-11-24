// src-tauri/src/tablero_general/dados.rs
use std::fmt::Write;
use tauri::State;
use crate::tablero_general::escalas::{self, Operador};
use crate::tablero_general::estado::{DeltaPartida, EstadoPartida};
use crate::tablero_general::jugadores::controla_instancia;
use crate::tablero_general::modelos::{IdJugador, Instancia, ValorParametro, ID_HOST};
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

/// Lo que vale en una tirada la variable que nombra `texto` (por nombre o abreviatura) o el parámetro visual `@nombre`, en la ficha
/// con el foco (`id_instancia`): su etiqueta para el desglose y su valor. Un jugador solo puede usar las de sus fichas.
fn resolver_variable(
    est: &EstadoPartida,
    jugador: IdJugador,
    id_instancia: Option<u16>,
    texto: &str,
) -> Result<(String, i32), String> {
    // Antes de mirar la ficha, que el nombre exista (así el error dice lo que de verdad falla)
    if !texto.trim().starts_with('@') && variables::buscar(&est.definiciones_variables, texto).is_none() {
        return Err(format!("«{}» no es una variable (un parámetro visual se escribe con @: @{})", texto.trim(), texto.trim()));
    }
    let id = id_instancia.ok_or("Elige primero una ficha (púlsala) para usar sus variables o parámetros")?;
    let inst = est.instancias_activas.iter().find(|i| i.id_instancia == id).ok_or("Esa ficha ya no está activa")?;
    if !controla_instancia(est, jugador, id) {
        return Err("Solo puedes usar las variables y los parámetros de tus fichas".into());
    }
    valor_en_ficha(est, inst, texto)
}

/// Lo que vale en la ficha `inst` un término de una tirada que no es un dado ni un número:
///  * una VARIABLE (solo letras, por nombre o abreviatura): su resultado (una derivada suma sus bases y sus opcionales marcadas);
///    en el desglose, su abreviatura o, sin ella, su nombre;
///  * un PARÁMETRO VISUAL con `@` delante (`@velocidad`, `@rango arma`: el nombre llega hasta el siguiente operador, sin distinguir
///    mayúsculas): su valor. Con máximo (la vida) cuenta el valor ACTUAL de esta instancia, y `@vida.max` el máximo de la ficha. Uno
///    de texto no se puede tirar.
pub(crate) fn valor_en_ficha(est: &EstadoPartida, inst: &Instancia, texto: &str) -> Result<(String, i32), String> {
    if let Some(nombre) = texto.trim().strip_prefix('@') {
        return valor_de_parametro(est, inst, nombre);
    }
    let def = variables::buscar(&est.definiciones_variables, texto).ok_or_else(|| format!("«{}» no es una variable", texto.trim()))?;
    let nombre = if def.abreviatura.is_empty() { &def.nombre } else { &def.abreviatura };
    Ok((nombre.clone(), variables::resultado(&est.definiciones_variables, &inst.base.parametros, &inst.base.variables, def)))
}

/// Un nombre de parámetro para compararlo: sin mayúsculas y con los espacios de dentro reducidos a uno.
pub(crate) fn clave_parametro(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// El valor del parámetro visual `nombre` (lo que sigue a la `@`) en la ficha `inst`. Ver `valor_en_ficha`.
pub(crate) fn valor_de_parametro(est: &EstadoPartida, inst: &Instancia, nombre: &str) -> Result<(String, i32), String> {
    let buscado = clave_parametro(nombre);
    if buscado.is_empty() {
        return Err("Falta el nombre del parámetro detrás de la @".into());
    }
    let por_nombre = |k: &str| est.definiciones_parametros.iter().find(|d| clave_parametro(&d.clave) == k);
    // `vida.max` es el máximo; se mira antes el nombre entero, por si algún parámetro se llamara así
    let (def, maximo) = match por_nombre(&buscado) {
        Some(d) => (d, false),
        None => match buscado.strip_suffix(".max").map(str::trim_end).and_then(por_nombre) {
            Some(d) => (d, true),
            None => return Err(format!("«@{}» no es un parámetro", nombre.trim())),
        },
    };
    if !matches!(def.valor_por_defecto, ValorParametro::Numero(_)) {
        return Err(format!("«{}» es un texto: no se puede usar en una tirada", def.clave));
    }
    if maximo && !def.tiene_maximo {
        return Err(format!("«{}» no tiene máximo: usa @{}", def.clave, def.clave));
    }
    let de_la_ficha = variables::valor_de_parametro(&inst.base.parametros, def.id);
    let valor = if def.tiene_maximo && !maximo {
        inst.valores_actuales.iter().find(|v| v.id_definicion == def.id).map_or(de_la_ficha, |v| i32::from(v.valor))
    } else {
        de_la_ficha
    };
    // El nombre lo escribe el host: al desglose (HTML) no llega nada que pueda pasar por una etiqueta
    let limpio: String = def.clave.chars().filter(|c| !matches!(c, '<' | '>' | '&' | '"')).collect();
    Ok((if maximo { format!("{limpio} máx") } else { limpio }, valor))
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

/// Solo acepta tiradas: números, `d`, `+`, `-`, `*` y `adv`/`desv`/`dis`, variables (un término solo de letras) y parámetros
/// visuales (un término que empieza por `@`), que `resolver` convierte en (etiqueta, valor). Cualquier otro texto es un error, así que nada
/// de lo escrito llega al desglose (que lo forma `ejecutar_tiradas` con números, palabras fijas y la abreviatura
/// que definió el host, que solo lleva letras).
pub(crate) fn parsear_comando(
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
        // Una variable de la ficha (`f`, `Fuerza`: un término de dados siempre lleva algún dígito) o un parámetro visual (`@vida`)
        if variables::parece_variable(&termino) || termino.starts_with('@') {
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
#[cfg(test)]
fn ejecutar_tiradas(tiradas: &[Tirada]) -> Result<(String, i32), String> {
    ejecutar_con_natural(tiradas).map(|(desglose, total, _)| (desglose, total))
}

/// Como `ejecutar_tiradas`, y además el resultado natural `(cara, caras)` cuando la tirada es UN solo dado normal, sin
/// ventaja ni multiplicador (lo que hace falta para los críticos y las pifias de una tirada `vs`).
fn ejecutar_con_natural(tiradas: &[Tirada]) -> Result<(String, i32, Option<(i32, i32)>), String> {
    let mut tirar = |caras: i32| rand::random_range(1..=caras);
    let mut natural: Option<(i32, i32)> = None;
    let mut resultado = String::new();
    let mut total: i64 = 0;
    let mut primero = true;

    for t in tiradas {
        if primero {
            if t.signo < 0 {
                resultado.push_str("- ");
            }
        } else if t.signo < 0 {
            resultado.push_str(" - ");
        } else {
            resultado.push_str(" + ");
        }
        primero = false;

        let (texto, valor, nat) = tirar_termino(t, &mut tirar)?;
        resultado.push_str(&texto);
        if nat.is_some() {
            natural = nat;
        }
        total = total.checked_add(i64::from(t.signo) * valor).ok_or("El resultado es demasiado grande")?;
    }

    let total = i32::try_from(total).map_err(|_| "El resultado es demasiado grande".to_string())?;
    let un_dado_normal = tiradas.iter().filter(|t| t.bonificador.is_none()).count() == 1
        && tiradas.iter().any(|t| t.bonificador.is_none() && t.veces == 1 && t.signo > 0 && t.multiplicador == 1 && !t.ventaja && !t.desventaja);
    Ok((resultado, total, if un_dado_normal { natural } else { None }))
}

/// Tira UN término (sin su signo): su texto para el desglose, su valor (sin signo) y la última cara de un dado normal. `tirar(caras)`
/// da una cara entre 1 y `caras`: en el chat es aleatoria; las macros (macros/) pasan la suya, y sus pruebas una con semilla.
pub(crate) fn tirar_termino(t: &Tirada, tirar: &mut dyn FnMut(i32) -> i32) -> Result<(String, i64, Option<(i32, i32)>), String> {
    let mut resultado = String::new();
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
        return Ok((resultado, i64::from(bonificador), None));
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

    let mut natural = None;
    let mut subtotal_dados: i64 = 0;
    for roll_idx in 0..t.veces {
        if roll_idx > 0 {
            resultado.push_str(", ");
        }
        if t.ventaja || t.desventaja {
            // Dos dados: con ventaja cuenta el mayor, con desventaja el menor
            let r1 = tirar(t.d);
            let r2 = tirar(t.d);
            let (ganador, perdedor) = if t.ventaja {
                if r1 >= r2 { (r1, r2) } else { (r2, r1) }
            } else if r1 <= r2 {
                (r1, r2)
            } else {
                (r2, r1)
            };
            subtotal_dados += i64::from(ganador);
            // El ganador se muestra en negrita <b>
            if t.veces == 1 {
                let _ = write!(resultado, "<b>{ganador}</b>, {perdedor}");
            } else {
                let _ = write!(resultado, "[<b>{ganador}</b>, {perdedor}]");
            }
        } else {
            let r = tirar(t.d);
            natural = Some((r, t.d));
            subtotal_dados += i64::from(r);
            let _ = write!(resultado, "{}", formatear_num(r, t.d));
        }
    }
    resultado.push(')');
    let multiplicado = subtotal_dados.checked_mul(i64::from(t.multiplicador)).ok_or("El resultado es demasiado grande")?;
    Ok((resultado, multiplicado, natural))
}

/// Parte un comando en sus dos lados por una comparación suelta: `<=`, `>=`, `<`, `>` o `vs` (azúcar de `<=`): `1d100 vs acro`,
/// `1d20 + 2 >= 12`. Sin comparación, todo es tirada. Un `vs` pegado a letras (parte de un nombre) no cuenta, solo puede haber una
/// comparación y le tienen que quedar los dos lados escritos.
pub(crate) fn dividir_comparacion(comando: &str) -> Result<(&str, Option<(Operador, &str)>), String> {
    let mut encontrada: Option<(usize, usize, Operador)> = None; // (dónde empieza, cuánto mide, cuál)
    let mut poner = |inicio: usize, largo: usize, op: Operador| -> Result<(), String> {
        if encontrada.is_some() {
            return Err("Solo puede haber una comparación (<=, >= o vs) en la tirada".into());
        }
        encontrada = Some((inicio, largo, op));
        Ok(())
    };
    let mut saltar = 0;
    for (i, c) in comando.char_indices() {
        if i < saltar {
            continue;
        }
        let resto = &comando[i..];
        if resto.starts_with("<=") {
            poner(i, 2, Operador::MenorIgual)?;
            saltar = i + 2;
        } else if resto.starts_with(">=") {
            poner(i, 2, Operador::MayorIgual)?;
            saltar = i + 2;
        } else if c == '<' {
            poner(i, 1, Operador::Menor)?;
        } else if c == '>' {
            poner(i, 1, Operador::Mayor)?;
        } else if (c == 'v' || c == 'V') && resto.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("vs")) {
            let antes = comando[..i].chars().next_back().is_some_and(char::is_alphabetic);
            let despues = comando[i + 2..].chars().next().is_some_and(char::is_alphabetic);
            if !antes && !despues {
                poner(i, 2, Operador::MenorIgual)?;
                saltar = i + 2;
            }
        }
    }
    let Some((i, largo, op)) = encontrada else { return Ok((comando, None)) };
    let (izquierda, derecha) = (&comando[..i], &comando[i + largo..]);
    if izquierda.trim().is_empty() {
        return Err("Falta la tirada a la izquierda de la comparación".into());
    }
    if derecha.trim().is_empty() {
        return Err("Falta el objetivo de la comparación (un número o una variable)".into());
    }
    Ok((izquierda, Some((op, derecha))))
}

/// Un lado de una comparación, ya tirado.
pub(crate) struct Lado {
    pub(crate) desglose: String,
    pub(crate) total: i32,
    /// `(cara, caras)` si el total sale de UN solo dado normal.
    natural: Option<(i32, i32)>,
    /// ¿Lleva algún dado? (un número o una variable no tiran nada)
    con_dados: bool,
}

pub(crate) fn tirar_lado(tiradas: &[Tirada]) -> Result<Lado, String> {
    let (desglose, total, natural) = ejecutar_con_natural(tiradas)?;
    Ok(Lado { desglose, total, natural, con_dados: tiradas.iter().any(|t| t.bonificador.is_none()) })
}

/// Lo que se compara: el SUJETO (el lado con dados; el izquierdo si los dos los llevan) contra el otro lado. Devuelve el total del
/// sujeto, el objetivo, el operador visto desde el sujeto (`12 >= 1d20` es `1d20 <= 12`) y la cara natural del sujeto.
fn sujeto_y_objetivo(izquierda: &Lado, op: Operador, derecha: &Lado) -> (i32, i32, Operador, Option<(i32, i32)>) {
    if izquierda.con_dados || !derecha.con_dados {
        (izquierda.total, derecha.total, op, izquierda.natural)
    } else {
        (derecha.total, izquierda.total, op.invertido(), derecha.natural)
    }
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
/// `publica`: si los demás jugadores ven la tirada (el host y quien la hace la ven siempre).
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
    if comando.chars().count() > MAX_LONGITUD_COMANDO {
        return Err(format!("El comando no puede pasar de {MAX_LONGITUD_COMANDO} caracteres"));
    }
    // `Empezar [tirada]` y `Terminar` no son tiradas: son órdenes de batalla (solo del host)
    if let Some(orden) = crate::tablero_general::batalla::interpretar_comando(comando) {
        return crate::tablero_general::batalla::ejecutar_comando(state, jugador, comando, orden);
    }
    // `1d100 <= acro`, `1d20 >= 12`, `1d100 vs 45`: dos lados que se tiran por separado y se comparan
    let (izquierda, comparacion) = dividir_comparacion(comando)?;
    let mut resolver = |texto: &str| resolver_variable(&state.lock(), jugador, id_instancia, texto);
    let lado_izquierdo = tirar_lado(&parsear_comando(izquierda, &mut resolver)?)?;

    let (desglose, total, valoracion) = match comparacion {
        None => (lado_izquierdo.desglose, lado_izquierdo.total, None),
        Some((op, derecha)) => {
            let lado_derecho = tirar_lado(&parsear_comando(derecha, &mut resolver)?)?;
            let (total, objetivo, op_sujeto, natural) = sujeto_y_objetivo(&lado_izquierdo, op, &lado_derecho);
            let valoracion = escalas::valorar(state.lock().escala_resultado.as_ref(), op_sujeto, total, objetivo, natural);
            (format!("{} {} {}", lado_izquierdo.desglose, op.simbolo(), lado_derecho.desglose), total, Some(valoracion))
        }
    };

    // 1. Registra en el estado de la partida / deltas (con el total tal cual, sin releerlo de un texto)
    // Cada uno elige si la suya es pública: una privada de un jugador la ven él y el host (`vista_para`)
    registrar_tirada(state, jugador, &desglose, total, publica, valoracion.as_ref());

    // 2. El historial personal (↑ ↓) es de quien tira: se guarda en el disco del host, uno por jugador (así lo
    //    recupera al reconectar; se borra al quitarlo de la partida)
    state.almacen.anadir_historial_de(jugador, comando);

    // El front espera "desglose = total"
    match valoracion {
        None => Ok(format!("{desglose} = {total}")),
        Some(v) => Ok(format!("{desglose} = {total} · {}", v.nombre)),
    }
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
            variables: vec![ValorVariable { id_definicion: 1, valor: fuerza, opcionales: vec![], eleccion: 0, parametros_marcados: vec![] }, ValorVariable { id_definicion: 2, valor: 9, opcionales: vec![], eleccion: 0, parametros_marcados: vec![] }],
            pasos_valor: vec![],
            cuerpo: Cuerpo::Pequenio, zona_id: 1, foto_url: None, miniatura_url: None, foto_encuadre: Default::default(),
            archivos_adjuntos: vec![], es_unico: false, vision: 0, plantilla_id: None, actuales: vec![], propietarios,
        };
        let inst = |id: u16, base: Entidad| Instancia {
            id_instancia: id, id_mapa: None, base, valores_actuales: vec![], coordenadas: Coordenada::NoPuesto,
            activo: true, rotacion: 0, espejo: false, orden: 0, capa: 0, oculto: false, atenuada: false,
        };
        e.jugadores = vec![Jugador { id: 3, nombre: "Ana".into() }];
        e.instancias_activas = vec![inst(1, ficha(1, vec![3], 12)), inst(2, ficha(2, vec![], 4))];
        e
    }

    /// La partida de `partida_con_variables` con los parámetros de serie (Vida con máximo, Armadura, Velocidad, Rango arma) y uno de
    /// texto («Notas»). La instancia 1 tiene Vida 20 de máximo y 7 de actual, Armadura 15, Velocidad 9 y Rango arma 3.
    fn partida_con_parametros() -> EstadoPartida {
        let mut e = partida_con_variables();
        let mut defs = parametros_por_defecto();
        defs.push(DefinicionParametro {
            id: 9, clave: "Notas".into(), valor_por_defecto: ValorParametro::Texto(String::new()), tiene_maximo: false,
            representar_mapa: None, visibilidad: Visibilidad::Variable, unidad: None, atenuar_si_negativo: false, calculo: None,
        });
        e.definiciones_parametros = defs;
        let num = |id, n| Parametro { id_definicion: id, valor: ValorParametro::Numero(n), publico: true };
        e.instancias_activas[0].base.parametros = vec![
            num(ID_VIDA, 20), num(ID_ARMADURA, 15), num(ID_VELOCIDAD, 9), num(ID_RANGO_ARMA, 3),
            Parametro { id_definicion: 9, valor: ValorParametro::Texto("x".into()), publico: true },
        ];
        e.instancias_activas[0].valores_actuales = vec![ValorActual { id_definicion: ID_VIDA, valor: 7 }];
        e
    }

    #[test]
    fn un_parametro_visual_se_tira_con_arroba() {
        let e = partida_con_parametros();
        let mut r = |t: &str| resolver_variable(&e, ID_HOST, Some(1), t);
        let tirar_con = |cmd: &str, r: &mut dyn FnMut(&str) -> Result<(String, i32), String>| ejecutar_tiradas(&super::parsear_comando(cmd, r)?);
        // Sin distinguir mayúsculas, y un nombre con espacio llega hasta el siguiente operador
        assert_eq!(tirar_con("@velocidad", &mut r).unwrap(), ("Velocidad (+9)".into(), 9));
        assert_eq!(tirar_con("@VELOCIDAD + @rango arma + 1", &mut r).unwrap().1, 9 + 3 + 1);
        assert_eq!(tirar_con("@Rango  Arma", &mut r).unwrap().1, 3, "los espacios de más no cuentan");
        // Con máximo: el valor ACTUAL de la instancia; `.max`, el de la ficha
        assert_eq!(tirar_con("@vida", &mut r).unwrap(), ("Vida (+7)".into(), 7));
        assert_eq!(tirar_con("@vida.max", &mut r).unwrap(), ("Vida máx (+20)".into(), 20));
        assert!(tirar_con("@armadura.max", &mut r).unwrap_err().contains("no tiene máximo"));
        // Uno de texto, uno que no existe o una @ sola, no
        assert!(tirar_con("@notas", &mut r).unwrap_err().contains("texto"));
        assert!(tirar_con("@inventado", &mut r).unwrap_err().contains("no es un parámetro"));
        assert!(tirar_con("1d6 + @", &mut r).is_err());
        // Sin @ un parámetro no es nada (las variables siguen sin @): el error lo dice
        let e2 = tirar_con("velocidad", &mut r).unwrap_err();
        assert!(e2.contains("@velocidad"), "{e2}");
    }

    #[test]
    fn un_parametro_con_arroba_sigue_las_reglas_de_las_variables() {
        let e = partida_con_parametros();
        let tirar_como = |jugador, inst: Option<u16>, cmd: &str| {
            let mut r = |t: &str| resolver_variable(&e, jugador, inst, t);
            ejecutar_tiradas(&super::parsear_comando(cmd, &mut r)?)
        };
        assert_eq!(tirar_como(3, Some(1), "@armadura").unwrap().1, 15, "su dueña");
        assert!(tirar_como(3, Some(2), "@armadura").is_err(), "la de otro");
        assert!(tirar_como(ID_HOST, None, "@armadura").is_err(), "sin foco");
    }

    #[test]
    fn las_comparaciones_separan_los_lados_sin_confundirse_con_nombres() {
        use Operador::{Mayor, MayorIgual, Menor, MenorIgual};
        assert_eq!(dividir_comparacion("1d20 > 12").unwrap(), ("1d20 ", Some((Mayor, " 12"))));
        assert_eq!(dividir_comparacion("1d100<45").unwrap(), ("1d100", Some((Menor, "45"))));
        assert_eq!(dividir_comparacion("1d20+2>=1d20").unwrap(), ("1d20+2", Some((MayorIgual, "1d20"))), "el = no cuenta como otra comparación");
        assert_eq!(dividir_comparacion("1d20 <= 5").unwrap().1.unwrap().0, MenorIgual);
        assert_eq!(dividir_comparacion("1d100 vs 45").unwrap(), ("1d100 ", Some((MenorIgual, " 45"))));
        assert_eq!(dividir_comparacion("1d100VS acro").unwrap(), ("1d100", Some((MenorIgual, " acro"))));
        assert_eq!(dividir_comparacion("1d100vs45").unwrap(), ("1d100", Some((MenorIgual, "45"))));
        assert_eq!(dividir_comparacion("1d100 <= 45").unwrap(), ("1d100 ", Some((MenorIgual, " 45"))));
        assert_eq!(dividir_comparacion("1d20 + 2>=12").unwrap(), ("1d20 + 2", Some((MayorIgual, "12"))));
        assert_eq!(dividir_comparacion("12 >= 1d20").unwrap(), ("12 ", Some((MayorIgual, " 1d20"))));
        assert_eq!(dividir_comparacion("1d20 + avs").unwrap(), ("1d20 + avs", None), "pegado a letras es un nombre");
        assert_eq!(dividir_comparacion("1d20 + vsa").unwrap(), ("1d20 + vsa", None));
        assert_eq!(dividir_comparacion("1d6").unwrap(), ("1d6", None));
        for dos in ["1d100 vs 40 vs 30", "1d20 >= 10 <= 15", "1d20 >= 10 vs 3", "1d20 <= <= 3", "1d20 < 5 > 2", "1d20 <> 5"] {
            assert!(dividir_comparacion(dos).is_err(), "{dos}");
        }
        // Le tienen que quedar los dos lados
        for (sin_lado, falta) in [("1d20 >=", "objetivo"), ("1d20 vs  ", "objetivo"), (">= 12", "izquierda"), ("< 3", "izquierda")] {
            let e = dividir_comparacion(sin_lado).unwrap_err();
            assert!(e.contains(falta), "{sin_lado}: {e}");
        }
    }

    #[test]
    fn el_sujeto_es_el_lado_con_dados_y_el_izquierdo_si_los_dos_tiran() {
        use Operador::{MayorIgual, MenorIgual};
        let lado = |total, con_dados, natural| Lado { desglose: String::new(), total, natural, con_dados };
        let dado = lado(37, true, Some((37, 100)));
        let numero = lado(45, false, None);
        assert_eq!(sujeto_y_objetivo(&dado, MenorIgual, &numero), (37, 45, MenorIgual, Some((37, 100))));
        // `12 >= 1d20` es `1d20 <= 12`: el sujeto pasa a la derecha y el operador se invierte
        assert_eq!(sujeto_y_objetivo(&numero, MayorIgual, &dado), (37, 45, MenorIgual, Some((37, 100))));
        // Los dos tiran: manda el izquierdo (tirada enfrentada)
        let otro = lado(20, true, Some((20, 20)));
        assert_eq!(sujeto_y_objetivo(&dado, MayorIgual, &otro), (37, 20, MayorIgual, Some((37, 100))));
        // Ninguno tira (`5 >= 3`): el izquierdo
        assert_eq!(sujeto_y_objetivo(&numero, MayorIgual, &lado(3, false, None)), (45, 3, MayorIgual, None));
    }

    #[test]
    fn solo_un_dado_normal_da_resultado_natural() {
        let natural = |cmd: &str| ejecutar_con_natural(&parsear_comando(cmd).unwrap()).unwrap().2;
        let (cara, caras) = natural("1d100").unwrap();
        assert!((1..=100).contains(&cara) && caras == 100);
        assert!(natural("1d100 + 5").is_some(), "el bonificador no quita el natural");
        assert!(natural("2d6").is_none());
        assert!(natural("1d20 + 1d4").is_none());
        assert!(natural("1d20adv").is_none());
        assert!(natural("2*1d6").is_none());
        assert!(natural("7").is_none());
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
    fn una_derivada_suma_sus_bases_y_la_opcional_si_la_ficha_la_marca() {
        use crate::tablero_general::variables::pruebas::{derivada, grupos};
        let mut e = partida_con_variables(); // Fuerza 12 (+1), Cordura 9 (0)
        e.grupos_variables = grupos();
        e.definiciones_variables.push(var(3, "Competencia", "com", 0, 1));
        e.definiciones_variables.push(derivada(4, "Atletismo", "atl", &[1], &[3]));
        e.ajustar_a_definiciones();
        e.instancias_activas[0].base.variables[2].valor = 2; // competencia +2
        assert_eq!(tirar_con(&e, ID_HOST, Some(1), "1d1 + atl").unwrap(), ("1d1 (<b>1</b>) + atl (+1)".into(), 2), "sin el check");
        e.instancias_activas[0].base.variables[3].opcionales = vec![3];
        assert_eq!(tirar_con(&e, ID_HOST, Some(1), "1d1 + Atletismo").unwrap(), ("1d1 (<b>1</b>) + atl (+3)".into(), 4), "con él");
    }

    #[test]
    fn las_variables_necesitan_foco_existir_y_ser_de_quien_tira() {
        let e = partida_con_variables();
        assert!(tirar_con(&e, ID_HOST, None, "1d20 + f").unwrap_err().contains("Elige primero una ficha"));
        // Sin abreviatura se tira por el nombre y el desglose lo lleva
        let mut sin = e.clone();
        sin.definiciones_variables[0].abreviatura.clear();
        assert_eq!(tirar_con(&sin, ID_HOST, Some(1), "fuerza").unwrap(), ("Fuerza (+1)".into(), 1));
        assert!(tirar_con(&sin, ID_HOST, Some(1), "f").is_err());
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
        let t = crate::tablero_general::modelos::Tiradas { total: 3, comando: "1d6 (3)".into(), hora: "10:00".into(), jugador: 0, autor: String::new(), publica: true, veredicto: String::new(), acierta: None, especial: false, informativa: false };
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
