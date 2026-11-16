// src-tauri/src/tablero_general/conjuntos_serie.rs
//
// Los conjuntos de parámetros que trae el programa, definidos como DATOS en código: D&D 5e, Pathfinder 2e y La llamada de Cthulhu 7e
// (`conjuntos::integrados` los lista, junto a «Genérico»). Nada de aquí es lógica del chat ni de las fichas: son definiciones de
// variables, parámetros y escala de resultados que pasan por la misma validación que un conjunto importado (hay un test que lo comprueba).
//
// Reglas que condicionan cómo se escriben (ver variables.rs): el nombre de una variable solo lleva letras y espacios y es único entre
// TODAS las variables (por eso las salvaciones de D&D se llaman «Salvación de Fuerza»), la abreviatura son ≤ 10 letras, una derivada solo
// depende de variables BASE y una ficha nueva nace con el valor neutro de cada base.
use crate::tablero_general::escalas;
use crate::tablero_general::modelos::*;

const CARACTERISTICAS: u16 = 1;

/// Una variable base. Con neutro 10 y escala 2 su resultado es el modificador de D&D (12 → +1); con 0 y 1, es su valor.
fn base(id: u16, nombre: &str, abreviatura: &str, grupo: u16, neutro: i16, escala: i16) -> DefinicionVariable {
    DefinicionVariable {
        id,
        nombre: nombre.into(),
        abreviatura: abreviatura.into(),
        neutro,
        escala,
        grupo,
        bases: vec![],
        opcionales: vec![],
        multiplicador: 1,
        divisor: 1,
        constante: 0,
        opciones: vec![],
    }
}

/// Una derivada de UNA característica (con las opcionales que se indiquen: la competencia de D&D, el nivel de Pathfinder).
fn derivada(id: u16, nombre: &str, abreviatura: &str, grupo: u16, de: u16, opcionales: &[u16]) -> DefinicionVariable {
    DefinicionVariable { bases: vec![de], opcionales: opcionales.to_vec(), ..base(id, nombre, abreviatura, grupo, 0, 1) }
}

fn grupo(id: u16, nombre: &str, tipo: TipoGrupoVariables) -> GrupoVariables {
    GrupoVariables { id, nombre: nombre.into(), tipo }
}

fn escala(clave: &str) -> EscalaResultado {
    escalas::ejemplos().into_iter().find(|e| e.clave == clave).expect("la escala de ejemplo existe").escala
}

/// Un parámetro numérico (los de texto no hacen falta aquí).
#[allow(clippy::too_many_arguments)]
fn parametro(
    id: u16,
    clave: &str,
    valor: i16,
    tiene_maximo: bool,
    representar_mapa: Option<Forma>,
    visibilidad: Visibilidad,
    unidad: Option<&str>,
    atenuar_si_negativo: bool,
    calculo: Option<CalculoParametro>,
) -> DefinicionParametro {
    DefinicionParametro {
        id,
        clave: clave.into(),
        valor_por_defecto: ValorParametro::Numero(valor),
        tiene_maximo,
        representar_mapa,
        visibilidad,
        unidad: unidad.map(Into::into),
        atenuar_si_negativo,
        calculo,
    }
}

/// Las seis características de D&D y Pathfinder, en su orden: nombre y abreviatura.
const SEIS: [(&str, &str); 6] = [
    ("Fuerza", "fue"),
    ("Destreza", "des"),
    ("Constitución", "con"),
    ("Inteligencia", "int"),
    ("Sabiduría", "sab"),
    ("Carisma", "car"),
];

/// Posición (1..=6) de una característica por su abreviatura: el id de la variable en D&D y en Pathfinder.
fn id_de(abreviatura: &str) -> u16 {
    SEIS.iter().position(|(_, a)| *a == abreviatura).map(|i| i as u16 + 1).expect("característica conocida")
}

// ------------------------------------------------------------------
// D&D 5e
// ------------------------------------------------------------------

