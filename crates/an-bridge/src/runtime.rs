//! Contrato del motor JS. Nada por encima de esta capa sabe si debajo hay
//! QuickJS, V8 o JavaScriptCore.

use an_host::HostEvent;

#[derive(Debug)]
pub enum JsError {
    /// Excepción de JS, con su traza si la había.
    Exception(String),
    /// Fallo del propio motor (memoria, módulo no encontrado...).
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

/// A dónde va `console.*`. En iOS acaba en `NSLog`; en tests, a un `Vec`.
pub trait LogSink: 'static {
    fn log(&self, level: u8, message: &str);
}

/// Sumidero por defecto: escribe en stderr con el nivel delante.
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
    /// Evalúa un fichero. `name` solo se usa en las trazas.
    fn eval(&mut self, name: &str, code: &str) -> Result<(), JsError>;

    /// Entrega eventos nativos a sus manejadores. Se llama antes de los timers
    /// para que lo que tocó el usuario se vea en este mismo frame.
    fn dispatch_events(&mut self, events: &[HostEvent]) -> Result<(), JsError>;

    /// Un frame de JS: timers vencidos, microtareas hasta agotarlas, y el
    /// búfer de comandos que haya salido de todo ello.
    fn tick(&mut self, now_ms: f64) -> Result<Vec<u8>, JsError>;

    /// Estado que la app quiere conservar si la recargan. Se pide justo antes
    /// de tirar el motor.
    fn take_hot_state(&mut self) -> String;

    /// Se lo devuelve al motor nuevo, antes de evaluar el bundle: los
    /// componentes lo leen mientras se construyen.
    fn restore_hot_state(&mut self, state: &str) -> Result<(), JsError>;
}
