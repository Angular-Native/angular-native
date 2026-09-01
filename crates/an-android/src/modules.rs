//! Módulos nativos de Android.
//!
//! El de dispositivo delega en `AnHost.deviceInfo()`: leer `android.os.Build`
//! por JNI campo a campo serían cinco llamadas para lo que Java resuelve en
//! una línea.

use an_bridge::modules::{NativeModule, Responder};
use jni::objects::{Global, JObject, JString, JValue};
use jni::JavaVM;
use serde_json::Value;

pub struct DeviceModule {
    vm: JavaVM,
    host: Global<JObject<'static>>,
}

impl DeviceModule {
    pub fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        DeviceModule { vm, host }
    }
}

impl NativeModule for DeviceModule {
    fn name(&self) -> &'static str {
        "device"
    }

    fn call(&mut self, method: &str, _args: Value, respond: Responder) {
        if method != "info" {
            respond.reject(format!("el módulo device no tiene ningún método {method:?}"));
            return;
        }
        let leido = self
            .vm
            .attach_current_thread(|env| -> Result<Option<String>, jni::errors::Error> {
                let name = jni::strings::JNIString::from("deviceInfo");
                let Ok(sig) =
                    jni::signature::RuntimeMethodSignature::from_str("()Ljava/lang/String;")
                else {
                    return Ok(None);
                };
                let result = env
                    .call_method(self.host.as_obj(), &name, sig.method_signature(), &[])
                    .and_then(|value| value.l());
                let Ok(object) = result else {
                    let _ = env.exception_clear();
                    return Ok(None);
                };
                // El cast comprobado: en jni 0.22 pasar de `JObject` a
                // `JString` ya no es un `from`, se le pregunta a la JVM si el
                // objeto es de esa clase.
                let Ok(text) = env.cast_local::<JString>(object) else {
                    return Ok(None);
                };
                Ok(Some(text.to_string()))
            })
            .ok()
            .flatten();
        let Some(json) = leido else {
            respond.reject("AnHost.deviceInfo falló");
            return;
        };
        match serde_json::from_str(&json) {
            Ok(value) => respond.resolve(value),
            Err(error) => respond.reject(format!("deviceInfo devolvió JSON inválido: {error}")),
        }
    }
}

// Silencia el aviso por `JValue` sin usar si el módulo crece.
const _: Option<JValue<'static>> = None;
