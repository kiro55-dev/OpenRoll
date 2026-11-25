// src-tauri/src/tablero_general/macros/interprete.rs
//
// Ejecuta una macro ya leída (sintaxis.rs). No toca el estado directamente: todo pasa por un `Mundo`, que da lo que se lee y
// recibe lo que se hace (deltas y líneas del chat). En la app es el estado con el lock cogido y cada delta se registra en el
// momento con `partida::registrar` (así la sentencia siguiente ya ve el valor nuevo); en las pruebas, un estado en memoria con
// dados fijos.
//
// Topes: pasos, dados y objetivos de un SPREAD, para que ninguna macro (tampoco la de un jugador) pueda colgar al host.

use super::sintaxis::{leer, AreaMacro, Condicion, Expr, Sentencia, Sujeto, Zona};
use crate::tablero_general::dados::{self, Tirada};
use crate::tablero_general::escalas::{self, Valoracion};
use crate::tablero_general::estado::{DeltaPartida, EstadoPartida};
use crate::tablero_general::instancias;
use crate::tablero_general::modelos::{
    AsociacionMacro, Casilla, Coordenada, DestinoMacro, Forma, IdJugador, Instancia, ValorParametro, ID_HOST,
};
use crate::tablero_general::tablero::{self, FormaZona};
use crate::tablero_general::variables;

/// Pasos (sentencias y términos evaluados) de una ejecución.
pub const MAX_PASOS: u32 = 1000;
/// Dados de una ejecución, todos juntos.
pub const MAX_DADOS: i64 = 999;
/// Entidades a las que llega un SPREAD.
pub const MAX_OBJETIVOS: usize = 100;
/// Atributos (`SAVE`) de una asociación.
pub const MAX_ATRIBUTOS: usize = 30;
/// Macros asociadas a una misma ficha o instancia.
pub const MAX_ASOCIACIONES_POR_DESTINO: usize = 10;
/// Tope de un atributo (como un valor actual).
pub const LIMITE_ATRIBUTO: i64 = instancias::LIMITE_VALOR_ACTUAL as i64;

/// Lo que la macro ve y hace.
pub trait Mundo {
    fn estado(&self) -> &EstadoPartida;
    /// Registra un cambio (en la app, por `partida::registrar`).
    fn aplicar(&mut self, delta: DeltaPartida);
    /// Una cara entre 1 y `caras`.
    fn tirar(&mut self, caras: i32) -> i32;
    /// Una línea del chat de tiradas.
    fn chat(&mut self, jugador: IdJugador, desglose: &str, total: i32, publica: bool, valoracion: Option<&Valoracion>, informativa: bool);
}

/// Un número ya calculado con su desglose para el chat.
struct Valor {
    texto: String,
    total: i64,
    /// `(cara, caras)` si el valor sale de UN solo dado normal (para críticos y pifias de la escala).
    natural: Option<(i32, i32)>,
    /// Cuántos términos de dados lleva.
    terminos_dados: u32,
}

struct Ejecucion {
    id_asociacion: u32,
    /// Quien la asoció: con su autoridad se buscan las macros de `SPREAD MACROS`.
    autoridad: IdJugador,
    /// Quien sale en el chat (quien pulsó el botón, o el autor de la asociación en un turno).
    quien: IdJugador,
    nombre: String,
    propia: u16,
    pasos: u32,
    dados: i64,
    parar: bool,
    /// Los atributos de `SAVE`: valen solo durante esta ejecución (no se guardan en la partida ni pasan a la siguiente).
    atributos: Vec<(String, i32)>,
    /// Los `AREA` de la macro y el centro que eligió el usuario para cada uno (en el mismo orden).
    areas: Vec<AreaMacro>,
    centros: Vec<Casilla>,
    /// Los lados ya calculados de cada `AREA` (se calculan una vez).
    lados_areas: Vec<Option<[i32; 4]>>,
}

/// Tope de cada lado de un `AREA`, en casillas.
pub const MAX_LADO_AREA: i64 = 100;

/// Un `AREA` de una macro, ya calculado con la ficha: su forma y sus lados `[arriba, derecha, abajo, izquierda]`. Es lo que necesita
/// el front para enseñar la vista previa bajo el ratón antes de ejecutar.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct AreaResuelta {
    pub forma: FormaZona,
    pub lados: [i32; 4],
}

