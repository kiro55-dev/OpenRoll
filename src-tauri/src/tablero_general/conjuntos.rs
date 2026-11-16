// src-tauri/src/tablero_general/conjuntos.rs
//
// Conjuntos de parámetros: lo que define cómo es el juego al que se juega (parámetros visuales, variables y sus grupos). Una
// partida guarda varios (`EstadoPartida.conjuntos_parametros`) y usa uno (`conjunto_activo`); las definiciones «vivas» del
// estado (`definiciones_parametros`, `grupos_variables`, `definiciones_variables`) son el contenido del ACTIVO, así que todo lo
// que ya funciona (editar parámetros, variables, red) no cambia.
//
//  * De serie (`integrados`): definidos en código, independientes de la partida. Elegir uno COPIA su contenido a la partida, que
//    lo edita sin tocar el original; el archivo de la partida nunca depende de este código al reproducir el log.
//  * Exportar / importar: un archivo JSON (`Archivo`) con un conjunto. Lo que se importa es entrada no confiable: se valida entero
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
pub const MAX_BYTES_ARCHIVO: u64 = 1024 * 1024;
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
    }
}

/// Los conjuntos que trae el programa, definidos en código. Para añadir uno: otra entrada aquí (y pasa solo por el test que
/// comprueba que toda entrada es válida).
pub fn integrados() -> Vec<Integrado> {
    vec![Integrado { clave: "generico", contenido: generico() }]
}

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
        }
    }

    /// El contenido del conjunto `id`: del activo, lo que hay AHORA (lo editado incluido); de los demás, lo guardado.
    pub fn contenido_de(&self, id: u16) -> Option<ConjuntoParametros> {
        if id == self.conjunto_activo && self.conjuntos_parametros.iter().any(|c| c.id == id) {
            return Some(self.contenido_vivo());
        }
        self.conjuntos_parametros.iter().find(|c| c.id == id).map(|c| c.contenido.clone())
    }

    /// Cambia al conjunto `id`: el que se deja guarda lo editado, se cargan las definiciones del elegido y las fichas empiezan
    /// de cero (los ids se repiten entre conjuntos: sin vaciarlas, un parámetro con el mismo id heredaría el valor del otro).
    pub(crate) fn activar_conjunto(&mut self, id: u16) {
        if id == self.conjunto_activo || !self.conjuntos_parametros.iter().any(|c| c.id == id) {
            return;
        }
        let vivo = self.contenido_vivo();
        if let Some(c) = self.conjuntos_parametros.iter_mut().find(|c| c.id == self.conjunto_activo) {
            c.contenido = vivo;
        }
        let nuevo = self.conjuntos_parametros.iter().find(|c| c.id == id).map(|c| c.contenido.clone()).expect("existe");
        self.definiciones_parametros = nuevo.parametros;
        self.grupos_variables = nuevo.grupos_variables;
        self.definiciones_variables = nuevo.variables;
        self.conjunto_activo = id;
        self.reiniciar_valores_de_fichas();
        self.ajustar_a_definiciones();
    }

    /// Vacía todo lo que las fichas guardan de las definiciones (valores de parámetros y variables, variación de − y +, vida
    /// actual de las instancias): el banco, las instancias y lo que hay en la papelera. `ajustar_a_definiciones` lo rellena
    /// con los valores por defecto del conjunto nuevo.
    pub(crate) fn reiniciar_valores_de_fichas(&mut self) {
        let vaciar = |e: &mut Entidad| {
            e.parametros.clear();
            e.variables.clear();
            e.pasos_valor.clear();
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
    };
    let g = generico();
    let es_generico = vivo.parametros == g.parametros && vivo.variables.is_empty() && vivo.grupos_variables == g.grupos_variables;
    let contenido = if es_generico {
        g
    } else {
        ConjuntoParametros { nombre: "Personalizado".into(), descripcion: "Los parámetros y variables que tenía esta partida.".into(), ..vivo }
    };
    est.conjuntos_parametros = vec![ConjuntoGuardado { id: 1, origen: es_generico.then(|| "generico".to_string()), contenido }];
    est.conjunto_activo = 1;
}

