//! El protocolo binario, mirado como lo que es: un decodificador de bytes que
//! recibe lo que escribe otro lenguaje.
//!
//! Aquí se comprueban tres cosas distintas, y conviene no mezclarlas:
//!
//! 1. **El formato está fijado.** Los bytes exactos de cada comando y el código
//!    de cada primitiva. Si alguien los cambia en Rust, el prelude de JS sigue
//!    escribiendo los de antes y lo que se monta es otra cosa —o nada— sin que
//!    salte ningún error.
//! 2. **Ningún búfer hace panic.** Un panic dentro de `apply` se lleva el hilo
//!    del motor por delante y la pantalla se queda congelada sin decir por qué.
//!    Lo correcto ante basura es un `Err` que diga qué comando y en qué
//!    posición.
//! 3. **El error nombra el comando.** Que falle "algo" no sirve de nada cuando
//!    en un frame van mil doscientas mutaciones.

use an_bridge::protocol::{kind_from_byte, kind_to_byte, op};
use an_bridge::{apply, Encoder, ProtocolError};
use an_core::tree::Error;
use an_core::{MountOp, NaiveMeasurer, NodeKind, PropValue, ShadowTree};

const VIEWPORT: (f32, f32) = (393.0, 852.0);

/// Las 26 clases de nodo, en el orden del enum. Es la tabla que hay que tocar
/// al añadir una primitiva.
const TODAS: [NodeKind; 26] = [
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

fn ops_de(bytes: &[u8]) -> Vec<MountOp> {
    let mut tree = ShadowTree::new();
    apply(bytes, &mut tree).expect("el búfer tenía que ser válido");
    tree.commit(VIEWPORT, &NaiveMeasurer).expect("commit").ops
}

// ---------------------------------------------------------------- el formato

/// Cazaría: una clase de nodo añadida a `kind_to_byte` y olvidada en
/// `kind_from_byte`.
///
/// El compilador obliga a completar `kind_to_byte` —es un `match` exhaustivo
/// sobre el enum— pero no dice nada de la vuelta, que es un `match` sobre un
/// `u8`. Una primitiva nueva viajaría con su código y el decodificador la
/// rechazaría entera: la app no monta nada y el error habla de un byte.
#[test]
fn toda_clase_de_nodo_sobrevive_a_la_ida_y_la_vuelta() {
    for kind in TODAS {
        let byte = kind_to_byte(kind);
        assert_eq!(
            kind_from_byte(byte),
            Some(kind),
            "{kind:?} viaja como {byte} y a la vuelta no es la misma"
        );
    }
}

/// Cazaría: renumerar las clases de nodo en Rust.
///
/// El código no es un detalle interno: es el contrato con `KIND` en
/// `packages/runtime/runtime.js`. `check-kinds.sh` comprueba que las dos
/// tablas coincidan, así que renumerar las dos a la vez pasaría su revisión;
/// esta lista fija además el número, que es lo que hace que un bundle ya
/// compilado siga significando lo mismo.
#[test]
fn el_codigo_de_cada_clase_esta_clavado() {
    let esperado: [(NodeKind, u8); 26] = [
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
        // `<an-select>` en la plantilla, `Picker` en el núcleo. El número es
        // lo único que los dos vocabularios comparten.
        (NodeKind::Picker, 19),
        (NodeKind::DatePicker, 20),
        (NodeKind::NavigationBar, 21),
        // Y `<an-textarea>` es `TextEditor`, por lo mismo.
        (NodeKind::TextEditor, 22),
        (NodeKind::WebView, 23),
        (NodeKind::MapView, 24),
        (NodeKind::VideoView, 25),
    ];
    for (kind, byte) in esperado {
        assert_eq!(kind_to_byte(kind), byte, "{kind:?} cambió de código");
    }
    assert_eq!(kind_from_byte(26), None, "26 todavía no es de nadie");
}

/// Cazaría: cambiar el orden de los campos, el tamaño de un entero o el
/// endianness de cualquier comando.
///
/// El prelude escribe estos bytes a mano —`writer.u32`, `writer.str`— y no
/// comparte código con este lado. Los únicos que pueden avisar de que se han
/// separado son los bytes.
#[test]
fn cada_comando_ocupa_exactamente_los_bytes_acordados() {
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

    // Las cadenas van con longitud en bytes, no en caracteres: la eñe ocupa
    // dos y el decodificador lee esos dos.
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

/// Cazaría: un opcode reasignado. Son doce números y el prelude los repite.
#[test]
fn los_doce_opcodes_estan_clavados() {
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

// ------------------------------------------------------- los doce, uno a uno

/// Cazaría: un opcode que se decodifica pero cuyo efecto sobre el árbol se
/// pierde por el camino —leer los campos en otro orden, por ejemplo—.
#[test]
fn los_cuatro_tipos_de_prop_llegan_con_su_tipo() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::TextInput)
        .set_root(1)
        .set_prop_str(1, "placeholder", "correo")
        .set_prop_num(1, "fontSize", 21.5)
        .set_prop_bool(1, "secureTextEntry", true)
        .set_prop_null(1, "value");

    let ops = ops_de(&e.into_bytes());
    let props: Vec<(&str, &PropValue)> = ops
        .iter()
        .filter_map(|op| match op {
            MountOp::SetProp { key, value, .. } => Some((key.as_str(), value)),
            _ => None,
        })
        .collect();

    assert!(props.contains(&("placeholder", &PropValue::Str("correo".into()))));
    assert!(props.contains(&("fontSize", &PropValue::Number(21.5))));
    assert!(props.contains(&("secureTextEntry", &PropValue::Bool(true))));
    assert!(props.contains(&("value", &PropValue::Null)));
}

/// Cazaría: `SET_STYLE` decodificando nombre y valor al revés. Los dos son
/// cadenas, así que no habría error de tipo en ninguna parte: la vista
/// simplemente no cogería el ancho.
#[test]
fn set_style_no_confunde_el_nombre_con_el_valor() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1).set_style(1, "width", "120");

    let ops = ops_de(&e.into_bytes());
    let frame = ops.iter().rev().find_map(|op| match op {
        MountOp::SetLayout { id: 1, frame } => Some(*frame),
        _ => None,
    });
    assert_eq!(frame.expect("la raíz tiene marco").width, 120.0);
}

