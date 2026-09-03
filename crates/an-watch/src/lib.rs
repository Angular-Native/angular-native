//! The watchOS host.
//!
//! This is not the iOS host under another `cfg`: it is a different thing. On
//! watchOS the `UIView` hierarchy that `an-ios` and `an-android` lean on does
//! not exist. SwiftUI draws the interface and there is no way around it.
//!
//! That runs head-on into the project's mounting model, which is imperative:
//! create a view, put it here, change its frame. SwiftUI takes no orders, only
//! state: you describe what there is and it decides what to redraw. The way
//! out is to mirror the Rust tree in a model SwiftUI observes, and to turn
//! every `MountOp` into a mutation of that model.
//!
//! Who does what:
//!
//! ```text
//!   Rust                                   Swift
//!   ────────────────────────────────       ─────────────────────────────
//!   QuickJS ─▶ ShadowTree ─▶ taffy         TimelineView (one tick per frame)
//!                  │ commit                       │ an_watch_runtime_frame
//!                  ▼                              ▼
//!            Frame { MountOp[] }            did the revision change?
//!                  │                              │ yes
//!                  ▼                              ▼
//!             WatchHost (modelo)  ──JSON──▶  AnTree (@Observable)
//!                                                  │
//!                                                  ▼
//!                                            ZStack + .offset
//! ```
//!
//! The layout is still taffy's. Swift places each node by its absolute frame
//! and does not use `VStack`/`HStack` to position anything: if it did there
//! would be two layout engines deciding the same thing, and whichever ran last
//! would win.

pub mod host;
pub mod measure;
// The device module touches no platform: the four fields it answers come from
// the shell. That is what lets `cargo test` exercise it on the Mac, with no
// watch and no nightly.
pub mod modules;
pub mod snapshot;

#[cfg(target_os = "watchos")]
mod ffi;

pub use host::{WatchHost, WatchNode};
pub use modules::DeviceModule;
pub use measure::{ControlSizes, WatchMeasurer};
pub use snapshot::{snapshot, Snapshot};

#[cfg(test)]
mod tests {
    use an_core::{NodeKind, PropValue, ShadowTree};
    use an_host::{new_event_queue, MountSide};

    use crate::host::WatchHost;
    use crate::measure::WatchMeasurer;

