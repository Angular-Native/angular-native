//! The iOS host: it implements `HostRenderer` and `TextMeasurer` over UIKit,
//! and exposes the runtime to the Xcode shell over FFI.
//!
//! Everything here runs on the main thread. UIKit will have it no other way
//! and objc2's `MainThreadMarker` makes that explicit in the type.

#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod accessibility;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod alert;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod color;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod controls;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod events;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod family;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod ffi;
/// The focus engine only exists on tvOS: it is the platform with no touches.
#[cfg(target_os = "tvos")]
mod focus;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod host;
/// The gaze highlight only exists on visionOS.
#[cfg(target_os = "visionos")]
mod hover;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod icons;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod plugin_views;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod images;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod map;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod measure;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod menu;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod modal;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod modules;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
mod video;
/// The embedded browser does not exist on tvOS: WebKit is not part of its
/// SDK. The whole module leaves the binary, and not only because of the class:
/// its `#[link]` would have the linker look for a framework that is not in
/// that SDK.
#[cfg(any(target_os = "ios", target_os = "visionos"))]
mod web;

#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
pub use host::UikitHost;
#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
pub use measure::UikitMeasurer;

/// A sample tree the pipeline is validated against while there is still no JS
/// engine: a header, a row of two cards and a paragraph that has to wrap on
/// its own.
///
/// It lives outside the iOS `cfg` so it can be checked from `cargo test` with
/// the approximate measurer, with no simulator.
pub fn build_demo(tree: &mut an_core::ShadowTree) -> Result<(), an_core::tree::Error> {
    use an_core::{NodeKind, PropValue};

    // 1 root > 2 header > 3 header text, 4 row > 5,6 cards, 7 paragraph
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
        "UIKit measures this paragraph and taffy places it. No view on this \
         screen is a WebView: they are UIView, UILabel and nothing else.",
    )?;
    tree.insert_child(7, 8, 0)?;
    tree.insert_child(1, 7, 2)?;
    Ok(())
}
