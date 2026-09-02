//! Colores en macOS: el análisis lo hace el núcleo, aquí solo se envuelve el
//! resultado en un `NSColor`.
//!
//! `colorWithSRGBRed:...` y no `colorWithRed:...`: el segundo interpreta los
//! componentes en el espacio de color calibrado de AppKit, que no es sRGB, y
//! el mismo `#6ee7b7` saldría de un verde distinto que en iOS y en Android.

pub use an_core::color::{contrast_on, parse};

pub fn to_nscolor(raw: &str) -> Option<objc2::rc::Retained<objc2_app_kit::NSColor>> {
    let (r, g, b, a) = parse(raw)?;
    Some(objc2_app_kit::NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a))
}

/// El mismo color como `CGColor`, que es lo que quieren las capas.
pub fn to_cgcolor(
    raw: &str,
) -> Option<objc2::rc::Retained<objc2_core_graphics::CGColor>> {
    let color = to_nscolor(raw)?;
    Some(color.CGColor().to_owned())
}

/// El color que contrasta con `raw`: el que hay que usar para el rótulo de un
/// botón relleno de ese color. El cálculo lo hace el núcleo, que es donde vive
/// para los tres hosts.
pub fn contrasting(raw: &str) -> Option<objc2::rc::Retained<objc2_app_kit::NSColor>> {
    let (r, g, b, a) = contrast_on(parse(raw)?);
    Some(objc2_app_kit::NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a))
}
