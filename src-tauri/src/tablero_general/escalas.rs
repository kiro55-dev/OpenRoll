// src-tauri/src/tablero_general/escalas.rs
//
// La ESCALA DE RESULTADOS: cómo se nombra el resultado de una comparación del chat (`1d100 <= 45`, `1d20 >= 12`, `1d100 vs acro`).
// El chat solo compara (total, ¿cumple?, cara natural del dado); darle nombre («Éxito difícil», «Crítico», «Pifia»...) es de cada
// rol, así que la escala es parte del conjunto de parámetros (`ConjuntoParametros.escala_resultado`) y el chat no sabe nada de
// ningún juego. Sin escala, el resultado se llama solo «Éxito» o «Fallo».
//
// Este módulo es puro (sin Tauri) salvo los comandos del final, que son solo del host (nada en `PETICIONES`).
use serde::Serialize;
use tauri::State;

use crate::tablero_general::estado::DeltaPartida;
use crate::tablero_general::jugadores::caracter_enganoso;
use crate::tablero_general::modelos::{EfectoNatural, EscalaResultado, ReglaNatural, TramoEscala};
use crate::tablero_general::partida::{registrar, AppState};

pub const MAX_TRAMOS: usize = 6;
pub const MAX_REGLAS_NATURALES: usize = 10;
pub const MAX_NOMBRE_RESULTADO: usize = 30;
/// Tope de caras de un dado en una regla, y de las fracciones de un tramo.
pub const MAX_CARAS_REGLA: u16 = 1000;
pub const MAX_FRACCION: i16 = 100;
/// Tope del umbral de una regla (el mismo de los valores de las variables).
pub const MAX_UMBRAL: i32 = 9999;

const EXITO: &str = "Éxito";
const FALLO: &str = "Fallo";

/// En qué sentido se compara: por debajo (`<`, `<=`: tirar bajo la habilidad, como Cthulhu) o por encima (`>`, `>=`: superar un
/// umbral, como una CD de D&D). Con `=` se acierta igualando; sin él, hay que pasarse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operador {
    Menor,
    MenorIgual,
    Mayor,
    MayorIgual,
}

impl Operador {
    /// El mismo operador con los lados cambiados de sitio (`12 >= 1d20` es `1d20 <= 12`; `12 > 1d20` es `1d20 < 12`).
    pub fn invertido(self) -> Self {
        match self {
            Operador::Menor => Operador::Mayor,
            Operador::MenorIgual => Operador::MayorIgual,
            Operador::Mayor => Operador::Menor,
            Operador::MayorIgual => Operador::MenorIgual,
        }
    }

    /// Cómo se escribe (para el desglose de la tirada).
    pub fn simbolo(self) -> &'static str {
        match self {
            Operador::Menor => "<",
            Operador::MenorIgual => "<=",
            Operador::Mayor => ">",
            Operador::MayorIgual => ">=",
        }
    }

    /// ¿Se acierta quedándose por debajo del objetivo? (los niveles de una escala solo valen así)
    pub fn por_debajo(self) -> bool {
        matches!(self, Operador::Menor | Operador::MenorIgual)
    }
}

/// El resultado de valorar una comparación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Valoracion {
    /// Cómo se llama (el nombre de la escala, o «Éxito» / «Fallo» sin ella).
    pub nombre: String,
    pub acierta: bool,
    /// Salió de una regla del dado natural (crítico, pifia...) y no del nivel normal.
    pub especial: bool,
}

