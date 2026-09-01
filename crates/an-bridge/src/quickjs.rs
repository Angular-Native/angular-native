//! Backend QuickJS. Sin JIT, que en iOS está prohibido de todas formas fuera
//! de WKWebView, y con un arranque de milisegundos en vez de decenas.
//!
//! El bucle de eventos no es propio: JS no tiene hilo, tiene un turno por
//! frame. El `CADisplayLink` llama a `tick()`, y ahí dentro se ejecutan
//! temporizadores y microtareas hasta agotarlas. Nada queda pendiente entre
//! frames sin que el core lo sepa.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use an_host::HostEvent;
use rquickjs::function::Func;
use rquickjs::{CatchResultExt, Context, Ctx, Function, Object, Runtime, TypedArray};

use crate::modules::{ModuleRegistry, NativeModule};
use crate::runtime::{JsError, JsRuntime, LogSink, StderrLog};

/// El prelude que convierte un intérprete pelado en algo utilizable.
const RUNTIME_JS: &str = include_str!("../../../packages/runtime/runtime.js");

pub struct QuickJsRuntime {
    // El orden importa: el contexto tiene que morir antes que el runtime.
    context: Context,
    runtime: Runtime,
    commands: Rc<RefCell<Vec<u8>>>,
    log: Rc<dyn LogSink>,
    /// Rechazos vistos en este turno que todavía no tienen manejador.
    pending_rejections: Rc<RefCell<Vec<String>>>,
    modules: Rc<RefCell<ModuleRegistry>>,
}

impl QuickJsRuntime {
    pub fn new() -> Result<Self, JsError> {
        Self::with_log(Rc::new(StderrLog))
    }

    /// Límite de pila del motor.
    ///
    /// El de fábrica de QuickJS se queda corto para Angular: una cadena de
    /// doce operadores de RxJS ya lo agota, y el desbordamiento no lanza nada
    /// visible — la suscripción simplemente no entrega valores. El router,
    /// que encadena diecisiete, no llegaba a navegar nunca.
    ///
    /// El techo real no lo pone esto sino el hilo: el principal de iOS tiene
    /// 1 MB y no se puede cambiar. Mover el motor a un hilo propio con pila
    /// grande —lo que hace React Native— es la solución de verdad, y arrastra
    /// mover con él el árbol y el layout.
    /// Medido: el router de Angular necesita algo más de 3 MB para completar
    /// una navegación. Con 2 MB la transición avanza siete eventos y se para.
    pub const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

    pub fn with_log(log: Rc<dyn LogSink>) -> Result<Self, JsError> {
        Self::with_options(log, Self::DEFAULT_STACK_SIZE)
    }

