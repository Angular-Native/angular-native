//! The binary protocol, looked at as what it is: a byte decoder fed by what
//! another language writes.
//!
//! Three different things are checked here, and they are worth keeping apart:
//!
//! 1. **The format is pinned down.** The exact bytes of every command and the
//!    code of every primitive. If somebody changes them in Rust, the JS prelude
//!    goes on writing the old ones and what gets mounted is something else —or
//!    nothing— with no error raised anywhere.
//! 2. **No buffer panics.** A panic inside `apply` takes the engine thread with
//!    it and the screen freezes without saying why. The right answer to garbage
//!    is an `Err` saying which command and at what offset.
//! 3. **The error names the command.** "Something" failing is worth nothing when
//!    a frame carries twelve hundred mutations.

use an_bridge::protocol::{kind_from_byte, kind_to_byte, op};
use an_bridge::{apply, Encoder, ProtocolError};
use an_core::tree::Error;
use an_core::{MountOp, NaiveMeasurer, NodeKind, PropValue, ShadowTree};

const VIEWPORT: (f32, f32) = (393.0, 852.0);

/// All 26 node kinds, in the enum's order. It is the table to edit when a
/// primitive is added.
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

fn ops_from(bytes: &[u8]) -> Vec<MountOp> {
    let mut tree = ShadowTree::new();
    apply(bytes, &mut tree).expect("the buffer was supposed to be valid");
    tree.commit(VIEWPORT, &NaiveMeasurer).expect("commit").ops
}

// ------------------------------------------------------------------ the format

/// Would catch: a node kind added to `kind_to_byte` and forgotten in
/// `kind_from_byte`.
///
/// The compiler forces `kind_to_byte` to be complete —it is an exhaustive
/// `match` over the enum— but says nothing about the way back, which is a
/// `match` over a `u8`. A new primitive would travel with its code and the
/// decoder would turn the whole thing down: the app mounts nothing and the error
/// talks about a byte.
#[test]
fn every_node_kind_survives_the_round_trip() {
    for kind in ALL {
        let byte = kind_to_byte(kind);
        assert_eq!(
            kind_from_byte(byte),
            Some(kind),
            "{kind:?} travels as {byte} and comes back as something else"
        );
    }
}

/// Would catch: renumbering the node kinds in Rust.
///
/// The code is not an internal detail: it is the contract with `KIND` in
/// `packages/runtime/runtime.js`. `check-kinds.sh` checks that the two tables
/// agree, so renumbering both at once would pass its review; this list pins the
/// number down as well, which is what keeps an already-compiled bundle meaning
/// the same thing.
#[test]
fn the_code_of_every_kind_is_nailed_down() {
    let expected: [(NodeKind, u8); 27] = [
        (NodeKind::View, 0),
        (NodeKind::Text, 1),
        (NodeKind::RawText, 2),
        (NodeKind::Image, 3),
        (NodeKind::ScrollView, 4),
        (NodeKind::TextInput, 5),
        (NodeKind::StackView, 6),
        (NodeKind::TabBar, 7),
        (NodeKind::Switch, 8),
        (NodeKind::Slider, 9),
        (NodeKind::ActivityIndicator, 10),
        (NodeKind::ProgressBar, 11),
        (NodeKind::Button, 12),
        (NodeKind::Modal, 13),
        (NodeKind::Alert, 14),
        (NodeKind::Icon, 15),
        (NodeKind::SegmentedControl, 16),
        (NodeKind::Stepper, 17),
        (NodeKind::SearchBar, 18),
        // `<an-select>` in the template, `Picker` in the core. The number is
        // the only thing the two vocabularies share.
        (NodeKind::Picker, 19),
        (NodeKind::DatePicker, 20),
        (NodeKind::NavigationBar, 21),
        // And `<an-textarea>` is `TextEditor`, for the same reason.
        (NodeKind::TextEditor, 22),
        (NodeKind::WebView, 23),
        (NodeKind::MapView, 24),
        (NodeKind::VideoView, 25),
        // The hole a plugin's view comes through. The kind says "ask the host's
        // plugin-view registry"; which view is a prop, not a code, which is what
        // keeps this list finite.
        (NodeKind::Custom, 26),
    ];
    for (kind, byte) in expected {
        assert_eq!(kind_to_byte(kind), byte, "{kind:?} changed code");
    }
    assert_eq!(kind_from_byte(27), None, "27 does not belong to anybody yet");
}

