//! The QuickJS backend. No JIT, which on iOS is forbidden outside WKWebView
//! anyway, and a startup measured in milliseconds instead of tens of them.
//!
//! The event loop is not its own: JS has no thread, it has a turn per frame.
//! The `CADisplayLink` calls `tick()`, and inside that timers and microtasks
//! run until there are none left. Nothing is left pending between frames
//! without the core knowing about it.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use an_host::HostEvent;
use rquickjs::function::Func;
use rquickjs::{CatchResultExt, Context, Ctx, Function, Object, Runtime, TypedArray};

use crate::modules::{ModuleRegistry, NativeModule};
use crate::runtime::{JsError, JsRuntime, LogSink, StderrLog};

/// The prelude that turns a bare interpreter into something usable.
const RUNTIME_JS: &str = include_str!("../../../packages/runtime/runtime.js");

pub struct QuickJsRuntime {
    // Order matters: the context has to die before the runtime does.
    context: Context,
    runtime: Runtime,
    commands: Rc<RefCell<Vec<u8>>>,
    log: Rc<dyn LogSink>,
    /// Rejections seen this turn that still have no handler.
    pending_rejections: Rc<RefCell<Vec<String>>>,
    modules: Rc<RefCell<ModuleRegistry>>,
}

impl QuickJsRuntime {
    pub fn new() -> Result<Self, JsError> {
        Self::with_log(Rc::new(StderrLog))
    }

    /// The engine's stack limit.
    ///
    /// QuickJS's factory setting falls short for Angular: a chain of twelve RxJS
    /// operators exhausts it, and the overflow throws nothing you can see — the
    /// subscription simply stops delivering values. The router, which chains
    /// seventeen, never managed to navigate at all.
    ///
    /// The real ceiling is not set here but by the thread: iOS's main one has
    /// 1 MB and it cannot be changed. Moving the engine onto a thread of its own
    /// with a big stack —what React Native does— is the real fix, and it drags
    /// the tree and the layout along with it.
    /// Measured: Angular's router needs a bit over 3 MB to complete one
    /// navigation. With 2 MB the transition gets seven events in and stops.
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
                        // The parameters go untyped and are pinned down inside:
                        // it is how rquickjs infers the `'js` lifetime of the
                        // view over the JS buffer.
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
                        // It returns the call's identifier, not the result: JS
                        // never blocks waiting on native.
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

        // A promise rejected with no `catch` vanishes without a trace in
        // QuickJS. In a framework that is unacceptable: half of Angular's stack
        // is promises, and a silent failure shows up as a blank screen with no
        // hint of anything.
        //
        // The engine reports it the moment it is rejected, not at the end of the
        // turn, and the `catch` may be hooked up afterwards: Angular uses
        // rejections as internal flow control. So they are written down and
        // reported as the tick closes, minus the ones that ended up handled.
        let pending: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let tracked = pending.clone();
        runtime.set_host_promise_rejection_tracker(Some(Box::new(
            move |ctx, _promise, reason, is_handled| {
                if is_handled {
                    // Somebody put a `catch` on it: it stops being a problem.
                    tracked.borrow_mut().pop();
                    return;
                }
                let text = match reason.as_exception() {
                    Some(exception) => {
                        let message =
                            exception.message().unwrap_or_else(|| "no message".to_owned());
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

    /// Registers a native module. It has to be done before the app is
    /// evaluated.
    pub fn register_module(&mut self, module: Box<dyn NativeModule>) {
        self.modules.borrow_mut().register(module);
    }

    /// Says why a module that is not registered is not there. It is appended to
    /// the rejection; see `ModuleRegistry::explain_absent`.
    pub fn explain_absent_modules(&mut self, note: impl Into<String>) {
        self.modules.borrow_mut().explain_absent(note);
    }

    /// Hands JS the module answers that are already in.
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

    /// Empties the microtask queue. A promise resolved during the frame lands
    /// in that very frame, not the next one.
    fn drain_microtasks(&mut self) -> Result<(), JsError> {
        loop {
            match self.runtime.execute_pending_job() {
                Ok(true) => continue,
                Ok(false) => return Ok(()),
                Err(job) => {
                    // The exception stayed behind in the job's context: it has
                    // to be pulled out of there or all anyone knows is that
                    // "something failed".
                    let message = job.0.with(|ctx| {
                        let value = ctx.catch();
                        match value.as_exception() {
                            Some(exception) => {
                                let msg = exception
                                    .message()
                                    .unwrap_or_else(|| "no message".to_owned());
                                match exception.stack() {
                                    Some(stack) => format!("{msg}\n{stack}"),
                                    None => msg,
                                }
                            }
                            None => format!("{value:?}"),
                        }
                    });
                    return Err(JsError::Exception(format!(
                        "uncaught microtask: {message}"
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
            let message = exception.message().unwrap_or_else(|| "no message".to_owned());
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

    fn eval_hot(&mut self, name: &str, code: &str) -> Result<bool, JsError> {
        self.context.with(|ctx| {
            // The flag is set to `false` before evaluating: if the new bundle
            // blows up halfway through, whatever was left in the global must not
            // make anyone believe it went well.
            ctx.globals()
                .set("__anHotOk", false)
                .map_err(|e| JsError::Engine(format!("{name}: {e}")))?;
            ctx.eval::<(), _>(code)
                .catch(&ctx)
                .map_err(|e| JsError::Exception(format!("{name}: {e}")))?;
            Ok(ctx.globals().get::<_, bool>("__anHotOk").unwrap_or(false))
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
                eprintln!("angular-native: the state could not be saved: {error}");
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

        // Module answers go in before the microtasks are drained, so that
        // whatever depends on them resolves in this very frame.
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
            self.log.log(3, &format!("uncaught rejected promise: {rejection}"));
        }

        Ok(std::mem::take(&mut *self.commands.borrow_mut()))
    }
}