    pub fn with_options(log: Rc<dyn LogSink>, stack_size: usize) -> Result<Self, JsError> {
        let runtime = Runtime::new().map_err(|e| JsError::Engine(e.to_string()))?;
        runtime.set_max_stack_size(stack_size);
        let context = Context::full(&runtime).map_err(|e| JsError::Engine(e.to_string()))?;
        let commands: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::with_capacity(4096)));
        let modules: Rc<RefCell<ModuleRegistry>> = Rc::new(RefCell::new(ModuleRegistry::new()));
        let start = Instant::now();

        context
            .with(|ctx| -> Result<(), JsError> {
                let native = Object::new(ctx.clone()).map_err(|e| JsError::Engine(e.to_string()))?;

                let sink = log.clone();
                native
                    .set(
                        "log",
                        Func::from(move |level: i32, message: String| {
                            sink.log(level.clamp(0, 3) as u8, &message);
                        }),
                    )
                    .map_err(|e| JsError::Engine(e.to_string()))?;

                native
                    .set(
                        "now",
                        Func::from(move || start.elapsed().as_secs_f64() * 1000.0),
                    )
                    .map_err(|e| JsError::Engine(e.to_string()))?;

                let sink = commands.clone();
                native
                    .set(
                        "flush",
                        // Los parámetros van sin tipo y se fijan dentro: es la
                        // forma de que rquickjs infiera la vida `'js` de la
                        // vista sobre el búfer de JS.
                        Func::from(move |data, length| {
                            struct Args<'js>(TypedArray<'js, u8>, usize);
                            let Args(data, length) = Args(data, length);
                            let bytes: &[u8] = data.as_ref();
                            let end = length.min(bytes.len());
                            sink.borrow_mut().extend_from_slice(&bytes[..end]);
                        }),
                    )
                    .map_err(|e| JsError::Engine(e.to_string()))?;

                let registry = modules.clone();
                native
                    .set(
                        "invoke",
                        // Devuelve el identificador de la llamada, no el
                        // resultado: JS no bloquea nunca esperando a nativo.
                        Func::from(move |module: String, method: String, args: String| {
                            let parsed = serde_json::from_str(&args).unwrap_or(serde_json::Value::Null);
                            registry.borrow_mut().invoke(&module, &method, parsed) as f64
                        }),
                    )
                    .map_err(|e| JsError::Engine(e.to_string()))?;

                ctx.globals()
                    .set("__an", native)
                    .map_err(|e| JsError::Engine(e.to_string()))?;
                Ok(())
            })
            .map_err(|e| e)?;

        // Una promesa rechazada sin `catch` desaparece sin dejar rastro en
        // QuickJS. En un framework eso es inaceptable: media pila de Angular
        // son promesas, y un fallo silencioso se manifiesta como una pantalla
        // en blanco sin ninguna pista.
        //
        // El motor avisa en cuanto se rechaza, no al final del turno, y el
        // `catch` puede engancharse después: Angular usa rechazos como control
        // de flujo interno. Así que se apuntan y se reportan al cerrar el tick,
        // descontando los que acabaron manejados.
        let pending: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let tracked = pending.clone();
        runtime.set_host_promise_rejection_tracker(Some(Box::new(
            move |ctx, _promise, reason, is_handled| {
                if is_handled {
                    // Alguien le puso un `catch`: deja de ser un problema.
                    tracked.borrow_mut().pop();
                    return;
                }
                let text = match reason.as_exception() {
                    Some(exception) => {
                        let message =
                            exception.message().unwrap_or_else(|| "sin mensaje".to_owned());
                        match exception.stack() {
                            Some(stack) => format!("{message}\n{stack}"),
                            None => message,
                        }
                    }
                    None => ctx
                        .json_stringify(reason.clone())
                        .ok()
                        .flatten()
                        .and_then(|s| s.to_string().ok())
                        .unwrap_or_else(|| format!("{reason:?}")),
                };
                tracked.borrow_mut().push(text);
            },
        )));

        let mut this =
            QuickJsRuntime { context, runtime, commands, log, pending_rejections: pending, modules };
        this.eval("runtime.js", RUNTIME_JS)?;
        Ok(this)
    }

    /// Da de alta un módulo nativo. Tiene que hacerse antes de evaluar la app.
    pub fn register_module(&mut self, module: Box<dyn NativeModule>) {
        self.modules.borrow_mut().register(module);
    }

    /// Entrega a JS las respuestas de módulos que ya estén listas.
    fn settle_module_calls(&mut self) -> Result<(), JsError> {
        let answers = self.modules.borrow_mut().drain();
        if answers.is_empty() {
            return Ok(());
        }
        self.context.with(|ctx| -> Result<(), JsError> {
            let settle: Function = ctx
                .globals()
                .get("__an_settle")
                .map_err(|e| exception_message(&ctx, e))?;
            for (id, result) in answers {
                let (ok, payload) = match result {
                    Ok(value) => (true, value.to_string()),
                    Err(message) => (false, serde_json::Value::String(message).to_string()),
                };
                settle
                    .call::<_, ()>((id as f64, ok, payload))
                    .map_err(|e| exception_message(&ctx, e))?;
            }
            Ok(())
        })
    }

    /// Vacía la cola de microtareas. Una promesa resuelta durante el frame
    /// entra en este mismo frame, no en el siguiente.
    fn drain_microtasks(&mut self) -> Result<(), JsError> {
        loop {
            match self.runtime.execute_pending_job() {
                Ok(true) => continue,
                Ok(false) => return Ok(()),
                Err(job) => {
                    // La excepción se quedó en el contexto del trabajo: hay
                    // que sacarla de ahí o solo se sabe que "algo falló".
                    let message = job.0.with(|ctx| {
                        let value = ctx.catch();
                        match value.as_exception() {
                            Some(exception) => {
                                let msg = exception
                                    .message()
                                    .unwrap_or_else(|| "sin mensaje".to_owned());
                                match exception.stack() {
                                    Some(stack) => format!("{msg}\n{stack}"),
                                    None => msg,
                                }
                            }
                            None => format!("{value:?}"),
                        }
                    });
                    return Err(JsError::Exception(format!(
                        "microtarea sin capturar: {message}"
                    )));
                }
            }
        }
    }
}