/// Would catch: changing the order of the fields, the size of an integer or the
/// endianness of any command.
///
/// The prelude writes these bytes by hand —`writer.u32`, `writer.str`— and
/// shares no code with this side. The only ones that can report the two drifting
/// apart are the bytes.
#[test]
fn every_command_takes_exactly_the_agreed_bytes() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::Text);
    assert_eq!(e.into_bytes(), vec![op::CREATE_NODE, 1, 0, 0, 0, 1]);

    let mut e = Encoder::new();
    e.destroy_node(0x0102_0304);
    assert_eq!(e.into_bytes(), vec![op::DESTROY_NODE, 0x04, 0x03, 0x02, 0x01]);

    let mut e = Encoder::new();
    e.insert_child(1, 2, 3);
    assert_eq!(
        e.into_bytes(),
        vec![op::INSERT_CHILD, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]
    );

    let mut e = Encoder::new();
    e.remove_child(1, 2);
    assert_eq!(e.into_bytes(), vec![op::REMOVE_CHILD, 1, 0, 0, 0, 2, 0, 0, 0]);

    // Strings carry a length in bytes, not in characters: the ñ takes two and
    // the decoder reads those two.
    let mut e = Encoder::new();
    e.set_text(1, "ñ");
    assert_eq!(e.into_bytes(), vec![op::SET_TEXT, 1, 0, 0, 0, 2, 0, 0, 0, 0xc3, 0xb1]);

    let mut e = Encoder::new();
    e.set_prop_num(1, "a", 1.0);
    assert_eq!(
        e.into_bytes(),
        vec![op::SET_PROP_NUM, 1, 0, 0, 0, 1, 0, 0, 0, b'a', 0, 0, 0, 0, 0, 0, 0xf0, 0x3f]
    );

    let mut e = Encoder::new();
    e.set_prop_bool(1, "a", true);
    assert_eq!(e.into_bytes(), vec![op::SET_PROP_BOOL, 1, 0, 0, 0, 1, 0, 0, 0, b'a', 1]);

    let mut e = Encoder::new();
    e.set_prop_null(1, "a");
    assert_eq!(e.into_bytes(), vec![op::SET_PROP_NULL, 1, 0, 0, 0, 1, 0, 0, 0, b'a']);

    let mut e = Encoder::new();
    e.set_listener(1, "press", false);
    assert_eq!(
        e.into_bytes(),
        vec![op::SET_LISTENER, 1, 0, 0, 0, 5, 0, 0, 0, b'p', b'r', b'e', b's', b's', 0]
    );

    let mut e = Encoder::new();
    e.set_root(7);
    assert_eq!(e.into_bytes(), vec![op::SET_ROOT, 7, 0, 0, 0]);
}

/// Would catch: an opcode reassigned. There are twelve numbers and the prelude
/// repeats them.
#[test]
fn the_twelve_opcodes_are_nailed_down() {
    assert_eq!(
        [
            op::CREATE_NODE,
            op::DESTROY_NODE,
            op::INSERT_CHILD,
            op::REMOVE_CHILD,
            op::SET_STYLE,
            op::SET_PROP_STR,
            op::SET_PROP_NUM,
            op::SET_PROP_BOOL,
            op::SET_PROP_NULL,
            op::SET_TEXT,
            op::SET_LISTENER,
            op::SET_ROOT,
        ],
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C]
    );
}

// ------------------------------------------------------ the twelve, one by one

