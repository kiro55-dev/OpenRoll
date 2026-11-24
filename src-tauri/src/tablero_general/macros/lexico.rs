// src-tauri/src/tablero_general/macros/lexico.rs
//
// Trocea el código de una macro en piezas (tokens), cada una con su línea y columna para los errores.
// Las palabras clave están en una tabla (`CLAVES`): una palabra nueva es una fila más + su variante de `Clave`.
//
// Nombres: `.nombre` es una variable de la ficha, `@nombre` un parámetro visual (`@vida.max` su máximo) y una palabra suelta un
// atributo (lo que guarda `SAVE`). Los nombres son UNA palabra (letras, cifras y `_`): uno con espacios se escribe junto
// (`@rangoarma` = «Rango arma»; se compara sin espacios ni mayúsculas). Así `CHANGE @vida dano` no puede confundirse con un
// parámetro «vida dano».

use crate::tablero_general::escalas::Operador;

/// Una palabra reservada del lenguaje (se acepta en mayúsculas o minúsculas).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clave {
    OnTurn,
    OnClick,
    Do,
    If,
    IfNot,
    Else,
    Execute,
    ExecuteP,
    Quit,
    Change,
    Spread,
    To,
    Macros,
    Entities,
    And,
    Or,
    In,
    Save,
    Turn,
    Distance,
}

/// Las palabras clave y lo que son. Para añadir una: una fila aquí, su variante en `Clave` y su sitio en el parser.
pub const CLAVES: &[(&str, Clave)] = &[
    ("ONTURN", Clave::OnTurn),
    ("ONCLICK", Clave::OnClick),
    ("DO", Clave::Do),
    ("IF", Clave::If),
    ("IFNOT", Clave::IfNot),
    ("ELSE", Clave::Else),
    ("EXECUTE", Clave::Execute),
    ("EXECUTEP", Clave::ExecuteP),
    ("QUIT", Clave::Quit),
    ("CHANGE", Clave::Change),
    ("SPREAD", Clave::Spread),
    ("TO", Clave::To),
    ("MACROS", Clave::Macros),
    ("ENTITIES", Clave::Entities),
    ("AND", Clave::And),
    ("OR", Clave::Or),
    ("IN", Clave::In),
    ("SAVE", Clave::Save),
    ("TURN", Clave::Turn),
    ("DISTANCE", Clave::Distance),
];

/// Además de las claves, palabras que no pueden ser nombre de atributo (son de los dados).
const RESERVADAS_DADOS: &[&str] = &["d", "adv", "desv", "dis", "vs"];

pub fn clave(palabra: &str) -> Option<Clave> {
    CLAVES.iter().find(|(k, _)| k.eq_ignore_ascii_case(palabra)).map(|(_, c)| *c)
}

/// ¿Es una palabra que no vale como nombre de atributo?
pub fn reservada(palabra: &str) -> bool {
    clave(palabra).is_some() || RESERVADAS_DADOS.iter().any(|r| r.eq_ignore_ascii_case(palabra))
}

/// Comparación de una condición.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparacion {
    Menor,
    MenorIgual,
    Igual,
    Distinto,
    MayorIgual,
    Mayor,
}

impl Comparacion {
    pub fn simbolo(self) -> &'static str {
        match self {
            Comparacion::Menor => "<",
            Comparacion::MenorIgual => "<=",
            Comparacion::Igual => "=",
            Comparacion::Distinto => "!=",
            Comparacion::MayorIgual => ">=",
            Comparacion::Mayor => ">",
        }
    }

    pub fn cumple(self, a: i64, b: i64) -> bool {
        match self {
            Comparacion::Menor => a < b,
            Comparacion::MenorIgual => a <= b,
            Comparacion::Igual => a == b,
            Comparacion::Distinto => a != b,
            Comparacion::MayorIgual => a >= b,
            Comparacion::Mayor => a > b,
        }
    }

    /// La de la escala de resultados del conjunto (`escalas::valorar`); `=` y `!=` no tienen niveles.
    pub fn operador(self) -> Option<Operador> {
        match self {
            Comparacion::Menor => Some(Operador::Menor),
            Comparacion::MenorIgual => Some(Operador::MenorIgual),
            Comparacion::MayorIgual => Some(Operador::MayorIgual),
            Comparacion::Mayor => Some(Operador::Mayor),
            Comparacion::Igual | Comparacion::Distinto => None,
        }
    }

    /// La misma vista desde el otro lado (`12 >= 1d20` es `1d20 <= 12`).
    pub fn invertida(self) -> Self {
        match self {
            Comparacion::Menor => Comparacion::Mayor,
            Comparacion::MenorIgual => Comparacion::MayorIgual,
            Comparacion::MayorIgual => Comparacion::MenorIgual,
            Comparacion::Mayor => Comparacion::Menor,
            otra => otra,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(i64),
    /// `2d6`, `d20`, `1d20adv`, `1d20desv` (`dis` = `desv`).
    Dado { veces: i32, caras: i32, ventaja: bool, desventaja: bool },
    /// `.nombre`: variable.
    Var(String),
    /// `@nombre` o `@nombre.max`: parámetro visual.
    Param { nombre: String, max: bool },
    /// Palabra suelta: atributo (o, tras `MACROS.`, el nombre de una macro, que llega como `Var`).
    Ident(String),
    Clave(Clave),
    Mas,
    Menos,
    Por,
    Cmp(Comparacion),
    AbreP,
    CierraP,
    AbreC,
    CierraC,
    Coma,
    PuntoComa,
    Salto,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pieza {
    pub tok: Tok,
    pub linea: usize,
    pub columna: usize,
}

/// Error con la posición, para enseñarlo en el editor («línea 2, columna 5: …»).
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorMacro {
    pub linea: usize,
    pub columna: usize,
    pub mensaje: String,
}

impl std::fmt::Display for ErrorMacro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "línea {}, columna {}: {}", self.linea, self.columna, self.mensaje)
    }
}

