//! Puente con el motor JS.
//!
//! El motor concreto está detrás de un trait: QuickJS ahora, V8 después en
//! Android sin tocar nada de lo que hay por encima. Lo que sí es fijo es el
//! protocolo: JS acumula mutaciones en un búfer binario y lo entrega una vez
//! por frame.

pub mod protocol;
pub mod quickjs;
pub mod runtime;

pub use protocol::{apply, Encoder, ProtocolError};
pub use quickjs::QuickJsRuntime;
pub use runtime::{JsError, JsRuntime, LogSink};
