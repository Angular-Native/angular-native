//! The JS engine's contract. Nothing above this layer knows whether QuickJS,
//! V8 or JavaScriptCore is underneath.

use an_host::HostEvent;

#[derive(Debug)]
pub enum JsError {
    /// A JS exception, with its stack trace if there was one.
    Exception(String),
    /// A failure of the engine itself (memory, module not found...).
    Engine(String),
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsError::Exception(m) => write!(f, "excepción de JS: {m}"),
            JsError::Engine(m) => write!(f, "fallo del motor: {m}"),
        }
    }
}

impl std::error::Error for JsError {}

/// Where `console.*` ends up. On iOS it lands in `NSLog`; in tests, in a
/// `Vec`.
pub trait LogSink: 'static {
    fn log(&self, level: u8, message: &str);
}

/// The default sink: writes to stderr with the level up front.
pub struct StderrLog;

impl LogSink for StderrLog {
    fn log(&self, level: u8, message: &str) {
        let tag = match level {
            0 => "debug",
            2 => "warn",
            3 => "error",
            _ => "log",
        };
        eprintln!("[js {tag}] {message}");
    }
}

pub trait JsRuntime {
    /// Evaluates a file. `name` is only ever used in stack traces.
    fn eval(&mut self, name: &str, code: &str) -> Result<(), JsError>;

    /// Delivers native events to their handlers. It is called before the timers
    /// so that what the user touched shows up in this very frame.
    fn dispatch_events(&mut self, events: &[HostEvent]) -> Result<(), JsError>;

    /// One JS frame: timers that came due, microtasks until there are none
    /// left, and whatever command buffer came out of all that.
    fn tick(&mut self, now_ms: f64) -> Result<Vec<u8>, JsError>;

    /// Evaluates a new bundle on top of the one already running. Says whether
    /// the app could be stitched back together hot; if not, it all has to be
    /// reloaded.
    fn eval_hot(&mut self, name: &str, code: &str) -> Result<bool, JsError>;

    /// The state the app wants kept if it gets reloaded. It is asked for right
    /// before the engine is thrown away.
    fn take_hot_state(&mut self) -> String;

    /// Hands it back to the new engine, before the bundle is evaluated: the
    /// components read it while they are being built.
    fn restore_hot_state(&mut self, state: &str) -> Result<(), JsError>;
}
