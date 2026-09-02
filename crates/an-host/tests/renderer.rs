//! The whole loop: mutations, commit, and ops landing on the host.

use an_core::{NaiveMeasurer, NodeKind};
use an_host::{new_event_queue, HostEvent, RecordingHost, Renderer};

#[test]
fn it_mounts_and_only_repeats_what_changed() {
    let mut renderer = Renderer::new(RecordingHost::default(), NaiveMeasurer, (320.0, 568.0), new_event_queue());

    renderer.create_node(1, NodeKind::View).unwrap();
    renderer.set_style(1, "width", "100%").unwrap();
    renderer.set_style(1, "height", "100%").unwrap();
    renderer.set_root(1).unwrap();
    renderer.create_node(2, NodeKind::View).unwrap();
    renderer.set_style(2, "height", "44").unwrap();
    renderer.insert_child(1, 2, 0).unwrap();

    assert!(renderer.render_frame().unwrap() > 0);
    assert_eq!(renderer.render_frame().unwrap(), 0, "the second frame has to be empty");

    let log = &renderer.host().log;
    assert!(log.iter().any(|l| l == "create 2 View"));
    assert!(log.iter().any(|l| l == "insert 2 into 1 at 0"));

    let (_, frame) = renderer.host().frames.iter().find(|(id, _)| *id == 2).unwrap();
    assert_eq!((frame.width, frame.height), (320.0, 44.0));
}

#[test]
fn host_events_are_drained_exactly_once() {
    let mut renderer = Renderer::new(RecordingHost::default(), NaiveMeasurer, (320.0, 568.0), new_event_queue());
    renderer.push_event(HostEvent { target: 7, name: "press".into(), payload: vec![] });

    assert_eq!(renderer.drain_events().len(), 1);
    assert!(renderer.drain_events().is_empty());
}
