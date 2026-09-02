//! Las tablas que hay que tocar a mano cuando entra una primitiva nueva.
//!
//! Ninguna de ellas falla si se queda atrás: una clase que no aparece en una
//! tabla no da error, simplemente deja de recibir lo que esa tabla reparte.
//! Un control que nadie sabe medir sale de cero puntos, y una vista de cero
//! puntos es una vista que no está.

use an_core::props::{affects_measure, font_from_props, NodeKind};
use an_core::{NaiveMeasurer, PropValue, TextMeasurer};

/// Las 26 clases del enum. Si el compilador se queja de que falta una, es que
/// entró una primitiva: añádela aquí y mira qué otros tests se ponen rojos.
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

/// Cazaría —y cazó— un control cuyo nombre no está en la tabla de medidas por
/// defecto.
///
/// `NodeKind::control_name()` es lo que viaja hasta el medidor, y el medidor
/// contesta `(0, 0)` a lo que no reconoce. Un control de cero por cero no da
/// error: se monta, recibe sus props, y no se ve. Eso es justo lo que le pasó
/// al desplegable, que en la tabla figuraba como "Select" mientras el núcleo
/// lo llamaba "Picker" desde siempre; solo se notaba si la plantilla no le
/// daba un alto a mano, y la que hay en `examples/pickers` se lo da.
#[test]
fn todo_control_tiene_una_medida_natural_que_no_es_cero() {
    for kind in TODAS.into_iter().filter(|k| k.is_control()) {
        let name = kind.control_name();
        assert!(!name.is_empty(), "{kind:?} es un control y no dice cómo se llama");
        let (width, height) = NaiveMeasurer.measure_control(name, None);
        assert!(
            width > 0.0 && height > 0.0,
            "{kind:?} viaja como {name:?} y el medidor por defecto \
             le da {width}x{height}: se montaría invisible"
        );
    }
}

/// Cazaría: darle nombre de control a algo que no lo es. `control_name()`
/// devuelve `""` para el resto, y una cadena vacía como clave de medición
/// sería otra medida de cero que nadie explica.
#[test]
fn solo_los_controles_tienen_nombre_de_control() {
    for kind in TODAS.into_iter().filter(|k| !k.is_control()) {
        assert_eq!(kind.control_name(), "", "{kind:?} no es un control");
    }
}

/// Cazaría: una primitiva nueva que no se pueda nombrar desde una etiqueta.
///
/// `from_tag` es la puerta por la que entra cualquier nombre que no venga del
/// protocolo binario —el host de demostración de iOS, por ejemplo—. Una clase
/// que no esté ahí no se puede pedir, y el `None` se traduce en un envoltorio
/// silencioso.
#[test]
fn toda_primitiva_montable_se_puede_nombrar_por_etiqueta() {
    for kind in TODAS.into_iter().filter(|k| k.is_mountable()) {
        let pascal = format!("{kind:?}");
        assert_eq!(
            NodeKind::from_tag(&pascal),
            Some(kind),
            "{pascal} no se reconoce como etiqueta"
        );
    }
    // `RawText` es interno: lo crea `Renderer2.createText()` y no se escribe
    // en ninguna plantilla, así que no tiene etiqueta a propósito.
    assert_eq!(NodeKind::from_tag("RawText"), None);
    assert_eq!(NodeKind::from_tag("Div"), None);
}

/// Cazaría: una hoja que se mide pero a la que no se le marca la medición, o
/// al revés. Las dos listas tienen que hablar de los mismos nodos: un nodo
/// medible que no se marca sale de cero, y uno marcado que no se mide gasta
/// un contexto de medición para nada.
#[test]
fn las_hojas_medibles_son_las_que_se_marcan_al_nacer() {
    for kind in TODAS {
        if kind.is_control() {
            assert!(kind.is_measured_leaf(), "{kind:?} es control y tiene que medirse");
        }
    }
    // Las tres que no son controles y aun así se miden.
    for kind in [NodeKind::Text, NodeKind::Image, NodeKind::TextInput] {
        assert!(kind.is_measured_leaf(), "{kind:?}");
    }
    // Y un contenedor no se mide: lo dimensionan sus hijos.
    for kind in [NodeKind::View, NodeKind::ScrollView, NodeKind::StackView] {
        assert!(!kind.is_measured_leaf(), "{kind:?} no es una hoja");
    }
}