/// Un mundo que solo deja leer (para calcular los lados de las áreas antes de ejecutar): no tira dados ni cambia nada.
struct SoloLectura<'a>(&'a EstadoPartida);

impl Mundo for SoloLectura<'_> {
    fn estado(&self) -> &EstadoPartida {
        self.0
    }
    fn aplicar(&mut self, _delta: DeltaPartida) {}
    fn tirar(&mut self, _caras: i32) -> i32 {
        1
    }
    fn chat(&mut self, _: IdJugador, _: &str, _: i32, _: bool, _: Option<&Valoracion>, _: bool) {}
}

/// Las áreas que hay que apuntar antes de ejecutar la macro de esa asociación sobre esa instancia, con sus lados ya calculados.
pub fn areas_de(est: &EstadoPartida, id_asociacion: u32, id_instancia: u16) -> Result<Vec<AreaResuelta>, String> {
    let asoc = est.asociaciones_macro.iter().find(|a| a.id == id_asociacion).ok_or("Esa macro ya no está asociada")?;
    let mac = est.macros.iter().find(|m| m.id == asoc.id_macro).ok_or("Esa macro ya no existe")?;
    let programa = leer(&mac.codigo).map_err(|e| e.to_string())?;
    if programa.areas.is_empty() {
        return Ok(Vec::new());
    }
    instancia(est, id_instancia)?;
    let mut ej = Ejecucion::nueva(id_asociacion, asoc.autor, ID_HOST, limpio(&mac.nombre), id_instancia, programa.areas, Vec::new());
    let mut mundo = SoloLectura(est);
    (0..ej.areas.len())
        .map(|i| Ok(AreaResuelta { forma: ej.areas[i].forma, lados: ej.lados_area(&mut mundo, i)? }))
        .collect()
}

/// Quita del texto lo que pudiera pasar por una etiqueta en el desglose (el chat lo escapa igual).
fn limpio(texto: &str) -> String {
    texto.chars().filter(|c| !matches!(c, '<' | '>' | '&' | '"')).collect()
}

fn sin_espacios(texto: &str) -> String {
    texto.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

fn instancia<'a>(est: &'a EstadoPartida, id: u16) -> Result<&'a Instancia, String> {
    est.instancias_activas.iter().find(|i| i.id_instancia == id).ok_or_else(|| "La ficha de la macro ya no está activa".to_string())
}

fn casilla(inst: &Instancia) -> Option<crate::tablero_general::modelos::Casilla> {
    match &inst.coordenadas {
        Coordenada::Casilla(c) => Some(*c),
        _ => None,
    }
}

/// El parámetro visual que nombra la macro: por su nombre (sin mayúsculas) o, si no, con los espacios quitados (`@rangoarma`).
fn definicion_parametro<'a>(est: &'a EstadoPartida, nombre: &str) -> Result<&'a crate::tablero_general::modelos::DefinicionParametro, String> {
    let clave = dados::clave_parametro(nombre);
    let junto = sin_espacios(nombre);
    est.definiciones_parametros
        .iter()
        .find(|d| dados::clave_parametro(&d.clave) == clave)
        .or_else(|| est.definiciones_parametros.iter().find(|d| sin_espacios(&d.clave) == junto))
        .ok_or_else(|| format!("«@{nombre}» no es un parámetro"))
}

/// Etiqueta y valor de `.nombre` (variable) en la ficha `inst`.
fn valor_variable(est: &EstadoPartida, inst: &Instancia, nombre: &str) -> Result<(String, i32), String> {
    let junto = sin_espacios(nombre);
    let def = variables::buscar(&est.definiciones_variables, nombre)
        .or_else(|| {
            est.definiciones_variables
                .iter()
                .find(|d| sin_espacios(&d.nombre) == junto || (!d.abreviatura.is_empty() && sin_espacios(&d.abreviatura) == junto))
        })
        .ok_or_else(|| format!("«.{nombre}» no es una variable"))?;
    let etiqueta = if def.abreviatura.is_empty() { &def.nombre } else { &def.abreviatura };
    Ok((etiqueta.clone(), variables::resultado(&est.definiciones_variables, &inst.base.parametros, &inst.base.variables, def)))
}

