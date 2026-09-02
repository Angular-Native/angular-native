//! El árbol de Rust, en la forma que SwiftUI sabe leer.
//!
//! Por qué una foto entera y no una op por llamada: cruzar la frontera una vez
//! por `MountOp` son cientos de cruces por frame, que es justo lo que el
//! protocolo binario del puente existe para evitar. Y por qué JSON y no el
//! protocolo binario: una pantalla de reloj son diez o quince nodos, `Codable`
//! lo decodifica sin escribir un parser, y el sitio donde esto dejaría de valer
//! —una lista larga— todavía no existe en watchOS. Cuando exista, lo que hay
//! que cambiar es este fichero y el `Decodable` de Swift, no el host.
//!
//! Los colores salen ya resueltos a canales 0..1. Swift no vuelve a analizar
//! `#0b1020`: si lo hiciera habría dos analizadores que mantener de acuerdo.
//!
//! Dos cosas no viajan dentro del árbol y salen aparte, en `overlays`: el
//! `Alert` y el `Modal`. En SwiftUI no son vistas que se coloquen, son
//! modificadores —`.alert`, `.sheet`, `.fullScreenCover`— que se cuelgan de la
//! raíz, y el sistema decide dónde van. Dejarlos dentro obligaría al shell a
//! buscarlos por el árbol en cada frame.

use std::cell::RefCell;
use std::collections::HashSet;

use an_core::{NodeId, NodeKind, PropValue};
use serde::Serialize;

use crate::host::WatchHost;

#[derive(Serialize)]
pub struct Snapshot {
    pub revision: u64,
    /// `None` mientras la app todavía no ha montado nada.
    pub root: Option<Node>,
    /// Lo que el sistema presenta encima: diálogos y hojas. Salen del árbol
    /// porque en SwiftUI no se colocan, se declaran sobre la raíz.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<Node>,
}

/// Un nodo tal y como lo pinta el shell. Todo lo que no aplica al tipo del
/// nodo se omite, para que el JSON de una pantalla quepa en un vistazo cuando
/// haya que depurarlo.
#[derive(Serialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: &'static str,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    /// Por qué este nodo no se pinta en el reloj. Lo rellena `unsupported()` y
    /// es lo que el shell enseña —y lo que el host dice por el registro— en vez
    /// de dejar un hueco que nadie sabe de dónde salió.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported: Option<&'static str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Solo viaja cuando la plantilla lo apaga: lo normal es que un control
    /// esté vivo, y mandarlo siempre engordaría cada nodo por nada.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_decoration: Option<String>,
    /// `numberOfLines`. Cero o ausente es «las que hagan falta».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u16>,

    // ------------------------------------------------------------- controles
    /// `Switch`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
    /// `Slider` y `Stepper`; en `DatePicker`, milisegundos desde 1970.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// `ProgressBar`, de 0 a 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f32>,
    /// `ActivityIndicator`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animating: Option<bool>,
    /// `TextInput`: el texto que manda la plantilla. No es el mismo campo que
    /// `text`, que es el rótulo de un `Text` o de un `Button`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub secure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<String>,
    /// `Picker`: los rótulos, ya deshechos del JSON con el que viajan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_index: Option<i64>,
    /// `DatePicker`: `date`, `time` o `dateAndTime`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_mode: Option<String>,
    /// `Icon`: nombre de SF Symbol, ya traducido de los nombres comunes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_weight: Option<u16>,
    /// `Image`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resize_mode: Option<String>,

    // --------------------------------------------------------- presentaciones
    /// `Alert` y `Modal`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buttons: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<String>,
    /// `StackView`: hacia dónde va la próxima transición. Lo decide quien
    /// navega, que es el único que sabe si se avanza o se retrocede.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,

    /// Solo en `ScrollView`, y solo cuando el contenido desborda.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_height: Option<f32>,

    /// Qué escucha la plantilla sobre este nodo.
    ///
    /// Va como lista y no como un booleano por gesto porque el shell solo
    /// pregunta si un nombre está: añadir `crown` fue añadir una cadena, no un
    /// campo en tres sitios. Y sin la lista el shell engancharía reconocedores
    /// que nadie escucha, que en el reloj se nota —el sistema realza lo que se
    /// puede tocar— y además se comería los gestos del `ScrollView` de debajo.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub listens: Vec<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