/// Valora `total` frente a `objetivo` con el operador `op`. `natural` es `(cara, caras)` cuando el total sale de UN solo dado normal.
/// Por debajo (`<`, `<=`), un objetivo de 0 o menos siempre falla.
pub fn valorar(escala: Option<&EscalaResultado>, op: Operador, total: i32, objetivo: i32, natural: Option<(i32, i32)>) -> Valoracion {
    let mut acierta = match op {
        Operador::Menor => objetivo > 0 && total < objetivo,
        Operador::MenorIgual => objetivo > 0 && total <= objetivo,
        Operador::Mayor => total > objetivo,
        Operador::MayorIgual => total >= objetivo,
    };
    let Some(escala) = escala else {
        return Valoracion { nombre: (if acierta { EXITO } else { FALLO }).into(), acierta, especial: false };
    };
    if let Some((cara, caras)) = natural {
        for r in &escala.naturales {
            let casa = r.dado.is_none_or(|d| i32::from(d) == caras)
                && (i32::from(r.desde)..=i32::from(r.hasta)).contains(&cara)
                && r.umbral_menor_que.is_none_or(|u| objetivo < u);
            let aplica = match r.efecto {
                EfectoNatural::SoloSiAcierta => acierta,
                EfectoNatural::SoloSiFalla => !acierta,
                EfectoNatural::ForzarAcierto | EfectoNatural::ForzarFallo => true,
            };
            if casa && aplica {
                match r.efecto {
                    EfectoNatural::ForzarAcierto => acierta = true,
                    EfectoNatural::ForzarFallo => acierta = false,
                    _ => {}
                }
                return Valoracion { nombre: r.nombre.clone(), acierta, especial: true };
            }
        }
    }
    if !acierta {
        return Valoracion { nombre: escala.nombre_fallo.clone(), acierta, especial: false };
    }
    // Los niveles de acierto solo cuentan al tirar por debajo (qué parte del objetivo se alcanza)
    if op.por_debajo() {
        let nivel = escala.tramos.iter().find(|t| {
            i64::from(total) * i64::from(t.denominador.max(1)) <= i64::from(objetivo) * i64::from(t.numerador)
        });
        if let Some(t) = nivel {
            return Valoracion { nombre: t.nombre.clone(), acierta, especial: false };
        }
    }
    Valoracion { nombre: escala.nombre_exito.clone(), acierta, especial: false }
}

fn nombre_valido(nombre: &str, que: &str) -> Result<String, String> {
    let n = nombre.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() || n.chars().count() > MAX_NOMBRE_RESULTADO || n.chars().any(|c| c.is_control() || caracter_enganoso(c)) {
        return Err(format!("{que}: el nombre debe tener entre 1 y {MAX_NOMBRE_RESULTADO} caracteres, sin símbolos raros"));
    }
    Ok(n)
}

/// Comprueba una escala (entrada no confiable: puede venir de un archivo importado) y la devuelve con los nombres limpios.
pub fn validar(e: EscalaResultado) -> Result<EscalaResultado, String> {
    let nombre_exito = nombre_valido(&e.nombre_exito, "Acierto")?;
    let nombre_fallo = nombre_valido(&e.nombre_fallo, "Fallo")?;
    if e.tramos.len() > MAX_TRAMOS {
        return Err(format!("Una escala no puede tener más de {MAX_TRAMOS} niveles"));
    }
    if e.naturales.len() > MAX_REGLAS_NATURALES {
        return Err(format!("Una escala no puede tener más de {MAX_REGLAS_NATURALES} reglas del dado"));
    }
    let mut tramos: Vec<TramoEscala> = Vec::new();
    for t in e.tramos {
        let nombre = nombre_valido(&t.nombre, "Nivel")?;
        if !(1..=MAX_FRACCION).contains(&t.numerador) || !(1..=MAX_FRACCION).contains(&t.denominador) || t.numerador > t.denominador {
            return Err(format!("«{nombre}»: la fracción debe ser de 1 a {MAX_FRACCION} y no pasar de 1 (p. ej. 1/5)"));
        }
        // Del más estricto al más laxo: si no, el primero que casa taparía a los demás
        if let Some(a) = tramos.last() {
            if i32::from(t.numerador) * i32::from(a.denominador) <= i32::from(a.numerador) * i32::from(t.denominador) {
                return Err(format!("«{nombre}» debe ser menos estricto que «{}» (ordénalos de menor a mayor fracción)", a.nombre));
            }
        }
        tramos.push(TramoEscala { nombre, ..t });
    }
    let mut naturales: Vec<ReglaNatural> = Vec::new();
    for r in e.naturales {
        let nombre = nombre_valido(&r.nombre, "Regla del dado")?;
        if r.desde == 0 || r.desde > r.hasta || r.hasta > MAX_CARAS_REGLA {
            return Err(format!("«{nombre}»: las caras deben ir de 1 a {MAX_CARAS_REGLA} y «desde» no puede pasar de «hasta»"));
        }
        if r.dado.is_some_and(|d| d < 2 || d > MAX_CARAS_REGLA || r.hasta > d) {
            return Err(format!("«{nombre}»: el dado debe tener entre 2 y {MAX_CARAS_REGLA} caras y contener esas caras"));
        }
        if r.umbral_menor_que.is_some_and(|u| u.abs() > MAX_UMBRAL) {
            return Err(format!("«{nombre}»: el umbral debe estar entre -{MAX_UMBRAL} y {MAX_UMBRAL}"));
        }
        naturales.push(ReglaNatural { nombre, ..r });
    }
    Ok(EscalaResultado { nombre_exito, nombre_fallo, tramos, naturales })
}

