//! The whole pipeline with no platform under it: mutations, layout and diff.

use an_core::props::PropValue;
use an_core::{MountOp, NaiveMeasurer, NodeKind, ShadowTree};

const VIEWPORT: (f32, f32) = (320.0, 568.0);

fn frame_of(ops: &[MountOp], id: u32) -> Option<an_core::Rect> {
    ops.iter().rev().find_map(|op| match op {
        MountOp::SetLayout { id: got, frame } if *got == id => Some(*frame),
        _ => None,
    })
}

/// A root that fills the viewport, with two fixed-width children in a row.
fn build_row() -> ShadowTree {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::View).unwrap();
    tree.create_node(3, NodeKind::View).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(1, "flexDirection", "row").unwrap();
    tree.set_style(2, "width", "50").unwrap();
    tree.set_style(2, "height", "20").unwrap();
    tree.set_style(3, "flexGrow", "1").unwrap();
    tree.set_style(3, "height", "20").unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.insert_child(1, 3, 1).unwrap();
    tree.set_root(1).unwrap();
    tree
}

#[test]
fn resolves_flexbox_in_a_row() {
    let mut tree = build_row();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    assert_eq!(frame_of(&frame.ops, 1).unwrap().width, 320.0);
    let left = frame_of(&frame.ops, 2).unwrap();
    let right = frame_of(&frame.ops, 3).unwrap();
    assert_eq!((left.x, left.width), (0.0, 50.0));
    assert_eq!((right.x, right.width), (50.0, 270.0));
}

#[test]
fn a_commit_with_no_changes_produces_no_ops() {
    let mut tree = build_row();
    assert!(!tree.commit(VIEWPORT, &NaiveMeasurer).unwrap().is_empty());
    let second = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(second.is_empty(), "commit is idempotent, out came {:?}", second.ops);
}

#[test]
fn only_the_node_that_changed_is_re_emitted() {
    let mut tree = build_row();
    tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    tree.set_style(2, "width", "80").unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    let touched: Vec<u32> = frame
        .ops
        .iter()
        .filter_map(|op| match op {
            MountOp::SetLayout { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    // The root does not move: only the two children change.
    assert_eq!(touched, vec![2, 3]);
    assert_eq!(frame_of(&frame.ops, 3).unwrap().x, 80.0);
}

#[test]
fn the_text_is_what_sets_the_height() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::Text).unwrap();
    tree.create_node(3, NodeKind::RawText).unwrap();
    tree.set_style(1, "width", "100").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_prop(2, "fontSize", PropValue::Number(20.0)).unwrap();
    tree.set_text(3, "a fairly long sentence that has to be broken over several lines")
        .unwrap();
    tree.insert_child(2, 3, 0).unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    let text = frame_of(&frame.ops, 2).unwrap();
    assert!(text.height > 20.0 * 1.25, "it has to take more than one line: {text:?}");
    assert!(text.width <= 100.0);

    // The host gets the text already concatenated, not the raw nodes.
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetText { id: 2, text } if text.starts_with("a fairly long")
    )));
    // A `RawText` never gets mounted.
    assert!(!frame.ops.iter().any(|op| matches!(op, MountOp::Create { id: 3, .. })));
}

#[test]
fn the_host_index_ignores_non_mountable_nodes() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::RawText).unwrap();
    tree.create_node(3, NodeKind::View).unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.insert_child(1, 3, 1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::Insert { parent: 1, child: 3, index: 0 }
    )));
}

#[test]
fn destroying_takes_the_subtree_down_from_children_to_parents() {
    let mut tree = build_row();
    tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    tree.create_node(4, NodeKind::View).unwrap();
    tree.insert_child(3, 4, 0).unwrap();
    tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    tree.destroy_node(3).unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    let destroyed: Vec<u32> = frame
        .ops
        .iter()
        .filter_map(|op| match op {
            MountOp::Destroy { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(destroyed, vec![4, 3]);
    assert!(frame.ops.iter().any(|op| matches!(op, MountOp::Remove { parent: 1, child: 3 })));
}

/// A style that is not a layout one travels as a host prop, and **in camel
/// case**.
///
/// Angular turns style names into hyphenated ones before handing them over, so
/// `[style.fontSize]` reaches here as `font-size`. Forwarding that as-is to the
/// host, which looks for `fontSize`, was asking it for something it was never
/// going to recognise: nothing failed, the text was simply measured in one font
/// and drawn in another.
#[test]
fn an_unrecognised_style_travels_as_a_host_prop_in_camel_case() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.set_style(1, "background-color", "#ff0000").unwrap();
    tree.set_style(1, "font-size", "18").unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetProp { id: 1, key, value: PropValue::Str(v) }
            if key == "backgroundColor" && v == "#ff0000"
    )));
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetProp { id: 1, key, value: PropValue::Str(v) }
            if key == "fontSize" && v == "18"
    )));
}


/// A scroll view is not sized by its content, and that used to be said with a
/// `flex-basis: 0` that also beat any `[style.height]` the app wrote: the view
/// came out zero points tall and nothing anywhere said why.
#[test]
fn a_height_on_a_scroll_view_is_the_height_it_gets() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::ScrollView).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(2, "height", "200").unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(frame_of(&frame.ops, 2).unwrap().height, 200.0);
}

/// The same in a row, where the main axis is the other one: there it is `width`
/// that the basis must yield to, and `height` is an ordinary cross-axis size.
#[test]
fn a_width_on_a_scroll_view_inside_a_row_is_the_width_it_gets() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::ScrollView).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(1, "flexDirection", "row").unwrap();
    tree.set_style(2, "width", "120").unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(frame_of(&frame.ops, 2).unwrap().width, 120.0);
}

