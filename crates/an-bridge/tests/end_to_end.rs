//! From JavaScript to resolved frames, with no platform in the way.

use std::cell::RefCell;
use std::rc::Rc;

use an_bridge::{apply, Encoder, JsRuntime, ProtocolError, QuickJsRuntime};
use an_bridge::runtime::LogSink;
use an_core::{MountOp, NaiveMeasurer, NodeKind, ShadowTree};

const VIEWPORT: (f32, f32) = (393.0, 852.0);
const MAIN_JS: &str = include_str!("../../../examples/hello/main.js");

#[derive(Default)]
struct CapturedLog(RefCell<Vec<String>>);

impl LogSink for CapturedLog {
    fn log(&self, level: u8, message: &str) {
        self.0.borrow_mut().push(format!("{level}:{message}"));
    }
}

fn rect(ops: &[MountOp], id: u32) -> Option<an_core::Rect> {
    ops.iter().rev().find_map(|op| match op {
        MountOp::SetLayout { id: got, frame } if *got == id => Some(*frame),
        _ => None,
    })
}

#[test]
fn the_encoder_and_the_decoder_speak_the_same_format() {
    let mut encoder = Encoder::new();
    encoder
        .create_node(1, NodeKind::View)
        .set_style(1, "width", "100%")
        .set_style(1, "height", "40")
        .set_root(1)
        .create_node(2, NodeKind::Text)
        .set_prop_num(2, "fontSize", 17.0)
        .create_node(3, NodeKind::RawText)
        .set_text(3, "an ñ and an emoji 🐿")
        .insert_child(2, 3, 0)
        .insert_child(1, 2, 0);

    let mut tree = ShadowTree::new();
    assert_eq!(apply(&encoder.into_bytes(), &mut tree).unwrap(), 10);

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(rect(&frame.ops, 1).unwrap().width, VIEWPORT.0);
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetText { id: 2, text } if text.contains("🐿")
    )));
}

#[test]
fn a_truncated_buffer_does_not_spin() {
    let mut encoder = Encoder::new();
    encoder.create_node(1, NodeKind::View).set_style(1, "width", "100%");
    let mut bytes = encoder.into_bytes();
    bytes.truncate(bytes.len() - 3);

    let mut tree = ShadowTree::new();
    assert!(matches!(
        apply(&bytes, &mut tree),
        Err(ProtocolError::Truncated { .. })
    ));
}

#[test]
fn javascript_builds_the_whole_screen() {
    let log = Rc::new(CapturedLog::default());
    let mut js = QuickJsRuntime::with_log(log.clone()).unwrap();
    js.eval("main.js", MAIN_JS).unwrap();

    let mut tree = ShadowTree::new();
    let commands = js.tick(0.0).unwrap();
    assert!(!commands.is_empty(), "the first tick has to bring the tree");
    apply(&commands, &mut tree).unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    assert!(log.0.borrow().iter().any(|l| l.ends_with("main.js mounted")));

    // The tree was mounted with the ids JS hands out: 1 root, 2 title, 3 its
    // raw text, 4 row, 5 and 6 the cards.
    let root = rect(&frame.ops, 1).unwrap();
    assert_eq!((root.width, root.height), VIEWPORT);

    let (left, right) = (rect(&frame.ops, 5).unwrap(), rect(&frame.ops, 6).unwrap());
    let usable = VIEWPORT.0 - 32.0 - 12.0;
    assert!((left.width - usable / 3.0).abs() < 0.5, "left: {left:?}");
    assert!((right.width - usable * 2.0 / 3.0).abs() < 0.5, "right: {right:?}");
    assert_eq!(left.y, right.y);
}

#[test]
fn the_frame_is_what_sets_the_timers_clock() {
    let mut js = QuickJsRuntime::with_log(Rc::new(CapturedLog::default())).unwrap();
    js.eval("main.js", MAIN_JS).unwrap();

    let mut tree = ShadowTree::new();
    apply(&js.tick(0.0).unwrap(), &mut tree).unwrap();
    tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    // Half a second: the 1000 ms interval has not come due yet.
    let quiet = js.tick(500.0).unwrap();
    assert!(quiet.is_empty(), "there should be no commands: {quiet:?}");

    // Once the second has passed the counter updates and only its text is
    // resent.
    apply(&js.tick(1500.0).unwrap(), &mut tree).unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(
        frame.ops.iter().any(|op| matches!(
            op,
            MountOp::SetText { text, .. } if text == "seconds running: 1"
        )),
        "ops: {:?}",
        frame.ops
    );

    // Three frames later the counter is at 4, with no drift piled up.
    for now in [2500.0, 3500.0, 4500.0] {
        apply(&js.tick(now).unwrap(), &mut tree).unwrap();
    }
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetText { text, .. } if text == "seconds running: 4"
    )));
}

#[test]
fn microtasks_land_in_the_same_frame() {
    let mut js = QuickJsRuntime::with_log(Rc::new(CapturedLog::default())).unwrap();
    js.eval(
        "promise.js",
        r#"
        const id = __an_dom.createNode('View')
        __an_dom.setRoot(id)
        Promise.resolve().then(() => {
            __an_dom.setStyle(id, 'width', '123')
        })
        "#,
    )
    .unwrap();

    let mut tree = ShadowTree::new();
    apply(&js.tick(0.0).unwrap(), &mut tree).unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert_eq!(
        rect(&frame.ops, 1).unwrap().width,
        123.0,
        "the promise had to resolve before the frame closed"
    );
}

#[test]
fn a_js_exception_arrives_with_its_stack_trace() {
    let mut js = QuickJsRuntime::with_log(Rc::new(CapturedLog::default())).unwrap();
    let error = js.eval("broken.js", "function boom() { null.x }; boom()").unwrap_err();
    let text = error.to_string();
    assert!(text.contains("boom"), "no useful stack trace: {text}");
}
