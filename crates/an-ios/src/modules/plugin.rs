//! El lado iOS de los plugins.
//!
//! Un plugin no se compila dentro del core: es Swift que trae un paquete npm y
//! que `an-cli` enlaza en el `.app`. Rust no sabe qué hace ni qué métodos
//! tiene; solo lleva la llamada al hilo principal y trae la respuesta.
//!
//! Todo lo que hay aquí es global al proceso y no al runtime. El registro
//! ocurre antes de `an_runtime_new` —Swift no tiene dónde guardarlo si no— y
//! sobrevive a un reinicio en caliente: el `.app` es el mismo, los plugins
//! también.

use std::ffi::{c_char, CStr, CString};
use std::sync::{Arc, Mutex, OnceLock};

use an_bridge::plugins::{HostPlugin, PluginBridge};

/// Lo que Swift instala para que Rust pueda avisarle de una llamada.
///
/// Las cadenas son prestadas y solo valen mientras dure la llamada: el shell
/// las copia a `String` nada más entrar.
pub type DispatchFn =
    unsafe extern "C" fn(id: u64, module: *const c_char, method: *const c_char, args: *const c_char);

fn bridge() -> &'static Arc<PluginBridge> {
    static BRIDGE: OnceLock<Arc<PluginBridge>> = OnceLock::new();
    BRIDGE.get_or_init(PluginBridge::new)
}

fn names() -> &'static Mutex<Vec<String>> {
    static NAMES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    NAMES.get_or_init(|| Mutex::new(Vec::new()))
}

fn dispatch() -> &'static Mutex<Option<DispatchFn>> {
    static DISPATCH: OnceLock<Mutex<Option<DispatchFn>>> = OnceLock::new();
    DISPATCH.get_or_init(|| Mutex::new(None))
}

/// Un módulo por plugin registrado, listo para viajar al hilo del motor.
///
/// Se construyen en `an_runtime_new`, después de que Swift haya registrado los
/// suyos y antes de arrancar el worker.
pub fn host_plugins() -> Vec<HostPlugin> {
    names()
        .lock()
        .expect("registro de plugins envenenado")
        .iter()
        .map(|name| HostPlugin::new(name, bridge().clone()))
        .collect()
}

/// Lleva al shell las llamadas que se hayan acumulado. Se llama desde
/// `an_runtime_frame`, que corre en el hilo principal: es justo donde un
/// plugin puede tocar UIKit.
pub fn pump() {
    let calls = bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let Some(dispatch) = *dispatch().lock().expect("despachador envenenado") else {
        // Hay plugins registrados pero nadie que los atienda. Es un fallo de
        // montaje del shell, y callarlo dejaría la promesa colgada sin pista.
        for call in calls {
            let _ = bridge().reject(
                call.id,
                "el shell de iOS no instaló el despachador de plugins",
            );
        }
        return;
    };
    for call in calls {
        let (Ok(module), Ok(method), Ok(args)) = (
            CString::new(call.module.as_str()),
            CString::new(call.method.as_str()),
            CString::new(call.args.as_str()),
        ) else {
            let _ = bridge().reject(call.id, "los argumentos llevaban un cero dentro");
            continue;
        };
        unsafe { dispatch(call.id, module.as_ptr(), method.as_ptr(), args.as_ptr()) };
    }
}

/// Da de alta un plugin por su nombre de módulo. Lo llama el registro de Swift
/// al arrancar, antes de crear el runtime.
///
/// # Safety
/// `name` tiene que ser una cadena C válida y terminada en cero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_register(name: *const c_char) {
    if name.is_null() {
        return;
    }
    let Ok(name) = (unsafe { CStr::from_ptr(name) }).to_str() else { return };
    let mut names = names().lock().expect("registro de plugins envenenado");
    // Registrar dos veces el mismo nombre daría dos módulos con el mismo
    // nombre en el registro, y el segundo no lo vería nadie.
    if names.iter().any(|existing| existing == name) {
        eprintln!("angular-native: el plugin {name:?} ya estaba registrado");
        return;
    }
    names.push(name.to_owned());
}

/// Instala el despachador. Pasar `None` lo quita.
#[unsafe(no_mangle)]
pub extern "C" fn an_plugin_set_dispatch(callback: Option<DispatchFn>) {
    *dispatch().lock().expect("despachador envenenado") = callback;
}

/// Contesta a una llamada. `json` es el valor de vuelta ya serializado.
/// Devuelve 0 si la llamada existía y -1 si no.
///
/// # Safety
/// `json` tiene que ser una cadena C válida y terminada en cero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_resolve(id: u64, json: *const c_char) -> i32 {
    let text = match unsafe { read(json) } {
        Some(text) => text,
        None => return report(bridge().reject(id, "el plugin contestó con una cadena inválida")),
    };
    report(bridge().resolve(id, &text))
}

/// Rechaza una llamada. Devuelve 0 si la llamada existía y -1 si no.
///
/// # Safety
/// `message` tiene que ser una cadena C válida y terminada en cero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_reject(id: u64, message: *const c_char) -> i32 {
    let text = unsafe { read(message) }.unwrap_or_else(|| "el plugin falló".to_owned());
    report(bridge().reject(id, &text))
}

/// # Safety
/// `text` tiene que ser nulo o una cadena C válida.
unsafe fn read(text: *const c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(text) }.to_str().ok().map(str::to_owned)
}

fn report(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("angular-native: {message}");
            -1
        }
    }
}