/// With no size of its own it still does not grow with its content: five
/// thousand rows must not make the scroll view five thousand rows tall.
#[test]
fn without_a_size_a_scroll_view_still_does_not_grow_with_its_content() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::ScrollView).unwrap();
    tree.create_node(3, NodeKind::View).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(2, "flexGrow", "1").unwrap();
    tree.set_style(3, "height", "4000").unwrap();
    tree.insert_child(2, 3, 0).unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(frame_of(&frame.ops, 2).unwrap().height, VIEWPORT.1);
}

fn content_of(ops: &[MountOp], id: u32) -> Option<(f32, f32)> {
    ops.iter().rev().find_map(|op| match op {
        MountOp::SetContentSize { id: got, width, height } if *got == id => Some((*width, *height)),
        _ => None,
    })
}

/// `[horizontal]` turns the children sideways and lets the content come out
/// wider than the frame. The cross axis is clamped instead, which is the same
/// rule as before with the two axes swapped.
#[test]
fn a_horizontal_scroll_view_overflows_sideways_and_not_downwards() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::ScrollView).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(2, "height", "120").unwrap();
    tree.set_prop(2, "horizontal", PropValue::Bool(true)).unwrap();
    for id in 3..=6 {
        tree.create_node(id, NodeKind::View).unwrap();
        tree.set_style(id, "width", "200").unwrap();
        tree.set_style(id, "height", "400").unwrap();
        tree.insert_child(2, id, (id - 3) as usize).unwrap();
    }
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    // Four cards of 200 in a row: the children run along x and the last one
    // starts at 600.
    assert_eq!(frame_of(&frame.ops, 6).unwrap().x, 600.0);
    assert_eq!(frame_of(&frame.ops, 2).unwrap().height, 120.0);
    // 800 points of content in a 320-point frame, and the 400-point cards
    // clamped to the frame's own height rather than offering a second axis.
    assert_eq!(content_of(&frame.ops, 2), Some((800.0, 120.0)));
}

/// The direction is a default, not an override: a template that wrote
/// `flexDirection` keeps it.
#[test]
fn a_template_that_set_the_direction_keeps_it() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::ScrollView).unwrap();
    tree.create_node(3, NodeKind::View).unwrap();
    tree.create_node(4, NodeKind::View).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(2, "flexDirection", "column").unwrap();
    tree.set_prop(2, "horizontal", PropValue::Bool(true)).unwrap();
    for id in [3, 4] {
        tree.set_style(id, "width", "50").unwrap();
        tree.set_style(id, "height", "60").unwrap();
        tree.insert_child(2, id, (id - 3) as usize).unwrap();
    }
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(frame_of(&frame.ops, 4).unwrap().y, 60.0);
}

/// The basis the core resolves is a default: a template that wrote `flexBasis`
/// out — or the `flex` shorthand, which writes it — keeps what it wrote.
#[test]
fn a_template_that_set_the_basis_keeps_it() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::ScrollView).unwrap();
    tree.create_node(3, NodeKind::ScrollView).unwrap();
    tree.set_style(1, "width", "100%").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_style(2, "flexBasis", "90").unwrap();
    tree.set_style(2, "height", "200").unwrap();
    tree.set_style(3, "height", "200").unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.insert_child(1, 3, 1).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(frame_of(&frame.ops, 2).unwrap().height, 90.0);
    assert_eq!(frame_of(&frame.ops, 3).unwrap().height, 200.0);
}

/// The per-side border widths are layout and only layout.
///
/// The line a host draws is the `[borderWidth]` prop, which is one number for
/// all four sides and moves no child. These four are styles: they push the
/// children in and no `SetProp` ever leaves the core with their name on it. The
/// core says so once per name on stderr; what is pinned here is the behaviour
/// the sentence describes, because a future host that started reading
/// `borderTopWidth` would make that sentence a lie.
#[test]
fn a_per_side_border_width_insets_the_children_and_reaches_no_host() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::View).unwrap();
    tree.set_style(1, "width", "200").unwrap();
    tree.set_style(1, "height", "60").unwrap();
    tree.set_style(1, "borderTopWidth", "2").unwrap();
    tree.set_style(1, "borderRightWidth", "4").unwrap();
    tree.set_style(1, "borderBottomWidth", "8").unwrap();
    tree.set_style(1, "borderLeftWidth", "16").unwrap();
    tree.set_style(2, "flexGrow", "1").unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    let child = frame_of(&frame.ops, 2).unwrap();
    assert_eq!((child.x, child.y), (16.0, 2.0), "the four widths inset the children");
    assert_eq!((child.width, child.height), (180.0, 50.0));

    let leaked: Vec<&str> = frame
        .ops
        .iter()
        .filter_map(|op| match op {
            MountOp::SetProp { key, .. } if key.starts_with("border") => Some(&**key),
            _ => None,
        })
        .collect();
    assert!(leaked.is_empty(), "a border style reached a host as a prop: {leaked:?}");
}

/// And the prop is the other half: it travels and it moves nothing.
#[test]
fn the_border_width_prop_is_drawn_and_insets_nothing() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::View).unwrap();
    tree.set_style(1, "width", "200").unwrap();
    tree.set_style(1, "height", "60").unwrap();
    tree.set_prop(1, "borderWidth", PropValue::Number(4.0)).unwrap();
    tree.set_style(2, "flexGrow", "1").unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    let child = frame_of(&frame.ops, 2).unwrap();
    assert_eq!((child.x, child.y, child.width, child.height), (0.0, 0.0, 200.0, 60.0));
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetProp { id: 1, key, value: PropValue::Number(w) }
            if key == "borderWidth" && *w == 4.0
    )));
}
