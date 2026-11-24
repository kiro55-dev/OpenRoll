// src-tauri/src/tablero_general/macros/sintaxis.rs
//
// El árbol de una macro y el parser (descendente recursivo) que lo construye a partir de las piezas de lexico.rs.
//
// Para AMPLIAR el lenguaje:
//  * una sentencia nueva: su variante en `Sentencia`, una fila en `SENTENCIAS` (palabra clave → función que la lee) y su caso en
//    interprete.rs;
//  * un término nuevo de los argumentos (como `TURN`): su variante en `Expr`, su caso en `Parser::termino` y en el intérprete.

use super::lexico::{reservada, trocear, Clave, Comparacion, ErrorMacro, Pieza, Tok};

/// Largo máximo del código de una macro, en caracteres, y de sus líneas.
pub const MAX_CARACTERES: usize = 2000;
pub const MAX_LINEAS: usize = 60;
/// Anidado máximo (bloques, DO dentro de DO, paréntesis): nada que un jugador escriba puede llenar la pila.
pub const MAX_PROFUNDIDAD: usize = 16;
/// Largo de un nombre de atributo.
pub const MAX_NOMBRE_ATRIBUTO: usize = 30;

/// De quién se lee una variable o un parámetro: de la ficha de la macro o de la entidad que se está mirando en un SPREAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sujeto {
    Propia,
    /// `ENTITIES.var` / `ENTITIES@param`: la candidata del SPREAD.
    Candidata,
}

/// Un argumento: suma y resta de términos.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(i64),
    Dado { veces: i32, caras: i32, ventaja: bool, desventaja: bool, multiplicador: i32 },
    Var { de: Sujeto, nombre: String },
    Param { de: Sujeto, nombre: String, max: bool },
    /// Lo que guardó `SAVE` en esta asociación.
    Atributo(String),
    /// `TURN`: la ronda del grupo de batalla de la ficha.
    Turno,
    /// `DISTANCE`: casillas entre la ficha de la macro y la candidata del SPREAD.
    Distancia,
    Neg(Box<Expr>),
    Suma(Box<Expr>, Box<Expr>),
    Resta(Box<Expr>, Box<Expr>),
    Grupo(Box<Expr>),
}