// ------------------------------------------------------------------
// Ejemplos (datos para empezar; el chat no depende de ellos)
// ------------------------------------------------------------------

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct EjemploEscala {
    pub clave: &'static str,
    pub nombre: &'static str,
    pub descripcion: &'static str,
    pub escala: EscalaResultado,
}

fn tramo(nombre: &str, numerador: i16, denominador: i16) -> TramoEscala {
    TramoEscala { nombre: nombre.into(), numerador, denominador }
}

fn regla(nombre: &str, dado: Option<u16>, desde: u16, hasta: u16, efecto: EfectoNatural, umbral_menor_que: Option<i32>) -> ReglaNatural {
    ReglaNatural { nombre: nombre.into(), dado, desde, hasta, efecto, umbral_menor_que }
}

/// Escalas de ejemplo para rellenar el editor: se COPIAN, la partida no depende de este código.
pub fn ejemplos() -> Vec<EjemploEscala> {
    vec![
        EjemploEscala {
            clave: "cthulhu",
            nombre: "Llamada de Cthulhu",
            descripcion: "Tira un d100 por debajo de la habilidad: extremo (1/5), difícil (1/2), crítico con un 1 y pifia con el 100 (del 96 si la habilidad es menor de 50).",
            escala: EscalaResultado {
                nombre_exito: "Éxito".into(),
                nombre_fallo: "Fallo".into(),
                tramos: vec![tramo("Éxito extremo", 1, 5), tramo("Éxito difícil", 1, 2)],
                naturales: vec![
                    regla("Crítico", Some(100), 1, 1, EfectoNatural::SoloSiAcierta, None),
                    regla("Pifia", Some(100), 96, 100, EfectoNatural::SoloSiFalla, Some(50)),
                    regla("Pifia", Some(100), 100, 100, EfectoNatural::SoloSiFalla, None),
                ],
            },
        },
        EjemploEscala {
            clave: "dnd",
            nombre: "D&D (d20 contra una CD)",
            descripcion: "Se supera una dificultad con `>=`: un 20 natural acierta siempre y un 1 natural falla siempre.",
            escala: EscalaResultado {
                nombre_exito: "Superado".into(),
                nombre_fallo: "Fallado".into(),
                tramos: vec![],
                naturales: vec![
                    regla("Crítico", Some(20), 20, 20, EfectoNatural::ForzarAcierto, None),
                    regla("Pifia", Some(20), 1, 1, EfectoNatural::ForzarFallo, None),
                ],
            },
        },
    ]
}

// ------------------------------------------------------------------
// Comandos (solo del host)
// ------------------------------------------------------------------

#[tauri::command]
pub fn obtener_escala_resultado(state: State<'_, AppState>) -> Option<EscalaResultado> {
    state.lock().escala_resultado.clone()
}

#[tauri::command]
pub fn obtener_ejemplos_escala() -> Vec<EjemploEscala> {
    ejemplos()
}

/// Guarda (o quita, con `None`) la escala del conjunto activo. Se valida entera; sin cambios no se registra nada.
#[tauri::command]
pub fn guardar_escala_resultado(escala: Option<EscalaResultado>, state: State<'_, AppState>) -> Result<(), String> {
    guardar_escala_resultado_en(escala, &state)
}

