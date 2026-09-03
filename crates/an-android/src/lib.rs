//! The Android host.
//!
//! The core is the same one as on iOS: the shadow tree, the layout and the diff
//! have no idea which platform they are on. The only thing that changes is who
//! builds the views and who measures the text.
//!
//! The split between Rust and Kotlin is not the same one as on iOS. There Rust
//! talks to UIKit directly through `objc2`, because the Objective-C bridge is
//! cheap and typed. Here every call crosses JNI, so the surface is kept to a
//! minimum: Kotlin exposes a handful of methods on `AnHost` and Rust calls
//! those. View logic lives where it is natural to write it.

#[cfg(target_os = "android")]
mod builtins;
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
