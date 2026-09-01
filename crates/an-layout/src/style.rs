//! Props de estilo: nombres, valores y su proyección sobre `taffy::Style`.

use taffy::prelude::*;
use taffy::style::{BoxSizing, Overflow};

/// Prop de estilo soportada. Subconjunto flexbox al estilo React Native:
/// sin cascada, sin herencia, sin selectores.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StyleKey {
    Display,
    Position,
    Overflow,
    BoxSizing,

    FlexDirection,
    FlexWrap,
    JustifyContent,
    AlignItems,
    AlignSelf,
    AlignContent,
    /// El atajo: `flex: N` es crecer N, encoger 1 y partir de cero. Es lo
    /// que significa en CSS y en React Native, y es lo que casi todo el
    /// mundo escribe en vez de las tres por separado.
    Flex,
    FlexGrow,
    FlexShrink,
    FlexBasis,

    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    AspectRatio,

    Margin,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    MarginHorizontal,
    MarginVertical,

    Padding,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    PaddingHorizontal,
    PaddingVertical,

    BorderWidth,
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,

    Top,
    Right,
    Bottom,
    Left,

    Gap,
    RowGap,
    ColumnGap,
}

/// `flex-direction` a `flexDirection`.
///
/// Angular pasa los nombres de estilo a guiones antes de entregarlos, así que
/// todo lo que llega por esa vía viene así aunque en la plantilla se escriba
/// en camello.
pub fn camelize(name: &str) -> String {
    let mut camel = String::with_capacity(name.len());
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '-' || ch == '_' {
            upper_next = true;
        } else if upper_next {
            camel.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            camel.push(ch);
        }
    }
    camel
}

impl StyleKey {
    /// Acepta `flexDirection` y `flex-direction` indistintamente.
    /// Devuelve `None` para props que no afectan al layout (color, fondo...);
    /// esas viajan como props de host, no como estilo.
    pub fn from_name(name: &str) -> Option<Self> {
        let camel = camelize(name);
        use StyleKey::*;
        Some(match camel.as_str() {
            "display" => Display,
            "position" => Position,
            "overflow" => Overflow,
            "boxSizing" => BoxSizing,
            "flexDirection" => FlexDirection,
            "flexWrap" => FlexWrap,
            "justifyContent" => JustifyContent,
            "alignItems" => AlignItems,
            "alignSelf" => AlignSelf,
            "alignContent" => AlignContent,
            "flex" => Flex,
            "flexGrow" => FlexGrow,
            "flexShrink" => FlexShrink,
            "flexBasis" => FlexBasis,
            "width" => Width,
            "height" => Height,
            "minWidth" => MinWidth,
            "minHeight" => MinHeight,
            "maxWidth" => MaxWidth,
            "maxHeight" => MaxHeight,
            "aspectRatio" => AspectRatio,
            "margin" => Margin,
            "marginTop" => MarginTop,
            "marginRight" | "marginEnd" => MarginRight,
            "marginBottom" => MarginBottom,
            "marginLeft" | "marginStart" => MarginLeft,
            "marginHorizontal" => MarginHorizontal,
            "marginVertical" => MarginVertical,
            "padding" => Padding,
            "paddingTop" => PaddingTop,
            "paddingRight" | "paddingEnd" => PaddingRight,
            "paddingBottom" => PaddingBottom,
            "paddingLeft" | "paddingStart" => PaddingLeft,
            "paddingHorizontal" => PaddingHorizontal,
            "paddingVertical" => PaddingVertical,
            "borderWidth" => BorderWidth,
            "borderTopWidth" => BorderTopWidth,
            "borderRightWidth" => BorderRightWidth,
            "borderBottomWidth" => BorderBottomWidth,
            "borderLeftWidth" => BorderLeftWidth,
            "top" => Top,
            "right" => Right,
            "bottom" => Bottom,
            "left" => Left,
            "gap" => Gap,
            "rowGap" => RowGap,
            "columnGap" => ColumnGap,
            _ => return None,
        })
    }
}

/// Palabra clave de estilo ya resuelta. Se parsea una vez, al asignar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keyword {
    None,
    Flex,
    Relative,
    Absolute,
    Row,
    Column,
    RowReverse,
    ColumnReverse,
    Wrap,
    NoWrap,
    WrapReverse,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
    Baseline,
    Visible,
    Hidden,
    Scroll,
    BorderBox,
    ContentBox,
}

