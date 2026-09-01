//! Medición de texto con la tipografía real del sistema.
//!
//! El layout pide el tamaño de cada `<Text>` varias veces por nodo y por frame
//! (mínimo intrínseco, máximo intrínseco, y el definitivo), así que la caché no
//! es una optimización: sin ella cada frame cruza a Objective-C cientos de
//! veces. La clave incluye el ancho disponible porque el salto de línea depende
//! de él.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};
use objc2::rc::Retained;
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSAttributedStringKey, NSDictionary, NSString};
use objc2_ui_kit::{
    NSFontAttributeName, NSStringDrawingOptions, NSStringNSExtendedStringDrawing, UIFont,
};

/// Ancho redondeado a 1/8 de punto: anchos que difieren en flotantes
/// irrelevantes comparten entrada de caché.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    size_bits: u32,
    weight: u16,
    italic: bool,
    family: Option<String>,
    spacing_bits: u32,
    max_width_eighths: Option<i32>,
}

#[derive(Default)]
pub struct UikitMeasurer {
    cache: RefCell<HashMap<Key, (f32, f32)>>,
    /// Tamaños naturales de los controles, preguntados en el arranque. No se
    /// pueden consultar aquí: crear un `UISwitch` exige el hilo principal, y
    /// el medidor vive en el del motor.
    controls: crate::controls::ControlSizes,
}

impl UikitMeasurer {
    pub fn new(controls: crate::controls::ControlSizes) -> Self {
        UikitMeasurer { cache: RefCell::new(HashMap::new()), controls }
    }

    /// Se llama cuando cambia la escala de la pantalla o la fuente dinámica.
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }

    pub fn cache_len(&self) -> usize {
        self.cache.borrow().len()
    }

    fn uifont(font: &FontSpec) -> Retained<UIFont> {
        let size = font.size as f64;
        if let Some(family) = &font.family {
            let name = NSString::from_str(family);
            if let Some(f) = UIFont::fontWithName_size(&name, size) {
                return f;
            }
        }
        if font.italic {
            return UIFont::italicSystemFontOfSize(size);
        }
        // Escala CSS 100..900 a la escala de pesos de UIKit, -1.0..1.0.
        let weight = match font.weight {
            0..=199 => -0.8,
            200..=299 => -0.6,
            300..=399 => -0.4,
            400..=499 => 0.0,
            500..=599 => 0.23,
            600..=699 => 0.3,
            700..=799 => 0.4,
            800..=899 => 0.56,
            _ => 0.62,
        };
        UIFont::systemFontOfSize_weight(size, weight)
    }
}

impl TextMeasurer for UikitMeasurer {
    fn measure_control(&self, name: &str, available_width: Option<f32>) -> (f32, f32) {
        let Some((width, height)) = self.controls.get(name).copied() else {
            return (0.0, 0.0);
        };
        // Deslizadores, barras de progreso y de pestañas ocupan todo el ancho
        // que se les dé; su medida natural solo manda en el alto.
        let stretches = matches!(name, "Slider" | "ProgressBar" | "TabBar");
        match available_width {
            Some(available) if stretches && available.is_finite() => (available, height),
            _ => (width, height),
        }
    }

    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32) {
        let key = Key {
            text: text.to_owned(),
            size_bits: font.size.to_bits(),
            weight: font.weight,
            italic: font.italic,
            family: font.family.clone(),
            spacing_bits: font.letter_spacing.to_bits(),
            max_width_eighths: max_width
                .filter(|w| w.is_finite())
                .map(|w| (w * 8.0).round() as i32),
        };
        if let Some(hit) = self.cache.borrow().get(&key) {
            return *hit;
        }

        let ns_text = NSString::from_str(text);
        let uifont = Self::uifont(font);
        let font_ref: &objc2::runtime::AnyObject = &uifont;
        let attrs: Retained<NSDictionary<NSAttributedStringKey, _>> =
            NSDictionary::from_slices(&[unsafe { NSFontAttributeName }], &[font_ref]);

        let constraint = CGSize {
            width: max_width.filter(|w| w.is_finite()).unwrap_or(f32::MAX / 2.0) as f64,
            height: f64::MAX / 2.0,
        };
        let options = NSStringDrawingOptions::UsesLineFragmentOrigin
            | NSStringDrawingOptions::UsesFontLeading;
        let rect = unsafe {
            ns_text.boundingRectWithSize_options_attributes_context(
                constraint,
                options,
                Some(&attrs),
                None,
            )
        };

        let natural_line = unsafe { uifont.lineHeight() } as f32;
        let mut width = rect.size.width as f32;
        let mut height = rect.size.height as f32;

        // `boundingRect` no conoce `lineHeight` ni `numberOfLines`: los
        // aplicamos sobre el número de líneas que devolvió.
        let lines = if natural_line > 0.0 {
            (height / natural_line).round().max(1.0)
        } else {
            1.0
        };
        let lines = match font.max_lines {
            Some(max) if max > 0 => lines.min(max as f32),
            _ => lines,
        };
        let line_height = font.line_height.unwrap_or(natural_line);
        height = lines * line_height;

        if let Some(limit) = max_width.filter(|w| w.is_finite()) {
            width = width.min(limit);
        }
        // UIKit devuelve fraccionarios; redondear hacia arriba evita el
        // truncado de la última letra.
        let result = (width.ceil(), height.ceil());
        self.cache.borrow_mut().insert(key, result);
        result
    }
}
