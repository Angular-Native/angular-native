//! iOS's native modules.
//!
//! `device` is compiled into the core; the plugins come from outside and all
//! they have here is the postman that carries the call to them.

mod device;
mod plugin;

pub use device::DeviceModule;
pub use plugin::{host_plugins, pump};