impl Keyword {
    fn from_str(raw: &str) -> Option<Self> {
        use Keyword::*;
        Some(match raw {
            "none" => None,
            "flex" => Flex,
            "relative" | "static" => Relative,
            "absolute" | "fixed" => Absolute,
            "row" => Row,
            "column" => Column,
            "row-reverse" | "rowReverse" => RowReverse,
            "column-reverse" | "columnReverse" => ColumnReverse,
            "wrap" => Wrap,
            "nowrap" | "no-wrap" => NoWrap,
            "wrap-reverse" | "wrapReverse" => WrapReverse,
            "flex-start" | "flexStart" | "start" => FlexStart,
            "flex-end" | "flexEnd" | "end" => FlexEnd,
            "center" => Center,
            "space-between" | "spaceBetween" => SpaceBetween,
            "space-around" | "spaceAround" => SpaceAround,
            "space-evenly" | "spaceEvenly" => SpaceEvenly,
            "stretch" | "auto" => Stretch,
            "baseline" => Baseline,
            "visible" => Visible,
            "hidden" => Hidden,
            "scroll" => Scroll,
            "border-box" | "borderBox" => BorderBox,
            "content-box" | "contentBox" => ContentBox,
            _ => return Option::None,
        })
    }
}

/// Valor de una prop de estilo.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StyleValue {
    /// Vuelve al valor por defecto de la prop.
    Unset,
    Auto,
    /// Puntos lógicos (no píxeles físicos).
    Points(f32),
    /// Porcentaje en 0..100.
    Percent(f32),
    /// Escalar sin unidad: `flexGrow`, `aspectRatio`.
    Number(f32),
    Keyword(Keyword),
}

impl StyleValue {
    /// Parsea lo que llega de JS ya normalizado a texto.
    /// `"12"`, `"12px"`, `"50%"`, `"auto"`, `"row"`.
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        if raw.is_empty() {
            return StyleValue::Unset;
        }
        if raw == "auto" {
            return StyleValue::Auto;
        }
        if let Some(num) = raw.strip_suffix('%') {
            if let Ok(v) = num.trim().parse::<f32>() {
                return StyleValue::Percent(v);
            }
        }
        let numeric = raw.strip_suffix("px").unwrap_or(raw).trim();
        if let Ok(v) = numeric.parse::<f32>() {
            return StyleValue::Points(v);
        }
        match Keyword::from_str(raw) {
            Some(k) => StyleValue::Keyword(k),
            None => StyleValue::Unset,
        }
    }

    fn dimension(self) -> Option<Dimension> {
        match self {
            StyleValue::Auto | StyleValue::Unset => Some(Dimension::auto()),
            StyleValue::Points(v) | StyleValue::Number(v) => Some(Dimension::length(v)),
            StyleValue::Percent(v) => Some(Dimension::percent(v / 100.0)),
            StyleValue::Keyword(_) => None,
        }
    }

    fn length_percentage(self) -> Option<LengthPercentage> {
        match self {
            StyleValue::Unset => Some(LengthPercentage::length(0.0)),
            StyleValue::Points(v) | StyleValue::Number(v) => Some(LengthPercentage::length(v)),
            StyleValue::Percent(v) => Some(LengthPercentage::percent(v / 100.0)),
            StyleValue::Auto | StyleValue::Keyword(_) => None,
        }
    }

    fn length_percentage_auto(self) -> Option<LengthPercentageAuto> {
        match self {
            StyleValue::Auto => Some(LengthPercentageAuto::auto()),
            StyleValue::Unset => Some(LengthPercentageAuto::length(0.0)),
            StyleValue::Points(v) | StyleValue::Number(v) => Some(LengthPercentageAuto::length(v)),
            StyleValue::Percent(v) => Some(LengthPercentageAuto::percent(v / 100.0)),
            StyleValue::Keyword(_) => None,
        }
    }

    fn number(self) -> Option<f32> {
        match self {
            StyleValue::Points(v) | StyleValue::Number(v) => Some(v),
            StyleValue::Percent(v) => Some(v / 100.0),
            _ => None,
        }
    }

    fn keyword(self) -> Option<Keyword> {
        match self {
            StyleValue::Keyword(k) => Some(k),
            _ => None,
        }
    }
}

/// `taffy::Style` con defaults propios (columna, no fila — como React Native)
/// y aplicación prop a prop.
#[derive(Clone, Debug)]
pub struct LayoutStyle(pub Style);

impl Default for LayoutStyle {
    fn default() -> Self {
        LayoutStyle(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..Style::default()
        })
    }
}

impl LayoutStyle {
    /// Aplica una prop. Devuelve `true` si el estilo cambió de verdad
    /// (el llamante usa esto para no marcar layout sucio de balde).
    pub fn set(&mut self, key: StyleKey, value: StyleValue) -> bool {
        let before = self.0.clone();
        self.apply(key, value);
        !styles_equal(&before, &self.0)
    }

