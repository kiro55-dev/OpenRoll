// src-tauri/src/tablero_general/variables.rs
//
// Las variables de las fichas (Fuerza «f», Inteligencia «in»...). El host define qué variables existen
// (`DefinicionVariable`: nombre, abreviatura, valor neutro y escala) y cada ficha pone su valor
// (`ValorVariable`). Lo que cuenta en una tirada del chat (`2d6 + f`) es el RESULTADO:
// `floor((valor − neutro) / escala)`, que se calcula aquí y nunca se guarda.
//
// Nombre y abreviatura son solo letras (y espacios dentro del nombre): así se distinguen de los dados y
// números en el chat, y nada de lo que escribe el host puede meter HTML en el desglose de una tirada.
// Mismo espejo en el front: src/lib/variables.ts.
use crate::tablero_general::modelos::{DefinicionVariable, ValorVariable};

pub const MAX_VARIABLES: usize = 40;
pub const MAX_NOMBRE_VARIABLE: usize = 30;
pub const MAX_ABREVIATURA_VARIABLE: usize = 10;
pub const MAX_ESCALA_VARIABLE: i16 = 100;
/// Tope del valor de una variable en una ficha y de su valor neutro (en positivo y en negativo).
pub const LIMITE_VALOR_VARIABLE: i16 = 9999;
/// Palabras del chat que no pueden ser variables: limpiar el chat, el dado y la ventaja/desventaja.
pub const PALABRAS_RESERVADAS: [&str; 6] = ["c", "clean", "d", "adv", "desv", "dis"];

/// El resultado de una variable con ese valor: cuántas veces cabe la escala en lo que se aparta del neutro,
/// redondeado hacia abajo (como en D&D: 12 → +1, 9 → −1 con neutro 10 y escala 2).
pub fn modificador(def: &DefinicionVariable, valor: i16) -> i32 {
    (i32::from(valor) - i32::from(def.neutro)).div_euclid(i32::from(def.escala.max(1)))
}

/// Deja una variable por definición y en su orden: las que faltan nacen con el valor neutro; las de
/// definiciones que ya no existen (o repetidas) se quitan.
pub fn normalizar_variables(defs: &[DefinicionVariable], valores: &mut Vec<ValorVariable>) {
    let mut anteriores = std::mem::take(valores);
    for d in defs {
        let v = match anteriores.iter().position(|v| v.id_definicion == d.id) {
            Some(i) => anteriores.swap_remove(i),
            None => ValorVariable { id_definicion: d.id, valor: d.neutro },
        };
        valores.push(v);
    }
}

/// Comprueba los valores de una ficha YA normalizada.
pub fn validar_valores(defs: &[DefinicionVariable], valores: &[ValorVariable]) -> Result<(), String> {
    for (d, v) in defs.iter().zip(valores) {
        if v.valor.abs() > LIMITE_VALOR_VARIABLE {
            return Err(format!("{}: el valor debe estar entre -{LIMITE_VALOR_VARIABLE} y {LIMITE_VALOR_VARIABLE}", d.nombre));
        }
    }
    Ok(())
}

/// Lo escrito en el chat como nombre de variable: sin espacios de los lados y en minúsculas.
fn clave(texto: &str) -> String {
    texto.trim().to_lowercase()
}

/// ¿Es un texto que puede ser una variable en el chat (solo letras, y espacios dentro)? Un término de dados
/// siempre lleva algún dígito, así que nunca se confunde con una variable.
pub fn parece_variable(texto: &str) -> bool {
    let t = texto.trim();
    !t.is_empty() && t.chars().all(|c| c.is_alphabetic() || c == ' ') && t.chars().any(char::is_alphabetic)
}

/// La variable que nombra `texto` (por nombre o abreviatura, sin distinguir mayúsculas).
pub fn buscar<'a>(defs: &'a [DefinicionVariable], texto: &str) -> Option<&'a DefinicionVariable> {
    let k = clave(texto);
    if k.is_empty() {
        return None;
    }
    defs.iter().find(|d| clave(&d.nombre) == k || clave(&d.abreviatura) == k)
}

/// Comprueba una definición contra las DEMÁS que existen (al crear: todas; al modificar: todas menos ella).
pub fn validar_definicion(otras: &[DefinicionVariable], d: &DefinicionVariable) -> Result<(), String> {
    let nombre = d.nombre.trim();
    let abreviatura = d.abreviatura.trim();
    if nombre.is_empty() {
        return Err("La variable necesita un nombre".into());
    }
    // La abreviatura es opcional: sin ella se tira con el nombre completo
    if nombre.chars().count() > MAX_NOMBRE_VARIABLE {
        return Err(format!("El nombre de la variable no puede pasar de {MAX_NOMBRE_VARIABLE} caracteres"));
    }
    if abreviatura.chars().count() > MAX_ABREVIATURA_VARIABLE {
        return Err(format!("La abreviatura no puede pasar de {MAX_ABREVIATURA_VARIABLE} letras"));
    }
    if !parece_variable(nombre) {
        return Err("El nombre de la variable solo puede llevar letras".into());
    }
    if !abreviatura.chars().all(char::is_alphabetic) {
        return Err("La abreviatura solo puede llevar letras (sin espacios)".into());
    }
    for texto in [nombre, abreviatura].into_iter().filter(|t| !t.is_empty()) {
        if PALABRAS_RESERVADAS.contains(&clave(texto).as_str()) {
            return Err(format!("«{texto}» está reservado en el chat de dados"));
        }
        if otras.iter().any(|o| clave(&o.nombre) == clave(texto) || clave(&o.abreviatura) == clave(texto)) {
            return Err(format!("Ya hay una variable que se llama o se abrevia «{texto}»"));
        }
    }
    if !(1..=MAX_ESCALA_VARIABLE).contains(&d.escala) {
        return Err(format!("La escala debe estar entre 1 y {MAX_ESCALA_VARIABLE}"));
    }
    if d.neutro.abs() > LIMITE_VALOR_VARIABLE {
        return Err(format!("El valor neutro debe estar entre -{LIMITE_VALOR_VARIABLE} y {LIMITE_VALOR_VARIABLE}"));
    }
    if otras.len() >= MAX_VARIABLES {
        return Err(format!("No puede haber más de {MAX_VARIABLES} variables"));
    }
    Ok(())
}

