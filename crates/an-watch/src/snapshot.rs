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

use an_core::{NodeId, NodeKind, PropValue};
use serde::Serialize;

use crate::host::WatchHost;

#[derive(Serialize)]
pub struct Snapshot {
    pub revision: u64,
    /// `None` mientras la app todavía no ha montado nada.
    pub root: Option<Node>,
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,

    /// `true` si la plantilla puso un `(press)`. Sin esto el shell envolvería
    /// en un botón de SwiftUI cosas que nadie escucha, y en el reloj eso se
    /// nota: el sistema les da realce al tocarlas.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pressable: bool,

    /// Solo en `ScrollView`, y solo cuando el contenido desborda.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_height: Option<f32>,

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
        NodeKind::ScrollView => "ScrollView",
        NodeKind::Button => "Button",
        NodeKind::Image => "Image",
        NodeKind::TextInput => "TextInput",
        NodeKind::StackView => "StackView",
        NodeKind::Switch => "Switch",
        NodeKind::Slider => "Slider",
        NodeKind::ActivityIndicator => "ActivityIndicator",
        NodeKind::ProgressBar => "ProgressBar",
        // Lo que el reloj todavía no sabe pintar viaja igualmente con su
        // nombre: el shell lo dibuja como una caja y así se ve dónde está, en
        // vez de desaparecer sin dejar rastro.
        _ => "Unsupported",
    }
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

fn string_of(host: &WatchHost, id: NodeId, key: &str) -> Option<String> {
    host.node(id)?.props.get(key)?.as_str().map(str::to_owned)
}

pub fn snapshot(host: &WatchHost) -> Snapshot {
    Snapshot {
        revision: host.revision(),
        root: host.root().and_then(|root| node(host, root)),
    }
}

fn node(host: &WatchHost, id: NodeId) -> Option<Node> {
    let source = host.node(id)?;
    let kind = source.kind?;
    if !kind.is_mountable() {
        return None;
    }
    let frame = source.frame;

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
            .filter_map(|child| node(host, child))
            .collect()
    };

    let content = source.content_size.filter(|_| kind.is_scrollable());

    Some(Node {
        id,
        kind: kind_name(kind),
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
        background: color_of(host, id, "backgroundColor"),
        color: color_of(host, id, "color"),
        border_radius: number_of(host, id, "borderRadius"),
        opacity: number_of(host, id, "opacity"),
        text,
        font_size: number_of(host, id, "fontSize"),
        font_weight: font_weight_of(host, id),
        text_align: string_of(host, id, "textAlign"),
        pressable: source.listeners.contains("press"),
        content_width: content.map(|c| c.0),
        content_height: content.map(|c| c.1),
        children,
    })
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
