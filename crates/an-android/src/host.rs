//! `HostRenderer` over `android.view.View`, by way of a Kotlin class.

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::HostRenderer;
use jni::objects::{Global, JObject, JValue};
use jni::JavaVM;

/// Wraps Kotlin's `AnHost`.
///
/// It keeps the `JavaVM` and not the `JNIEnv`: a `JNIEnv` is tied to the thread
/// that obtained it and cannot be stored. All of this runs on the UI thread, so
/// `attach_current_thread` is cheap and does not switch threads.
pub struct JniHost {
    vm: JavaVM,
    host: Global<JObject<'static>>,
}

/// Calls a Java method and swallows the failure.
///
/// Since jni 0.22 the method name and its signature are no longer `&str`: the
/// name has to be nul-terminated the way JNI wants it, and the signature comes
/// parsed into types. It is safer —a mistyped signature is caught when it is
/// built rather than when it is called— but it fills every call site with
/// noise, so it is wrapped here once.
///
/// A failure is reported and execution continues: one stray method that does
/// not line up should not leave the user without an app.
pub(crate) fn call_java(
    env: &mut jni::Env,
    obj: &JObject,
    method: &str,
    signature: &str,
    args: &[JValue],
) {
    let Ok(sig) = jni::signature::RuntimeMethodSignature::from_str(signature) else {
        eprintln!("angular-native: unreadable signature for {method}: {signature}");
        return;
    };
    let name = jni::strings::JNIString::from(method);
    if let Err(error) = env.call_method(obj, &name, sig.method_signature(), args) {
        // The exception's trace before clearing it: without this the only
        // thing on show is "Java exception was thrown", which says nothing.
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        eprintln!("angular-native: call to {method} failed: {error}");
    }
}

/// The same as [`call_java`] but for methods that return a `long`.
///
/// It returns `None` if something failed, which is what the measuring side
/// wants: use its fallback value instead of blowing up.
pub(crate) fn call_java_long(
    env: &mut jni::Env,
    obj: &JObject,
    method: &str,
    signature: &str,
    args: &[JValue],
) -> Option<i64> {
    let sig = jni::signature::RuntimeMethodSignature::from_str(signature).ok()?;
    let name = jni::strings::JNIString::from(method);
    match env.call_method(obj, &name, sig.method_signature(), args).and_then(|v| v.j()) {
        Ok(value) => Some(value),
        Err(_) => {
            let _ = env.exception_clear();
            None
        }
    }
}

impl JniHost {
    pub fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        JniHost { vm, host }
    }

    /// Calls a method on the Kotlin host. A failure here is a mismatch between
    /// Kotlin's signature and Rust's: it is reported and execution continues,
    /// because tearing the app down over one stray method leaves the user with
    /// nothing.
    fn call(&self, method: &str, signature: &str, args: &[JValue]) {
        // Since jni 0.22 the thread is attached around a closure instead of
        // handing back a guard: detaching no longer depends on nobody
        // forgetting to drop it.
        let _ = self.vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
            call_java(env, self.host.as_obj(), method, signature, args);
            Ok(())
        });
    }

    fn kind_code(kind: NodeKind) -> i32 {
        match kind {
            NodeKind::View => 0,
            NodeKind::Text => 1,
            NodeKind::RawText => 2,
            NodeKind::Image => 3,
            NodeKind::ScrollView => 4,
            NodeKind::TextInput => 5,
            NodeKind::StackView => 6,
            NodeKind::TabBar => 7,
            NodeKind::Switch => 8,
            NodeKind::Slider => 9,
            NodeKind::ActivityIndicator => 10,
            NodeKind::ProgressBar => 11,
            NodeKind::Button => 12,
            NodeKind::Modal => 13,
            NodeKind::Alert => 14,
            NodeKind::Icon => 15,
            NodeKind::SegmentedControl => 16,
            NodeKind::Stepper => 17,
            NodeKind::SearchBar => 18,
            NodeKind::Picker => 19,
            NodeKind::DatePicker => 20,
            NodeKind::NavigationBar => 21,
            NodeKind::TextEditor => 22,
            NodeKind::WebView => 23,
            NodeKind::MapView => 24,
            NodeKind::VideoView => 25,
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
        // Props travel as text: the number of distinct types does not justify
        // one JNI signature apiece, and Kotlin already knows what each property
        // expects.
        let text = match value {
            PropValue::Null => String::new(),
            PropValue::Bool(v) => v.to_string(),
            PropValue::Number(v) => v.to_string(),
            PropValue::Str(v) => v.clone(),
            PropValue::Color(v) => format!("#{v:08x}"),
        };
        let _ = self.vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
            let (Ok(key), Ok(text)) = (env.new_string(key), env.new_string(&text)) else {
                return Ok(());
            };
            call_java(env, self.host.as_obj(), "setProp", "(ILjava/lang/String;Ljava/lang/String;)V", &[JValue::Int(id as i32), JValue::Object(&key), JValue::Object(&text)]);
            Ok(())
        });
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        let _ = self.vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
            let Ok(text) = env.new_string(text) else { return Ok(()) };
            call_java(env, self.host.as_obj(), "setText", "(ILjava/lang/String;)V", &[JValue::Int(id as i32), JValue::Object(&text)]);
            Ok(())
        });
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let _ = self.vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
            let Ok(event) = env.new_string(event) else { return Ok(()) };
            call_java(env, self.host.as_obj(), "setListener", "(ILjava/lang/String;Z)V", &[JValue::Int(id as i32), JValue::Object(&event), JValue::Bool(enabled)]);
            Ok(())
        });
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
        // Android relays out nothing on its own: it has to be asked for a
        // layout pass once every frame's rectangles have been applied.
        self.call("flush", "()V", &[]);
    }

    fn clear(&mut self) {
        self.call("clearAll", "()V", &[]);
    }
}
