// src-tauri/src/tablero_general/variables.rs
//
// Las variables de las fichas (Fuerza «f», Acrobacias «acro»...), repartidas en GRUPOS (`GrupoVariables`) de dos tipos:
//  * Base: el host define nombre, abreviatura, valor neutro y escala, y cada ficha pone su valor (`ValorVariable`).
//  * Derivadas: no tienen valor propio; suman el resultado de una o más variables base (`bases`) y, si la ficha las marca,
//    el de sus `opcionales` (la competencia en las habilidades de D&D).
// Lo que cuenta en una tirada del chat (`2d6 + f`) es el RESULTADO (`resultado`): en una base,
// `floor((valor − neutro) / escala)`; en una derivada, la suma. Se calcula aquí y nunca se guarda.
//
// Nombre y abreviatura son solo letras (y espacios dentro del nombre): así se distinguen de los dados y
// números en el chat, y nada de lo que escribe el host puede meter HTML en el desglose de una tirada.
// Mismo espejo en el front: src/lib/variables.ts.
use crate::tablero_general::modelos::{
    CalculoParametro, DefinicionParametro, DefinicionVariable, Entidad, GrupoVariables, OrigenEntrada, Parametro, TipoGrupoVariables,
    ValorParametro, ValorVariable,
};

pub const MAX_VARIABLES: usize = 80;
pub const MAX_NOMBRE_VARIABLE: usize = 30;
pub const MAX_ABREVIATURA_VARIABLE: usize = 10;
pub const MAX_ESCALA_VARIABLE: i16 = 100;
/// Tope del valor de una variable en una ficha y de su valor neutro (en positivo y en negativo).
pub const LIMITE_VALOR_VARIABLE: i16 = 9999;
/// Palabras del chat que no pueden ser variables: limpiar el chat, el dado, la ventaja/desventaja, «vs» (tirada bajo objetivo) y las
/// órdenes de batalla «batalla» y «terminar».
pub const PALABRAS_RESERVADAS: [&str; 9] = ["c", "clean", "d", "adv", "desv", "dis", "vs", "batalla", "terminar"];
/// Tope del multiplicador y del divisor de una fórmula.
pub const MAX_FACTOR_FORMULA: i16 = 100;
/// Tope de opciones (rangos) de una derivada y de la longitud del nombre de una.
pub const MAX_OPCIONES_VARIABLE: usize = 12;
pub const MAX_NOMBRE_OPCION: usize = 30;
/// Tope del valor de un parámetro calculado (los parámetros no son negativos).
pub const LIMITE_PARAMETRO_CALCULADO: i32 = 9999;
pub const MAX_GRUPOS_VARIABLES: usize = 20;
pub const MAX_NOMBRE_GRUPO: usize = 30;
/// Tope de bases (y de opcionales) de una derivada.
pub const MAX_BASES_DERIVADA: usize = 10;

/// El resultado de una variable BASE con ese valor: cuántas veces cabe la escala en lo que se aparta del neutro,
/// redondeado hacia abajo (como en D&D: 12 → +1, 9 → −1 con neutro 10 y escala 2).
pub fn modificador(def: &DefinicionVariable, valor: i16) -> i32 {
    (i32::from(valor) - i32::from(def.neutro)).div_euclid(i32::from(def.escala.max(1)))
}

/// ¿Es una derivada? (las que tienen bases o parámetros fijos; una derivada siempre tiene al menos una fuente de esas, y una
/// base ninguna).
pub fn es_derivada(def: &DefinicionVariable) -> bool {
    !def.bases.is_empty() || !def.parametros.is_empty()
}

/// Lo que aporta un parámetro a una derivada: su valor tal cual si es un número (0 si falta o es texto).
pub fn valor_de_parametro(params: &[Parametro], id: u16) -> i32 {
    match params.iter().find(|p| p.id_definicion == id).map(|p| &p.valor) {
        Some(ValorParametro::Numero(n)) => i32::from(*n),
        _ => 0,
    }
}

/// ¿Puede un parámetro ser fuente de una derivada? Solo un número escrito a mano (uno calculado depende de las variables).
pub fn parametro_como_fuente(p: &DefinicionParametro) -> bool {
    p.calculo.is_none() && matches!(p.valor_por_defecto, ValorParametro::Numero(_))
}

/// `floor(suma · multiplicador / divisor) + constante`, redondeando hacia abajo también en negativo. Un divisor menor que 1
/// cuenta como 1.
pub fn formula(suma: i32, multiplicador: i16, divisor: i16, constante: i16) -> i32 {
    (suma * i32::from(multiplicador)).div_euclid(i32::from(divisor.max(1))) + i32::from(constante)
}

/// Lo que vale una variable en una ficha (sus `valores`): una base, su `modificador`; una derivada, la suma de los de sus
/// bases y de sus opcionales que la ficha tiene marcadas, pasada por su fórmula (multiplicador, divisor y constante) más el
/// bono de la opción que la ficha ha elegido. Un valor que falte cuenta como el neutro (0).
pub fn resultado(defs: &[DefinicionVariable], params: &[Parametro], valores: &[ValorVariable], def: &DefinicionVariable) -> i32 {
    let valor_de = |id: u16| valores.iter().find(|v| v.id_definicion == id);
    let de_base = |id: u16| {
        defs.iter()
            .find(|d| d.id == id && !es_derivada(d))
            .map_or(0, |b| modificador(b, valor_de(id).map_or(b.neutro, |v| v.valor)))
    };
    if !es_derivada(def) {
        return de_base(def.id);
    }
    let marcadas: &[u16] = valor_de(def.id).map_or(&[], |v| v.opcionales.as_slice());
    let marcados: &[u16] = valor_de(def.id).map_or(&[], |v| v.parametros_marcados.as_slice());
    // La fórmula individual de una entrada (si la tiene) se aplica a lo que aporta ANTES de sumarlas todas
    let de_variable = |id: u16| aplicar_a_entrada(def, OrigenEntrada::Variable(id), de_base(id));
    let de_param = |id: u16| aplicar_a_entrada(def, OrigenEntrada::Parametro(id), valor_de_parametro(params, id));
    let suma = def.bases.iter().map(|&b| de_variable(b)).sum::<i32>()
        + def.opcionales.iter().filter(|o| marcadas.contains(o)).map(|&o| de_variable(o)).sum::<i32>()
        + def.parametros.iter().map(|&p| de_param(p)).sum::<i32>()
        + def.parametros_opcionales.iter().filter(|p| marcados.contains(p)).map(|&p| de_param(p)).sum::<i32>();
    let bono = bono_elegido(def, valor_de(def.id));
    formula(suma, def.multiplicador, def.divisor, def.constante) + bono
}

/// Lo que aporta una entrada de la derivada tras su fórmula individual (sin ella, `valor` tal cual).
pub fn aplicar_a_entrada(def: &DefinicionVariable, origen: OrigenEntrada, valor: i32) -> i32 {
    match def.formulas_entradas.iter().find(|f| f.origen == origen) {
        Some(f) => formula(valor, f.multiplicador, f.divisor, f.constante),
        None => valor,
    }
}

/// El bono de la opción que la ficha ha elegido en esta derivada (0 si no tiene opciones).
pub fn bono_elegido(def: &DefinicionVariable, valor: Option<&ValorVariable>) -> i32 {
    let i = valor.map_or(0, |v| usize::from(v.eleccion));
    def.opciones.get(i).or_else(|| def.opciones.first()).map_or(0, |o| i32::from(o.bonus))
}

