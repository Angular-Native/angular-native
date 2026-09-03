//! The system's icons: SF Symbols, the same ones as on iOS.
//!
//! The symbols arrived on macOS in Big Sur, so the same names and the same
//! translation of the common ones hold here as in the phone's host. No icon
//! set is bundled: the system supplies the drawing, with whatever stroke that
//! version of macOS gives it.
//!
//! The table of common names is knowingly duplicated from `an-ios`: moving it
//! into `an-core` would force the core to know about SF Symbols, which is a
//! detail of two platforms out of four. Nothing checks that this copy still
//! agrees — `scripts/check-watchos.sh` diffs `an-core` against `an-ios`, and
//! the macOS one is not in that diff.

use objc2::rc::Retained;
use objc2_app_kit::{NSImage, NSImageSymbolConfiguration};
use objc2_foundation::NSString;

/// The symbol a name maps to, at the size asked for.
pub fn symbol(name: &str, size: f32, weight: u16) -> Option<Retained<NSImage>> {
    let resolved = translate(name);
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(resolved),
        None,
    )?;
    if size <= 0.0 {
        return Some(image);
    }
    // Just as on iOS: a symbol's size is asked for through a configuration,
    // because it does not scale the drawing, it picks the stroke.
    let config = NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
        size as f64,
        symbol_weight(weight),
        objc2_app_kit::NSImageSymbolScale::Medium,
    );
    image.imageWithSymbolConfiguration(&config)
}

/// Common names, translated into the SF Symbol each one gets. The same list
/// as in `an-ios`: a template has no business knowing which Apple it runs on.
fn translate(name: &str) -> &str {
    match name {
        "home" => "house.fill",
        "search" => "magnifyingglass",
        "settings" => "gearshape.fill",
        "profile" | "account" => "person.crop.circle.fill",
        "back" => "chevron.left",
        "forward" => "chevron.right",
        "close" => "xmark",
        "add" => "plus",
        "remove" => "minus",
        "delete" => "trash",
        "edit" => "pencil",
        "share" => "square.and.arrow.up",
        "favorite" => "heart.fill",
        "star" => "star.fill",
        "menu" => "line.3.horizontal",
        "more" => "ellipsis",
        "check" => "checkmark",
        "info" => "info.circle",
        "warning" => "exclamationmark.triangle.fill",
        "refresh" => "arrow.clockwise",
        "calendar" => "calendar",
        "camera" => "camera.fill",
        "bell" => "bell.fill",
        "chat" => "bubble.left.fill",
        "mail" => "envelope.fill",
        "list" => "list.bullet",
        "play" => "play.fill",
        "pause" => "pause.fill",
        "download" => "arrow.down.circle",
        "upload" => "arrow.up.circle",
        "location" => "location.fill",
        "lock" => "lock.fill",
        other => other,
    }
}

fn symbol_weight(weight: u16) -> objc2_app_kit::NSFontWeight {
    // `NSImageSymbolConfiguration` takes the weight on `NSFont`'s scale,
    // which runs from -1 to 1 and not from 100 to 900.
    match weight {
        0..=299 => unsafe { objc2_app_kit::NSFontWeightLight },
        300..=499 => unsafe { objc2_app_kit::NSFontWeightRegular },
        500..=599 => unsafe { objc2_app_kit::NSFontWeightMedium },
        600..=699 => unsafe { objc2_app_kit::NSFontWeightSemibold },
        700..=799 => unsafe { objc2_app_kit::NSFontWeightBold },
        _ => unsafe { objc2_app_kit::NSFontWeightHeavy },
    }
}