/// Nombre estable del tipo de nodo. No se usa `Debug`: el shell de Swift hace
/// `switch` sobre estas cadenas, y renombrar una variante en Rust no puede
/// romper el reloj en silencio.
fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::View => "View",
        NodeKind::Text => "Text",
        NodeKind::RawText => "RawText",
        NodeKind::ScrollView => "ScrollView",
        NodeKind::Button => "Button",
        NodeKind::Image => "Image",
        NodeKind::TextInput => "TextInput",
        NodeKind::StackView => "StackView",
        NodeKind::Switch => "Switch",
        NodeKind::Slider => "Slider",
        NodeKind::ActivityIndicator => "ActivityIndicator",
        NodeKind::ProgressBar => "ProgressBar",
        NodeKind::Stepper => "Stepper",
        NodeKind::Picker => "Picker",
        NodeKind::DatePicker => "DatePicker",
        NodeKind::Icon => "Icon",
        NodeKind::Alert => "Alert",
        NodeKind::Modal => "Modal",
        NodeKind::TabBar => "TabBar",
        NodeKind::NavigationBar => "NavigationBar",
        NodeKind::SegmentedControl => "SegmentedControl",
        NodeKind::SearchBar => "SearchBar",
        NodeKind::TextEditor => "TextEditor",
        NodeKind::WebView => "WebView",
        NodeKind::MapView => "MapView",
        NodeKind::VideoView => "VideoView",
    }
}

/// Lo que el reloj no puede dibujar, y por qué.
///
/// La razón no es una opinión: casi todas son lo que dice el SDK de watchOS, y
/// están escritas aquí para que el hueco se explique solo. Es la misma decisión
/// que tomó tvOS en `an-ios/src/family.rs`: una lista a mano, porque desde Rust
/// no se puede leer la anotación de disponibilidad de Swift.
pub fn unsupported(kind: NodeKind) -> Option<&'static str> {
    Some(match kind {
        NodeKind::TabBar => {
            "una barra de pestañas no cabe en 205 puntos: en el reloj las secciones \
             se pasan con el dedo a pantalla completa, que es un contenedor y no una \
             barra con marco"
        }
        NodeKind::NavigationBar => {
            "la franja de arriba del reloj ya es del sistema —la hora y el título de \
             la app—, y una barra propia se pintaría debajo o encima de ella"
        }
        NodeKind::SegmentedControl => {
            "SegmentedPickerStyle está marcado @available(watchOS, unavailable) en \
             SwiftUI; lo que el reloj usa en su lugar es an-select"
        }
        NodeKind::SearchBar => {
            "en el reloj buscar es una pantalla del sistema y no un campo con lupa: \
             .searchable existe, pero es un modificador de navegación, no una vista \
             con marco"
        }
        NodeKind::TextEditor => {
            "TextEditor está marcado @available(watchOS, unavailable) en SwiftUI; el \
             texto largo se dicta o se garabatea, y eso ya lo da an-text-input"
        }
        NodeKind::WebView => "WebKit no está en el SDK de watchOS",
        NodeKind::VideoView => {
            "AVKit en watchOS no trae AVPlayerViewController ni VideoPlayer: sus \
             cabeceras solo declaran tipos, ninguna vista de reproducción"
        }
        NodeKind::MapView => {
            "el Map de SwiftUI existe en watchOS, pero no acepta ni centro ni zoom \
             desde la app: enseñaría un sitio que la plantilla no eligió"
        }
        _ => return None,
    })
}

fn color_of(host: &WatchHost, id: NodeId, key: &str) -> Option<[f32; 4]> {
    let value = host.node(id)?.props.get(key)?;
    let (r, g, b, a) = match value {
        PropValue::Str(raw) => an_core::color::parse(raw)?,
        // RGBA empaquetado, 8 bits por canal, como lo manda el protocolo.
        PropValue::Color(packed) => {
            let byte = |shift: u32| ((packed >> shift) & 0xff) as f64 / 255.0;
            (byte(24), byte(16), byte(8), byte(0))
        }
        _ => return None,
    };
    Some([r as f32, g as f32, b as f32, a as f32])
}

fn number_of(host: &WatchHost, id: NodeId, key: &str) -> Option<f32> {
    host.node(id)?.props.get(key)?.as_f32()
}