pub fn guardar_escala_resultado_en(escala: Option<EscalaResultado>, state: &AppState) -> Result<(), String> {
    let escala = escala.map(validar).transpose()?;
    let mut est = state.lock();
    if est.escala_resultado == escala {
        return Ok(());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::EscalaResultadoCambiada(escala));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use Operador::{Mayor, MayorIgual, Menor, MenorIgual};

    fn cthulhu() -> EscalaResultado {
        ejemplos().into_iter().find(|e| e.clave == "cthulhu").unwrap().escala
    }
    fn dnd() -> EscalaResultado {
        ejemplos().into_iter().find(|e| e.clave == "dnd").unwrap().escala
    }

    #[test]
    fn sin_escala_solo_hay_exito_o_fallo() {
        let v = |op, total, objetivo, nat| valorar(None, op, total, objetivo, nat);
        assert_eq!(v(MenorIgual, 45, 45, None), Valoracion { nombre: "Éxito".into(), acierta: true, especial: false });
        assert_eq!(v(MenorIgual, 46, 45, None).nombre, "Fallo");
        assert_eq!(v(MenorIgual, 1, 0, None).nombre, "Fallo", "con un objetivo de 0 siempre falla");
        assert_eq!(v(MayorIgual, 12, 12, None).nombre, "Éxito");
        assert_eq!(v(MayorIgual, 11, 12, None).nombre, "Fallo");
        // Sin escala ni los críticos ni los niveles cuentan
        assert_eq!(v(MenorIgual, 1, 60, Some((1, 100))).nombre, "Éxito");
    }

    #[test]
    fn la_escala_de_cthulhu_da_los_mismos_resultados_que_el_codigo_de_antes() {
        let c = cthulhu();
        let v = |total, objetivo, natural| valorar(Some(&c), MenorIgual, total, objetivo, natural).nombre;
        // Objetivo 60: la mitad es 30 y la quinta parte 12
        assert_eq!(v(60, 60, Some((60, 100))), "Éxito");
        assert_eq!(v(31, 60, Some((31, 100))), "Éxito");
        assert_eq!(v(30, 60, Some((30, 100))), "Éxito difícil");
        assert_eq!(v(12, 60, Some((12, 100))), "Éxito extremo");
        assert_eq!(v(61, 60, Some((61, 100))), "Fallo");
        assert_eq!(v(1, 60, Some((1, 100))), "Crítico");
        assert_eq!(v(100, 60, Some((100, 100))), "Pifia");
        // Con un objetivo menor de 50, del 96 en adelante es pifia en un d100
        assert_eq!(v(96, 45, Some((96, 100))), "Pifia");
        assert_eq!(v(96, 60, Some((96, 100))), "Fallo");
        // Sin un único dado normal no hay crítico ni pifia
        assert_eq!(v(1, 60, None), "Éxito extremo");
        assert_eq!(v(100, 60, None), "Fallo");
        // Un objetivo de 0 o menos siempre falla; un crítico necesita acertar
        assert_eq!(v(1, 0, Some((1, 100))), "Fallo");
        assert_eq!(v(11, 5, Some((1, 100))), "Fallo");
        // La regla es de un d100: en un d20 no hay crítico ni pifia
        assert_eq!(v(1, 14, Some((1, 20))), "Éxito extremo");
        assert_eq!(v(20, 14, Some((20, 20))), "Fallo");
        // El valor `especial` distingue la regla del nivel
        assert!(valorar(Some(&c), MenorIgual, 1, 60, Some((1, 100))).especial);
        assert!(!valorar(Some(&c), MenorIgual, 12, 60, Some((12, 100))).especial);
    }

    #[test]
    fn la_escala_de_dnd_fuerza_el_20_y_el_1_con_mayor_o_igual() {
        let e = dnd();
        let v = |total, cd, natural| valorar(Some(&e), MayorIgual, total, cd, natural);
        assert_eq!(v(12, 12, Some((12, 20))).nombre, "Superado");
        assert_eq!(v(11, 12, Some((11, 20))).nombre, "Fallado");
        // Un 20 natural acierta aunque el total no llegue (y un 1 falla aunque llegue), con un modificador
        let critico = v(20, 30, Some((20, 20)));
        assert_eq!((critico.nombre.as_str(), critico.acierta, critico.especial), ("Crítico", true, true));
        let pifia = v(1, -5, Some((1, 20)));
        assert_eq!((pifia.nombre.as_str(), pifia.acierta, pifia.especial), ("Pifia", false, true));
        // Los niveles de una escala solo valen al tirar por debajo: con >= no hay
        let mut con_niveles = e.clone();
        con_niveles.tramos = vec![tramo("Holgado", 1, 2)];
        assert_eq!(valorar(Some(&con_niveles), MayorIgual, 5, 3, None).nombre, "Superado");
    }

    #[test]
    fn un_nivel_exacto_en_el_borde_y_el_orden_de_las_reglas() {
        let mut e = cthulhu();
        // 1/5 de 60 = 12 (incluido), 1/2 de 61 = 30,5: 30 entra y 31 no
        assert_eq!(valorar(Some(&e), MenorIgual, 12, 60, None).nombre, "Éxito extremo");
        assert_eq!(valorar(Some(&e), MenorIgual, 30, 61, None).nombre, "Éxito difícil");
        assert_eq!(valorar(Some(&e), MenorIgual, 31, 61, None).nombre, "Éxito");
        // La primera regla que casa gana
        e.naturales.insert(0, regla("Suerte", None, 1, 1, EfectoNatural::SoloSiAcierta, None));
        assert_eq!(valorar(Some(&e), MenorIgual, 1, 60, Some((1, 100))).nombre, "Suerte");
    }

    #[test]
    fn los_ejemplos_son_validos_y_la_validacion_rechaza_lo_roto() {
        for e in ejemplos() {
            assert_eq!(validar(e.escala.clone()).as_ref(), Ok(&e.escala), "{}", e.clave);
        }
        let ok = cthulhu();
        let mal = |f: &dyn Fn(&mut EscalaResultado)| {
            let mut e = ok.clone();
            f(&mut e);
            validar(e)
        };
        assert!(mal(&|e| e.nombre_exito = "  ".into()).is_err());
        assert!(mal(&|e| e.nombre_fallo = "x".repeat(31)).is_err());
        assert!(mal(&|e| e.nombre_exito = "a\u{202e}b".into()).is_err(), "caracteres engañosos");
        assert!(mal(&|e| e.tramos[0].numerador = 0).is_err());
        assert!(mal(&|e| e.tramos[0].denominador = 101).is_err());
        assert!(mal(&|e| e.tramos[1].numerador = 3).is_err(), "una fracción mayor que 1");
        assert!(mal(&|e| e.tramos.swap(0, 1)).is_err(), "del más estricto al más laxo");
        assert!(mal(&|e| e.tramos = (0..7).map(|i| tramo(&format!("N{i}"), i + 1, 100)).collect()).is_err(), "demasiados niveles");
        assert!(mal(&|e| e.naturales[0].desde = 0).is_err());
        assert!(mal(&|e| e.naturales[0].desde = 5).is_err(), "desde > hasta");
        assert!(mal(&|e| e.naturales[1].hasta = 101).is_err(), "más caras que el dado");
        assert!(mal(&|e| e.naturales[0].dado = Some(1)).is_err());
        assert!(mal(&|e| e.naturales[1].umbral_menor_que = Some(10_000)).is_err());
        assert!(mal(&|e| e.naturales = (0..11).map(|i| regla(&format!("R{i}"), None, 1, 1, EfectoNatural::SoloSiFalla, None)).collect()).is_err());
        // Los nombres se limpian (espacios de más)
        let limpia = mal(&|e| e.nombre_exito = "  Muy   bien ".into()).unwrap();
        assert_eq!(limpia.nombre_exito, "Muy bien");
    }

    #[test]
    fn el_operador_se_invierte_al_cambiar_los_lados() {
        assert_eq!(MenorIgual.invertido(), MayorIgual);
        assert_eq!(MayorIgual.invertido(), MenorIgual);
        assert_eq!(Menor.invertido(), Mayor);
        assert_eq!(Mayor.invertido(), Menor);
        assert_eq!([Menor, MenorIgual, Mayor, MayorIgual].map(Operador::simbolo), ["<", "<=", ">", ">="]);
        assert_eq!([Menor, MenorIgual, Mayor, MayorIgual].map(Operador::por_debajo), [true, true, false, false]);
    }

    #[test]
    fn los_estrictos_no_aciertan_en_el_borde() {
        let v = |op, total, objetivo| valorar(None, op, total, objetivo, None).acierta;
        assert!(v(MenorIgual, 45, 45) && !v(Menor, 45, 45) && v(Menor, 44, 45));
        assert!(v(MayorIgual, 12, 12) && !v(Mayor, 12, 12) && v(Mayor, 13, 12));
        assert!(!v(Menor, 1, 0), "por debajo de un objetivo de 0 no se acierta");
        // Los niveles de una escala valen también con `<`
        let c = cthulhu();
        assert_eq!(valorar(Some(&c), Menor, 11, 60, None).nombre, "Éxito extremo");
        assert_eq!(valorar(Some(&c), Menor, 60, 60, None).nombre, "Fallo");
        // Y no con `>`
        assert_eq!(valorar(Some(&c), Mayor, 13, 12, None).nombre, "Éxito");
    }

    #[test]
    fn una_escala_se_lee_aunque_falten_los_campos_opcionales() {
        let e: EscalaResultado = serde_json::from_str(r#"{"nombre_exito":"Sí","nombre_fallo":"No"}"#).unwrap();
        assert!(e.tramos.is_empty() && e.naturales.is_empty());
        let r: ReglaNatural = serde_json::from_str(r#"{"nombre":"X","desde":1,"hasta":1,"efecto":"SoloSiFalla"}"#).unwrap();
        assert_eq!((r.dado, r.umbral_menor_que), (None, None));
    }
}
