//! Colours on iOS: the core does the parsing; here the result is only wrapped
//! in a `UIColor`.
//!
//! The table lived here until a third host turned up. With watchOS painting on
//! its own, two copies of the same table were two places where `#0b1020` could
//! stop being the same blue.

pub use an_core::color::{contrast_on, parse};

pub fn to_uicolor(raw: &str) -> Option<objc2::rc::Retained<objc2_ui_kit::UIColor>> {
    let (r, g, b, a) = parse(raw)?;
    Some(objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a))
}