fn f64_of(host: &WatchHost, id: NodeId, key: &str) -> Option<f64> {
    match host.node(id)?.props.get(key)? {
        PropValue::Number(n) => Some(*n),
        PropValue::Str(s) => s.parse().ok(),
        _ => None,
    }
}

fn string_of(host: &WatchHost, id: NodeId, key: &str) -> Option<String> {
    host.node(id)?.props.get(key)?.as_str().map(str::to_owned)
}

fn bool_of(host: &WatchHost, id: NodeId, key: &str) -> Option<bool> {
    match host.node(id)?.props.get(key)? {
        PropValue::Bool(b) => Some(*b),
        PropValue::Number(n) => Some(*n != 0.0),
        PropValue::Str(s) => match s.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Las listas de rótulos viajan como JSON porque el protocolo del puente no
/// lleva listas. Se deshacen aquí y no en Swift: si se deshicieran allí habría
/// dos sitios que tendrían que saber que esto era JSON.
fn strings_of(host: &WatchHost, id: NodeId, key: &str) -> Option<Vec<String>> {
    let raw = string_of(host, id, key)?;
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(items) => Some(items),
        Err(error) => {
            eprintln!("angular-native: [{key}] no es una lista de rótulos: {error}");
            None
        }
    }
}

pub fn snapshot(host: &WatchHost) -> Snapshot {
    let mut overlays = Vec::new();
    let root = host.root().and_then(|root| node(host, root, &mut overlays));
    Snapshot { revision: host.revision(), root, overlays }
}

fn node(host: &WatchHost, id: NodeId, overlays: &mut Vec<Node>) -> Option<Node> {
    let source = host.node(id)?;
    let kind = source.kind?;
    if !kind.is_mountable() {
        return None;
    }
    let frame = source.frame;
    let name = kind_name(kind);
    let unsupported = unsupported(kind);
    if let Some(reason) = unsupported {
        warn_once(name, "", &format!("{name} no se pinta en watchOS: {reason}"));
    }

    // El texto de un `<Text>` es la concatenación de sus hijos `RawText`, que
    // no bajan al shell: SwiftUI quiere la cadena entera. Un `Button` lleva su
    // rótulo en una prop, igual que en iOS.
    let text = match kind {
        NodeKind::Text => Some(host.text_of(id)),
        NodeKind::Button => string_of(host, id, "title"),
        _ => None,
    };

    let children = if kind == NodeKind::Text {
        // Los hijos de un `<Text>` ya se han fundido en `text`.
        Vec::new()
    } else {
        crate::host::mountable_children(host, id)
            .into_iter()
            .filter_map(|child| node(host, child, overlays))
            .collect()
    };

    let content = source.content_size.filter(|_| kind.is_scrollable());
    let mut listens: Vec<String> = source.listeners.iter().cloned().collect();
    // Orden estable: la foto entra en un test y en un `diff`, y un `HashSet` la
    // barajaría en cada frame.
    listens.sort();
    warn_unheard(name, &listens);

    let built = Node {
        id,
        kind: name,
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
        unsupported,
        background: color_of(host, id, "backgroundColor"),
        color: color_of(host, id, "color"),
        border_radius: number_of(host, id, "borderRadius"),
        opacity: number_of(host, id, "opacity"),
        // Un control apagado es la excepción, no la norma: solo viaja el `false`.
        disabled: bool_of(host, id, "enabled") == Some(false)
            || bool_of(host, id, "editable") == Some(false),
        test_id: string_of(host, id, "testID"),
        text,
        font_size: number_of(host, id, "fontSize"),
        font_weight: font_weight_of(host, id),
        italic: string_of(host, id, "fontStyle").as_deref() == Some("italic"),
        font_family: string_of(host, id, "fontFamily"),
        letter_spacing: number_of(host, id, "letterSpacing"),
        text_align: string_of(host, id, "textAlign"),
        text_decoration: string_of(host, id, "textDecoration").filter(|d| d != "none"),
        max_lines: number_of(host, id, "numberOfLines").map(|n| n as u16).filter(|n| *n > 0),
        on: bool_of(host, id, "on"),
        value: value_of(host, id, kind),
        minimum: f64_of(host, id, "minimumValue"),
        maximum: f64_of(host, id, "maximumValue"),
        step: f64_of(host, id, "stepValue").filter(|s| *s > 0.0),
        progress: number_of(host, id, "progress").map(|p| p.clamp(0.0, 1.0)),
        animating: bool_of(host, id, "animating"),
        field: string_of(host, id, "value").filter(|_| kind == NodeKind::TextInput),
        placeholder: string_of(host, id, "placeholder"),
        secure: bool_of(host, id, "secureTextEntry") == Some(true),
        keyboard: string_of(host, id, "keyboardType"),
        items: strings_of(host, id, "items"),
        selected_index: f64_of(host, id, "selectedIndex").map(|i| i as i64),
        date_mode: string_of(host, id, "mode"),
        symbol: string_of(host, id, "name").map(|raw| an_core::icons::translate(&raw).to_owned()),
        symbol_size: number_of(host, id, "iconSize"),
        symbol_weight: number_of(host, id, "iconWeight").map(|w| w as u16),
        source: string_of(host, id, "source"),
        resize_mode: string_of(host, id, "resizeMode"),
        visible: bool_of(host, id, "visible"),
        title: string_of(host, id, "title").filter(|_| kind == NodeKind::Alert),
        message: string_of(host, id, "message"),
        buttons: strings_of(host, id, "buttons"),
        presentation: string_of(host, id, "presentation"),
        transition: string_of(host, id, "transition"),
        content_width: content.map(|c| c.0),
        content_height: content.map(|c| c.1),
        listens,
        children,
    };
    warn_unread(host, id, kind);

    // `Alert` y `Modal` no se colocan: los presenta el sistema. Salen del árbol
    // y se cuelgan de la raíz, que es donde SwiftUI espera el modificador.
    if kind.is_overlay() {
        overlays.push(built);
        return None;
    }
    Some(built)
}

/// El valor con el que arranca un control numérico.
///
/// `DatePicker` lo manda en milisegundos desde 1970 —lo que da y toma `Date`—
/// y así viaja hasta Swift: convertirlo aquí obligaría a convertirlo de vuelta
/// al mandar el `change`.
fn value_of(host: &WatchHost, id: NodeId, kind: NodeKind) -> Option<f64> {
    match kind {
        NodeKind::Slider | NodeKind::Stepper | NodeKind::DatePicker => f64_of(host, id, "value"),
        _ => None,
    }
}

/// `fontWeight` llega como número (`700`) o como nombre (`bold`), igual que en
/// CSS. Se normaliza aquí para que el shell solo vea números.
fn font_weight_of(host: &WatchHost, id: NodeId) -> Option<u16> {
    let value = host.node(id)?.props.get("fontWeight")?;
    match value {
        PropValue::Number(n) => Some(*n as u16),
        PropValue::Str(s) => match s.as_str() {
            "normal" => Some(400),
            "bold" => Some(700),
            other => other.parse().ok(),
        },
        _ => None,
    }
}

/// Gestos que la plantilla pide y que en el reloj no llegan nunca.
///
/// Callarse aquí sería lo peor de los dos mundos: la plantilla escribe un
/// `(pinch)`, no falla nada, y el gesto sencillamente no responde jamás. La
/// razón viaja con el aviso porque casi siempre es del SDK, no una decisión de
/// este proyecto.
fn warn_unheard(kind: &'static str, listens: &[String]) {
    for event in listens {
        let Some(reason) = unheard(event) else { continue };
        warn_once(kind, event, &format!("({event}) no llega en watchOS: {reason}"));
    }
}

fn unheard(event: &str) -> Option<&'static str> {
    Some(match event {
        "pinch" => {
            "MagnifyGesture está marcado @available(watchOS, unavailable), y en una \
             pantalla de 40 mm no caben dos dedos"
        }
        "rotate" => "RotateGesture está marcado @available(watchOS, unavailable)",
        "back" => {
            "fuera de un NavigationStack el reloj no da el arrastre desde el borde, y \
             montar uno metería el layout de SwiftUI dentro del de taffy"
        }
        "refresh" => {
            "en el reloj no se tira de una lista para recargar: eso se hace con la \
             corona, que ya llega como (crown)"
        }
        "scroll" => {
            "el ScrollView de SwiftUI no publica su desplazamiento en watchOS 11, que \
             es el mínimo de este shell"
        }
        "safeArea" => {
            "la app del reloj ocupa la pantalla entera y el sistema no reserva \
             márgenes que se puedan preguntar"
        }
        "focus" | "blur" => {
            "todavía no: en el reloj el foco es el mismo que decide quién tiene la \
             corona, y darle dos dueños haría que la corona saltase de sitio al \
             escribir"
        }
        _ => return None,
    })
}