/// El texto de `@nombre` en la ficha `inst` (su etiqueta y su valor): un parámetro visual de texto o, si no hay ningún parámetro con ese
/// nombre, los campos fijos `@nombre` (el de la ficha) y `@tipo` (Enemigo, NPC, Jugador, Objeto).
fn valor_texto(est: &EstadoPartida, inst: &Instancia, nombre: &str) -> Result<(String, String), String> {
    if let Ok(def) = definicion_parametro(est, nombre) {
        if !matches!(def.valor_por_defecto, ValorParametro::Texto(_)) {
            return Err(format!("«{}» es un número: se compara con números, no con un texto", def.clave));
        }
        let valor = inst.base.parametros.iter().find(|p| p.id_definicion == def.id).and_then(|p| match &p.valor {
            ValorParametro::Texto(t) => Some(t.clone()),
            ValorParametro::Numero(_) => None,
        });
        return Ok((def.clave.clone(), valor.unwrap_or_default()));
    }
    match sin_espacios(nombre).as_str() {
        "nombre" => Ok(("nombre".into(), inst.base.nombre.clone())),
        "tipo" => Ok(("tipo".into(), format!("{:?}", inst.base.tipo))),
        _ => Err(format!("«@{nombre}» no es un parámetro de texto (ni @nombre ni @tipo)")),
    }
}

fn valor_parametro(est: &EstadoPartida, inst: &Instancia, nombre: &str, max: bool) -> Result<(String, i32), String> {
    let def = definicion_parametro(est, nombre)?;
    dados::valor_de_parametro(est, inst, &format!("{}{}", def.clave, if max { ".max" } else { "" }))
}

/// Ejecuta la asociación `id_asociacion` sobre la instancia `id_instancia`. `quien` es quien sale en el chat. Un error para la macro y
/// sale en el chat (en privado: solo lo ven el host y `quien`); lo ya hecho se queda.
pub fn ejecutar(mundo: &mut dyn Mundo, id_asociacion: u32, id_instancia: u16, quien: IdJugador, centros: &[Casilla]) -> Result<(), String> {
    let est = mundo.estado();
    let asoc: AsociacionMacro =
        est.asociaciones_macro.iter().find(|a| a.id == id_asociacion).cloned().ok_or("Esa macro ya no está asociada")?;
    let mac = est.macros.iter().find(|m| m.id == asoc.id_macro).ok_or("Esa macro ya no existe")?;
    let nombre = limpio(&mac.nombre);
    instancia(est, id_instancia)?;
    let programa = leer(&mac.codigo).map_err(|e| format!("{nombre}: {e}"))?;
    let mut ej = Ejecucion::nueva(id_asociacion, asoc.autor, quien, nombre, id_instancia, programa.areas.clone(), centros.to_vec());
    match ej.bloque(mundo, &programa.cuerpo, None) {
        Ok(()) => Ok(()),
        Err(e) => {
            mundo.chat(quien, &format!("{}: error: {}", ej.nombre, limpio(&e)), 0, false, None, true);
            Err(e)
        }
    }
}

impl Ejecucion {
    fn nueva(id_asociacion: u32, autoridad: IdJugador, quien: IdJugador, nombre: String, propia: u16, areas: Vec<AreaMacro>, centros: Vec<Casilla>) -> Self {
        let lados_areas = vec![None; areas.len()];
        Ejecucion { id_asociacion, autoridad, quien, nombre, propia, pasos: 0, dados: 0, parar: false, atributos: Vec::new(), areas, centros, lados_areas }
    }

    /// Los lados `[arriba, derecha, abajo, izquierda]` del `AREA` número `i` con los valores de la ficha de la macro: 1 lado = todos;
    /// 2 = horizontal (izquierda y derecha) y vertical (arriba y abajo); 4 = arriba, derecha, abajo, izquierda (como CSS).
    fn lados_area(&mut self, m: &mut dyn Mundo, i: usize) -> Result<[i32; 4], String> {
        if let Some(Some(l)) = self.lados_areas.get(i) {
            return Ok(*l);
        }
        let exprs = self.areas.get(i).ok_or("Esa área no existe")?.lados.clone();
        let mut v = Vec::with_capacity(exprs.len());
        for e in &exprs {
            v.push(self.evaluar(m, e, None)?.total.clamp(0, MAX_LADO_AREA) as i32);
        }
        let lados = match v.as_slice() {
            [a] => [*a; 4],
            [h, vert] => [*vert, *h, *vert, *h],
            [arriba, derecha, abajo, izquierda] => [*arriba, *derecha, *abajo, *izquierda],
            _ => return Err("Un AREA lleva 1, 2 o 4 lados".into()),
        };
        if let Some(hueco) = self.lados_areas.get_mut(i) {
            *hueco = Some(lados);
        }
        Ok(lados)
    }

