//! The tables that have to be edited by hand when a new primitive shows up.
//!
//! Not one of them fails if it falls behind: a kind missing from a table raises
//! no error, it simply stops receiving whatever that table hands out. A control
//! nobody knows how to measure comes out at zero points, and a view of zero
//! points is a view that is not there.

use an_core::props::{affects_measure, font_from_props, NodeKind};
use an_core::{NaiveMeasurer, PropValue, TextMeasurer};

/// All 26 kinds in the enum. If the compiler complains that one is missing, a
/// primitive came in: add it here and see which other tests turn red.
const ALL: [NodeKind; 26] = [
    NodeKind::View,
    NodeKind::Text,
    NodeKind::RawText,
    NodeKind::Image,
    NodeKind::ScrollView,
    NodeKind::TextInput,
    NodeKind::StackView,
    NodeKind::TabBar,
    NodeKind::Switch,
    NodeKind::Slider,
    NodeKind::ActivityIndicator,
    NodeKind::ProgressBar,
    NodeKind::Button,
    NodeKind::Modal,
    NodeKind::Alert,
    NodeKind::Icon,
    NodeKind::SegmentedControl,
    NodeKind::Stepper,
    NodeKind::SearchBar,
    NodeKind::Picker,
    NodeKind::DatePicker,
    NodeKind::NavigationBar,
    NodeKind::TextEditor,
    NodeKind::WebView,
    NodeKind::MapView,
    NodeKind::VideoView,
];

/// Would catch —and did catch— a control whose name is missing from the table
/// of default measurements.
///
/// `NodeKind::control_name()` is what travels down to the measurer, and the
/// measurer answers `(0, 0)` to anything it does not recognise. A control that
/// is zero by zero raises no error: it mounts, it gets its props, and it is not
/// there to see. That is exactly what happened to the dropdown, which was in
/// the table as "Select" while the core had always called it "Picker"; it only
/// showed if the template did not give it a height by hand, and the one in
/// `examples/pickers` does.
#[test]
fn every_control_has_a_natural_size_that_is_not_zero() {
    for kind in ALL.into_iter().filter(|k| k.is_control()) {
        let name = kind.control_name();
        assert!(!name.is_empty(), "{kind:?} is a control and will not say what it is called");
        let (width, height) = NaiveMeasurer.measure_control(name, None);
        assert!(
            width > 0.0 && height > 0.0,
            "{kind:?} travels as {name:?} and the default measurer \
             gives it {width}x{height}: it would mount invisible"
        );
    }
}

/// Would catch: giving a control's name to something that is not one.
/// `control_name()` returns `""` for the rest, and an empty string as a
/// measuring key would be one more measurement of zero that nobody can explain.
#[test]
fn only_controls_have_a_control_name() {
    for kind in ALL.into_iter().filter(|k| !k.is_control()) {
        assert_eq!(kind.control_name(), "", "{kind:?} is not a control");
    }
}

/// Would catch: a new primitive that cannot be named from a tag.
///
/// `from_tag` is the door any name that does not come from the binary protocol
/// walks in through —the iOS demo host, for instance—. A kind that is not there
/// cannot be asked for, and the `None` turns into a silent wrapper.
#[test]
fn every_mountable_primitive_can_be_named_by_a_tag() {
    for kind in ALL.into_iter().filter(|k| k.is_mountable()) {
        let pascal = format!("{kind:?}");
        assert_eq!(
            NodeKind::from_tag(&pascal),
            Some(kind),
            "{pascal} is not recognised as a tag"
        );
    }
    // `RawText` is internal: `Renderer2.createText()` creates it and no template
    // ever writes it, so it deliberately has no tag.
    assert_eq!(NodeKind::from_tag("RawText"), None);
    assert_eq!(NodeKind::from_tag("Div"), None);
}