/// Cazaría: `SET_LISTENER` ignorando el byte de alta/baja, que es lo que
/// distingue enganchar un gesto de soltarlo.
#[test]
fn el_alta_y_la_baja_de_un_oyente_no_son_lo_mismo() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1).set_listener(1, "press", true);
    let ops = ops_de(&e.into_bytes());
    assert!(ops.iter().any(|op| matches!(
        op,
        MountOp::SetListener { id: 1, event, enabled: true } if event == "press"
    )));

    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1).set_listener(1, "press", false);
    let ops = ops_de(&e.into_bytes());
    assert!(ops.iter().any(|op| matches!(
        op,
        MountOp::SetListener { id: 1, event, enabled: false } if event == "press"
    )));
}

/// Cazaría: `apply` mintiendo sobre cuántos comandos aplicó. Es el número que
/// mira el runtime para saber si el frame trajo trabajo.
#[test]
fn apply_cuenta_los_comandos_que_aplico() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View)
        .create_node(2, NodeKind::View)
        .insert_child(1, 2, 0)
        .set_root(1);
    let mut tree = ShadowTree::new();
    assert_eq!(apply(&e.into_bytes(), &mut tree).unwrap(), 4);
    assert_eq!(apply(&[], &mut tree).unwrap(), 0, "un búfer vacío no es un error");
}

// ------------------------------------------------------------ búfer corrupto

