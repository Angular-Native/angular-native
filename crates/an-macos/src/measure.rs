//! Medición de texto con la tipografía real del sistema.
//!
//! Mismo planteamiento que en iOS y por el mismo motivo: el layout pide el
//! tamaño de cada `<Text>` varias veces por nodo y por frame, así que la caché
//! no es una optimización, es lo que evita cientos de cruces a Objective-C por
//! frame. La clave incluye el ancho disponible porque el salto de línea depende
//! de él.
//!
//! Lo que cambia respecto a UIKit es de dónde sale la medida. AppKit no tiene
//! el `boundingRectWithSize:` de `NSString` con las opciones de fragmento de
//! línea que usa el host de iOS, así que se mide sobre un
//! `NSAttributedString`, que sí las tiene y da el mismo resultado.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAttributedStringNSExtendedStringDrawing, NSFont, NSFontAttributeName, NSStringDrawingOptions,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSAttributedString, NSAttributedStringKey, NSDictionary, NSString};

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
pub struct AppKitMeasurer {
    cache: RefCell<HashMap<Key, (f32, f32)>>,
    /// Tamaños naturales de los controles, preguntados en el arranque: aquí no
    /// se pueden consultar, porque este medidor vive en el hilo del motor y
    /// crear un `NSSwitch` exige el principal.
    controls: crate::controls::ControlSizes,
}

impl AppKitMeasurer {
    pub fn new(controls: crate::controls::ControlSizes) -> Self {
        AppKitMeasurer { cache: RefCell::new(HashMap::new()), controls }
    }

    /// Lo que el rótulo se guarda a los lados de su texto, preguntado al
    /// arrancar. Ver `controls::text_inset`: sin sumarlo, un texto en una caja
    /// de su tamaño exacto parte de línea y no se ve ninguna.
    fn text_inset(&self) -> f32 {
        self.controls.get(crate::controls::TEXT_INSET).map(|(w, _)| *w).unwrap_or(0.0)
    }

    pub fn cache_len(&self) -> usize {
        self.cache.borrow().len()
    }

    pub fn nsfont(font: &FontSpec) -> Retained<NSFont> {
        let size = font.size as f64;
        if let Some(family) = &font.family {
            let name = NSString::from_str(family);
            if let Some(f) = NSFont::fontWithName_size(&name, size) {
                return f;
            }
        }
        // Escala CSS 100..900 a la de AppKit, que va de -1 a 1 igual que la de
        // UIKit. La cursiva no tiene fábrica propia en `NSFont`: se pide por
        // rasgo sobre el descriptor, y eso es más trabajo del que merece
        // mientras `fontStyle` solo lo use el rótulo.
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
        NSFont::systemFontOfSize_weight(size, weight)
    }
}

impl TextMeasurer for AppKitMeasurer {
    fn measure_control(&self, name: &str, available_width: Option<f32>) -> (f32, f32) {
        let Some((width, height)) = self.controls.get(name).copied() else {
            return (0.0, 0.0);
        };
        // Los mismos que en iOS ocupan todo el ancho que se les dé; su medida
        // natural solo manda en el alto.
        let stretches =
            matches!(name, "Slider" | "ProgressBar" | "TabBar" | "SearchBar" | "SegmentedControl");
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

        let nsfont = Self::nsfont(font);
        let font_ref: &objc2::runtime::AnyObject = &nsfont;
        let attrs: Retained<NSDictionary<NSAttributedStringKey, _>> =
            NSDictionary::from_slices(&[unsafe { NSFontAttributeName }], &[font_ref]);
        // SAFETY: el diccionario lleva un `NSFont` bajo `NSFontAttributeName`,
        // que es el tipo que ese atributo espera.
        let attributed = unsafe {
            NSAttributedString::new_with_attributes(&NSString::from_str(text), &attrs)
        };

        // El hueco que se le da al texto es el de la caja menos lo que el
        // rótulo se guarda a los lados: medir con el ancho entero haría que
        // partiera de línea una palabra más tarde de lo que va a partir.
        let inset = self.text_inset();
        let constraint = CGSize {
            width: max_width
                .filter(|w| w.is_finite())
                .map(|w| (w - inset).max(0.0))
                .unwrap_or(f32::MAX / 2.0) as f64,
            height: f64::MAX / 2.0,
        };
        let options = NSStringDrawingOptions::UsesLineFragmentOrigin
            | NSStringDrawingOptions::UsesFontLeading;
        let rect = attributed.boundingRectWithSize_options_context(constraint, options, None);

        // `UIFont` tiene `lineHeight` y `NSFont` no: hay que sumar las tres
        // métricas que lo componen. El descendente viene en negativo, así que
        // se resta.
        let natural_line = (nsfont.ascender() - nsfont.descender() + nsfont.leading()) as f32;
        let mut width = rect.size.width as f32 + inset;
        let mut height = rect.size.height as f32;

        // `boundingRect` no conoce `lineHeight` ni `numberOfLines`: se aplican
        // sobre el número de líneas que devolvió, igual que en iOS.
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
        // AppKit devuelve fraccionarios; redondear hacia arriba evita el
        // truncado de la última letra.
        let result = (width.ceil(), height.ceil());
        self.cache.borrow_mut().insert(key, result);
        result
    }
}
