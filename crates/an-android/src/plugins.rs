//! The Android side of plugins.
//!
//! The twin of `an-ios/src/modules/plugin.rs`, with the same split: Rust takes
//! the call to the UI thread and brings the answer back, and whatever the
//! plugin does lives in Java. What differs is the return path —here everything
//! crosses JNI— and that the dispatcher is not a function pointer but a Java
//! object a global reference is kept to.
//!
//! As on iOS, this is global to the process and not to the runtime:
//! registration happens before `nativeNew` because the core builds one module
//! per plugin when it starts the engine, and anything arriving after that would
//! no longer get in.

use std::sync::{Arc, Mutex, OnceLock};

use an_bridge::plugins::{HostPlugin, PluginBridge};
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::sys::{jint, jlong};
use jni::{EnvUnowned, JavaVM};

fn bridge() -> &'static Arc<PluginBridge> {
    static BRIDGE: OnceLock<Arc<PluginBridge>> = OnceLock::new();
    BRIDGE.get_or_init(PluginBridge::new)
}

fn names() -> &'static Mutex<Vec<String>> {
    static NAMES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    NAMES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Java's `AnPluginRegistry`, together with its VM so it can be attached to
/// from any thread.
type Registry = Mutex<Option<(JavaVM, Global<JObject<'static>>)>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

/// One module per registered plugin, ready to travel to the engine's thread.
pub fn host_plugins() -> Vec<HostPlugin> {
    names()
        .lock()
        .expect("poisoned plugin registry")
        .iter()
        .map(|name| HostPlugin::new(name, bridge().clone()))
        .collect()
}

/// Carries the queued calls over to Java. It is called from `nativeFrame`,
/// which runs on the UI thread: that is where a plugin may touch the view or
/// ask for a permission.
pub fn pump(env: &mut jni::Env) {
    let calls = bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let guard = registry().lock().expect("poisoned plugin registry");
    let Some((_, registry_ref)) = guard.as_ref() else {
        // There are plugins registered and nobody to serve them: that is a
        // wiring failure in the shell. Keeping quiet about it would leave the
        // promise hanging with no clue why.
        for call in calls {
            let _ = bridge()
                .reject(call.id, "the Android shell did not install the plugin registry");
        }
        return;
    };
    for call in calls {
        let (Ok(module), Ok(method), Ok(args)) = (
            env.new_string(&call.module),
            env.new_string(&call.method),
            env.new_string(&call.args),
        ) else {
            let _ = bridge().reject(call.id, "the arguments could not be converted to Java");
            continue;
        };
        crate::host::call_java(
            env,
            registry_ref.as_obj(),
            "dispatch",
            "(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Long(call.id as jlong),
                JValue::Object(&module),
                JValue::Object(&method),
                JValue::Object(&args),
            ],
        );
    }
}

/// Stores the Java registry Rust will hand every call to.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeSetPluginRegistry(
    mut env: EnvUnowned,
    _class: JClass,
    java_registry: JObject,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let (Ok(vm), Ok(global)) = (env.get_java_vm(), env.new_global_ref(&java_registry)) else {
            eprintln!("angular-native: the plugin registry could not be stored");
            return Ok(());
        };
        *registry().lock().expect("poisoned plugin registry") = Some((vm, global));
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Registers a plugin under the name JS invokes it by.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeRegisterPlugin(
    mut env: EnvUnowned,
    _class: JClass,
    name: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(name) = env.get_string(&name) else { return Ok(()) };
        let name: String = name.into();
        let mut names = names().lock().expect("poisoned plugin registry");
        // Two modules under the same name would give a second one nobody sees.
        if names.iter().any(|existing| *existing == name) {
            eprintln!("angular-native: plugin {name:?} was already registered");
            return Ok(());
        }
        names.push(name);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Answers a call. `json` is the return value, already serialised.
/// Returns 0 if the call existed and -1 if it did not.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativePluginResolve(
    mut env: EnvUnowned,
    _class: JClass,
    id: jlong,
    json: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let Ok(json) = env.get_string(&json) else {
            let json: String = "null".to_owned();
            return Ok(report(bridge().resolve(id as u64, &json)));
        };
        let json: String = json.into();
        Ok(report(bridge().resolve(id as u64, &json)))
    })
    .resolve::<LogErrorAndDefault>()
}

/// Rejects a call. Returns 0 if the call existed and -1 if it did not.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativePluginReject(
    mut env: EnvUnowned,
    _class: JClass,
    id: jlong,
    message: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let message: String = match env.get_string(&message) {
            Ok(text) => text.into(),
            Err(_) => "the plugin failed".to_owned(),
        };
        Ok(report(bridge().reject(id as u64, &message)))
    })
    .resolve::<LogErrorAndDefault>()
}

fn report(result: Result<(), String>) -> jint {
    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("angular-native: {message}");
            -1
        }
    }
}
