//! El realce de visionOS.
//!
//! visionOS sí tiene toques —el pellizco con la mano cuenta como un toque
//! indirecto y llega por los mismos reconocedores que en iOS—, así que aquí no
//! hace falta ningún motor de foco. Lo que sí cambia es cómo sabe el usuario
//! qué va a pulsar: apunta con la mirada, y sin realce no hay forma de ver
//! dónde está apuntando.
//!
//! Ese realce lo dibuja el sistema, fuera del proceso de la app y sin pasar por
//! nuestro ciclo de frames, pero solo si la vista lo pide con `hoverStyle`. Una
//! `UIView` con un reconocedor de toque no lo pide: el valor de fábrica es
//! `nil`, «esta vista no tiene realce». `UIButton` y compañía sí lo traen, por
//! lo mismo que en tvOS traen el foco.
//!
//! `automaticStyle` es el realce del sistema con la forma que el propio UIKit
//! deduce de la vista. No se dibuja nada a mano: si visionOS cambia el aspecto
//! del realce en una versión, esto cambia con él.

use objc2_ui_kit::{UIHoverStyle, UIView};

/// Marca una vista como pulsable a la vista del usuario.
pub fn mark_pressable(mtm: objc2::MainThreadMarker, view: &UIView) {
    if view.hoverStyle().is_some() {
        return;
    }
    view.setHoverStyle(Some(&UIHoverStyle::automaticStyle(mtm)));
}
