//! Módulos nativos de Android.
//!
//! El de dispositivo delega en `AnHost.deviceInfo()`: leer `android.os.Build`
//! por JNI campo a campo serían cinco llamadas para lo que Java resuelve en
//! una línea.

use an_bridge::modules::{NativeModule, Responder};
use jni::objects::{GlobalRef, JString, JValue};
use jni::JavaVM;
use serde_json::Value;

pub struct DeviceModule {
    vm: JavaVM,
    host: GlobalRef,
}

impl DeviceModule {
    pub fn new(vm: JavaVM, host: GlobalRef) -> Self {
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
        let Ok(mut env) = self.vm.attach_current_thread() else {
            respond.reject("no se pudo hablar con la JVM");
            return;
        };
        let result = env
            .call_method(self.host.as_obj(), "deviceInfo", "()Ljava/lang/String;", &[])
            .and_then(|value| value.l());
        let Ok(object) = result else {
            let _ = env.exception_clear();
            respond.reject("AnHost.deviceInfo falló");
            return;
        };
        let text = JString::from(object);
        let Ok(json) = env.get_string(&text) else {
            respond.reject("deviceInfo no devolvió texto");
            return;
        };
        let json: String = json.into();
        match serde_json::from_str(&json) {
            Ok(value) => respond.resolve(value),
            Err(error) => respond.reject(format!("deviceInfo devolvió JSON inválido: {error}")),
        }
    }
}

// Silencia el aviso por `JValue` sin usar si el módulo crece.
const _: Option<JValue<'static, 'static>> = None;