/// Would catch: a leaf that gets measured but never gets marked for measuring,
/// or the other way round. Both lists have to be talking about the same nodes:
/// a measurable node that is not marked comes out at zero, and a marked one
/// that is not measured burns a measuring context for nothing.
#[test]
fn the_measurable_leaves_are_the_ones_marked_at_birth() {
    for kind in ALL {
        if kind.is_control() {
            assert!(kind.is_measured_leaf(), "{kind:?} is a control and has to be measured");
        }
    }
    // The three that are not controls and get measured anyway.
    for kind in [NodeKind::Text, NodeKind::Image, NodeKind::TextInput] {
        assert!(kind.is_measured_leaf(), "{kind:?}");
    }
    // And a container is not measured: its children are what size it.
    for kind in [NodeKind::View, NodeKind::ScrollView, NodeKind::StackView] {
        assert!(!kind.is_measured_leaf(), "{kind:?} is not a leaf");
    }
}

/// Would catch: a new typography prop that layout reads in order to measure and
/// that nobody marks dirty when it changes.
///
/// `font_from_props` decides which font is measured with; `affects_measure`
/// decides when to measure again. If the second one does not know about a prop
/// the first one reads, changing it on the fly does not re-measure: the text is
/// drawn in the new font inside the old font's box. Same family as `lineHeight`
/// and `letterSpacing`, which the core measured and the host did not draw.
#[test]
fn every_prop_that_changes_the_font_forces_a_re_measure() {
    let base = font_from_props(|_| None);
    let candidates: [(&str, PropValue); 7] = [
        ("fontSize", PropValue::Number(33.0)),
        ("fontWeight", PropValue::Str("bold".into())),
        ("fontStyle", PropValue::Str("italic".into())),
        ("fontFamily", PropValue::Str("Menlo".into())),
        ("lineHeight", PropValue::Number(40.0)),
        ("letterSpacing", PropValue::Number(3.0)),
        ("numberOfLines", PropValue::Number(2.0)),
    ];
    for (key, value) in candidates {
        let font = font_from_props(|k| (k == key).then(|| value.clone()));
        assert_ne!(font, base, "{key} was supposed to change the measuring font");
        assert!(
            affects_measure(key),
            "{key} changes the font things are measured with and does not mark the node for re-measuring"
        );
    }
}

/// Would catch: `fontWeight` losing one of the three shapes it arrives in.
/// Angular hands over whatever the template put there —`700`, `'700'`,
/// `'bold'`— and all three have to weigh the same, or the same text gets
/// measured differently depending on how the binding was written.
#[test]
fn the_font_weight_is_understood_in_all_three_spellings() {
    let weight = |value: PropValue| font_from_props(|k| (k == "fontWeight").then(|| value.clone())).weight;
    assert_eq!(weight(PropValue::Number(700.0)), 700);
    assert_eq!(weight(PropValue::Str("700".into())), 700);
    assert_eq!(weight(PropValue::Str("bold".into())), 700);
    assert_eq!(weight(PropValue::Str("normal".into())), 400);
    // And something that means nothing does not leave the weight at zero, which
    // would be a font with no thickness: it falls back to normal.
    assert_eq!(weight(PropValue::Str("extremely-bold".into())), 400);
}

/// Would catch: `numberOfLines` slipping through a `max_lines` of zero.
///
/// Zero lines is not "no limit", it is "none at all": the text would measure
/// zero tall and disappear. An unset `[numberOfLines]` arrives as `0` from more
/// than one template.
#[test]
fn zero_lines_is_not_a_limit_of_zero_lines() {
    let no_limit = font_from_props(|k| (k == "numberOfLines").then_some(PropValue::Number(0.0)));
    assert_eq!(no_limit.max_lines, None);

    let one = font_from_props(|k| (k == "numberOfLines").then_some(PropValue::Number(1.0)));
    assert_eq!(one.max_lines, Some(1));
}

/// Would catch: a prop the core consumes that stopped forcing a re-measure.
/// A button's title or a tab bar's items change how much room the control takes
/// up, not just what it says inside.
#[test]
fn a_controls_content_forces_a_re_measure_too() {
    for key in ["value", "placeholder", "title", "items", "buttons", "intrinsicWidth", "intrinsicHeight"] {
        assert!(affects_measure(key), "{key}");
    }
    // And what only paints does not cost a measurement: repainting is cheap,
    // measuring the whole tree is not.
    for key in ["color", "backgroundColor", "borderRadius", "opacity", "translateX"] {
        assert!(!affects_measure(key), "{key} does not change the size of anything");
    }
}
