//! Iconos del sistema: SF Symbols.
//!
//! No se dibuja nada ni se empaqueta ningún juego de iconos. Se pide el
//! símbolo por su nombre y lo entrega el sistema, con el peso y el grosor que
//! le toquen a esa versión de iOS y a los ajustes de accesibilidad del
//! usuario. Un icono así envejece con el sistema en vez de quedarse anclado al
//! día en que se metió en el proyecto.

use objc2::rc::Retained;
use objc2_foundation::NSString;
use objc2_ui_kit::{UIImage, UIImageSymbolConfiguration};

/// El símbolo que corresponde a un nombre, al tamaño pedido.
///
/// El nombre puede ser el de un SF Symbol tal cual —`chevron.left`,
/// `square.and.arrow.up`— o uno de los comunes, que se traducen al de cada
/// plataforma para no obligar a escribir dos plantillas.
pub fn symbol(name: &str, size: f32, weight: u16) -> Option<Retained<UIImage>> {
    let resolved = translate(name);
    let image = unsafe { UIImage::systemImageNamed(&NSString::from_str(resolved)) }?;
    if size <= 0.0 {
        return Some(image);
    }
    // El tamaño de un símbolo no es el de una imagen: se pide por
    // configuración, y así el sistema elige el trazo que le corresponde en vez
    // de escalar el dibujo.
    let config = unsafe {
        UIImageSymbolConfiguration::configurationWithPointSize_weight(
            size as f64,
            symbol_weight(weight),
        )
    };
    unsafe { image.imageByApplyingSymbolConfiguration(&config) }
}

/// Nombres comunes, traducidos al SF Symbol que les toca.
///
/// La lista es corta a propósito: cubre lo que lleva casi cualquier app —una
/// barra de pestañas, una cabecera— y para lo demás se escribe el nombre del
/// símbolo directamente, que son más de cinco mil y no tiene sentido
/// duplicarlos aquí.
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
