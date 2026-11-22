// src-tauri/src/tablero_general/modelos.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum TipoEntidad {
    Enemigo,
    NPC,
    Jugador,
    Objeto,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Cuerpo {
    Grande(Vec<u8>),
    Pequenio,
}

/// Casilla del tablero, como en una hoja de cálculo: (0,0) arriba a la izquierda, x hacia la
/// derecha, y hacia abajo. La lógica vive en `tablero.rs`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Casilla {
    pub x: u16,
    pub y: u16,
}

/// Dónde está una instancia. En una ficha de varias casillas es la casilla CENTRAL
/// (ver `tablero::extension`), para que pueda girar sobre sí misma.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(from = "CoordenadaGuardada")]
pub enum Coordenada {
    Casilla(Casilla),
    NoPuesto,
}

/// Forma en que se LEE una `Coordenada`: la actual más la antigua `Localizacion([x, y])`.
#[derive(Deserialize)]
enum CoordenadaGuardada {
    Casilla(Casilla),
    NoPuesto,
    Localizacion(Vec<i16>),
}

impl From<CoordenadaGuardada> for Coordenada {
    fn from(g: CoordenadaGuardada) -> Self {
        match g {
            CoordenadaGuardada::Casilla(c) => Coordenada::Casilla(c),
            CoordenadaGuardada::NoPuesto => Coordenada::NoPuesto,
            CoordenadaGuardada::Localizacion(v) => match v[..] {
                [x, y, ..] => Coordenada::Casilla(Casilla { x: x.max(0) as u16, y: y.max(0) as u16 }),
                _ => Coordenada::NoPuesto,
            },
        }
    }
}

// ========================================================
// PARÁMETROS
// ========================================================
//
// Tres niveles:
//  * Partida   -> `DefinicionParametro`: qué parámetros existen (común a todas las fichas).
//  * Ficha     -> `Parametro`: el valor de esa ficha y si se ha hecho público.
//  * Instancia -> `ValorActual`: lo que cambia durante la partida. Solo lo tienen los parámetros
//                 con `tiene_maximo` (p. ej. la vida): el valor de la ficha es el máximo y cada
//                 instancia lleva su valor actual.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(from = "DefinicionGuardada")]
pub struct DefinicionParametro {
    pub id: u16,
    /// Nombre que se muestra: "Vida", "Armadura", "Cordura"...
    pub clave: String,
    /// Da el tipo (número o texto) y el valor con el que nace una ficha nueva.
    pub valor_por_defecto: ValorParametro,
    /// El valor de la ficha es el máximo; cada instancia guarda su valor actual. Solo números.
    #[serde(default)]
    pub tiene_maximo: bool,
    /// Cómo se dibuja en el tablero al seleccionar la instancia (None = no se dibuja).
    #[serde(default)]
    pub representar_mapa: Option<Forma>,
    pub visibilidad: Visibilidad,
    /// Texto tras el valor, p. ej. "m".
    #[serde(default)]
    pub unidad: Option<String>,
    /// Solo con `tiene_maximo`: si el valor actual de una instancia llega a 0 o menos, la ficha se ve
    /// atenuada (en gris) en el tablero y en «Fichas activas». Es lo que hace la vida.
    pub atenuar_si_negativo: bool,
    /// Si lo hay, el valor de este parámetro NO se escribe: se calcula con las variables de la ficha (los puntos de vida de
    /// Cthulhu = (CON + TAM) / 10). Solo números. Lo recalcula Rust al guardar la ficha y al cambiar las variables.
    pub calculo: Option<CalculoParametro>,
}

/// Cómo se calcula un parámetro a partir de las variables de la ficha: `floor(suma · multiplicador / divisor) + constante`,
/// donde `suma` es el valor de cada variable de la lista (en una base, el valor escrito; en una derivada, su resultado).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CalculoParametro {
    pub variables: Vec<u16>,
    #[serde(default = "uno")]
    pub multiplicador: i16,
    #[serde(default = "uno")]
    pub divisor: i16,
    #[serde(default)]
    pub constante: i16,
}

fn uno() -> i16 {
    1
}

/// Forma en que se LEE una `DefinicionParametro`: las partidas guardadas antes de `atenuar_si_negativo`
/// no lo traen, y su Vida de serie debe seguir atenuando como hasta ahora.
#[derive(Deserialize)]
struct DefinicionGuardada {
    id: u16,
    clave: String,
    valor_por_defecto: ValorParametro,
    #[serde(default)]
    tiene_maximo: bool,
    #[serde(default)]
    representar_mapa: Option<Forma>,
    visibilidad: Visibilidad,
    #[serde(default)]
    unidad: Option<String>,
    #[serde(default)]
    atenuar_si_negativo: Option<bool>,
    #[serde(default)]
    calculo: Option<CalculoParametro>,
}