/// Would catch: an opcode that decodes fine but whose effect on the tree gets
/// lost on the way —reading the fields in another order, for instance—.
#[test]
fn all_four_prop_types_arrive_with_their_type() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::TextInput)
        .set_root(1)
        .set_prop_str(1, "placeholder", "email")
        .set_prop_num(1, "fontSize", 21.5)
        .set_prop_bool(1, "secureTextEntry", true)
        .set_prop_null(1, "value");

    let ops = ops_from(&e.into_bytes());
    let props: Vec<(&str, &PropValue)> = ops
        .iter()
        .filter_map(|op| match op {
            MountOp::SetProp { key, value, .. } => Some((key.as_str(), value)),
            _ => None,
        })
        .collect();

    assert!(props.contains(&("placeholder", &PropValue::Str("email".into()))));
    assert!(props.contains(&("fontSize", &PropValue::Number(21.5))));
    assert!(props.contains(&("secureTextEntry", &PropValue::Bool(true))));
    assert!(props.contains(&("value", &PropValue::Null)));
}

/// Would catch: `SET_STYLE` decoding name and value the wrong way round. Both
/// are strings, so there would be no type error anywhere: the view simply would
/// not take the width.
#[test]
fn set_style_does_not_mix_up_the_name_with_the_value() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1).set_style(1, "width", "120");

    let ops = ops_from(&e.into_bytes());
    let frame = ops.iter().rev().find_map(|op| match op {
        MountOp::SetLayout { id: 1, frame } => Some(*frame),
        _ => None,
    });
    assert_eq!(frame.expect("the root has a frame").width, 120.0);
}

/// Would catch: `SET_LISTENER` ignoring the on/off byte, which is what tells
/// hooking a gesture up apart from letting it go.
#[test]
fn hooking_a_listener_up_and_dropping_it_are_not_the_same_thing() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1).set_listener(1, "press", true);
    let ops = ops_from(&e.into_bytes());
    assert!(ops.iter().any(|op| matches!(
        op,
        MountOp::SetListener { id: 1, event, enabled: true } if event == "press"
    )));

    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1).set_listener(1, "press", false);
    let ops = ops_from(&e.into_bytes());
    assert!(ops.iter().any(|op| matches!(
        op,
        MountOp::SetListener { id: 1, event, enabled: false } if event == "press"
    )));
}

/// Would catch: `apply` lying about how many commands it applied. It is the
/// number the runtime looks at to know whether the frame brought any work.
#[test]
fn apply_counts_the_commands_it_applied() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View)
        .create_node(2, NodeKind::View)
        .insert_child(1, 2, 0)
        .set_root(1);
    let mut tree = ShadowTree::new();
    assert_eq!(apply(&e.into_bytes(), &mut tree).unwrap(), 4);
    assert_eq!(apply(&[], &mut tree).unwrap(), 0, "an empty buffer is not an error");
}

// --------------------------------------------------------- a corrupted buffer

/// Would catch: any read that moves the cursor before checking there are bytes
/// left. A panic inside `apply` is not an error you can show anyone: it kills
/// the engine thread and the screen stays exactly as it was.
///
/// The buffer is cut everywhere, not in one place: the failure would be in
/// whichever particular read is left half-done, and which one that is depends on
/// where the cut falls.
#[test]
fn cutting_the_buffer_anywhere_gives_an_error_and_not_a_panic() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View)
        .set_root(1)
        .set_style(1, "width", "100%")
        .set_prop_str(1, "backgroundColor", "#0b1020")
        .set_prop_num(1, "opacity", 0.5)
        .set_prop_bool(1, "hidden", false)
        .set_prop_null(1, "borderColor")
        .create_node(2, NodeKind::Text)
        .set_listener(2, "press", true)
        .insert_child(1, 2, 0)
        .create_node(3, NodeKind::RawText)
        .set_text(3, "hello")
        .insert_child(2, 3, 0)
        .remove_child(2, 3)
        .destroy_node(3);
    let whole = e.into_bytes();

    for cut in 0..whole.len() {
        let mut tree = ShadowTree::new();
        // The only thing demanded is that it answers. A prefix can end right on
        // a command boundary and be perfectly valid.
        let _ = apply(&whole[..cut], &mut tree);
    }
    let mut tree = ShadowTree::new();
    assert!(apply(&whole, &mut tree).is_ok(), "the whole buffer is valid");
}

