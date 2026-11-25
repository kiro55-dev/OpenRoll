// src-tauri/src/tablero_general/conjuntos.rs
//
// Conjuntos de parámetros: lo que define cómo es el juego al que se juega (parámetros visuales, variables y sus grupos). Una
// partida guarda varios (`EstadoPartida.conjuntos_parametros`) y usa uno (`conjunto_activo`); las definiciones «vivas» del
// estado (`definiciones_parametros`, `grupos_variables`, `definiciones_variables`) son el contenido del ACTIVO, así que todo lo
// que ya funciona (editar parámetros, variables, red) no cambia.
//
//  * De serie (`integrados`): definidos en código, independientes de la partida. Elegir uno COPIA su contenido a la partida, que
//    lo edita sin tocar el original; el archivo de la partida nunca depende de este código al reproducir el log.
//  * Importar: la plantilla de ficha (.html) que exporta OpenRoll lleva dentro el conjunto completo (`definicion_conjunto`), aparte de
//    los datos de la ficha: se importa de CUALQUIER plantilla, rellena o no, y solo se lee ese bloque (nunca el personaje ni sus .txt).
//    Exportar es exportar esa plantilla en blanco (plantilla.rs / el front). El archivo JSON (`Archivo`) queda solo como formato interno
//    de la biblioteca. Lo que se importa es entrada no confiable: se valida entero
//    (`validar`) con las mismas reglas que al crear a mano cada parámetro, variable y grupo.
//  * Cambiar de conjunto reinicia los valores de las fichas (`EstadoPartida::reiniciar_valores_de_fichas`): el aviso es de la UI.
//
// Los comandos (solo del host, nada en `PETICIONES`) están al final, cada uno con su `xxx_en` para probarlo sin Tauri.
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::tablero_general::estado::{DeltaPartida, EstadoPartida};
use crate::tablero_general::jugadores::caracter_enganoso;
use crate::tablero_general::modelos::*;
use crate::tablero_general::partida::{registrar, AppState};
use crate::tablero_general::variables::{self, MAX_GRUPOS_VARIABLES, MAX_VARIABLES};

/// Lo que va en el archivo para reconocerlo: no es un conjunto cualquiera cuyo JSON se parezca.
pub const FORMATO: &str = "openroll-conjunto";
/// Versión del formato del archivo. Un archivo de una versión MÁS NUEVA se rechaza con un motivo claro.
pub const VERSION_ARCHIVO: u32 = 1;
/// Tope del tamaño del archivo a importar (se mira antes de leerlo).
pub const MAX_BYTES_ARCHIVO: u64 = 8 * 1024 * 1024;
pub const MAX_NOMBRE_CONJUNTO: usize = 60;
pub const MAX_DESCRIPCION_CONJUNTO: usize = 300;
/// Conjuntos que puede guardar una partida.
pub const MAX_CONJUNTOS: usize = 30;

// ------------------------------------------------------------------
// De serie
// ------------------------------------------------------------------

/// Un conjunto de serie: su clave estable (la que recuerda `ConjuntoGuardado.origen`) y su contenido.
pub struct Integrado {
    pub clave: &'static str,
    pub contenido: ConjuntoParametros,
}

/// El conjunto «Genérico»: lo que nace en una partida nueva (vida, armadura, velocidad y rango de arma, sin variables).
fn generico() -> ConjuntoParametros {
    ConjuntoParametros {
        nombre: "Genérico".into(),
        descripcion: "Vida, armadura, velocidad y rango de arma. Sin variables.".into(),
        parametros: parametros_por_defecto(),
        grupos_variables: grupos_por_defecto(),
        variables: Vec::new(),
        listas_rangos: Vec::new(),
        escala_resultado: None,
        archivos_texto: Vec::new(),
        macros: Vec::new(),
    }
}

/// Los conjuntos que trae el programa, definidos en código (ver conjuntos_serie.rs). Para añadir uno: otra entrada aquí (y pasa solo por
/// el test que comprueba que toda entrada es válida).
pub fn integrados() -> Vec<Integrado> {
    use crate::tablero_general::conjuntos_serie::{cthulhu7e, dnd5e, pathfinder2e};
    vec![
        Integrado { clave: "dnd5e", contenido: dnd5e() },
        Integrado { clave: "pathfinder2e", contenido: pathfinder2e() },
        Integrado { clave: "cthulhu7e", contenido: cthulhu7e() },
        Integrado { clave: "generico", contenido: generico() },
    ]
}

/// El conjunto con el que nace una partida NUEVA: D&D 5e.
pub const CONJUNTO_POR_DEFECTO: &str = "dnd5e";

fn integrado(clave: &str) -> Option<Integrado> {
    integrados().into_iter().find(|i| i.clave == clave)
}

// ------------------------------------------------------------------
// Estado
// ------------------------------------------------------------------

impl EstadoPartida {
    /// Las definiciones «vivas» como un conjunto (con el nombre y la descripción que tiene guardados el activo).
    fn contenido_vivo(&self) -> ConjuntoParametros {
        let guardado = self.conjuntos_parametros.iter().find(|c| c.id == self.conjunto_activo).map(|c| &c.contenido);
        ConjuntoParametros {
            nombre: guardado.map_or_else(|| "Personalizado".into(), |c| c.nombre.clone()),
            descripcion: guardado.map(|c| c.descripcion.clone()).unwrap_or_default(),
            parametros: self.definiciones_parametros.clone(),
            grupos_variables: self.grupos_variables.clone(),
            variables: self.definiciones_variables.clone(),
            listas_rangos: self.listas_rangos.clone(),
            escala_resultado: self.escala_resultado.clone(),
            archivos_texto: self.archivos_texto_defecto.clone(),
            // Las macros del conjunto viven en `EstadoPartida.macros` (ver `obtener_conjunto_activo`)
            macros: Vec::new(),
        }
    }

    /// El contenido del conjunto `id`: del activo, lo que hay AHORA (lo editado incluido); de los demás, lo guardado.
    pub fn contenido_de(&self, id: u16) -> Option<ConjuntoParametros> {
        if id == self.conjunto_activo && self.conjuntos_parametros.iter().any(|c| c.id == id) {
            return Some(self.contenido_vivo());
        }
        self.conjuntos_parametros.iter().find(|c| c.id == id).map(|c| c.contenido.clone())
    }

    /// Cambia al conjunto `id`. El que se deja guarda lo editado Y lo que tenía cada ficha; se cargan las definiciones del elegido y
    /// las fichas reciben lo que ESE conjunto tenía guardado (o los valores por defecto si es la primera vez). Nada pasa de un
    /// conjunto a otro: se vacía todo antes de restaurar, aunque los ids de sus parámetros coincidan.
    pub(crate) fn activar_conjunto(&mut self, id: u16) {
        if id == self.conjunto_activo || !self.conjuntos_parametros.iter().any(|c| c.id == id) {
            return;
        }
        let vivo = self.contenido_vivo();
        let valores = self.recoger_valores();
        let antes = self.conjunto_activo;
        if let Some(c) = self.conjuntos_parametros.iter_mut().find(|c| c.id == antes) {
            c.contenido = vivo;
            c.valores = valores;
        }
        let nuevo = self.conjuntos_parametros.iter().find(|c| c.id == id).cloned().expect("existe");
        self.definiciones_parametros = nuevo.contenido.parametros;
        self.grupos_variables = nuevo.contenido.grupos_variables;
        self.definiciones_variables = nuevo.contenido.variables;
        self.listas_rangos = nuevo.contenido.listas_rangos;
        self.escala_resultado = nuevo.contenido.escala_resultado;
        self.archivos_texto_defecto = nuevo.contenido.archivos_texto;
        self.conjunto_activo = id;
        self.reiniciar_valores_de_fichas();
        self.restaurar_valores(&nuevo.valores);
        self.ajustar_a_definiciones();
    }

    /// Las fichas de la partida (banco y papelera) y sus instancias (activas y de la papelera), para guardar y restaurar valores.
    fn fichas(&self) -> Vec<&Entidad> {
        use crate::tablero_general::papelera::Borrado;
        let mut v: Vec<&Entidad> = self.banco_entidades.iter().collect();
        for g in &self.papelera {
            for b in &g.borrados {
                match b {
                    Borrado::Entidad { entidad, .. } => v.push(entidad),
                    Borrado::Zona { entidades, .. } => v.extend(entidades.iter()),
                    Borrado::Mapa { .. } => {}
                }
            }
        }
        v
    }

    fn instancias(&self) -> Vec<&Instancia> {
        use crate::tablero_general::papelera::Borrado;
        let mut v: Vec<&Instancia> = self.instancias_activas.iter().collect();
        for g in &self.papelera {
            for b in &g.borrados {
                match b {
                    Borrado::Entidad { instancias, .. } => v.extend(instancias.iter()),
                    Borrado::Mapa { instancias, retiradas, .. } | Borrado::Zona { instancias, retiradas, .. } => {
                        v.extend(instancias.iter().chain(retiradas.iter()))
                    }
                }
            }
        }
        v
    }

    /// Lo que tienen ahora las fichas y sus instancias: lo que se guarda en el conjunto que se deja.
    fn recoger_valores(&self) -> ValoresDeFichas {
        let mut v = ValoresDeFichas::default();
        for e in self.fichas() {
            if !v.fichas.iter().any(|f| f.id_entidad == e.id) {
                v.fichas.push(ValoresFicha {
                    id_entidad: e.id,
                    parametros: e.parametros.clone(),
                    variables: e.variables.clone(),
                    pasos_valor: e.pasos_valor.clone(),
                    actuales: e.actuales.clone(),
                });
            }
        }
        for i in self.instancias() {
            if !v.instancias.iter().any(|x| x.id_instancia == i.id_instancia) {
                v.instancias.push(ValoresInstancia {
                    id_instancia: i.id_instancia,
                    id_entidad: i.base.id,
                    valores_actuales: i.valores_actuales.clone(),
                });
            }
        }
        v
    }

    /// Devuelve a las fichas lo que tenían guardado (ya vacías): lo que no esté guardado se queda vacío y
    /// `ajustar_a_definiciones` lo rellena con los valores por defecto del conjunto. Las instancias solo recuperan su valor actual si
    /// siguen siendo de la misma ficha (el id de una instancia borrada puede reutilizarse para otra).
    fn restaurar_valores(&mut self, v: &ValoresDeFichas) {
        use crate::tablero_general::papelera::Borrado;
        let poner_ficha = |e: &mut Entidad| {
            if let Some(g) = v.fichas.iter().find(|g| g.id_entidad == e.id) {
                e.parametros = g.parametros.clone();
                e.variables = g.variables.clone();
                e.pasos_valor = g.pasos_valor.clone();
                e.actuales = g.actuales.clone();
            }
        };
        let poner_instancia = |i: &mut Instancia| {
            poner_ficha(&mut i.base);
            if let Some(g) = v.instancias.iter().find(|g| g.id_instancia == i.id_instancia && g.id_entidad == i.base.id) {
                i.valores_actuales = g.valores_actuales.clone();
            }
        };
        self.banco_entidades.iter_mut().for_each(poner_ficha);
        self.instancias_activas.iter_mut().for_each(poner_instancia);
        for grupo in self.papelera.iter_mut() {
            for borrado in grupo.borrados.iter_mut() {
                match borrado {
                    Borrado::Entidad { entidad, instancias } => {
                        poner_ficha(entidad);
                        instancias.iter_mut().for_each(poner_instancia);
                    }
                    Borrado::Mapa { instancias, retiradas, .. } => {
                        instancias.iter_mut().chain(retiradas.iter_mut()).for_each(poner_instancia);
                    }
                    Borrado::Zona { entidades, instancias, retiradas, .. } => {
                        entidades.iter_mut().for_each(poner_ficha);
                        instancias.iter_mut().chain(retiradas.iter_mut()).for_each(poner_instancia);
                    }
                }
            }
        }
    }

    /// Quita lo guardado de fichas o instancias que ya no existen (ni en la partida ni en la papelera): sus ids pueden reutilizarse
    /// para otras nuevas, y esas no deben heredar nada. Se llama al borrar una instancia y al vaciar la papelera.
    pub(crate) fn podar_valores_guardados(&mut self) {
        let fichas: Vec<u16> = self.fichas().iter().map(|e| e.id).collect();
        let instancias: Vec<(u16, u16)> = self.instancias().iter().map(|i| (i.id_instancia, i.base.id)).collect();
        for c in self.conjuntos_parametros.iter_mut() {
            c.valores.fichas.retain(|g| fichas.contains(&g.id_entidad));
            c.valores.instancias.retain(|g| instancias.contains(&(g.id_instancia, g.id_entidad)));
        }
    }

