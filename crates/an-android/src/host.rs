//! `HostRenderer` sobre `android.view.View`, a través de una clase Kotlin.

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::HostRenderer;
use jni::objects::{GlobalRef, JObject, JValue};
use jni::JavaVM;

/// Envuelve el `AnHost` de Kotlin.
///
/// Se guarda la `JavaVM` y no el `JNIEnv`: un `JNIEnv` está atado al hilo que
/// lo obtuvo y no se puede guardar. Todo esto corre en el hilo de UI, así que
/// `attach_current_thread` es barato y no cambia de hilo.
pub struct JniHost {
    vm: JavaVM,
    host: GlobalRef,
}

impl JniHost {
    pub fn new(vm: JavaVM, host: GlobalRef) -> Self {
        JniHost { vm, host }
    }

    /// Llama a un método del host Kotlin. Un fallo aquí es un desajuste entre
    /// la firma de Kotlin y la de Rust: se reporta y se sigue, porque tirar la
    /// app por un método suelto deja al usuario sin nada.
    fn call(&self, method: &str, signature: &str, args: &[JValue]) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return;
        };
        let host: &JObject = self.host.as_obj();
        if let Err(error) = env.call_method(host, method, signature, args) {
            let _ = env.exception_clear();
            eprintln!("angular-native: fallo llamando a AnHost.{method}: {error}");
        }
    }

    fn kind_code(kind: NodeKind) -> i32 {
        match kind {
            NodeKind::View => 0,
            NodeKind::Text => 1,
            NodeKind::RawText => 2,
            NodeKind::Image => 3,
            NodeKind::ScrollView => 4,
            NodeKind::TextInput => 5,
        }
    }
}

impl HostRenderer for JniHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        self.call(
            "createView",
            "(II)V",
            &[JValue::Int(id as i32), JValue::Int(Self::kind_code(kind))],
        );
    }

    fn destroy(&mut self, id: NodeId) {
        self.call("destroyView", "(I)V", &[JValue::Int(id as i32)]);
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        self.call(
            "insertView",
            "(III)V",
            &[
                JValue::Int(parent as i32),
                JValue::Int(child as i32),
                JValue::Int(index as i32),
            ],
        );
    }

    fn remove(&mut self, parent: NodeId, child: NodeId) {
        self.call(
            "removeView",
            "(II)V",
            &[JValue::Int(parent as i32), JValue::Int(child as i32)],
        );
    }

    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return;
        };
        // Las props viajan como texto: el número de tipos distintos no
        // justifica una firma JNI por cada uno, y Kotlin ya sabe qué espera
        // cada propiedad.
        let text = match value {
            PropValue::Null => String::new(),
            PropValue::Bool(v) => v.to_string(),
            PropValue::Number(v) => v.to_string(),
            PropValue::Str(v) => v.clone(),
            PropValue::Color(v) => format!("#{v:08x}"),
        };
        let (Ok(key), Ok(text)) = (env.new_string(key), env.new_string(&text)) else {
            return;
        };
        let result = env.call_method(
            self.host.as_obj(),
            "setProp",
            "(ILjava/lang/String;Ljava/lang/String;)V",
            &[JValue::Int(id as i32), JValue::Object(&key), JValue::Object(&text)],
        );
        if result.is_err() {
            let _ = env.exception_clear();
        }
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return;
        };
        let Ok(text) = env.new_string(text) else { return };
        let result = env.call_method(
            self.host.as_obj(),
            "setText",
            "(ILjava/lang/String;)V",
            &[JValue::Int(id as i32), JValue::Object(&text)],
        );
        if result.is_err() {
            let _ = env.exception_clear();
        }
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return;
        };
        let Ok(event) = env.new_string(event) else { return };
        let result = env.call_method(
            self.host.as_obj(),
            "setListener",
            "(ILjava/lang/String;Z)V",
            &[JValue::Int(id as i32), JValue::Object(&event), JValue::Bool(enabled as u8)],
        );
        if result.is_err() {
            let _ = env.exception_clear();
        }
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        self.call(
            "setLayout",
            "(IFFFF)V",
            &[
                JValue::Int(id as i32),
                JValue::Float(frame.x),
                JValue::Float(frame.y),
                JValue::Float(frame.width),
                JValue::Float(frame.height),
            ],
        );
    }

    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        self.call(
            "setContentSize",
            "(IFF)V",
            &[JValue::Int(id as i32), JValue::Float(width), JValue::Float(height)],
        );
    }

    fn set_root(&mut self, id: NodeId) {
        self.call("setRoot", "(I)V", &[JValue::Int(id as i32)]);
    }

    fn flush(&mut self) {
        // Android no recoloca nada por su cuenta: hay que pedirle una pasada
        // de layout cuando ya se aplicaron todos los marcos del frame.
        self.call("flush", "()V", &[]);
    }

    fn clear(&mut self) {
        self.call("clearAll", "()V", &[]);
    }
}
