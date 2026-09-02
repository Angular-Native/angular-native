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
