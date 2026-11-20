// src/lib/edicionVariables.svelte.ts
//
// Qué filas de variables de la ventana de parámetros tienen cambios sin guardar (franja roja) y cuáles se guardaron en esta
// sesión (franja verde, como en Notepad++). Nada se guarda al salir de una fila: se guarda con Enter (esa fila), con Ctrl+S o
// con el botón «Guardar cambios» (todas las pendientes). Cada fila se registra aquí con lo que sabe hacer (guardarse y
// deshacerse); la ventana solo pregunta si hay algo pendiente y le pide guardarlo todo.
//
// Un singleton como `partida`: las filas y la ventana no se pasan nada por props. Sin Tauri: se prueba en Node.

export interface FilaEditable {
  /** Guarda la fila. `true` = quedó guardada (o no había nada); `false` = falló (la fila ya enseña su error) y sigue pendiente. */
  guardar: () => Promise<boolean>;
  /** Vuelve a lo que tiene Rust (o quita el borrador). */
  descartar: () => void;
}

export class EdicionVariables {
  /** Las filas montadas, en orden de registro (los grupos de bases se montan antes que los de derivadas). */
  #filas = new Map<symbol, FilaEditable>();
  /** Las que tienen cambios sin guardar. */
  #sucias = $state(new Set<symbol>());
  /** Ids (de Rust) de las variables guardadas en esta sesión, hasta que se vuelvan a editar o se cierre la ventana. */
  verdes = $state(new Set<number>());

  get hayCambios(): boolean {
    return this.#sucias.size > 0;
  }

  get cuantas(): number {
    return this.#sucias.size;
  }

  /** Apunta una fila. Devuelve la función que la quita (al desmontarse). */
  registrar(clave: symbol, fila: FilaEditable): () => void {
    this.#filas.set(clave, fila);
    return () => {
      this.#filas.delete(clave);
      this.marcar(clave, false);
    };
  }

  /** Dice si la fila tiene ahora cambios sin guardar. */
  marcar(clave: symbol, sucia: boolean) {
    if (this.#sucias.has(clave) === sucia) return;
    const nuevo = new Set(this.#sucias);
    if (sucia) nuevo.add(clave);
    else nuevo.delete(clave);
    this.#sucias = nuevo;
  }

  esSucia(clave: symbol): boolean {
    return this.#sucias.has(clave);
  }

  marcarVerde(id: number) {
    if (this.verdes.has(id)) return;
    this.verdes = new Set(this.verdes).add(id);
  }

  quitarVerde(id: number) {
    if (!this.verdes.has(id)) return;
    const nuevo = new Set(this.verdes);
    nuevo.delete(id);
    this.verdes = nuevo;
  }

  /**
   * Guarda todas las pendientes, una a una y en orden (una derivada puede depender de una base recién creada). Si una falla se
   * sigue con las demás, para que un error no deje sin guardar lo que sí vale. `false` si alguna falló.
   */
  async guardarTodo(): Promise<boolean> {
    let todo = true;
    for (const [clave, fila] of [...this.#filas]) {
      if (!this.#sucias.has(clave)) continue;
      if (!(await fila.guardar())) todo = false;
    }
    return todo;
  }

  /** Deshace todo lo pendiente (borradores incluidos). */
  descartarTodo() {
    for (const [clave, fila] of [...this.#filas]) if (this.#sucias.has(clave)) fila.descartar();
  }

  /** Empieza de cero: al abrir y al cerrar la ventana. */
  reiniciar() {
    this.#filas.clear();
    this.#sucias = new Set();
    this.verdes = new Set();
  }
}

export const edicionVariables = new EdicionVariables();