impl From<DefinicionGuardada> for DefinicionParametro {
    fn from(g: DefinicionGuardada) -> Self {
        DefinicionParametro {
            atenuar_si_negativo: g.atenuar_si_negativo.unwrap_or(g.id == ID_VIDA && g.tiene_maximo),
            id: g.id,
            clave: g.clave,
            valor_por_defecto: g.valor_por_defecto,
            tiene_maximo: g.tiene_maximo,
            representar_mapa: g.representar_mapa,
            visibilidad: g.visibilidad,
            unidad: g.unidad,
            calculo: g.calculo,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Visibilidad {
    SiemprePublico,
    SiemprePrivado,
    /// Cada ficha decide (casilla "Hacer pública...").
    Variable,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Forma {
    //en todos los casos vec<u8> representan RGBA
    Rombo(Vec<u8>),
    Circulo(Vec<u8>),
    Cuadrado(Vec<u8>),
    BarraVida(Vec<u8>),
    /// Solo con `tiene_maximo`: en «Fichas activas» se muestra «actual/máx» en este color.
    Resumen(Vec<u8>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ValorParametro {
    Numero(i16),
    Texto(String),
}

impl ValorParametro {
    pub fn mismo_tipo(&self, otro: &ValorParametro) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(otro)
    }
}

/// Valor de un parámetro en una ficha.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Parametro {
    pub id_definicion: u16,
    pub valor: ValorParametro,
    /// Si los jugadores lo ven. Con visibilidad fija siempre coincide con ella (ver `normalizar_parametros`).
    #[serde(default)]
    pub publico: bool,
}

/// Valor actual, en una instancia, de un parámetro con máximo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ValorActual {
    pub id_definicion: u16,
    pub valor: i16,
}

// Ids de los parámetros por defecto. Son fijos: la migración de fichas antiguas depende de ellos.
pub const ID_VIDA: u16 = 1;
pub const ID_ARMADURA: u16 = 2;
pub const ID_VELOCIDAD: u16 = 3;
pub const ID_RANGO_ARMA: u16 = 4;

/// Los parámetros con los que nace una partida (los que antes eran campos fijos de `Entidad`).
pub fn parametros_por_defecto() -> Vec<DefinicionParametro> {
    let def = |id, clave: &str, valor, tiene_maximo, representar_mapa, visibilidad, unidad: Option<&str>| {
        DefinicionParametro {
            id,
            clave: clave.into(),
            valor_por_defecto: ValorParametro::Numero(valor),
            tiene_maximo,
            representar_mapa,
            visibilidad,
            unidad: unidad.map(Into::into),
            // La vida: a 0 o menos, la ficha se ve atenuada
            atenuar_si_negativo: id == ID_VIDA,
            calculo: None,
        }
    };
    vec![
        def(ID_VIDA, "Vida", 10, true, Some(Forma::Resumen(vec![34, 177, 76, 255])), Visibilidad::Variable, None),
        def(ID_ARMADURA, "Armadura", 10, false, None, Visibilidad::Variable, None),
        def(
            ID_VELOCIDAD,
            "Velocidad",
            6,
            false,
            Some(Forma::Rombo(vec![59, 130, 246, 90])),
            Visibilidad::SiemprePublico,
            Some("m"),
        ),
        def(
            ID_RANGO_ARMA,
            "Rango arma",
            1,
            false,
            Some(Forma::Circulo(vec![239, 68, 68, 90])),
            Visibilidad::SiemprePublico,
            Some("m"),
        ),
    ]
}

/// Deja un parámetro por definición y en su orden: añade con su valor por defecto los que faltan,
/// quita los de definiciones que ya no existen (o repetidos) y ajusta `publico` a la visibilidad fija.
pub fn normalizar_parametros(defs: &[DefinicionParametro], parametros: &mut Vec<Parametro>) {
    let mut anteriores = std::mem::take(parametros);
    for d in defs {
        let mut p = match anteriores.iter().position(|p| p.id_definicion == d.id) {
            Some(i) => anteriores.swap_remove(i),
            None => Parametro { id_definicion: d.id, valor: d.valor_por_defecto.clone(), publico: false },
        };
        match d.visibilidad {
            Visibilidad::SiemprePublico => p.publico = true,
            Visibilidad::SiemprePrivado => p.publico = false,
            Visibilidad::Variable => {}
        }
        parametros.push(p);
    }
}

// ========================================================
// VARIABLES
// ========================================================
//
// Dos niveles (la lógica está en `variables.rs`):
//  * Partida -> `DefinicionVariable`: qué variables existen (Fuerza «f», neutro 10, escala 2...).
//  * Ficha   -> `ValorVariable`: su valor (12). El resultado (+1) se calcula, no se guarda, y es lo que vale
//               la variable en una tirada del chat (`2d6 + f`).

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DefinicionVariable {
    pub id: u16,
    /// Nombre completo («Fuerza»); también vale en el chat.
    pub nombre: String,
    /// Abreviatura («f»): lo normal en el chat y lo que sale en el desglose de la tirada.
    pub abreviatura: String,
    /// Valor con el que el resultado es 0.
    pub neutro: i16,
    /// Cuántos puntos por encima (o por debajo) del neutro suman (o restan) 1. Siempre ≥ 1.
    pub escala: i16,
    /// El grupo al que pertenece (`GrupoVariables.id`). Las partidas antiguas traen 0: se pasan al primer grupo de bases.
    #[serde(default)]
    pub grupo: u16,
    /// Solo en un grupo de derivadas: las variables BASE cuyos resultados se suman (al menos una).
    #[serde(default)]
    pub bases: Vec<u16>,
    /// Solo en un grupo de derivadas: variables BASE que cada ficha puede sumar o no (un check «+ com»).
    #[serde(default)]
    pub opcionales: Vec<u16>,
    /// Solo en una derivada: `resultado = floor(suma · multiplicador / divisor) + constante (+ el bono del rango elegido)`,
    /// donde `suma` son los resultados de sus bases y de las opcionales marcadas. Sin tocar son 1, 1 y 0 (la suma tal cual).
    #[serde(default = "uno")]
    pub multiplicador: i16,
    #[serde(default = "uno")]
    pub divisor: i16,
    #[serde(default)]
    pub constante: i16,
    /// Solo en una derivada: la LISTA DE RANGOS (`ListaRangos.id`) de la que cada ficha elige UNO (Sin entrenar +0, Entrenado +2...). Varias
    /// variables comparten una lista: se define una vez y se cambia en un solo sitio.
    #[serde(default)]
    pub rangos: Option<u16>,
    /// Copia de las opciones de su lista de rangos (`rangos`), que Rust deja al día en `rangos::normalizar_rangos` para que la ficha, el chat y
    /// los jugadores no necesiten buscar la lista. Nunca se escribe a mano. Si, al leer una partida o un archivo antiguos, una variable trae
    /// opciones propias sin lista, pasan a una lista. Si tiene opciones, cada ficha elige UNA y su bono se suma al
    /// resultado. Vacío = no hay elección.
    #[serde(default)]
    pub opciones: Vec<OpcionVariable>,
    /// Solo en una derivada: parámetros visuales (`DefinicionParametro.id`) cuyo valor se suma tal cual, sin neutro ni escala.
    /// Solo vale un parámetro numérico escrito a mano (uno calculado ya depende de las variables: sería un ciclo).
    #[serde(default)]
    pub parametros: Vec<u16>,
    /// Solo en una derivada: parámetros que cada ficha puede sumar o no (un check «+ armadura»).
    #[serde(default)]
    pub parametros_opcionales: Vec<u16>,
}

/// Una opción de una variable derivada con rangos (la competencia de Pathfinder: Entrenado +2, Experto +4...).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OpcionVariable {
    pub nombre: String,
    pub bonus: i16,
}

/// Una lista de rangos con nombre («Competencia»: Sin entrenar +0, Entrenado +2, Experto +4...). Las variables derivadas la usan por su id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ListaRangos {
    pub id: u16,
    pub nombre: String,
    pub opciones: Vec<OpcionVariable>,
}

/// Los dos tipos de grupo: variables con valor propio (Fuerza...) o calculadas a partir de ellas (Acrobacias...).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoGrupoVariables {
    Base,
    Derivadas,
}

