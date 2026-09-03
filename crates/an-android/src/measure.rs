//! Text measurement with `android.text.StaticLayout`, by way of Kotlin.
//!
//! Same cache as on iOS and for the same reason: the layout asks for each
//! node's size several times per frame, and here every query crosses JNI, which
//! is a good deal more expensive than an `objc_msgSend`.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};
use jni::objects::{Global, JObject, JValue};
use jni::JavaVM;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    size_bits: u32,
    weight: u16,
    italic: bool,
    family: Option<String>,
    max_width_eighths: Option<i32>,
}

pub struct JniMeasurer {
    vm: JavaVM,
    host: Global<JObject<'static>>,
    cache: RefCell<HashMap<Key, (f32, f32)>>,
}

impl JniMeasurer {
    pub fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        JniMeasurer { vm, host, cache: RefCell::new(HashMap::new()) }
    }

    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }
}

impl TextMeasurer for JniMeasurer {
    fn measure_control(&self, name: &str, available_width: Option<f32>) -> (f32, f32) {
        self.vm
            .attach_current_thread(|env| -> Result<(f32, f32), jni::errors::Error> {
                let Ok(name_ref) = env.new_string(name) else { return Ok((0.0, 0.0)) };
                let available = available_width.filter(|w| w.is_finite()).unwrap_or(-1.0);
                let result = crate::host::call_java_long(
                    env,
                    self.host.as_obj(),
                    "measureControl",
                    "(Ljava/lang/String;F)J",
                    &[JValue::Object(&name_ref), JValue::Float(available)],
                );
                Ok(match result {
                    Some(packed) => (
                        ((packed >> 32) as i32) as f32 / 100.0,
                        ((packed & 0xffff_ffff) as i32) as f32 / 100.0,
                    ),
                    None => (0.0, 0.0),
                })
            })
            .unwrap_or((0.0, 0.0))
    }

    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32) {
        let key = Key {
            text: text.to_owned(),
            size_bits: font.size.to_bits(),
            weight: font.weight,
            italic: font.italic,
            family: font.family.clone(),
            max_width_eighths: max_width
                .filter(|w| w.is_finite())
                .map(|w| (w * 8.0).round() as i32),
        };
        if let Some(hit) = self.cache.borrow().get(&key) {
            return *hit;
        }

        let fallback = (0.0, font.line_height.unwrap_or(font.size * 1.25));
        let raw = self
            .vm
            .attach_current_thread(|env| -> Result<Option<i64>, jni::errors::Error> {
                let Ok(text_ref) = env.new_string(text) else { return Ok(None) };
                let family = font.family.clone().unwrap_or_default();
                let Ok(family_ref) = env.new_string(&family) else { return Ok(None) };
                // Infinite width travels as -1: JNI has no Option.
                let limit = max_width.filter(|w| w.is_finite()).unwrap_or(-1.0);
                Ok(crate::host::call_java_long(
                    env,
                    self.host.as_obj(),
                    "measureText",
                    "(Ljava/lang/String;FIZLjava/lang/String;FI)J",
                    &[
                        JValue::Object(&text_ref),
                        JValue::Float(font.size),
                        JValue::Int(font.weight as i32),
                        JValue::Bool(font.italic),
                        JValue::Object(&family_ref),
                        JValue::Float(limit),
                        JValue::Int(font.max_lines.unwrap_or(0) as i32),
                    ],
                ))
            })
            .ok()
            .flatten();
        let Some(packed) = raw else { return fallback };
        // Width and height arrive packed into a single long, in hundredths of a
        // point: two JNI calls per measurement would cost twice as much for
        // nothing.
        let width = ((packed >> 32) as i32) as f32 / 100.0;
        let height = ((packed & 0xffff_ffff) as i32) as f32 / 100.0;

        let measured = (width, height);
        self.cache.borrow_mut().insert(key, measured);
        measured
    }
}
