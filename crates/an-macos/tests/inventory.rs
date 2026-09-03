//! `support.rs`'s inventory, from the inside.
//!
//! It runs on any machine and without opening any window: `support` sits
//! outside `cfg(target_os = "macos")` precisely for this.
//!
//! The list of primitives is not repeated here. That the table covers the
//! whole enum is `scripts/check-macos.py`'s to check, reading both files:
//! copying the twenty-five names in here would add the fourth hand-maintained
//! list, which is what `check-kinds.sh` and `check-styles.sh` exist to avoid.
//! What is looked at from Rust is what a script cannot see without rewriting
//! the compiler: that every entry says something, that none is repeated, and
//! that what is not a view is not in it.

use an_core::NodeKind;
use an_macos::support::{support, Support, KNOWN_EVENTS, SUPPORT};

/// A `Missing` or an `Assembled` with no reason is the same as saying nothing:
/// the report comes out with a gap and whoever reads it is none the wiser.
#[test]
fn what_is_not_native_comes_with_its_reason() {
    for (kind, entry) in SUPPORT {
        let text = match entry {
            Support::Native(name) => name,
            Support::Assembled(reason)
            | Support::Missing(reason)
            | Support::Elsewhere(reason) => reason,
        };
        assert!(!text.trim().is_empty(), "{kind:?} does not say what is behind it");
    }
}

/// `support()` returns the first match, so a repeated entry leaves a second
/// one nobody ever reads: changing it would have no effect and would raise no
/// error either.
#[test]
fn no_primitive_appears_twice() {
    for (kind, _) in SUPPORT {
        assert_eq!(
            SUPPORT.iter().filter(|(k, _)| k == kind).count(),
            1,
            "{kind:?} appears twice in the table"
        );
    }
}

/// `RawText` is never a view. Were it ever to become one, this test says so
/// before the host mounts it as an empty box.
#[test]
fn raw_text_is_not_a_view() {
    assert!(support(NodeKind::RawText).is_none());
}

/// The host consults `is_known_event` with a linear search; a repeated name
/// breaks nothing, but it gives away that the list was edited blind, and the
/// next edit may remove only one of the two copies.
#[test]
fn event_names_are_not_repeated() {
    for event in KNOWN_EVENTS {
        assert_eq!(
            KNOWN_EVENTS.iter().filter(|e| *e == event).count(),
            1,
            "\"{event}\" appears twice in KNOWN_EVENTS"
        );
    }
}

/// Which way each sign goes in an AppKit swipe.
///
/// It is the one thing about this host that cannot be checked by running it:
/// it takes a trackpad and a hand on it. What can be done is not to get what
/// Apple says wrong while copying it, which is what this test pins down. From
/// `NSEvent.h`: "A non-0 deltaX will represent a horizontal swipe, -1 for
/// swipe right and 1 for swipe left. A non-0 deltaY will represent a vertical
/// swipe, -1 for swipe down and 1 for swipe up."
#[test]
fn the_swipes_sign_is_the_one_appkit_says() {
    use an_macos::support::{swipe_bit, swipe_direction};

    assert_eq!(swipe_direction(-1.0, 0.0).map(|(_, n)| n), Some("swipeRight"));
    assert_eq!(swipe_direction(1.0, 0.0).map(|(_, n)| n), Some("swipeLeft"));
    assert_eq!(swipe_direction(0.0, -1.0).map(|(_, n)| n), Some("swipeDown"));
    assert_eq!(swipe_direction(0.0, 1.0).map(|(_, n)| n), Some("swipeUp"));
    // With no direction there is no swipe: the event is passed on down the
    // chain.
    assert!(swipe_direction(0.0, 0.0).is_none());

    // And the bit that comes out of the direction is the one subscribed to
    // under that name, or a view would listen for one direction and receive
    // another.
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        let (bit, name) = swipe_direction(dx, dy).expect("there is a direction");
        assert_eq!(swipe_bit(name), Some(bit), "\"{name}\" is subscribed to under another bit");
    }
}