    fn paso(&mut self) -> Result<(), String> {
        self.pasos += 1;
        if self.pasos > MAX_PASOS {
            return Err(format!("La macro hace demasiadas cosas de golpe (más de {MAX_PASOS} pasos)"));
        }
        Ok(())
    }

    /// Lo que va delante de cada línea del chat: el nombre de la macro y, en un SPREAD, la entidad.
    fn prefijo(&self, m: &dyn Mundo, candidata: Option<u16>) -> String {
        match candidata.and_then(|c| m.estado().instancias_activas.iter().find(|i| i.id_instancia == c)) {
            Some(i) => format!("{} → {}: ", self.nombre, limpio(&i.base.nombre)),
            None => format!("{}: ", self.nombre),
        }
    }

    fn bloque(&mut self, m: &mut dyn Mundo, lista: &[Sentencia], candidata: Option<u16>) -> Result<(), String> {
        for s in lista {
            if self.parar {
                break;
            }
            self.sentencia(m, s, candidata)?;
        }
        Ok(())
    }

    fn sentencia(&mut self, m: &mut dyn Mundo, s: &Sentencia, candidata: Option<u16>) -> Result<(), String> {
        self.paso()?;
        match s {
            Sentencia::Mostrar { expr, privada } => {
                let v = self.evaluar(m, expr, candidata)?;
                let total = i32::try_from(v.total).map_err(|_| "El resultado es demasiado grande")?;
                let texto = format!("{}{}", self.prefijo(m, candidata), v.texto);
                m.chat(self.quien, &texto, total, !privada, None, false);
            }
            Sentencia::Guardar { nombre, expr } => {
                let v = self.evaluar(m, expr, candidata)?;
                let valor = v.total.clamp(-LIMITE_ATRIBUTO, LIMITE_ATRIBUTO) as i32;
                match self.atributos.iter().position(|(n, _)| n.eq_ignore_ascii_case(nombre)) {
                    Some(i) => self.atributos[i].1 = valor,
                    None if self.atributos.len() >= MAX_ATRIBUTOS => {
                        return Err(format!("Una macro no puede guardar más de {MAX_ATRIBUTOS} atributos"));
                    }
                    None => self.atributos.push((nombre.clone(), valor)),
                }
            }
            Sentencia::Cambiar { de, param, expr, relativo } => {
                let objetivo = match de {
                    Sujeto::Propia => self.propia,
                    Sujeto::Candidata => candidata.ok_or("ENTITIES solo vale dentro de un SPREAD")?,
                };
                let (id_def, actual) = {
                    let est = m.estado();
                    let def = definicion_parametro(est, param)?;
                    if !def.tiene_maximo || !matches!(def.valor_por_defecto, ValorParametro::Numero(_)) {
                        return Err(format!("CHANGE solo cambia parámetros con máximo (su valor actual): «{}» no lo tiene", def.clave));
                    }
                    let inst = instancia(est, objetivo)?;
                    let actual = inst.valores_actuales.iter().find(|v| v.id_definicion == def.id).map(|v| i64::from(v.valor));
                    (def.id, actual.ok_or_else(|| format!("«{}» no tiene valor actual en esa ficha", def.clave))?)
                };
                let v = self.evaluar(m, expr, candidata)?;
                let cambio = if *relativo { v.total } else { v.total - actual };
                let cambio = cambio.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
                if let Some(d) = instancias::cambiar_valor_actual(m.estado(), objetivo, id_def, cambio)? {
                    m.aplicar(d);
                }
            }
            Sentencia::Quitar => {
                m.aplicar(DeltaPartida::MacroDesasociada(self.id_asociacion));
                self.parar = true;
            }
            Sentencia::Si { cuerpo, condicion, negada, sino } => {
                let cumple = match condicion {
                    None => true,
                    // IFNOT: el cuerpo es para cuando la condición NO se cumple
                    Some(c) => self.condicion(m, c, candidata)? != *negada,
                };
                self.bloque(m, if cumple { cuerpo } else { sino }, candidata)?;
            }
            Sentencia::Repartir { accion, condicion } => self.repartir(m, accion, condicion)?,
            Sentencia::Asociar(nombres) => {
                let destino = candidata.ok_or("MACROS solo vale dentro de un SPREAD")?;
                for n in nombres {
                    self.asociar(m, n, destino)?;
                }
            }
        }
        Ok(())
    }

