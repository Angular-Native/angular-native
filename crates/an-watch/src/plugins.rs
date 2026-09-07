//! The watchOS side of plugins.
//!
//! A plugin is not compiled into the core: it is Swift that an npm package
//! brings and that `an-cli` links into the `.app`. Rust does not know what it
//! does or what methods it has; it only carries the call to the main thread and
//! brings the answer back.
//!
//! It is `an-ios`'s mechanism, deliberately: the same mailbox in `an-bridge`,
//! the same register/dispatch/resolve/reject quartet, the same `pump()` inside
//! the frame. The names carry the `an_watch_` prefix every other entry point on
//! this host carries, and that is the only difference. It costs nothing — a
//! plugin never calls the FFI, it calls `AnPluginCall` in the shell — and it
//! keeps the C header readable as one family of functions instead of two.
//!
//! **What a watch can and cannot do is not decided here.** This file would carry
//! a call to a camera plugin just as happily as to a keychain one; what stops
//! the camera is `an-cli`, at build time, because the plugin never declared a
//! watchOS half. That separation is on purpose: the postman does not read the
//! letters.
//!
//! Everything here is global to the process and not to the runtime. Registration
//! happens before `an_watch_runtime_new` —Swift has nowhere to keep it
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

/// The names registered so far, for the note that goes on a rejection.
pub fn registered_names() -> Vec<String> {
    names().lock().expect("poisoned plugin registry").clone()
}

/// One module per registered plugin, ready to travel to the engine's thread.
///
/// They are built in `an_watch_runtime_new`, after Swift has registered its own
/// and before the worker is started.
pub fn host_plugins() -> Vec<HostPlugin> {
    names()
        .lock()
        .expect("poisoned plugin registry")
        .iter()
        .map(|name| HostPlugin::new(name, bridge().clone()))
        .collect()
}

/// Carries the queued calls over to the shell. It is called from
/// `an_watch_runtime_frame`, which the shell runs on the main actor: exactly
/// where a plugin may touch WatchKit.
pub fn pump() {
    let calls = bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let Some(dispatch) = *dispatch().lock().expect("poisoned dispatcher") else {
        // There are plugins registered and nobody to serve them. It is a wiring
        // failure in the shell, and keeping quiet about it would leave the
        // promise hanging with no clue why.
        for call in calls {
            let _ = bridge().reject(
                call.id,
                "the watchOS shell did not install the plugin dispatcher",
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

/// Why a name that is not in the registry may still be a name somebody wrote in
/// good faith.
///
/// The watch has one branch the phone does not, and it is the one worth having:
/// an app with no plugins on a watch is very often an app whose plugin *does*
/// exist on the phone and cannot exist here. `an watchos` already said so at
/// build time; whoever is reading a rejection at run time did not see that, so
/// it is pointed at.
pub fn absent_note() -> String {
    let names = registered_names();
    if names.is_empty() {
        return "this watch .app carries no plugins, so only the modules compiled into the core \
                exist here. A plugin has to declare a watchOS half —angularNative.watchos in its \
                package.json— and not every one can: there is no pasteboard on a watch and no \
                biometric sensor. See https://angular-native.github.io/extending/plugins/"
            .to_owned();
    }
    format!(
        "the plugins in this watch .app are {}, and only those plus the modules compiled into the \
         core exist here. A plugin that works on the phone is not here unless it declared a \
         watchOS half. See https://angular-native.github.io/extending/plugins/",
        names.join(", ")
    )
}

/// Registers a plugin under its module name. The Swift registry calls it at
/// startup, before the runtime is created.
///
/// # Safety
/// `name` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_plugin_register(name: *const c_char) {
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
pub extern "C" fn an_watch_plugin_set_dispatch(callback: Option<DispatchFn>) {
    *dispatch().lock().expect("poisoned dispatcher") = callback;
}

/// Answers a call. `json` is the return value, already serialised.
/// Returns 0 if the call existed and -1 if it did not.
///
/// # Safety
/// `json` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_plugin_resolve(id: u64, json: *const c_char) -> i32 {
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
/// Emits an event from a plugin, under the module's own name.
///
/// Unlike an answer, this belongs to no call: it can arrive at any time,
/// including never, and nothing on the JS side is waiting for it. It reaches
/// whoever subscribed with `NativeModules.on(module, event, …)` at the top of
/// the next frame.
///
/// # Safety
/// All three have to be valid, nul-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_plugin_emit(
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_plugin_reject(id: u64, message: *const c_char) -> i32 {
    let text = unsafe { read(message) }.unwrap_or_else(|| "the plugin failed".to_owned());
    report(bridge().reject(id, &text))
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

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    /// The whole way round without a watch: register a name, build the module
    /// the engine would get, call it, pick the call up on the shell's side and
    /// answer it.
    ///
    /// It exercises the same `PluginBridge` the phone uses, which is the point:
    /// if this ever stopped matching iOS, this test is where it would show.
    #[test]
    fn a_call_goes_out_to_the_shell_and_the_answer_comes_back() {
        use an_bridge::modules::ModuleRegistry;

        let name = CString::new("keychain").expect("no zero byte");
        unsafe { an_watch_plugin_register(name.as_ptr()) };
        assert!(registered_names().iter().any(|each| each == "keychain"));

        let mut registry = ModuleRegistry::new();
        for plugin in host_plugins() {
            registry.register(Box::new(plugin));
        }
        registry.invoke("keychain", "has", serde_json::json!({ "key": "token" }));
        // The engine does not block: nothing is answered until the shell has
        // been through a frame.
        assert!(registry.drain().is_empty());

        let calls = bridge().take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method, "has");
        bridge().resolve(calls[0].id, "true").expect("the call was waiting");
        assert_eq!(registry.drain()[0].1, Ok(serde_json::Value::Bool(true)));
    }

    /// With plugins registered and no dispatcher installed, the calls are
    /// rejected rather than left in the queue for ever. A shell that forgot to
    /// wire itself up is a bug, and a promise that never settles is the one
    /// symptom nobody can debug.
    #[test]
    fn with_no_dispatcher_the_calls_are_turned_down_instead_of_hanging() {
        use an_bridge::modules::ModuleRegistry;

        let name = CString::new("nowhere").expect("no zero byte");
        unsafe { an_watch_plugin_register(name.as_ptr()) };
        let mut registry = ModuleRegistry::new();
        for plugin in host_plugins() {
            registry.register(Box::new(plugin));
        }
        registry.invoke("nowhere", "anything", serde_json::Value::Null);
        // No `an_watch_plugin_set_dispatch` anywhere in this test.
        pump();
        let answers = registry.drain();
        let error = answers
            .iter()
            .find(|(_, result)| result.is_err())
            .map(|(_, result)| result.as_ref().unwrap_err().clone())
            .expect("it has to be rejected");
        assert!(error.contains("dispatcher"), "{error}");
    }

    /// The note that goes on a rejection names the plugins that are there.
    /// Without that, a typo and a missing dependency read the same.
    #[test]
    fn the_absent_note_names_what_is_in_the_app() {
        let name = CString::new("clipboard-on-the-watch").expect("no zero byte");
        unsafe { an_watch_plugin_register(name.as_ptr()) };
        let note = absent_note();
        assert!(note.contains("clipboard-on-the-watch"), "{note}");
        assert!(note.contains("watchOS half"), "{note}");
    }
}