/// Would catch: `Reader::str` adding the declared length to the offset without
/// first checking that the buffer reaches that far. With `u32::MAX` as the
/// length that is an impossible range, and in a less careful version an attempt
/// to reserve four gigabytes.
#[test]
fn a_string_of_impossible_length_does_not_blow_up() {
    let mut bytes = vec![op::SET_TEXT];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(b"hello");

    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::RawText).unwrap();
    assert!(matches!(apply(&bytes, &mut tree), Err(ProtocolError::Truncated { .. })));
}

/// Would catch: taking a string on trust without validating UTF-8.
/// `str::from_utf8` is what separates a readable error from an `unsafe` with
/// garbage inside it. A lone surrogate —half an emoji cut in two by a `slice` in
/// the app— arrives looking exactly like this.
#[test]
fn bytes_that_are_not_utf8_give_an_error_and_not_a_panic() {
    let mut bytes = vec![op::SET_TEXT];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    // The high surrogate U+D800, encoded as though it were an ordinary
    // character.
    bytes.extend_from_slice(&[0xed, 0xa0, 0x80]);

    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::RawText).unwrap();
    assert!(matches!(apply(&bytes, &mut tree), Err(ProtocolError::InvalidUtf8 { .. })));
}

/// Would catch: an error that says "the buffer is wrong" and nothing else. A
/// frame carries twelve hundred mutations; without the opcode and the offset,
/// finding it means reading the buffer by hand.
#[test]
fn the_error_says_which_command_failed_and_where() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View);
    let prefix = e.into_bytes().len();

    // An opcode that does not exist.
    let mut bytes = {
        let mut e = Encoder::new();
        e.create_node(1, NodeKind::View);
        e.into_bytes()
    };
    bytes.push(0x7f);
    let mut tree = ShadowTree::new();
    assert_eq!(
        apply(&bytes, &mut tree),
        Err(ProtocolError::UnknownOpcode { opcode: 0x7f, offset: prefix })
    );

    // A node kind that does not exist.
    let mut bytes = vec![op::CREATE_NODE];
    bytes.extend_from_slice(&9_u32.to_le_bytes());
    bytes.push(200);
    let mut tree = ShadowTree::new();
    assert_eq!(
        apply(&bytes, &mut tree),
        Err(ProtocolError::UnknownKind { kind: 200, offset: 0 })
    );

    // And a well-formed command the tree turns down: the error carries the
    // opcode, which is what says *which* mutation it was.
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).insert_child(1, 99, 0);
    let bytes = e.into_bytes();
    let mut tree = ShadowTree::new();
    match apply(&bytes, &mut tree) {
        Err(ProtocolError::Tree { opcode, error, .. }) => {
            assert_eq!(opcode, op::INSERT_CHILD);
            assert_eq!(error, an_core::tree::Error::UnknownNode(99));
        }
        other => panic!("it had to say which command failed: {other:?}"),
    }
}

/// Would catch: adding a rollback. It is written down that there is none —a
/// malformed buffer is a bug on the JS side and leaving the tree half-done puts
/// the failure on the screen— and it is worth keeping that a decision rather
/// than an accident.
#[test]
fn an_error_halfway_leaves_what_came_before_applied() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1);
    let mut bytes = e.into_bytes();
    bytes.push(0x7f);

    let mut tree = ShadowTree::new();
    assert!(apply(&bytes, &mut tree).is_err());
    assert_eq!(tree.root(), Some(1), "what was applied before the error stays");
}

// ------------------------------------------------------------ garbage, in bulk

