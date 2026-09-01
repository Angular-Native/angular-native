//! Colores en iOS: el análisis lo hace el núcleo; aquí solo se envuelve el
//! resultado en un `UIColor`.
//!
//! La tabla vivía aquí hasta que apareció un tercer host. Con watchOS pintando
//! por su cuenta, dos copias de la misma tabla eran dos sitios donde `#0b1020`
//! podía dejar de ser el mismo azul.

pub use an_core::color::{contrast_on, parse, Rgba};

pub fn to_uicolor(raw: &str) -> Option<objc2::rc::Retained<objc2_ui_kit::UIColor>> {
    let (r, g, b, a) = parse(raw)?;
    Some(objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a))
}
