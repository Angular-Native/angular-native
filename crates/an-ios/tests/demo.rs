//! El árbol de demostración, verificado sin simulador con el medidor
//! aproximado. Comprueba la forma del layout, no los píxeles de UIKit.

use an_core::{MountOp, NaiveMeasurer, ShadowTree};

#[test]
fn el_arbol_de_demostracion_cabe_en_la_pantalla() {
    let viewport = (393.0_f32, 852.0_f32); // iPhone 17 Pro en puntos
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
            .unwrap_or_else(|| panic!("el nodo {id} no recibió layout"))
    };

    // Raíz a pantalla completa, con 16 de padding lateral.
    assert_eq!(rect(1).width, viewport.0);
    let content_width = viewport.0 - 32.0;

    // Las tarjetas reparten el ancho 1:2 con 12 de hueco entre ellas.
    let (left, right) = (rect(5), rect(6));
    let usable = content_width - 12.0;
    assert!((left.width - usable / 3.0).abs() < 0.5, "izquierda: {left:?}");
    assert!((right.width - usable * 2.0 / 3.0).abs() < 0.5, "derecha: {right:?}");
    assert_eq!(left.y, right.y, "las tarjetas van a la misma altura");
    assert!((right.x - (left.x + left.width + 12.0)).abs() < 0.5);

    // El párrafo parte en varias líneas y queda por debajo de las tarjetas.
    let paragraph = rect(7);
    assert!(paragraph.y > right.y + right.height, "párrafo: {paragraph:?}");
    assert!(paragraph.height > 40.0, "debe ocupar varias líneas: {paragraph:?}");

    // Ningún nodo se sale por el lado derecho.
    for op in &frame.ops {
        if let MountOp::SetLayout { id, frame } = op {
            assert!(frame.width <= viewport.0, "nodo {id} más ancho que la pantalla");
        }
    }
}