/// Un grupo de variables («Características», «Habilidades»...): una subpestaña en su definición y una sección en la ficha.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GrupoVariables {
    pub id: u16,
    pub nombre: String,
    pub tipo: TipoGrupoVariables,
}

/// Un CONJUNTO DE PARÁMETROS: todo lo que define cómo es el juego al que se juega (los parámetros visuales, las variables y
/// sus grupos). Una partida guarda varios y usa uno (`EstadoPartida.conjunto_activo`); se pueden exportar a un archivo y
/// traer de otra partida, y el programa trae algunos de serie definidos en código (`conjuntos::integrados`). Es también el
/// contenido del archivo exportado.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConjuntoParametros {
    pub nombre: String,
    #[serde(default)]
    pub descripcion: String,
    #[serde(default)]
    pub parametros: Vec<DefinicionParametro>,
    #[serde(default)]
    pub grupos_variables: Vec<GrupoVariables>,
    #[serde(default)]
    pub variables: Vec<DefinicionVariable>,
    /// Las listas de rangos que usan las variables derivadas (ver rangos.rs).
    #[serde(default)]
    pub listas_rangos: Vec<ListaRangos>,
    /// Cómo se nombra el resultado de una comparación del chat (`1d100 <= 45`): sus niveles (extremo, difícil...) y sus críticos y
    /// pifias. Sin ella, solo «Éxito» o «Fallo». Ver escalas.rs.
    #[serde(default)]
    pub escala_resultado: Option<EscalaResultado>,
    /// Los archivos de texto (.txt) con los que nace toda ficha NUEVA, en su orden (ver archivos_defecto.rs).
    #[serde(default)]
    pub archivos_texto: Vec<ArchivoTextoDefecto>,
}

/// Un archivo de texto que trae por defecto cada ficha nueva del conjunto: su nombre (sin «.txt») y su texto inicial.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArchivoTextoDefecto {
    /// Estable mientras exista (lo asigna Rust al guardar un archivo con id 0): la pantalla lo usa para seguir cada fila.
    pub id: u16,
    pub nombre: String,
    #[serde(default)]
    pub contenido: String,
}

/// Cómo se nombra el resultado de una comparación del chat. Es del conjunto de parámetros (cada rol tiene la suya): el chat solo
/// compara, y esto decide si un acierto es «Éxito difícil» o un 1 natural es un «Crítico».
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EscalaResultado {
    /// Cómo se llama acertar sin más y fallar sin más.
    pub nombre_exito: String,
    pub nombre_fallo: String,
    /// Solo por debajo (`<`, `<=`): niveles de acierto según qué parte del objetivo alcanza el total, del más estricto al más laxo.
    #[serde(default)]
    pub tramos: Vec<TramoEscala>,
    /// Críticos y pifias: reglas sobre la cara que sale en un dado suelto. La primera que casa gana.
    #[serde(default)]
    pub naturales: Vec<ReglaNatural>,
}