    fn apply(&mut self, key: StyleKey, value: StyleValue) {
        use StyleKey as K;
        let s = &mut self.0;
        match key {
            K::Display => {
                s.display = match value.keyword() {
                    Some(Keyword::None) => Display::None,
                    _ => Display::Flex,
                }
            }
            K::Position => {
                s.position = match value.keyword() {
                    Some(Keyword::Absolute) => Position::Absolute,
                    _ => Position::Relative,
                }
            }
            K::Overflow => {
                let o = match value.keyword() {
                    Some(Keyword::Hidden) => Overflow::Hidden,
                    Some(Keyword::Scroll) => Overflow::Scroll,
                    _ => Overflow::Visible,
                };
                s.overflow = taffy::Point { x: o, y: o };
            }
            K::BoxSizing => {
                s.box_sizing = match value.keyword() {
                    Some(Keyword::ContentBox) => BoxSizing::ContentBox,
                    _ => BoxSizing::BorderBox,
                }
            }
            K::FlexDirection => {
                s.flex_direction = match value.keyword() {
                    Some(Keyword::Row) => FlexDirection::Row,
                    Some(Keyword::RowReverse) => FlexDirection::RowReverse,
                    Some(Keyword::ColumnReverse) => FlexDirection::ColumnReverse,
                    _ => FlexDirection::Column,
                }
            }
            K::FlexWrap => {
                s.flex_wrap = match value.keyword() {
                    Some(Keyword::Wrap) => FlexWrap::Wrap,
                    Some(Keyword::WrapReverse) => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                }
            }
            K::JustifyContent => s.justify_content = to_justify(value.keyword()),
            K::AlignItems => s.align_items = to_align(value.keyword()),
            K::AlignSelf => s.align_self = to_align(value.keyword()),
            K::AlignContent => s.align_content = to_align_content(value.keyword()),
            K::FlexGrow => s.flex_grow = value.number().unwrap_or(0.0),
            K::FlexShrink => s.flex_shrink = value.number().unwrap_or(0.0),
            K::FlexBasis => {
                if let Some(d) = value.dimension() {
                    s.flex_basis = d;
                }
            }
            K::Width => set_dim(&mut s.size.width, value),
            K::Height => set_dim(&mut s.size.height, value),
            K::MinWidth => set_dim(&mut s.min_size.width, value),
            K::MinHeight => set_dim(&mut s.min_size.height, value),
            K::MaxWidth => set_dim(&mut s.max_size.width, value),
            K::MaxHeight => set_dim(&mut s.max_size.height, value),
            K::AspectRatio => s.aspect_ratio = value.number(),
            K::Margin => set_rect_lpa(&mut s.margin, value, Edge::All),
            K::MarginTop => set_rect_lpa(&mut s.margin, value, Edge::Top),
            K::MarginRight => set_rect_lpa(&mut s.margin, value, Edge::Right),
            K::MarginBottom => set_rect_lpa(&mut s.margin, value, Edge::Bottom),
            K::MarginLeft => set_rect_lpa(&mut s.margin, value, Edge::Left),
            K::MarginHorizontal => set_rect_lpa(&mut s.margin, value, Edge::Horizontal),
            K::MarginVertical => set_rect_lpa(&mut s.margin, value, Edge::Vertical),
            K::Padding => set_rect_lp(&mut s.padding, value, Edge::All),
            K::PaddingTop => set_rect_lp(&mut s.padding, value, Edge::Top),
            K::PaddingRight => set_rect_lp(&mut s.padding, value, Edge::Right),
            K::PaddingBottom => set_rect_lp(&mut s.padding, value, Edge::Bottom),
            K::PaddingLeft => set_rect_lp(&mut s.padding, value, Edge::Left),
            K::PaddingHorizontal => set_rect_lp(&mut s.padding, value, Edge::Horizontal),
            K::PaddingVertical => set_rect_lp(&mut s.padding, value, Edge::Vertical),
            K::BorderWidth => set_rect_lp(&mut s.border, value, Edge::All),
            K::BorderTopWidth => set_rect_lp(&mut s.border, value, Edge::Top),
            K::BorderRightWidth => set_rect_lp(&mut s.border, value, Edge::Right),
            K::BorderBottomWidth => set_rect_lp(&mut s.border, value, Edge::Bottom),
            K::BorderLeftWidth => set_rect_lp(&mut s.border, value, Edge::Left),
            K::Top => set_rect_lpa(&mut s.inset, value, Edge::Top),
            K::Right => set_rect_lpa(&mut s.inset, value, Edge::Right),
            K::Bottom => set_rect_lpa(&mut s.inset, value, Edge::Bottom),
            K::Left => set_rect_lpa(&mut s.inset, value, Edge::Left),
            K::Flex => {
                let Some(grow) = value.number() else { return };
                s.flex_grow = grow;
                s.flex_shrink = 1.0;
                s.flex_basis = taffy::Dimension::length(0.0);
            }
            K::Gap => {
                if let Some(lp) = value.length_percentage() {
                    s.gap = taffy::Size { width: lp, height: lp };
                }
            }
            K::RowGap => {
                if let Some(lp) = value.length_percentage() {
                    s.gap.height = lp;
                }
            }
            K::ColumnGap => {
                if let Some(lp) = value.length_percentage() {
                    s.gap.width = lp;
                }
            }
        }
    }
}