/// Las habilidades de D&D 5e: nombre, abreviatura y característica.
const HABILIDADES_DND: [(&str, &str, &str); 18] = [
    ("Acrobacias", "acro", "des"),
    ("Atletismo", "atl", "fue"),
    ("Arcanos", "arc", "int"),
    ("Engaño", "eng", "car"),
    ("Historia", "his", "int"),
    ("Interpretación", "interp", "car"),
    ("Intimidación", "intim", "car"),
    ("Investigación", "inv", "int"),
    ("Juego de manos", "jdm", "des"),
    ("Medicina", "med", "sab"),
    ("Naturaleza", "nat", "int"),
    ("Percepción", "perc", "sab"),
    ("Perspicacia", "persp", "sab"),
    ("Persuasión", "pers", "car"),
    ("Religión", "rel", "int"),
    ("Sigilo", "sig", "des"),
    ("Supervivencia", "sup", "sab"),
    ("Trato con animales", "tan", "sab"),
];

/// D&D 5e: las seis características y la Competencia (variables base), las tiradas de salvación y las habilidades (derivadas, con la
/// competencia como opcional: un check «+ com» en la ficha). Los parámetros visuales son los de siempre.
pub fn dnd5e() -> ConjuntoParametros {
    const SALVACIONES: u16 = 2;
    const HABILIDADES: u16 = 3;
    let competencia = 7;

    let mut variables: Vec<DefinicionVariable> =
        SEIS.iter().enumerate().map(|(i, (n, a))| base(i as u16 + 1, n, a, CARACTERISTICAS, 10, 2)).collect();
    // La Competencia es una base más (junto a las características): una derivada solo puede tener bases como opcionales
    variables.push(base(competencia, "Competencia", "com", CARACTERISTICAS, 0, 1));

    let mut id = competencia;
    for (nombre, abreviatura) in SEIS {
        id += 1;
        variables.push(derivada(id, &format!("Salvación de {nombre}"), &format!("s{abreviatura}"), SALVACIONES, id_de(abreviatura), &[competencia]));
    }
    for (nombre, abreviatura, caracteristica) in HABILIDADES_DND {
        id += 1;
        variables.push(derivada(id, nombre, abreviatura, HABILIDADES, id_de(caracteristica), &[competencia]));
    }

    ConjuntoParametros {
        nombre: "D&D 5e".into(),
        descripcion: "Las seis características y la competencia, las tiradas de salvación y las 18 habilidades (con su check «+ com»). Vida, armadura, velocidad y rango de arma como siempre.".into(),
        parametros: parametros_por_defecto(),
        grupos_variables: vec![
            grupo(CARACTERISTICAS, "Características", TipoGrupoVariables::Base),
            grupo(SALVACIONES, "Tiradas de salvación", TipoGrupoVariables::Derivadas),
            grupo(HABILIDADES, "Habilidades", TipoGrupoVariables::Derivadas),
        ],
        variables,
        escala_resultado: Some(escala("dnd")),
    }
}

// ------------------------------------------------------------------
// Pathfinder 2e
// ------------------------------------------------------------------

/// Las habilidades de Pathfinder 2e: nombre, abreviatura y característica.
const HABILIDADES_PATHFINDER: [(&str, &str, &str); 18] = [
    ("Acrobacias", "acro", "des"),
    ("Arcanos", "arc", "int"),
    ("Artesanía", "art", "int"),
    ("Atletismo", "atl", "fue"),
    ("Diplomacia", "dip", "car"),
    ("Engaño", "eng", "car"),
    ("Interpretación", "interp", "car"),
    ("Intimidación", "intim", "car"),
    ("Latrocinio", "lat", "des"),
    ("Medicina", "med", "sab"),
    ("Naturaleza", "nat", "sab"),
    ("Ocultismo", "ocu", "int"),
    ("Percepción", "perc", "sab"),
    ("Religión", "rel", "sab"),
    ("Saber", "saber", "int"),
    ("Sigilo", "sig", "des"),
    ("Sociedad", "soc", "int"),
    ("Supervivencia", "sup", "sab"),
];