/// Cazaría: cualquier lectura que avance el cursor antes de comprobar que hay
/// bytes. Un panic dentro de `apply` no es un error que se pueda enseñar:
/// mata el hilo del motor y la pantalla se queda como estaba.
///
/// Se corta el búfer por todas partes, no por una: el fallo estaría en la
/// lectura concreta que quede a medias, y cuál es depende de dónde se corte.
#[test]
fn cortar_el_buffer_por_cualquier_sitio_da_error_y_no_panico() {
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
        .set_text(3, "hola")
        .insert_child(2, 3, 0)
        .remove_child(2, 3)
        .destroy_node(3);
    let completo = e.into_bytes();

    for corte in 0..completo.len() {
        let mut tree = ShadowTree::new();
        // Lo único que se exige es que conteste. Un prefijo puede terminar
        // justo en el límite de un comando y ser válido.
        let _ = apply(&completo[..corte], &mut tree);
    }
    let mut tree = ShadowTree::new();
    assert!(apply(&completo, &mut tree).is_ok(), "el búfer entero sí es válido");
}

/// Cazaría: `Reader::str` sumando la longitud declarada al desplazamiento sin
/// comprobar antes que el búfer llegue hasta ahí. Con `u32::MAX` como longitud
/// eso es un rango imposible, y en una versión menos cuidadosa un intento de
/// reservar cuatro gigas.
#[test]
fn una_cadena_de_longitud_imposible_no_revienta() {
    let mut bytes = vec![op::SET_TEXT];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(b"hola");

    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::RawText).unwrap();
    assert!(matches!(apply(&bytes, &mut tree), Err(ProtocolError::Truncated { .. })));
}

/// Cazaría: dar por buena una cadena sin validar UTF-8. `str::from_utf8` es lo
/// que separa un error legible de un `unsafe` con basura dentro. Un sustituto
/// suelto —el trozo de un emoji cortado por una `slice` en la app— llega
/// exactamente así.
#[test]
fn bytes_que_no_son_utf8_dan_error_y_no_panico() {
    let mut bytes = vec![op::SET_TEXT];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    // El sustituto alto U+D800 codificado como si fuera un carácter normal.
    bytes.extend_from_slice(&[0xed, 0xa0, 0x80]);

    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::RawText).unwrap();
    assert!(matches!(apply(&bytes, &mut tree), Err(ProtocolError::InvalidUtf8 { .. })));
}

/// Cazaría: un error que diga "el búfer está mal" y nada más. En un frame van
/// mil doscientas mutaciones; sin el opcode y el desplazamiento, buscarlo es
/// leer el búfer a mano.
#[test]
fn el_error_dice_que_comando_fallo_y_donde() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View);
    let prefijo = e.into_bytes().len();

    // Un opcode que no existe.
    let mut bytes = {
        let mut e = Encoder::new();
        e.create_node(1, NodeKind::View);
        e.into_bytes()
    };
    bytes.push(0x7f);
    let mut tree = ShadowTree::new();
    assert_eq!(
        apply(&bytes, &mut tree),
        Err(ProtocolError::UnknownOpcode { opcode: 0x7f, offset: prefijo })
    );

    // Una clase de nodo que no existe.
    let mut bytes = vec![op::CREATE_NODE];
    bytes.extend_from_slice(&9_u32.to_le_bytes());
    bytes.push(200);
    let mut tree = ShadowTree::new();
    assert_eq!(
        apply(&bytes, &mut tree),
        Err(ProtocolError::UnknownKind { kind: 200, offset: 0 })
    );

    // Y un comando bien formado que el árbol rechaza: el error lleva el
    // opcode, que es lo que dice *qué* mutación era.
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).insert_child(1, 99, 0);
    let bytes = e.into_bytes();
    let mut tree = ShadowTree::new();
    match apply(&bytes, &mut tree) {
        Err(ProtocolError::Tree { opcode, error, .. }) => {
            assert_eq!(opcode, op::INSERT_CHILD);
            assert_eq!(error, an_core::tree::Error::UnknownNode(99));
        }
        otro => panic!("tenía que decir qué comando falló: {otro:?}"),
    }
}

/// Cazaría: añadir un rollback. Está escrito que no lo hay —un búfer mal
/// formado es un bug del lado JS y dejar el árbol a medias hace el fallo
/// visible en pantalla— y conviene que siga siendo una decisión y no un
/// accidente.
#[test]
fn un_error_a_mitad_deja_aplicado_lo_de_antes() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View).set_root(1);
    let mut bytes = e.into_bytes();
    bytes.push(0x7f);

    let mut tree = ShadowTree::new();
    assert!(apply(&bytes, &mut tree).is_err());
    assert_eq!(tree.root(), Some(1), "lo aplicado antes del error se queda");
}