    /// `SPREAD acción TO condición`: la acción se hace en cada entidad del mapa (no oculta, colocada, sin la propia) que cumple.
    /// Cada entidad tira sus propios dados; si la condición no mira a ninguna entidad, se evalúa una vez y vale para todas.
    fn repartir(&mut self, m: &mut dyn Mundo, accion: &Sentencia, condicion: &Condicion) -> Result<(), String> {
        let candidatas: Vec<u16> = {
            let est = m.estado();
            let propia = instancia(est, self.propia)?;
            let Some(mapa) = propia.id_mapa.filter(|_| casilla(propia).is_some()) else {
                return Err("SPREAD necesita que la ficha esté en el tablero".into());
            };
            let mut l: Vec<&Instancia> = est
                .instancias_activas
                .iter()
                .filter(|i| i.id_mapa == Some(mapa) && !i.oculto && i.id_instancia != self.propia && casilla(i).is_some())
                .collect();
            l.sort_by_key(|i| i.orden);
            l.into_iter().take(MAX_OBJETIVOS).map(|i| i.id_instancia).collect()
        };
        let para_todas = !condicion.usa_candidata();
        let todas = para_todas && self.condicion(m, condicion, None)?;
        for c in candidatas {
            if self.parar {
                break;
            }
            // Pudo desaparecer por algo que hizo la propia macro
            if !m.estado().instancias_activas.iter().any(|i| i.id_instancia == c) {
                continue;
            }
            let cumple = if para_todas { todas } else { self.condicion(m, condicion, Some(c))? };
            if cumple {
                self.sentencia(m, accion, Some(c))?;
            }
        }
        Ok(())
    }

    /// `MACROS.nombre`: asocia a la instancia `destino` la macro de ese nombre (una del mismo autor; el host puede repartir cualquiera).
    fn asociar(&mut self, m: &mut dyn Mundo, nombre: &str, destino: u16) -> Result<(), String> {
        let est = m.estado();
        let buscado = sin_espacios(nombre);
        let ficha = instancia(est, self.propia)?.base.id;
        let por_nombre = |autor: Option<IdJugador>| {
            // Solo las del conjunto que se está usando, y de este personaje o generales
            est.macros.iter().find(|x| {
                sin_espacios(&x.nombre) == buscado
                    && (x.es_general() || autor.is_none_or(|a| x.autor == a))
                    && x.propietaria.is_none_or(|p| p == ficha)
                    && super::activa(est, x)
            })
        };
        let mac = por_nombre(Some(self.autoridad))
            .or_else(|| if self.autoridad == ID_HOST { por_nombre(None) } else { None })
            .ok_or_else(|| format!("No hay ninguna macro tuya que se llame «{nombre}»"))?;
        let de_ahi: Vec<&AsociacionMacro> = est.asociaciones_macro.iter().filter(|a| a.destino == DestinoMacro::Instancia(destino)).collect();
        if de_ahi.iter().any(|a| a.id_macro == mac.id) {
            return Ok(()); // ya la tiene: no se repite (el veneno no se acumula)
        }
        if de_ahi.len() >= MAX_ASOCIACIONES_POR_DESTINO {
            return Ok(());
        }
        let id = est.asociaciones_macro.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        let id_macro = mac.id;
        m.aplicar(DeltaPartida::MacroAsociada(AsociacionMacro {
            id,
            id_macro,
            destino: DestinoMacro::Instancia(destino),
            autor: self.autoridad,
        }));
        Ok(())
    }

