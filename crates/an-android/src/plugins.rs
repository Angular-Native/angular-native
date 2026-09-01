//! El lado Android de los plugins.
//!
//! El gemelo de `an-ios/src/modules/plugin.rs`, con el mismo reparto: Rust
//! lleva la llamada al hilo de UI y se trae la respuesta, y lo que el plugin
//! hace vive en Java. Lo que cambia es el camino de vuelta —aquí todo cruza
//! JNI— y que el despachador no es un puntero a función sino un objeto Java al
//! que se guarda una referencia global.
//!
//! Como en iOS, esto es global al proceso y no al runtime: el registro ocurre
//! antes de `nativeNew` porque el core construye un módulo por plugin al
//! arrancar el motor, y lo que llegue después ya no entraría.

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

/// El `AnPluginRegistry` de Java, con su VM para poder engancharse desde
/// cualquier hilo.
type Registry = Mutex<Option<(JavaVM, Global<JObject<'static>>)>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

/// Un módulo por plugin registrado, listo para viajar al hilo del motor.
pub fn host_plugins() -> Vec<HostPlugin> {
    names()
        .lock()
        .expect("registro de plugins envenenado")
        .iter()
        .map(|name| HostPlugin::new(name, bridge().clone()))
        .collect()
}

/// Lleva a Java las llamadas acumuladas. Se llama desde `nativeFrame`, que
/// corre en el hilo de UI: es donde un plugin puede tocar la vista o pedir un
/// permiso.
pub fn pump(env: &mut jni::Env) {
    let calls = bridge().take_calls();
    if calls.is_empty() {
        return;
    }
    let guard = registry().lock().expect("registro de plugins envenenado");
    let Some((_, registry_ref)) = guard.as_ref() else {
        // Hay plugins registrados y nadie que los atienda: es un fallo de
        // montaje del shell. Callarlo dejaría la promesa colgada sin pista.
        for call in calls {
            let _ = bridge()
                .reject(call.id, "el shell de Android no instaló el registro de plugins");
        }
        return;
    };
    for call in calls {
        let (Ok(module), Ok(method), Ok(args)) = (
            env.new_string(&call.module),
            env.new_string(&call.method),
            env.new_string(&call.args),
        ) else {
            let _ = bridge().reject(call.id, "no se pudieron convertir los argumentos a Java");
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

/// Guarda el registro de Java al que Rust le pasará cada llamada.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeSetPluginRegistry(
    mut env: EnvUnowned,
    _class: JClass,
    java_registry: JObject,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let (Ok(vm), Ok(global)) = (env.get_java_vm(), env.new_global_ref(&java_registry)) else {
            eprintln!("angular-native: no se pudo guardar el registro de plugins");
            return Ok(());
        };
        *registry().lock().expect("registro de plugins envenenado") = Some((vm, global));
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Da de alta un plugin por el nombre con el que JS lo invoca.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeRegisterPlugin(
    mut env: EnvUnowned,
    _class: JClass,
    name: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(name) = env.get_string(&name) else { return Ok(()) };
        let name: String = name.into();
        let mut names = names().lock().expect("registro de plugins envenenado");
        // Dos módulos con el mismo nombre darían un segundo que no vería nadie.
        if names.iter().any(|existing| *existing == name) {
            eprintln!("angular-native: el plugin {name:?} ya estaba registrado");
            return Ok(());
        }
        names.push(name);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Contesta a una llamada. `json` es el valor de vuelta ya serializado.
/// Devuelve 0 si la llamada existía y -1 si no.
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

/// Rechaza una llamada. Devuelve 0 si la llamada existía y -1 si no.
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
            Err(_) => "el plugin falló".to_owned(),
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