/// Pathfinder 2e: cada salvación y habilidad es su característica + el rango que elige cada ficha (Sin entrenar +0 … Legendario +8) y, si
/// está marcado el check «+ niv», el nivel (que solo se suma en las entrenadas o mejores). Sin escala: sus grados de éxito dependen del
/// margen sobre la CD, que la escala de resultados no expresa.
pub fn pathfinder2e() -> ConjuntoParametros {
    const SALVACIONES: u16 = 2;
    const HABILIDADES: u16 = 3;
    let nivel = 7;
    let rangos = || {
        [("Sin entrenar", 0), ("Entrenado", 2), ("Experto", 4), ("Maestro", 6), ("Legendario", 8)]
            .map(|(nombre, bonus)| OpcionVariable { nombre: nombre.into(), bonus })
            .to_vec()
    };
    let con_rangos = |d: DefinicionVariable| DefinicionVariable { opciones: rangos(), ..d };

    let mut variables: Vec<DefinicionVariable> =
        SEIS.iter().enumerate().map(|(i, (n, a))| base(i as u16 + 1, n, a, CARACTERISTICAS, 10, 2)).collect();
    variables.push(base(nivel, "Nivel", "niv", CARACTERISTICAS, 0, 1));

    let mut id = nivel;
    for (nombre, abreviatura, caracteristica) in [("Fortaleza", "fort", "con"), ("Reflejos", "refl", "des"), ("Voluntad", "vol", "sab")] {
        id += 1;
        variables.push(con_rangos(derivada(id, nombre, abreviatura, SALVACIONES, id_de(caracteristica), &[nivel])));
    }
    for (nombre, abreviatura, caracteristica) in HABILIDADES_PATHFINDER {
        id += 1;
        variables.push(con_rangos(derivada(id, nombre, abreviatura, HABILIDADES, id_de(caracteristica), &[nivel])));
    }

    let por_defecto = parametros_por_defecto();
    ConjuntoParametros {
        nombre: "Pathfinder 2e".into(),
        descripcion: "Las seis características y el nivel; salvaciones y habilidades con su rango (Sin entrenar … Legendario) y un check «+ niv» para las entrenadas.".into(),
        parametros: vec![
            por_defecto[0].clone(),
            DefinicionParametro { clave: "CA".into(), ..por_defecto[1].clone() },
            DefinicionParametro { valor_por_defecto: ValorParametro::Numero(9), ..por_defecto[2].clone() },
        ],
        grupos_variables: vec![
            grupo(CARACTERISTICAS, "Características", TipoGrupoVariables::Base),
            grupo(SALVACIONES, "Salvaciones", TipoGrupoVariables::Derivadas),
            grupo(HABILIDADES, "Habilidades", TipoGrupoVariables::Derivadas),
        ],
        variables,
        escala_resultado: None,
    }
}

// ------------------------------------------------------------------
// La llamada de Cthulhu 7e
// ------------------------------------------------------------------

/// Las habilidades de Cthulhu: nombre y abreviatura. Todas son porcentajes (base con neutro 0 y escala 1: el resultado es el valor).
const HABILIDADES_CTHULHU: [(&str, &str); 39] = [
    ("Antropología", "antro"),
    ("Arqueología", "arq"),
    ("Pistola", "pist"),
    ("Rifle y escopeta", "rifle"),
    ("Arte y artesanía", "arte"),
    ("Charlatanería", "char"),
    ("Ciencias", "cien"),
    ("Conducir", "cond"),
    ("Descubrir", "desc"),
    ("Disfrazarse", "disf"),
    ("Electricidad", "elec"),
    ("Encanto", "enc"),
    ("Equitación", "equi"),
    ("Escuchar", "esc"),
    ("Esquivar", "esq"),
    ("Historia", "hist"),
    ("Intimidar", "inti"),
    ("Juego de manos", "jdm"),
    ("Lanzar", "lanz"),
    ("Lengua propia", "lprop"),
    ("Lengua extranjera", "lext"),
    ("Leyes", "ley"),
    ("Mecánica", "mec"),
    ("Medicina", "med"),
    ("Mitos de Cthulhu", "mito"),
    ("Naturaleza", "nat"),
    ("Nadar", "nad"),
    ("Ocultismo", "ocu"),
    ("Orientarse", "orie"),
    ("Persuasión", "pers"),
    ("Pilotar", "pil"),
    ("Primeros auxilios", "pauxi"),
    ("Psicoanálisis", "psico"),
    ("Psicología", "psic"),
    ("Saltar", "salt"),
    ("Sigilo", "sig"),
    ("Tasación", "tas"),
    ("Trepar", "trep"),
    ("Pelea", "pel"),
];

