//! Props de host: todo lo que no es layout. Color, texto, fuente, `source`...
//! El core no las interpreta salvo las que afectan a la medición.

use an_layout::FontSpec;

/// Tipo de nodo. Se corresponde 1:1 con una primitiva nativa, salvo `RawText`,
/// que es interno y nunca llega a montarse como vista.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    View,
    Text,
    /// Nodo de texto crudo creado por `Renderer2.createText()`.
    RawText,
    Image,
    ScrollView,
    TextInput,
    /// Pila de pantallas. Es un contenedor normal de cara al layout —sus hijos
    /// se apilan y ocupan todo— pero el host lo trata distinto: anima la
    /// entrada y la salida, y engancha el gesto de volver atrás.
    StackView,
}

impl NodeKind {
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "View" | "view" => NodeKind::View,
            "Text" | "text" => NodeKind::Text,
            "Image" | "image" => NodeKind::Image,
            "ScrollView" | "scroll-view" => NodeKind::ScrollView,
            "TextInput" | "text-input" => NodeKind::TextInput,
            "StackView" | "stack-view" => NodeKind::StackView,
            _ => return None,
        })
    }

    /// `false` solo para nodos internos que no tienen vista nativa detrás.
    pub fn is_mountable(self) -> bool {
        self != NodeKind::RawText
    }

    /// Nodos hoja de cara al layout: su tamaño se mide, no se deriva de hijos.
    ///
    /// `TextInput` entra aquí para que un campo sin altura explícita ocupe lo
    /// que ocupa su texto, en vez de colapsar a cero.
    pub fn is_measured_leaf(self) -> bool {
        matches!(self, NodeKind::Text | NodeKind::Image | NodeKind::TextInput)
    }

    /// Nodos cuyo contenido puede desbordar y necesita `contentSize`.
    pub fn is_scrollable(self) -> bool {
        self == NodeKind::ScrollView
    }

    /// Contenedores cuyos hijos entran y salen con animación.
    pub fn is_stack(self) -> bool {
        self == NodeKind::StackView
    }
}

/// Valor de una prop de host. Deliberadamente pequeño: lo que cabe en el
/// protocolo binario del puente sin serializar objetos arbitrarios.
#[derive(Clone, PartialEq, Debug)]
pub enum PropValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    /// RGBA empaquetado, 8 bits por canal.
    Color(u32),
}

impl PropValue {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            PropValue::Number(n) => Some(*n as f32),
            PropValue::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Extrae la fuente con la que hay que medir un `<Text>` de sus props.
pub fn font_from_props(get: impl Fn(&str) -> Option<PropValue>) -> FontSpec {
    let mut font = FontSpec::default();
    if let Some(v) = get("fontSize").and_then(|v| v.as_f32()) {
        font.size = v;
    }
    if let Some(v) = get("fontWeight") {
        font.weight = match &v {
            PropValue::Number(n) => *n as u16,
            PropValue::Str(s) => match s.as_str() {
                "normal" => 400,
                "bold" => 700,
                other => other.parse().unwrap_or(400),
            },
            _ => 400,
        };
    }
    if let Some(v) = get("fontStyle").and_then(|v| v.as_str().map(str::to_owned)) {
        font.italic = v == "italic";
    }
    if let Some(v) = get("fontFamily").and_then(|v| v.as_str().map(str::to_owned)) {
        font.family = Some(v);
    }
    if let Some(v) = get("lineHeight").and_then(|v| v.as_f32()) {
        font.line_height = Some(v);
    }
    if let Some(v) = get("letterSpacing").and_then(|v| v.as_f32()) {
        font.letter_spacing = v;
    }
    if let Some(v) = get("numberOfLines").and_then(|v| v.as_f32()) {
        if v >= 1.0 {
            font.max_lines = Some(v as u32);
        }
    }
    font
}

/// Props que, al cambiar, obligan a volver a medir el nodo.
pub fn affects_measure(key: &str) -> bool {
    matches!(
        key,
        "fontSize"
            | "fontWeight"
            | "fontStyle"
            | "fontFamily"
            | "lineHeight"
            | "letterSpacing"
            | "numberOfLines"
            | "intrinsicWidth"
            | "intrinsicHeight"
            | "value"
            | "placeholder"
    )
}
