//! Layout: traduce props de estilo a `taffy::Style`, mantiene el árbol de layout
//! espejo del shadow tree y resuelve la medición de nodos hoja (texto, imagen).

pub mod engine;
pub mod measure;
pub mod style;

pub use engine::{LayoutEngine, Rect};
pub use measure::{FontSpec, MeasureCtx, NaiveMeasurer, TextMeasurer};
pub use style::{Keyword, LayoutStyle, StyleKey, StyleValue};