/// «Éxito difícil» si `total <= objetivo · numerador / denominador`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TramoEscala {
    pub nombre: String,
    pub numerador: i16,
    pub denominador: i16,
}

/// Una regla sobre la cara natural de UN dado normal (sin ventaja ni multiplicador): si sale una cara de `desde` a `hasta`
/// (y, si se indica, en un dado de `dado` caras y con un objetivo menor que `umbral_menor_que`), el resultado se llama `nombre`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ReglaNatural {
    pub nombre: String,
    /// Solo en un dado de estas caras (el 96-100 de Cthulhu es de un d100). `None` = en cualquiera.
    #[serde(default)]
    pub dado: Option<u16>,
    pub desde: u16,
    pub hasta: u16,
    pub efecto: EfectoNatural,
    /// Solo si el objetivo es menor que esto (la pifia de Cthulhu del 96 al 100 solo vale con un objetivo menor de 50).
    #[serde(default)]
    pub umbral_menor_que: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EfectoNatural {
    /// Solo se nombra si la comparación acierta (el crítico de Cthulhu).
    SoloSiAcierta,
    /// Solo se nombra si la comparación falla (la pifia de Cthulhu).
    SoloSiFalla,
    /// Acierta aunque la comparación fallara (el 20 de D&D).
    ForzarAcierto,
    /// Falla aunque la comparación acertara (el 1 de D&D).
    ForzarFallo,
}

/// Un conjunto que la partida tiene guardado. De los de serie guarda su propia copia (se puede editar sin tocar el original).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConjuntoGuardado {
    pub id: u16,
    /// La clave del conjunto de serie del que viene («generico»); `None` si es propio o importado.
    #[serde(default)]
    pub origen: Option<String>,
    /// Del activo, esto puede estar desfasado: sus definiciones de verdad son las «vivas» del estado
    /// (`EstadoPartida::contenido_de`).
    pub contenido: ConjuntoParametros,
    /// Lo que tenía cada ficha cuando se dejó de usar este conjunto (solo el que NO está activo lo lleva al día). Es de la partida
    /// y no del conjunto: no se exporta. Cada conjunto guarda lo suyo y solo lo recupera él: nada pasa de uno a otro aunque
    /// los ids de sus parámetros coincidan.
    #[serde(default)]
    pub valores: ValoresDeFichas,
}

/// Los valores que las fichas tenían bajo un conjunto: de cada ficha, los de sus parámetros y variables y la variación de − y +;
/// de cada instancia, el valor actual de sus parámetros con máximo.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ValoresDeFichas {
    #[serde(default)]
    pub fichas: Vec<ValoresFicha>,
    #[serde(default)]
    pub instancias: Vec<ValoresInstancia>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ValoresFicha {
    pub id_entidad: u16,
    pub parametros: Vec<Parametro>,
    pub variables: Vec<ValorVariable>,
    pub pasos_valor: Vec<PasoValor>,
    /// Los valores actuales de una ficha única (`Entidad.actuales`).
    #[serde(default)]
    pub actuales: Vec<ValorActual>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ValoresInstancia {
    pub id_instancia: u16,
    /// La ficha de esa instancia: si el id de la instancia se reutilizó para otra ficha, no se le devuelve nada.
    pub id_entidad: u16,
    pub valores_actuales: Vec<ValorActual>,
}

/// El grupo con el que nace una partida (y al que van las variables de las partidas anteriores a los grupos).
pub fn grupos_por_defecto() -> Vec<GrupoVariables> {
    vec![GrupoVariables { id: 1, nombre: "Variables".into(), tipo: TipoGrupoVariables::Base }]
}

/// Valor de una variable en una ficha.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ValorVariable {
    pub id_definicion: u16,
    /// El valor escrito (solo en las bases: una derivada se calcula).
    pub valor: i16,
    /// Solo en una derivada: las opcionales que esta ficha tiene marcadas (se suman).
    #[serde(default)]
    pub opcionales: Vec<u16>,
    /// Solo en una derivada con opciones: la que ha elegido esta ficha (su posición en `DefinicionVariable.opciones`).
    #[serde(default)]
    pub eleccion: u16,
    /// Solo en una derivada: los parámetros opcionales (`DefinicionVariable.parametros_opcionales`) que esta ficha tiene marcados.
    #[serde(default)]
    pub parametros_marcados: Vec<u16>,
}

/// Cuánto suman o restan los botones − / + de un valor actual (vida...) en una ficha. Solo se guardan los que no son 1.
/// Lo cambia quien controla la ficha (el host o su dueño) y es de la ENTIDAD: lo comparten todas sus instancias.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PasoValor {
    pub id_definicion: u16,
    pub paso: u16,
}

/// Tope del paso de − / +.
pub const PASO_VALOR_MAX: u16 = 9999;

/// Deja solo los pasos válidos: de un parámetro con máximo que exista, de 2 a `PASO_VALOR_MAX` (1 es el de serie y no se
/// guarda), uno por parámetro y en el orden de las definiciones.
pub fn normalizar_pasos(defs: &[DefinicionParametro], pasos: &mut Vec<PasoValor>) {
    let anteriores = std::mem::take(pasos);
    for d in defs.iter().filter(|d| d.tiene_maximo) {
        if let Some(p) = anteriores.iter().find(|p| p.id_definicion == d.id && (2..=PASO_VALOR_MAX).contains(&p.paso)) {
            pasos.push(p.clone());
        }
    }
}