    fn condicion(&mut self, m: &mut dyn Mundo, c: &Condicion, candidata: Option<u16>) -> Result<bool, String> {
        self.paso()?;
        match c {
            Condicion::Y(a, b) => Ok(self.condicion(m, a, candidata)? && self.condicion(m, b, candidata)?),
            Condicion::O(a, b) => Ok(self.condicion(m, a, candidata)? || self.condicion(m, b, candidata)?),
            Condicion::Dentro { zona: Zona::Area(i) } => {
                let cand = candidata.ok_or("ENTITIES IN solo vale dentro de un SPREAD")?;
                let lados = self.lados_area(m, *i)?;
                let centro = *self.centros.get(*i).ok_or("Apunta el área en el tablero (pulsa el botón de la macro y elige dónde)")?;
                let forma = self.areas[*i].forma;
                let est = m.estado();
                let otra = instancia(est, cand)?;
                let Some(b) = casilla(otra) else { return Ok(false) };
                Ok(tablero::cuerpo_en_caja(forma, tablero::extremos_area(centro, lados), b, tablero::lados_instancia(otra)))
            }
            Condicion::CompararTexto { de, param, distinto, texto, mostrar } => {
                let id = match de {
                    Sujeto::Propia => self.propia,
                    Sujeto::Candidata => candidata.ok_or("ENTITIES solo vale dentro de un SPREAD")?,
                };
                let (etiqueta, valor) = {
                    let est = m.estado();
                    valor_texto(est, instancia(est, id)?, param)?
                };
                let iguales = valor.trim().to_lowercase() == texto.trim().to_lowercase();
                let acierta = iguales != *distinto;
                if let Some(privada) = mostrar {
                    let v = Valoracion { nombre: if acierta { "Éxito" } else { "Fallo" }.into(), acierta, especial: false };
                    let texto_chat = format!(
                        "{}{} ({}) {} \"{}\"",
                        self.prefijo(m, candidata),
                        limpio(&etiqueta),
                        limpio(&valor),
                        if *distinto { "!=" } else { "=" },
                        limpio(texto)
                    );
                    m.chat(self.quien, &texto_chat, 0, !privada, Some(&v), false);
                }
                Ok(acierta)
            }
            Condicion::Dentro { zona: Zona::Parametro(param) } => {
                let cand = candidata.ok_or("ENTITIES IN solo vale dentro de un SPREAD")?;
                let est = m.estado();
                let propia = instancia(est, self.propia)?;
                let otra = instancia(est, cand)?;
                let def = definicion_parametro(est, param)?;
                let forma = match def.representar_mapa {
                    Some(Forma::Cuadrado(_)) => FormaZona::Cuadrado,
                    Some(Forma::Rombo(_)) => FormaZona::Rombo,
                    Some(Forma::Circulo(_)) => FormaZona::Circulo,
                    _ => return Err(format!("«{}» no dibuja ninguna zona en el tablero (cuadrado, rombo o círculo)", def.clave)),
                };
                let (_, n) = valor_parametro(est, propia, param, false)?;
                let (Some(a), Some(b)) = (casilla(propia), casilla(otra)) else { return Ok(false) };
                Ok(tablero::cuerpo_en_forma(forma, a, tablero::lados_instancia(propia), n, b, tablero::lados_instancia(otra)))
            }
            Condicion::Comparar { izq, cmp, der, mostrar } => {
                let a = self.evaluar(m, izq, candidata)?;
                let b = self.evaluar(m, der, candidata)?;
                // El sujeto es el lado con dados (el izquierdo si los dos tiran), como en el chat
                let (sujeto, objetivo, cmp_sujeto) = if a.terminos_dados > 0 || b.terminos_dados == 0 { (&a, &b, *cmp) } else { (&b, &a, cmp.invertida()) };
                let total = i32::try_from(sujeto.total).map_err(|_| "El resultado es demasiado grande")?;
                let meta = i32::try_from(objetivo.total).map_err(|_| "El resultado es demasiado grande")?;
                let valoracion = match cmp_sujeto.operador() {
                    Some(op) => escalas::valorar(m.estado().escala_resultado.as_ref(), op, total, meta, sujeto.natural),
                    None => {
                        let acierta = cmp.cumple(a.total, b.total);
                        Valoracion { nombre: if acierta { "Éxito" } else { "Fallo" }.into(), acierta, especial: false }
                    }
                };
                if let Some(privada) = mostrar {
                    let texto = format!("{}{} {} {}", self.prefijo(m, candidata), a.texto, cmp.simbolo(), b.texto);
                    m.chat(self.quien, &texto, total, !privada, Some(&valoracion), false);
                }
                Ok(valoracion.acierta)
            }
        }
    }

