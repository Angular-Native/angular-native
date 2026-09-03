//! Built-in modules: the ones the framework brings, served by the shell.
//!
//! A module of the [`crate::modules`] sort is written in Rust and answers from
//! the engine's thread. A [`crate::plugins`] one is written outside the repo and
//! answers from the UI thread, through a queue. A *built-in* is the second shape
//! with the first one's provenance: the code is ours and ships with the
//! framework, but what it calls —`UIActivityViewController`, `NWPathMonitor`,
//! `ConnectivityManager`— may only be touched on the UI thread.
//!
//! So this is not a second mechanism. It is [`PluginBridge`] with a mailbox of
//! its own: "plugin" says where the code came from, not how the call travels,
//! and the way the call travels is the same one. What it buys by being separate
//! is that a host with no plugin registry —macOS, the watch— still has these,
//! and that a built-in cannot be shadowed by an npm package claiming its name.
//!
//! ```text
//!   engine thread                            UI thread
//!   ──────────────────                       ─────────────────────────
//!   HostPlugin::call
//!        │ queues it with its Responder
//!        ▼
//!   builtin_bridge() ──── take_calls() ────▶ AnBuiltinModules (Swift/Java)
//!        ▲                                          │
//!        └────────── resolve(id, json) ◀────────────┘
//! ```
//!
//! The C surface at the bottom of this file is shared by the three Apple hosts:
//! they are separate static libraries that never end up in the same binary, and
//! writing `an_builtin_resolve` three times would be three places for the same
//! bug. Android does not use it —everything there crosses JNI— and declares its
//! own shims in `an-android`.

use std::ffi::{c_char, CStr, CString};
use std::sync::{Arc, Mutex, OnceLock};

use crate::plugins::{HostPlugin, PluginBridge};

/// The modules every host registers, in the order they are registered.
///
/// This list is the contract. `packages/platform-native/src/native-modules.ts`
/// offers one typed service per name and `scripts/check-builtins.sh` reads both,
/// so a name added here and forgotten on the other side is caught before an app
/// ever sees a rejection.
///
/// A host that cannot honestly provide one of them still registers it: the
/// refusal belongs in the shell, where it can say what this platform is and what
/// it has instead. A name missing from the registry only ever produces "there is
/// no native module called …", and that reads like a typo.
pub const BUILTIN_MODULES: &[&str] = &["files", "share", "network"];

/// The mailbox the built-ins share. One per process, like the plugins'.
pub fn builtin_bridge() -> &'static Arc<PluginBridge> {
    static BRIDGE: OnceLock<Arc<PluginBridge>> = OnceLock::new();
    BRIDGE.get_or_init(PluginBridge::new)
}

/// One module per name in [`BUILTIN_MODULES`], ready to travel to the engine's
/// thread. Built in the host's `runtime_new`, before the worker is started.
pub fn builtin_modules() -> Vec<HostPlugin> {
    BUILTIN_MODULES.iter().map(|name| HostPlugin::new(name, builtin_bridge().clone())).collect()
}

/// What the shell installs so that Rust can tell it about a call.
///
/// The strings are borrowed and only hold for the duration of the call: the
/// shell copies them into `String`s the moment it is entered.
pub type BuiltinDispatchFn =
    unsafe extern "C" fn(id: u64, module: *const c_char, method: *const c_char, args: *const c_char);

fn dispatch() -> &'static Mutex<Option<BuiltinDispatchFn>> {
    static DISPATCH: OnceLock<Mutex<Option<BuiltinDispatchFn>>> = OnceLock::new();
    DISPATCH.get_or_init(|| Mutex::new(None))
}

/// Carries the queued calls over to the shell. The Apple hosts call it from
/// their `runtime_frame`, which runs on the main thread: exactly where a
/// built-in may touch UIKit or AppKit.
pub fn pump_c() {
    let calls = builtin_bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let Some(dispatch) = *dispatch().lock().expect("poisoned built-in dispatcher") else {
        // The modules are registered and nobody is serving them. It is a wiring
        // failure in the shell, and keeping quiet about it would leave the
        // promise hanging with no clue why.
        for call in calls {
            let _ = builtin_bridge()
                .reject(call.id, "the shell did not install the built-in module dispatcher");
        }
        return;
    };
    for call in calls {
        let (Ok(module), Ok(method), Ok(args)) = (
            CString::new(call.module.as_str()),
            CString::new(call.method.as_str()),
            CString::new(call.args.as_str()),
        ) else {
            let _ =
                builtin_bridge().reject(call.id, "the arguments carried a zero byte inside them");
            continue;
        };
        unsafe { dispatch(call.id, module.as_ptr(), method.as_ptr(), args.as_ptr()) };
    }
}

/// Installs the dispatcher. Passing `None` takes it away.
#[unsafe(no_mangle)]
pub extern "C" fn an_builtin_set_dispatch(callback: Option<BuiltinDispatchFn>) {
    *dispatch().lock().expect("poisoned built-in dispatcher") = callback;
}

/// Answers a call. `json` is the return value, already serialised.
/// Returns 0 if the call existed and -1 if it did not.
///
/// # Safety
/// `json` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_builtin_resolve(id: u64, json: *const c_char) -> i32 {
    let Some(text) = (unsafe { read(json) }) else {
        return report(
            builtin_bridge().reject(id, "the built-in module answered with an invalid string"),
        );
    };
    report(builtin_bridge().resolve(id, &text))
}

/// Rejects a call. Returns 0 if the call existed and -1 if it did not.
///
/// # Safety
/// `message` has to be a valid, nul-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_builtin_reject(id: u64, message: *const c_char) -> i32 {
    let text = unsafe { read(message) }.unwrap_or_else(|| "the built-in module failed".to_owned());
    report(builtin_bridge().reject(id, &text))
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
    use serde_json::{json, Value};

    use super::*;
    use crate::modules::ModuleRegistry;

    /// Every name in the list becomes a module in the registry. It is the whole
    /// promise of a built-in: the app calls it without declaring anything.
    #[test]
    fn every_declared_builtin_reaches_the_registry() {
        let mut registry = ModuleRegistry::new();
        for module in builtin_modules() {
            registry.register(Box::new(module));
        }
        let mut names = registry.names();
        names.sort_unstable();
        let mut expected: Vec<&str> = BUILTIN_MODULES.to_vec();
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    /// And the call really travels: out through the mailbox, back as an answer.
    /// This is the plugin postman doing its job under another name, which is the
    /// point of not having written a second one.
    #[test]
    fn a_builtin_call_travels_out_and_the_answer_comes_back() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(HostPlugin::new("network", builtin_bridge().clone())));
        registry.invoke("network", "status", json!(null));

        // Nothing yet: the engine does not block waiting on the UI thread.
        assert!(registry.drain().is_empty());

        let calls = builtin_bridge().take_calls();
        let call = calls.iter().find(|call| call.module == "network").expect("it was queued");
        builtin_bridge()
            .resolve(call.id, r#"{"online":true}"#)
            .expect("the call was waiting");

        let answers = registry.drain();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].1, Ok(json!({ "online": true }) as Value));
    }
}
