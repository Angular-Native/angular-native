//! The bridge to the JS engine.
//!
//! Which engine it is sits behind a trait: QuickJS now, V8 later on Android
//! without touching a thing above it. What is fixed is the protocol: JS piles
//! mutations up in a binary buffer and hands it over once per frame.

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
