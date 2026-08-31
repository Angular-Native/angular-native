//! Medición de texto con `android.text.StaticLayout`, vía Kotlin.
//!
//! Misma caché que en iOS y por el mismo motivo: el layout pide el tamaño de
//! cada nodo varias veces por frame, y aquí cada consulta cruza JNI, que es
//! bastante más caro que un `objc_msgSend`.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};
use jni::objects::{GlobalRef, JValue};
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
    host: GlobalRef,
    cache: RefCell<HashMap<Key, (f32, f32)>>,
}

impl JniMeasurer {
    pub fn new(vm: JavaVM, host: GlobalRef) -> Self {
        JniMeasurer { vm, host, cache: RefCell::new(HashMap::new()) }
    }

    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }
}

impl TextMeasurer for JniMeasurer {
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
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return fallback;
        };
        let Ok(text_ref) = env.new_string(text) else {
            return fallback;
        };
        let family = font.family.clone().unwrap_or_default();
        let Ok(family_ref) = env.new_string(&family) else {
            return fallback;
        };
        // Ancho infinito viaja como -1: JNI no tiene Option.
        let limit = max_width.filter(|w| w.is_finite()).unwrap_or(-1.0);

        let result = env.call_method(
            self.host.as_obj(),
            "measureText",
            "(Ljava/lang/String;FIZLjava/lang/String;FI)J",
            &[
                JValue::Object(&text_ref),
                JValue::Float(font.size),
                JValue::Int(font.weight as i32),
                JValue::Bool(font.italic as u8),
                JValue::Object(&family_ref),
                JValue::Float(limit),
                JValue::Int(font.max_lines.unwrap_or(0) as i32),
            ],
        );
        let packed = match result.and_then(|value| value.j()) {
            Ok(packed) => packed,
            Err(_) => {
                let _ = env.exception_clear();
                return fallback;
            }
        };
        // Ancho y alto llegan empaquetados en un long, en centésimas de punto:
        // dos llamadas JNI por medición costarían el doble por nada.
        let width = ((packed >> 32) as i32) as f32 / 100.0;
        let height = ((packed & 0xffff_ffff) as i32) as f32 / 100.0;

        let measured = (width, height);
        self.cache.borrow_mut().insert(key, measured);
        measured
    }
}
