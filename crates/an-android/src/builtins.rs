//! The Android side of the built-in modules.
//!
//! The twin of the C surface in `an_bridge::builtins`, with the same split:
//! Rust takes the call to the UI thread and brings the answer back, and what
//! the module actually does lives in Java. What differs is that everything
//! crosses JNI, so the dispatcher is not a function pointer but a Java object a
//! global reference is kept to — exactly as `crate::plugins` does.
//!
//! Like the plugins', this is global to the process and not to the runtime: the
//! core builds one module per name when it starts the engine, and anything
//! arriving after that would no longer get in.

use std::sync::{Mutex, OnceLock};

use an_bridge::builtins::builtin_bridge;
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::sys::{jint, jlong};
use jni::{EnvUnowned, JavaVM};

/// Java's `AnBuiltinModules`, together with its VM so it can be attached to
/// from any thread.
type Registry = Mutex<Option<(JavaVM, Global<JObject<'static>>)>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

/// Carries the queued calls over to Java. It is called from `nativeFrame`,
/// which runs on the UI thread: that is where a built-in may show a chooser or
/// read the vibrator.
pub fn pump(env: &mut jni::Env) {
    let calls = builtin_bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let guard = registry().lock().expect("poisoned built-in registry");
    let Some((_, registry_ref)) = guard.as_ref() else {
        // The modules are registered and nobody is serving them: a wiring
        // failure in the shell. Keeping quiet about it would leave the promise
        // hanging with no clue why.
        for call in calls {
            let _ = builtin_bridge()
                .reject(call.id, "the Android shell did not install the built-in modules");
        }
        return;
    };
    for call in calls {
        let (Ok(module), Ok(method), Ok(args)) = (
            env.new_string(&call.module),
            env.new_string(&call.method),
            env.new_string(&call.args),
        ) else {
            let _ =
                builtin_bridge().reject(call.id, "the arguments could not be converted to Java");
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

/// Stores the Java object Rust will hand every built-in call to.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeSetBuiltinModules(
    mut env: EnvUnowned,
    _class: JClass,
    java_registry: JObject,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let (Ok(vm), Ok(global)) = (env.get_java_vm(), env.new_global_ref(&java_registry)) else {
            eprintln!("angular-native: the built-in modules could not be stored");
            return Ok(());
        };
        *registry().lock().expect("poisoned built-in registry") = Some((vm, global));
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Answers a call. `json` is the return value, already serialised.
/// Returns 0 if the call existed and -1 if it did not.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeBuiltinResolve(
    mut env: EnvUnowned,
    _class: JClass,
    id: jlong,
    json: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let json: String = match env.get_string(&json) {
            Ok(text) => text.into(),
            Err(_) => "null".to_owned(),
        };
        Ok(report(builtin_bridge().resolve(id as u64, &json)))
    })
    .resolve::<LogErrorAndDefault>()
}

/// Rejects a call. Returns 0 if the call existed and -1 if it did not.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeBuiltinReject(
    mut env: EnvUnowned,
    _class: JClass,
    id: jlong,
    message: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let message: String = match env.get_string(&message) {
            Ok(text) => text.into(),
            Err(_) => "the built-in module failed".to_owned(),
        };
        Ok(report(builtin_bridge().reject(id as u64, &message)))
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