/// Lo que cuenta de una variable al calcular un parámetro: en una base, el valor escrito; en una derivada, su resultado.
pub fn valor_para_calculo(defs: &[DefinicionVariable], params: &[Parametro], valores: &[ValorVariable], id: u16) -> i32 {
    match defs.iter().find(|d| d.id == id) {
        None => 0,
        Some(d) if es_derivada(d) => resultado(defs, params, valores, d),
        Some(d) => i32::from(valores.iter().find(|v| v.id_definicion == id).map_or(d.neutro, |v| v.valor)),
    }
}

/// El valor de un parámetro calculado en una ficha con esas variables: `floor(suma · multiplicador / divisor) + constante`,
/// dentro de 0 y `LIMITE_PARAMETRO_CALCULADO` (un parámetro no es negativo).
pub fn calcular_parametro(calculo: &CalculoParametro, defs: &[DefinicionVariable], params: &[Parametro], valores: &[ValorVariable]) -> i16 {
    let suma: i32 = calculo.variables.iter().map(|&v| valor_para_calculo(defs, params, valores, v)).sum();
    let v = formula(suma, calculo.multiplicador, calculo.divisor, calculo.constante);
    v.clamp(0, LIMITE_PARAMETRO_CALCULADO) as i16
}

/// Pone en la ficha el valor de cada parámetro calculado. Hay que llamarla tras normalizar parámetros y variables, y
/// siempre que cambien las variables de la ficha.
pub fn recalcular_parametros(defs: &[DefinicionParametro], vars: &[DefinicionVariable], e: &mut Entidad) {
    for d in defs {
        let Some(c) = &d.calculo else { continue };
        // Un parámetro calculado nunca es fuente de una derivada: los que se leen aquí no cambian al ir recalculando
        let nuevo = calcular_parametro(c, vars, &e.parametros, &e.variables);
        if let Some(p) = e.parametros.iter_mut().find(|p| p.id_definicion == d.id) {
            p.valor = ValorParametro::Numero(nuevo);
        }
    }
}

/// Quita de los cálculos las variables que ya no existen (y el cálculo entero si no le queda ninguna): el parámetro
/// se queda con el último valor y pasa a escribirse a mano.
pub fn limpiar_calculos(defs: &mut [DefinicionParametro], vars: &[DefinicionVariable]) {
    for d in defs.iter_mut() {
        if let Some(c) = &mut d.calculo {
            c.variables.retain(|v| vars.iter().any(|x| x.id == *v));
            if c.variables.is_empty() {
                d.calculo = None;
            }
        }
    }
}

/// Comprueba un cálculo de parámetro contra las variables que existen.
pub fn validar_calculo(c: &CalculoParametro, vars: &[DefinicionVariable]) -> Result<(), String> {
    if c.variables.is_empty() || c.variables.len() > MAX_BASES_DERIVADA {
        return Err(format!("Un parámetro calculado usa de 1 a {MAX_BASES_DERIVADA} variables"));
    }
    if (1..c.variables.len()).any(|i| c.variables[..i].contains(&c.variables[i])) {
        return Err("Una variable no puede repetirse en el cálculo".into());
    }
    if !c.variables.iter().all(|v| vars.iter().any(|x| x.id == *v)) {
        return Err("El cálculo usa una variable que no existe".into());
    }
    validar_formula(c.multiplicador, c.divisor, c.constante)
}

/// Si el parámetro es calculado, comprueba que sea un número y que su cálculo valga contra las variables que existen.
pub fn validar_parametro_calculado(d: &DefinicionParametro, vars: &[DefinicionVariable]) -> Result<(), String> {
    let Some(c) = &d.calculo else { return Ok(()) };
    if !matches!(d.valor_por_defecto, ValorParametro::Numero(_)) {
        return Err("Solo un número puede calcularse con variables".into());
    }
    validar_calculo(c, vars)
}

fn validar_formula(multiplicador: i16, divisor: i16, constante: i16) -> Result<(), String> {
    if !(1..=MAX_FACTOR_FORMULA).contains(&multiplicador) || !(1..=MAX_FACTOR_FORMULA).contains(&divisor) {
        return Err(format!("El multiplicador y el divisor deben estar entre 1 y {MAX_FACTOR_FORMULA}"));
    }
    if constante.abs() > LIMITE_VALOR_VARIABLE {
        return Err(format!("La constante debe estar entre -{LIMITE_VALOR_VARIABLE} y {LIMITE_VALOR_VARIABLE}"));
    }
    Ok(())
}

