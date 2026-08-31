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

use crate::runtime::{JsError, JsRuntime, LogSink, StderrLog};

/// El prelude que convierte un intérprete pelado en algo utilizable.
const RUNTIME_JS: &str = include_str!("../../../packages/runtime/runtime.js");

pub struct QuickJsRuntime {
    // El orden importa: el contexto tiene que morir antes que el runtime.
    context: Context,
    runtime: Runtime,
    commands: Rc<RefCell<Vec<u8>>>,
}

impl QuickJsRuntime {
    pub fn new() -> Result<Self, JsError> {
        Self::with_log(Rc::new(StderrLog))
    }

    pub fn with_log(log: Rc<dyn LogSink>) -> Result<Self, JsError> {
        let runtime = Runtime::new().map_err(|e| JsError::Engine(e.to_string()))?;
        let context = Context::full(&runtime).map_err(|e| JsError::Engine(e.to_string()))?;
        let commands: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::with_capacity(4096)));
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

                ctx.globals()
                    .set("__an", native)
                    .map_err(|e| JsError::Engine(e.to_string()))?;
                Ok(())
            })
            .map_err(|e| e)?;

        let mut this = QuickJsRuntime { context, runtime, commands };
        this.eval("runtime.js", RUNTIME_JS)?;
        Ok(this)
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

    fn tick(&mut self, now_ms: f64) -> Result<Vec<u8>, JsError> {
        self.context.with(|ctx| -> Result<(), JsError> {
            let tick: Function = ctx
                .globals()
                .get("__an_tick")
                .map_err(|e| exception_message(&ctx, e))?;
            tick.call::<_, ()>((now_ms,)).map_err(|e| exception_message(&ctx, e))
        })?;

        self.drain_microtasks()?;

        self.context.with(|ctx| -> Result<(), JsError> {
            let drain: Function = ctx
                .globals()
                .get("__an_drain")
                .map_err(|e| exception_message(&ctx, e))?;
            drain.call::<_, ()>(()).map_err(|e| exception_message(&ctx, e))
        })?;

        Ok(std::mem::take(&mut *self.commands.borrow_mut()))
    }
}
