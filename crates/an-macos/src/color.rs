//! Colours on macOS: the core does the parsing, here the result is only
//! wrapped in an `NSColor`.
//!
//! `colorWithSRGBRed:...` and not `colorWithRed:...`: the second reads the
//! components in AppKit's calibrated colour space, which is not sRGB, and the
//! very same `#6ee7b7` would come out a different green from the one on iOS
//! and on Android.

pub use an_core::color::{contrast_on, parse};

pub fn to_nscolor(raw: &str) -> Option<objc2::rc::Retained<objc2_app_kit::NSColor>> {
    let (r, g, b, a) = parse(raw)?;
    Some(objc2_app_kit::NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a))
}

/// The same colour as a `CGColor`, which is what layers want.
pub fn to_cgcolor(
    raw: &str,
) -> Option<objc2::rc::Retained<objc2_core_graphics::CGColor>> {
    let color = to_nscolor(raw)?;
    Some(color.CGColor().to_owned())
}

/// The colour that contrasts with `raw`: the one to use for the label of a
/// button filled with that colour. The core does the arithmetic, which is
/// where it lives for all three hosts.
pub fn contrasting(raw: &str) -> Option<objc2::rc::Retained<objc2_app_kit::NSColor>> {
    let (r, g, b, a) = contrast_on(parse(raw)?);
    Some(objc2_app_kit::NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a))
}