/// A deterministic generator. Neither `rand` nor `proptest` is used, on purpose:
/// twenty lines give the same thousand passes on every run, and a fixed seed
/// makes a failure repeatable without keeping a regressions file around.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

/// Would catch: any path through the decoder that panics on arbitrary input. It
/// is the one test that can be done exhaustively on a byte decoder: there is no
/// knowing what should come out, but there is knowing that something has to.
#[test]
fn no_stream_of_bytes_makes_it_panic() {
    let mut rng = Xorshift(0x5eed_1234_abcd_0001);
    for _ in 0..4_000 {
        let len = rng.below(64);
        let bytes: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let mut tree = ShadowTree::new();
        let _ = apply(&bytes, &mut tree);
    }
}

/// Would catch the same thing, but reaching a great deal further in.
///
/// Random bytes die on the first unknown opcode and prove very little. Flipping
/// bits in a buffer that is valid, most of the mutations are still recognisable
/// commands with one field broken: a huge string length, an id that does not
/// exist, an index out of all proportion. That is where the overflow would be.
#[test]
fn a_valid_buffer_with_flipped_bits_does_not_panic_either() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View)
        .set_root(1)
        .set_style(1, "width", "100%")
        .set_style(1, "flexDirection", "row")
        .create_node(2, NodeKind::Text)
        .set_prop_str(2, "color", "#f4f7ff")
        .set_prop_num(2, "fontSize", 28.0)
        .insert_child(1, 2, 0)
        .create_node(3, NodeKind::RawText)
        .set_text(3, "angular-native")
        .insert_child(2, 3, 0)
        .set_listener(2, "press", true)
        .set_prop_bool(2, "enabled", true)
        .set_prop_null(2, "letterSpacing")
        .remove_child(1, 2)
        .destroy_node(2);
    let original = e.into_bytes();

    let mut rng = Xorshift(0xfeed_face_0000_0007);
    for _ in 0..6_000 {
        let mut bytes = original.clone();
        // Between one and three bits, so as not to stray so far from the format
        // that everything dies on the first byte.
        for _ in 0..=rng.below(3) {
            let at = rng.below(bytes.len());
            bytes[at] ^= 1 << (rng.below(8));
        }
        let mut tree = ShadowTree::new();
        if apply(&bytes, &mut tree).is_ok() {
            // If it applied, the tree has to still be usable: layout is what
            // walks whatever ended up mounted.
            let _ = tree.commit(VIEWPORT, &NaiveMeasurer);
        }
    }
}

/// Would catch: `insert_child` letting a node hang off itself.
///
/// The flipped-bit fuzz found it, and in the worst possible way: one bit in the
/// child's id turns `insert_child(2, 3, 0)` into `insert_child(2, 2, 0)`. The
/// tree accepted it, and then layout walked children looking for a bottom that
/// no longer exists. No panic and no error: the process went round and round for
/// ever, which in an app is a frozen screen.
#[test]
fn a_node_cannot_hang_off_itself() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::View).unwrap();
    tree.insert_child(1, 2, 0).unwrap();

    assert!(matches!(tree.insert_child(2, 2, 0), Err(Error::Cycle { .. })));
    // Nor off one of its own descendants, which is the same cycle one hop
    // longer: 1 is 2's parent, so 1 cannot hang off 2.
    assert!(matches!(tree.insert_child(2, 1, 0), Err(Error::Cycle { .. })));
}

/// Would catch: `insert_child` taking the incoming index on trust. Angular does
/// not send absurd indices, but the buffer comes from outside the core and an
/// index larger than the number of children must not run off the end of the
/// vector.
#[test]
fn an_out_of_all_proportion_insertion_index_gets_clamped() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View)
        .set_root(1)
        .create_node(2, NodeKind::View)
        .insert_child(1, 2, u32::MAX);

    let ops = ops_from(&e.into_bytes());
    assert!(ops.iter().any(|op| matches!(
        op,
        MountOp::Insert { parent: 1, child: 2, index: 0 }
    )));
}