/// El conjunto con el que nace una partida nueva.
pub fn conjuntos_iniciales() -> Vec<ConjuntoGuardado> {
    vec![ConjuntoGuardado { id: 1, origen: Some("generico".into()), contenido: generico() }]
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
    if c.variables.len() > MAX_VARIABLES {
        return Err(format!("No puede haber más de {MAX_VARIABLES} variables"));
    }
    let mut aceptadas: Vec<DefinicionVariable> = Vec::new();
    for derivadas in [false, true] {
        for v in c.variables.iter().filter(|v| variables::es_derivada(v) == derivadas) {
            if c.variables.iter().filter(|o| o.id == v.id).count() > 1 {
                return Err(format!("La variable «{}» repite el id {}", v.nombre, v.id));
            }
            let v = DefinicionVariable {
                nombre: v.nombre.split_whitespace().collect::<Vec<_>>().join(" "),
                abreviatura: v.abreviatura.trim().to_string(),
                ..v.clone()
            };
            variables::validar_definicion(&aceptadas, &grupos, &v)?;
            aceptadas.push(v);
        }
    }
    // Se conserva el orden del archivo (es el que siguen las fichas)
    let variables_validadas: Vec<DefinicionVariable> =
        c.variables.iter().filter_map(|v| aceptadas.iter().find(|a| a.id == v.id).cloned()).collect();

    Ok(ConjuntoParametros { nombre: c.nombre, descripcion: c.descripcion, parametros, grupos_variables: grupos, variables: variables_validadas })
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
    let guardado = ConjuntoGuardado { id: id_libre(est), origen, contenido };
    registrar(est, &state.almacen, DeltaPartida::ConjuntoAnadido(guardado.clone()));
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
    validar(ConjuntoParametros { nombre: nombre.clone(), descripcion: String::new(), parametros: vec![], grupos_variables: vec![], variables: vec![] })?;
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

/// Escribe el conjunto `id` en `ruta` (la que eligió el host en el diálogo «Guardar como»), de forma atómica.
#[tauri::command]
pub fn exportar_conjunto(id: u16, ruta: String, state: State<'_, AppState>) -> Result<(), String> {
    exportar_conjunto_en(id, &ruta, &state)
}

pub fn exportar_conjunto_en(id: u16, ruta: &str, state: &AppState) -> Result<(), String> {
    let texto = {
        let est = state.lock();
        a_archivo(&est.contenido_de(id).ok_or("Ese conjunto ya no existe")?)?
    };
    let destino = Path::new(ruta);
    if !destino.is_absolute() {
        return Err("La ruta de destino no es válida".into());
    }
    let destino = if destino.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) { destino.to_path_buf() } else { destino.with_extension("json") };
    let tmp = destino.with_extension("json.tmp");
    std::fs::write(&tmp, texto).map_err(|e| format!("No se pudo escribir el archivo: {e}"))?;
    std::fs::rename(&tmp, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("No se pudo escribir el archivo: {e}")
    })
}

/// Lee el archivo, lo valida entero y lo añade a la partida (sin activarlo). Devuelve el id del conjunto nuevo.
#[tauri::command]
pub fn importar_conjunto(ruta: String, state: State<'_, AppState>) -> Result<u16, String> {
    importar_conjunto_en(&ruta, &state)
}

