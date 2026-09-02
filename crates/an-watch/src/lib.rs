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

    /// The six accessibility props, as the shell receives them.
    ///
    /// This is the closest the watch gets to being checked. The other three
    /// Apple hosts can be read from outside the app — a separate process walks
    /// their accessibility tree — and watchOS cannot: there is no route into a
    /// watch simulator's tree, so nothing here proves a reader would announce
    /// any of it.
    ///
    /// What it does prove is the half that is ours. The whole translation from
    /// the contract to SwiftUI happens in Rust, in `accessibility_traits_of`,
    /// and the shell only attaches what arrives; so if the snapshot carries the
    /// right trait names and the right strings, everything left between here
    /// and a spoken word belongs to SwiftUI. That is worth having and it is
    /// worth not overselling.
    #[test]
    fn la_foto_lleva_la_accesibilidad_traducida_a_swiftui() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "176").unwrap();
        tree.set_style(1, "height", "223").unwrap();
        tree.set_root(1).unwrap();

        // A row acting as a switch, on, and read as one stop.
        tree.create_node(2, NodeKind::View).unwrap();
        tree.set_style(2, "height", "40").unwrap();
        tree.set_prop(2, "accessibilityLabel", PropValue::Str("Night mode".into())).unwrap();
        tree.set_prop(2, "accessibilityHint", PropValue::Str("dims the screen".into())).unwrap();
        tree.set_prop(2, "accessibilityRole", PropValue::Str("switch".into())).unwrap();
        tree.set_prop(2, "accessibilityState", PropValue::Str(r#"{"checked":true}"#.into()))
            .unwrap();
        tree.set_prop(2, "accessible", PropValue::Bool(true)).unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        // And a row asking for the two things SwiftUI has no trait for.
        tree.create_node(3, NodeKind::View).unwrap();
        tree.set_style(3, "height", "40").unwrap();
        tree.set_prop(3, "accessibilityRole", PropValue::Str("radio".into())).unwrap();
        tree.set_prop(3, "accessibilityState", PropValue::Str(r#"{"selected":true}"#.into()))
            .unwrap();
        tree.insert_child(1, 3, 1).unwrap();

        let events = new_event_queue();
        let mut mount = MountSide::new(WatchHost::new(events));
        let frame = tree.commit((176.0, 223.0), &WatchMeasurer::new(Default::default())).unwrap();
        mount.apply(&frame);

        let snapshot = crate::snapshot::snapshot(mount.host());
        let root = snapshot.root.expect("tiene que haber raíz");

        let row = &root.children[0];
        assert_eq!(row.accessibility_label.as_deref(), Some("Night mode"));
        assert_eq!(row.accessibility_hint.as_deref(), Some("dims the screen"));
        assert_eq!(row.accessible, Some(true));
        // The role travels already translated: the shell never sees "switch".
        assert_eq!(row.accessibility_traits, vec!["isToggle"]);
        // `checked` is nobody's trait — it goes in the value, and with the
        // platform's own convention rather than a word of ours.
        assert_eq!(row.accessibility_value.as_deref(), Some("1"));

        let gap = &root.children[1];
        // `radio` has no SwiftUI trait, so nothing is sent for it — and
        // `selected` does have one, so the row is not left empty either.
        assert_eq!(gap.accessibility_traits, vec!["isSelected"]);
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

    /// El diálogo y la hoja no viajan dentro del árbol: SwiftUI no los coloca,
    /// los presenta, y el shell los cuelga de la raíz. Si volvieran a bajar como
    /// hijos, el shell tendría que buscarlos por dentro en cada frame.
    #[test]
    fn el_dialogo_y_la_hoja_salen_del_arbol() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "208").unwrap();
        tree.set_style(1, "height", "248").unwrap();
        tree.set_root(1).unwrap();

        tree.create_node(2, NodeKind::Alert).unwrap();
        tree.set_prop(2, "visible", PropValue::Bool(true)).unwrap();
        tree.set_prop(2, "title", PropValue::Str("batería".into())).unwrap();
        tree.set_prop(2, "buttons", PropValue::Str(r#"["vale","ahora no"]"#.into())).unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        tree.create_node(3, NodeKind::Modal).unwrap();
        tree.set_prop(3, "visible", PropValue::Bool(false)).unwrap();
        tree.set_prop(3, "presentation", PropValue::Str("sheet".into())).unwrap();
        tree.insert_child(1, 3, 1).unwrap();

        let mut mount = MountSide::new(WatchHost::new(new_event_queue()));
        let measurer = WatchMeasurer::new(Default::default());
        mount.apply(&tree.commit((208.0, 248.0), &measurer).unwrap());

        let foto = crate::snapshot::snapshot(mount.host());
        let raiz = foto.root.expect("tiene que haber raíz");
        assert!(raiz.children.is_empty(), "ni el diálogo ni la hoja son hijos");
        assert_eq!(foto.overlays.len(), 2);

        let alerta = &foto.overlays[0];
        assert_eq!(alerta.kind, "Alert");
        assert_eq!(alerta.visible, Some(true));
        assert_eq!(alerta.title.as_deref(), Some("batería"));
        // La lista viaja como JSON porque el protocolo no lleva listas, y se
        // deshace en Rust para que Swift no tenga que saberlo.
        assert_eq!(
            alerta.buttons.as_deref(),
            Some(["vale".to_owned(), "ahora no".to_owned()].as_slice())
        );
        assert_eq!(foto.overlays[1].kind, "Modal");
        assert_eq!(foto.overlays[1].presentation.as_deref(), Some("sheet"));
    }

    /// Los controles bajan con su estado, no solo con su marco: el shell tiene
    /// que poder pintar un interruptor encendido en el primer frame.
    #[test]
    fn los_controles_bajan_con_su_estado() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "208").unwrap();
        tree.set_style(1, "height", "248").unwrap();
        tree.set_root(1).unwrap();

        tree.create_node(2, NodeKind::Switch).unwrap();
        tree.set_prop(2, "on", PropValue::Bool(true)).unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        tree.create_node(3, NodeKind::Slider).unwrap();
        tree.set_prop(3, "value", PropValue::Number(40.0)).unwrap();
        tree.set_prop(3, "minimumValue", PropValue::Number(0.0)).unwrap();
        tree.set_prop(3, "maximumValue", PropValue::Number(100.0)).unwrap();
        tree.set_prop(3, "enabled", PropValue::Bool(false)).unwrap();
        tree.insert_child(1, 3, 1).unwrap();

        tree.create_node(4, NodeKind::Picker).unwrap();
        tree.set_prop(4, "items", PropValue::Str(r#"["suave","normal"]"#.into())).unwrap();
        tree.set_prop(4, "selectedIndex", PropValue::Number(1.0)).unwrap();
        tree.insert_child(1, 4, 2).unwrap();

        tree.create_node(5, NodeKind::Icon).unwrap();
        tree.set_prop(5, "name", PropValue::Str("back".into())).unwrap();
        tree.insert_child(1, 5, 3).unwrap();

        let mut mount = MountSide::new(WatchHost::new(new_event_queue()));
        let measurer = WatchMeasurer::new(Default::default());
        mount.apply(&tree.commit((208.0, 248.0), &measurer).unwrap());
        let raiz = crate::snapshot::snapshot(mount.host()).root.unwrap();

        assert_eq!(raiz.children[0].on, Some(true));
        assert_eq!(raiz.children[1].value, Some(40.0));
        assert_eq!(raiz.children[1].maximum, Some(100.0));
        // `enabled` solo viaja cuando lo apagan: mandarlo siempre engordaría
        // cada nodo por lo que casi nunca cambia.
        assert!(raiz.children[1].disabled);
        assert!(!raiz.children[0].disabled);
        assert_eq!(raiz.children[2].selected_index, Some(1));
        assert_eq!(
            raiz.children[2].items.as_deref(),
            Some(["suave".to_owned(), "normal".to_owned()].as_slice())
        );
        // El nombre común se traduce en Rust, con la tabla del núcleo, para que
        // Swift no tenga una segunda copia que mantener de acuerdo.
        assert_eq!(raiz.children[3].symbol.as_deref(), Some("chevron.left"));
    }

    /// La corona es un oyente más y viaja en la misma lista que el resto: eso es
    /// lo que permitió añadirla sin tocar el protocolo ni las directivas.
    #[test]
    fn la_corona_viaja_como_un_oyente_mas() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "208").unwrap();
        tree.set_style(1, "height", "248").unwrap();
        tree.set_root(1).unwrap();
        tree.set_listener(1, "crown", true).unwrap();
        tree.set_listener(1, "longPress", true).unwrap();
        tree.set_listener(1, "swipeLeft", true).unwrap();

        let mut mount = MountSide::new(WatchHost::new(new_event_queue()));
        let measurer = WatchMeasurer::new(Default::default());
        mount.apply(&tree.commit((208.0, 248.0), &measurer).unwrap());
        let raiz = crate::snapshot::snapshot(mount.host()).root.unwrap();

        // Ordenada: la foto entra en un test y en un `diff`, y un `HashSet` la
        // barajaría en cada frame.
        assert_eq!(raiz.listens, vec!["crown", "longPress", "swipeLeft"]);
    }

    /// Cada primitiva o la pinta el reloj o dice por qué no. Lo que no puede
    /// haber es una tercera respuesta: un hueco sin explicación.
    #[test]
    fn cada_primitiva_esta_decidida() {
        use an_core::NodeKind::*;
        let pintadas = [
            View, Text, ScrollView, Button, Image, Icon, TextInput, StackView, Switch, Slider,
            Stepper, ProgressBar, ActivityIndicator, Picker, DatePicker, Alert, Modal,
        ];
        let descartadas = [
            TabBar, NavigationBar, SegmentedControl, SearchBar, TextEditor, WebView, MapView,
            VideoView,
        ];
        for kind in pintadas {
            assert!(
                crate::snapshot::unsupported(kind).is_none(),
                "{kind:?} se pinta, así que no puede tener motivo para no pintarse"
            );
        }
        for kind in descartadas {
            assert!(
                crate::snapshot::unsupported(kind).is_some(),
                "{kind:?} no se pinta y tiene que decir por qué"
            );
        }
        // 25 primitivas y `RawText`, que no es una: lo crea `createText()` y no
        // se escribe en ninguna plantilla.
        assert_eq!(pintadas.len() + descartadas.len(), 25);
    }

}