impl Expr {
    /// ¿Lee algo de la candidata de un SPREAD?
    pub fn usa_candidata(&self) -> bool {
        match self {
            Expr::Var { de, .. } | Expr::Param { de, .. } => *de == Sujeto::Candidata,
            Expr::Distancia => true,
            Expr::Neg(e) | Expr::Grupo(e) => e.usa_candidata(),
            Expr::Suma(a, b) | Expr::Resta(a, b) => a.usa_candidata() || b.usa_candidata(),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Condicion {
    /// `mostrar`: la comparación sale en el chat (`EXECUTE` = pública, `EXECUTEP` = privada: `Some(true)`).
    Comparar { izq: Expr, cmp: Comparacion, der: Expr, mostrar: Option<bool> },
    /// `ENTITIES IN @param`: la candidata cae en la zona que dibuja ese parámetro de la ficha de la macro.
    Dentro { param: String },
    Y(Box<Condicion>, Box<Condicion>),
    O(Box<Condicion>, Box<Condicion>),
}

impl Condicion {
    pub fn usa_candidata(&self) -> bool {
        match self {
            Condicion::Comparar { izq, der, .. } => izq.usa_candidata() || der.usa_candidata(),
            Condicion::Dentro { .. } => true,
            Condicion::Y(a, b) | Condicion::O(a, b) => a.usa_candidata() || b.usa_candidata(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Sentencia {
    /// `EXECUTE x` / `EXECUTEP x`: al chat (pública o privada).
    Mostrar { expr: Expr, privada: bool },
    /// `SAVE nombre x`.
    Guardar { nombre: String, expr: Expr },
    /// `CHANGE @param x`: `relativo` si `x` empieza por `+`/`-` (se suma); si no, se pone ese valor.
    Cambiar { de: Sujeto, param: String, expr: Expr, relativo: bool },
    /// `QUIT`: quita la macro de lo que la tiene asociada y para.
    Quitar,
    /// `DO cuerpo [IF condición [ELSE cuerpo]]`.
    Si { cuerpo: Vec<Sentencia>, condicion: Option<Condicion>, sino: Vec<Sentencia> },
    /// `SPREAD acción TO condición`.
    Repartir { accion: Box<Sentencia>, condicion: Condicion },
    /// La acción `MACROS.a, MACROS.b` de un SPREAD: asocia esas macros a cada candidata que cumple.
    Asociar(Vec<String>),
}

/// Una macro ya leída.
#[derive(Clone, Debug, PartialEq)]
pub struct Programa {
    pub al_pulsar: bool,
    pub en_turno: bool,
    pub cuerpo: Vec<Sentencia>,
}

type LeerSentencia = fn(&mut Parser) -> Result<Sentencia, ErrorMacro>;

/// Las sentencias y quién las lee. `DO` y `SPREAD` también están: su cuerpo vuelve a pasar por aquí.
const SENTENCIAS: &[(Clave, LeerSentencia)] = &[
    (Clave::Execute, |p| p.mostrar(false)),
    (Clave::ExecuteP, |p| p.mostrar(true)),
    (Clave::Save, Parser::guardar),
    (Clave::Change, Parser::cambiar),
    (Clave::Quit, |_| Ok(Sentencia::Quitar)),
    (Clave::Do, Parser::si),
    (Clave::Spread, Parser::repartir),
];

/// Lee el código de una macro entero (con sus topes).
pub fn leer(codigo: &str) -> Result<Programa, ErrorMacro> {
    if codigo.chars().count() > MAX_CARACTERES {
        return Err(ErrorMacro { linea: 1, columna: 1, mensaje: format!("Una macro no puede pasar de {MAX_CARACTERES} caracteres") });
    }
    if codigo.lines().count() > MAX_LINEAS {
        return Err(ErrorMacro { linea: MAX_LINEAS + 1, columna: 1, mensaje: format!("Una macro no puede pasar de {MAX_LINEAS} líneas") });
    }
    let piezas = trocear(codigo)?;
    let mut p = Parser { piezas, pos: 0, profundidad: 0, en_reparto: false };
    p.programa()
}

pub struct Parser {
    piezas: Vec<Pieza>,
    pos: usize,
    profundidad: usize,
    /// Dentro de un SPREAD: ahí valen `ENTITIES…` y `DISTANCE`.
    en_reparto: bool,
}

impl Parser {
    fn ver(&self) -> Option<&Tok> {
        self.piezas.get(self.pos).map(|p| &p.tok)
    }

    fn ver_en(&self, n: usize) -> Option<&Tok> {
        self.piezas.get(self.pos + n).map(|p| &p.tok)
    }

    fn avanzar(&mut self) -> Option<Tok> {
        let t = self.piezas.get(self.pos).map(|p| p.tok.clone());
        self.pos += 1;
        t
    }

    fn error<T>(&self, mensaje: impl Into<String>) -> Result<T, ErrorMacro> {
        let (linea, columna) = match self.piezas.get(self.pos).or(self.piezas.last()) {
            Some(p) => (p.linea, p.columna),
            None => (1, 1),
        };
        Err(ErrorMacro { linea, columna, mensaje: mensaje.into() })
    }

    fn es_clave(&self, k: Clave) -> bool {
        self.ver() == Some(&Tok::Clave(k))
    }

    /// Si tras saltos de línea viene la palabra `k` (`IF`, `ELSE`, `AND`, `OR`, `TO` en la línea siguiente), se saltan las líneas.
    fn sigue_con(&mut self, claves: &[Clave]) -> bool {
        let mut n = 0;
        while self.ver_en(n) == Some(&Tok::Salto) {
            n += 1;
        }
        let si = matches!(self.ver_en(n), Some(Tok::Clave(k)) if claves.contains(k));
        if si {
            self.pos += n;
        }
        si
    }

    fn entrar(&mut self) -> Result<(), ErrorMacro> {
        self.profundidad += 1;
        if self.profundidad > MAX_PROFUNDIDAD {
            return self.error("Demasiado anidado");
        }
        Ok(())
    }

    fn salir(&mut self) {
        self.profundidad -= 1;
    }

    fn programa(&mut self) -> Result<Programa, ErrorMacro> {
        while self.ver() == Some(&Tok::Salto) {
            self.pos += 1;
        }
        let (mut al_pulsar, mut en_turno) = (false, false);
        loop {
            match self.ver() {
                Some(Tok::Clave(Clave::OnClick)) => al_pulsar = true,
                Some(Tok::Clave(Clave::OnTurn)) => en_turno = true,
                _ => break,
            }
            self.pos += 1;
        }
        if !al_pulsar && !en_turno {
            return self.error("La macro empieza con ONTURN, ONCLICK o los dos");
        }
        let cuerpo = self.sentencias(None)?;
        if cuerpo.is_empty() {
            return self.error("La macro no hace nada: falta alguna sentencia después de la cabecera");
        }
        Ok(Programa { al_pulsar, en_turno, cuerpo })
    }

    /// Sentencias separadas por saltos de línea o `;`, hasta el final (o hasta `cierre`).
    fn sentencias(&mut self, cierre: Option<Tok>) -> Result<Vec<Sentencia>, ErrorMacro> {
        let mut lista = Vec::new();
        loop {
            while matches!(self.ver(), Some(Tok::Salto) | Some(Tok::PuntoComa)) {
                self.pos += 1;
            }
            match (self.ver(), &cierre) {
                (None, None) => return Ok(lista),
                (None, Some(_)) => return self.error("Falta cerrar el bloque con «]»"),
                (Some(t), Some(c)) if t == c => {
                    self.pos += 1;
                    return Ok(lista);
                }
                _ => {}
            }
            lista.push(self.sentencia()?);
            match self.ver() {
                None | Some(Tok::Salto) | Some(Tok::PuntoComa) => {}
                Some(t) if Some(t) == cierre.as_ref() => {}
                _ => return self.error("Sobra algo al final de la sentencia (cada una va en su línea o separada por «;»)"),
            }
        }
    }

    fn sentencia(&mut self) -> Result<Sentencia, ErrorMacro> {
        let Some(Tok::Clave(k)) = self.ver().cloned() else {
            return self.error("Se esperaba una sentencia: EXECUTE, EXECUTEP, SAVE, CHANGE, QUIT, DO o SPREAD");
        };
        let Some((_, leer)) = SENTENCIAS.iter().find(|(c, _)| *c == k) else {
            return self.error(format!("«{k:?}» no empieza una sentencia").to_uppercase());
        };
        self.pos += 1;
        self.entrar()?;
        let s = leer(self);
        self.salir();
        s
    }

    /// El cuerpo de un DO o un ELSE: una sentencia o un bloque `[ … ]`.
    fn cuerpo(&mut self) -> Result<Vec<Sentencia>, ErrorMacro> {
        if self.ver() == Some(&Tok::AbreC) {
            self.pos += 1;
            self.entrar()?;
            let l = self.sentencias(Some(Tok::CierraC));
            self.salir();
            let l = l?;
            if l.is_empty() {
                return self.error("Un bloque «[ ]» vacío no hace nada");
            }
            return Ok(l);
        }
        Ok(vec![self.sentencia()?])
    }

    /// `EXECUTE x`, o `EXECUTE x <= y` (una comparación que solo se enseña, como en el chat: con su resultado).
    fn mostrar(&mut self, privada: bool) -> Result<Sentencia, ErrorMacro> {
        let expr = self.expr()?;
        if let Some(Tok::Cmp(cmp)) = self.ver().cloned() {
            self.pos += 1;
            let der = self.expr()?;
            let condicion = Condicion::Comparar { izq: expr, cmp, der, mostrar: Some(privada) };
            return Ok(Sentencia::Si { cuerpo: vec![], condicion: Some(condicion), sino: vec![] });
        }
        Ok(Sentencia::Mostrar { expr, privada })
    }

    fn guardar(&mut self) -> Result<Sentencia, ErrorMacro> {
        let Some(Tok::Ident(nombre)) = self.ver().cloned() else {
            return self.error("SAVE va con el nombre del atributo y lo que guarda: SAVE dano 2d6");
        };
        if nombre.chars().count() > MAX_NOMBRE_ATRIBUTO || !nombre.chars().all(|c| c.is_alphabetic()) || reservada(&nombre) {
            return self.error(format!("«{nombre}» no vale como atributo (solo letras, hasta {MAX_NOMBRE_ATRIBUTO}, y que no sea una palabra reservada)"));
        }
        self.pos += 1;
        if matches!(self.ver(), None | Some(Tok::Salto) | Some(Tok::PuntoComa)) {
            return self.error(format!("Falta lo que se guarda en «{nombre}»"));
        }
        Ok(Sentencia::Guardar { nombre, expr: self.expr()? })
    }

    fn cambiar(&mut self) -> Result<Sentencia, ErrorMacro> {
        let de = if self.es_clave(Clave::Entities) {
            if !self.en_reparto {
                return self.error("ENTITIES solo vale dentro de un SPREAD");
            }
            self.pos += 1;
            Sujeto::Candidata
        } else {
            Sujeto::Propia
        };
        let Some(Tok::Param { nombre, max }) = self.ver().cloned() else {
            return self.error("CHANGE va con un parámetro visual: CHANGE @vida -1d4");
        };
        if max {
            return self.error("CHANGE cambia el valor actual, no el máximo (quita «.max»)");
        }
        self.pos += 1;
        let relativo = matches!(self.ver(), Some(Tok::Mas) | Some(Tok::Menos));
        Ok(Sentencia::Cambiar { de, param: nombre, expr: self.expr()?, relativo })
    }

    fn si(&mut self) -> Result<Sentencia, ErrorMacro> {
        let cuerpo = self.cuerpo()?;
        let (mut condicion, mut sino) = (None, Vec::new());
        if self.sigue_con(&[Clave::If]) {
            self.pos += 1;
            condicion = Some(self.condicion()?);
            if self.sigue_con(&[Clave::Else]) {
                self.pos += 1;
                sino = self.cuerpo()?;
            }
        } else if self.sigue_con(&[Clave::Else]) {
            return self.error("ELSE va después de un IF");
        }
        Ok(Sentencia::Si { cuerpo, condicion, sino })
    }

    fn repartir(&mut self) -> Result<Sentencia, ErrorMacro> {
        if self.en_reparto {
            return self.error("No se puede poner un SPREAD dentro de otro");
        }
        self.en_reparto = true;
        let r = self.repartir_dentro();
        self.en_reparto = false;
        r
    }

    fn repartir_dentro(&mut self) -> Result<Sentencia, ErrorMacro> {
        let accion = match self.avanzar() {
            Some(Tok::Clave(Clave::Execute)) => self.mostrar(false)?,
            Some(Tok::Clave(Clave::ExecuteP)) => self.mostrar(true)?,
            Some(Tok::Clave(Clave::Change)) => {
                if !self.es_clave(Clave::Entities) {
                    return self.error("En un SPREAD se cambia el parámetro de cada entidad: SPREAD CHANGE ENTITIES@vida -1d6 TO …");
                }
                self.cambiar()?
            }
            Some(Tok::Clave(Clave::Macros)) => {
                self.pos -= 1;
                let mut nombres = Vec::new();
                loop {
                    if !self.es_clave(Clave::Macros) {
                        return self.error("Se esperaba MACROS.nombre");
                    }
                    self.pos += 1;
                    let Some(Tok::Var(n)) = self.avanzar() else {
                        self.pos -= 1;
                        return self.error("Detrás de MACROS va un punto y el nombre de la macro: MACROS.envenenado");
                    };
                    nombres.push(n);
                    if self.ver() == Some(&Tok::Coma) {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                Sentencia::Asociar(nombres)
            }
            _ => {
                self.pos -= 1;
                return self.error("SPREAD va con EXECUTE, EXECUTEP, CHANGE ENTITIES@… o MACROS.…");
            }
        };
        if !self.sigue_con(&[Clave::To]) {
            return self.error("Falta «TO» y la condición de las entidades a las que llega");
        }
        self.pos += 1;
        let condicion = self.condicion()?;
        Ok(Sentencia::Repartir { accion: Box::new(accion), condicion })
    }

    // ---------------- Condiciones ----------------

    fn condicion(&mut self) -> Result<Condicion, ErrorMacro> {
        let mut c = self.conjuncion()?;
        while self.sigue_con(&[Clave::Or]) {
            self.pos += 1;
            c = Condicion::O(Box::new(c), Box::new(self.conjuncion()?));
        }
        Ok(c)
    }

    fn conjuncion(&mut self) -> Result<Condicion, ErrorMacro> {
        let mut c = self.atomo()?;
        while self.sigue_con(&[Clave::And]) {
            self.pos += 1;
            c = Condicion::Y(Box::new(c), Box::new(self.atomo()?));
        }
        Ok(c)
    }

    fn atomo(&mut self) -> Result<Condicion, ErrorMacro> {
        self.entrar()?;
        let r = self.atomo_dentro();
        self.salir();
        r
    }

    fn atomo_dentro(&mut self) -> Result<Condicion, ErrorMacro> {
        // `( condición )`: se prueba primero; si no lo es (`(2d8 + 1) <= 5`), se vuelve atrás y es una comparación
        if self.ver() == Some(&Tok::AbreP) {
            let guardado = self.pos;
            self.pos += 1;
            if let Ok(c) = self.condicion() {
                if self.ver() == Some(&Tok::CierraP) && !matches!(self.ver_en(1), Some(Tok::Mas) | Some(Tok::Menos) | Some(Tok::Cmp(_))) {
                    self.pos += 1;
                    return Ok(c);
                }
            }
            self.pos = guardado;
        }
        // `ENTITIES IN @param`
        if self.es_clave(Clave::Entities) && self.ver_en(1) == Some(&Tok::Clave(Clave::In)) {
            if !self.en_reparto {
                return self.error("ENTITIES solo vale dentro de un SPREAD");
            }
            self.pos += 2;
            let Some(Tok::Param { nombre, max: false }) = self.avanzar() else {
                self.pos -= 1;
                return self.error("Detrás de IN va el parámetro visual que dibuja la zona: ENTITIES IN @rangoarma");
            };
            return Ok(Condicion::Dentro { param: nombre });
        }
        let (izq, m1) = self.lado()?;
        let Some(Tok::Cmp(cmp)) = self.ver().cloned() else {
            return self.error("Falta la comparación: <, <=, =, !=, >= o >");
        };
        self.pos += 1;
        let (der, m2) = self.lado()?;
        Ok(Condicion::Comparar { izq, cmp, der, mostrar: m1.or(m2) })
    }

    /// Un lado de una comparación, quizá con `EXECUTE`/`EXECUTEP` delante (la comparación sale entonces en el chat).
    fn lado(&mut self) -> Result<(Expr, Option<bool>), ErrorMacro> {
        let mostrar = match self.ver() {
            Some(Tok::Clave(Clave::Execute)) => Some(false),
            Some(Tok::Clave(Clave::ExecuteP)) => Some(true),
            _ => None,
        };
        if mostrar.is_some() {
            self.pos += 1;
        }
        Ok((self.expr()?, mostrar))
    }

    // ---------------- Argumentos ----------------

    fn expr(&mut self) -> Result<Expr, ErrorMacro> {
        let mut e = match self.ver() {
            Some(Tok::Menos) => {
                self.pos += 1;
                Expr::Neg(Box::new(self.termino()?))
            }
            Some(Tok::Mas) => {
                self.pos += 1;
                self.termino()?
            }
            _ => self.termino()?,
        };
        loop {
            match self.ver() {
                Some(Tok::Mas) => {
                    self.pos += 1;
                    e = Expr::Suma(Box::new(e), Box::new(self.termino()?));
                }
                Some(Tok::Menos) => {
                    self.pos += 1;
                    e = Expr::Resta(Box::new(e), Box::new(self.termino()?));
                }
                _ => return Ok(e),
            }
        }
    }

    fn termino(&mut self) -> Result<Expr, ErrorMacro> {
        let Some(t) = self.ver().cloned() else {
            return self.error("Falta un argumento (un número, un dado, una .variable, un @parámetro o un atributo)");
        };
        self.pos += 1;
        Ok(match t {
            Tok::Num(n) => {
                if self.ver() == Some(&Tok::Por) {
                    self.pos += 1;
                    let Some(Tok::Dado { veces, caras, ventaja, desventaja }) = self.avanzar() else {
                        self.pos -= 1;
                        return self.error("El multiplicador solo se aplica a dados: 2*1d8");
                    };
                    if n <= 0 || n > 1000 {
                        return self.error("El multiplicador va de 1 a 1000");
                    }
                    Expr::Dado { veces, caras, ventaja, desventaja, multiplicador: n as i32 }
                } else {
                    Expr::Num(n)
                }
            }
            Tok::Dado { veces, caras, ventaja, desventaja } => Expr::Dado { veces, caras, ventaja, desventaja, multiplicador: 1 },
            Tok::Var(nombre) => Expr::Var { de: Sujeto::Propia, nombre },
            Tok::Param { nombre, max } => Expr::Param { de: Sujeto::Propia, nombre, max },
            Tok::Ident(nombre) => {
                if reservada(&nombre) {
                    self.pos -= 1;
                    return self.error(format!("«{nombre}» es una palabra reservada"));
                }
                Expr::Atributo(nombre)
            }
            Tok::Clave(Clave::Turn) => Expr::Turno,
            Tok::Clave(Clave::Distance) => {
                if !self.en_reparto {
                    self.pos -= 1;
                    return self.error("DISTANCE solo vale dentro de un SPREAD");
                }
                Expr::Distancia
            }
            Tok::Clave(Clave::Entities) => {
                if !self.en_reparto {
                    self.pos -= 1;
                    return self.error("ENTITIES solo vale dentro de un SPREAD");
                }
                match self.avanzar() {
                    Some(Tok::Var(nombre)) => Expr::Var { de: Sujeto::Candidata, nombre },
                    Some(Tok::Param { nombre, max }) => Expr::Param { de: Sujeto::Candidata, nombre, max },
                    _ => {
                        self.pos -= 1;
                        return self.error("Detrás de ENTITIES va .variable o @parámetro (o IN @parámetro en una condición)");
                    }
                }
            }
            Tok::AbreP => {
                self.entrar()?;
                let e = self.expr();
                self.salir();
                let e = e?;
                if self.ver() != Some(&Tok::CierraP) {
                    return self.error("Falta cerrar el paréntesis «)»");
                }
                self.pos += 1;
                Expr::Grupo(Box::new(e))
            }
            _ => {
                self.pos -= 1;
                return self.error("Se esperaba un argumento (un número, un dado, una .variable, un @parámetro o un atributo)");
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atr(n: &str) -> Expr {
        Expr::Atributo(n.into())
    }

    #[test]
    fn lee_los_ejemplos() {
        let p = leer("ONTURN\nDO CHANGE @vida -1d4 IF EXECUTE 1d20 <= .cont ELSE QUIT").unwrap();
        assert!(p.en_turno && !p.al_pulsar);
        let Sentencia::Si { cuerpo, condicion: Some(Condicion::Comparar { mostrar, cmp, .. }), sino } = &p.cuerpo[0] else { panic!("{p:?}") };
        assert_eq!(*mostrar, Some(false));
        assert_eq!(*cmp, Comparacion::MenorIgual);
        assert!(matches!(&cuerpo[0], Sentencia::Cambiar { relativo: true, de: Sujeto::Propia, .. }));
        assert_eq!(sino, &vec![Sentencia::Quitar]);

        let p = leer("ONCLICK onturn\nSPREAD CHANGE ENTITIES@vida -(2d8 + .fue) TO ENTITIES@armadura <= 1d20 + .destreza AND ENTITIES IN @rangoarma").unwrap();
        assert!(p.en_turno && p.al_pulsar);
        let Sentencia::Repartir { accion, condicion } = &p.cuerpo[0] else { panic!() };
        assert!(matches!(**accion, Sentencia::Cambiar { de: Sujeto::Candidata, relativo: true, .. }));
        assert!(matches!(condicion, Condicion::Y(_, b) if matches!(**b, Condicion::Dentro { .. })));
        assert!(condicion.usa_candidata());
    }

    #[test]
    fn and_liga_mas_que_or_y_los_parentesis_mandan() {
        let p = leer("ONCLICK\nDO QUIT IF 1 = 1 OR 2 = 2 AND 3 = 3").unwrap();
        let Sentencia::Si { condicion: Some(c), .. } = &p.cuerpo[0] else { panic!() };
        assert!(matches!(c, Condicion::O(_, b) if matches!(**b, Condicion::Y(..))));
        let p = leer("ONCLICK\nDO QUIT IF (1 = 1 OR 2 = 2) AND (1d4 + 1) <= 3").unwrap();
        let Sentencia::Si { condicion: Some(c), .. } = &p.cuerpo[0] else { panic!() };
        assert!(matches!(c, Condicion::Y(a, b) if matches!(**a, Condicion::O(..)) && matches!(**b, Condicion::Comparar { izq: Expr::Grupo(_), .. })));
    }

    #[test]
    fn bloques_lineas_y_continuaciones() {
        let p = leer("ONCLICK\nSAVE dano 2d6 + 3\nDO [\n  EXECUTE dano\n  CHANGE @vida 10; QUIT\n]\nIF dano >= 5\nELSE EXECUTEP 0").unwrap();
        assert_eq!(p.cuerpo.len(), 2);
        assert!(matches!(&p.cuerpo[0], Sentencia::Guardar { nombre, .. } if nombre == "dano"));
        let Sentencia::Si { cuerpo, condicion, sino } = &p.cuerpo[1] else { panic!() };
        assert_eq!(cuerpo.len(), 3);
        assert!(matches!(&cuerpo[1], Sentencia::Cambiar { relativo: false, .. }));
        assert!(matches!(condicion, Some(Condicion::Comparar { izq, .. }) if *izq == atr("dano")));
        assert!(matches!(&sino[0], Sentencia::Mostrar { privada: true, .. }));
        let p = leer("ONCLICK\nSPREAD MACROS.envenenado, MACROS.lento TO DISTANCE <= 2").unwrap();
        assert!(matches!(&p.cuerpo[0], Sentencia::Repartir { accion, .. } if **accion == Sentencia::Asociar(vec!["envenenado".into(), "lento".into()])));
    }

    #[test]
    fn los_ejemplos_de_la_documentacion_se_leen() {
        for codigo in [
            "ONTURN\nDO CHANGE @vida -1d4 IF EXECUTE 1d20 <= .cont ELSE QUIT",
            "ONCLICK\nSPREAD CHANGE ENTITIES@vida -(2d8 + .fue) TO ENTITIES@armadura <= 1d20 + .destreza AND ENTITIES IN @rangoarma",
            "ONCLICK\nSAVE dano 8d6\nSPREAD CHANGE ENTITIES@vida -dano TO ENTITIES.des + 1d20 < 15 AND DISTANCE <= 4",
            "ONCLICK\nSPREAD MACROS.envenenado TO DISTANCE <= 1",
            "ONCLICK # comentario\nEXECUTE 1d20 vs 12\nDO [ SAVE a 1; EXECUTEP a ]\n  IF TURN >= 2\n  AND @vida.max != 0",
        ] {
            assert!(leer(codigo).is_ok(), "{codigo}: {:?}", leer(codigo));
        }
    }

    #[test]
    fn errores_claros() {
        let falla = |c: &str| leer(c).unwrap_err().mensaje;
        assert!(falla("EXECUTE 1d20").contains("ONTURN, ONCLICK"));
        assert!(falla("ONCLICK").contains("no hace nada"));
        assert!(falla("ONCLICK\nEXECUTE ENTITIES@vida").contains("SPREAD"));
        assert!(falla("ONCLICK\nEXECUTE DISTANCE").contains("SPREAD"));
        assert!(falla("ONCLICK\nSPREAD EXECUTE 1 TO 1 = 1 AND ENTITIES IN @vida.max").contains("IN"));
        assert!(falla("ONCLICK\nSPREAD SPREAD EXECUTE 1 TO 1 = 1 TO 1 = 1").contains("SPREAD"));
        assert!(falla("ONCLICK\nSPREAD EXECUTE 1").contains("TO"));
        assert!(falla("ONCLICK\nDO QUIT ELSE QUIT").contains("ELSE"));
        assert!(falla("ONCLICK\nSAVE turn 1").contains("atributo"));
        assert!(falla("ONCLICK\nSAVE dano").contains("Falta"));
        assert!(falla("ONCLICK\nCHANGE @vida.max 3").contains("máximo"));
        assert!(falla("ONCLICK\nEXECUTE (1 + 2").contains("paréntesis"));
        assert!(falla("ONCLICK\nDO [ EXECUTE 1").contains("]"));
        assert!(falla("ONCLICK\nEXECUTE 1 2").contains("Sobra"));
        assert!(falla("ONCLICK\nDO QUIT IF 1d20").contains("comparación"));
        assert!(falla("ONCLICK\nEXECUTE 2*3").contains("dados"));
        let mut profundo = "ONCLICK\nEXECUTE ".to_string();
        profundo.push_str(&"(".repeat(40));
        assert!(falla(&profundo).contains("anidado"));
        assert!(falla(&format!("ONCLICK\n{}", "EXECUTE 1\n".repeat(70))).contains("líneas"));
        let e = leer("ONCLICK\nEXECUTE 1 +").unwrap_err();
        assert_eq!(e.linea, 2);
    }
}