    /// Vacía todo lo que las fichas guardan de las definiciones (valores de parámetros y variables, variación de − y +, valor
    /// actual de los parámetros con máximo de las instancias): el banco, las instancias y lo que hay en la papelera.
    /// `ajustar_a_definiciones` lo rellena con los valores por defecto del conjunto en uso.
    pub(crate) fn reiniciar_valores_de_fichas(&mut self) {
        let vaciar = |e: &mut Entidad| {
            e.parametros.clear();
            e.variables.clear();
            e.pasos_valor.clear();
            e.actuales.clear();
        };
        let vaciar_instancia = |i: &mut Instancia| {
            vaciar(&mut i.base);
            i.valores_actuales.clear();
            i.atenuada = false;
        };
        self.banco_entidades.iter_mut().for_each(vaciar);
        self.instancias_activas.iter_mut().for_each(vaciar_instancia);
        use crate::tablero_general::papelera::Borrado;
        for grupo in self.papelera.iter_mut() {
            for borrado in grupo.borrados.iter_mut() {
                match borrado {
                    Borrado::Entidad { entidad, instancias } => {
                        vaciar(entidad);
                        instancias.iter_mut().for_each(vaciar_instancia);
                    }
                    Borrado::Mapa { instancias, retiradas, .. } => {
                        instancias.iter_mut().chain(retiradas.iter_mut()).for_each(vaciar_instancia);
                    }
                    Borrado::Zona { entidades, instancias, retiradas, .. } => {
                        entidades.iter_mut().for_each(vaciar);
                        instancias.iter_mut().chain(retiradas.iter_mut()).for_each(vaciar_instancia);
                    }
                }
            }
        }
    }
}

/// Una partida sin conjuntos (nueva, o de antes de ellos) recibe uno con sus definiciones actuales, que pasa a ser el activo:
/// «Genérico» si son las de serie y «Personalizado» si no. No se pierde nada, y es determinista.
pub fn asegurar(est: &mut EstadoPartida) {
    if !est.conjuntos_parametros.is_empty() {
        // Un activo que ya no existe (no debería pasar) se corrige al primero
        if !est.conjuntos_parametros.iter().any(|c| c.id == est.conjunto_activo) {
            est.conjunto_activo = est.conjuntos_parametros[0].id;
        }
        return;
    }
    let vivo = ConjuntoParametros {
        nombre: String::new(),
        descripcion: String::new(),
        parametros: est.definiciones_parametros.clone(),
        grupos_variables: est.grupos_variables.clone(),
        variables: est.definiciones_variables.clone(),
        listas_rangos: est.listas_rangos.clone(),
        escala_resultado: est.escala_resultado.clone(),
        archivos_texto: est.archivos_texto_defecto.clone(),
        macros: Vec::new(),
    };
    let g = generico();
    let es_generico = vivo.parametros == g.parametros && vivo.variables.is_empty() && vivo.grupos_variables == g.grupos_variables;
    let contenido = if es_generico {
        g
    } else {
        ConjuntoParametros { nombre: "Personalizado".into(), descripcion: "Los parámetros y variables que tenía esta partida.".into(), ..vivo }
    };
    est.conjuntos_parametros = vec![ConjuntoGuardado { id: 1, origen: es_generico.then(|| "generico".to_string()), contenido, valores: Default::default() }];
    est.conjunto_activo = 1;
}

impl EstadoPartida {
    /// El estado de una partida NUEVA: una zona con un mapa vacío y D&D 5e como conjunto activo (sus parámetros, variables, grupos y escala son las definiciones
    /// vivas). `default()` sigue siendo el genérico: es la base de las partidas anteriores a los conjuntos y de casi todas las pruebas.
    /// Una partida que ya existe no pasa por aquí: conserva lo que tiene (`asegurar`).
    pub fn para_partida_nueva() -> Self {
        let mut dnd = integrado(CONJUNTO_POR_DEFECTO).expect("el conjunto por defecto existe").contenido;
        let mut e = Self::default();
        // Sus macros nacen como macros del host del conjunto (como al añadirlo a una partida)
        e.macros = crate::tablero_general::macros::macros_nuevas(&e, 1, std::mem::take(&mut dnd.macros));
        e.definiciones_parametros = dnd.parametros.clone();
        e.grupos_variables = dnd.grupos_variables.clone();
        e.definiciones_variables = dnd.variables.clone();
        e.listas_rangos = dnd.listas_rangos.clone();
        e.escala_resultado = dnd.escala_resultado.clone();
        e.archivos_texto_defecto = dnd.archivos_texto.clone();
        e.conjuntos_parametros =
            vec![ConjuntoGuardado { id: 1, origen: Some(CONJUNTO_POR_DEFECTO.into()), contenido: dnd, valores: Default::default() }];
        e.conjunto_activo = 1;
        // Una partida nueva no nace vacía: una zona con un mapa sin imagen (como si el usuario los hubiera creado)
        e.zonas = vec![Zona { id: 1, nombre: "Zona 1".into(), orden: 0 }];
        e.mapas = vec![Mapa {
            id: 1,
            id_zona: 1,
            nombre: "1".into(),
            url_imagen_fondo: None,
            ancho: 1000,
            alto: 1000,
            rejilla: 50,
            color_opacidad: vec![255, 255, 255, 76],
            grosor: 1,
            orden: 0,
            compartido: false,
            batalla: None,
        }];
        e.ajustar_a_definiciones();
        e
    }
}

/// El conjunto genérico con el que nace un `EstadoPartida::default()` (no una partida nueva de verdad: ver `para_partida_nueva`).
pub fn conjuntos_iniciales() -> Vec<ConjuntoGuardado> {
    vec![ConjuntoGuardado { id: 1, origen: Some("generico".into()), contenido: generico(), valores: Default::default() }]
}

// ------------------------------------------------------------------
// Archivo y validación
// ------------------------------------------------------------------

/// Cómo se escribe un conjunto en un archivo.
#[derive(Serialize, Deserialize)]
struct Archivo {
    formato: String,
    version: u32,
    #[serde(flatten)]
    conjunto: ConjuntoParametros,
}

/// El texto JSON de un conjunto, listo para escribir en un archivo.
pub fn a_archivo(c: &ConjuntoParametros) -> Result<String, String> {
    let a = Archivo { formato: FORMATO.into(), version: VERSION_ARCHIVO, conjunto: c.clone() };
    serde_json::to_string_pretty(&a).map_err(|e| format!("No se pudo preparar el archivo: {e}"))
}

/// Lee y valida el texto de un archivo de conjunto. Lo que sale ya es válido y está limpio (espacios, nombres).
pub fn de_archivo(texto: &str) -> Result<ConjuntoParametros, String> {
    // Primero lo justo para reconocerlo: así un archivo de otra cosa o de una versión nueva da su motivo y no un error de campos
    #[derive(Deserialize)]
    struct Cabecera {
        formato: Option<String>,
        version: Option<u32>,
    }
    let cab: Cabecera = serde_json::from_str(texto).map_err(|_| "El archivo no es un conjunto de parámetros de OpenRoll".to_string())?;
    if cab.formato.as_deref() != Some(FORMATO) {
        return Err("El archivo no es un conjunto de parámetros de OpenRoll".into());
    }
    match cab.version {
        Some(v) if v > VERSION_ARCHIVO => {
            return Err("Este archivo es de una versión más nueva de OpenRoll: actualiza la aplicación para poder importarlo".into())
        }
        Some(v) if v >= 1 => {}
        _ => return Err("El archivo no tiene una versión válida".into()),
    }
    let a: Archivo = serde_json::from_str(texto).map_err(|e| format!("El conjunto del archivo está mal formado: {e}"))?;
    validar(a.conjunto)
}

/// Comprueba un conjunto entero con las reglas de siempre y lo devuelve limpio. Cualquier fallo lo rechaza todo.
pub fn validar(mut c: ConjuntoParametros) -> Result<ConjuntoParametros, String> {
    c.nombre = c.nombre.split_whitespace().collect::<Vec<_>>().join(" ");
    c.descripcion = c.descripcion.trim().to_string();
    if c.nombre.is_empty() {
        return Err("El conjunto necesita un nombre".into());
    }
    if c.nombre.chars().count() > MAX_NOMBRE_CONJUNTO || c.nombre.chars().any(caracter_enganoso) {
        return Err(format!("El nombre del conjunto debe tener entre 1 y {MAX_NOMBRE_CONJUNTO} caracteres, sin caracteres especiales"));
    }
    if c.descripcion.chars().count() > MAX_DESCRIPCION_CONJUNTO || c.descripcion.chars().any(|ch| ch != '\n' && caracter_enganoso(ch)) {
        return Err(format!("La descripción no puede pasar de {MAX_DESCRIPCION_CONJUNTO} caracteres"));
    }

    // Parámetros: ids únicos y cada uno con las reglas de crearlo a mano, contra los anteriores
    let mut parametros: Vec<DefinicionParametro> = Vec::new();
    for p in c.parametros {
        if parametros.iter().any(|o| o.id == p.id) {
            return Err(format!("El parámetro «{}» repite el id {}", p.clave, p.id));
        }
        let p = DefinicionParametro {
            clave: p.clave.split_whitespace().collect::<Vec<_>>().join(" "),
            unidad: p.unidad.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()),
            ..p
        };
        crate::tablero_general::partida::validar_definicion(&parametros, &p)?;
        parametros.push(p);
    }

    // Listas de rangos: una variable solo puede apuntar a una que exista; los rangos propios que traiga una variable (un archivo de antes de
    // las listas) pasan a una lista; cada lista se valida y las variables reciben su copia (con los nombres ya limpios)
    let mut listas_entrada = c.listas_rangos;
    let mut variables_entrada = c.variables;
    if let Some(v) = variables_entrada.iter().find(|v| v.rangos.is_some_and(|r| !listas_entrada.iter().any(|l| l.id == r))) {
        return Err(format!("La variable «{}» usa una lista de rangos que el archivo no trae", v.nombre));
    }
    crate::tablero_general::rangos::normalizar_rangos(&mut listas_entrada, &mut variables_entrada);
    let mut listas: Vec<ListaRangos> = Vec::new();
    for l in listas_entrada {
        if listas.iter().any(|o| o.id == l.id) {
            return Err(format!("La lista de rangos «{}» repite el id {}", l.nombre, l.id));
        }
        let l = crate::tablero_general::rangos::validar_lista(&listas, l)?;
        listas.push(l);
    }
    crate::tablero_general::rangos::normalizar_rangos(&mut listas, &mut variables_entrada);

    // Grupos de variables: ids únicos y nombres válidos y distintos
    let mut grupos: Vec<GrupoVariables> = Vec::new();
    for g in c.grupos_variables {
        if grupos.iter().any(|o| o.id == g.id) {
            return Err(format!("El grupo «{}» repite el id {}", g.nombre, g.id));
        }
        variables::validar_grupo(&grupos, &g.nombre)?;
        grupos.push(GrupoVariables { nombre: g.nombre.trim().to_string(), ..g });
    }
    if grupos.len() > MAX_GRUPOS_VARIABLES {
        return Err(format!("No puede haber más de {MAX_GRUPOS_VARIABLES} grupos de variables"));
    }

    // Variables: ids únicos; las bases se validan antes que las derivadas (que dependen de ellas), da igual el orden del archivo
    if variables_entrada.len() > MAX_VARIABLES {
        return Err(format!("No puede haber más de {MAX_VARIABLES} variables"));
    }
    let mut aceptadas: Vec<DefinicionVariable> = Vec::new();
    for derivadas in [false, true] {
        for v in variables_entrada.iter().filter(|v| variables::es_derivada(v) == derivadas) {
            if variables_entrada.iter().filter(|o| o.id == v.id).count() > 1 {
                return Err(format!("La variable «{}» repite el id {}", v.nombre, v.id));
            }
            let v = DefinicionVariable {
                nombre: v.nombre.split_whitespace().collect::<Vec<_>>().join(" "),
                abreviatura: v.abreviatura.trim().to_string(),
                ..v.clone()
            };
            variables::validar_definicion(&aceptadas, &grupos, &parametros, &v)?;
            aceptadas.push(v);
        }
    }
    // Se conserva el orden del archivo (es el que siguen las fichas)
    let variables_validadas: Vec<DefinicionVariable> =
        variables_entrada.iter().filter_map(|v| aceptadas.iter().find(|a| a.id == v.id).cloned()).collect();
    // Los parámetros calculados solo pueden usar variables del propio conjunto
    for p in &parametros {
        variables::validar_parametro_calculado(p, &variables_validadas).map_err(|e| format!("{}: {e}", p.clave))?;
    }

    // La escala de resultados, con las mismas reglas que al editarla a mano
    let escala_resultado = c.escala_resultado.map(crate::tablero_general::escalas::validar).transpose()?;

    // Los archivos de texto por defecto, con las mismas reglas que al editarlos a mano
    let archivos_texto = crate::tablero_general::archivos_defecto::validar(c.archivos_texto)?;

    // Las macros, con las mismas reglas que al escribirlas a mano (nombre y código), sin nombres repetidos
    let macros = crate::tablero_general::macros::validar_de_conjunto(c.macros)?;

    Ok(ConjuntoParametros {
        nombre: c.nombre,
        descripcion: c.descripcion,
        parametros,
        grupos_variables: grupos,
        variables: variables_validadas,
        listas_rangos: listas,
        escala_resultado,
        archivos_texto,
        macros,
    })
}

