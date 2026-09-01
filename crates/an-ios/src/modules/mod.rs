//! Módulos nativos de iOS.
//!
//! `device` se compila dentro del core; los plugins vienen de fuera y solo
//! tienen aquí al cartero que les lleva la llamada.

mod device;
mod plugin;

pub use device::DeviceModule;
pub use plugin::{host_plugins, pump};
