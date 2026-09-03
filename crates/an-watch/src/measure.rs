//! Text measurement on the watch.
//!
//! watchOS has no `UIView`, but it does have `UIFont` and Foundation's string
//! drawing (`boundingRectWithSize:`), which is exactly what is needed: taffy
//! has to know how much room a `<Text>` takes before it can place it, and that
//! does not depend on a view hierarchy existing.
//!
//! The cache is not an optimisation. The layout asks for each text node's size
//! several times per frame —intrinsic minimum, maximum, and the final one— and
//! without a cache every frame would cross into Objective-C dozens of times.
//! The key includes the available width because where the lines break depends
//! on it.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};

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

/// The controls' natural sizes, indexed by name. On the watch SwiftUI draws
/// them, so unlike on iOS there is no real `UISwitch` to ask: the shell
/// measures them at startup and they arrive already worked out.
pub type ControlSizes = HashMap<String, (f32, f32)>;

pub struct WatchMeasurer {
    cache: RefCell<HashMap<Key, (f32, f32)>>,
    controls: ControlSizes,
}

impl WatchMeasurer {
    pub fn new(controls: ControlSizes) -> Self {
        WatchMeasurer { cache: RefCell::new(HashMap::new()), controls }
    }

    pub fn cache_len(&self) -> usize {
        self.cache.borrow().len()
    }
}

impl TextMeasurer for WatchMeasurer {
    fn measure_control(&self, name: &str, available_width: Option<f32>) -> (f32, f32) {
        let Some((width, height)) = self.controls.get(name).copied() else {
            return (0.0, 0.0);
        };
        // On a 40 mm screen a control that does not take the full width looks
        // wrong and is hard to hit with a finger, so it is stretched. It is
        // watchOS's own convention: its rows run edge to edge.
        //
        // `Icon` is not in the list: the template fixes its size with `[size]`,
        // and stretching it would give a symbol as wide as the screen.
        let stretches = matches!(
            name,
            "Button"
                | "Slider"
                | "ProgressBar"
                | "SegmentedControl"
                | "Switch"
                | "Stepper"
                | "Picker"
                | "DatePicker"
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
        let result = platform::measure(text, font, max_width);
        self.cache.borrow_mut().insert(key, result);
        result
    }
}

#[cfg(target_os = "watchos")]
mod platform {
    use an_layout::FontSpec;
    use objc2::rc::Retained;
    use objc2_core_foundation::CGSize;
    use objc2_foundation::{NSAttributedStringKey, NSDictionary, NSString};
    use objc2_ui_kit::{
        NSFontAttributeName, NSStringDrawingOptions, NSStringNSExtendedStringDrawing, UIFont,
    };

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
        // Scales CSS's 100..900 onto UIKit's weight scale, -1.0..1.0. It is
        // the same table the iOS host uses: were they to diverge, the same text
        // would measure differently on each platform.
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

    pub fn measure(text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32) {
        let ns_text = NSString::from_str(text);
        let uifont = uifont(font);
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
        let lines = if natural_line > 0.0 { (height / natural_line).round().max(1.0) } else { 1.0 };
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
        (width.ceil(), height.ceil())
    }
}

/// Off the watch there is no UIKit. Measuring falls back to the core's
/// approximation, which is enough for the model and serialisation tests.
#[cfg(not(target_os = "watchos"))]
mod platform {
    use an_layout::{FontSpec, NaiveMeasurer, TextMeasurer};

    pub fn measure(text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32) {
        NaiveMeasurer.measure_text(text, font, max_width)
    }
}