#[cfg(test)]
pub mod pruebas {
    use super::*;

    pub fn var(id: u16, nombre: &str, abreviatura: &str, neutro: i16, escala: i16) -> DefinicionVariable {
        DefinicionVariable { id, nombre: nombre.into(), abreviatura: abreviatura.into(), neutro, escala }
    }

    #[test]
    fn el_resultado_se_redondea_hacia_abajo() {
        let fuerza = var(1, "Fuerza", "f", 10, 2);
        assert_eq!(modificador(&fuerza, 12), 1);
        assert_eq!(modificador(&fuerza, 8), -1);
        assert_eq!(modificador(&fuerza, 9), -1);
        assert_eq!(modificador(&fuerza, 10), 0);
        assert_eq!(modificador(&fuerza, 11), 0);
        assert_eq!(modificador(&fuerza, 20), 5);
        let cordura = var(2, "Cordura", "cor", 7, 3);
        assert_eq!(modificador(&cordura, 9), 0);
        assert_eq!(modificador(&cordura, 10), 1);
        assert_eq!(modificador(&cordura, 6), -1);
        // Los extremos no desbordan
        assert_eq!(modificador(&var(3, "X", "x", -9999, 1), 9999), 19998);
    }

    #[test]
    fn normalizar_rellena_con_el_neutro_ordena_y_quita() {
        let defs = vec![var(1, "Fuerza", "f", 10, 2), var(2, "Cordura", "cor", 7, 3)];
        let mut vs = vec![
            ValorVariable { id_definicion: 2, valor: 9 },
            ValorVariable { id_definicion: 99, valor: 1 },
            ValorVariable { id_definicion: 2, valor: 1 },
        ];
        normalizar_variables(&defs, &mut vs);
        assert_eq!(vs, vec![ValorVariable { id_definicion: 1, valor: 10 }, ValorVariable { id_definicion: 2, valor: 9 }]);
    }

    #[test]
    fn se_busca_por_nombre_o_abreviatura_sin_mayusculas() {
        let defs = vec![var(1, "Fuerza", "f", 10, 2), var(2, "Inteligencia", "in", 10, 2)];
        assert_eq!(buscar(&defs, "f").unwrap().id, 1);
        assert_eq!(buscar(&defs, " F ").unwrap().id, 1);
        assert_eq!(buscar(&defs, "FUERZA").unwrap().id, 1);
        assert_eq!(buscar(&defs, "in").unwrap().id, 2);
        assert!(buscar(&defs, "fu").is_none());
        assert!(parece_variable("f") && parece_variable("Fuerza bruta") && parece_variable("ñu"));
        assert!(!parece_variable("1d6") && !parece_variable("d6") && !parece_variable("<b>") && !parece_variable(" "));
    }

    #[test]
    fn validacion_de_las_definiciones() {
        let otras = vec![var(1, "Fuerza", "f", 10, 2)];
        assert!(validar_definicion(&otras, &var(2, "Inteligencia", "in", 10, 2)).is_ok());
        assert!(validar_definicion(&otras, &var(2, "Fuerza mágica", "fm", 10, 2)).is_ok(), "acentos y espacios en el nombre");
        for mala in [
            var(2, "", "x", 10, 2),
            var(2, "Fuerza", "fz", 10, 2),
            var(2, "Fe", "F", 10, 2),
            var(2, "f", "x", 10, 2),
            var(2, "Destreza", "c", 10, 2),
            var(2, "Clean", "cl", 10, 2),
            var(2, "Ventaja", "adv", 10, 2),
            var(2, "Dado", "d", 10, 2),
            var(2, "Fuerza2", "f2", 10, 2),
            var(2, "Inte", "i n", 10, 2),
            var(2, "<b>", "b", 10, 2),
            var(2, "Inteligencia", "in", 10, 0),
            var(2, "Inteligencia", "in", 10, 101),
            var(2, "Inteligencia", "in", 10000, 2),
        ] {
            assert!(validar_definicion(&otras, &mala).is_err(), "{mala:?}");
        }
        // La abreviatura es opcional, y varias variables pueden no tenerla
        let sin = vec![var(1, "Fuerza", "", 10, 2)];
        assert!(validar_definicion(&sin, &var(2, "Destreza", "", 10, 2)).is_ok());
        assert!(buscar(&[var(1, "Fuerza", "", 10, 2)], "").is_none());
        assert_eq!(buscar(&[var(1, "Fuerza", "", 10, 2)], "fuerza").unwrap().id, 1);
        let muchas: Vec<_> = (0..MAX_VARIABLES as u16).map(|i| var(i, &"a".repeat(i as usize + 1), &"b".repeat(i as usize % 9 + 1), 10, 2)).collect();
        assert!(validar_definicion(&muchas, &var(99, "Otra", "z", 10, 2)).unwrap_err().contains("más de"));
    }

    #[test]
    fn los_valores_tienen_tope() {
        let defs = vec![var(1, "Fuerza", "f", 10, 2)];
        assert!(validar_valores(&defs, &[ValorVariable { id_definicion: 1, valor: 9999 }]).is_ok());
        assert!(validar_valores(&defs, &[ValorVariable { id_definicion: 1, valor: -10000 }]).is_err());
    }
}
