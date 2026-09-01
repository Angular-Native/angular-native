'use strict'
// Runtime JS de angular-native.
//
// Es lo mínimo que el motor embebido necesita para no ser un intérprete pelado:
// consola, temporizadores, y el escritor de comandos hacia el core en Rust.
// Encima de esto se monta el `Renderer2` de Angular.
//
// Rust inyecta tres funciones antes de evaluar este fichero:
//   __an.log(level, message)   salida a la consola del sistema
//   __an.now()                 milisegundos monótonos desde el arranque
//   __an.flush(bytes, length)  entrega el búfer de comandos al core
;(function (global) {
  const native = global.__an
  if (!native) {
    throw new Error('runtime.js necesita que Rust haya inyectado __an')
  }

  // ------------------------------------------------------------------ consola

  function format(args) {
    let out = ''
    for (let i = 0; i < args.length; i++) {
      if (i > 0) out += ' '
      const value = args[i]
      if (typeof value === 'string') {
        out += value
      } else if (value instanceof Error) {
        // QuickJS pone en `stack` solo los marcos, sin la línea del mensaje:
        // si se imprime `stack` a secas se pierde qué falló.
        out += `${value.name}: ${value.message}`
        if (value.stack) out += `\n${value.stack}`
      } else {
        try {
          out += JSON.stringify(value)
        } catch {
          out += String(value)
        }
      }
    }
    return out
  }

  const LEVELS = { debug: 0, log: 1, info: 1, warn: 2, error: 3 }
  global.console = {}
  for (const name of Object.keys(LEVELS)) {
    global.console[name] = function () {
      native.log(LEVELS[name], format(arguments))
    }
  }

  // ----------------------------------------------------------- temporizadores
  //
  // La cola vive en JS y la vacía el core una vez por frame. Así no hay hilos
  // ni relojes compitiendo: el único tiempo que existe es el del vsync.

  let nextTimerId = 1
  const timers = new Map()
  // Reloj de los temporizadores: el que trae cada frame, no el del sistema.
  // Así el tiempo de la app avanza en pasos de vsync, es determinista, y un
  // test puede simular diez segundos sin esperarlos.
  let frameNow = native.now()

  function schedule(fn, delay, args, repeat) {
    const id = nextTimerId++
    timers.set(id, {
      at: frameNow + Math.max(0, delay || 0),
      fn,
      args,
      interval: repeat ? Math.max(1, delay || 1) : null
    })
    return id
  }

  global.setTimeout = (fn, delay, ...args) => schedule(fn, delay, args, false)
  global.setInterval = (fn, delay, ...args) => schedule(fn, delay, args, true)
  global.clearTimeout = (id) => timers.delete(id)
  global.clearInterval = (id) => timers.delete(id)
  global.performance = { now: () => native.now() }

  // requestAnimationFrame es literalmente el frame: el planificador zoneless de
  // Angular lo usa para agrupar la detección de cambios, y aquí coincide con el
  // vsync sin aproximaciones.
  let nextFrameId = 1
  let frameCallbacks = new Map()

  global.requestAnimationFrame = function (fn) {
    const id = nextFrameId++
    frameCallbacks.set(id, fn)
    return id
  }
  global.cancelAnimationFrame = (id) => frameCallbacks.delete(id)

  function runFrameCallbacks(now) {
    if (frameCallbacks.size === 0) return
    // Se cambia el mapa antes de ejecutar: un callback que vuelve a pedir
    // frame entra en el siguiente, no en este, o el bucle no termina.
    const pending = frameCallbacks
    frameCallbacks = new Map()
    for (const fn of pending.values()) {
      try {
        fn(now)
      } catch (error) {
        console.error('requestAnimationFrame sin capturar:', error)
      }
    }
  }

  function runTimers(now) {
    if (timers.size === 0) return
    // Se resuelve sobre una copia: un callback puede añadir o quitar timers.
    const due = []
    for (const [id, timer] of timers) {
      if (timer.at <= now) due.push([id, timer])
    }
    due.sort((a, b) => a[1].at - b[1].at)
    for (const [id, timer] of due) {
      if (!timers.has(id)) continue
      if (timer.interval === null) {
        timers.delete(id)
      } else {
        timer.at = now + timer.interval
      }
      try {
        timer.fn.apply(null, timer.args)
      } catch (error) {
        console.error('timer sin capturar:', error)
      }
    }
  }

  // ------------------------------------------------- APIs web que sí hacen falta
  //
  // El motor es un intérprete de ECMAScript pelado: no trae nada de la
  // plataforma web. La mayoría de esas APIs no pintan nada aquí, pero unas
  // pocas están en el camino de código de librerías que sí se usan. El router
  // de Angular, sin ir más lejos, crea un `AbortController` por navegación, y
  // como se suscribe tragándose los errores, su ausencia no da ningún fallo:
  // simplemente no navega nunca.

  global.queueMicrotask =
    global.queueMicrotask ||
    function (fn) {
      Promise.resolve().then(fn)
    }

  class AbortSignal {
    constructor() {
      this.aborted = false
      this.reason = undefined
      this.onabort = null
      this._listeners = []
    }

    addEventListener(type, listener) {
      if (type === 'abort') this._listeners.push(listener)
    }

    removeEventListener(type, listener) {
      if (type !== 'abort') return
      const at = this._listeners.indexOf(listener)
      if (at !== -1) this._listeners.splice(at, 1)
    }

    throwIfAborted() {
      if (this.aborted) throw this.reason
    }

    _abort(reason) {
      if (this.aborted) return
      this.aborted = true
      this.reason =
        reason !== undefined ? reason : Object.assign(new Error('Aborted'), { name: 'AbortError' })
      const event = { type: 'abort', target: this }
      if (typeof this.onabort === 'function') this.onabort(event)
      for (const listener of this._listeners.slice()) {
        try {
          typeof listener === 'function' ? listener(event) : listener.handleEvent(event)
        } catch (error) {
          console.error('listener de abort sin capturar:', error)
        }
      }
    }

    static abort(reason) {
      const signal = new AbortSignal()
      signal._abort(reason)
      return signal
    }

    static timeout(ms) {
      const signal = new AbortSignal()
      setTimeout(() => signal._abort(Object.assign(new Error('Timeout'), { name: 'TimeoutError' })), ms)
      return signal
    }
  }

  class AbortController {
    constructor() {
      this.signal = new AbortSignal()
    }

    abort(reason) {
      this.signal._abort(reason)
    }
  }

  global.AbortSignal = AbortSignal
  global.AbortController = AbortController

  // -------------------------------------------------------- búfer de comandos

  const OP = {
    CREATE_NODE: 0x01,
    DESTROY_NODE: 0x02,
    INSERT_CHILD: 0x03,
    REMOVE_CHILD: 0x04,
    SET_STYLE: 0x05,
    SET_PROP_STR: 0x06,
    SET_PROP_NUM: 0x07,
    SET_PROP_BOOL: 0x08,
    SET_PROP_NULL: 0x09,
    SET_TEXT: 0x0a,
    SET_LISTENER: 0x0b,
    SET_ROOT: 0x0c
  }

  const KIND = {
    View: 0,
    Text: 1,
    RawText: 2,
    Image: 3,
    ScrollView: 4,
    TextInput: 5,
    StackView: 6,
    TabBar: 7,
    Switch: 8,
    Slider: 9,
    ActivityIndicator: 10,
    ProgressBar: 11,
    Button: 12,
    Modal: 13,
    Alert: 14,
    Icon: 15
  }

  class CommandWriter {
    constructor(capacity) {
      this.bytes = new Uint8Array(capacity)
      this.view = new DataView(this.bytes.buffer)
      this.offset = 0
    }

    reserve(extra) {
      const needed = this.offset + extra
      if (needed <= this.bytes.length) return
      let size = this.bytes.length
      while (size < needed) size *= 2
      const grown = new Uint8Array(size)
      grown.set(this.bytes)
      this.bytes = grown
      this.view = new DataView(grown.buffer)
    }

    u8(value) {
      this.reserve(1)
      this.bytes[this.offset++] = value
    }

    u32(value) {
      this.reserve(4)
      this.view.setUint32(this.offset, value >>> 0, true)
      this.offset += 4
    }

    f64(value) {
      this.reserve(8)
      this.view.setFloat64(this.offset, value, true)
      this.offset += 8
    }

    // El motor no trae TextEncoder, así que codificamos UTF-8 a mano.
    str(value) {
      const text = String(value)
      this.reserve(4 + text.length * 3)
      const lengthAt = this.offset
      this.offset += 4
      const start = this.offset
      for (let i = 0; i < text.length; i++) {
        let code = text.charCodeAt(i)
        if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
          const low = text.charCodeAt(i + 1)
          if (low >= 0xdc00 && low <= 0xdfff) {
            code = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00)
            i++
          }
        }
        if (code < 0x80) {
          this.reserve(1)
          this.bytes[this.offset++] = code
        } else if (code < 0x800) {
          this.reserve(2)
          this.bytes[this.offset++] = 0xc0 | (code >> 6)
          this.bytes[this.offset++] = 0x80 | (code & 0x3f)
        } else if (code < 0x10000) {
          this.reserve(3)
          this.bytes[this.offset++] = 0xe0 | (code >> 12)
          this.bytes[this.offset++] = 0x80 | ((code >> 6) & 0x3f)
          this.bytes[this.offset++] = 0x80 | (code & 0x3f)
        } else {
          this.reserve(4)
          this.bytes[this.offset++] = 0xf0 | (code >> 18)
          this.bytes[this.offset++] = 0x80 | ((code >> 12) & 0x3f)
          this.bytes[this.offset++] = 0x80 | ((code >> 6) & 0x3f)
          this.bytes[this.offset++] = 0x80 | (code & 0x3f)
        }
      }
      this.view.setUint32(lengthAt, this.offset - start, true)
    }

    drain() {
      if (this.offset === 0) return
      native.flush(this.bytes, this.offset)
      this.offset = 0
    }
  }

  const writer = new CommandWriter(4096)

  // ------------------------------------------------------------- API de nodos
  //
  // Los ids los asigna JS: crear un nodo no espera respuesta del core.

  let nextNodeId = 1
  const listeners = new Map()

  const dom = {
    /// `kind` es una clave de KIND: 'View', 'Text', 'RawText'...
    createNode(kind) {
      const id = nextNodeId++
      const code = KIND[kind]
      if (code === undefined) throw new Error(`primitiva desconocida: ${kind}`)
      writer.u8(OP.CREATE_NODE)
      writer.u32(id)
      writer.u8(code)
      return id
    },

    destroyNode(id) {
      listeners.delete(id)
      writer.u8(OP.DESTROY_NODE)
      writer.u32(id)
    },

    insertChild(parent, child, index) {
      writer.u8(OP.INSERT_CHILD)
      writer.u32(parent)
      writer.u32(child)
      writer.u32(index)
    },

    removeChild(parent, child) {
      writer.u8(OP.REMOVE_CHILD)
      writer.u32(parent)
      writer.u32(child)
    },

    setStyle(id, name, value) {
      writer.u8(OP.SET_STYLE)
      writer.u32(id)
      writer.str(name)
      writer.str(value === null || value === undefined ? '' : value)
    },

    setProp(id, key, value) {
      if (value === null || value === undefined) {
        writer.u8(OP.SET_PROP_NULL)
        writer.u32(id)
        writer.str(key)
        return
      }
      switch (typeof value) {
        case 'number':
          writer.u8(OP.SET_PROP_NUM)
          writer.u32(id)
          writer.str(key)
          writer.f64(value)
          break
        case 'boolean':
          writer.u8(OP.SET_PROP_BOOL)
          writer.u32(id)
          writer.str(key)
          writer.u8(value ? 1 : 0)
          break
        default:
          writer.u8(OP.SET_PROP_STR)
          writer.u32(id)
          writer.str(key)
          writer.str(value)
      }
    },

    setText(id, text) {
      writer.u8(OP.SET_TEXT)
      writer.u32(id)
      writer.str(text)
    },

    setRoot(id) {
      writer.u8(OP.SET_ROOT)
      writer.u32(id)
    },

    listen(id, event, handler) {
      let byEvent = listeners.get(id)
      if (!byEvent) {
        byEvent = new Map()
        listeners.set(id, byEvent)
      }
      byEvent.set(event, handler)
      writer.u8(OP.SET_LISTENER)
      writer.u32(id)
      writer.str(event)
      writer.u8(1)
      return () => {
        byEvent.delete(event)
        writer.u8(OP.SET_LISTENER)
        writer.u32(id)
        writer.str(event)
        writer.u8(0)
      }
    }
  }

  global.__an_dom = dom

  // ---------------------------------------------------------- módulos nativos
  //
  // Una llamada nativa nunca bloquea: se manda, se guarda el resolvedor, y la
  // respuesta llega en este frame o en uno posterior. Es el mismo trato que el
  // búfer de comandos, en la otra dirección.

  const pendingCalls = new Map()

  global.__an_native = {
    call(module, method, args) {
      return new Promise((resolve, reject) => {
        let id
        try {
          id = native.invoke(module, method, JSON.stringify(args === undefined ? null : args))
        } catch (error) {
          reject(error)
          return
        }
        pendingCalls.set(id, { resolve, reject })
      })
    }
  }

  global.__an_settle = function (id, ok, payload) {
    const pending = pendingCalls.get(id)
    if (!pending) return
    pendingCalls.delete(id)
    let value
    try {
      value = JSON.parse(payload)
    } catch (error) {
      pending.reject(new Error(`respuesta nativa ilegible: ${payload}`))
      return
    }
    if (ok) {
      pending.resolve(value)
    } else {
      pending.reject(new Error(String(value)))
    }
  }

  // ------------------------------------------------------- estado entre recargas
  //
  // Una recarga tira el motor entero y levanta otro: los componentes son
  // nuevos y sus señales vuelven a su valor inicial. Lo que sobrevive es lo
  // que se apunte aquí — la ruta actual, el scroll, lo escrito en un
  // formulario— que en la práctica es casi todo lo que uno no quiere perder.
  //
  // No es el *fast refresh* de React Native: eso conserva los propios
  // componentes, y para eso hace falta cargar los módulos por separado y
  // sustituir la metadata de los que cambiaron.

  let restored = {}
  const hotSources = new Map()

  global.__an_hot = {
    /** Lo que este `key` valía antes de la última recarga, si valía algo. */
    read(key) {
      return Object.prototype.hasOwnProperty.call(restored, key) ? restored[key] : undefined
    },

    /** Apunta de dónde leer el valor de `key` cuando toque recargar. */
    register(key, read) {
      hotSources.set(key, read)
      return () => hotSources.delete(key)
    }
  }

  /// La llama el core justo antes de tirar el motor.
  global.__an_hot_state = function () {
    const state = {}
    for (const [key, read] of hotSources) {
      try {
        state[key] = read()
      } catch (error) {
        console.warn(`no se pudo guardar el estado de ${key}:`, error)
      }
    }
    return JSON.stringify(state)
  }

  /// La llama el core en el motor nuevo, antes de evaluar el bundle.
  global.__an_restore_hot_state = function (json) {
    try {
      restored = JSON.parse(json) || {}
    } catch {
      restored = {}
    }
  }

  // --------------------------------------------------------------- ciclo de frame

  /// Eventos nativos, antes que los timers: lo que tocó el usuario en este
  /// frame se ve reflejado en el mismo frame.
  global.__an_dispatch = function (targetId, name, payload) {
    const handler = listeners.get(targetId) && listeners.get(targetId).get(name)
    if (!handler) return
    try {
      handler(payload || {})
    } catch (error) {
      console.error(`manejador de ${name} sin capturar:`, error)
    }
  }

  global.__an_tick = function (now) {
    frameNow = now
    runTimers(now)
    runFrameCallbacks(now)
  }

  /// La llama Rust tras vaciar las microtareas: lo que hayan producido las
  /// promesas entra en este mismo frame, no en el siguiente.
  global.__an_drain = function () {
    writer.drain()
  }
})(globalThis)
