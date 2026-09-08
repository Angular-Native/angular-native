//! What one frame of the watch's snapshot costs.
//!
//! The shell serialises the whole tree every time the revision moves, and the
//! only way to know whether that matters is to time it on a real bundle rather
//! than on a tree written by hand. This mounts an example the way the device
//! does —QuickJS, taffy, `MountSide<WatchHost>`— and then reports, for a
//! settled screen: how many nodes, how many bytes of JSON, and how long the
//! walk and `serde_json` each take.
//!
//! Half of the answer is not here. Rust's half of a changed frame is tens of
//! microseconds; Swift's `JSONDecoder` is around seven times that, because the
//! synthesised `init(from:)` asks for all sixty optional keys of `AnNode` on
//! every node whether they were sent or not. `AN_DUMP=/tmp/screen.json` writes
//! the exact bytes out so the other half can be timed against `AnTree.swift`'s
//! own structs. Optimising what this file prints without looking at that would
//! be sharpening the cheaper end.
//!
//! Usage: cargo run --release -p an-watch --example snapshot-cost -- bundle.js [frames]

use std::time::Instant;

use an_bridge::{apply, JsRuntime, QuickJsRuntime};
use an_host::{new_event_queue, Renderer};
use an_watch::{snapshot, WatchHost, WatchMeasurer};

/// A 46 mm Series 11, which is the biggest watch there is: the widest screen is
/// the worst case for a snapshot, because more fits on it.
const VIEWPORT: (f32, f32) = (208.0, 248.0);

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: snapshot-cost bundle.js [frames]");
    let frames: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(6);
    let code = std::fs::read_to_string(&path).expect("the bundle could not be read");

    let mut js = QuickJsRuntime::with_options(
        std::rc::Rc::new(an_bridge::runtime::StderrLog),
        QuickJsRuntime::DEFAULT_STACK_SIZE,
    )
    .expect("the JS engine never started");
    if let Err(error) = js.eval(&path, &code) {
        eprintln!("{error}");
        std::process::exit(1);
    }

    let events = new_event_queue();
    let mut renderer = Renderer::new(
        WatchHost::new(events),
        WatchMeasurer::new(Default::default()),
        VIEWPORT,
        new_event_queue(),
    );

    // How many of the frames raised the revision. The shell only serialises on
    // those, so the cost below is paid this often and not thirty times a
    // second.
    let mut changed = 0;
    let mut previous = 0;
    for frame in 0..frames {
        let now = frame as f64 * 33.0;
        let pending = renderer.drain_events();
        if !pending.is_empty() {
            js.dispatch_events(&pending).expect("dispatching events");
        }
        let commands = js.tick(now).expect("the frame's turn");
        apply(&commands, &mut renderer).expect("the frame's buffer");
        renderer.render_frame().expect("commit");
        let revision = renderer.host().revision();
        if revision != previous {
            changed += 1;
            previous = revision;
        }
    }

    let host = renderer.host();
    let json = serde_json::to_string(&snapshot(host)).unwrap();
    let nodes = count(&snapshot(host));
    // The same bytes the shell would decode, so the Swift side can be timed on
    // exactly what Rust sends and not on something written to look like it.
    if let Ok(dump) = std::env::var("AN_DUMP") {
        std::fs::write(&dump, &json).expect("the snapshot could not be written");
    }
    println!("{path}");
    println!("  nodes in the snapshot: {nodes}");
    println!("  frames that changed:   {changed} of {frames}");
    println!("  bytes of JSON:         {}", json.len());

    // Enough repetitions that the timer's resolution does not decide the
    // answer, and the best of the runs rather than the mean: what is wanted is
    // the cost of the work, not the cost of whatever else the Mac was doing.
    let rounds = 200;
    let mut walk = f64::MAX;
    let mut whole = f64::MAX;
    let mut encode = f64::MAX;
    for _ in 0..rounds {
        let start = Instant::now();
        let snap = snapshot(host);
        let walked = start.elapsed().as_secs_f64() * 1e6;
        let start = Instant::now();
        let text = serde_json::to_string(&snap).unwrap();
        let encoded = start.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(&text);
        walk = walk.min(walked);
        encode = encode.min(encoded);
        whole = whole.min(walked + encoded);
    }
    println!("  walk:                  {walk:.1} us");
    println!("  serde_json:            {encode:.1} us");
    println!("  both:                  {whole:.1} us");
    println!("  budget at 30 Hz:       {:.2}% of 33.3 ms", whole / 333.0);
}

fn count(snap: &an_watch::Snapshot) -> usize {
    fn walk(node: &an_watch::snapshot::Node) -> usize {
        1 + node.children.iter().map(walk).sum::<usize>()
    }
    snap.root.as_ref().map_or(0, walk) + snap.overlays.iter().map(walk).sum::<usize>()
}
