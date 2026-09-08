'use strict'
// angular-native's JS runtime.
//
// It is the least the embedded engine needs in order not to be a bare
// interpreter: a console, timers, and the command writer that talks to the Rust
// core. Angular's `Renderer2` is mounted on top of this.
//
// Rust injects three functions before evaluating this file:
//   __an.log(level, message)   output to the system console
//   __an.now()                 monotonic milliseconds since startup
//   __an.flush(bytes, length)  hands the command buffer to the core
;(function (global) {
  const native = global.__an
  if (!native) {
    throw new Error('runtime.js needs Rust to have injected __an')
  }

  // ------------------------------------------------------------------ console

  function format(args) {
    let out = ''
    for (let i = 0; i < args.length; i++) {
      if (i > 0) out += ' '
      const value = args[i]
      if (typeof value === 'string') {
        out += value
      } else if (value instanceof Error) {
        // QuickJS puts only the frames in `stack`, without the message line:
        // printing `stack` on its own loses what actually failed.
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

  // ------------------------------------------------------------------- timers
  //
  // The queue lives in JS and the core drains it once per frame. That way there
  // are no threads and no clocks competing: the only time that exists is the
  // vsync's.

  let nextTimerId = 1
  const timers = new Map()
  // The timers' clock: the one each frame brings along, not the system's. That
  // way the app's time advances in vsync steps, it is deterministic, and a test
  // can simulate ten seconds without waiting for them.
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

  // requestAnimationFrame is literally the frame: Angular's zoneless scheduler
  // uses it to batch change detection, and here it lines up with the vsync with
  // nothing approximated.
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
    // The map is swapped before running: a callback that asks for another frame
    // goes into the next one and not into this one, or the loop never ends.
    const pending = frameCallbacks
    frameCallbacks = new Map()
    for (const fn of pending.values()) {
      try {
        fn(now)
      } catch (error) {
        console.error('uncaught requestAnimationFrame:', error)
      }
    }
  }

  function runTimers(now) {
    if (timers.size === 0) return
    // It is resolved over a copy: a callback can add or remove timers.
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
        console.error('uncaught timer:', error)
      }
    }
  }

  // -------------------------------------------- the web APIs that are needed
  //
  // The engine is a bare ECMAScript interpreter: it brings nothing of the web
  // platform with it. Most of those APIs have no business here, but a few sit on
  // the code path of libraries that do get used. Angular's router, to take the
  // obvious one, creates an `AbortController` per navigation, and since it
  // subscribes swallowing the errors, its absence produces no failure at all: it
  // simply never navigates.

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
          console.error('uncaught abort listener:', error)
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

  // ------------------------------------------------------- the command buffer

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

  // The name of each primitive and the code it travels under. The code is the
  // contract with Rust; the name is only used on this side.
  //
  // Two of them are not called here what they are called over there: the
  // `<an-select>` tag maps to `Select` and `<an-textarea>` to `Textarea`, while
  // the core still calls them `Picker` and `TextEditor` —names that were picked
  // back when the tag could not be called `Select` or `TextArea` because Angular
  // does not self-close anything named after an HTML element, and that can no
  // longer be changed without touching the Rust enum and the three hosts—. This
  // is the one place where the two vocabularies meet, and `check-kinds.sh`
  // verifies that they do not drift any further apart.
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
    Icon: 15,
    SegmentedControl: 16,
    Stepper: 17,
    SearchBar: 18,
    Select: 19,
    DatePicker: 20,
    NavigationBar: 21,
    Textarea: 22,
    WebView: 23,
    MapView: 24,
    VideoView: 25,
    Custom: 26
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

    // The engine has no TextEncoder, so UTF-8 is encoded by hand.
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

  // -------------------------------------------------------------- the node API
  //
  // The ids are handed out by JS: creating a node waits for no answer from the
  // core.

  let nextNodeId = 1
  const listeners = new Map()

  const dom = {
    /// `kind` is a key of KIND: 'View', 'Text', 'RawText'...
    createNode(kind) {
      const id = nextNodeId++
      const code = KIND[kind]
      if (code === undefined) throw new Error(`unknown primitive: ${kind}`)
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

  // ------------------------------------------------------------ native modules
  //
  // A native call never blocks: it is sent, the resolver is kept, and the answer
  // arrives in this frame or in a later one. It is the same bargain as the
  // command buffer, in the other direction.

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
    },

    /// Subscribes to what a module says without being asked.
    ///
    /// A call has one answer; an event has none or a thousand, so it needs a
    /// road of its own. Several handlers may listen to the same event and each
    /// gets its own unsubscriber — a module has no idea who is listening, and
    /// one component unsubscribing must not deafen another.
    on(module, event, handler) {
      const key = module + '\u0000' + event
      let handlers = moduleListeners.get(key)
      if (!handlers) {
        handlers = new Set()
        moduleListeners.set(key, handlers)
      }
      handlers.add(handler)
      return function off() {
        const current = moduleListeners.get(key)
        if (!current) return
        current.delete(handler)
        if (current.size === 0) moduleListeners.delete(key)
      }
    },

    /// The URLs the app was opened with, read without waiting for a frame.
    ///
    /// A deep link can reach the shell before this bundle has been evaluated:
    /// the system starts the process *because* of the URL. A native call would
    /// answer a frame later, and by then the router has already put the first
    /// screen up. This is the one thing that has to be synchronous, and it can
    /// be: the queue is a Rust global sitting on this very thread.
    ///
    /// Calling it is also what says "from now on I am listening": whatever
    /// arrives afterwards comes through `on('deeplink', 'url')` instead. So it
    /// is called once, after subscribing, and never in the other order.
    deepLinks() {
      if (typeof native.deepLinks !== 'function') return []
      try {
        const parsed = JSON.parse(native.deepLinks())
        return Array.isArray(parsed) ? parsed : []
      } catch (error) {
        console.error('the deep links could not be read:', error)
        return []
      }
    }
  }

  /// Keyed by module and event name together. A `Set` per key, so subscribing
  /// twice with the same function is idempotent and unsubscribing is cheap.
  const moduleListeners = new Map()

  global.__an_settle = function (id, ok, payload) {
    const pending = pendingCalls.get(id)
    if (!pending) return
    pendingCalls.delete(id)
    let value
    try {
      value = JSON.parse(payload)
    } catch (error) {
      pending.reject(new Error(`unreadable native answer: ${payload}`))
      return
    }
    if (ok) {
      pending.resolve(value)
    } else {
      pending.reject(new Error(String(value)))
    }
  }

  // ------------------------------------------------------- state across reloads
  //
  // A reload throws the whole engine away and stands another one up: the
  // components are new and their signals are back to their initial value. What
  // survives is whatever is registered here —the current route, the scroll
  // position, what has been typed into a form— which in practice is very nearly
  // everything one does not want to lose.
  //
  // It is not React Native's *fast refresh*: that one keeps the components
  // themselves, and doing that takes loading the modules separately and
  // replacing the metadata of the ones that changed.

  let restored = {}
  const hotSources = new Map()

  global.__an_hot = {
    /** What this `key` was worth before the last reload, if it was worth anything. */
    read(key) {
      return Object.prototype.hasOwnProperty.call(restored, key) ? restored[key] : undefined
    },

    /** Registers where to read the value of `key` from when the time to reload comes. */
    register(key, read) {
      hotSources.set(key, read)
      return () => hotSources.delete(key)
    }
  }

  /// Called by the core right before it throws the engine away.
  global.__an_hot_state = function () {
    const state = {}
    for (const [key, read] of hotSources) {
      try {
        state[key] = read()
      } catch (error) {
        console.warn(`the state of ${key} could not be saved:`, error)
      }
    }
    return JSON.stringify(state)
  }

  /// Called by the core on the new engine, before evaluating the bundle.
  global.__an_restore_hot_state = function (json) {
    try {
      restored = JSON.parse(json) || {}
    } catch {
      restored = {}
    }
  }

  // ----------------------------------------------------------- the frame cycle

  /// Native events, ahead of the timers: whatever the user touched in this frame
  /// is reflected in that same frame.
  global.__an_dispatch = function (targetId, name, payload) {
    const handler = listeners.get(targetId) && listeners.get(targetId).get(name)
    if (!handler) return
    try {
      handler(payload || {})
    } catch (error) {
      console.error(`uncaught ${name} handler:`, error)
    }
  }

  /// An event from a native module, at the top of a frame.
  ///
  /// A handler that throws is reported and the rest still run: one component's
  /// bug is not a reason for every other subscriber to miss the event. The list
  /// is copied before iterating, because a handler is allowed to unsubscribe
  /// itself — which is exactly what a "once" wrapper does.
  global.__an_module_event = function (module, event, payload) {
    const handlers = moduleListeners.get(module + '\u0000' + event)
    if (!handlers || handlers.size === 0) return
    let value
    try {
      value = JSON.parse(payload)
    } catch (error) {
      console.error(`unreadable event payload from ${module}.${event}: ${payload}`)
      return
    }
    for (const handler of [...handlers]) {
      try {
        handler(value)
      } catch (error) {
        console.error(`uncaught ${module}.${event} handler:`, error)
      }
    }
  }

  global.__an_tick = function (now) {
    frameNow = now
    runTimers(now)
    runFrameCallbacks(now)
  }

  /// Called by Rust after draining the microtasks: whatever the promises
  /// produced goes into this very frame, not into the next one.
  global.__an_drain = function () {
    writer.drain()
  }
})(globalThis)