enum Edge {
    All,
    Top,
    Right,
    Bottom,
    Left,
    Horizontal,
    Vertical,
}

fn set_dim(slot: &mut Dimension, value: StyleValue) {
    if let Some(d) = value.dimension() {
        *slot = d;
    }
}

fn set_rect_lp(rect: &mut taffy::Rect<LengthPercentage>, value: StyleValue, edge: Edge) {
    let Some(v) = value.length_percentage() else { return };
    match edge {
        Edge::All => *rect = taffy::Rect { left: v, right: v, top: v, bottom: v },
        Edge::Top => rect.top = v,
        Edge::Right => rect.right = v,
        Edge::Bottom => rect.bottom = v,
        Edge::Left => rect.left = v,
        Edge::Horizontal => {
            rect.left = v;
            rect.right = v;
        }
        Edge::Vertical => {
            rect.top = v;
            rect.bottom = v;
        }
    }
}

fn set_rect_lpa(rect: &mut taffy::Rect<LengthPercentageAuto>, value: StyleValue, edge: Edge) {
    let Some(v) = value.length_percentage_auto() else { return };
    match edge {
        Edge::All => *rect = taffy::Rect { left: v, right: v, top: v, bottom: v },
        Edge::Top => rect.top = v,
        Edge::Right => rect.right = v,
        Edge::Bottom => rect.bottom = v,
        Edge::Left => rect.left = v,
        Edge::Horizontal => {
            rect.left = v;
            rect.right = v;
        }
        Edge::Vertical => {
            rect.top = v;
            rect.bottom = v;
        }
    }
}

fn to_align(k: Option<Keyword>) -> Option<AlignItems> {
    match k? {
        Keyword::FlexStart => Some(AlignItems::FlexStart),
        Keyword::FlexEnd => Some(AlignItems::FlexEnd),
        Keyword::Center => Some(AlignItems::Center),
        Keyword::Baseline => Some(AlignItems::Baseline),
        Keyword::Stretch => Some(AlignItems::Stretch),
        _ => None,
    }
}

fn to_align_content(k: Option<Keyword>) -> Option<AlignContent> {
    match k? {
        Keyword::FlexStart => Some(AlignContent::FlexStart),
        Keyword::FlexEnd => Some(AlignContent::FlexEnd),
        Keyword::Center => Some(AlignContent::Center),
        Keyword::Stretch => Some(AlignContent::Stretch),
        Keyword::SpaceBetween => Some(AlignContent::SpaceBetween),
        Keyword::SpaceAround => Some(AlignContent::SpaceAround),
        Keyword::SpaceEvenly => Some(AlignContent::SpaceEvenly),
        _ => None,
    }
}

fn to_justify(k: Option<Keyword>) -> Option<JustifyContent> {
    to_align_content(k)
}

/// `taffy::Style` no implementa `PartialEq`, así que comparamos el subconjunto
/// que este renderer sabe escribir.
fn styles_equal(a: &Style, b: &Style) -> bool {
    a.display == b.display
        && a.position == b.position
        && a.overflow == b.overflow
        && a.box_sizing == b.box_sizing
        && a.flex_direction == b.flex_direction
        && a.flex_wrap == b.flex_wrap
        && a.justify_content == b.justify_content
        && a.align_items == b.align_items
        && a.align_self == b.align_self
        && a.align_content == b.align_content
        && a.flex_grow == b.flex_grow
        && a.flex_shrink == b.flex_shrink
        && a.flex_basis == b.flex_basis
        && a.size == b.size
        && a.min_size == b.min_size
        && a.max_size == b.max_size
        && a.aspect_ratio == b.aspect_ratio
        && a.margin == b.margin
        && a.padding == b.padding
        && a.border == b.border
        && a.inset == b.inset
        && a.gap == b.gap
}
