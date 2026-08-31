//! De JavaScript a marcos resueltos, sin plataforma por medio.

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
fn el_codificador_y_el_decodificador_hablan_el_mismo_formato() {
    let mut encoder = Encoder::new();
    encoder
        .create_node(1, NodeKind::View)
        .set_style(1, "width", "100%")
        .set_style(1, "height", "40")
        .set_root(1)
        .create_node(2, NodeKind::Text)
        .set_prop_num(2, "fontSize", 17.0)
        .create_node(3, NodeKind::RawText)
        .set_text(3, "eñe y emoji 🐿")
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
fn un_buffer_truncado_no_entra_en_bucle() {
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
fn javascript_construye_la_pantalla_entera() {
    let log = Rc::new(CapturedLog::default());
    let mut js = QuickJsRuntime::with_log(log.clone()).unwrap();
    js.eval("main.js", MAIN_JS).unwrap();

    let mut tree = ShadowTree::new();
    let commands = js.tick(0.0).unwrap();
    assert!(!commands.is_empty(), "el primer tick tiene que traer el árbol");
    apply(&commands, &mut tree).unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    assert!(log.0.borrow().iter().any(|l| l.ends_with("main.js montado")));

    // El árbol se montó con los ids que reparte JS: 1 raíz, 2 título,
    // 3 su texto crudo, 4 fila, 5 y 6 tarjetas.
    let root = rect(&frame.ops, 1).unwrap();
    assert_eq!((root.width, root.height), VIEWPORT);

    let (left, right) = (rect(&frame.ops, 5).unwrap(), rect(&frame.ops, 6).unwrap());
    let usable = VIEWPORT.0 - 32.0 - 12.0;
    assert!((left.width - usable / 3.0).abs() < 0.5, "izquierda: {left:?}");
    assert!((right.width - usable * 2.0 / 3.0).abs() < 0.5, "derecha: {right:?}");
    assert_eq!(left.y, right.y);
}

#[test]
fn el_reloj_de_los_temporizadores_lo_marca_el_frame() {
    let mut js = QuickJsRuntime::with_log(Rc::new(CapturedLog::default())).unwrap();
    js.eval("main.js", MAIN_JS).unwrap();

    let mut tree = ShadowTree::new();
    apply(&js.tick(0.0).unwrap(), &mut tree).unwrap();
    tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();

    // Medio segundo: el intervalo de 1000 ms todavía no ha vencido.
    let quiet = js.tick(500.0).unwrap();
    assert!(quiet.is_empty(), "no debería haber comandos: {quiet:?}");

    // Pasado el segundo, el contador se actualiza y solo se remite su texto.
    apply(&js.tick(1500.0).unwrap(), &mut tree).unwrap();
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(
        frame.ops.iter().any(|op| matches!(
            op,
            MountOp::SetText { text, .. } if text == "segundos en marcha: 1"
        )),
        "ops: {:?}",
        frame.ops
    );

    // Tres frames más tarde el contador va por 4, sin acumular retraso.
    for now in [2500.0, 3500.0, 4500.0] {
        apply(&js.tick(now).unwrap(), &mut tree).unwrap();
    }
    let frame = tree.commit(VIEWPORT, &NaiveMeasurer).unwrap();
    assert!(frame.ops.iter().any(|op| matches!(
        op,
        MountOp::SetText { text, .. } if text == "segundos en marcha: 4"
    )));
}

#[test]
fn las_microtareas_entran_en_el_mismo_frame() {
    let mut js = QuickJsRuntime::with_log(Rc::new(CapturedLog::default())).unwrap();
    js.eval(
        "promesa.js",
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
        "la promesa tenía que resolverse antes de cerrar el frame"
    );
}

#[test]
fn una_excepcion_de_js_llega_con_su_traza() {
    let mut js = QuickJsRuntime::with_log(Rc::new(CapturedLog::default())).unwrap();
    let error = js.eval("roto.js", "function boom() { null.x }; boom()").unwrap_err();
    let text = error.to_string();
    assert!(text.contains("boom"), "sin traza útil: {text}");
}
