//! Text measurement with the system's real typeface.
//!
//! The same approach as on iOS and for the same reason: the layout asks for
//! each `<Text>`'s size several times per node and per frame, so the cache is
//! not an optimisation, it is what avoids hundreds of crossings into
//! Objective-C per frame. The key includes the available width because where
//! the lines break depends on it.
//!
//! What differs from UIKit is where the measurement comes from. AppKit does
//! not have `NSString`'s `boundingRectWithSize:` with the line-fragment
//! options the iOS host uses, so the measuring is done over an
//! `NSAttributedString`, which does have them and gives the same result.

use std::cell::RefCell;
use std::collections::HashMap;

use an_layout::{FontSpec, TextMeasurer};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAttributedStringNSExtendedStringDrawing, NSFont, NSFontAttributeName, NSStringDrawingOptions,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSAttributedString, NSAttributedStringKey, NSDictionary, NSString};

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
    /// Both of these change the answer — the height that comes back is
    /// `lines * line_height` — and neither was here. A key that leaves out
    /// something the result depends on is the worst kind of wrong: it is right
    /// the first time and wrong every time after, and which way round depends
    /// on which text happened to be laid out first. A 40-point line asked for
    /// behind an 18-point one came back as 18.
    line_height_bits: Option<u32>,
    max_lines: Option<u32>,
    max_width_eighths: Option<i32>,
}

#[derive(Default)]
pub struct AppKitMeasurer {
    cache: RefCell<HashMap<Key, (f32, f32)>>,
    /// The controls' natural sizes, asked for at startup: they cannot be
    /// asked about here, because this measurer lives on the engine's thread
    /// and creating an `NSSwitch` demands the main one.
    controls: crate::controls::ControlSizes,
}

impl AppKitMeasurer {
    pub fn new(controls: crate::controls::ControlSizes) -> Self {
        AppKitMeasurer { cache: RefCell::new(HashMap::new()), controls }
    }

    /// What the label keeps to either side of its text, asked for at startup.
    /// See `controls::text_inset`: without adding it back, a text in a box of
    /// its own exact size wraps and not one line of it can be seen.
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
        // Scales CSS's 100..900 onto AppKit's, which runs from -1 to 1 just
        // as UIKit's does. Italic has no factory of its own on `NSFont`: it is
        // asked for as a trait on the descriptor, and that is more work than
        // it is worth while `fontStyle` is only used by the label.
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
            // Never a silent zero: the same as on iOS, and for the same
            // reason. What is not in the table is laid out 0x0 and reads on
            // screen as a layout that went wrong. `controls.rs` is where the
            // entry goes; `scripts/check-measure.sh` is what makes sure it is
            // there.
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
        // The same ones as on iOS take whatever width they are given; their
        // natural measurement only rules the height.
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
            line_height_bits: font.line_height.map(f32::to_bits),
            max_lines: font.max_lines,
            max_width_eighths: max_width
                .filter(|w| w.is_finite())
                .map(|w| (w * 8.0).round() as i32),
        };
        if let Some(hit) = self.cache.borrow().get(&key) {
            return *hit;
        }

        // The kerning goes into the measurement because it goes into the
        // drawing. `letter_spacing` was in this cache key and in nothing else:
        // the host applied it when painting and the measurer did not, so
        // layout reserved a box for text without it and the label drew text
        // with it. Positive spacing came out wider than the box it was given
        // and wrapped a word early, or was cut; negative spacing left a gap
        // nobody asked for. The one direction that looked fine was zero.
        //
        // It is only added when there is any, so text that asks for none is
        // measured with exactly the attributes it was measured with before —
        // an empty `kern` and no `kern` are not quite the same to Core Text.
        let nsfont = Self::nsfont(font);
        let font_ref: &objc2::runtime::AnyObject = &nsfont;
        let kern = objc2_foundation::NSNumber::new_f64(font.letter_spacing as f64);
        let kern_ref: &objc2::runtime::AnyObject = &kern;
        let attrs: Retained<NSDictionary<NSAttributedStringKey, _>> =
            if font.letter_spacing == 0.0 {
                NSDictionary::from_slices(&[unsafe { NSFontAttributeName }], &[font_ref])
            } else {
                NSDictionary::from_slices(
                    &[unsafe { NSFontAttributeName }, unsafe {
                        objc2_app_kit::NSKernAttributeName
                    }],
                    &[font_ref, kern_ref],
                )
            };
        // SAFETY: the dictionary carries an `NSFont` under
        // `NSFontAttributeName`, which is the type that attribute expects.
        let attributed = unsafe {
            NSAttributedString::new_with_attributes(&NSString::from_str(text), &attrs)
        };

        // The room the text is given is the box's minus what the label keeps
        // to either side: measuring with the full width would have it wrap one
        // word later than it is actually going to wrap.
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

        // `UIFont` has `lineHeight` and `NSFont` does not: the three metrics
        // it is made of have to be added up. The descender comes through
        // negative, hence the subtraction.
        let natural_line = (nsfont.ascender() - nsfont.descender() + nsfont.leading()) as f32;
        let mut width = rect.size.width as f32 + inset;
        let mut height = rect.size.height as f32;

        // `boundingRect` knows nothing of `lineHeight` or `numberOfLines`:
        // they are applied to the number of lines it came back with, just as
        // on iOS.
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
        // AppKit returns fractional values; rounding up keeps the last letter
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