// ------------------------------------------------------------------
// Resumen para la pantalla
// ------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct ResumenConjunto {
    pub id: u16,
    pub nombre: String,
    pub descripcion: String,
    pub origen: Option<String>,
    pub parametros: usize,
    pub variables: usize,
    pub grupos: usize,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ResumenDeSerie {
    pub clave: String,
    pub nombre: String,
    pub descripcion: String,
    /// La partida ya tiene una copia de este.
    pub ya_anadido: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ListaConjuntos {
    pub activo: u16,
    pub guardados: Vec<ResumenConjunto>,
    pub de_serie: Vec<ResumenDeSerie>,
}

pub fn resumen(est: &EstadoPartida) -> ListaConjuntos {
    let guardados = est
        .conjuntos_parametros
        .iter()
        .filter_map(|g| {
            let c = est.contenido_de(g.id)?;
            Some(ResumenConjunto {
                id: g.id,
                nombre: c.nombre,
                descripcion: c.descripcion,
                origen: g.origen.clone(),
                parametros: c.parametros.len(),
                variables: c.variables.len(),
                grupos: c.grupos_variables.len(),
            })
        })
        .collect();
    let de_serie = integrados()
        .into_iter()
        .map(|i| ResumenDeSerie {
            ya_anadido: est.conjuntos_parametros.iter().any(|g| g.origen.as_deref() == Some(i.clave)),
            clave: i.clave.to_string(),
            nombre: i.contenido.nombre,
            descripcion: i.contenido.descripcion,
        })
        .collect();
    ListaConjuntos { activo: est.conjunto_activo, guardados, de_serie }
}

// ------------------------------------------------------------------
// Comandos (solo el host)
// ------------------------------------------------------------------

#[tauri::command]
pub fn obtener_conjuntos_parametros(state: State<'_, AppState>) -> ListaConjuntos {
    resumen(&state.lock())
}

/// Un nombre que no choque con los demás conjuntos: «Nombre», «Nombre (2)», «Nombre (3)»...
fn nombre_libre(est: &EstadoPartida, nombre: &str) -> String {
    let usados: Vec<String> = est.conjuntos_parametros.iter().filter_map(|g| est.contenido_de(g.id)).map(|c| c.nombre.to_lowercase()).collect();
    if !usados.contains(&nombre.to_lowercase()) {
        return nombre.to_string();
    }
    (2..)
        .map(|n| format!("{nombre} ({n})"))
        .find(|candidato| !usados.contains(&candidato.to_lowercase()))
        .expect("siempre hay uno libre")
}

fn id_libre(est: &EstadoPartida) -> u16 {
    est.conjuntos_parametros.iter().map(|c| c.id).max().unwrap_or(0) + 1
}

/// Guarda un conjunto YA validado en la partida (sin activarlo) y lo devuelve.
fn anadir(est: &mut EstadoPartida, state: &AppState, origen: Option<String>, mut contenido: ConjuntoParametros) -> Result<ConjuntoGuardado, String> {
    if est.conjuntos_parametros.len() >= MAX_CONJUNTOS {
        return Err(format!("Una partida no puede guardar más de {MAX_CONJUNTOS} conjuntos: borra alguno"));
    }
    contenido.nombre = nombre_libre(est, &contenido.nombre);
    // Sus macros no se guardan dentro del conjunto: pasan a ser macros del host de ese conjunto (las vivas de la partida)
    let macros = std::mem::take(&mut contenido.macros);
    let guardado = ConjuntoGuardado { id: id_libre(est), origen, contenido, valores: Default::default() };
    registrar(est, &state.almacen, DeltaPartida::ConjuntoAnadido(guardado.clone()));
    crate::tablero_general::macros::crear_de_conjunto(est, &state.almacen, guardado.id, macros);
    Ok(guardado)
}

/// Copia un conjunto de serie a la partida (si ya tenía una copia, devuelve esa) SIN activarlo: «Usar» viene después.
#[tauri::command]
pub fn anadir_conjunto_de_serie(clave: String, state: State<'_, AppState>) -> Result<u16, String> {
    anadir_conjunto_de_serie_en(&clave, &state)
}

pub fn anadir_conjunto_de_serie_en(clave: &str, state: &AppState) -> Result<u16, String> {
    let mut est = state.lock();
    if let Some(ya) = est.conjuntos_parametros.iter().find(|g| g.origen.as_deref() == Some(clave)) {
        return Ok(ya.id);
    }
    let serie = integrado(clave).ok_or("Ese conjunto no existe")?;
    Ok(anadir(&mut est, state, Some(serie.clave.to_string()), serie.contenido)?.id)
}

/// Crea un conjunto NUEVO y vacío («Conjunto nuevo»: sin parámetros ni variables, con el grupo de variables de siempre) SIN activarlo: se
/// elige después con «Usar» y se rellena desde la ventana de parámetros. Es la forma de partir de cero en vez de importar o copiar uno.
#[tauri::command]
pub fn crear_conjunto_vacio(state: State<'_, AppState>) -> Result<u16, String> {
    crear_conjunto_vacio_en(&state)
}

pub fn crear_conjunto_vacio_en(state: &AppState) -> Result<u16, String> {
    let contenido = validar(ConjuntoParametros {
        nombre: "Conjunto nuevo".into(),
        descripcion: String::new(),
        parametros: vec![],
        grupos_variables: grupos_por_defecto(),
        variables: vec![],
        listas_rangos: vec![],
        escala_resultado: None,
        archivos_texto: vec![],
        macros: vec![],
    })?;
    let mut est = state.lock();
    Ok(anadir(&mut est, state, None, contenido)?.id)
}

/// Pasa a usar el conjunto `id`. Las fichas empiezan de cero (la pantalla avisa antes). No con una ficha nueva abierta:
/// su borrador tiene los valores del conjunto anterior.
#[tauri::command]
pub fn usar_conjunto(id: u16, state: State<'_, AppState>) -> Result<(), String> {
    usar_conjunto_en(id, &state)
}

pub fn usar_conjunto_en(id: u16, state: &AppState) -> Result<(), String> {
    let mut est = state.lock();
    if !est.conjuntos_parametros.iter().any(|c| c.id == id) {
        return Err("Ese conjunto ya no existe".into());
    }
    if est.conjunto_activo == id {
        return Err("Ya estás usando ese conjunto".into());
    }
    if !state.reservas().is_empty() {
        return Err("Cierra la ficha que tienes abierta antes de cambiar de conjunto".into());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::ConjuntoActivado(id));
    Ok(())
}

#[tauri::command]
pub fn renombrar_conjunto(id: u16, nombre: String, state: State<'_, AppState>) -> Result<(), String> {
    renombrar_conjunto_en(id, nombre, &state)
}

pub fn renombrar_conjunto_en(id: u16, nombre: String, state: &AppState) -> Result<(), String> {
    let mut est = state.lock();
    let actual = est.contenido_de(id).ok_or("Ese conjunto ya no existe")?;
    let nombre = nombre.split_whitespace().collect::<Vec<_>>().join(" ");
    if nombre == actual.nombre {
        return Ok(());
    }
    // Se valida como el resto (largo, caracteres) y que no lo tenga otro
    validar(ConjuntoParametros { nombre: nombre.clone(), descripcion: String::new(), parametros: vec![], grupos_variables: vec![], variables: vec![], listas_rangos: vec![], escala_resultado: None, archivos_texto: vec![], macros: vec![] })?;
    let repetido = est.conjuntos_parametros.iter().filter(|g| g.id != id).filter_map(|g| est.contenido_de(g.id)).any(|c| c.nombre.to_lowercase() == nombre.to_lowercase());
    if repetido {
        return Err(format!("Ya hay un conjunto que se llama «{nombre}»"));
    }
    registrar(&mut est, &state.almacen, DeltaPartida::ConjuntoRenombrado { id, nombre });
    Ok(())
}

/// Borra un conjunto guardado (nunca el que se está usando). No toca las fichas.
#[tauri::command]
pub fn borrar_conjunto(id: u16, state: State<'_, AppState>) -> Result<(), String> {
    borrar_conjunto_en(id, &state)
}

pub fn borrar_conjunto_en(id: u16, state: &AppState) -> Result<(), String> {
    let mut est = state.lock();
    if !est.conjuntos_parametros.iter().any(|c| c.id == id) {
        return Err("Ese conjunto ya no existe".into());
    }
    if est.conjunto_activo == id {
        return Err("No puedes borrar el conjunto que estás usando: cambia antes a otro".into());
    }
    registrar(&mut est, &state.almacen, DeltaPartida::ConjuntoBorrado(id));
    Ok(())
}

/// El conjunto completo (con el nombre y la descripción que tiene guardados) que está en uso: lo que lleva dentro una plantilla de ficha
/// exportada, para que se pueda importar desde ella. `con_macros`: también sus macros (las del host de este conjunto); lo pide la
/// plantilla de los parámetros (en blanco), no la de una ficha, que lleva solo las suyas.
#[tauri::command]
pub fn obtener_conjunto_activo(con_macros: Option<bool>, state: State<'_, AppState>) -> ConjuntoParametros {
    let est = state.lock();
    let mut c = est.contenido_de(est.conjunto_activo).unwrap_or_else(|| est.contenido_vivo());
    c.macros = if con_macros.unwrap_or(false) { crate::tablero_general::macros::del_conjunto_activo(&est) } else { Vec::new() };
    c
}

/// El conjunto que lleva una plantilla de ficha (el texto del .html): se busca el bloque de datos `<script id="datos-openroll">`, se
/// comprueba que es una plantilla de OpenRoll y se valida el conjunto entero (`validar`). Es entrada NO fiable: no se ejecuta nada y lo
/// demás de la plantilla (la ficha, su UUID, sus .txt) ni se mira. Una plantilla sin el bloque (de antes, o exportada por un jugador, que
/// no conoce el conjunto entero) se rechaza con su motivo.
pub fn de_plantilla(html: &str) -> Result<ConjuntoParametros, String> {
    const NO_ES: &str = "Ese archivo no es una plantilla de ficha de OpenRoll";
    let datos = bloque_de_datos(html).ok_or(NO_ES)?;
    let json: serde_json::Value = serde_json::from_str(datos).map_err(|_| "La plantilla está dañada".to_string())?;
    if json.get("formato").and_then(|f| f.as_str()) != Some("openroll-plantilla") {
        return Err(NO_ES.into());
    }
    match json.get("version").and_then(|v| v.as_u64()) {
        Some(v) if v > VERSION_PLANTILLA => return Err("La plantilla es de una versión más nueva de OpenRoll: actualiza la aplicación".into()),
        Some(v) if v >= 1 => {}
        _ => return Err("La plantilla está dañada (versión)".into()),
    }
    let conjunto = json.get("definicion_conjunto").filter(|c| !c.is_null()).ok_or(
        "Esta plantilla no lleva el conjunto de parámetros (es de una versión anterior, o la exportó un jugador): vuelve a exportarla desde OpenRoll",
    )?;
    let conjunto: ConjuntoParametros =
        serde_json::from_value(conjunto.clone()).map_err(|e| format!("El conjunto de la plantilla está mal formado: {e}"))?;
    validar(conjunto)
}

/// Versión del formato de la plantilla que sabe leer esta app (`VERSION_PLANTILLA` en el front).
const VERSION_PLANTILLA: u64 = 1;

/// El contenido del `<script ... id="datos-openroll">` de un HTML (sin ejecutar nada): el primer script cuya etiqueta lleva ese id.
fn bloque_de_datos(html: &str) -> Option<&str> {
    let mut resto = html;
    while let Some(i) = resto.find("<script") {
        let tras = &resto[i..];
        let fin_etiqueta = tras.find('>')?;
        let etiqueta = &tras[..fin_etiqueta];
        let contenido = &tras[fin_etiqueta + 1..];
        let cierre = contenido.find("</script>")?;
        if etiqueta.contains("id=\"datos-openroll\"") || etiqueta.contains("id='datos-openroll'") {
            return Some(&contenido[..cierre]);
        }
        resto = &contenido[cierre + "</script>".len()..];
    }
    None
}

/// Lee la plantilla (.html, con el tamaño comprobado ANTES de leerla), saca su conjunto, lo valida entero y lo añade a la partida (sin
/// activarlo). Devuelve el id del conjunto nuevo.
#[tauri::command]
pub fn importar_conjunto(ruta: String, state: State<'_, AppState>) -> Result<u16, String> {
    importar_conjunto_en(&ruta, &state)
}

pub fn importar_conjunto_en(ruta: &str, state: &AppState) -> Result<u16, String> {
    let contenido = de_plantilla(&crate::tablero_general::plantilla::leer_plantilla_en(ruta)?)?;
    let mut est = state.lock();
    Ok(anadir(&mut est, state, None, contenido)?.id)
}

/// Lee y valida un archivo de conjunto del disco (tamaño máximo comprobado ANTES de leerlo).
fn leer_archivo(origen: &Path) -> Result<ConjuntoParametros, String> {
    let tam = std::fs::metadata(origen).map_err(|e| format!("No se pudo abrir el archivo: {e}"))?.len();
    if tam > MAX_BYTES_ARCHIVO {
        return Err(format!("El archivo es demasiado grande para ser un conjunto de parámetros (máx. {} KiB)", MAX_BYTES_ARCHIVO / 1024));
    }
    let texto = std::fs::read_to_string(origen).map_err(|_| "El archivo no es un conjunto de parámetros de OpenRoll".to_string())?;
    de_archivo(&texto)
}

// ------------------------------------------------------------------
// Biblioteca: los conjuntos propios del host, compartidos entre TODAS sus partidas
// ------------------------------------------------------------------
// Son archivos del mismo formato que exportar/importar, en `<datos de la app>/biblioteca_conjuntos/`, fuera de las partidas
// (como `red.json`, es de este equipo). Usar uno de la biblioteca lo COPIA a la partida, igual que los de serie: la partida
// sigue siendo independiente y su log no depende de estos archivos.

const DIR_BIBLIOTECA: &str = "biblioteca_conjuntos";
/// Máximo de conjuntos en la biblioteca (que nadie la llene sin querer).
const MAX_BIBLIOTECA: usize = 200;

#[derive(Serialize, Debug, PartialEq)]
pub struct ResumenBiblioteca {
    /// Nombre del archivo dentro de la carpeta (es lo que identifica la entrada).
    pub archivo: String,
    pub nombre: String,
    pub descripcion: String,
    pub parametros: usize,
    pub variables: usize,
}

fn dir_biblioteca(raiz: &Path) -> std::path::PathBuf {
    raiz.join(DIR_BIBLIOTECA)
}

/// La ruta de una entrada de la biblioteca a partir de su nombre de archivo, sin salirse de la carpeta.
fn ruta_biblioteca(raiz: &Path, archivo: &str) -> Result<std::path::PathBuf, String> {
    if !archivo.to_lowercase().ends_with(".json") {
        return Err("Ese conjunto de la biblioteca no es válido".into());
    }
    crate::tablero_general::archivos::ruta_segura(raiz, &format!("{DIR_BIBLIOTECA}/{archivo}"), &[DIR_BIBLIOTECA])
        .map_err(|_| "Ese conjunto de la biblioteca no es válido".to_string())
}

/// Un nombre de archivo a partir del nombre del conjunto: sin caracteres que Windows no admite.
fn nombre_de_archivo(nombre: &str) -> String {
    let limpio: String = nombre.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let limpio: String = limpio.trim().trim_end_matches('.').chars().take(60).collect();
    if limpio.trim().is_empty() { "conjunto".into() } else { limpio.trim().to_string() }
}

/// Los conjuntos de la biblioteca (los archivos que no valen se saltan), por nombre.
pub fn listar_biblioteca_en(raiz: &Path) -> Vec<ResumenBiblioteca> {
    let Ok(entradas) = std::fs::read_dir(dir_biblioteca(raiz)) else { return vec![] };
    let mut v: Vec<ResumenBiblioteca> = entradas
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .take(MAX_BIBLIOTECA)
        .filter_map(|e| {
            let archivo = e.file_name().into_string().ok()?;
            let c = leer_archivo(&e.path()).ok()?;
            Some(ResumenBiblioteca { archivo, nombre: c.nombre, descripcion: c.descripcion, parametros: c.parametros.len(), variables: c.variables.len() })
        })
        .collect();
    v.sort_by_key(|r| r.nombre.to_lowercase());
    v
}

/// Guarda el conjunto `id` de la partida en la biblioteca. Si ya hay uno con ese nombre lo ACTUALIZA (así se define una vez
/// y se mejora donde sea). Devuelve el nombre del archivo.
pub fn guardar_en_biblioteca_en(id: u16, state: &AppState) -> Result<String, String> {
    let contenido = state.lock().contenido_de(id).ok_or("Ese conjunto ya no existe")?;
    let texto = a_archivo(&contenido)?;
    let dir = dir_biblioteca(&state.raiz);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear la biblioteca: {e}"))?;
    let base = nombre_de_archivo(&contenido.nombre);
    let mut archivo = format!("{base}.json");
    let mut n = 2;
    // Mismo nombre de archivo: si es el mismo conjunto (mismo nombre) se sobrescribe; si es otro, se busca un hueco
    while let Ok(existente) = leer_archivo(&dir.join(&archivo)) {
        if existente.nombre.to_lowercase() == contenido.nombre.to_lowercase() {
            break;
        }
        archivo = format!("{base} ({n}).json");
        n += 1;
    }
    let destino = ruta_biblioteca(&state.raiz, &archivo)?;
    if !destino.exists() && listar_biblioteca_en(&state.raiz).len() >= MAX_BIBLIOTECA {
        return Err(format!("La biblioteca no puede tener más de {MAX_BIBLIOTECA} conjuntos: borra alguno"));
    }
    let tmp = destino.with_extension("json.tmp");
    std::fs::write(&tmp, texto).map_err(|e| format!("No se pudo escribir en la biblioteca: {e}"))?;
    std::fs::rename(&tmp, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("No se pudo escribir en la biblioteca: {e}")
    })?;
    Ok(archivo)
}