    /// Mounts a small tree and checks that the snapshot that comes out is the
    /// one the shell expects: a root with a background, a text with its string
    /// already fused, and a button that says it can be pressed.
    ///
    /// This is the test that runs on the Mac. The simulator checks what this
    /// one cannot —that SwiftUI paints it— but seeing that the model comes out
    /// right does not take a watch.
    #[test]
    fn the_snapshot_carries_what_the_shell_needs() {
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
        tree.set_text(3, "hello ").unwrap();
        tree.create_node(4, NodeKind::RawText).unwrap();
        tree.set_text(4, "watch").unwrap();
        tree.insert_child(2, 3, 0).unwrap();
        tree.insert_child(2, 4, 1).unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        tree.create_node(5, NodeKind::Button).unwrap();
        tree.set_prop(5, "title", PropValue::Str("press".into())).unwrap();
        tree.set_listener(5, "press", true).unwrap();
        tree.insert_child(1, 5, 1).unwrap();

        let events = new_event_queue();
        let mut mount = MountSide::new(WatchHost::new(events));
        let frame = tree.commit((176.0, 223.0), &WatchMeasurer::new(Default::default())).unwrap();
        mount.apply(&frame);

        let snapshot = crate::snapshot::snapshot(mount.host());
        let root = snapshot.root.expect("there has to be a root");
        assert_eq!(root.kind, "View");
        assert_eq!(root.background, Some([11.0 / 255.0, 16.0 / 255.0, 32.0 / 255.0, 1.0]));
        assert_eq!(root.children.len(), 2, "RawText nodes do not go down to the shell");

        let text = &root.children[0];
        assert_eq!(text.kind, "Text");
        // The two `RawText` nodes are fused: SwiftUI wants the whole string.
        assert_eq!(text.text.as_deref(), Some("hello watch"));
        assert_eq!(text.font_size, Some(16.0));
        assert!(text.children.is_empty());

        let button = &root.children[1];
        assert_eq!(button.kind, "Button");
        assert_eq!(button.text.as_deref(), Some("press"));
        assert!(button.listens.iter().any(|e| e == "press"), "the template put a (press) on it");
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
    fn the_snapshot_carries_accessibility_translated_to_swiftui() {
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
        let root = snapshot.root.expect("there has to be a root");

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

    /// A node that goes away has to go from the snapshot too, and the revision
    /// has to rise: if it did not, SwiftUI would keep showing what is no longer
    /// there.
    #[test]
    fn removing_a_node_raises_the_revision_and_takes_it_out_of_the_snapshot() {
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
        let before = mount.host().revision();
        assert_eq!(crate::snapshot::snapshot(mount.host()).root.unwrap().children.len(), 1);

        tree.remove_child(1, 2).unwrap();
        tree.destroy_node(2).unwrap();
        mount.apply(&tree.commit((176.0, 223.0), &measurer).unwrap());

        assert!(mount.host().revision() > before, "a change has to raise the revision");
        assert!(crate::snapshot::snapshot(mount.host()).root.unwrap().children.is_empty());
    }

    /// The dialog and the sheet do not travel inside the tree: SwiftUI does not
    /// place them, it presents them, and the shell hangs them off the root. Were
    /// they to come down as children again, the shell would have to go looking
    /// for them inside it on every frame.
    #[test]
    fn the_dialog_and_the_sheet_come_out_of_the_tree() {
        let mut tree = ShadowTree::new();
        tree.create_node(1, NodeKind::View).unwrap();
        tree.set_style(1, "width", "208").unwrap();
        tree.set_style(1, "height", "248").unwrap();
        tree.set_root(1).unwrap();

        tree.create_node(2, NodeKind::Alert).unwrap();
        tree.set_prop(2, "visible", PropValue::Bool(true)).unwrap();
        tree.set_prop(2, "title", PropValue::Str("battery".into())).unwrap();
        tree.set_prop(2, "buttons", PropValue::Str(r#"["ok","not now"]"#.into())).unwrap();
        tree.insert_child(1, 2, 0).unwrap();

        tree.create_node(3, NodeKind::Modal).unwrap();
        tree.set_prop(3, "visible", PropValue::Bool(false)).unwrap();
        tree.set_prop(3, "presentation", PropValue::Str("sheet".into())).unwrap();
        tree.insert_child(1, 3, 1).unwrap();

        let mut mount = MountSide::new(WatchHost::new(new_event_queue()));
        let measurer = WatchMeasurer::new(Default::default());
        mount.apply(&tree.commit((208.0, 248.0), &measurer).unwrap());

        let snapshot = crate::snapshot::snapshot(mount.host());
        let root = snapshot.root.expect("there has to be a root");
        assert!(root.children.is_empty(), "neither the dialog nor the sheet is a child");
        assert_eq!(snapshot.overlays.len(), 2);

        let alert = &snapshot.overlays[0];
        assert_eq!(alert.kind, "Alert");
        assert_eq!(alert.visible, Some(true));
        assert_eq!(alert.title.as_deref(), Some("battery"));
        // The list travels as JSON because the protocol carries no lists, and it
        // is taken apart in Rust so that Swift never has to know that.
        assert_eq!(
            alert.buttons.as_deref(),
            Some(["ok".to_owned(), "not now".to_owned()].as_slice())
        );
        assert_eq!(snapshot.overlays[1].kind, "Modal");
        assert_eq!(snapshot.overlays[1].presentation.as_deref(), Some("sheet"));
    }

    /// Controls come down with their state and not only with their frame: the
    /// shell has to be able to paint a switch that is on in the very first
    /// frame.
    #[test]
    fn controls_come_down_with_their_state() {
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
        tree.set_prop(4, "items", PropValue::Str(r#"["gentle","normal"]"#.into())).unwrap();
        tree.set_prop(4, "selectedIndex", PropValue::Number(1.0)).unwrap();
        tree.insert_child(1, 4, 2).unwrap();

        tree.create_node(5, NodeKind::Icon).unwrap();
        tree.set_prop(5, "name", PropValue::Str("back".into())).unwrap();
        tree.insert_child(1, 5, 3).unwrap();

        let mut mount = MountSide::new(WatchHost::new(new_event_queue()));
        let measurer = WatchMeasurer::new(Default::default());
        mount.apply(&tree.commit((208.0, 248.0), &measurer).unwrap());
        let root = crate::snapshot::snapshot(mount.host()).root.unwrap();

        assert_eq!(root.children[0].on, Some(true));
        assert_eq!(root.children[1].value, Some(40.0));
        assert_eq!(root.children[1].maximum, Some(100.0));
        // `enabled` only travels when it is switched off: sending it always
        // would fatten every node for the sake of what almost never changes.
        assert!(root.children[1].disabled);
        assert!(!root.children[0].disabled);
        assert_eq!(root.children[2].selected_index, Some(1));
        assert_eq!(
            root.children[2].items.as_deref(),
            Some(["gentle".to_owned(), "normal".to_owned()].as_slice())
        );
        // The common name is translated in Rust, with the core's table, so that
        // Swift does not hold a second copy that has to be kept in agreement.
        assert_eq!(root.children[3].symbol.as_deref(), Some("chevron.left"));
    }

    /// The crown is one more listener and travels in the same list as the rest:
    /// that is what made it possible to add it without touching the protocol or
    /// the directives.
    #[test]
    fn the_crown_travels_as_one_more_listener() {
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
        let root = crate::snapshot::snapshot(mount.host()).root.unwrap();

        // Sorted: the snapshot goes into a test and into a `diff`, and a
        // `HashSet` would shuffle it on every frame.
        assert_eq!(root.listens, vec!["crown", "longPress", "swipeLeft"]);
    }

    /// Every primitive is either painted by the watch or says why it is not.
    /// What there cannot be is a third answer: a gap nobody accounted for.
    #[test]
    fn every_primitive_is_decided() {
        use an_core::NodeKind::*;
        let painted = [
            View, Text, ScrollView, Button, Image, Icon, TextInput, StackView, Switch, Slider,
            Stepper, ProgressBar, ActivityIndicator, Picker, DatePicker, Alert, Modal,
        ];
        let ruled_out = [
            TabBar, NavigationBar, SegmentedControl, SearchBar, TextEditor, WebView, MapView,
            VideoView,
        ];
        for kind in painted {
            assert!(
                crate::snapshot::unsupported(kind).is_none(),
                "{kind:?} is painted, so it cannot have a reason for not being painted"
            );
        }
        for kind in ruled_out {
            assert!(
                crate::snapshot::unsupported(kind).is_some(),
                "{kind:?} is not painted and has to say why"
            );
        }
        // 25 primitives plus `RawText`, which is not one: `createText()` makes
        // it and it is written in no template.
        assert_eq!(painted.len() + ruled_out.len(), 25);
    }

}
