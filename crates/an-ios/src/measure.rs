//! Text measurement with the system's real typeface.
//!
//! The layout asks for each `<Text>`'s size several times per node and per
//! frame (intrinsic minimum, intrinsic maximum, and the final one), so the
//! cache is not an optimisation: without it every frame crosses into
//! Objective-C hundreds of times. The key includes the available width because
//! where the lines break depends on it.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};
use objc2::rc::Retained;
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSAttributedStringKey, NSDictionary, NSString};
use objc2_ui_kit::{
    NSFontAttributeName, NSStringDrawingOptions, NSStringNSExtendedStringDrawing, UIFont,
};

/// Width rounded to 1/8 of a point: widths that differ by floats nobody cares
/// about share a cache entry.
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
    /// The controls' natural sizes, asked for at startup. They cannot be
    /// asked about here: creating a `UISwitch` demands the main thread, and
    /// the measurer lives on the engine's.
    controls: crate::controls::ControlSizes,
}

impl UikitMeasurer {
    pub fn new(controls: crate::controls::ControlSizes) -> Self {
        UikitMeasurer { cache: RefCell::new(HashMap::new()), controls }
    }

    /// Called when the screen's scale or the dynamic type size changes.
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
        // Scales CSS's 100..900 onto UIKit's weight scale, -1.0..1.0.
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
            // Never a silent zero. A control the core asks about and that is
            // not in the table is laid out 0x0, which looks like a layout gone
            // wrong and not like a table missing an entry —which is how
            // `<an-navigation-bar>` went unnoticed here. `controls.rs` is
            // where the entry goes, and `scripts/check-measure.sh` is what
            // stops the next one from shipping.
            warn_once(
                &format!("unmeasured:{name}"),
                &format!(
                    "{name} has no measurement in this host: the core asks for it in \
                     NodeKind::control_name() and controls.rs never recorded it, so it is \
                     laid out 0x0 and cannot be seen"
                ),
            );
            return (0.0, 0.0);
        };
        // Sliders, progress bars, tab bars and the header take whatever width
        // they are given; their natural measurement only rules the height.
        let stretches = matches!(
            name,
            "Slider" | "ProgressBar" | "TabBar" | "SearchBar" | "SegmentedControl" | "NavigationBar"
        );
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

        // `boundingRect` knows nothing of `lineHeight` or `numberOfLines`:
        // they are applied to the number of lines it came back with.
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
        // UIKit returns fractional values; rounding up keeps the last letter
        // from being clipped.
        let result = (width.ceil(), height.ceil());
        self.cache.borrow_mut().insert(key, result);
        result
    }
}

/// What has already been said. The layout measures several times per node and
/// per frame, so a warning said here without this would be said hundreds of
/// times a second. Same reason and same shape as `accessibility.rs`'s.
fn warn_once(key: &str, message: &str) {
    thread_local! {
        static SAID: std::cell::RefCell<std::collections::HashSet<String>> =
            std::cell::RefCell::new(std::collections::HashSet::new());
    }
    SAID.with(|said| {
        if said.borrow_mut().insert(key.to_owned()) {
            eprintln!("angular-native: {message}");
        }
    });
}
