//! Host de Android.
//!
//! El núcleo es el mismo que en iOS: shadow tree, layout y diff no saben en
//! qué plataforma están. Lo único que cambia es quién monta las vistas y quién
//! mide el texto.
//!
//! El reparto entre Rust y Kotlin no es el mismo que en iOS. Allí Rust habla
//! con UIKit directamente por `objc2`, porque el puente Objective-C es barato
//! y está tipado. Aquí cada llamada cruza JNI, así que la superficie se
//! mantiene mínima: Kotlin expone un puñado de métodos sobre `AnHost` y Rust
//! llama a esos. La lógica de vistas vive donde es natural escribirla.

#[cfg(target_os = "android")]
mod host;
#[cfg(target_os = "android")]
mod jni_bridge;
#[cfg(target_os = "android")]
mod logging;
#[cfg(target_os = "android")]
mod measure;
#[cfg(target_os = "android")]
mod modules;
#[cfg(target_os = "android")]
mod plugins;

#[cfg(target_os = "android")]
pub use host::JniHost;
#[cfg(target_os = "android")]
pub use measure::JniMeasurer;