/// Copia un conjunto de la biblioteca a la partida (sin usarlo). Devuelve el id del conjunto nuevo.
pub fn anadir_de_biblioteca_en(archivo: &str, state: &AppState) -> Result<u16, String> {
    let contenido = leer_archivo(&ruta_biblioteca(&state.raiz, archivo)?)?;
    let mut est = state.lock();
    Ok(anadir(&mut est, state, None, contenido)?.id)
}

/// Quita un conjunto de la biblioteca. No toca ninguna partida (las copias que ya tengan siguen ahí).
pub fn borrar_de_biblioteca_en(archivo: &str, state: &AppState) -> Result<(), String> {
    let ruta = ruta_biblioteca(&state.raiz, archivo)?;
    std::fs::remove_file(ruta).map_err(|e| format!("No se pudo borrar de la biblioteca: {e}"))
}

#[tauri::command]
pub fn listar_biblioteca_conjuntos(state: State<'_, AppState>) -> Vec<ResumenBiblioteca> {
    listar_biblioteca_en(&state.raiz)
}

#[tauri::command]
pub fn guardar_en_biblioteca(id: u16, state: State<'_, AppState>) -> Result<String, String> {
    guardar_en_biblioteca_en(id, &state)
}

#[tauri::command]
pub fn anadir_de_biblioteca(archivo: String, state: State<'_, AppState>) -> Result<u16, String> {
    anadir_de_biblioteca_en(&archivo, &state)
}