/// Props que llegan al reloj y que nadie mira.
///
/// Una prop que viaja, no la reconoce nadie y no da error es un fallo que se ve
/// meses después, cuando alguien se pregunta por qué su `[borderWidth]` no
/// pinta nada. Se dice una vez por tipo y clave —no una vez por frame, que a
/// 30 Hz sería un registro ilegible— y se dice desde aquí, que es el único
/// sitio que sabe qué acabó usándose de verdad.
fn warn_unread(host: &WatchHost, id: NodeId, kind: NodeKind) {
    let Some(node) = host.node(id) else { return };
    let name = kind_name(kind);
    for key in node.props.keys() {
        if reads(kind, key) {
            continue;
        }
        warn_once(name, key, &format!("<{name}> recibió [{key}], y el host de watchOS no la mira"));
    }
}

/// Si el reloj hace algo con esa prop sobre ese tipo de nodo.
fn reads(kind: NodeKind, key: &str) -> bool {
    // Comunes a todo lo que se pinta.
    if matches!(key, "backgroundColor" | "borderRadius" | "opacity" | "testID" | "enabled") {
        return true;
    }
    // Lo que solo mira el otro host nunca es un descuido: viene del `[ios]` o
    // del `[android]` de la plantilla, que ya dicen a quién van dirigidas.
    if key.starts_with("ios:") || key.starts_with("android:") {
        return true;
    }
    // La marca que Angular le pone a la raíz. No sale de ninguna plantilla y no
    // la mira ningún host, así que avisar de ella sería avisar en todas las
    // apps de algo que nadie escribió.
    if key == "ng-version" {
        return true;
    }
    match kind {
        NodeKind::Text => matches!(
            key,
            "color"
                | "fontSize"
                | "fontWeight"
                | "fontStyle"
                | "fontFamily"
                | "letterSpacing"
                | "textAlign"
                | "textDecoration"
                | "numberOfLines"
        ),
        NodeKind::Button => matches!(key, "title" | "color" | "fontSize" | "fontWeight"),
        NodeKind::Image => {
            matches!(key, "source" | "resizeMode" | "intrinsicWidth" | "intrinsicHeight")
        }
        NodeKind::Icon => matches!(key, "name" | "iconSize" | "iconWeight" | "color"),
        NodeKind::Switch => matches!(key, "on" | "color"),
        NodeKind::Slider => matches!(key, "value" | "minimumValue" | "maximumValue" | "color"),
        NodeKind::Stepper => matches!(key, "value" | "minimumValue" | "maximumValue" | "stepValue"),
        NodeKind::ProgressBar => matches!(key, "progress" | "color"),
        NodeKind::ActivityIndicator => matches!(key, "animating" | "color"),
        NodeKind::TextInput => matches!(
            key,
            "value"
                | "placeholder"
                | "secureTextEntry"
                | "editable"
                | "color"
                | "fontSize"
                | "fontWeight"
                | "textAlign"
                | "keyboardType"
        ),
        NodeKind::Picker => matches!(key, "items" | "selectedIndex"),
        NodeKind::DatePicker => matches!(key, "value" | "mode"),
        NodeKind::Alert => matches!(key, "visible" | "title" | "message" | "buttons" | "sheet"),
        NodeKind::Modal => matches!(key, "visible" | "presentation"),
        NodeKind::ScrollView => matches!(key, "scrollEnabled" | "showsScrollIndicator"),
        NodeKind::StackView => key == "transition",
        // Lo que el reloj no pinta ya se avisó entero por su tipo: repetir sus
        // props sería decir dos veces lo mismo.
        other => unsupported(other).is_some(),
    }
}

thread_local! {
    /// Lo ya dicho, para no repetirlo treinta veces por segundo. Es
    /// `thread_local` porque la foto se construye siempre en el hilo de UI: un
    /// `Mutex` aquí sería un candado que nadie disputa.
    static DICHO: RefCell<HashSet<(&'static str, String)>> = RefCell::new(HashSet::new());
}

fn warn_once(kind: &'static str, key: &str, message: &str) {
    DICHO.with(|dicho| {
        if dicho.borrow_mut().insert((kind, key.to_owned())) {
            eprintln!("angular-native: {message}");
        }
    });
}
