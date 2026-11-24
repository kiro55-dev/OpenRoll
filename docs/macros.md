# Macros

Una macro es un pequeño programa que guardas en la partida y asocias a una ficha. Hace lo que harías a mano: tirar
dados, sacar resultados al chat, quitar o dar vida, guardar números para luego, y repartir efectos (o más macros) a
otras fichas del tablero. Las escriben el host y los jugadores (⚙ → «Macros…», o el botón «+ Macro» de la caja «Parámetros», que abre el editor
ya ligado a esa ficha: buscas por nombre, pulsas una para asociársela o creas una nueva; clic derecho sobre una macro de la
ficha la abre para editarla); cada uno solo puede asociarlas a fichas que controla. Rust la interpreta en el ordenador del host, así que vale lo mismo para todos.

**Crear y editar:** «+ Nueva macro» la crea al momento como «Macro 1», «Macro 2»… y la abre. El nombre se aplica al salir del
campo (o con Enter). El texto se guarda solo como borrador mientras escribes, así que puedes dejarla a medias y seguir
después (aunque cierres la ventana o la aplicación); **Guardar** (o Ctrl+S) es lo que aplica el texto a la macro.

## Cuándo se ejecuta

La primera línea dice cuándo:

| Cabecera | Qué pasa |
|---|---|
| `ONCLICK` | Sale un botón con su nombre en la caja «Parámetros» de la derecha (con la ficha seleccionada). |
| `ONTURN` | Se ejecuta sola cuando, en batalla, le llega el turno a esa ficha (también si vuelve a ella en otra vuelta). No sale botón. |
| `ONCLICK ONTURN` | Las dos cosas: botón, y sola al llegarle el turno. |

## Argumentos

Un argumento es una suma (o resta) de términos; se pueden agrupar con paréntesis:

| Término | Ejemplo | Qué vale |
|---|---|---|
| Número | `12` | Eso. |
| Dados | `2d6`, `d20`, `1d20adv`, `1d20desv`, `2*1d8` | Lo que salga, como en el chat. |
| Variable de la ficha | `.fue`, `.destreza` | Su resultado (por nombre o abreviatura). |
| Parámetro visual | `@vida`, `@vida.max`, `@rangoarma` | Su valor (con máximo, el actual; `.max` el máximo). |
| Atributo | `dano` | Lo que guardó `SAVE` en esta macro. |
| `TURN` | `TURN` | La vuelta de la batalla en la que va su grupo (error fuera de batalla). |

Los nombres van **juntos, sin espacios**: «Rango arma» se escribe `@rangoarma` (da igual mayúsculas). Las palabras en
mayúsculas de esta página están reservadas: no valen como nombre de atributo.

## Condiciones

`argumento comparación argumento`, con `<`, `<=`, `=`, `!=`, `>=`, `>` (y `vs` como `<=`), unidas con `AND` y `OR`
(`AND` va primero; usa paréntesis para cambiarlo). Con `EXECUTE` (o `EXECUTEP`) delante, la comparación sale en el chat
con el resultado que le da la escala del conjunto (crítico, pifia, éxito difícil…), y esa escala decide si se cumple.

## Sentencias

Una por línea. Para varias juntas (en un `DO` o un `ELSE`), un bloque `[ … ]` con una por línea o separadas por `;`.
Una línea que empieza por `IF`, `ELSE`, `AND`, `OR` o `TO` sigue la anterior. `#` empieza un comentario.

| Sentencia | Qué hace |
|---|---|
| `EXECUTE x` | Calcula `x` y lo pone en el chat (lo ven todos). Con una comparación (`EXECUTE 1d20 + .des >= 12`) la enseña con su resultado. |
| `EXECUTEP x` | Igual, pero privado: solo lo ven el host y quien la ejecuta. |
| `SAVE nombre x` | Guarda el número en el atributo `nombre` (lo crea o lo sustituye). Se conserva entre ejecuciones. |
| `CHANGE @param x` | Con `+` o `-` delante, **suma**: `CHANGE @vida -1d4`. Sin signo, **pone** ese valor: `CHANGE @vida 10`. Solo parámetros con máximo (cambia el valor actual de esa ficha). |
| `QUIT` | Quita la macro de la ficha y para. |
| `DO cuerpo IF condición ELSE cuerpo` | Si se cumple, el primer cuerpo; si no, el del `ELSE` (opcional). Sin `IF`, siempre. |
| `DO cuerpo IFNOT condición ELSE cuerpo` | Al revés: el primer cuerpo es para cuando la condición **no** se cumple (y el `ELSE`, para cuando sí). Se lee mejor con las salvaciones: `DO CHANGE @vida -1d4 IFNOT EXECUTE 1d20 + .con >= 14`. |
| `SPREAD acción TO condición` | Hace la acción en cada ficha del mapa que cumple la condición (ver abajo). |

Fuera del chat no sale nada: solo lo que va dentro de `EXECUTE`/`EXECUTEP`.

## SPREAD: afectar a otras fichas

Las candidatas son las fichas colocadas en el mismo mapa (no las ocultas ni la propia). En la condición y en la acción
se puede mirar a cada una:

- `ENTITIES@armadura`, `ENTITIES.fue`: su parámetro o su variable.
- `ENTITIES IN @rangoarma`: que alguna de sus casillas caiga en la zona que dibuja ese parámetro de TU ficha (cuadrado,
  rombo o círculo).
- `DISTANCE`: casillas entre tu ficha y la suya (pegadas = 1, también en diagonal).

La acción puede ser `EXECUTE x` / `EXECUTEP x`, `CHANGE ENTITIES@param x` o `MACROS.nombre, MACROS.otra` (asocia esas
macros tuyas a cada una; una que ya la tiene no la recibe dos veces). **Cada ficha tira sus propios dados**; para una
tirada común (la misma para todas), guárdala antes con `SAVE`. Si la condición no mira a ninguna ficha, se evalúa una vez y
vale para todas o para ninguna.

## Ejemplos

Envenenado (se asocia con SPREAD y se va solo al superar la salvación):

```
ONTURN
DO CHANGE @vida -1d4 IFNOT EXECUTE 1d20 + .con >= 14
ELSE QUIT
```

(Si no supera la salvación pierde vida; si la supera, se quita el veneno.)

Ataque en área (cada objetivo con su tirada de ataque y su daño):

```
ONCLICK
SPREAD CHANGE ENTITIES@vida -(2d8 + .fue) TO ENTITIES@armadura <= 1d20 + .destreza AND ENTITIES IN @rangoarma
```

Bola de fuego (el mismo daño para todos los que no superen la salvación):

```
ONCLICK
SAVE dano 8d6
SPREAD CHANGE ENTITIES@vida -dano TO ENTITIES.des + 1d20 < 15 AND DISTANCE <= 4
```

Toque venenoso (reparte la macro «Envenenado» a quien está pegado):

```
ONCLICK
SPREAD MACROS.envenenado TO DISTANCE <= 1
```

## Límites

Hasta 2000 caracteres y 60 líneas por macro, 20 macros por jugador, 10 macros por ficha, 30 atributos por macro asociada,
999 dados y 1000 pasos por ejecución, y un SPREAD llega como mucho a 100 fichas. Un error para la macro y sale en el chat
(en privado) con el motivo; lo que ya hizo se queda.