// ========================================================
// JUGADORES
// ========================================================

/// Identifica a un jugador de la partida. Lo asigna el host al registrarlo (`jugadores::registrar`) y se
/// guarda en la partida, así quien vuelve otro día conserva sus fichas.
pub type IdJugador = u16;

/// El host (el máster). En el fondo es un jugador más, pero tiene acceso a todo: nunca hace falta
/// ponerlo en `Entidad.propietarios`.
pub const ID_HOST: IdJugador = 0;

/// Un jugador registrado en la partida (ver `EstadoPartida.jugadores`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Jugador {
    pub id: IdJugador,
    pub nombre: String,
}

// ========================================================
// ENTIDADES E INSTANCIAS
// ========================================================

pub const ZOOM_MIN: u16 = 100;
pub const ZOOM_MAX: u16 = 400;

/// Cómo se encuadra la foto de una ficha en su miniatura (lo elige el usuario en el editor, que recorta
/// el WebP y lo manda a Rust). `zoom` en % (100 = la foto cubre justo el cuadrado, hasta `ZOOM_MAX`);
/// `x` e `y` (0-100) eligen qué parte de lo que sobra se ve: 0 = arriba/izquierda, 50 = centro. Se guarda
/// para que, al volver a abrir la ficha, la foto aparezca como se dejó y se pueda seguir moviendo.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncuadreFoto {
    pub x: u8,
    pub y: u8,
    pub zoom: u16,
}

impl Default for EncuadreFoto {
    fn default() -> Self {
        EncuadreFoto { x: 50, y: 50, zoom: ZOOM_MIN }
    }
}

impl EncuadreFoto {
    pub fn es_valido(&self) -> bool {
        self.x <= 100 && self.y <= 100 && (ZOOM_MIN..=ZOOM_MAX).contains(&self.zoom)
    }
}

// Representa la definición de una entidad
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(from = "EntidadGuardada")]
pub struct Entidad {
    pub id: u16,
    pub nombre: String,
    pub tipo: TipoEntidad,
    /// Valores de los parámetros de la partida, en el orden de sus definiciones.
    pub parametros: Vec<Parametro>,
    /// Valores de las variables de la partida, en el orden de sus definiciones (ver `variables.rs`). Solo los
    /// ven el host y los propietarios.
    pub variables: Vec<ValorVariable>,
    /// Lo que suman o restan los botones − / + de cada valor de esta ficha (ver `PasoValor`). Lo cambia solo el delta
    /// `EntidadCambiada { PasoValor }` (`poner_paso_valor`): lo que mande el front en `modificar_entidad` se ignora.
    pub pasos_valor: Vec<PasoValor>,
    pub cuerpo: Cuerpo,
    /// Cuánto ve la ficha, en metros. Es un dato de SERIE de la ficha, como el nombre o el cuerpo: no es un parámetro de la partida y
    /// no se puede quitar. Por ahora no hace nada (queda para la niebla de guerra y la visión). 0 = sin definir.
    pub vision: u16,
    /// La identidad del archivo de plantilla con el que se rellenó la ficha (un UUID que genera la propia plantilla la primera
    /// vez que se guarda): sirve para reconocerla si ese mismo archivo vuelve a importarse, desde cualquier navegador. Es
    /// independiente de `id`, que sigue repartiendo el host. `None` = no viene de ninguna plantilla.
    pub plantilla_id: Option<String>,
    pub zona_id: u16,

    // Una criatura o NPC puede no tener foto (en JS llegará como string o null)
    pub foto_url: Option<String>,

    /// Miniatura propia de la ficha (`miniaturas/ficha/entidad_<id>_<versión>.webp`): la que el usuario
    /// encuadró en el editor; el front la recorta y Rust la guarda tal cual. La pone solo el delta
    /// `MiniaturaFichaCambiada`; lo que mande el front en `modificar_entidad` se ignora. Sin ella, todo
    /// usa la miniatura automática de `foto_url`.
    pub miniatura_url: Option<String>,
    /// Encuadre con el que se hizo esa miniatura (ver `EncuadreFoto`).
    pub foto_encuadre: EncuadreFoto,

    /* Concepto incorrecto debido a que prefiero que sea modular lo que es publico y que no
    * Además, una ficha (excepto si es jugador) como concepto es público si tiene una instancia
    *pub es_publico: bool,
    */
    pub archivos_adjuntos: Vec<Option<String>>,

    pub es_unico: bool,

    /// Solo en una ficha ÚNICA: el valor actual de cada parámetro con máximo (la vida actual...). Una única solo tiene una instancia
    /// y su valor actual se muestra y se escribe junto al máximo en la ficha. Se mantiene al día con el de su instancia (si cambia la
    /// vida en el tablero, cambia aquí) y sobrevive a que se borre la instancia: la siguiente nace con estos valores. En las fichas no
    /// únicas está vacío (`instancias::normalizar_actuales`).
    #[serde(default)]
    pub actuales: Vec<ValorActual>,

    /// Jugadores que controlan la ficha (la ven entera, la editan, mueven sus instancias...), sin el host,
    /// que siempre puede todo. Vacío = solo el host. Ver `jugadores.rs` y `visibilidad.rs`.
    pub propietarios: Vec<IdJugador>,
}

