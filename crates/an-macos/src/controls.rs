//! How big each of the system's controls is.
//!
//! Just as on iOS: the framework does not decide it. An `NSSwitch` does not
//! measure the same on Sonoma as on Tahoe, nor does it at the large text size.
//! They are asked once, at startup and on the main thread, because creating an
//! AppKit view off it is not allowed.
//!
//! The difference from iOS is what they are asked. UIKit has `sizeThatFits:`,
//! which is a question; AppKit has `fittingSize`, which is a property and
//! which on top of that makes the control resolve its internal constraints.
//! For whatever comes back zero —the ones that have no content yet— the same
//! fallback as over there holds.

use std::collections::HashMap;

use objc2::rc::Retained;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAttributedStringNSExtendedStringDrawing, NSButton, NSDatePicker, NSFont, NSFontAttributeName,
    NSPopUpButton, NSProgressIndicator, NSProgressIndicatorStyle, NSSearchField,
    NSSegmentedControl, NSSlider, NSStepper, NSStringDrawingOptions, NSSwitch, NSTextField, NSView,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSAttributedString, NSAttributedStringKey, NSDictionary, NSString};

/// The name under which what an `NSTextField` reserves to either side of its
/// text is stored. It is not a control: it is a measurement of the system's
/// that travels the same way, because it is asked for at the same moment and
/// for the same reason.
pub const TEXT_INSET: &str = "__text-inset";

/// Natural sizes, in points, indexed by the control's name.
pub type ControlSizes = HashMap<String, (f32, f32)>;

/// How much wider than its text an AppKit label is.
///
/// The layout measures text with `boundingRectWithSize:`, which measures **the
/// text** and nothing else. An `NSTextField` draws that text inside its cell,
/// and the cell keeps a few points to either side. Today it is four.
///
/// Four points look like nothing and are precisely the worst size of error: a
/// box that comes up short does not show the text clipped, it shows it
/// **empty**. The label has `wraps` on, so whatever does not fit is carried to
/// the next line, and that line is outside the height the layout reserved for
/// one. The text disappears with nothing raising an error, which is exactly
/// what this project does not allow. It shows up the moment a label lands in a
/// box of its own exact size — that is, in any `align-items: center`.
///
/// It is asked for and not written down: it is a measurement of the system's,
/// it changes with the version and with the accessibility settings, just as an
/// `NSSwitch`'s height does.
fn text_inset(mtm: MainThreadMarker) -> f32 {
    let font = NSFont::systemFontOfSize(an_layout::FontSpec::default().size as f64);
    let sample = NSString::from_str("angular-native");

    let label = NSTextField::new(mtm);
    unsafe {
        label.setEditable(false);
        label.setBordered(false);
        label.setBezeled(false);
        label.setDrawsBackground(false);
        label.setFont(Some(&font));
        label.setStringValue(&sample);
    }
    let fits = label.fittingSize().width as f32;

    let font_ref: &objc2::runtime::AnyObject = &font;
    let attrs: Retained<NSDictionary<NSAttributedStringKey, _>> =
        NSDictionary::from_slices(&[unsafe { NSFontAttributeName }], &[font_ref]);
    // SAFETY: the dictionary carries an `NSFont` under `NSFontAttributeName`,
    // which is the type that attribute expects.
    let attributed = unsafe { NSAttributedString::new_with_attributes(&sample, &attrs) };
    let measured = attributed
        .boundingRectWithSize_options_context(
            CGSize { width: f64::MAX / 2.0, height: f64::MAX / 2.0 },
            NSStringDrawingOptions::UsesLineFragmentOrigin
                | NSStringDrawingOptions::UsesFontLeading,
            None,
        )
        .size
        .width as f32;

    // Never negative: were `fittingSize` ever to measure less than the text,
    // taking width away would be worse than doing nothing.
    (fits - measured).max(0.0)
}

/// Asks each control how much room it takes. Called once, at startup.
pub fn measure_controls(mtm: MainThreadMarker) -> ControlSizes {
    let mut sizes = ControlSizes::new();

    let mut record = |name: &str, view: &NSView| {
        let fitted = view.fittingSize();
        sizes.insert(name.to_owned(), (fitted.width as f32, fitted.height as f32));
    };

    record("Switch", &NSSwitch::new(mtm));
    record("Slider", &NSSlider::new(mtm));

    // The spinner and the bar are the same class under different styles, and
    // their natural size depends on the style: each has to be asked once
    // already configured, not a freshly made `NSProgressIndicator`.
    let spinner = NSProgressIndicator::new(mtm);
    spinner.setStyle(NSProgressIndicatorStyle::Spinning);
    record("ActivityIndicator", &spinner);

    let bar = NSProgressIndicator::new(mtm);
    bar.setStyle(NSProgressIndicatorStyle::Bar);
    record("ProgressBar", &bar);

    // A button with no label measures whatever its margins measure. It is
    // given a sample one so the height that comes out is a real button's.
    let button = NSButton::new(mtm);
    button.setTitle(&NSString::from_str("Button"));
    record("Button", &button);

    record("SegmentedControl", &NSSegmentedControl::new(mtm));
    record("Stepper", &NSStepper::new(mtm));
    record("SearchBar", &NSSearchField::new(mtm));
    record("Picker", &NSPopUpButton::new(mtm));
    record("DatePicker", &NSDatePicker::new(mtm));
    // macOS's tab bar is a segmented control (see `support.rs`), so it
    // measures what that one measures. The height is nudged up a little
    // because in the tree it goes as a bar and not as a loose control.
    let tabs = NSSegmentedControl::new(mtm);
    record("TabBar", &tabs);

    sizes.insert(TEXT_INSET.to_owned(), (text_inset(mtm), 0.0));

    // The navigation header is not an AppKit control and here it takes up
    // nothing: on a Mac the header is the window's title bar, and the
    // `[title]` ends up there (see `support.rs`). Zero by zero is the
    // decision, and it is written down: without this line the same zero would
    // come out of not being in the table, which is a different thing and
    // cannot be told apart by looking at the result.
    sizes.insert("NavigationBar".to_owned(), (0.0, 0.0));

    for (name, fallback) in [
        ("Slider", (200.0, 21.0)),
        ("ProgressBar", (200.0, 6.0)),
        ("Button", (80.0, 24.0)),
        ("SegmentedControl", (320.0, 24.0)),
        ("SearchBar", (320.0, 24.0)),
        ("Stepper", (13.0, 27.0)),
        ("Picker", (140.0, 25.0)),
        ("DatePicker", (200.0, 24.0)),
        ("TabBar", (320.0, 32.0)),
        ("Switch", (38.0, 22.0)),
        ("ActivityIndicator", (20.0, 20.0)),
    ] {
        let entry = sizes.entry(name.to_owned()).or_insert(fallback);
        if entry.0 <= 0.0 {
            entry.0 = fallback.0;
        }
        if entry.1 <= 0.0 {
            entry.1 = fallback.1;
        }
    }
    sizes
}
