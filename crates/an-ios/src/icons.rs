//! The system's icons: SF Symbols.
//!
//! Nothing is drawn and no icon set is bundled. The symbol is asked for by
//! name and the system supplies it, with whatever weight and stroke that
//! version of iOS and the user's accessibility settings give it. An icon like
//! that ages with the system instead of staying pinned to the day it went into
//! the project.
//!
//! The table of common names is `an_core::icons`, not a copy of it. It used to
//! be one here as well, and the core's own comment says why that was wrong:
//! four Apple hosts share these names, so two copies are two places where
//! `back` can stop being `chevron.left`, and an icon that draws differently
//! depending on the screen is the kind of bug nobody sees until a user does.

use objc2::rc::Retained;
use objc2_foundation::NSString;
use objc2_ui_kit::{UIImage, UIImageSymbolConfiguration};

/// The symbol a name maps to, at the size asked for.
///
/// The name may be an SF Symbol's as it stands —`chevron.left`,
/// `square.and.arrow.up`— or one of the common ones, which are translated into
/// each platform's so that nobody has to write two templates.
pub fn symbol(name: &str, size: f32, weight: u16) -> Option<Retained<UIImage>> {
    let resolved = an_core::icons::translate(name);
    let image = unsafe { UIImage::systemImageNamed(&NSString::from_str(resolved)) }?;
    if size <= 0.0 {
        return Some(image);
    }
    // A symbol's size is not an image's: it is asked for through a
    // configuration, and that way the system picks the stroke that goes with
    // it instead of scaling the drawing.
    let config = unsafe {
        UIImageSymbolConfiguration::configurationWithPointSize_weight(
            size as f64,
            symbol_weight(weight),
        )
    };
    unsafe { image.imageByApplyingSymbolConfiguration(&config) }
}

fn symbol_weight(weight: u16) -> objc2_ui_kit::UIImageSymbolWeight {
    use objc2_ui_kit::UIImageSymbolWeight as W;
    match weight {
        0..=299 => W::Light,
        300..=399 => W::Regular,
        400..=499 => W::Regular,
        500..=599 => W::Medium,
        600..=699 => W::Semibold,
        700..=799 => W::Bold,
        _ => W::Heavy,
    }
}
