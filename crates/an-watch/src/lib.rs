//! Host de watchOS.
//!
//! No es el host de iOS con otro `cfg`: es otra cosa. En watchOS no existe la
//! jerarquía de `UIView` sobre la que se apoyan `an-ios` y `an-android`. La
//! interfaz la dibuja SwiftUI y no hay forma de saltárselo.
//!
//! Eso choca de frente con el modelo de montaje del proyecto, que es
//! imperativo: crea una vista, métela aquí, cámbiale el marco. SwiftUI no
//! admite órdenes, solo estado: se le describe qué hay y él decide qué
//! redibujar. La salida es reflejar el árbol de Rust en un modelo que SwiftUI
//! observe, y convertir cada `MountOp` en una mutación de ese modelo.
//!
//! Reparto de trabajo:
//!
//! ```text
//!   Rust                                   Swift
//!   ────────────────────────────────       ─────────────────────────────
//!   QuickJS ─▶ ShadowTree ─▶ taffy         TimelineView (un tick por frame)
//!                  │ commit                       │ an_watch_runtime_frame
//!                  ▼                              ▼
//!            Frame { MountOp[] }            ¿cambió la revisión?
//!                  │                              │ sí
//!                  ▼                              ▼
//!             WatchHost (modelo)  ──JSON──▶  AnTree (@Observable)
//!                                                  │
//!                                                  ▼
//!                                            ZStack + .offset
//! ```
//!
//! El layout sigue siendo de taffy. Swift coloca cada nodo por su marco
//! absoluto y no usa `VStack`/`HStack` para posicionar: si los usara habría dos
//! motores de layout decidiendo lo mismo, y ganaría el que corriera después.

pub mod host;
pub mod measure;
pub mod snapshot;

#[cfg(target_os = "watchos")]
mod ffi;

pub use host::{WatchHost, WatchNode};
pub use measure::{ControlSizes, WatchMeasurer};
pub use snapshot::{snapshot, Snapshot};

#[cfg(test)]
mod tests {
    use an_core::{NodeKind, PropValue, ShadowTree};
    use an_host::{new_event_queue, MountSide};

    use crate::host::WatchHost;
    use crate::measure::WatchMeasurer;

    /// Monta un árbol pequeño y comprueba que la foto que sale es la que el
    /// shell espera: raíz con fondo, un texto con su cadena ya fundida, y un
    /// botón que dice que se puede pulsar.
    ///
    /// Es la prueba que corre en el Mac. El simulador comprueba lo que esta no
    /// puede —que SwiftUI lo pinte—, pero que el modelo salga bien no hace
    /// falta un reloj para verlo.
    #[test]
    fn la_foto_lleva_lo_que_el_shell_necesita() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "176").unwrap();
        tree.set_style(1, "height", "223").unwrap();
        tree.set_prop(1, "backgroundColor", PropValue::Str("#0b1020".into())).unwrap();
        tree.set_root(1).unwrap();

        tree.create_node(2, NodeKind::Text).unwrap();
        tree.set_prop(2, "fontSize", PropValue::Number(16.0)).unwrap();
        tree.set_prop(2, "color", PropValue::Str("#ffffff".into())).unwrap();
        tree.create_node(3, NodeKind::RawText).unwrap();
        tree.set_text(3, "hola ").unwrap();
        tree.create_node(4, NodeKind::RawText).unwrap();
        tree.set_text(4, "reloj").unwrap();
        tree.insert_child(2, 3, 0).unwrap();
        tree.insert_child(2, 4, 1).unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        tree.create_node(5, NodeKind::Button).unwrap();
        tree.set_prop(5, "title", PropValue::Str("pulsa".into())).unwrap();
        tree.set_listener(5, "press", true).unwrap();
        tree.insert_child(1, 5, 1).unwrap();

        let events = new_event_queue();
        let mut mount = MountSide::new(WatchHost::new(events));
        let frame = tree.commit((176.0, 223.0), &WatchMeasurer::new(Default::default())).unwrap();
        mount.apply(&frame);

        let snapshot = crate::snapshot::snapshot(mount.host());
        let root = snapshot.root.expect("tiene que haber raíz");
        assert_eq!(root.kind, "View");
        assert_eq!(root.background, Some([11.0 / 255.0, 16.0 / 255.0, 32.0 / 255.0, 1.0]));
        assert_eq!(root.children.len(), 2, "los RawText no bajan al shell");

        let text = &root.children[0];
        assert_eq!(text.kind, "Text");
        // Los dos `RawText` se funden: SwiftUI quiere la cadena entera.
        assert_eq!(text.text.as_deref(), Some("hola reloj"));
        assert_eq!(text.font_size, Some(16.0));
        assert!(text.children.is_empty());

        let button = &root.children[1];
        assert_eq!(button.kind, "Button");
        assert_eq!(button.text.as_deref(), Some("pulsa"));
        assert!(button.listens.iter().any(|e| e == "press"), "la plantilla puso un (press)");
    }

    /// Un nodo que se va tiene que irse también de la foto, y la revisión tiene
    /// que subir: si no subiera, SwiftUI seguiría enseñando lo que ya no está.
    #[test]
    fn quitar_un_nodo_sube_la_revision_y_lo_saca_de_la_foto() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "176").unwrap();
        tree.set_style(1, "height", "223").unwrap();
        tree.set_root(1).unwrap();
        tree.create_node(2, NodeKind::View).unwrap();
        tree.set_style(2, "height", "20").unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        let measurer = WatchMeasurer::new(Default::default());
        let mut mount = MountSide::new(WatchHost::new(new_event_queue()));
        mount.apply(&tree.commit((176.0, 223.0), &measurer).unwrap());
        let antes = mount.host().revision();
        assert_eq!(crate::snapshot::snapshot(mount.host()).root.unwrap().children.len(), 1);

        tree.remove_child(1, 2).unwrap();
        tree.destroy_node(2).unwrap();
        mount.apply(&tree.commit((176.0, 223.0), &measurer).unwrap());

        assert!(mount.host().revision() > antes, "un cambio tiene que subir la revisión");
        assert!(crate::snapshot::snapshot(mount.host()).root.unwrap().children.is_empty());
    }
}