    fn evaluar(&mut self, m: &mut dyn Mundo, e: &Expr, candidata: Option<u16>) -> Result<Valor, String> {
        self.paso()?;
        let simple = |texto: String, total: i64| Valor { texto, total, natural: None, terminos_dados: 0 };
        let propia = self.propia;
        let de = |sujeto: Sujeto| -> Result<u16, String> {
            match sujeto {
                Sujeto::Propia => Ok(propia),
                Sujeto::Candidata => candidata.ok_or_else(|| "ENTITIES solo vale dentro de un SPREAD".to_string()),
            }
        };
        Ok(match e {
            Expr::Num(n) => simple(n.to_string(), *n),
            Expr::Var { de: s, nombre } => {
                let id = de(*s)?;
                let est = m.estado();
                let (etq, v) = valor_variable(est, instancia(est, id)?, nombre)?;
                simple(format!("{} ({:+})", limpio(&etq), v), i64::from(v))
            }
            Expr::Param { de: s, nombre, max } => {
                let id = de(*s)?;
                let est = m.estado();
                let (etq, v) = valor_parametro(est, instancia(est, id)?, nombre, *max)?;
                simple(format!("{etq} ({v:+})"), i64::from(v))
            }
            Expr::Atributo(nombre) => {
                let v = self
                    .atributos
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(nombre))
                    .map(|(_, v)| *v)
                    .ok_or_else(|| format!("El atributo «{nombre}» aún no tiene valor en esta ejecución (se le da con SAVE {nombre} … antes de usarlo)"))?;
                simple(format!("{} ({:+})", limpio(nombre), v), i64::from(v))
            }
            Expr::Turno => {
                let est = m.estado();
                let propia = instancia(est, self.propia)?;
                let ronda = est
                    .mapas
                    .iter()
                    .filter(|mp| Some(mp.id) == propia.id_mapa)
                    .filter_map(|mp| mp.batalla.as_ref())
                    .flat_map(|b| &b.grupos)
                    .find(|g| g.miembros.contains(&self.propia))
                    .map(|g| g.ronda)
                    .ok_or("TURN solo vale en batalla (la ficha no está en ningún grupo de batalla)")?;
                simple(format!("turno ({ronda})"), i64::from(ronda))
            }
            Expr::Distancia => {
                let cand = candidata.ok_or("DISTANCE solo vale dentro de un SPREAD")?;
                let est = m.estado();
                let (a, b) = (instancia(est, self.propia)?, instancia(est, cand)?);
                let (Some(ca), Some(cb)) = (casilla(a), casilla(b)) else { return Err("DISTANCE necesita las dos fichas en el tablero".into()) };
                let d = tablero::distancia_cuerpos(ca, tablero::lados_instancia(a), cb, tablero::lados_instancia(b));
                simple(format!("distancia ({d})"), i64::from(d))
            }
            Expr::Dado { veces, caras, ventaja, desventaja, multiplicador } => {
                self.dados += i64::from(*veces);
                if self.dados > MAX_DADOS {
                    return Err(format!("Una macro no puede tirar más de {MAX_DADOS} dados de golpe"));
                }
                let t = Tirada {
                    signo: 1,
                    veces: *veces,
                    d: *caras,
                    bonificador: None,
                    variable: None,
                    ventaja: *ventaja,
                    desventaja: *desventaja,
                    multiplicador: *multiplicador,
                };
                let mut tirar = |c: i32| m.tirar(c);
                let (texto, total, natural) = dados::tirar_termino(&t, &mut tirar)?;
                let normal = *veces == 1 && *multiplicador == 1 && !ventaja && !desventaja;
                Valor { texto, total, natural: if normal { natural } else { None }, terminos_dados: 1 }
            }
            Expr::Neg(a) => {
                let v = self.evaluar(m, a, candidata)?;
                Valor { texto: format!("- {}", v.texto), total: -v.total, natural: None, terminos_dados: v.terminos_dados }
            }
            Expr::Grupo(a) => {
                let v = self.evaluar(m, a, candidata)?;
                Valor { texto: format!("({})", v.texto), ..v }
            }
            Expr::Suma(a, b) | Expr::Resta(a, b) => {
                let resta = matches!(e, Expr::Resta(..));
                let x = self.evaluar(m, a, candidata)?;
                let y = self.evaluar(m, b, candidata)?;
                let total = if resta { x.total.checked_sub(y.total) } else { x.total.checked_add(y.total) }.ok_or("El resultado es demasiado grande")?;
                let terminos = x.terminos_dados + y.terminos_dados;
                // La cara natural solo cuenta si todo el argumento tira UN dado, y sumando
                let natural = if terminos == 1 { if y.terminos_dados == 1 && resta { None } else { x.natural.or(y.natural) } } else { None };
                Valor { texto: format!("{} {} {}", x.texto, if resta { "-" } else { "+" }, y.texto), total, natural, terminos_dados: terminos }
            }
        })
    }
}