pub fn importar_conjunto_en(ruta: &str, state: &AppState) -> Result<u16, String> {
    let origen = Path::new(ruta);
    let tam = std::fs::metadata(origen).map_err(|e| format!("No se pudo abrir el archivo: {e}"))?.len();
    if tam > MAX_BYTES_ARCHIVO {
        return Err(format!("El archivo es demasiado grande para ser un conjunto de parámetros (máx. {} KiB)", MAX_BYTES_ARCHIVO / 1024));
    }
    let texto = std::fs::read_to_string(origen).map_err(|_| "El archivo no es un conjunto de parámetros de OpenRoll".to_string())?;
    let contenido = de_archivo(&texto)?;
    let mut est = state.lock();
    Ok(anadir(&mut est, state, None, contenido)?.id)
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
            archivos_adjuntos: vec![], es_unico: false, propietarios: vec![],
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
    fn cambiar_de_conjunto_guarda_lo_editado_reinicia_las_fichas_y_se_puede_volver() {
        let mut e = EstadoPartida::default();
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd() }));
        // Una ficha con valores del conjunto 1, una instancia con la vida tocada y un paso de − y +
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

        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(2));
        assert_eq!(e.conjunto_activo, 2);
        assert_eq!(e.definiciones_variables.len(), 3);
        // Los valores empiezan de cero con los del conjunto nuevo (aunque el id 1 existía en los dos)
        let f = &e.banco_entidades[0];
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(10), "vida del conjunto nuevo, no 77");
        assert!(f.pasos_valor.is_empty());
        assert_eq!(f.variables.len(), 3);
        assert_eq!(e.instancias_activas[0].valores_actuales[0].valor, 10);
        assert_eq!(e.instancias_activas[0].base.variables.len(), 3);

        // Volver al 1: tiene lo que se editó (sin Armadura) y las fichas vuelven a empezar
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(1));
        assert_eq!(e.definiciones_parametros.len(), 3);
        assert!(e.definiciones_variables.is_empty());
        assert_eq!(e.banco_entidades[0].parametros.len(), 3);
        // Activar el que ya está, o uno que no existe, no hace nada
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(1));
        e.aplicar_delta(&DeltaPartida::ConjuntoActivado(99));
        assert_eq!(e.conjunto_activo, 1);
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
    fn exportar_e_importar_por_archivo_y_cambiar_de_conjunto_desde_los_comandos() {
        let state = estado_en_a("conjuntos_comandos");
        // Se edita el conjunto activo (vive en las definiciones) y se exporta
        crate::tablero_general::partida::procesar_delta(&state, DeltaPartida::ParametroBorrado(ID_ARMADURA));
        let archivo = state.raiz.join("mio");
        exportar_conjunto_en(1, archivo.to_str().unwrap(), &state).unwrap();
        let archivo = state.raiz.join("mio.json"); // sin extensión se le pone .json
        assert!(archivo.is_file() && !state.raiz.join("mio.json.tmp").exists());
        assert!(exportar_conjunto_en(99, archivo.to_str().unwrap(), &state).is_err());
        assert!(exportar_conjunto_en(1, "relativa.json", &state).is_err(), "solo rutas absolutas");

        // Importarlo no toca lo que hay y el nombre repetido se renombra: Genérico → Genérico (2)
        let id = importar_conjunto_en(archivo.to_str().unwrap(), &state).unwrap();
        assert_eq!(id, 2);
        let lista = resumen(&state.lock());
        assert_eq!(lista.activo, 1);
        assert_eq!(lista.guardados.iter().map(|g| g.nombre.as_str()).collect::<Vec<_>>(), vec!["Genérico", "Genérico (2)"]);
        assert_eq!(lista.guardados[1].parametros, 3, "el exportado ya no tenía Armadura");
        assert_eq!(importar_conjunto_en(archivo.to_str().unwrap(), &state).unwrap(), 3);
        assert_eq!(state.lock().contenido_de(3).unwrap().nombre, "Genérico (3)");

        // Lo que no es un conjunto, no existe o pesa demasiado: error y nada cambia
        let basura = state.raiz.join("basura.json");
        std::fs::write(&basura, "esto no es json").unwrap();
        assert!(importar_conjunto_en(basura.to_str().unwrap(), &state).unwrap_err().contains("no es un conjunto"));
        std::fs::write(&basura, vec![b' '; MAX_BYTES_ARCHIVO as usize + 1]).unwrap();
        assert!(importar_conjunto_en(basura.to_str().unwrap(), &state).unwrap_err().contains("demasiado grande"));
        assert!(importar_conjunto_en(state.raiz.join("no_existe.json").to_str().unwrap(), &state).is_err());
        assert_eq!(state.lock().conjuntos_parametros.len(), 3);

        // De serie: se copia una sola vez y no se activa
        assert!(anadir_conjunto_de_serie_en("no_existe", &state).is_err());
        let serie = anadir_conjunto_de_serie_en("generico", &state).unwrap();
        assert_eq!(serie, 1, "ya había una copia del genérico (la del activo)");
        assert_eq!(resumen(&state.lock()).de_serie.iter().map(|s| s.ya_anadido).collect::<Vec<_>>(), vec![true]);

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
            DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd() }),
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
        e.aplicar_delta(&DeltaPartida::ConjuntoAnadido(ConjuntoGuardado { id: 2, origen: None, contenido: conjunto_dnd() }));
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
