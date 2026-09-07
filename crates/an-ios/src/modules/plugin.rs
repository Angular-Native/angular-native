//! The iOS side of plugins.
//!
//! A plugin is not compiled into the core: it is Swift that an npm package
//! brings and that `an-cli` links into the `.app`. Rust does not know what it
//! does or what methods it has; it only carries the call to the main thread
//! and brings the answer back.
//!
//! Everything here is global to the process and not to the runtime.
//! Registration happens before `an_runtime_new` —Swift has nowhere to keep it
//! otherwise— and it survives a hot restart: the `.app` is the same one, and so
//! are the plugins.

use std::ffi::{c_char, CStr, CString};
use std::sync::{Arc, Mutex, OnceLock};

use an_bridge::plugins::{HostPlugin, PluginBridge};

/// What Swift installs so that Rust can tell it about a call.
///
/// The strings are borrowed and only hold for the duration of the call: the
/// shell copies them into `String`s the moment it is entered.
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

/// One module per registered plugin, ready to travel to the engine's thread.
///
/// They are built in `an_runtime_new`, after Swift has registered its own and
/// before the worker is started.
pub fn host_plugins() -> Vec<HostPlugin> {
    names()
        .lock()
        .expect("poisoned plugin registry")
        .iter()
        .map(|name| HostPlugin::new(name, bridge().clone()))
        .collect()
}

/// Carries the queued calls over to the shell. It is called from
/// `an_runtime_frame`, which runs on the main thread: exactly where a plugin
/// may touch UIKit.
pub fn pump() {
    let calls = bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let Some(dispatch) = *dispatch().lock().expect("poisoned dispatcher") else {
        // There are plugins registered and nobody to serve them. It is a
        // wiring failure in the shell, and keeping quiet about it would leave
        // the promise hanging with no clue why.
        for call in calls {
            let _ = bridge().reject(
                call.id,
                "the iOS shell did not install the plugin dispatcher",
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
            let _ = bridge().reject(call.id, "the arguments carried a zero byte inside them");
            continue;
        };
        unsafe { dispatch(call.id, module.as_ptr(), method.as_ptr(), args.as_ptr()) };
    }
}

/// Registers a plugin under its module name. The Swift registry calls it at
/// startup, before the runtime is created.
///
/// # Safety
/// `name` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_register(name: *const c_char) {
    if name.is_null() {
        return;
    }
    let Ok(name) = (unsafe { CStr::from_ptr(name) }).to_str() else { return };
    let mut names = names().lock().expect("poisoned plugin registry");
    // Registering the same name twice would put two modules under one name in
    // the registry, and nobody would ever see the second.
    if names.iter().any(|existing| existing == name) {
        eprintln!("angular-native: plugin {name:?} was already registered");
        return;
    }
    names.push(name.to_owned());
}

/// Installs the dispatcher. Passing `None` takes it away.
#[unsafe(no_mangle)]
pub extern "C" fn an_plugin_set_dispatch(callback: Option<DispatchFn>) {
    *dispatch().lock().expect("poisoned dispatcher") = callback;
}

/// Answers a call. `json` is the return value, already serialised.
/// Returns 0 if the call existed and -1 if it did not.
///
/// # Safety
/// `json` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_resolve(id: u64, json: *const c_char) -> i32 {
    let text = match unsafe { read(json) } {
        Some(text) => text,
        None => return report(bridge().reject(id, "the plugin answered with an invalid string")),
    };
    report(bridge().resolve(id, &text))
}

/// Rejects a call. Returns 0 if the call existed and -1 if it did not.
///
/// # Safety
/// `message` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_reject(id: u64, message: *const c_char) -> i32 {
    let text = unsafe { read(message) }.unwrap_or_else(|| "the plugin failed".to_owned());
    report(bridge().reject(id, &text))
}

/// Emits an event from a plugin, under the module's own name.
///
/// Unlike an answer, this belongs to no call: it can arrive at any time,
/// including never, and nothing on the JS side is waiting for it. It reaches
/// whoever subscribed with `NativeModules.on(module, event, …)` at the top of
/// the next frame.
///
/// Returns 0 if the module is registered and -1 if it is not — a name nobody
/// registered is a typo in a plugin, and it says so rather than dropping the
/// event in silence.
///
/// # Safety
/// All three have to be valid, nul-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_plugin_emit(
    module: *const c_char,
    event: *const c_char,
    json: *const c_char,
) -> i32 {
    let (Some(module), Some(event)) = (unsafe { read(module) }, unsafe { read(event) }) else {
        eprintln!("angular-native: a plugin emitted with an invalid module or event name");
        return -1;
    };
    let payload = unsafe { read(json) }.unwrap_or_else(|| "null".to_owned());
    report(bridge().emit(&module, &event, &payload))
}

/// # Safety
/// `text` has to be null or a valid C string.
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
