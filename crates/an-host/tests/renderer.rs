//! El lazo completo: mutaciones, commit, y ops llegando al host.

use an_core::{NaiveMeasurer, NodeKind};
use an_host::{HostEvent, RecordingHost, Renderer};

#[test]
fn monta_y_solo_repite_lo_que_cambia() {
    let mut renderer = Renderer::new(RecordingHost::default(), NaiveMeasurer, (320.0, 568.0));

    renderer.tree.create_node(1, NodeKind::View).unwrap();
    renderer.tree.set_style(1, "width", "100%").unwrap();
    renderer.tree.set_style(1, "height", "100%").unwrap();
    renderer.tree.set_root(1).unwrap();
    renderer.tree.create_node(2, NodeKind::View).unwrap();
    renderer.tree.set_style(2, "height", "44").unwrap();
    renderer.tree.insert_child(1, 2, 0).unwrap();

    assert!(renderer.render_frame().unwrap() > 0);
    assert_eq!(renderer.render_frame().unwrap(), 0, "segundo frame debe ser vacío");

    let log = &renderer.host().log;
    assert!(log.iter().any(|l| l == "create 2 View"));
    assert!(log.iter().any(|l| l == "insert 2 into 1 at 0"));

    let (_, frame) = renderer.host().frames.iter().find(|(id, _)| *id == 2).unwrap();
    assert_eq!((frame.width, frame.height), (320.0, 44.0));
}

#[test]
fn los_eventos_del_host_se_drenan_una_sola_vez() {
    let mut renderer = Renderer::new(RecordingHost::default(), NaiveMeasurer, (320.0, 568.0));
    renderer.push_event(HostEvent { target: 7, name: "press".into(), payload: vec![] });

    assert_eq!(renderer.drain_events().len(), 1);
    assert!(renderer.drain_events().is_empty());
}
