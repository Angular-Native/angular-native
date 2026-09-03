//! The demo tree, verified without a simulator using the approximate
//! measurer. It checks the layout's shape, not UIKit's pixels.

use an_core::{MountOp, NaiveMeasurer, ShadowTree};

#[test]
fn the_demo_tree_fits_on_the_screen() {
    let viewport = (393.0_f32, 852.0_f32); // an iPhone 17 Pro, in points
    let mut tree = ShadowTree::new();
    an_ios::build_demo(&mut tree).unwrap();
    let frame = tree.commit(viewport, &NaiveMeasurer).unwrap();

    let rect = |id: u32| {
        frame
            .ops
            .iter()
            .rev()
            .find_map(|op| match op {
                MountOp::SetLayout { id: got, frame } if *got == id => Some(*frame),
                _ => None,
            })
            .unwrap_or_else(|| panic!("node {id} got no layout"))
    };

    // A full-screen root, with 16 of padding down the sides.
    assert_eq!(rect(1).width, viewport.0);
    let content_width = viewport.0 - 32.0;

    // The cards split the width 1:2 with a gap of 12 between them.
    let (left, right) = (rect(5), rect(6));
    let usable = content_width - 12.0;
    assert!((left.width - usable / 3.0).abs() < 0.5, "left: {left:?}");
    assert!((right.width - usable * 2.0 / 3.0).abs() < 0.5, "right: {right:?}");
    assert_eq!(left.y, right.y, "the cards sit at the same height");
    assert!((right.x - (left.x + left.width + 12.0)).abs() < 0.5);

    // The paragraph wraps onto several lines and sits below the cards.
    let paragraph = rect(7);
    assert!(paragraph.y > right.y + right.height, "paragraph: {paragraph:?}");
    assert!(paragraph.height > 40.0, "it has to take several lines: {paragraph:?}");

    // No node spills out past the right-hand edge.
    for op in &frame.ops {
        if let MountOp::SetLayout { id, frame } = op {
            assert!(frame.width <= viewport.0, "node {id} is wider than the screen");
        }
    }
}