/// La llamada de Cthulhu 7e: las ocho características y las habilidades son variables base con el valor tal cual (`1d100 vs esq`), la
/// vida y los puntos de magia se CALCULAN con las características y la escala de resultados trae los niveles de éxito y las pifias.
/// Una ficha nueva nace con todo a 0 (no hay «valor inicial» aparte del neutro): hay que escribir los porcentajes.
pub fn cthulhu7e() -> ConjuntoParametros {
    const HABILIDADES: u16 = 2;
    let caracteristicas = [
        ("Fuerza", "fue"),
        ("Constitución", "con"),
        ("Tamaño", "tam"),
        ("Destreza", "des"),
        ("Apariencia", "apa"),
        ("Inteligencia", "int"),
        ("Poder", "pod"),
        ("Educación", "edu"),
    ];
    let mut variables: Vec<DefinicionVariable> =
        caracteristicas.iter().enumerate().map(|(i, (n, a))| base(i as u16 + 1, n, a, CARACTERISTICAS, 0, 1)).collect();
    for (i, (nombre, abreviatura)) in HABILIDADES_CTHULHU.iter().enumerate() {
        variables.push(base((caracteristicas.len() + i) as u16 + 1, nombre, abreviatura, HABILIDADES, 0, 1));
    }
    let (constitucion, tamano, poder) = (2, 3, 7);

    let por_defecto = parametros_por_defecto();
    let vida = DefinicionParametro {
        atenuar_si_negativo: true,
        calculo: Some(CalculoParametro { variables: vec![constitucion, tamano], multiplicador: 1, divisor: 10, constante: 0 }),
        ..por_defecto[0].clone()
    };
    let resumen = |r, g, b| Some(Forma::Resumen(vec![r, g, b, 255]));
    ConjuntoParametros {
        nombre: "La llamada de Cthulhu 7e".into(),
        descripcion: "Características y habilidades en porcentaje (tira un d100 por debajo con `vs`), vida y puntos de magia calculados, y niveles de éxito, críticos y pifias.".into(),
        parametros: vec![
            vida,
            parametro(
                2,
                "Puntos de magia",
                10,
                true,
                resumen(59, 130, 246),
                Visibilidad::Variable,
                None,
                false,
                Some(CalculoParametro { variables: vec![poder], multiplicador: 1, divisor: 5, constante: 0 }),
            ),
            // Con la cordura a 0 el investigador queda inutilizado (o muerto): la ficha se ve gris, como con la vida
            parametro(3, "Cordura", 50, true, resumen(168, 85, 247), Visibilidad::Variable, None, true, None),
            parametro(4, "Suerte", 50, false, None, Visibilidad::Variable, None, false, None),
            // Cuánto se mueve: se dibuja en el tablero (un rombo, como la velocidad de serie) al seleccionar la ficha
            parametro(5, "Movimiento", 8, false, Some(Forma::Rombo(vec![59, 130, 246, 90])), Visibilidad::SiemprePublico, None, false, None),
        ],
        grupos_variables: vec![
            grupo(CARACTERISTICAS, "Características", TipoGrupoVariables::Base),
            grupo(HABILIDADES, "Habilidades", TipoGrupoVariables::Base),
        ],
        variables,
        escala_resultado: Some(escala("cthulhu")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tablero_general::variables::{resultado, MAX_VARIABLES};

    fn por_abreviatura<'a>(c: &'a ConjuntoParametros, a: &str) -> &'a DefinicionVariable {
        c.variables.iter().find(|v| v.abreviatura == a).unwrap_or_else(|| panic!("no hay «{a}»"))
    }

    /// Los valores de una ficha con lo que se indica (`abreviatura`, valor) y el resto en su neutro.
    fn valores(c: &ConjuntoParametros, puestos: &[(&str, i16)]) -> Vec<ValorVariable> {
        c.variables
            .iter()
            .map(|d| {
                let valor = puestos.iter().find(|(a, _)| *a == d.abreviatura).map_or(d.neutro, |(_, v)| *v);
                ValorVariable { id_definicion: d.id, valor, opcionales: vec![], eleccion: 0 }
            })
            .collect()
    }

    fn marcar(c: &ConjuntoParametros, v: &mut [ValorVariable], derivada: &str, opcional: &str) {
        let (d, o) = (por_abreviatura(c, derivada).id, por_abreviatura(c, opcional).id);
        v.iter_mut().find(|x| x.id_definicion == d).unwrap().opcionales.push(o);
    }

    fn r(c: &ConjuntoParametros, v: &[ValorVariable], a: &str) -> i32 {
        resultado(&c.variables, v, por_abreviatura(c, a))
    }

    #[test]
    fn dnd_reproduce_la_hoja_de_la_captura() {
        let c = dnd5e();
        // Fue 8, Des 17, Con 14, Int 18, Sab 16, Car 12 y Competencia 3
        let mut v = valores(&c, &[("fue", 8), ("des", 17), ("con", 14), ("int", 18), ("sab", 16), ("car", 12), ("com", 3)]);
        for (a, esperado) in [("fue", -1), ("des", 3), ("con", 2), ("int", 4), ("sab", 3), ("car", 1), ("com", 3)] {
            assert_eq!(r(&c, &v, a), esperado, "{a}");
        }
        // Tiradas de salvación, sin marcar la competencia
        for (a, esperado) in [("sfue", -1), ("sdes", 3), ("scon", 2), ("sint", 4), ("ssab", 3), ("scar", 1)] {
            assert_eq!(r(&c, &v, a), esperado, "{a}");
        }
        // Habilidades: las marcadas de la captura suman la competencia
        for a in ["acro", "eng", "inv", "jdm", "perc", "persp", "pers", "rel", "sig"] {
            marcar(&c, &mut v, a, "com");
        }
        for (a, esperado) in [
            ("acro", 6), ("atl", -1), ("arc", 4), ("eng", 4), ("his", 4), ("interp", 1), ("intim", 1), ("inv", 7), ("jdm", 6),
            ("med", 3), ("nat", 4), ("perc", 6), ("persp", 6), ("pers", 4), ("rel", 7), ("sig", 6), ("sup", 3), ("tan", 3),
        ] {
            assert_eq!(r(&c, &v, a), esperado, "{a}");
        }
        // Estructura: tres grupos, la competencia con las características y cada derivada con su característica y la competencia opcional
        assert_eq!(c.grupos_variables.iter().map(|g| g.nombre.as_str()).collect::<Vec<_>>(), ["Características", "Tiradas de salvación", "Habilidades"]);
        assert_eq!(c.variables.len(), 6 + 1 + 6 + 18);
        assert_eq!(por_abreviatura(&c, "com").grupo, CARACTERISTICAS);
        let competencia = por_abreviatura(&c, "com").id;
        assert!(c.variables.iter().filter(|d| d.grupo != CARACTERISTICAS).all(|d| d.bases.len() == 1 && d.opcionales == [competencia]));
        assert_eq!(por_abreviatura(&c, "acro").bases, [por_abreviatura(&c, "des").id]);
        assert_eq!(por_abreviatura(&c, "ssab").bases, [por_abreviatura(&c, "sab").id]);
        // Los parámetros visuales son los de siempre, y la escala es la de D&D
        assert_eq!(c.parametros, parametros_por_defecto());
        assert_eq!(c.escala_resultado, Some(escala("dnd")));
    }

    #[test]
    fn pathfinder_suma_caracteristica_rango_y_nivel_si_se_marca() {
        let c = pathfinder2e();
        let mut v = valores(&c, &[("des", 14), ("niv", 3)]);
        let acro = por_abreviatura(&c, "acro").clone();
        assert_eq!(acro.opciones.iter().map(|o| (o.nombre.as_str(), o.bonus)).collect::<Vec<_>>(),
            [("Sin entrenar", 0), ("Entrenado", 2), ("Experto", 4), ("Maestro", 6), ("Legendario", 8)]);
        // Sin entrenar: solo la característica (el nivel no se suma)
        assert_eq!(r(&c, &v, "acro"), 2);
        // Experto y con el nivel marcado: característica + nivel + rango
        marcar(&c, &mut v, "acro", "niv");
        v.iter_mut().find(|x| x.id_definicion == acro.id).unwrap().eleccion = 2;
        assert_eq!(r(&c, &v, "acro"), 2 + 3 + 4);
        // Cada habilidad tiene su rango propio
        assert_eq!(r(&c, &v, "sig"), 2, "Sigilo (Des) sigue sin entrenar");
        // Salvaciones: Fortaleza con Constitución, Reflejos con Destreza, Voluntad con Sabiduría
        assert_eq!(por_abreviatura(&c, "fort").bases, [por_abreviatura(&c, "con").id]);
        assert_eq!(por_abreviatura(&c, "refl").bases, [por_abreviatura(&c, "des").id]);
        assert_eq!(por_abreviatura(&c, "vol").bases, [por_abreviatura(&c, "sab").id]);
        assert_eq!(c.variables.len(), 6 + 1 + 3 + 18);
        assert!(c.variables.iter().filter(|d| d.grupo != CARACTERISTICAS).all(|d| d.opciones.len() == 5 && d.opcionales == [por_abreviatura(&c, "niv").id]));
        assert_eq!(c.escala_resultado, None);
        assert_eq!(c.parametros.iter().map(|p| p.clave.as_str()).collect::<Vec<_>>(), ["Vida", "CA", "Velocidad"]);
    }

    #[test]
    fn cthulhu_calcula_vida_y_puntos_de_magia_y_las_habilidades_valen_su_porcentaje() {
        use crate::tablero_general::variables::calcular_parametro;
        let c = cthulhu7e();
        let v = valores(&c, &[("con", 60), ("tam", 55), ("pod", 65), ("esq", 45)]);
        let calculo = |clave: &str| c.parametros.iter().find(|p| p.clave == clave).unwrap().calculo.clone().unwrap();
        assert_eq!(calcular_parametro(&calculo("Vida"), &c.variables, &v), 11, "(60 + 55) ÷ 10");
        assert_eq!(calcular_parametro(&calculo("Puntos de magia"), &c.variables, &v), 13, "65 ÷ 5");
        assert_eq!(r(&c, &v, "esq"), 45, "la habilidad es su valor entero, para `1d100 vs esq`");
        assert_eq!(r(&c, &v, "pod"), 65);
        assert_eq!(c.variables.len(), 8 + HABILIDADES_CTHULHU.len());
        assert_eq!(c.grupos_variables.iter().map(|g| g.tipo).collect::<Vec<_>>(), [TipoGrupoVariables::Base, TipoGrupoVariables::Base]);
        assert_eq!(c.escala_resultado, Some(escala("cthulhu")));
        // La vida atenúa la ficha a 0 o menos, y es la primera (id 1: la migración y los valores actuales dependen de él)
        assert!(c.parametros[0].atenuar_si_negativo && c.parametros[0].id == ID_VIDA && c.parametros[0].tiene_maximo);
        // La cordura a 0 también deja la ficha en gris, y el movimiento se dibuja en el tablero
        let cordura = c.parametros.iter().find(|p| p.clave == "Cordura").unwrap();
        assert!(cordura.tiene_maximo && cordura.atenuar_si_negativo);
        let movimiento = c.parametros.iter().find(|p| p.clave == "Movimiento").unwrap();
        assert!(matches!(movimiento.representar_mapa, Some(Forma::Rombo(_))));
        assert!(!c.parametros.iter().any(|p| p.clave == "Suerte" && (p.atenuar_si_negativo || p.representar_mapa.is_some())));
    }

    #[test]
    fn los_conjuntos_de_serie_caben_en_los_topes_y_no_repiten_ids() {
        for c in [dnd5e(), pathfinder2e(), cthulhu7e()] {
            assert!(c.variables.len() <= MAX_VARIABLES, "{}", c.nombre);
            assert!(c.parametros.len() <= 40, "{}", c.nombre);
            for (i, v) in c.variables.iter().enumerate() {
                assert!(c.variables[..i].iter().all(|o| o.id != v.id), "{}: id repetido {}", c.nombre, v.id);
            }
            assert!(c.descripcion.chars().count() <= 300, "{}", c.nombre);
        }
    }
}
