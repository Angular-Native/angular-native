//! Puente con el motor JS.
//!
//! El motor concreto está detrás de un trait: QuickJS ahora, V8 después en
//! Android sin tocar nada de lo que hay por encima. Lo que sí es fijo es el
//! protocolo: JS acumula mutaciones en un búfer binario y lo entrega una vez
//! por frame.

pub mod modules;
pub mod plugins;
pub mod protocol;
pub mod quickjs;
pub mod runtime;
pub mod worker;

pub use modules::{ModuleRegistry, ModuleResult, NativeModule, Responder};
pub use plugins::{HostPlugin, PluginBridge, PluginCall};
pub use protocol::{apply, Encoder, ProtocolError};
pub use quickjs::QuickJsRuntime;
pub use worker::{Reply, Request, RuntimeWorker};
pub use runtime::{JsError, JsRuntime, LogSink};