// ------------------------------------------------------- basura, a montones

/// Generador determinista. No se usa `rand` ni `proptest` a propósito: veinte
/// líneas dan las mismas mil pasadas por ejecución, y una semilla fija hace
/// que un fallo se pueda repetir sin guardar un fichero de regresiones.
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

/// Cazaría: cualquier camino del decodificador que haga panic con entrada
/// arbitraria. Es la prueba que sí se puede hacer exhaustivamente sobre un
/// decodificador de bytes: no se sabe qué debería salir, pero sí que tiene
/// que salir algo.
#[test]
fn ningun_reguero_de_bytes_hace_panic() {
    let mut rng = Xorshift(0x5eed_1234_abcd_0001);
    for _ in 0..4_000 {
        let len = rng.below(64);
        let bytes: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let mut tree = ShadowTree::new();
        let _ = apply(&bytes, &mut tree);
    }
}

/// Cazaría lo mismo, pero llegando mucho más adentro.
///
/// Bytes al azar mueren en el primer opcode desconocido y no prueban gran
/// cosa. Volteando bits de un búfer que sí es válido, la mayoría de las
/// mutaciones siguen siendo comandos reconocibles con un campo estropeado:
/// una longitud de cadena enorme, un id que no existe, un índice desmesurado.
/// Ahí es donde estaría el desbordamiento.
#[test]
fn un_buffer_valido_con_bits_volteados_tampoco_hace_panic() {
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
        // Entre uno y tres bits, para no alejarse tanto del formato que todo
        // muera en el primer byte.
        for _ in 0..=rng.below(3) {
            let at = rng.below(bytes.len());
            bytes[at] ^= 1 << (rng.below(8));
        }
        let mut tree = ShadowTree::new();
        if apply(&bytes, &mut tree).is_ok() {
            // Si se aplicó, el árbol tiene que seguir siendo utilizable: el
            // layout es lo que recorre lo que quedó montado.
            let _ = tree.commit(VIEWPORT, &NaiveMeasurer);
        }
    }
}

/// Cazaría: `insert_child` dejando que un nodo cuelgue de sí mismo.
///
/// Lo encontró el fuzz de bits volteados, y de la peor manera posible: un bit
/// en el id del hijo convierte `insert_child(2, 3, 0)` en `insert_child(2, 2,
/// 0)`. El árbol lo aceptaba, y entonces el layout recorría hijos buscando un
/// fondo que ya no existe. Ni pánico ni error: el proceso se quedaba dando
/// vueltas para siempre, que en una app es una pantalla congelada.
#[test]
fn un_nodo_no_puede_colgar_de_si_mismo() {
    let mut tree = ShadowTree::new();
    tree.create_node(1, NodeKind::View).unwrap();
    tree.create_node(2, NodeKind::View).unwrap();
    tree.insert_child(1, 2, 0).unwrap();

    assert!(matches!(tree.insert_child(2, 2, 0), Err(Error::Cycle { .. })));
    // Y tampoco de un descendiente suyo, que es el mismo ciclo con un salto
    // más: 1 es el padre de 2, así que 1 no puede colgar de 2.
    assert!(matches!(tree.insert_child(2, 1, 0), Err(Error::Cycle { .. })));
}

/// Cazaría: `insert_child` fiándose del índice que llega. Angular no manda
/// índices absurdos, pero el búfer viene de fuera del núcleo y un índice
/// mayor que el número de hijos no puede salirse del vector.
#[test]
fn un_indice_de_insercion_desmesurado_se_recorta() {
    let mut e = Encoder::new();
    e.create_node(1, NodeKind::View)
        .set_root(1)
        .create_node(2, NodeKind::View)
        .insert_child(1, 2, u32::MAX);

    let ops = ops_de(&e.into_bytes());
    assert!(ops.iter().any(|op| matches!(
        op,
        MountOp::Insert { parent: 1, child: 2, index: 0 }
    )));
}