#[tauri::command]
pub fn borrar_de_biblioteca(archivo: String, state: State<'_, AppState>) -> Result<(), String> {
    borrar_de_biblioteca_en(&archivo, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tablero_general::instancias::valores_iniciales;
    use crate::tablero_general::variables::pruebas::{derivada, grupos, var};

    /// Una ficha con los parámetros de serie y sin variables.
    fn ficha(id: u16) -> Entidad {
        let mut parametros = Vec::new();
        normalizar_parametros(&parametros_por_defecto(), &mut parametros);
        Entidad {
            id, nombre: format!("F{id}"), tipo: TipoEntidad::Enemigo, parametros, variables: vec![], pasos_valor: vec![],
            cuerpo: Cuerpo::Pequenio, zona_id: 1, foto_url: None, miniatura_url: None, foto_encuadre: Default::default(),
            archivos_adjuntos: vec![], es_unico: false, vision: 0, plantilla_id: None, actuales: vec![], propietarios: vec![],
        }
    }

    fn instancia(id: u16, base: Entidad) -> Instancia {
        Instancia {
            id_instancia: id, id_mapa: None, valores_actuales: valores_iniciales(&parametros_por_defecto(), &base), base,
            coordenadas: Coordenada::NoPuesto, activo: true, rotacion: 0, espejo: false, orden: 0, capa: 0, oculto: false, atenuada: false,
        }
    }

    fn conjunto_dnd() -> ConjuntoParametros {
        ConjuntoParametros {
            nombre: "Mini D&D".into(),
            descripcion: "Para probar".into(),
            parametros: parametros_por_defecto(),
            grupos_variables: grupos(),
            variables: vec![var(1, "Destreza", "des", 10, 2), var(3, "Competencia", "com", 0, 1), derivada(4, "Acrobacias", "acro", &[1], &[3])],
            listas_rangos: vec![],
            escala_resultado: None,
            archivos_texto: vec![],
            macros: vec![],
        }
    }

    #[test]
    fn todo_conjunto_de_serie_es_valido_y_tiene_clave_unica() {
        let todos = integrados();
        assert!(!todos.is_empty());
        for i in &todos {
            assert_eq!(validar(i.contenido.clone()).as_ref(), Ok(&i.contenido), "{}", i.clave);
        }
        let mut claves: Vec<_> = todos.iter().map(|i| i.clave).collect();
        claves.sort_unstable();
        claves.dedup();
        assert_eq!(claves.len(), todos.len(), "claves repetidas");
        // Una partida nueva nace con el genérico como activo
        let e = EstadoPartida::default();
        assert_eq!((e.conjunto_activo, e.conjuntos_parametros.len()), (1, 1));
        assert_eq!(e.contenido_de(1).unwrap(), generico());
    }

    #[test]
    fn el_archivo_va_y_vuelve_igual() {
        let c = conjunto_dnd();
        let texto = a_archivo(&c).unwrap();
        assert!(texto.contains("\"formato\": \"openroll-conjunto\"") && texto.contains("\"version\": 1"));
        assert_eq!(de_archivo(&texto).unwrap(), c);
    }

    #[test]
    fn un_archivo_ajeno_roto_o_de_otra_version_se_rechaza_con_su_motivo() {
        for malo in ["", "hola", "[]", "{}", r#"{"formato":"otra-cosa","version":1,"nombre":"X"}"#, r#"{"formato":"openroll-conjunto","nombre":"X"}"#] {
            assert!(de_archivo(malo).is_err(), "{malo}");
        }
        let nueva = a_archivo(&conjunto_dnd()).unwrap().replace("\"version\": 1", "\"version\": 99");
        assert!(de_archivo(&nueva).unwrap_err().contains("más nueva"));
        let cero = a_archivo(&conjunto_dnd()).unwrap().replace("\"version\": 1", "\"version\": 0");
        assert!(de_archivo(&cero).is_err());
    }

    #[test]
    fn la_validacion_rechaza_lo_que_a_mano_tampoco_se_podria_crear() {
        let ok = conjunto_dnd();
        assert!(validar(ok.clone()).is_ok());
        let mal = |f: &dyn Fn(&mut ConjuntoParametros)| {
            let mut c = ok.clone();
            f(&mut c);
            validar(c)
        };
        assert!(mal(&|c| c.nombre = "  ".into()).is_err(), "sin nombre");
        assert!(mal(&|c| c.nombre = "x".repeat(61)).is_err(), "nombre largo");
        assert!(mal(&|c| c.nombre = "a\u{202E}b".into()).is_err(), "bidi");
        assert!(mal(&|c| c.descripcion = "x".repeat(301)).is_err());
        assert!(mal(&|c| c.parametros.push(c.parametros[0].clone())).is_err(), "parámetro con el mismo id");
        let mut repetido = ok.parametros[0].clone();
        repetido.id = 99;
        assert!(mal(&|c| c.parametros.push(repetido.clone())).is_err(), "mismo nombre de parámetro");
        assert!(mal(&|c| c.parametros[0].representar_mapa = Some(Forma::Rombo(vec![1, 2]))).is_err(), "color mal");
        assert!(mal(&|c| c.variables.push(var(1, "Otra", "o", 10, 2))).is_err(), "variable con el mismo id");
        assert!(mal(&|c| c.variables[0].escala = 0).is_err(), "escala");
        assert!(mal(&|c| c.variables.push(derivada(9, "Sigilo", "sig", &[77], &[]))).is_err(), "base inexistente");
        assert!(mal(&|c| c.variables.push(derivada(9, "Sigilo", "sig", &[4], &[]))).is_err(), "base que es derivada");
        assert!(mal(&|c| c.variables[2].grupo = 9).is_err(), "grupo inexistente");
        assert!(mal(&|c| c.grupos_variables.push(c.grupos_variables[0].clone())).is_err(), "grupo repetido");
        assert!(mal(&|c| c.variables.push(var(9, "Destreza", "z", 10, 2))).is_err(), "nombre repetido entre variables");
        assert!(mal(&|c| c.parametros = (0..41).map(|i| DefinicionParametro { id: i, clave: format!("P{i}"), ..ok.parametros[0].clone() }).collect()).is_err(), "demasiados");
    }

    #[test]
    fn una_derivada_antes_que_su_base_en_el_archivo_tambien_vale_y_el_orden_se_conserva() {
        let mut c = conjunto_dnd();
        c.variables = vec![derivada(4, "Acrobacias", "acro", &[1], &[3]), var(1, "Destreza", "des", 10, 2), var(3, "Competencia", "com", 0, 1)];
        let v = validar(c).unwrap();
        assert_eq!(v.variables.iter().map(|d| d.id).collect::<Vec<_>>(), vec![4, 1, 3]);
    }

    #[test]
    fn una_partida_antigua_sin_conjuntos_recibe_uno_con_lo_que_ya_tenia() {
        let mut e = EstadoPartida { conjuntos_parametros: vec![], conjunto_activo: 0, ..EstadoPartida::default() };
        asegurar(&mut e);
        assert_eq!((e.conjunto_activo, e.conjuntos_parametros[0].contenido.nombre.as_str()), (1, "Genérico"));
        assert_eq!(e.conjuntos_parametros[0].origen.as_deref(), Some("generico"));

        // Con parámetros propios: «Personalizado», sin perder nada
        let mut e = EstadoPartida { conjuntos_parametros: vec![], conjunto_activo: 0, ..EstadoPartida::default() };
        e.definiciones_parametros.pop();
        e.definiciones_variables.push(var(1, "Fuerza", "f", 10, 2));
        asegurar(&mut e);
        let c = e.contenido_de(1).unwrap();
        assert_eq!((c.nombre.as_str(), c.parametros.len(), c.variables.len()), ("Personalizado", 3, 1));
        assert_eq!(e.conjuntos_parametros[0].origen, None);

        // Una partida guardada antes de los conjuntos se lee sin los campos nuevos
        let vieja: EstadoPartida = serde_json::from_str("{}").unwrap();
        assert!(vieja.conjuntos_parametros.is_empty());
    }

    #[test]
    fn cada_conjunto_guarda_lo_suyo_y_al_volver_las_fichas_recuperan_lo_que_tenian() {
        let mut e = EstadoPartida::default();
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        // Una ficha con valores del conjunto 1, una instancia con el valor actual de su parámetro con máximo tocado y un paso de − y +
        let mut f = ficha(1);
        f.parametros[0].valor = ValorParametro::Numero(77);
        f.pasos_valor = vec![PasoValor { id_definicion: ID_VIDA, paso: 5 }];
        e.banco_entidades.push(f.clone());
        let mut inst = instancia(1, f);
        inst.valores_actuales[0].valor = 3;
        e.instancias_activas.push(inst);
        // Se edita el conjunto activo (el 1): se quita Armadura
        e.aplicar_delta(&DeltaPartida::ParametroBorrado(ID_ARMADURA));
        assert_eq!(e.contenido_de(1).unwrap().parametros.len(), 3, "lo vivo cuenta, aunque lo guardado esté desfasado");

        // Primera vez en el 2: todo por defecto, NADA heredado del 1 aunque el parámetro tenga el mismo id (1)
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        assert_eq!(e.conjunto_activo, 2);
        assert_eq!(e.definiciones_variables.len(), 3);
        let f = &e.banco_entidades[0];
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(10), "el valor del conjunto nuevo, no el 77 del otro");
        assert!(f.pasos_valor.is_empty(), "ni la variación de − y +");
        assert_eq!(f.variables.len(), 3);
        assert_eq!(e.instancias_activas[0].valores_actuales[0].valor, 10, "ni el valor actual");
        assert_eq!(e.instancias_activas[0].base.variables.len(), 3);

        // En el 2 se toca todo otra vez: sus valores no deben acabar en el 1
        e.banco_entidades[0].parametros[0].valor = ValorParametro::Numero(55);
        e.instancias_activas[0].base.parametros[0].valor = ValorParametro::Numero(55);
        e.instancias_activas[0].valores_actuales[0].valor = 8;
        e.banco_entidades[0].variables[0].valor = 16;
        e.instancias_activas[0].base.variables[0].valor = 16;

        // Volver al 1: sus definiciones editadas (sin Armadura) y LO QUE TENÍAN las fichas
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(1));
        assert_eq!(e.definiciones_parametros.len(), 3);
        assert!(e.definiciones_variables.is_empty());
        let f = &e.banco_entidades[0];
        assert_eq!(f.parametros.len(), 3);
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(77));
        assert_eq!(f.pasos_valor, vec![PasoValor { id_definicion: ID_VIDA, paso: 5 }]);
        assert!(f.variables.is_empty());
        assert_eq!(e.instancias_activas[0].valores_actuales[0].valor, 3);
        assert_eq!(e.instancias_activas[0].base.parametros[0].valor, ValorParametro::Numero(77), "la copia de la instancia también");

        // Y al volver al 2, lo del 2 (no lo del 1)
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        assert_eq!(e.banco_entidades[0].parametros[0].valor, ValorParametro::Numero(55));
        assert_eq!(e.banco_entidades[0].variables[0].valor, 16);
        assert_eq!(e.instancias_activas[0].valores_actuales[0].valor, 8);
        assert!(e.banco_entidades[0].pasos_valor.is_empty());

        // Activar el que ya está, o uno que no existe, no hace nada
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(99));
        assert_eq!(e.conjunto_activo, 2);
    }

    #[test]
    fn una_ficha_creada_en_otro_conjunto_no_hereda_nada_y_un_id_de_instancia_reutilizado_tampoco() {
        let mut e = EstadoPartida::default();
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        let mut f = ficha(1);
        f.parametros[0].valor = ValorParametro::Numero(77);
        e.banco_entidades.push(f.clone());
        let mut inst = instancia(5, f.clone());
        inst.valores_actuales[0].valor = 2;
        e.instancias_activas.push(inst);

        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        // En el 2: la instancia 5 se borra y se crea otra con el MISMO id; y nace una ficha nueva (la 9)
        e.aplicar_delta(&DeltaPartida::InstanciaBorrada(5));
        let mut nueva = instancia(5, ficha(1));
        nueva.base.parametros.clear();
        e.aplicar_delta(&DeltaPartida::InstanciaCreada(nueva));
        e.banco_entidades.push(ficha(9));
        e.ajustar_a_definiciones();

        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(1));
        assert_eq!(e.banco_entidades.iter().find(|x| x.id == 1).unwrap().parametros[0].valor, ValorParametro::Numero(77), "la ficha de siempre sí");
        let nueva = e.banco_entidades.iter().find(|x| x.id == 9).unwrap();
        assert_eq!(nueva.parametros[0].valor, ValorParametro::Numero(10), "la ficha 9 nació en el 2: por defecto");
        // La instancia 5 nueva empieza en el máximo de su ficha (77), no en el 2 que tenía la vieja con el mismo id
        assert_eq!(e.instancias_activas[0].valores_actuales[0].valor, 77, "no hereda el valor actual de la instancia borrada");
        assert!(e.conjuntos_parametros.iter().find(|c| c.id == 1).unwrap().valores.instancias.is_empty(), "lo guardado de la instancia vieja se podó");
    }

    #[test]
    fn lo_que_esta_en_la_papelera_tambien_vuelve_y_al_vaciarla_se_poda() {
        use crate::tablero_general::papelera::{Borrado, GrupoPapelera};
        let mut e = EstadoPartida::default();
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        let mut f = ficha(4);
        f.parametros[0].valor = ValorParametro::Numero(31);
        // Una ficha borrada (en la papelera) con una instancia
        let mut inst = instancia(8, f.clone());
        inst.valores_actuales[0].valor = 6;
        e.papelera.push(GrupoPapelera { id: 1, borrados: vec![Borrado::Entidad { entidad: f, instancias: vec![inst] }] });

        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        let Borrado::Entidad { entidad, instancias } = &e.papelera[0].borrados[0] else { panic!() };
        assert!(entidad.parametros.is_empty(), "en el 2 lo de la papelera está vacío hasta restaurarla");
        assert!(instancias[0].valores_actuales.is_empty());

        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(1));
        let Borrado::Entidad { entidad, instancias } = &e.papelera[0].borrados[0] else { panic!() };
        assert_eq!(entidad.parametros[0].valor, ValorParametro::Numero(31));
        assert_eq!(instancias[0].valores_actuales[0].valor, 6);

        // Vaciar la papelera: lo guardado de esas fichas se descarta (sus ids se pueden reutilizar)
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        assert!(!e.conjuntos_parametros.iter().find(|c| c.id == 1).unwrap().valores.fichas.is_empty());
        e.aplicar_delta(&DeltaPartida::PapeleraVaciada);
        assert!(e.conjuntos_parametros.iter().all(|c| c.valores.fichas.is_empty() && c.valores.instancias.is_empty()));
    }

    #[test]
    fn lo_guardado_por_conjunto_sobrevive_a_guardar_y_cargar_y_no_se_exporta() {
        let mut e = EstadoPartida::default();
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        let mut f = ficha(1);
        f.parametros[0].valor = ValorParametro::Numero(77);
        e.banco_entidades.push(f);
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        let leido: EstadoPartida = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(leido.conjuntos_parametros[0].valores.fichas[0].parametros[0].valor, ValorParametro::Numero(77));
        // El archivo de un conjunto solo lleva sus definiciones
        let texto = a_archivo(&leido.contenido_de(1).unwrap()).unwrap();
        assert!(!texto.contains("valores") && !texto.contains("id_entidad"), "{texto}");
        // Una partida guardada antes de esto (conjuntos sin `valores`) se lee igual
        let vieja: ConjuntoGuardado = serde_json::from_str(&serde_json::to_string(&leido.conjuntos_parametros[1]).unwrap().replace(",\"valores\":{\"fichas\":[],\"instancias\":[]}", "")).unwrap();
        assert!(vieja.valores.fichas.is_empty());
    }

    fn estado_en_a(nombre: &str) -> AppState {
        use crate::tablero_general::almacen::{pruebas::dir_temporal, Almacen};
        use crate::tablero_general::partidas;
        let raiz = dir_temporal(nombre);
        std::fs::create_dir_all(&raiz).unwrap();
        partidas::crear(&raiz, "A").unwrap();
        let almacen = Almacen::abrir(partidas::ruta_partida(&raiz, "A")).unwrap();
        AppState::nuevo(EstadoPartida::default(), almacen, raiz)
    }

    #[test]
    fn una_lista_de_rangos_compartida_se_cambia_en_un_sitio_y_las_variables_y_las_fichas_la_siguen() {
        use crate::tablero_general::modelos::OpcionVariable;
        use crate::tablero_general::partida::{crear_definicion_variable_en, procesar_delta};
        use crate::tablero_general::rangos::{borrar_lista_rangos_en, crear_lista_rangos_en, modificar_lista_rangos_en};
        let state = estado_en_a("listas_rangos");
        procesar_delta(&state, DeltaPartida::GrupoVariablesCreado(GrupoVariables { id: 2, nombre: "Habilidades".into(), tipo: TipoGrupoVariables::Derivadas }));
        let rango = |n: &str, b: i16| OpcionVariable { nombre: n.into(), bonus: b };
        let lista = ListaRangos { id: 0, nombre: "Competencia".into(), opciones: vec![rango("Sin entrenar", 0), rango("Entrenado", 2), rango("Experto", 4)] };

        // Crear una lista (el id lo pone Rust); guardar lo mismo otra vez devuelve la que hay; un nombre repetido con otros rangos, error
        let c = crear_lista_rangos_en(lista.clone(), &state).unwrap();
        assert_eq!(c.id, 1);
        assert_eq!(crear_lista_rangos_en(lista.clone(), &state).unwrap().id, 1);
        assert!(crear_lista_rangos_en(ListaRangos { opciones: vec![rango("Otro", 1)], ..lista.clone() }, &state).is_err());
        assert!(crear_lista_rangos_en(ListaRangos { opciones: vec![], ..lista.clone() }, &state).is_err());
        assert_eq!(state.lock().listas_rangos.len(), 1);

        // Variables: una base y dos derivadas que comparten la lista; una lista que no existe se rechaza
        let des = crear_definicion_variable_en(var(0, "Destreza", "des", 10, 2), &state).unwrap();
        let derivada_con = |nombre: &str, abreviatura: &str, rangos: Option<u16>| DefinicionVariable { rangos, ..derivada(0, nombre, abreviatura, &[des.id], &[]) };
        assert!(crear_definicion_variable_en(derivada_con("Acrobacias", "acro", Some(9)), &state).unwrap_err().contains("lista de rangos"));
        let acro = crear_definicion_variable_en(derivada_con("Acrobacias", "acro", Some(1)), &state).unwrap();
        let sig = crear_definicion_variable_en(derivada_con("Sigilo", "sig", Some(1)), &state).unwrap();
        for id in [acro.id, sig.id] {
            let d = state.lock().definiciones_variables.iter().find(|d| d.id == id).cloned().unwrap();
            assert_eq!((d.rangos, d.opciones.len()), (Some(1), 3), "cada variable lleva la copia de su lista");
        }
        // Una base no puede llevar rangos
        assert!(crear_definicion_variable_en(DefinicionVariable { rangos: Some(1), ..var(0, "Rara", "r", 10, 2) }, &state).unwrap().rangos.is_none());

        // Una ficha con Experto elegido en Acrobacias
        {
            let mut est = state.lock();
            est.banco_entidades.push(ficha(1));
            est.ajustar_a_definiciones();
            est.banco_entidades[0].variables.iter_mut().find(|v| v.id_definicion == acro.id).unwrap().eleccion = 2;
        }
        // Cambiar la lista cambia las dos variables; la elección que ya no existe vuelve a la primera
        modificar_lista_rangos_en(ListaRangos { id: 1, opciones: vec![rango("Sin entrenar", 0), rango("Entrenado", 3)], ..lista.clone() }, &state).unwrap();
        {
            let est = state.lock();
            assert!(est.definiciones_variables.iter().filter(|d| d.rangos == Some(1)).all(|d| d.opciones.len() == 2 && d.opciones[1].bonus == 3));
            assert_eq!(est.banco_entidades[0].variables.iter().find(|v| v.id_definicion == acro.id).unwrap().eleccion, 0);
        }
        // Sin cambios no se registra nada; renombrar, sí
        let seq = state.lock().ultimo_seq;
        modificar_lista_rangos_en(ListaRangos { id: 1, nombre: "Competencia".into(), opciones: vec![rango("Sin entrenar", 0), rango("Entrenado", 3)] }, &state).unwrap();
        assert_eq!(state.lock().ultimo_seq, seq);
        assert!(modificar_lista_rangos_en(ListaRangos { id: 7, ..lista.clone() }, &state).is_err());

        // Cada conjunto tiene sus listas: al pasar a otro se van y al volver se recuperan
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        procesar_delta(&state, DeltaPartida::ConjuntoActivado(2));
        assert!(state.lock().listas_rangos.is_empty());
        procesar_delta(&state, DeltaPartida::ConjuntoActivado(1));
        assert_eq!(state.lock().listas_rangos.len(), 1);
        assert_eq!(state.lock().definiciones_variables.iter().filter(|d| d.opciones.len() == 2).count(), 2);

        // Borrar la lista deja a las variables sin rangos (y no se pierde nada más)
        borrar_lista_rangos_en(1, &state).unwrap();
        assert!(state.lock().definiciones_variables.iter().all(|d| d.rangos.is_none() && d.opciones.is_empty()));
        assert!(borrar_lista_rangos_en(1, &state).is_err());

        // Una partida de antes de las listas, con rangos escritos en cada variable, los pasa a UNA lista al cargarla
        let mut vieja = EstadoPartida::default();
        let dos = vec![rango("A", 0), rango("B", 1)];
        vieja.grupos_variables.push(GrupoVariables { id: 2, nombre: "H".into(), tipo: TipoGrupoVariables::Derivadas });
        vieja.definiciones_variables = vec![
            var(1, "Destreza", "des", 10, 2),
            DefinicionVariable { opciones: dos.clone(), ..derivada(2, "Acrobacias", "acro", &[1], &[]) },
            DefinicionVariable { opciones: dos.clone(), ..derivada(3, "Sigilo", "sig", &[1], &[]) },
        ];
        vieja.ajustar_a_definiciones();
        assert_eq!(vieja.listas_rangos.len(), 1, "las dos tenían los mismos rangos: una sola lista");
        assert!(vieja.definiciones_variables[1..].iter().all(|d| d.rangos == Some(vieja.listas_rangos[0].id) && d.opciones == dos));
    }

    #[test]
    fn crear_un_conjunto_vacio_lo_guarda_sin_activarlo_con_nombre_libre_y_se_puede_usar() {
        let state = estado_en_a("conjunto_vacio");
        let antes = state.lock().conjuntos_parametros.len();
        let id = crear_conjunto_vacio_en(&state).unwrap();
        let otro = crear_conjunto_vacio_en(&state).unwrap();
        let est = state.lock();
        assert_eq!(est.conjuntos_parametros.len(), antes + 2);
        assert_ne!(id, otro);
        assert_ne!(est.conjunto_activo, id, "no se activa solo");
        let c = est.contenido_de(id).unwrap();
        assert_eq!((c.nombre.as_str(), c.parametros.len(), c.variables.len(), c.grupos_variables.len()), ("Conjunto nuevo", 0, 0, 1));
        assert_eq!(est.contenido_de(otro).unwrap().nombre, "Conjunto nuevo (2)", "el nombre se hace único");
        drop(est);
        // Se puede usar: las fichas se quedan sin parámetros ni variables
        usar_conjunto_en(id, &state).unwrap();
        assert!(state.lock().definiciones_parametros.is_empty());
    }

    #[test]
    fn una_partida_nueva_nace_con_dnd_y_una_que_ya_existe_conserva_lo_suyo() {
        use crate::tablero_general::almacen::{pruebas::dir_temporal, Almacen};
        use crate::tablero_general::estado::cargar;
        let nueva = EstadoPartida::para_partida_nueva();
        assert_eq!(nueva.conjunto_activo, 1);
        assert_eq!(nueva.conjuntos_parametros.len(), 1);
        assert_eq!(nueva.conjuntos_parametros[0].origen.as_deref(), Some("dnd5e"));
        assert_eq!(nueva.conjuntos_parametros[0].contenido.nombre, "D&D 5e");
        let dnd = crate::tablero_general::conjuntos_serie::dnd5e();
        assert_eq!(nueva.grupos_variables.len(), 6);
        assert_eq!(nueva.definiciones_variables.len(), 46);
        assert_eq!(nueva.definiciones_parametros, dnd.parametros, "los de la hoja de D&D");
        assert_eq!(nueva.archivos_texto_defecto, dnd.archivos_texto, "y sus archivos de texto");
        assert!(nueva.escala_resultado.is_some());
        // Y no nace vacía: una zona con un mapa sin imagen, con la rejilla de siempre
        assert_eq!(nueva.zonas.iter().map(|z| (z.id, z.nombre.as_str())).collect::<Vec<_>>(), [(1, "Zona 1")]);
        assert_eq!((nueva.mapas.len(), nueva.mapas[0].id_zona, nueva.mapas[0].url_imagen_fondo.clone(), nueva.mapas[0].rejilla), (1, 1, None, 50));
        // Las definiciones vivas SON las del conjunto activo
        assert_eq!(nueva.contenido_de(1).unwrap().variables, nueva.definiciones_variables);
        // El genérico sigue ahí para añadirlo, y `default()` no cambia (es el de las partidas anteriores a los conjuntos)
        assert!(integrados().iter().any(|i| i.clave == "generico"));
        assert!(EstadoPartida::default().definiciones_variables.is_empty());

        // Carpeta vacía y partida.json roto: partida nueva, con D&D
        let dir = dir_temporal("partida_nueva_dnd");
        let almacen = Almacen::abrir(dir.clone()).unwrap();
        let (e, informe) = cargar(&almacen).unwrap();
        assert!(informe.partida_nueva);
        assert_eq!(e.conjuntos_parametros[0].origen.as_deref(), Some("dnd5e"));
        std::fs::write(dir.join("partida.json"), "{esto no es json").unwrap();
        let (e, informe) = cargar(&almacen).unwrap();
        assert!(informe.partida_nueva && e.conjuntos_parametros[0].origen.as_deref() == Some("dnd5e"));

        // Una partida de antes de los conjuntos, con sus propias definiciones: las conserva y se queda como «Personalizado»
        let mut propia = EstadoPartida::default();
        propia.definiciones_parametros.pop();
        propia.definiciones_variables = vec![var(1, "Fuerza", "f", 10, 2)];
        propia.conjuntos_parametros.clear();
        let dir = dir_temporal("partida_propia");
        let almacen = Almacen::abrir(dir.clone()).unwrap();
        std::fs::write(dir.join("partida.json"), serde_json::to_string(&propia).unwrap()).unwrap();
        let (e, informe) = cargar(&almacen).unwrap();
        assert!(!informe.partida_nueva);
        assert_eq!(e.definiciones_parametros.len(), 3);
        assert_eq!(e.definiciones_variables.len(), 1);
        assert_eq!(e.conjuntos_parametros.len(), 1);
        assert_eq!(e.conjuntos_parametros[0].contenido.nombre, "Personalizado");
        assert_eq!(e.conjuntos_parametros[0].origen, None);
        assert_eq!(e.conjunto_activo, 1);
        // Una guardada con los parámetros de serie y nada más (la de antes de los conjuntos) sigue siendo «Genérico», no D&D
        let mut generica = EstadoPartida::default();
        generica.conjuntos_parametros.clear();
        let dir = dir_temporal("partida_generica");
        let almacen = Almacen::abrir(dir.clone()).unwrap();
        std::fs::write(dir.join("partida.json"), serde_json::to_string(&generica).unwrap()).unwrap();
        let (e, _) = cargar(&almacen).unwrap();
        assert_eq!(e.conjuntos_parametros[0].contenido.nombre, "Genérico");
        assert!(e.definiciones_variables.is_empty());
    }

    #[test]
    fn cada_conjunto_de_serie_se_importa_de_una_plantilla_con_sus_rangos_calculos_y_escala_y_se_puede_usar() {
        let state = estado_en_a("series_ida_y_vuelta");
        for clave in ["dnd5e", "pathfinder2e", "cthulhu7e"] {
            let id = anadir_conjunto_de_serie_en(clave, &state).unwrap();
            let original = state.lock().contenido_de(id).unwrap();
            let serie = integrado(clave).unwrap().contenido;
            // Igual al original, salvo sus macros, que pasan a ser macros de la partida de ese conjunto
            assert_eq!(original, ConjuntoParametros { macros: vec![], ..serie.clone() }, "{clave}: la copia es igual al original");
            let nombres: Vec<String> = state.lock().macros.iter().filter(|m| m.conjunto == Some(id)).map(|m| m.nombre.clone()).collect();
            assert_eq!(nombres, serie.macros.iter().map(|m| m.nombre.clone()).collect::<Vec<_>>(), "{clave}: sus macros");
            // Llevarlo en una plantilla y volver a importarla da lo mismo (con la escala, los rangos y los cálculos de parámetros)
            let archivo = state.raiz.join(format!("{clave}.html"));
            std::fs::write(&archivo, plantilla_con(&original)).unwrap();
            let copia = importar_conjunto_en(archivo.to_str().unwrap(), &state).unwrap();
            let importado = state.lock().contenido_de(copia).unwrap();
            assert_eq!(importado.escala_resultado, original.escala_resultado, "{clave}");
            assert_eq!(importado.variables, original.variables, "{clave}");
            assert_eq!(importado.parametros, original.parametros, "{clave}");
            assert_eq!(importado.nombre, format!("{} (2)", original.nombre), "ya había uno con ese nombre");
        }
        // Se puede usar cada uno: trae su escala y sus parámetros, y las fichas siguen siendo coherentes
        let ids: Vec<(String, u16)> = state.lock().conjuntos_parametros.iter().filter_map(|g| g.origen.clone().map(|o| (o, g.id))).collect();
        for (origen, id) in ids.into_iter().filter(|(o, _)| o != "generico") {
            usar_conjunto_en(id, &state).unwrap();
            let est = state.lock();
            let esperado = integrado(&origen).unwrap().contenido;
            assert_eq!(est.definiciones_variables, esperado.variables, "{origen}");
            assert_eq!(est.escala_resultado, esperado.escala_resultado, "{origen}");
            assert_eq!(est.definiciones_parametros, esperado.parametros, "{origen}");
        }
    }

    #[test]
    fn la_escala_de_resultados_se_aplica_en_el_chat_y_cada_conjunto_guarda_la_suya() {
        use crate::tablero_general::dados::tirar_dado_de;
        use crate::tablero_general::escalas::{ejemplos, guardar_escala_resultado_en};
        let state = estado_en_a("escala_resultados");
        let cthulhu = ejemplos().into_iter().find(|e| e.clave == "cthulhu").unwrap().escala;

        // Sin escala: «Éxito» o «Fallo»; con `vs`, `<=` o `>=`, y la tirada guarda si acertó
        let salida = tirar_dado_de(&state, ID_HOST, "1d100 vs 100", true, None).unwrap();
        assert!(salida.ends_with("· Éxito"), "{salida}");
        let ultima = state.lock().historial_tiradas[0].clone();
        assert_eq!((ultima.veredicto.as_str(), ultima.acierta, ultima.especial), ("Éxito", Some(true), false));
        assert!(ultima.comando.contains(" <= 100"), "{}", ultima.comando);
        assert!(tirar_dado_de(&state, ID_HOST, "1d20 >= 21", true, None).unwrap().ends_with("· Fallo"));
        assert!(tirar_dado_de(&state, ID_HOST, "1d20 > 20", true, None).unwrap().ends_with("· Fallo"), "un estricto no acierta igualando");
        assert!(tirar_dado_de(&state, ID_HOST, "1d20 < 21", true, None).unwrap().ends_with("· Éxito"));
        assert!(state.lock().historial_tiradas[0].comando.contains(" < 21"));
        assert!(tirar_dado_de(&state, ID_HOST, "1d20 >", true, None).unwrap_err().contains("objetivo"));
        assert!(tirar_dado_de(&state, ID_HOST, "1d6", true, None).unwrap().matches('·').count() == 0, "una tirada normal no se valora");
        assert_eq!(state.lock().historial_tiradas[0].acierta, None);

        // Con la escala de Cthulhu un 1 natural que acierta es «Crítico»; lo demás, un nivel o «Éxito»
        guardar_escala_resultado_en(Some(cthulhu.clone()), &state).unwrap();
        for _ in 0..30 {
            tirar_dado_de(&state, ID_HOST, "1d100 <= 100", true, None).unwrap();
            let t = state.lock().historial_tiradas[0].clone();
            assert!(["Crítico", "Éxito extremo", "Éxito difícil", "Éxito"].contains(&t.veredicto.as_str()), "{t:?}");
            assert_eq!(t.especial, t.veredicto == "Crítico");
        }
        // Los dos lados pueden tirar; con el número a la izquierda, el sujeto es el dado
        assert!(tirar_dado_de(&state, ID_HOST, "1d20 + 50 >= 1d20", true, None).is_ok());
        assert!(tirar_dado_de(&state, ID_HOST, "100 >= 1d100", true, None).unwrap().contains("· "));
        assert!(tirar_dado_de(&state, ID_HOST, "1d20 vs 5 vs 6", true, None).is_err());
        // Guardar lo mismo no registra nada; una escala rota se rechaza
        let seq = state.lock().ultimo_seq;
        guardar_escala_resultado_en(Some(cthulhu.clone()), &state).unwrap();
        assert_eq!(state.lock().ultimo_seq, seq);
        let mut rota = cthulhu.clone();
        rota.tramos[0].numerador = 0;
        assert!(guardar_escala_resultado_en(Some(rota), &state).is_err());
        assert_eq!(state.lock().escala_resultado, Some(cthulhu.clone()));

        // Cada conjunto guarda la suya: al pasar a otro sin escala se pierde de vista y al volver se recupera
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ConjuntoActivado(2));
        assert_eq!(state.lock().escala_resultado, None);
        assert!(tirar_dado_de(&state, ID_HOST, "1d100 <= 100", true, None).unwrap().ends_with("· Éxito"));
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ConjuntoActivado(1));
        assert_eq!(state.lock().escala_resultado, Some(cthulhu.clone()));
        // Quitar la escala
        guardar_escala_resultado_en(None, &state).unwrap();
        assert_eq!(state.lock().escala_resultado, None);
    }

    #[test]
    fn la_escala_se_exporta_importa_valida_y_las_partidas_viejas_no_la_traen() {
        use crate::tablero_general::escalas::ejemplos;
        let mut c = conjunto_dnd();
        c.escala_resultado = ejemplos().into_iter().find(|e| e.clave == "dnd").map(|e| e.escala);
        let limpio = validar(c.clone()).unwrap();
        assert_eq!(de_archivo(&a_archivo(&limpio).unwrap()).unwrap(), limpio);
        // Una escala rota rechaza todo el conjunto
        let mut mala = c.clone();
        mala.escala_resultado.as_mut().unwrap().nombre_exito = String::new();
        assert!(validar(mala).is_err());
        // Un archivo o una partida de antes de la escala se leen igual (sin ella)
        let sin: ConjuntoParametros = serde_json::from_str(r#"{"nombre":"Viejo"}"#).unwrap();
        assert_eq!(sin.escala_resultado, None);
        let e: EstadoPartida = serde_json::from_str(&serde_json::to_string(&EstadoPartida::default()).unwrap().replace(",\"escala_resultado\":null", "")).unwrap();
        assert_eq!(e.escala_resultado, None);
        // La escala viva forma parte del contenido del conjunto activo
        let mut e = EstadoPartida::default();
        e.escala_resultado = c.escala_resultado.clone();
        assert_eq!(e.contenido_de(1).unwrap().escala_resultado, c.escala_resultado);
    }

    #[test]
    fn los_archivos_de_texto_por_defecto_son_de_cada_conjunto_y_las_fichas_nuevas_nacen_con_ellos() {
        use crate::tablero_general::archivos_defecto::guardar_archivos_texto_defecto_en;
        use crate::tablero_general::partida::{cancelar_borrador_entidad_en, crear_entidad_en, iniciar_borrador_entidad_en, procesar_delta};
        let state = estado_en_a("archivos_texto_defecto");
        state.lock().zonas = vec![Zona { id: 1, nombre: "Z".into(), orden: 0 }];
        let nuevo = |nombre: &str, contenido: &str| ArchivoTextoDefecto { id: 0, nombre: nombre.into(), contenido: contenido.into() };

        // Sin archivos por defecto la ficha nace sin adjuntos
        assert!(iniciar_borrador_entidad_en(1, None, &state).unwrap().archivos_adjuntos.is_empty());

        // Se guardan (ids asignados por Rust); guardar lo mismo no registra nada
        let lista = guardar_archivos_texto_defecto_en(vec![nuevo("Historia", "Érase una vez"), nuevo("Notas", "")], &state).unwrap();
        assert_eq!(lista.iter().map(|a| a.id).collect::<Vec<_>>(), vec![1, 2]);
        let seq = state.lock().ultimo_seq;
        guardar_archivos_texto_defecto_en(lista.clone(), &state).unwrap();
        assert_eq!(state.lock().ultimo_seq, seq);
        assert!(guardar_archivos_texto_defecto_en(vec![nuevo("a.b", "")], &state).is_err(), "una lista mala se rechaza entera");
        assert_eq!(state.lock().archivos_texto_defecto, lista);

        // La ficha nueva nace con los archivos, en orden y con su texto; el borrador cancelado se los lleva
        let f = iniciar_borrador_entidad_en(1, None, &state).unwrap();
        let rutas: Vec<String> = f.archivos_adjuntos.iter().flatten().cloned().collect();
        assert_eq!(rutas, vec![format!("archivos/entidad_{}/Historia.txt", f.id), format!("archivos/entidad_{}/Notas.txt", f.id)]);
        let dir = state.almacen.dir();
        assert_eq!(std::fs::read_to_string(dir.join(&rutas[0])).unwrap(), "Érase una vez");
        cancelar_borrador_entidad_en(f.id, vec![], &state);
        assert!(!dir.join(format!("archivos/entidad_{}", f.id)).exists());

        // Guardada, la ficha los conserva como adjuntos suyos
        let g = iniciar_borrador_entidad_en(1, None, &state).unwrap();
        let guardada = crear_entidad_en(g.clone(), &state).unwrap();
        assert_eq!(guardada.archivos_adjuntos.iter().flatten().count(), 2);
        assert!(dir.join(format!("archivos/entidad_{}/Historia.txt", g.id)).is_file());

        // Cada conjunto guarda los suyos: al pasar a otro sin archivos desaparecen de vista y al volver se recuperan
        procesar_delta(&state, DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        procesar_delta(&state, DeltaPartida::ConjuntoActivado(2));
        assert!(state.lock().archivos_texto_defecto.is_empty());
        procesar_delta(&state, DeltaPartida::ConjuntoActivado(1));
        assert_eq!(state.lock().archivos_texto_defecto, lista);
        assert_eq!(state.lock().contenido_de(1).unwrap().archivos_texto, lista);
    }

    #[test]
    fn los_archivos_de_texto_por_defecto_se_exportan_importan_y_se_validan() {
        let mut c = conjunto_dnd();
        c.archivos_texto = vec![ArchivoTextoDefecto { id: 4, nombre: "Historia".into(), contenido: "x".repeat(100_000) }];
        let limpio = validar(c.clone()).unwrap();
        assert_eq!(de_archivo(&a_archivo(&limpio).unwrap()).unwrap(), limpio, "la lista llena cabe en el archivo");
        // Una lista rota rechaza todo el conjunto
        let mut mala = c.clone();
        mala.archivos_texto[0].nombre = "con".into();
        assert!(validar(mala).is_err());
        let mut larga = c.clone();
        larga.archivos_texto[0].contenido.push('x');
        assert!(validar(larga).is_err());
        // Un archivo o una partida de antes de esto se leen igual, sin archivos
        let viejo: ConjuntoParametros = serde_json::from_str(r#"{"nombre":"Viejo"}"#).unwrap();
        assert!(viejo.archivos_texto.is_empty());
        let e: EstadoPartida = serde_json::from_str(&serde_json::to_string(&EstadoPartida::default()).unwrap().replace(",\"archivos_texto_defecto\":[]", "")).unwrap();
        assert!(e.archivos_texto_defecto.is_empty());
        // De los conjuntos de serie, solo D&D trae (los de su hoja de personaje)
        assert!(integrados().iter().all(|i| i.contenido.archivos_texto.is_empty() == (i.clave != "dnd5e")));
    }

    #[test]
    fn un_parametro_calculado_se_recalcula_al_guardar_la_ficha_y_al_borrar_su_variable() {
        use crate::tablero_general::partida::{crear_definicion_variable_en, crear_entidad_en, iniciar_borrador_entidad_en, modificar_entidad_en, procesar_delta};
        let state = estado_en_a("parametro_calculado");
        state.lock().zonas = vec![Zona { id: 1, nombre: "Z".into(), orden: 0 }];
        let con = crear_definicion_variable_en(var(0, "Constitución", "con", 0, 1), &state).unwrap();
        let tam = crear_definicion_variable_en(var(0, "Tamaño", "tam", 0, 1), &state).unwrap();
        // La vida pasa a ser (CON + TAM) / 10
        let mut vida = state.lock().definiciones_parametros[0].clone();
        vida.calculo = Some(CalculoParametro { variables: vec![con.id, tam.id], multiplicador: 1, divisor: 10, constante: 0 });
        procesar_delta(&state, DeltaPartida::ParametroModificado(vida));

        let mut f = iniciar_borrador_entidad_en(1, None, &state).unwrap();
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(0), "las variables de por defecto valen 0");
        f.variables[0].valor = 60;
        f.variables[1].valor = 55;
        f.parametros[0].valor = ValorParametro::Numero(99); // lo escrito a mano no cuenta
        let f = crear_entidad_en(f, &state).unwrap();
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(11));

        // Cambiar una variable al guardar mueve el parámetro, y el delta lleva los dos cambios
        let mut g = state.lock().banco_entidades[0].clone();
        g.variables[0].valor = 5;
        modificar_entidad_en(g, &state).unwrap();
        assert_eq!(state.lock().banco_entidades[0].parametros[0].valor, ValorParametro::Numero(6));

        // Si se borra una variable del cálculo, se quita de él; si se borran todas, el parámetro vuelve a ser manual
        procesar_delta(&state, DeltaPartida::VariableBorrada(con.id));
        assert_eq!(state.lock().definiciones_parametros[0].calculo.as_ref().unwrap().variables, vec![tam.id]);
        procesar_delta(&state, DeltaPartida::VariableBorrada(tam.id));
        assert!(state.lock().definiciones_parametros[0].calculo.is_none());
        assert_eq!(state.lock().banco_entidades[0].parametros[0].valor, ValorParametro::Numero(5), "se queda con el último valor (solo contaba Tamaño: 55 / 10)");
    }

    #[test]
    fn un_conjunto_con_calculos_y_rangos_se_exporta_importa_y_se_valida() {
        use crate::tablero_general::modelos::OpcionVariable;
        let mut c = conjunto_dnd();
        c.listas_rangos = vec![ListaRangos {
            id: 1,
            nombre: " Competencia ".into(),
            opciones: vec![OpcionVariable { nombre: "Sin entrenar".into(), bonus: 0 }, OpcionVariable { nombre: " Experto ".into(), bonus: 4 }],
        }];
        c.variables[2].rangos = Some(1);
        c.variables[2].constante = 2;
        c.parametros[0].calculo = Some(CalculoParametro { variables: vec![1], multiplicador: 1, divisor: 2, constante: 3 });
        let limpio = validar(c.clone()).unwrap();
        assert_eq!(limpio.listas_rangos[0].nombre, "Competencia");
        assert_eq!(limpio.variables[2].opciones[1].nombre, "Experto", "sin espacios de los lados, y la variable lleva la copia de su lista");
        let texto = a_archivo(&limpio).unwrap();
        assert_eq!(de_archivo(&texto).unwrap(), limpio);
        // Un cálculo que usa una variable que el conjunto no tiene se rechaza, con el parámetro por delante
        let mut malo = c.clone();
        malo.parametros[0].calculo = Some(CalculoParametro { variables: vec![77], multiplicador: 1, divisor: 1, constante: 0 });
        assert!(validar(malo).unwrap_err().starts_with("Vida:"));
        // Una base con fórmula, un rango repetido o una lista que no existe también
        let mut malo = c.clone();
        malo.variables[0].divisor = 2;
        assert!(validar(malo).is_err());
        let mut malo = c.clone();
        malo.listas_rangos[0].opciones.push(OpcionVariable { nombre: "experto".into(), bonus: 1 });
        assert!(validar(malo).is_err());
        let mut malo = c.clone();
        malo.variables[2].rangos = Some(9);
        assert!(validar(malo).unwrap_err().contains("lista de rangos"));
        let mut malo = c.clone();
        malo.listas_rangos.push(malo.listas_rangos[0].clone());
        assert!(validar(malo).is_err(), "id y nombre repetidos");
        // Un archivo de antes de las listas, con los rangos escritos en la variable, los pasa a una lista (no se pierde nada)
        let mut viejo = c;
        viejo.listas_rangos.clear();
        viejo.variables[2].rangos = None;
        viejo.variables[2].opciones = vec![OpcionVariable { nombre: "Sin entrenar".into(), bonus: 0 }, OpcionVariable { nombre: "Experto".into(), bonus: 4 }];
        let migrado = validar(viejo).unwrap();
        assert_eq!(migrado.listas_rangos.len(), 1);
        assert_eq!(migrado.variables[2].rangos, Some(migrado.listas_rangos[0].id));
        assert_eq!(migrado.variables[2].opciones.len(), 2);
    }

    /// Una plantilla de ficha mínima con el conjunto dentro (como la que exporta OpenRoll, más el personaje que se quiera).
    fn plantilla_con(c: &ConjuntoParametros) -> String {
        plantilla_con_ficha(c, serde_json::Value::Null)
    }
    fn plantilla_con_ficha(c: &ConjuntoParametros, ficha: serde_json::Value) -> String {
        let datos = serde_json::json!({
            "formato": "openroll-plantilla", "version": 1, "conjunto": c.nombre, "id_ficha": 7, "plantilla_id": "3f2a9c1e-7b64-4d2a-9a51-0c8e2d7f6b13",
            "definicion_conjunto": c, "ficha": ficha,
            "archivos": [{ "original": "Notas.txt", "nombre": "Notas.txt", "contenido": "NO ES UN TEXTO POR DEFECTO" }]
        });
        // `<` escrito como en las plantillas de verdad, y el id del script como lo busca el lector
        let json = serde_json::to_string(&datos).unwrap().replace('<', "\\u003c");
        format!("<!doctype html><html><head><!-- bloque «datos-openroll» --><script>var x = 1;</script><script type=\"application/json\" id=\"datos-openroll\">{json}</script></head></html>")
    }

    #[test]
    fn el_conjunto_se_lee_de_la_plantilla_de_verdad_aunque_el_texto_nombre_el_bloque_en_otros_sitios() {
        // El HTML real de la plantilla nombra «datos-openroll» en un comentario y en su JavaScript: solo cuenta el <script> con ese id
        let html = include_str!("../../../src/lib/plantilla/plantilla.html");
        assert!(html.matches("datos-openroll").count() >= 3);
        let c = conjunto_dnd();
        let datos = serde_json::json!({ "formato": "openroll-plantilla", "version": 1, "definicion_conjunto": c, "ficha": null });
        let json = serde_json::to_string(&datos).unwrap().replace('<', "\\u003c");
        let completa = html.replace("__DATOS_OPENROLL__", &json);
        assert_eq!(de_plantilla(&completa).unwrap(), validar(c).unwrap());
        // La plantilla sin datos (el marcador sin sustituir) no vale
        assert!(de_plantilla(html).is_err());
    }

    #[test]
    fn importar_una_plantilla_y_cambiar_de_conjunto_desde_los_comandos() {
        let state = estado_en_a("conjuntos_comandos");
        // Se edita el conjunto activo (vive en las definiciones) y se lleva en una plantilla
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ParametroBorrado(ID_ARMADURA));
        let activo = {
            let est = state.lock();
            est.contenido_de(est.conjunto_activo).unwrap()
        };
        assert_eq!(activo.parametros.len(), 3);
        let archivo = state.raiz.join("mio.html");
        std::fs::write(&archivo, plantilla_con(&activo)).unwrap();

        // Importarlo no toca lo que hay y el nombre repetido se renombra: Genérico → Genérico (2)
        let id = importar_conjunto_en(archivo.to_str().unwrap(), &state).unwrap();
        assert_eq!(id, 2);
        let lista = resumen(&state.lock());
        assert_eq!(lista.activo, 1);
        assert_eq!(lista.guardados.iter().map(|g| g.nombre.as_str()).collect::<Vec<_>>(), vec!["Genérico", "Genérico (2)"]);
        assert_eq!(lista.guardados[1].parametros, 3, "el exportado ya no tenía Armadura");
        assert_eq!(importar_conjunto_en(archivo.to_str().unwrap(), &state).unwrap(), 3);
        assert_eq!(state.lock().contenido_de(3).unwrap().nombre, "Genérico (3)");

        // Da igual que la plantilla traiga un personaje, su UUID o .txt cambiados: solo se lee el conjunto, y nunca los textos de la ficha
        let con_personaje = state.raiz.join("rellena.html");
        std::fs::write(&con_personaje, plantilla_con_ficha(&activo, serde_json::json!({ "nombre": "Orco", "tipo": "Enemigo", "parametros": [], "variables": [] }))).unwrap();
        let rellena = importar_conjunto_en(con_personaje.to_str().unwrap(), &state).unwrap();
        assert!(state.lock().contenido_de(rellena).unwrap().archivos_texto.is_empty(), "los .txt de la ficha no son los de por defecto");
        let tras = state.lock().conjuntos_parametros.len();

        // Lo que no es una plantilla, no lleva el conjunto, no existe o pesa demasiado: error y nada cambia
        let mala = state.raiz.join("mala.html");
        for (texto, motivo) in [
            ("esto no es una plantilla", "no es una plantilla"),
            ("<script id=\"datos-openroll\">{no es json</script>", "dañada"),
            ("<script id=\"datos-openroll\">{\"formato\":\"otra-cosa\",\"version\":1}</script>", "no es una plantilla"),
            ("<script id=\"datos-openroll\">{\"formato\":\"openroll-plantilla\",\"version\":99,\"definicion_conjunto\":{}}</script>", "más nueva"),
            ("<script id=\"datos-openroll\">{\"formato\":\"openroll-plantilla\",\"version\":1,\"definicion_conjunto\":null}</script>", "vuelve a exportarla"),
            ("<script id=\"datos-openroll\">{\"formato\":\"openroll-plantilla\",\"version\":1}</script>", "vuelve a exportarla"),
            ("<script id=\"datos-openroll\">{\"formato\":\"openroll-plantilla\",\"version\":1,\"definicion_conjunto\":{\"nombre\":5}}</script>", "mal formado"),
        ] {
            std::fs::write(&mala, texto).unwrap();
            let e = importar_conjunto_en(mala.to_str().unwrap(), &state).unwrap_err();
            assert!(e.contains(motivo), "{texto}: {e}");
        }
        // Un conjunto inválido (aunque esté bien formado) se rechaza entero con las reglas de siempre
        let mut roto = activo.clone();
        roto.nombre = String::new();
        std::fs::write(&mala, plantilla_con(&roto)).unwrap();
        assert!(importar_conjunto_en(mala.to_str().unwrap(), &state).is_err());
        let mut con_txt_malo = activo.clone();
        con_txt_malo.archivos_texto = vec![ArchivoTextoDefecto { id: 0, nombre: "a.b".into(), contenido: String::new() }];
        std::fs::write(&mala, plantilla_con(&con_txt_malo)).unwrap();
        assert!(importar_conjunto_en(mala.to_str().unwrap(), &state).is_err());
        let json = state.raiz.join("conjunto.json");
        std::fs::write(&json, "{}").unwrap();
        assert!(importar_conjunto_en(json.to_str().unwrap(), &state).is_err(), "ya no se importan archivos .json");
        std::fs::write(&mala, vec![b' '; 17 * 1024 * 1024]).unwrap();
        assert!(importar_conjunto_en(mala.to_str().unwrap(), &state).unwrap_err().contains("MB"));
        assert!(importar_conjunto_en(state.raiz.join("no_existe.html").to_str().unwrap(), &state).is_err());
        assert_eq!(state.lock().conjuntos_parametros.len(), tras);
        borrar_conjunto_en(rellena, &state).unwrap(); // el resto de la prueba cuenta con tres

        // De serie: se copia una sola vez y no se activa
        assert!(anadir_conjunto_de_serie_en("no_existe", &state).is_err());
        let serie = anadir_conjunto_de_serie_en("generico", &state).unwrap();
        assert_eq!(serie, 1, "ya había una copia del genérico (la del activo)");
        // Hay cuatro de serie y solo el genérico está en la partida
        let de_serie = resumen(&state.lock()).de_serie;
        assert_eq!(de_serie.iter().map(|s| (s.clave.as_str(), s.ya_anadido)).collect::<Vec<_>>(),
            vec![("dnd5e", false), ("pathfinder2e", false), ("cthulhu7e", false), ("generico", true)]);

        // Usar: no el que ya se usa ni uno que no existe, ni con una ficha nueva abierta; y entonces las fichas se reinician
        assert!(usar_conjunto_en(1, &state).is_err() && usar_conjunto_en(99, &state).is_err());
        state.reservas().insert(7);
        assert!(usar_conjunto_en(2, &state).unwrap_err().contains("Cierra la ficha"));
        state.reservas().clear();
        usar_conjunto_en(2, &state).unwrap();
        assert_eq!(state.lock().conjunto_activo, 2);
        assert_eq!(state.lock().definiciones_parametros.len(), 3);
        // El activo no se borra; otro sí
        assert!(borrar_conjunto_en(2, &state).unwrap_err().contains("estás usando"));
        borrar_conjunto_en(3, &state).unwrap();
        assert_eq!(state.lock().conjuntos_parametros.len(), 2);
        // Renombrar: sin choques de nombre
        assert!(renombrar_conjunto_en(2, "genérico".into(), &state).is_err(), "ya lo tiene el 1 (sin mayúsculas)");
        renombrar_conjunto_en(2, "  Sin   armadura ".into(), &state).unwrap();
        assert_eq!(state.lock().contenido_de(2).unwrap().nombre, "Sin armadura");
        assert!(renombrar_conjunto_en(2, "".into(), &state).is_err());
    }

    #[test]
    fn la_biblioteca_comparte_conjuntos_entre_partidas() {
        let state = estado_en_a("conjuntos_biblioteca");
        assert!(listar_biblioteca_en(&state.raiz).is_empty());
        let archivo = guardar_en_biblioteca_en(1, &state).unwrap();
        assert_eq!(archivo, "Genérico.json");
        assert!(guardar_en_biblioteca_en(99, &state).is_err());
        // Guardar otra vez el mismo nombre lo actualiza, no lo duplica
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ParametroBorrado(ID_ARMADURA));
        assert_eq!(guardar_en_biblioteca_en(1, &state).unwrap(), archivo);
        let lista = listar_biblioteca_en(&state.raiz);
        assert_eq!((lista.len(), lista[0].parametros), (1, 3));
        // Se añade a la partida (copia, sin usarla) y el nombre repetido se renombra
        let id = anadir_de_biblioteca_en(&archivo, &state).unwrap();
        assert_eq!((id, state.lock().conjunto_activo), (2, 1));
        assert_eq!(state.lock().contenido_de(2).unwrap().nombre, "Genérico (2)");
        // Nada de salirse de la carpeta ni de archivos que no existen
        for malo in ["../red.json", "..\\x.json", "x.txt", "C:\\x.json", "no_existe.json"] {
            assert!(anadir_de_biblioteca_en(malo, &state).is_err(), "{malo}");
            assert!(borrar_de_biblioteca_en(malo, &state).is_err(), "{malo}");
        }
        // Los archivos rotos no estorban en la lista
        std::fs::write(dir_biblioteca(&state.raiz).join("roto.json"), "hola").unwrap();
        assert_eq!(listar_biblioteca_en(&state.raiz).len(), 1);
        // Borrar de la biblioteca no toca la partida
        borrar_de_biblioteca_en(&archivo, &state).unwrap();
        assert!(listar_biblioteca_en(&state.raiz).is_empty());
        assert_eq!(state.lock().conjuntos_parametros.len(), 2);
        assert_eq!(nombre_de_archivo("a/b:c?"), "a_b_c_");
        assert_eq!(nombre_de_archivo("  ..  "), "conjunto");
    }

    #[test]
    fn el_limite_de_conjuntos_por_partida() {
        let state = estado_en_a("conjuntos_limite");
        for n in 0..MAX_CONJUNTOS - 1 {
            let c = ConjuntoParametros { nombre: format!("C{n}"), ..conjunto_dnd() };
            anadir(&mut state.lock(), &state, None, c).unwrap();
        }
        assert_eq!(state.lock().conjuntos_parametros.len(), MAX_CONJUNTOS);
        let c = ConjuntoParametros { nombre: "Uno más".into(), ..conjunto_dnd() };
        assert!(anadir(&mut state.lock(), &state, None, c).unwrap_err().contains("no puede guardar más"));
    }

    #[test]
    fn un_conjunto_activado_se_reconstruye_igual_al_reproducir_el_log() {
        // El log lleva el contenido entero: no depende de lo que traiga el código de serie
        let mut e = EstadoPartida::default();
        let deltas = [
            DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }),
            DeltaPartida::ConjuntoActivado(2),
            DeltaPartida::VariableCreada(var(9, "Extra", "ex", 10, 2)),
            DeltaPartida::ConjuntoActivado(1),
        ];
        for d in &deltas {
            e.aplicar_delta(d);
        }
        let json = serde_json::to_string(&e).unwrap();
        let leido: EstadoPartida = serde_json::from_str(&json).unwrap();
        assert_eq!(leido.conjunto_activo, 1);
        assert_eq!(leido.contenido_de(2).unwrap().variables.len(), 4, "lo editado en el 2 se guardó al dejarlo");
        assert_eq!(leido.contenido_de(1), e.contenido_de(1));
    }

    #[test]
    fn borrar_y_renombrar_conjuntos_y_nunca_el_activo() {
        let mut e = EstadoPartida::default();
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd(), valores: Default::default() }));
        e.aplicar_delta(&DeltaPartida::ConjuntoRenombrado { id: 2, nombre: "Otro".into() });
        assert_eq!(e.contenido_de(2).unwrap().nombre, "Otro");
        e.aplicar_delta(&DeltaPartida::ConjuntoRenombrado { id: 1, nombre: "Mío".into() });
        assert_eq!(e.contenido_de(1).unwrap().nombre, "Mío", "también el activo");
        e.aplicar_delta(&DeltaPartida::ConjuntoBorrado(1)); // el activo no se borra
        assert_eq!(e.conjuntos_parametros.len(), 2);
        e.aplicar_delta(&DeltaPartida::ConjuntoBorrado(2));
        assert_eq!(e.conjuntos_parametros.len(), 1);
    }
}
