//! Iconos del sistema: SF Symbols, los mismos que en iOS.
//!
//! Los símbolos llegaron a macOS en Big Sur, así que aquí valen los mismos
//! nombres y la misma traducción de los comunes que en el host del teléfono.
//! No se empaqueta ningún juego de iconos: el dibujo lo pone el sistema, con el
//! trazo que le toque a esa versión de macOS.
//!
//! La tabla de nombres comunes está duplicada con la de `an-ios` a sabiendas:
//! sacarla a `an-core` obligaría al núcleo a saber de SF Symbols, que es un
//! detalle de dos plataformas de las cuatro. `scripts/check-macos.sh` comprueba
//! que las dos listas sigan diciendo lo mismo.

use objc2::rc::Retained;
use objc2_app_kit::{NSImage, NSImageSymbolConfiguration};
use objc2_foundation::NSString;

/// El símbolo que corresponde a un nombre, al tamaño pedido.
pub fn symbol(name: &str, size: f32, weight: u16) -> Option<Retained<NSImage>> {
    let resolved = translate(name);
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(resolved),
        None,
    )?;
    if size <= 0.0 {
        return Some(image);
    }
    // Igual que en iOS: el tamaño de un símbolo se pide por configuración,
    // porque no escala el dibujo sino que elige el trazo.
    let config = NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
        size as f64,
        symbol_weight(weight),
        objc2_app_kit::NSImageSymbolScale::Medium,
    );
    image.imageWithSymbolConfiguration(&config)
}

/// Nombres comunes, traducidos al SF Symbol que les toca. La misma lista que
/// en `an-ios`: una plantilla no tiene por qué saber en qué Apple corre.
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
    // `NSImageSymbolConfiguration` toma el peso en la escala de `NSFont`, que
    // va de -1 a 1 y no de 100 a 900.
    match weight {
        0..=299 => unsafe { objc2_app_kit::NSFontWeightLight },
        300..=499 => unsafe { objc2_app_kit::NSFontWeightRegular },
        500..=599 => unsafe { objc2_app_kit::NSFontWeightMedium },
        600..=699 => unsafe { objc2_app_kit::NSFontWeightSemibold },
        700..=799 => unsafe { objc2_app_kit::NSFontWeightBold },
        _ => unsafe { objc2_app_kit::NSFontWeightHeavy },
    }
}