/// Forma en que se LEE una `Entidad`: la actual más los campos fijos de versiones anteriores
/// (vida_max, armadura...), que se pasan a `parametros`. Así se cargan las partidas antiguas,
/// tanto en partida.json como en las líneas de deltas.jsonl.
#[derive(Deserialize)]
struct EntidadGuardada {
    id: u16,
    nombre: String,
    tipo: TipoEntidad,
    #[serde(default)]
    parametros: Vec<Parametro>,
    #[serde(default)]
    variables: Vec<ValorVariable>,
    #[serde(default)]
    pasos_valor: Vec<PasoValor>,
    cuerpo: Cuerpo,
    #[serde(default)]
    vision: u16,
    #[serde(default)]
    plantilla_id: Option<String>,
    zona_id: u16,
    #[serde(default)]
    foto_url: Option<String>,
    #[serde(default)]
    miniatura_url: Option<String>,
    #[serde(default)]
    foto_encuadre: EncuadreFoto,
    // Si falta en el JSON, se asume lista vacía []
    #[serde(default)]
    archivos_adjuntos: Vec<Option<String>>,
    #[serde(default)]
    es_unico: bool,
    #[serde(default)]
    actuales: Vec<ValorActual>,
    #[serde(default)]
    propietarios: Vec<IdJugador>,

    // Formato antiguo
    vida_max: Option<i16>,
    vida_es_publica: Option<bool>,
    armadura: Option<i16>,
    armadura_es_publica: Option<bool>,
    velocidad: Option<i16>,
    rango_arma: Option<i16>,
}

impl From<EntidadGuardada> for Entidad {
    fn from(g: EntidadGuardada) -> Self {
        let mut parametros = g.parametros;
        if parametros.is_empty() {
            let antiguos = [
                (ID_VIDA, g.vida_max, g.vida_es_publica),
                (ID_ARMADURA, g.armadura, g.armadura_es_publica),
                (ID_VELOCIDAD, g.velocidad, None),
                (ID_RANGO_ARMA, g.rango_arma, None),
            ];
            for (id_definicion, valor, publico) in antiguos {
                if let Some(v) = valor {
                    parametros.push(Parametro {
                        id_definicion,
                        valor: ValorParametro::Numero(v),
                        publico: publico.unwrap_or(false),
                    });
                }
            }
        }
        Entidad {
            id: g.id,
            nombre: g.nombre,
            tipo: g.tipo,
            parametros,
            variables: g.variables,
            pasos_valor: g.pasos_valor,
            cuerpo: g.cuerpo,
            vision: g.vision,
            plantilla_id: g.plantilla_id,
            zona_id: g.zona_id,
            foto_url: g.foto_url,
            miniatura_url: g.miniatura_url,
            foto_encuadre: g.foto_encuadre,
            archivos_adjuntos: g.archivos_adjuntos,
            es_unico: g.es_unico,
            actuales: g.actuales,
            propietarios: g.propietarios,
        }
    }
}

// Representa una instancia de una entidad en el mapa
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Instancia {
    pub id_instancia: u16,
    pub id_mapa: Option<u16>,
    pub base: Entidad,

    /// Valor actual de los parámetros con máximo (p. ej. la vida); el máximo es el de `base`.
    /// Si la base no lo tiene público, no se comparte.
    #[serde(default)]
    pub valores_actuales: Vec<ValorActual>,
    pub coordenadas: Coordenada,

    #[serde(default)]
    pub activo: bool,

    /// Cuartos de vuelta en el sentido del reloj (0..=3). Con uno impar, ancho y alto del cuerpo se
    /// intercambian (ver `tablero::lados_instancia`); el centro no se mueve.
    #[serde(default)]
    pub rotacion: u8,

    /// La imagen se dibuja en espejo (de izquierda a derecha, en el eje de la propia ficha, antes de girar).
    /// Solo es de aspecto: no cambia los lados ni la posición.
    #[serde(default)]
    pub espejo: bool,

    /// Posición en «Fichas activas» (de arriba abajo). Ver `DeltaPartida::InstanciasReordenadas`.
    #[serde(default)]
    pub orden: u16,

    /// Altura en el tablero: con fichas solapadas, la de capa mayor se dibuja encima. Al poner o mover
    /// una ficha en el tablero recibe la mayor (ver `instancias::capa_superior`).
    #[serde(default)]
    pub capa: u32,

    /// Oculta: solo la ve el host (al 50 % en el tablero y con un ojo cerrado en «Fichas activas»). A los jugadores
    /// no les llega nada de ella, ni a los dueños de su ficha (ver `visibilidad::instancias_para`).
    #[serde(default)]
    pub oculto: bool,

    /// DERIVADO, nunca se guarda: solo lo pone `visibilidad::vista_jugador` cuando la instancia estaría gris (un
    /// parámetro con `atenuar_si_negativo` a 0 o menos) aunque ese parámetro sea privado y no llegue su valor.
    #[serde(default)]
    pub atenuada: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Zona {
    pub id: u16,
    pub nombre: String,

    // Si no viene en el JSON, se asume 0
    #[serde(default)]
    pub orden: u16,
}

// El mapa relacionado con una zona
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Mapa {
    pub id: u16,
    pub id_zona: u16,
    #[serde(default)]
    pub nombre: String,
    // El mapa puede no tener imagen asignada (en JS llegará como string o null)
    #[serde(default)]
    pub url_imagen_fondo: Option<String>,

    pub ancho: u16,
    pub alto: u16,
    pub rejilla: u16,
    pub color_opacidad: Vec<u8>,
    pub grosor: u16,

    // Si no viene en el JSON, se asume 0
    #[serde(default)]
    pub orden: u16,

    /// Lo ven los jugadores de la sala (la casilla de arriba a la derecha del mapa). Lo cambia solo el delta
    /// `MapaCompartido`: `modificar_mapa` conserva el guardado.
    #[serde(default)]
    pub compartido: bool,

    /// La batalla de este mapa, si la hay (el parámetro «en batalla»: `batalla.is_some()`). Solo la cambian los deltas de batalla
    /// (ver `batalla.rs`): `modificar_mapa` conserva la guardada.
    #[serde(default)]
    pub batalla: Option<Batalla>,
}

/// Una batalla en curso en un mapa: uno o varios GRUPOS, cada uno con su turno y su contador de vueltas. Las instancias visibles
/// del mapa que no están en ningún grupo «no participan» (eso no se guarda).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Batalla {
    pub grupos: Vec<GrupoBatalla>,
    /// Id del siguiente grupo que se cree (los ids no se reutilizan dentro de la batalla).
    #[serde(default)]
    pub siguiente_id: u16,
}

