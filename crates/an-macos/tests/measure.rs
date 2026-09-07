//! The measurement cache, from the outside.
//!
//! It runs where the host runs — this is a Mac and `AppKitMeasurer` is
//! `cfg(target_os = "macos")` — so `NSFont` and Core Text are the real ones and
//! not a stand-in. That is the whole reason this is worth writing: what is
//! being checked is not arithmetic, it is that two texts the system measures
//! differently are not handed the same answer.

#![cfg(target_os = "macos")]

use an_layout::{FontSpec, TextMeasurer};
use an_macos::AppKitMeasurer;

fn spec() -> FontSpec {
    FontSpec { size: 17.0, ..FontSpec::default() }
}

/// A cache key that leaves out something the answer depends on is the worst
/// kind of wrong: it is right the first time and wrong afterwards, and which
/// way round depends on which text happened to be laid out first.
///
/// `line_height` and `max_lines` both change the height that comes back —
/// `height = lines * line_height` at the end of `measure_text` — and neither
/// was in the key. Two `<an-text>` alike in everything but their `lineHeight`
/// shared an entry, and the second one silently got the first one's height.
#[test]
fn line_height_is_part_of_what_is_remembered() {
    let measurer = AppKitMeasurer::default();
    let tall = FontSpec { line_height: Some(40.0), ..spec() };
    let short = FontSpec { line_height: Some(18.0), ..spec() };

    let (_, tall_height) = measurer.measure_text("one line", &tall, None);
    let (_, short_height) = measurer.measure_text("one line", &short, None);

    assert!(
        tall_height > short_height,
        "a 40-point line came back as {tall_height} and an 18-point one as {short_height}: \
         the second read the first's cache entry"
    );
}

/// The same, the other way round: asking for the short one first must not make
/// the tall one short.
#[test]
fn the_order_they_are_asked_in_does_not_decide_the_answer() {
    let first = AppKitMeasurer::default();
    let second = AppKitMeasurer::default();
    let tall = FontSpec { line_height: Some(40.0), ..spec() };
    let short = FontSpec { line_height: Some(18.0), ..spec() };

    let (_, tall_after_short) = {
        first.measure_text("one line", &short, None);
        first.measure_text("one line", &tall, None)
    };
    let (_, tall_alone) = second.measure_text("one line", &tall, None);

    assert_eq!(
        tall_after_short, tall_alone,
        "the tall line measured {tall_after_short} behind a short one and {tall_alone} on its own"
    );
}

/// `max_lines` clamps the line count, so it changes the height too, and it was
/// missing from the key for the same reason.
#[test]
fn the_line_limit_is_part_of_what_is_remembered() {
    let measurer = AppKitMeasurer::default();
    let text = "a paragraph long enough that it has to wrap more than twice at this width";
    let unlimited = spec();
    let one_line = FontSpec { max_lines: Some(1), ..spec() };

    let (_, full) = measurer.measure_text(text, &unlimited, Some(120.0));
    let (_, clamped) = measurer.measure_text(text, &one_line, Some(120.0));

    assert!(
        clamped < full,
        "clamped to one line it came back as {clamped} and unclamped as {full}"
    );
}
