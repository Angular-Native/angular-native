//! Host iOS: implementa `HostRenderer` y `TextMeasurer` sobre UIKit, y expone
//! el runtime al shell de Xcode por FFI.
//!
//! Todo aquí corre en el hilo principal. UIKit no admite otra cosa y el
//! `MainThreadMarker` de objc2 lo hace explícito en el tipo.

mod color;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod events;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod ffi;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod host;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod measure;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod modules;

#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
pub use host::UikitHost;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
pub use measure::UikitMeasurer;

/// Árbol de ejemplo con el que se valida el pipeline sin motor JS todavía:
/// cabecera, fila de dos tarjetas y un párrafo que tiene que partir solo.
///
/// Vive fuera del `cfg` de iOS para poder comprobarlo en `cargo test` con el
/// medidor aproximado, sin simulador.
pub fn build_demo(tree: &mut an_core::ShadowTree) -> Result<(), an_core::tree::Error> {
    use an_core::{NodeKind, PropValue};

    // 1 raíz > 2 cabecera > 3 texto cabecera, 4 fila > 5,6 tarjetas, 7 párrafo
    tree.create_node(1, NodeKind::View)?;
    tree.set_style(1, "width", "100%")?;
    tree.set_style(1, "height", "100%")?;
    tree.set_style(1, "paddingTop", "64")?;
    tree.set_style(1, "paddingHorizontal", "16")?;
    tree.set_style(1, "gap", "16")?;
    tree.set_prop(1, "backgroundColor", PropValue::Str("#0b1020".into()))?;
    tree.set_root(1)?;

    tree.create_node(2, NodeKind::Text)?;
    tree.set_prop(2, "fontSize", PropValue::Number(28.0))?;
    tree.set_prop(2, "fontWeight", PropValue::Str("bold".into()))?;
    tree.set_prop(2, "color", PropValue::Str("#f4f7ff".into()))?;
    tree.create_node(3, NodeKind::RawText)?;
    tree.set_text(3, "angular-native")?;
    tree.insert_child(2, 3, 0)?;
    tree.insert_child(1, 2, 0)?;

    tree.create_node(4, NodeKind::View)?;
    tree.set_style(4, "flexDirection", "row")?;
    tree.set_style(4, "gap", "12")?;
    tree.insert_child(1, 4, 1)?;

    for (id, color, grow) in [(5_u32, "#1e2a4a", 1.0_f32), (6, "#2b1e4a", 2.0)] {
        tree.create_node(id, NodeKind::View)?;
        tree.set_style(id, "flexGrow", &grow.to_string())?;
        tree.set_style(id, "height", "88")?;
        tree.set_style(id, "borderRadius", "12")?;
        tree.set_prop(id, "backgroundColor", PropValue::Str(color.into()))?;
        tree.set_prop(id, "borderRadius", PropValue::Number(12.0))?;
        tree.insert_child(4, id, (id - 5) as usize)?;
    }

    tree.create_node(7, NodeKind::Text)?;
    tree.set_prop(7, "fontSize", PropValue::Number(16.0))?;
    tree.set_prop(7, "color", PropValue::Str("#9fb0d4".into()))?;
    tree.create_node(8, NodeKind::RawText)?;
    tree.set_text(
        8,
        "Este párrafo lo mide UIKit y lo coloca taffy. Ninguna vista de esta \
         pantalla es un WebView: son UIView, UILabel y nada más.",
    )?;
    tree.insert_child(7, 8, 0)?;
    tree.insert_child(1, 7, 2)?;
    Ok(())
}