fn es_de_nombre(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn trocear(codigo: &str) -> Result<Vec<Pieza>, ErrorMacro> {
    let c: Vec<char> = codigo.chars().collect();
    let mut piezas: Vec<Pieza> = Vec::new();
    let (mut i, mut linea, mut inicio_linea) = (0usize, 1usize, 0usize);
    let error = |linea: usize, col: usize, mensaje: String| ErrorMacro { linea, columna: col, mensaje };

    while i < c.len() {
        let col = i - inicio_linea + 1;
        let ch = c[i];
        let poner = |tok: Tok, piezas: &mut Vec<Pieza>| piezas.push(Pieza { tok, linea, columna: col });
        match ch {
            '\n' => {
                if !matches!(piezas.last().map(|p| &p.tok), Some(Tok::Salto) | None) {
                    poner(Tok::Salto, &mut piezas);
                }
                i += 1;
                linea += 1;
                inicio_linea = i;
            }
            ' ' | '\t' | '\r' => i += 1,
            // Comentario hasta el final de la línea
            '#' => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '(' | ')' | '[' | ']' | ',' | ';' | '+' | '-' | '*' => {
                let tok = match ch {
                    '(' => Tok::AbreP,
                    ')' => Tok::CierraP,
                    '[' => Tok::AbreC,
                    ']' => Tok::CierraC,
                    ',' => Tok::Coma,
                    ';' => Tok::PuntoComa,
                    '+' => Tok::Mas,
                    '-' => Tok::Menos,
                    _ => Tok::Por,
                };
                poner(tok, &mut piezas);
                i += 1;
            }
            '<' | '>' | '=' | '!' => {
                let sig = c.get(i + 1).copied();
                let (cmp, largo) = match (ch, sig) {
                    ('<', Some('=')) => (Comparacion::MenorIgual, 2),
                    ('>', Some('=')) => (Comparacion::MayorIgual, 2),
                    ('=', Some('=')) => (Comparacion::Igual, 2),
                    ('!', Some('=')) => (Comparacion::Distinto, 2),
                    ('<', _) => (Comparacion::Menor, 1),
                    ('>', _) => (Comparacion::Mayor, 1),
                    ('=', _) => (Comparacion::Igual, 1),
                    _ => return Err(error(linea, col, "«!» va siempre con «=» («!=»: distinto)".into())),
                };
                poner(Tok::Cmp(cmp), &mut piezas);
                i += largo;
            }
            '.' | '@' => {
                let mut j = i + 1;
                while j < c.len() && es_de_nombre(c[j]) {
                    j += 1;
                }
                let nombre: String = c[i + 1..j].iter().collect();
                if nombre.is_empty() {
                    let que = if ch == '.' { "una variable" } else { "un parámetro" };
                    return Err(error(linea, col, format!("Falta el nombre de {que} detrás de «{ch}» (sin espacios: @rangoarma)")));
                }
                // `@vida.max`
                let mut max = false;
                if ch == '@' && j + 4 <= c.len() {
                    let sufijo: String = c[j..j + 4].iter().collect();
                    if sufijo.eq_ignore_ascii_case(".max") && c.get(j + 4).is_none_or(|x| !es_de_nombre(*x)) {
                        max = true;
                        j += 4;
                    }
                }
                let tok = if ch == '.' { Tok::Var(nombre) } else { Tok::Param { nombre, max } };
                poner(tok, &mut piezas);
                i = j;
            }
            _ if ch.is_ascii_digit() || ((ch == 'd' || ch == 'D') && c.get(i + 1).is_some_and(|x| x.is_ascii_digit())) => {
                // Un número o un dado (`2d6`, `d20`, `1d20adv`)
                let mut j = i;
                while j < c.len() && c[j].is_ascii_digit() {
                    j += 1;
                }
                let veces_txt: String = c[i..j].iter().collect();
                if j < c.len() && (c[j] == 'd' || c[j] == 'D') && c.get(j + 1).is_some_and(|x| x.is_ascii_digit()) {
                    let mut k = j + 1;
                    while k < c.len() && c[k].is_ascii_digit() {
                        k += 1;
                    }
                    let caras_txt: String = c[j + 1..k].iter().collect();
                    let mut m = k;
                    while m < c.len() && c[m].is_alphabetic() {
                        m += 1;
                    }
                    let sufijo: String = c[k..m].iter().collect::<String>().to_lowercase();
                    let (ventaja, desventaja) = match sufijo.as_str() {
                        "" => (false, false),
                        "adv" => (true, false),
                        "desv" | "dis" => (false, true),
                        otro => return Err(error(linea, col, format!("Modificador de dado desconocido: «{otro}» (adv, desv o dis)"))),
                    };
                    let veces: i32 = if veces_txt.is_empty() { 1 } else { veces_txt.parse().map_err(|_| error(linea, col, "Demasiados dados".into()))? };
                    let caras: i32 = caras_txt.parse().map_err(|_| error(linea, col, "Demasiadas caras".into()))?;
                    if veces <= 0 || caras <= 0 {
                        return Err(error(linea, col, "Los dados y sus caras deben ser mayores que 0".into()));
                    }
                    if veces > 999 {
                        return Err(error(linea, col, "No se pueden tirar más de 999 dados de golpe".into()));
                    }
                    poner(Tok::Dado { veces, caras, ventaja, desventaja }, &mut piezas);
                    i = m;
                } else {
                    if j < c.len() && es_de_nombre(c[j]) {
                        return Err(error(linea, col, "Un número no puede ir pegado a letras".into()));
                    }
                    let n: i64 = veces_txt.parse().map_err(|_| error(linea, col, "Número demasiado grande".into()))?;
                    if n > 1_000_000 {
                        return Err(error(linea, col, "Número demasiado grande".into()));
                    }
                    poner(Tok::Num(n), &mut piezas);
                    i = j;
                }
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                let mut j = i;
                while j < c.len() && es_de_nombre(c[j]) {
                    j += 1;
                }
                let palabra: String = c[i..j].iter().collect();
                // `vs` como en el chat: azúcar de `<=`
                let tok = if palabra.eq_ignore_ascii_case("vs") {
                    Tok::Cmp(Comparacion::MenorIgual)
                } else {
                    match clave(&palabra) {
                        Some(k) => Tok::Clave(k),
                        None => Tok::Ident(palabra),
                    }
                };
                poner(tok, &mut piezas);
                i = j;
            }
            otro => return Err(error(linea, col, format!("Carácter no válido: «{otro}»"))),
        }
    }
    if matches!(piezas.last().map(|p| &p.tok), Some(Tok::Salto)) {
        piezas.pop();
    }
    Ok(piezas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(c: &str) -> Vec<Tok> {
        trocear(c).unwrap().into_iter().map(|p| p.tok).collect()
    }

    #[test]
    fn trocea_dados_nombres_y_claves() {
        assert_eq!(
            toks("onturn\nDO CHANGE @vida -1d4 IF execute 1d20 <= .cont ELSE QUIT"),
            vec![
                Tok::Clave(Clave::OnTurn),
                Tok::Salto,
                Tok::Clave(Clave::Do),
                Tok::Clave(Clave::Change),
                Tok::Param { nombre: "vida".into(), max: false },
                Tok::Menos,
                Tok::Dado { veces: 1, caras: 4, ventaja: false, desventaja: false },
                Tok::Clave(Clave::If),
                Tok::Clave(Clave::Execute),
                Tok::Dado { veces: 1, caras: 20, ventaja: false, desventaja: false },
                Tok::Cmp(Comparacion::MenorIgual),
                Tok::Var("cont".into()),
                Tok::Clave(Clave::Else),
                Tok::Clave(Clave::Quit),
            ]
        );
        assert_eq!(
            toks("ENTITIES@vida.max != d20adv # comentario\n\n2*3d6dis"),
            vec![
                Tok::Clave(Clave::Entities),
                Tok::Param { nombre: "vida".into(), max: true },
                Tok::Cmp(Comparacion::Distinto),
                Tok::Dado { veces: 1, caras: 20, ventaja: true, desventaja: false },
                Tok::Salto,
                Tok::Num(2),
                Tok::Por,
                Tok::Dado { veces: 3, caras: 6, ventaja: false, desventaja: true },
            ]
        );
    }

    #[test]
    fn los_errores_dicen_donde() {
        let e = trocear("ONCLICK\nEXECUTE 1d20xx").unwrap_err();
        assert_eq!((e.linea, e.columna), (2, 9));
        assert!(trocear("EXECUTE 1000d6").is_err());
        assert!(trocear("EXECUTE 3abc").is_err());
        assert!(trocear("EXECUTE @ vida").is_err());
        assert!(trocear("EXECUTE 1 ! 2").is_err());
        assert!(trocear("EXECUTE {").is_err());
    }
}