/// Deja una variable por definición y en su orden: las que faltan nacen con el valor neutro; las de
/// definiciones que ya no existen (o repetidas) se quitan. En una derivada solo quedan marcadas las opcionales que lo
/// siguen siendo; en una base no hay ninguna.
pub fn normalizar_variables(defs: &[DefinicionVariable], valores: &mut Vec<ValorVariable>) {
    let mut anteriores = std::mem::take(valores);
    for d in defs {
        let mut v = match anteriores.iter().position(|v| v.id_definicion == d.id) {
            Some(i) => anteriores.swap_remove(i),
            None => ValorVariable { id_definicion: d.id, valor: d.neutro, opcionales: vec![], eleccion: 0, parametros_marcados: vec![] },
        };
        // La opción elegida existe, o se vuelve a la primera (y sin opciones no hay elección)
        if usize::from(v.eleccion) >= d.opciones.len() {
            v.eleccion = 0;
        }
        let mut marcadas = Vec::new();
        for o in v.opcionales.iter().filter(|o| d.opcionales.contains(o)) {
            if !marcadas.contains(o) {
                marcadas.push(*o);
            }
        }
        v.opcionales = marcadas;
        let mut marcados = Vec::new();
        for p in v.parametros_marcados.iter().filter(|p| d.parametros_opcionales.contains(p)) {
            if !marcados.contains(p) {
                marcados.push(*p);
            }
        }
        v.parametros_marcados = marcados;
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

/// El tipo del grupo `id` (`None` si no existe).
pub fn tipo_de_grupo(grupos: &[GrupoVariables], id: u16) -> Option<TipoGrupoVariables> {
    grupos.iter().find(|g| g.id == id).map(|g| g.tipo)
}

/// Comprueba una definición contra las DEMÁS que existen (al crear: todas; al modificar: todas menos ella) y los grupos.
pub fn validar_definicion(
    otras: &[DefinicionVariable], grupos: &[GrupoVariables], params: &[DefinicionParametro], d: &DefinicionVariable,
) -> Result<(), String> {
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
    // Únicas entre TODAS las variables (de cualquier grupo): el chat las busca por nombre o abreviatura
    for texto in [nombre, abreviatura].into_iter().filter(|t| !t.is_empty()) {
        if PALABRAS_RESERVADAS.contains(&clave(texto).as_str()) {
            return Err(format!("«{texto}» está reservado en el chat de dados"));
        }
        if let Some(o) = otras.iter().find(|o| clave(&o.nombre) == clave(texto) || clave(&o.abreviatura) == clave(texto)) {
            // Se dice contra cuál choca: así un conflicto real se reconoce (y uno que no lo parezca, se nota)
            let grupo = grupos.iter().find(|g| g.id == o.grupo).map_or("sin grupo", |g| g.nombre.as_str());
            return Err(format!("«{texto}» ya lo usa la variable «{}» (grupo «{grupo}»)", o.nombre));
        }
    }
    if otras.len() >= MAX_VARIABLES {
        return Err(format!("No puede haber más de {MAX_VARIABLES} variables"));
    }
    match tipo_de_grupo(grupos, d.grupo).ok_or("Ese grupo de variables ya no existe")? {
        TipoGrupoVariables::Base => {
            if !d.bases.is_empty() || !d.opcionales.is_empty() || !d.parametros.is_empty() || !d.parametros_opcionales.is_empty() {
                return Err("Una variable base no depende de otras".into());
            }
            if (d.multiplicador, d.divisor, d.constante) != (1, 1, 0) || d.rangos.is_some() || !d.opciones.is_empty() || !d.formulas_entradas.is_empty() {
                return Err("Una variable base no lleva fórmula ni rangos".into());
            }
            if !(1..=MAX_ESCALA_VARIABLE).contains(&d.escala) {
                return Err(format!("La escala debe estar entre 1 y {MAX_ESCALA_VARIABLE}"));
            }
            if d.neutro.abs() > LIMITE_VALOR_VARIABLE {
                return Err(format!("El valor neutro debe estar entre -{LIMITE_VALOR_VARIABLE} y {LIMITE_VALOR_VARIABLE}"));
            }
        }
        TipoGrupoVariables::Derivadas => {
            if d.bases.is_empty() && d.parametros.is_empty() {
                return Err("Una variable derivada depende de al menos una variable base o un parámetro".into());
            }
            if d.bases.len() > MAX_BASES_DERIVADA
                || d.opcionales.len() > MAX_BASES_DERIVADA
                || d.parametros.len() > MAX_BASES_DERIVADA
                || d.parametros_opcionales.len() > MAX_BASES_DERIVADA
            {
                return Err(format!("Una derivada no puede tener más de {MAX_BASES_DERIVADA} bases, parámetros ni opcionales de cada"));
            }
            let ps: Vec<u16> = d.parametros.iter().chain(&d.parametros_opcionales).copied().collect();
            if (1..ps.len()).any(|i| ps[..i].contains(&ps[i])) {
                return Err("Un parámetro no puede repetirse entre los fijos y los opcionales".into());
            }
            for id in &ps {
                match params.iter().find(|p| p.id == *id) {
                    None => return Err("La derivada usa un parámetro que no existe".into()),
                    Some(p) if !parametro_como_fuente(p) => {
                        return Err(format!("«{}» no sirve de fuente: solo valen los parámetros numéricos que se escriben a mano", p.clave));
                    }
                    Some(_) => {}
                }
            }
            let todas: Vec<u16> = d.bases.iter().chain(&d.opcionales).copied().collect();
            if (1..todas.len()).any(|i| todas[..i].contains(&todas[i])) {
                return Err("Una variable no puede repetirse entre las bases y las opcionales".into());
            }
            let es_base = |id: &u16| {
                otras.iter().any(|o| o.id == *id && tipo_de_grupo(grupos, o.grupo) == Some(TipoGrupoVariables::Base))
            };
            if !todas.iter().all(es_base) {
                return Err("Una derivada solo puede depender de variables base".into());
            }
            validar_formula(d.multiplicador, d.divisor, d.constante)?;
            // Las fórmulas individuales: de entradas que la derivada usa, una por entrada y con valores válidos
            for (i, f) in d.formulas_entradas.iter().enumerate() {
                let usa = match f.origen {
                    OrigenEntrada::Variable(id) => d.bases.contains(&id) || d.opcionales.contains(&id),
                    OrigenEntrada::Parametro(id) => d.parametros.contains(&id) || d.parametros_opcionales.contains(&id),
                };
                if !usa {
                    return Err("Una fórmula individual es de una entrada que la variable no usa".into());
                }
                if d.formulas_entradas[..i].iter().any(|o| o.origen == f.origen) {
                    return Err("Una entrada no puede tener dos fórmulas individuales".into());
                }
                validar_formula(f.multiplicador, f.divisor, f.constante)?;
            }
            // Que la lista de rangos exista se comprueba donde están las listas (partida.rs, conjuntos.rs): aquí no se conocen
        }
    }
    Ok(())
}

/// ¿Es la misma variable (mismo grupo, nombre y abreviatura sin mayúsculas, bases, opcionales, neutro y escala)? Crear dos
/// veces lo mismo (un doble guardado) no debe dar error ni duplicar nada: se devuelve la que ya hay.
pub fn es_la_misma(a: &DefinicionVariable, b: &DefinicionVariable) -> bool {
    a.grupo == b.grupo
        && clave(&a.nombre) == clave(&b.nombre)
        && clave(&a.abreviatura) == clave(&b.abreviatura)
        && a.bases == b.bases
        && a.opcionales == b.opcionales
        && a.parametros == b.parametros
        && a.parametros_opcionales == b.parametros_opcionales
        && a.neutro == b.neutro
        && a.escala == b.escala
        && (a.multiplicador, a.divisor, a.constante) == (b.multiplicador, b.divisor, b.constante)
        && a.rangos == b.rangos
        && a.formulas_entradas == b.formulas_entradas
}

/// Comprueba el nombre de un grupo contra los DEMÁS (al crear: todos; al renombrar: todos menos él).
pub fn validar_grupo(otros: &[GrupoVariables], nombre: &str) -> Result<(), String> {
    let nombre = nombre.trim();
    if nombre.is_empty() {
        return Err("El grupo necesita un nombre".into());
    }
    if nombre.chars().count() > MAX_NOMBRE_GRUPO {
        return Err(format!("El nombre del grupo no puede pasar de {MAX_NOMBRE_GRUPO} caracteres"));
    }
    if nombre.chars().any(char::is_control) {
        return Err("El nombre del grupo tiene caracteres no permitidos".into());
    }
    if otros.iter().any(|g| clave(&g.nombre) == clave(nombre)) {
        return Err(format!("Ya hay un grupo que se llama «{nombre}»"));
    }
    if otros.len() >= MAX_GRUPOS_VARIABLES {
        return Err(format!("No puede haber más de {MAX_GRUPOS_VARIABLES} grupos de variables"));
    }
    Ok(())
}

/// Qué pasa al borrar las variables `ids`: las derivadas que las usaban y siguen (`afectadas`) y TODO lo que se borra
/// (`borradas`: esas y las derivadas que se quedan sin ninguna base). Lo mismo que hace `aplicar_borrado`; el front lo
/// enseña en el aviso antes de confirmar.
pub fn efecto_de_borrar(defs: &[DefinicionVariable], ids: &[u16]) -> (Vec<u16>, Vec<u16>) {
    let mut borradas: Vec<u16> = ids.to_vec();
    // Una derivada no es base de nada, así que basta una pasada
    let sin_bases: Vec<u16> = defs
        .iter()
        .filter(|d| es_derivada(d) && !borradas.contains(&d.id) && d.parametros.is_empty() && d.bases.iter().all(|b| borradas.contains(b)))
        .map(|d| d.id)
        .collect();
    borradas.extend(sin_bases);
    let afectadas = defs
        .iter()
        .filter(|d| !borradas.contains(&d.id) && d.bases.iter().chain(&d.opcionales).any(|x| borradas.contains(x)))
        .map(|d| d.id)
        .collect();
    (afectadas, borradas)
}

/// Borra las variables `ids` en cascada (ver `efecto_de_borrar`): desaparecen, salen de las bases y opcionales de las
/// derivadas, y las derivadas que se quedan sin bases también se borran.
pub fn aplicar_borrado(defs: &mut Vec<DefinicionVariable>, ids: &[u16]) {
    let (_, borradas) = efecto_de_borrar(defs, ids);
    defs.retain(|d| !borradas.contains(&d.id));
    for d in defs.iter_mut() {
        d.bases.retain(|b| !borradas.contains(b));
        d.opcionales.retain(|o| !borradas.contains(o));
    }
}

/// Quita de las derivadas los parámetros que ya no pueden ser fuente (borrados, de texto o calculados) y borra las derivadas
/// que se quedan sin ninguna fuente fija (ni bases ni parámetros). Nada depende de una derivada, así que basta una pasada.
pub fn limpiar_parametros_fuente(defs: &mut Vec<DefinicionVariable>, params: &[DefinicionParametro]) {
    let vale = |id: &u16| params.iter().any(|p| p.id == *id && parametro_como_fuente(p));
    let eran: Vec<u16> = defs.iter().filter(|d| es_derivada(d)).map(|d| d.id).collect();
    for d in defs.iter_mut() {
        d.parametros.retain(vale);
        d.parametros_opcionales.retain(vale);
        // Una fórmula individual solo vale mientras su entrada siga siendo de la derivada
        let (bases, opcionales, parametros, opcionales_p) = (d.bases.clone(), d.opcionales.clone(), d.parametros.clone(), d.parametros_opcionales.clone());
        d.formulas_entradas.retain(|f| match f.origen {
            OrigenEntrada::Variable(id) => bases.contains(&id) || opcionales.contains(&id),
            OrigenEntrada::Parametro(id) => parametros.contains(&id) || opcionales_p.contains(&id),
        });
    }
    defs.retain(|d| !eran.contains(&d.id) || es_derivada(d));
}

/// Las variables cuyo grupo no existe (las de antes de los grupos traen 0) pasan al primer grupo de bases; si no hay
/// ninguno, se crea «Variables». Determinista: se hace al cargar y tras cada cambio de definiciones.
pub fn asegurar_grupos(grupos: &mut Vec<GrupoVariables>, defs: &mut [DefinicionVariable]) {
    if defs.iter().all(|d| grupos.iter().any(|g| g.id == d.grupo)) {
        return;
    }
    let base = match grupos.iter().find(|g| g.tipo == TipoGrupoVariables::Base) {
        Some(g) => g.id,
        None => {
            let id = grupos.iter().map(|g| g.id).max().unwrap_or(0) + 1;
            grupos.push(GrupoVariables { id, nombre: "Variables".into(), tipo: TipoGrupoVariables::Base });
            id
        }
    };
    for d in defs.iter_mut().filter(|d| !grupos.iter().any(|g| g.id == d.grupo)) {
        d.grupo = base;
        d.bases.clear();
        d.opcionales.clear();
        d.parametros.clear();
        d.parametros_opcionales.clear();
        d.opciones.clear();
        d.rangos = None;
        d.formulas_entradas.clear();
        (d.multiplicador, d.divisor, d.constante) = (1, 1, 0);
    }
}

#[cfg(test)]
pub mod pruebas {
    use super::*;
    use crate::tablero_general::modelos::grupos_por_defecto;

    /// Una variable base del grupo 1 (el de serie).
    pub fn var(id: u16, nombre: &str, abreviatura: &str, neutro: i16, escala: i16) -> DefinicionVariable {
        DefinicionVariable {
            id, nombre: nombre.into(), abreviatura: abreviatura.into(), neutro, escala, grupo: 1, bases: vec![], opcionales: vec![],
            multiplicador: 1, divisor: 1, constante: 0, opciones: vec![], rangos: None, parametros: vec![], parametros_opcionales: vec![], formulas_entradas: vec![],
        }
    }

    /// Una derivada del grupo 2.
    pub fn derivada(id: u16, nombre: &str, abreviatura: &str, bases: &[u16], opcionales: &[u16]) -> DefinicionVariable {
        DefinicionVariable {
            id, nombre: nombre.into(), abreviatura: abreviatura.into(), neutro: 0, escala: 1, grupo: 2,
            bases: bases.to_vec(), opcionales: opcionales.to_vec(), multiplicador: 1, divisor: 1, constante: 0, opciones: vec![], rangos: None, parametros: vec![], parametros_opcionales: vec![], formulas_entradas: vec![],
        }
    }

    pub fn valor(id: u16, valor: i16) -> ValorVariable {
        ValorVariable { id_definicion: id, valor, opcionales: vec![], eleccion: 0, parametros_marcados: vec![] }
    }

    /// El grupo de serie (bases, id 1) y «Habilidades» (derivadas, id 2).
    pub fn grupos() -> Vec<GrupoVariables> {
        let mut g = grupos_por_defecto();
        g.push(GrupoVariables { id: 2, nombre: "Habilidades".into(), tipo: TipoGrupoVariables::Derivadas });
        g
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
    fn una_derivada_suma_sus_bases_y_las_opcionales_marcadas() {
        // Destreza (10/2), Fuerza (10/2), Competencia (0/1); Acrobacias ← Destreza (+ com); Atletismo ← Fuerza + Destreza
        let defs = vec![
            var(1, "Destreza", "des", 10, 2),
            var(2, "Fuerza", "fue", 10, 2),
            var(3, "Competencia", "com", 0, 1),
            derivada(4, "Acrobacias", "acro", &[1], &[3]),
            derivada(5, "Atletismo", "atl", &[2, 1], &[3]),
        ];
        let mut valores = vec![valor(1, 16), valor(2, 8), valor(3, 2), valor(4, 0), valor(5, 0)];
        assert_eq!(resultado(&defs, &[], &valores, &defs[3]), 3, "sin el check, solo Destreza");
        assert_eq!(resultado(&defs, &[], &valores, &defs[4]), 3 - 1, "dos bases se suman");
        valores[3].opcionales = vec![3];
        assert_eq!(resultado(&defs, &[], &valores, &defs[3]), 5, "con competencia");
        assert_eq!(resultado(&defs, &[], &valores, &defs[4]), 2, "el check es de cada derivada");
        // Una base sigue siendo su modificador; un valor que falta es el neutro
        assert_eq!(resultado(&defs, &[], &valores, &defs[0]), 3);
        assert_eq!(resultado(&defs, &[], &[], &defs[3]), 0);
    }

    #[test]
    fn normalizar_rellena_con_el_neutro_ordena_quita_y_limpia_las_opcionales() {
        let defs = vec![var(1, "Fuerza", "f", 10, 2), var(2, "Cordura", "cor", 7, 3), derivada(3, "Atletismo", "atl", &[1], &[2])];
        let mut vs = vec![
            valor(2, 9),
            valor(99, 1),
            valor(2, 1),
            ValorVariable { id_definicion: 3, valor: 0, opcionales: vec![2, 2, 1, 77], eleccion: 0, parametros_marcados: vec![] },
        ];
        vs[0].opcionales = vec![1]; // una base no lleva opcionales
        normalizar_variables(&defs, &mut vs);
        assert_eq!(vs, vec![valor(1, 10), valor(2, 9), ValorVariable { id_definicion: 3, valor: 0, opcionales: vec![2], eleccion: 0, parametros_marcados: vec![] }]);
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
        let g = grupos();
        let otras = vec![var(1, "Fuerza", "f", 10, 2)];
        assert!(validar_definicion(&otras, &g, &[], &var(2, "Inteligencia", "in", 10, 2)).is_ok());
        assert!(validar_definicion(&otras, &g, &[], &var(2, "Fuerza mágica", "fm", 10, 2)).is_ok(), "acentos y espacios en el nombre");
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
            DefinicionVariable { grupo: 9, ..var(2, "Inteligencia", "in", 10, 2) },
        ] {
            assert!(validar_definicion(&otras, &g, &[], &mala).is_err(), "{mala:?}");
        }
        // La abreviatura es opcional, y varias variables pueden no tenerla
        let sin = vec![var(1, "Fuerza", "", 10, 2)];
        assert!(validar_definicion(&sin, &g, &[], &var(2, "Destreza", "", 10, 2)).is_ok());
        assert!(buscar(&[var(1, "Fuerza", "", 10, 2)], "").is_none());
        assert_eq!(buscar(&[var(1, "Fuerza", "", 10, 2)], "fuerza").unwrap().id, 1);
        let muchas: Vec<_> =
            (0..MAX_VARIABLES as u16).map(|i| var(i, &"a".repeat(i as usize % 29 + 1), "", 10, 2)).collect();
        assert!(validar_definicion(&muchas, &g, &[], &var(999, "Otra", "z", 10, 2)).unwrap_err().contains("más de"));
    }

    #[test]
    fn un_conflicto_dice_contra_que_variable_choca_y_la_misma_variable_se_reconoce() {
        let g = grupos();
        let otras = vec![var(1, "Prueba", "p", 10, 2)];
        let e = validar_definicion(&otras, &g, &[], &var(2, "Rty", "p", 10, 2)).unwrap_err();
        assert!(e.contains("«p»") && e.contains("«Prueba»") && e.contains("«Variables»"), "{e}");
        assert!(es_la_misma(&var(1, "Prueba", "p", 10, 2), &var(9, " prueba ", "P", 10, 2)), "sin mayúsculas ni espacios, el id no cuenta");
        assert!(!es_la_misma(&var(1, "Prueba", "p", 10, 2), &var(1, "Prueba", "p", 10, 3)));
        assert!(!es_la_misma(&var(1, "Prueba", "p", 10, 2), &DefinicionVariable { grupo: 2, ..var(1, "Prueba", "p", 10, 2) }));
    }

    #[test]
    fn validacion_de_las_derivadas() {
        let g = grupos();
        let otras = vec![var(1, "Destreza", "des", 10, 2), var(3, "Competencia", "com", 0, 1), derivada(4, "Sigilo", "sig", &[1], &[])];
        assert!(validar_definicion(&otras, &g, &[], &derivada(5, "Acrobacias", "acro", &[1], &[3])).is_ok());
        assert!(validar_definicion(&otras, &g, &[], &derivada(5, "Acrobacias", "acro", &[1, 3], &[])).is_ok(), "dos bases");
        for (mala, motivo) in [
            (derivada(5, "Acrobacias", "acro", &[], &[3]), "sin bases"),
            (derivada(5, "Acrobacias", "acro", &[4], &[]), "una derivada no es base"),
            (derivada(5, "Acrobacias", "acro", &[1], &[4]), "ni opcional"),
            (derivada(5, "Acrobacias", "acro", &[1, 1], &[]), "repetida"),
            (derivada(5, "Acrobacias", "acro", &[1], &[1]), "base y opcional a la vez"),
            (derivada(5, "Acrobacias", "acro", &[99], &[]), "no existe"),
            (derivada(5, "Sigilo", "s", &[1], &[]), "nombre repetido aunque sea de otro grupo"),
            (derivada(5, "Destreza veloz", "des", &[1], &[]), "abreviatura de una base"),
        ] {
            assert!(validar_definicion(&otras, &g, &[], &mala).is_err(), "{motivo}");
        }
        // Una base no puede tener bases
        assert!(validar_definicion(&otras, &g, &[], &DefinicionVariable { bases: vec![1], ..var(6, "Rara", "r", 10, 2) }).is_err());
    }

    #[test]
    fn validacion_de_los_grupos() {
        let g = grupos();
        assert!(validar_grupo(&g, "Salvaciones").is_ok());
        assert!(validar_grupo(&g, " habilidades ").is_err(), "repetido sin mayúsculas");
        assert!(validar_grupo(&g, "  ").is_err());
        assert!(validar_grupo(&g, &"x".repeat(31)).is_err());
        assert!(validar_grupo(&g, "a\u{7}b").is_err());
        let muchos: Vec<_> = (0..MAX_GRUPOS_VARIABLES as u16)
            .map(|i| GrupoVariables { id: i, nombre: format!("G{i}"), tipo: TipoGrupoVariables::Base })
            .collect();
        assert!(validar_grupo(&muchos, "Otro").is_err());
    }

    #[test]
    fn borrar_una_base_la_quita_de_las_derivadas_y_borra_las_que_se_quedan_sin_bases() {
        let mut defs = vec![
            var(1, "Destreza", "des", 10, 2),
            var(2, "Fuerza", "fue", 10, 2),
            var(3, "Competencia", "com", 0, 1),
            derivada(4, "Acrobacias", "acro", &[1], &[3]),
            derivada(5, "Atletismo", "atl", &[2, 1], &[3]),
        ];
        assert_eq!(efecto_de_borrar(&defs, &[1]), (vec![5], vec![1, 4]), "Acrobacias se queda sin bases");
        assert_eq!(efecto_de_borrar(&defs, &[3]), (vec![4, 5], vec![3]), "una opcional solo se quita");
        assert_eq!(efecto_de_borrar(&defs, &[4]), (vec![], vec![4]));
        aplicar_borrado(&mut defs, &[1]);
        assert_eq!(defs.iter().map(|d| d.id).collect::<Vec<_>>(), vec![2, 3, 5]);
        assert_eq!(defs[2].bases, vec![2]);
        aplicar_borrado(&mut defs, &[3]);
        assert!(defs[1].opcionales.is_empty());
    }

    #[test]
    fn las_variables_sin_grupo_pasan_al_primero_de_bases() {
        // Una partida de antes de los grupos: sus variables traen grupo 0
        let mut defs = vec![DefinicionVariable { grupo: 0, ..var(1, "Fuerza", "f", 10, 2) }];
        let mut g = grupos();
        asegurar_grupos(&mut g, &mut defs);
        assert_eq!((defs[0].grupo, g.len()), (1, 2));
        // Sin ningún grupo de bases, se crea «Variables»
        let mut solo_derivadas = vec![GrupoVariables { id: 5, nombre: "H".into(), tipo: TipoGrupoVariables::Derivadas }];
        let mut defs = vec![DefinicionVariable { grupo: 0, ..var(1, "Fuerza", "f", 10, 2) }];
        asegurar_grupos(&mut solo_derivadas, &mut defs);
        assert_eq!(solo_derivadas.len(), 2);
        assert_eq!((defs[0].grupo, solo_derivadas[1].tipo), (6, TipoGrupoVariables::Base));
        // Una de antes de los grupos se lee sin los campos nuevos
        let vieja: DefinicionVariable = serde_json::from_str(r#"{"id":1,"nombre":"Fuerza","abreviatura":"f","neutro":10,"escala":2}"#).unwrap();
        assert_eq!((vieja.grupo, vieja.bases.len()), (0, 0));
        let valor_viejo: ValorVariable = serde_json::from_str(r#"{"id_definicion":1,"valor":12}"#).unwrap();
        assert!(valor_viejo.opcionales.is_empty());
    }

    #[test]
    fn la_formula_de_una_derivada_redondea_hacia_abajo_y_suma_la_constante() {
        assert_eq!(formula(7, 1, 1, 0), 7, "sin tocar, la suma tal cual");
        assert_eq!(formula(7, 1, 2, 0), 3);
        assert_eq!(formula(-7, 1, 2, 0), -4, "hacia abajo también en negativo");
        assert_eq!(formula(3, 2, 1, 10), 16);
        assert_eq!(formula(5, 3, 2, -1), 6);
        // Cthulhu: la mitad de una habilidad
        let defs = vec![var(1, "Esquivar", "esq", 0, 1), DefinicionVariable { divisor: 2, ..derivada(2, "Esquivar difícil", "esqd", &[1], &[]) }];
        let valores = vec![valor(1, 65), valor(2, 0)];
        assert_eq!(resultado(&defs, &[], &valores, &defs[1]), 32);
    }

    #[test]
    fn la_opcion_elegida_suma_su_bono_y_una_eleccion_perdida_vuelve_a_la_primera() {
        use crate::tablero_general::modelos::OpcionVariable;
        // Pathfinder: habilidad = Destreza + nivel (base) + rango elegido
        let rangos = vec![
            OpcionVariable { nombre: "Sin entrenar".into(), bonus: 0 },
            OpcionVariable { nombre: "Entrenado".into(), bonus: 2 },
            OpcionVariable { nombre: "Experto".into(), bonus: 4 },
        ];
        let defs = vec![var(1, "Destreza", "des", 10, 2), var(2, "Nivel", "niv", 0, 1), DefinicionVariable { opciones: rangos, ..derivada(3, "Acrobacias", "acro", &[1, 2], &[]) }];
        let mut valores = vec![valor(1, 14), valor(2, 3), valor(3, 0)];
        assert_eq!(resultado(&defs, &[], &valores, &defs[2]), 2 + 3, "sin entrenar: +0");
        valores[2].eleccion = 2;
        assert_eq!(resultado(&defs, &[], &valores, &defs[2]), 2 + 3 + 4);
        // Una elección que ya no existe (se quitó una opción) vuelve a la primera al normalizar
        valores[2].eleccion = 7;
        normalizar_variables(&defs, &mut valores);
        assert_eq!(valores[2].eleccion, 0);
        // Sin opciones no hay elección ni bono
        let mut sin = valores.clone();
        sin[2].eleccion = 1;
        normalizar_variables(&[defs[0].clone(), defs[1].clone(), derivada(3, "Acrobacias", "acro", &[1, 2], &[])], &mut sin);
        assert_eq!(sin[2].eleccion, 0);
    }

    #[test]
    fn un_parametro_calculado_sigue_a_las_variables_y_no_sale_de_su_rango() {
        use crate::tablero_general::modelos::{normalizar_parametros, parametros_por_defecto, Cuerpo, TipoEntidad};
        // Cthulhu: vida = (CON + TAM) / 10
        let vars = vec![var(1, "Constitución", "con", 0, 1), var(2, "Tamaño", "tam", 0, 1)];
        let mut defs = crate::tablero_general::modelos::parametros_por_defecto();
        defs[0].calculo = Some(CalculoParametro { variables: vec![1, 2], multiplicador: 1, divisor: 10, constante: 0 });
        let mut ps = Vec::new();
        normalizar_parametros(&defs, &mut ps);
        let mut f = Entidad {
            id: 1, nombre: "A".into(), tipo: TipoEntidad::Jugador, parametros: ps, variables: vec![valor(1, 60), valor(2, 55)], pasos_valor: vec![],
            cuerpo: Cuerpo::Pequenio, zona_id: 1, foto_url: None, miniatura_url: None, foto_encuadre: Default::default(),
            archivos_adjuntos: vec![], es_unico: false, vision: 0, plantilla_id: None, actuales: vec![], propietarios: vec![],
        };
        recalcular_parametros(&defs, &vars, &mut f);
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(11), "(60 + 55) / 10");
        f.variables[0].valor = 5;
        recalcular_parametros(&defs, &vars, &mut f);
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(6));
        assert_eq!(f.parametros[1].valor, ValorParametro::Numero(10), "los demás no se tocan");
        // Un resultado negativo se queda en 0
        defs[0].calculo = Some(CalculoParametro { variables: vec![1], multiplicador: 1, divisor: 1, constante: -50 });
        recalcular_parametros(&defs, &vars, &mut f);
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(0));
        // Una derivada también cuenta, por su resultado
        let mut con_derivada = vars.clone();
        con_derivada.push(derivada(3, "Resistencia", "res", &[1], &[]));
        f.variables.push(valor(3, 0));
        defs[0].calculo = Some(CalculoParametro { variables: vec![3], multiplicador: 2, divisor: 1, constante: 4 });
        f.variables[0].valor = 9;
        recalcular_parametros(&defs, &con_derivada, &mut f);
        assert_eq!(f.parametros[0].valor, ValorParametro::Numero(22), "2 · 9 + 4");

        // Si desaparece la variable, el cálculo se limpia (y sin ninguna, el parámetro vuelve a ser manual)
        limpiar_calculos(&mut defs, &[vars[1].clone()]);
        assert!(defs[0].calculo.is_none());
        defs[0].calculo = Some(CalculoParametro { variables: vec![1, 2], multiplicador: 1, divisor: 1, constante: 0 });
        limpiar_calculos(&mut defs, &[vars[1].clone()]);
        assert_eq!(defs[0].calculo.as_ref().unwrap().variables, vec![2]);
    }

    #[test]
    fn validacion_de_formulas_opciones_y_calculos() {
        use crate::tablero_general::modelos::OpcionVariable;
        let g = grupos();
        let otras = vec![var(1, "Destreza", "des", 10, 2), var(3, "Competencia", "com", 0, 1)];
        let ok = DefinicionVariable { multiplicador: 2, divisor: 3, constante: -4, ..derivada(5, "Acrobacias", "acro", &[1], &[3]) };
        assert!(validar_definicion(&otras, &g, &[], &ok).is_ok());
        let rango = |n: &str, b: i16| OpcionVariable { nombre: n.into(), bonus: b };
        assert!(validar_definicion(&otras, &g, &[], &DefinicionVariable { rangos: Some(1), ..derivada(5, "Acrobacias", "acro", &[1], &[]) }).is_ok());
        for mala in [
            DefinicionVariable { multiplicador: 0, ..derivada(5, "Acrobacias", "acro", &[1], &[]) },
            DefinicionVariable { divisor: 0, ..derivada(5, "Acrobacias", "acro", &[1], &[]) },
            DefinicionVariable { divisor: 101, ..derivada(5, "Acrobacias", "acro", &[1], &[]) },
            DefinicionVariable { constante: 10000, ..derivada(5, "Acrobacias", "acro", &[1], &[]) },
            // Una base no lleva fórmula ni rangos
            DefinicionVariable { multiplicador: 2, ..var(6, "Rara", "r", 10, 2) },
            DefinicionVariable { rangos: Some(1), ..var(6, "Rara", "r", 10, 2) },
            DefinicionVariable { opciones: vec![rango("X", 1)], ..var(6, "Rara", "r", 10, 2) },
        ] {
            assert!(validar_definicion(&otras, &g, &[], &mala).is_err(), "{mala:?}");
        }
        assert!(validar_definicion(&otras, &g, &[], &var(6, "Versus", "vs", 10, 2)).is_err(), "«vs» está reservado");
        // Las que cambian la fórmula o las opciones no son «la misma»
        assert!(!es_la_misma(&derivada(5, "A", "a", &[1], &[]), &DefinicionVariable { constante: 1, ..derivada(5, "A", "a", &[1], &[]) }));
        assert!(!es_la_misma(&derivada(5, "A", "a", &[1], &[]), &DefinicionVariable { rangos: Some(1), ..derivada(5, "A", "a", &[1], &[]) }));

        // Cálculos de parámetro
        let c = |variables: Vec<u16>, multiplicador, divisor, constante| CalculoParametro { variables, multiplicador, divisor, constante };
        assert!(validar_calculo(&c(vec![1, 3], 1, 10, 0), &otras).is_ok());
        for malo in [c(vec![], 1, 1, 0), c(vec![1, 1], 1, 1, 0), c(vec![9], 1, 1, 0), c(vec![1], 0, 1, 0), c(vec![1], 1, 0, 0), c(vec![1], 1, 1, 10000)] {
            assert!(validar_calculo(&malo, &otras).is_err(), "{malo:?}");
        }
        let mut texto = crate::tablero_general::modelos::parametros_por_defecto().remove(0);
        texto.valor_por_defecto = ValorParametro::Texto(String::new());
        texto.tiene_maximo = false;
        texto.calculo = Some(c(vec![1], 1, 1, 0));
        assert!(validar_parametro_calculado(&texto, &otras).is_err(), "un texto no se calcula");
        // Una partida guardada antes de esto se lee igual (multiplicador y divisor 1)
        let vieja: DefinicionVariable = serde_json::from_str(r#"{"id":1,"nombre":"Fuerza","abreviatura":"f","neutro":10,"escala":2}"#).unwrap();
        assert_eq!((vieja.multiplicador, vieja.divisor, vieja.constante, vieja.opciones.len()), (1, 1, 0, 0));
        let vieja: CalculoParametro = serde_json::from_str(r#"{"variables":[1]}"#).unwrap();
        assert_eq!((vieja.multiplicador, vieja.divisor), (1, 1));
    }

    #[test]
    fn cada_entrada_puede_tener_su_formula_y_despues_se_aplica_la_del_resultado() {
        use crate::tablero_general::modelos::{FormulaEntrada, OrigenEntrada, Parametro, ValorParametro, ID_ARMADURA};
        let fe = |origen, multiplicador, divisor, constante| FormulaEntrada { origen, multiplicador, divisor, constante };
        let defs = vec![var(1, "Destreza", "des", 10, 2), var(2, "Fuerza", "fue", 10, 2), var(3, "Competencia", "com", 0, 1)];
        let mut d = derivada(4, "Mixta", "mix", &[1, 2], &[3]);
        let mut valores = vec![valor(1, 16), valor(2, 8), valor(3, 3), valor(4, 0)];
        valores[3].opcionales = vec![3]; // la competencia está marcada
        let defs_con = |d: &DefinicionVariable| [defs.clone(), vec![d.clone()]].concat();
        // Sin fórmulas: +3 (des) − 1 (fue) + 3 (com) = 5
        assert_eq!(resultado(&defs_con(&d), &[], &valores, &d), 5);
        // Destreza × 2 = 6 y Competencia + 1 = 4; Fuerza tal cual (−1): suma 9. Luego, la del resultado: ÷ 2 + 1 = 5
        d.formulas_entradas = vec![fe(OrigenEntrada::Variable(1), 2, 1, 0), fe(OrigenEntrada::Variable(3), 1, 1, 1)];
        assert_eq!(resultado(&defs_con(&d), &[], &valores, &d), 9, "solo las individuales");
        (d.divisor, d.constante) = (2, 1);
        assert_eq!(resultado(&defs_con(&d), &[], &valores, &d), 5, "individuales y luego la del resultado");
        // Una opcional desmarcada no aporta (tampoco su fórmula)
        valores[3].opcionales.clear();
        assert_eq!(resultado(&defs_con(&d), &[], &valores, &d), (6 - 1) / 2 + 1);
        // Redondea hacia abajo cada entrada: Fuerza −1 ÷ 2 = −1 (no 0)
        d.formulas_entradas.push(fe(OrigenEntrada::Variable(2), 1, 2, 0));
        (d.divisor, d.constante) = (1, 0);
        assert_eq!(resultado(&defs_con(&d), &[], &valores, &d), 6 - 1);

        // Un parámetro también: su entrada tiene fórmula propia (armadura ÷ 2 + el modificador de la destreza)
        let mut p = derivada(5, "Con CA", "ca", &[1], &[]);
        p.parametros = vec![ID_ARMADURA];
        p.formulas_entradas = vec![fe(OrigenEntrada::Parametro(ID_ARMADURA), 1, 2, 0)];
        let params = vec![Parametro { id_definicion: ID_ARMADURA, valor: ValorParametro::Numero(15), publico: false }];
        assert_eq!(resultado(&defs_con(&p), &params, &valores, &p), 3 + 7);
    }

    #[test]
    fn las_formulas_individuales_se_validan_se_limpian_y_una_base_no_las_lleva() {
        use crate::tablero_general::modelos::{FormulaEntrada, OrigenEntrada};
        let g = grupos();
        let otras = vec![var(1, "Destreza", "des", 10, 2), var(3, "Competencia", "com", 0, 1)];
        let fe = |origen, multiplicador, divisor, constante| FormulaEntrada { origen, multiplicador, divisor, constante };
        let con = |f: Vec<FormulaEntrada>| DefinicionVariable { formulas_entradas: f, ..derivada(5, "Acrobacias", "acro", &[1], &[3]) };
        assert!(validar_definicion(&otras, &g, &[], &con(vec![fe(OrigenEntrada::Variable(1), 2, 3, -4), fe(OrigenEntrada::Variable(3), 1, 1, 1)])).is_ok());
        for mala in [
            con(vec![fe(OrigenEntrada::Variable(9), 1, 1, 0)]), // una entrada que no usa
            con(vec![fe(OrigenEntrada::Parametro(1), 1, 1, 0)]),
            con(vec![fe(OrigenEntrada::Variable(1), 1, 1, 0), fe(OrigenEntrada::Variable(1), 2, 1, 0)]), // dos para la misma
            con(vec![fe(OrigenEntrada::Variable(1), 0, 1, 0)]),
            con(vec![fe(OrigenEntrada::Variable(1), 1, 101, 0)]),
            con(vec![fe(OrigenEntrada::Variable(1), 1, 1, 10_000)]),
            DefinicionVariable { formulas_entradas: vec![fe(OrigenEntrada::Variable(1), 2, 1, 0)], ..var(6, "Rara", "r", 10, 2) },
        ] {
            assert!(validar_definicion(&otras, &g, &[], &mala).is_err(), "{mala:?}");
        }
        assert!(!es_la_misma(&con(vec![]), &con(vec![fe(OrigenEntrada::Variable(1), 2, 1, 0)])));
        // Si la entrada deja de ser de la derivada (se quitó la base), su fórmula se va con ella
        let mut defs = vec![var(1, "Destreza", "des", 10, 2), var(3, "Competencia", "com", 0, 1), con(vec![fe(OrigenEntrada::Variable(3), 1, 1, 1), fe(OrigenEntrada::Variable(1), 2, 1, 0)])];
        aplicar_borrado(&mut defs, &[3]);
        limpiar_parametros_fuente(&mut defs, &[]);
        assert_eq!(defs[1].formulas_entradas, vec![fe(OrigenEntrada::Variable(1), 2, 1, 0)]);
        // Una partida o un archivo de antes se lee sin fórmulas individuales
        let vieja: DefinicionVariable = serde_json::from_str(r#"{"id":1,"nombre":"Fuerza","abreviatura":"f","neutro":10,"escala":2}"#).unwrap();
        assert!(vieja.formulas_entradas.is_empty());
        let f: FormulaEntrada = serde_json::from_str(r#"{"origen":{"Parametro":2}}"#).unwrap();
        assert_eq!((f.origen, f.multiplicador, f.divisor, f.constante), (OrigenEntrada::Parametro(2), 1, 1, 0));
    }

    #[test]
    fn los_valores_tienen_tope() {
        let defs = vec![var(1, "Fuerza", "f", 10, 2)];
        assert!(validar_valores(&defs, &[valor(1, 9999)]).is_ok());
        assert!(validar_valores(&defs, &[valor(1, -10000)]).is_err());
    }
    // ---- Parámetros visuales como fuente de una derivada ----

    fn parametro(id: u16, valor: i16) -> Parametro {
        Parametro { id_definicion: id, valor: ValorParametro::Numero(valor), publico: false }
    }

    #[test]
    fn una_derivada_suma_parametros_fijos_y_opcionales() {
        use crate::tablero_general::modelos::{ID_ARMADURA, ID_VELOCIDAD};
        let des = var(1, "Destreza", "des", 10, 2);
        let defensa = DefinicionVariable {
            parametros: vec![ID_ARMADURA], parametros_opcionales: vec![ID_VELOCIDAD], bases: vec![1], ..derivada(2, "Defensa", "def", &[], &[])
        };
        let defs = vec![des, defensa.clone()];
        let params = vec![parametro(ID_ARMADURA, 12), parametro(ID_VELOCIDAD, 9)];
        let mut vs = vec![valor(1, 14), valor(2, 0)];
        assert_eq!(resultado(&defs, &params, &vs, &defs[1]), 2 + 12, "destreza +2 y armadura 12");
        vs[1].parametros_marcados = vec![ID_VELOCIDAD];
        assert_eq!(resultado(&defs, &params, &vs, &defs[1]), 2 + 12 + 9, "con el opcional marcado");
        // La fórmula se aplica a toda la suma
        let con_formula = DefinicionVariable { multiplicador: 1, divisor: 2, constante: 1, ..defensa };
        let defs = vec![defs[0].clone(), con_formula];
        assert_eq!(resultado(&defs, &params, &vs, &defs[1]), (2 + 12 + 9) / 2 + 1);
        // Sin el parámetro en la ficha cuenta 0
        assert_eq!(resultado(&defs, &[], &vs, &defs[1]), 2 / 2 + 1);
    }

    #[test]
    fn una_derivada_solo_de_parametros_es_derivada_y_se_valida() {
        use crate::tablero_general::modelos::{CalculoParametro, ID_ARMADURA, ID_VIDA};
        let g = grupos();
        let params = crate::tablero_general::modelos::parametros_por_defecto();
        let solo_armadura = DefinicionVariable { parametros: vec![ID_ARMADURA], ..derivada(2, "Defensa", "def", &[], &[]) };
        assert!(es_derivada(&solo_armadura));
        assert!(validar_definicion(&[], &g, &params, &solo_armadura).is_ok());
        // Sin ninguna fuente fija, no
        let sin = DefinicionVariable { parametros_opcionales: vec![ID_ARMADURA], ..derivada(2, "Defensa", "def", &[], &[]) };
        assert!(validar_definicion(&[], &g, &params, &sin).is_err());
        // Parámetro inexistente o repetido
        let raro = DefinicionVariable { parametros: vec![999], ..derivada(2, "Defensa", "def", &[], &[]) };
        assert!(validar_definicion(&[], &g, &params, &raro).is_err());
        let repe = DefinicionVariable { parametros: vec![ID_ARMADURA], parametros_opcionales: vec![ID_ARMADURA], ..derivada(2, "Defensa", "def", &[], &[]) };
        assert!(validar_definicion(&[], &g, &params, &repe).is_err());
        // Uno calculado con variables no sirve (ciclo)
        let mut calculados = params.clone();
        calculados.iter_mut().find(|p| p.id == ID_VIDA).unwrap().calculo =
            Some(CalculoParametro { variables: vec![1], multiplicador: 1, divisor: 1, constante: 0 });
        let con_vida = DefinicionVariable { parametros: vec![ID_VIDA], ..derivada(2, "Defensa", "def", &[], &[]) };
        assert!(validar_definicion(&[], &g, &calculados, &con_vida).is_err());
        // Una base no lleva parámetros
        let base = DefinicionVariable { parametros: vec![ID_ARMADURA], ..var(3, "Fuerza", "f", 10, 2) };
        assert!(validar_definicion(&[], &g, &params, &base).is_err());
    }

    #[test]
    fn borrar_un_parametro_limpia_las_derivadas() {
        use crate::tablero_general::modelos::{ID_ARMADURA, ID_VELOCIDAD};
        let mut params = crate::tablero_general::modelos::parametros_por_defecto();
        let con_dos = DefinicionVariable { parametros: vec![ID_ARMADURA], parametros_opcionales: vec![ID_VELOCIDAD], ..derivada(2, "Defensa", "def", &[1], &[]) };
        let solo_param = DefinicionVariable { parametros: vec![ID_ARMADURA], ..derivada(3, "Coraza", "cor", &[], &[]) };
        let mut defs = vec![var(1, "Destreza", "des", 10, 2), con_dos, solo_param];
        params.retain(|p| p.id != ID_ARMADURA);
        limpiar_parametros_fuente(&mut defs, &params);
        assert_eq!(defs.len(), 2, "la que solo usaba Armadura desaparece");
        assert!(defs[1].parametros.is_empty());
        assert_eq!(defs[1].parametros_opcionales, vec![ID_VELOCIDAD], "lo que sigue valiendo se queda");
        assert_eq!(defs[1].bases, vec![1]);
    }

    #[test]
    fn las_marcas_de_parametros_siguen_a_los_opcionales() {
        use crate::tablero_general::modelos::{ID_ARMADURA, ID_VELOCIDAD};
        let d = DefinicionVariable { parametros: vec![ID_ARMADURA], parametros_opcionales: vec![ID_VELOCIDAD], ..derivada(2, "Defensa", "def", &[1], &[]) };
        let defs = vec![var(1, "Destreza", "des", 10, 2), d];
        let mut vs = vec![valor(1, 10), ValorVariable { parametros_marcados: vec![ID_VELOCIDAD, ID_VELOCIDAD, ID_ARMADURA, 77], ..valor(2, 0) }];
        normalizar_variables(&defs, &mut vs);
        assert_eq!(vs[1].parametros_marcados, vec![ID_VELOCIDAD], "sin repetidos ni lo que no es opcional");
    }
}