/// Cazaría: una prop de tipografía nueva que el layout lee para medir y que
/// nadie marca como sucia al cambiar.
///
/// `font_from_props` decide con qué letra se mide; `affects_measure` decide
/// cuándo hay que volver a medir. Si la segunda no conoce una prop de la
/// primera, cambiarla en caliente no remide: el texto se dibuja con la letra
/// nueva dentro de la caja de la vieja. Es la misma familia que `lineHeight`
/// y `letterSpacing`, que el núcleo medía y el host no dibujaba.
#[test]
fn toda_prop_que_cambia_la_letra_obliga_a_remedir() {
    let base = font_from_props(|_| None);
    let candidatas: [(&str, PropValue); 7] = [
        ("fontSize", PropValue::Number(33.0)),
        ("fontWeight", PropValue::Str("bold".into())),
        ("fontStyle", PropValue::Str("italic".into())),
        ("fontFamily", PropValue::Str("Menlo".into())),
        ("lineHeight", PropValue::Number(40.0)),
        ("letterSpacing", PropValue::Number(3.0)),
        ("numberOfLines", PropValue::Number(2.0)),
    ];
    for (key, value) in candidatas {
        let font = font_from_props(|k| (k == key).then(|| value.clone()));
        assert_ne!(font, base, "{key} tenía que cambiar la fuente de medición");
        assert!(
            affects_measure(key),
            "{key} cambia la letra con la que se mide y no marca el nodo para remedir"
        );
    }
}

/// Cazaría: `fontWeight` dejando de entender alguna de las tres formas en que
/// llega. Angular entrega lo que ponga la plantilla —`700`, `'700'`, `'bold'`—
/// y las tres tienen que pesar lo mismo, o el mismo texto se mide distinto
/// según cómo se escribiera el binding.
#[test]
fn el_peso_de_la_letra_se_entiende_escrito_de_las_tres_formas() {
    let peso = |value: PropValue| font_from_props(|k| (k == "fontWeight").then(|| value.clone())).weight;
    assert_eq!(peso(PropValue::Number(700.0)), 700);
    assert_eq!(peso(PropValue::Str("700".into())), 700);
    assert_eq!(peso(PropValue::Str("bold".into())), 700);
    assert_eq!(peso(PropValue::Str("normal".into())), 400);
    // Y algo que no significa nada no deja el peso en cero, que sería una
    // letra sin grosor: se vuelve al normal.
    assert_eq!(peso(PropValue::Str("gordísima".into())), 400);
}

/// Cazaría: `numberOfLines` colando un `max_lines` de cero.
///
/// Cero líneas no es "sin límite", es "ninguna": el texto se mediría a cero de
/// alto y desaparecería. `[numberOfLines]` sin poner llega como `0` desde más
/// de una plantilla.
#[test]
fn cero_lineas_no_es_un_limite_de_cero_lineas() {
    let sin_limite = font_from_props(|k| (k == "numberOfLines").then_some(PropValue::Number(0.0)));
    assert_eq!(sin_limite.max_lines, None);

    let una = font_from_props(|k| (k == "numberOfLines").then_some(PropValue::Number(1.0)));
    assert_eq!(una.max_lines, Some(1));
}

/// Cazaría: una prop que el núcleo consume y que dejara de marcar remedición.
/// El título de un botón o los ítems de una barra de pestañas cambian cuánto
/// ocupa el control, no solo lo que pone dentro.
#[test]
fn el_contenido_de_un_control_tambien_obliga_a_remedir() {
    for key in ["value", "placeholder", "title", "items", "buttons", "intrinsicWidth", "intrinsicHeight"] {
        assert!(affects_measure(key), "{key}");
    }
    // Y lo que solo pinta no cuesta una medición: repintar es barato, medir
    // el árbol entero no.
    for key in ["color", "backgroundColor", "borderRadius", "opacity", "translateX"] {
        assert!(!affects_measure(key), "{key} no cambia el tamaño de nada");
    }
}