/// Un grupo de una batalla. El ORDEN de los turnos no se guarda: es el de `Instancia.orden` («Fichas activas»), así el host puede
/// reordenar a mano y el turno sigue a su instancia por id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GrupoBatalla {
    pub id: u16,
    pub nombre: String,
    /// Ids de las instancias que participan.
    pub miembros: Vec<u16>,
    /// La instancia que tiene el turno (`None` si el grupo no tiene miembros).
    #[serde(default)]
    pub turno: Option<u16>,
    /// Vuelta en la que va (empieza en 1; sube al volver al primero de la lista).
    #[serde(default = "una_ronda")]
    pub ronda: u32,
}

fn una_ronda() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tiradas {
    pub total: i32,
    pub comando: String,
    pub hora: String,
    /// Quién tiró (`ID_HOST` para el host; las tiradas guardadas antes del campo también son suyas).
    #[serde(default)]
    pub jugador: IdJugador,
    /// El nombre del jugador al tirar, para escribirlo en el chat («Ana: 1d20 = 7»). Vacío si tiró el host.
    #[serde(default)]
    pub autor: String,
    /// Si los jugadores la ven. Solo el host puede tirar en privado; las de los jugadores son siempre públicas.
    #[serde(default = "tirada_publica")]
    pub publica: bool,
    /// Solo en una comparación (`1d100 <= acro`): el resultado frente al objetivo, con el nombre que le da la escala del conjunto
    /// («Éxito difícil», «Fallo», «Crítico»...); nunca texto escrito en el chat. Vacío en una tirada normal.
    #[serde(default)]
    pub veredicto: String,
    /// Solo en una comparación: si acertó (para pintarla sin depender del texto, que define el host).
    #[serde(default)]
    pub acierta: Option<bool>,
    /// Solo en una comparación: el resultado salió de una regla del dado natural (crítico, pifia...), no del nivel normal.
    #[serde(default)]
    pub especial: bool,
    /// Una línea del chat que solo informa (empezar o terminar una batalla): sin total ni arrastre.
    #[serde(default)]
    pub informativa: bool,
}

