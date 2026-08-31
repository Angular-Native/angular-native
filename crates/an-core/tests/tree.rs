//! El pipeline completo sin plataforma: mutaciones, layout y diff.

use an_core::props::PropValue;
use an_core::{MountOp, NaiveMeasurer, NodeKind, ShadowTree};

const VIEWPORT: (f32, f32) = (320.0, 568.0);

fn frame_of(ops: &[MountOp], id: u32) -> Option<an_core::Rect> {
    ops.iter().rev().find_map(|op| match op {
        MountOp::SetLayout { id: got, frame } if *got == id => Some(*frame),
        _ => None,
    })
}

/// Raíz que ocupa el viewport, con dos hijos en fila de ancho fijo.
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
fn resuelve_flexbox_en_fila() {
    let mut tree = build_row();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    assert_eq!(frame_of(&frame.ops, 1).unwrap().width, 320.0);
    let left = frame_of(&frame.ops, 2).unwrap();
    let right = frame_of(&frame.ops, 3).unwrap();
    assert_eq!((left.x, left.width), (0.0, 50.0));
    assert_eq!((right.x, right.width), (50.0, 270.0));
}

#[test]
fn commit_sin_cambios_no_produce_ops() {
    let mut tree = build_row();
    assert!(!tree.commit(VIEWPORT, &NaiveMeasurer).unwrap().is_empty());
    let second = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(second.is_empty(), "commit idempotente, salió {:?}", second.ops);
}

#[test]
fn solo_reemite_el_nodo_que_cambia() {
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
    // La raíz no se mueve: solo cambian los dos hijos.
    assert_eq!(touched, vec![2, 3]);
    assert_eq!(frame_of(&frame.ops, 3).unwrap().x, 80.0);
}

#[test]
fn el_texto_determina_el_alto() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::Text).unwrap();
    tree.create_node(3, NodeKind::RawText).unwrap();
    tree.set_style(1, "width", "100").unwrap();
    tree.set_style(1, "height", "100%").unwrap();
    tree.set_prop(2, "fontSize", PropValue::Number(20.0)).unwrap();
    tree.set_text(3, "una frase bastante larga que obliga a partir en varias lineas")
        .unwrap();
    tree.insert_child(2, 3, 0).unwrap();
    tree.insert_child(1, 2, 0).unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    let text = frame_of(&frame.ops, 2).unwrap();
    assert!(text.height > 20.0 * 1.25, "debe ocupar más de una línea: {text:?}");
    assert!(text.width <= 100.0);

    // El host recibe el texto ya concatenado, no los nodos crudos.
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetText { id: 2, text } if text.starts_with("una frase")
    )));
    // Un `RawText` nunca se monta.
    assert!(!frame.ops.iter().any(|op| matches!(op, MountOp::Create { id: 3, .. })));
}

#[test]
fn indice_de_host_ignora_nodos_no_montables() {
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
fn destruir_baja_el_subarbol_de_hijos_a_padres() {
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

#[test]
fn estilo_no_reconocido_viaja_como_prop_de_host() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.set_style(1, "background-color", "#ff0000").unwrap();
    tree.set_root(1).unwrap();

    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetProp { id: 1, key, value: PropValue::Str(v) }
            if key == "background-color" && v == "#ff0000"
    )));
}
