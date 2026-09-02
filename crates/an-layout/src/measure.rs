//! Measuring leaf nodes. Layout cannot resolve a `<Text>` without asking
//! somebody how much room that text takes up in that font.
//!
//! On iOS/Android the platform answers (UIKit / android.text). In tests and in
//! CI the answer comes from `NaiveMeasurer`, which approximates it without
//! depending on the system.

/// The font a `<Text>` is measured with.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub size: f32,
    /// 100..900, the CSS scale.
    pub weight: u16,
    pub italic: bool,
    pub family: Option<String>,
    /// Absolute line height in points. `None` = derived from the size.
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
    /// Truncated to N lines. `None` = no limit.
    pub max_lines: Option<u32>,
}

impl Default for FontSpec {
    fn default() -> Self {
        FontSpec {
            size: 14.0,
            weight: 400,
            italic: false,
            family: None,
            line_height: None,
            letter_spacing: 0.0,
            max_lines: None,
        }
    }
}

/// What there is to measure on a leaf node.
#[derive(Clone, Debug)]
pub enum MeasureCtx {
    Text { text: String, font: FontSpec },
    /// An image with a known intrinsic size (width, height).
    Image { intrinsic: (f32, f32) },
    /// A system control —a switch, a slider, a tab bar—. How big it is is not
    /// the framework's call: it is the platform's, and it changes between
    /// versions of the system.
    Control { name: String },
}

/// Each platform implements it. It has to be pure: same inputs, same output,
/// or layout oscillates from frame to frame.
pub trait TextMeasurer {
    /// `max_width` = `None` when the available width is infinite.
    /// Returns (width, height) in logical points.
    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32);

    /// The intrinsic minimum of a text: the narrowest it can get without
    /// breaking words in half, which is the width of its longest word.
    ///
    /// It has to be asked for separately and not slipped in as "available width
    /// zero": measuring with width zero returns zero on both UIKit and Android,
    /// and a text with a minimum of zero shrinks to nothing the moment its
    /// container stops imposing a width on it —inside an `alignItems: center`,
    /// for instance—. The text used to vanish off the screen without anything
    /// failing.
    fn measure_text_min_content(&self, text: &str, font: &FontSpec) -> (f32, f32) {
        let widest = text
            .split_whitespace()
            .map(|word| self.measure_text(word, font, None).0)
            .fold(0.0_f32, f32::max);
        if widest <= 0.0 {
            // A text with no spaces —or an empty one— has nothing to break:
            // its minimum is its whole size.
            return self.measure_text(text, font, None);
        }
        // The height is that of the text broken at that width, which is more
        // than one line: the intrinsic minimum is width *and* height, and
        // settling for the height of a single line would clip the text.
        let (_, height) = self.measure_text(text, font, Some(widest));
        (widest, height)
    }

    /// The natural size of a system control.
    ///
    /// The default implementation returns sensible numbers so that the core is
    /// usable with no platform behind it; each host replaces it by asking the
    /// real control, which is the one that knows how much room it takes on this
    /// version of the system and with this user's accessibility settings.
    fn measure_control(&self, name: &str, _available_width: Option<f32>) -> (f32, f32) {
        match name {
            "Switch" => (51.0, 31.0),
            "Slider" => (200.0, 32.0),
            "ActivityIndicator" => (20.0, 20.0),
            "ProgressBar" => (200.0, 4.0),
            "Button" => (80.0, 44.0),
            "TabBar" => (320.0, 49.0),
            // The default size of an icon. `[size]` can change it, and besides
            // configuring the symbol it pins the width and the height: that way
            // an icon with no measurements does not end up invisible.
            "Icon" => (24.0, 24.0),
            "SegmentedControl" => (320.0, 32.0),
            "Stepper" => (94.0, 32.0),
            "SearchBar" => (320.0, 56.0),
            // The dropdown, under the name the core uses. The tag has been
            // called `<an-select>` ever since tags took a prefix, but what
            // arrives here is `NodeKind::control_name()`, and that still says
            // `Picker`. For as long as this said "Select" nobody recognised it
            // and the dropdown measured zero: no error, no trace, and only
            // visible at all if the template gave it no explicit height.
            "Picker" => (140.0, 44.0),
            "DatePicker" => (200.0, 44.0),
            "NavigationBar" => (320.0, 44.0),
            _ => (0.0, 0.0),
        }
    }
}

/// A monospaced approximation. Good for tests, and for not holding up the core
/// while the native layer does not exist. Not for production.
#[derive(Clone, Copy, Debug, Default)]
pub struct NaiveMeasurer;

impl NaiveMeasurer {
    const ADVANCE_RATIO: f32 = 0.55;
    const LINE_RATIO: f32 = 1.25;
}

impl TextMeasurer for NaiveMeasurer {
    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32) {
        let advance = font.size * Self::ADVANCE_RATIO + font.letter_spacing;
        let line_height = font.line_height.unwrap_or(font.size * Self::LINE_RATIO);
        let char_width = |s: &str| s.chars().count() as f32 * advance;

        let Some(limit) = max_width.filter(|w| w.is_finite() && *w > 0.0) else {
            let widest = text.lines().map(char_width).fold(0.0_f32, f32::max);
            let lines = text.lines().count().max(1) as f32;
            return (widest, lines * line_height);
        };

        // Wrapping by words; a word wider than the limit overflows, it does
        // not get broken. That is what UIKit does by default.
        let mut lines = 0_u32;
        let mut widest = 0.0_f32;
        for paragraph in text.split('\n') {
            let mut current = 0.0_f32;
            let mut started = false;
            for word in paragraph.split_whitespace() {
                let w = char_width(word);
                let candidate = if started { current + advance + w } else { w };
                if started && candidate > limit {
                    widest = widest.max(current);
                    lines += 1;
                    current = w;
                } else {
                    current = candidate;
                    started = true;
                }
            }
            widest = widest.max(current);
            lines += 1;
        }

        let lines = match font.max_lines {
            Some(max) if max > 0 => lines.min(max),
            _ => lines,
        };
        (widest.min(limit), lines.max(1) as f32 * line_height)
    }
}

#[cfg(test)]
mod min_content_tests {
    use super::*;

    /// A text's minimum is its longest word, not zero.
    ///
    /// This is the case that made text vanish on the device: a `<Text>` inside
    /// a centred container has no width imposed on it, so layout settles for
    /// the minimum, and with the minimum at zero the text was left in a box of
    /// zero width.
    #[test]
    fn the_minimum_is_the_longest_word() {
        let font = FontSpec { size: 10.0, ..Default::default() };
        let (width, height) = NaiveMeasurer.measure_text_min_content("hello huge world", &font);
        let (alone, _) = NaiveMeasurer.measure_text("hello", &font, None);
        assert_eq!(width, alone);
        assert!(width > 0.0);
        // Broken at that width it takes three lines: the height is not that of
        // a single one.
        let (_, one_line) = NaiveMeasurer.measure_text("hello", &font, None);
        assert!(height > one_line);
    }

    /// A single word does not get broken: its minimum is the whole of it.
    #[test]
    fn a_lone_word_does_not_get_broken() {
        let font = FontSpec { size: 10.0, ..Default::default() };
        let whole = NaiveMeasurer.measure_text("indivisible", &font, None);
        assert_eq!(NaiveMeasurer.measure_text_min_content("indivisible", &font), whole);
    }
}