fn tirada_publica() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArchivoAdjunto {
    pub ruta: String,
    pub entidad_archivo: Entidad,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeros(ps: &[Parametro]) -> Vec<(u16, ValorParametro, bool)> {
        ps.iter().map(|p| (p.id_definicion, p.valor.clone(), p.publico)).collect()
    }

    #[test]
    fn una_ficha_antigua_pasa_sus_campos_a_parametros() {
        let json = r#"{"id":3,"nombre":"Orco","tipo":"Enemigo","vida_max":18,"vida_es_publica":true,
            "armadura":4,"armadura_es_publica":false,"velocidad":9,"rango_arma":2,"cuerpo":"Pequenio",
            "zona_id":1,"foto_url":null,"archivos_adjuntos":[],"es_unico":false}"#;
        let e: Entidad = serde_json::from_str(json).unwrap();
        use ValorParametro::Numero;
        assert_eq!(
            numeros(&e.parametros),
            vec![
                (ID_VIDA, Numero(18), true),
                (ID_ARMADURA, Numero(4), false),
                (ID_VELOCIDAD, Numero(9), false),
                (ID_RANGO_ARMA, Numero(2), false),
            ]
        );
        // Al volver a guardarla ya no lleva los campos antiguos
        let guardada = serde_json::to_string(&e).unwrap();
        assert!(!guardada.contains("vida_max"), "{guardada}");
        let releida: Entidad = serde_json::from_str(&guardada).unwrap();
        assert_eq!(releida.parametros, e.parametros);
    }

    #[test]
    fn el_encuadre_de_la_foto_se_guarda_y_las_fichas_antiguas_salen_centradas() {
        // Sin el campo (partida anterior): centrada y sin zoom
        let antigua = r#"{"id":3,"nombre":"Orco","tipo":"Enemigo","parametros":[],"cuerpo":"Pequenio",
            "zona_id":1,"foto_url":null,"archivos_adjuntos":[],"es_unico":false}"#;
        let e: Entidad = serde_json::from_str(antigua).unwrap();
        assert_eq!(e.foto_encuadre, EncuadreFoto { x: 50, y: 50, zoom: 100 });
        assert_eq!(e.miniatura_url, None);
        assert_eq!(e.vision, 0, "una ficha de antes de la visión nace con 0 (sin definir)");

        // Con encuadre: se conserva al guardar y releer
        let mut e = e;
        e.foto_encuadre = EncuadreFoto { x: 10, y: 80, zoom: 250 };
        let releida: Entidad = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(releida.foto_encuadre, EncuadreFoto { x: 10, y: 80, zoom: 250 });

        assert!(EncuadreFoto::default().es_valido());
        assert!(!EncuadreFoto { x: 101, y: 0, zoom: 100 }.es_valido());
        assert!(!EncuadreFoto { x: 0, y: 0, zoom: 99 }.es_valido());
        assert!(!EncuadreFoto { x: 0, y: 0, zoom: 401 }.es_valido());
    }

    #[test]
    fn una_ficha_sin_propietarios_es_solo_del_host_y_los_conserva_al_guardar() {
        let antigua = r#"{"id":3,"nombre":"Orco","tipo":"Enemigo","parametros":[],"cuerpo":"Pequenio",
            "zona_id":1,"foto_url":null,"archivos_adjuntos":[],"es_unico":false}"#;
        let mut e: Entidad = serde_json::from_str(antigua).unwrap();
        assert!(e.propietarios.is_empty());

        e.propietarios = vec![2, 5];
        let releida: Entidad = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(releida.propietarios, vec![2, 5]);

        // Las tiradas guardadas antes de saber quién tiraba son del host
        let t: Tiradas = serde_json::from_str(r#"{"total":3,"comando":"1d6 (3)","hora":"10:00"}"#).unwrap();
        assert_eq!(t.jugador, ID_HOST);
    }

    #[test]
    fn normalizar_rellena_ordena_quita_y_ajusta_la_visibilidad_fija() {
        let defs = parametros_por_defecto();
        let mut ps = vec![
            Parametro { id_definicion: ID_RANGO_ARMA, valor: ValorParametro::Numero(5), publico: false },
            Parametro { id_definicion: 99, valor: ValorParametro::Numero(1), publico: true },
            Parametro { id_definicion: ID_VIDA, valor: ValorParametro::Numero(30), publico: true },
            Parametro { id_definicion: ID_VIDA, valor: ValorParametro::Numero(1), publico: false },
        ];
        normalizar_parametros(&defs, &mut ps);
        use ValorParametro::Numero;
        assert_eq!(
            numeros(&ps),
            vec![
                (ID_VIDA, Numero(30), true),
                (ID_ARMADURA, Numero(10), false),
                (ID_VELOCIDAD, Numero(6), true),
                (ID_RANGO_ARMA, Numero(5), true),
            ]
        );
    }

    #[test]
    fn solo_la_vida_de_serie_atenua_y_las_partidas_antiguas_la_conservan() {
        let defs = parametros_por_defecto();
        let atenuan: Vec<_> = defs.iter().filter(|d| d.atenuar_si_negativo).map(|d| d.id).collect();
        assert_eq!(atenuan, vec![ID_VIDA]);

        // Una definición guardada antes de existir el campo: la Vida atenúa, lo demás no
        let vieja = |id: u16, max: bool| -> DefinicionParametro {
            serde_json::from_str(&format!(
                r#"{{"id":{id},"clave":"X","valor_por_defecto":{{"Numero":5}},"tiene_maximo":{max},"visibilidad":"Variable"}}"#
            ))
            .unwrap()
        };
        assert!(vieja(ID_VIDA, true).atenuar_si_negativo);
        assert!(!vieja(ID_VIDA, false).atenuar_si_negativo);
        assert!(!vieja(7, true).atenuar_si_negativo);

        // Si está guardado, manda lo guardado (aunque sea la Vida desmarcada)
        let json = serde_json::to_string(&DefinicionParametro { atenuar_si_negativo: false, ..vieja(ID_VIDA, true) }).unwrap();
        assert!(!serde_json::from_str::<DefinicionParametro>(&json).unwrap().atenuar_si_negativo);
    }

    #[test]
    fn las_coordenadas_antiguas_pasan_a_casilla() {
        let leer = |j: &str| serde_json::from_str::<Coordenada>(j).unwrap();
        assert_eq!(leer(r#"{"Localizacion":[3,4]}"#), Coordenada::Casilla(Casilla { x: 3, y: 4 }));
        assert_eq!(leer(r#"{"Localizacion":[-1,2]}"#), Coordenada::Casilla(Casilla { x: 0, y: 2 }));
        assert_eq!(leer(r#"{"Localizacion":[7]}"#), Coordenada::NoPuesto);
        assert_eq!(leer(r#""NoPuesto""#), Coordenada::NoPuesto);

        // El formato nuevo va y vuelve igual
        let nueva = Coordenada::Casilla(Casilla { x: 5, y: 1 });
        let json = serde_json::to_string(&nueva).unwrap();
        assert_eq!(json, r#"{"Casilla":{"x":5,"y":1}}"#);
        assert_eq!(leer(&json), nueva);
    }
}