fn exception_message(ctx: &Ctx, error: rquickjs::Error) -> JsError {
    if !error.is_exception() {
        return JsError::Engine(error.to_string());
    }
    let value = ctx.catch();
    let text = match value.as_exception() {
        Some(exception) => {
            let message = exception.message().unwrap_or_else(|| "sin mensaje".to_owned());
            match exception.stack() {
                Some(stack) => format!("{message}\n{stack}"),
                None => message,
            }
        }
        None => format!("{value:?}"),
    };
    JsError::Exception(text)
}

impl JsRuntime for QuickJsRuntime {
    fn eval(&mut self, name: &str, code: &str) -> Result<(), JsError> {
        self.context.with(|ctx| {
            ctx.eval::<(), _>(code)
                .catch(&ctx)
                .map_err(|e| JsError::Exception(format!("{name}: {e}")))
        })
    }

    fn dispatch_events(&mut self, events: &[HostEvent]) -> Result<(), JsError> {
        if events.is_empty() {
            return Ok(());
        }
        self.context.with(|ctx| -> Result<(), JsError> {
            let dispatch: Function = ctx
                .globals()
                .get("__an_dispatch")
                .map_err(|e| exception_message(&ctx, e))?;
            for event in events {
                let payload = Object::new(ctx.clone()).map_err(|e| exception_message(&ctx, e))?;
                for (key, value) in &event.payload {
                    let set = match value {
                        an_core::PropValue::Number(n) => payload.set(key.as_str(), *n),
                        an_core::PropValue::Bool(b) => payload.set(key.as_str(), *b),
                        an_core::PropValue::Str(s) => payload.set(key.as_str(), s.as_str()),
                        an_core::PropValue::Color(c) => payload.set(key.as_str(), *c),
                        an_core::PropValue::Null => payload.set(key.as_str(), ()),
                    };
                    set.map_err(|e| exception_message(&ctx, e))?;
                }
                dispatch
                    .call::<_, ()>((event.target, event.name.as_str(), payload))
                    .map_err(|e| exception_message(&ctx, e))?;
            }
            Ok(())
        })
    }

    fn take_hot_state(&mut self) -> String {
        self.context
            .with(|ctx| -> Result<String, JsError> {
                let collect: Function = ctx
                    .globals()
                    .get("__an_hot_state")
                    .map_err(|e| exception_message(&ctx, e))?;
                collect.call::<_, String>(()).map_err(|e| exception_message(&ctx, e))
            })
            .unwrap_or_else(|error| {
                eprintln!("angular-native: no se pudo guardar el estado: {error}");
                "{}".to_owned()
            })
    }

    fn restore_hot_state(&mut self, state: &str) -> Result<(), JsError> {
        self.context.with(|ctx| -> Result<(), JsError> {
            let restore: Function = ctx
                .globals()
                .get("__an_restore_hot_state")
                .map_err(|e| exception_message(&ctx, e))?;
            restore.call::<_, ()>((state,)).map_err(|e| exception_message(&ctx, e))
        })
    }

    fn tick(&mut self, now_ms: f64) -> Result<Vec<u8>, JsError> {
        self.context.with(|ctx| -> Result<(), JsError> {
            let tick: Function = ctx
                .globals()
                .get("__an_tick")
                .map_err(|e| exception_message(&ctx, e))?;
            tick.call::<_, ()>((now_ms,)).map_err(|e| exception_message(&ctx, e))
        })?;

        // Las respuestas de módulos entran antes de vaciar microtareas, para
        // que lo que dependa de ellas se resuelva en este mismo frame.
        self.settle_module_calls()?;
        self.drain_microtasks()?;

        self.context.with(|ctx| -> Result<(), JsError> {
            let drain: Function = ctx
                .globals()
                .get("__an_drain")
                .map_err(|e| exception_message(&ctx, e))?;
            drain.call::<_, ()>(()).map_err(|e| exception_message(&ctx, e))
        })?;

        for rejection in self.pending_rejections.borrow_mut().drain(..) {
            self.log.log(3, &format!("promesa rechazada sin capturar: {rejection}"));
        }

        Ok(std::mem::take(&mut *self.commands.borrow_mut()))
    }
}
