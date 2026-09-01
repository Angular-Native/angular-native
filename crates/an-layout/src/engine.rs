//! Árbol de layout: espejo del shadow tree, resuelto por taffy.
//!
//! Las claves son los ids que asigna JS (monotónicos, como los tags de Fabric),
//! no los `taffy::NodeId`. El mapeo vive aquí y no sale de este crate.

use std::collections::HashMap;

use taffy::prelude::*;
use taffy::TaffyError;

use crate::measure::{MeasureCtx, TextMeasurer};
use crate::style::LayoutStyle;

/// Rectángulo resuelto, relativo al padre, en puntos lógicos.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug)]
pub enum LayoutError {
    UnknownNode(u32),
    Taffy(TaffyError),
}

impl From<TaffyError> for LayoutError {
    fn from(e: TaffyError) -> Self {
        LayoutError::Taffy(e)
    }
}

pub struct LayoutEngine {
    tree: TaffyTree<MeasureCtx>,
    nodes: HashMap<u32, taffy::NodeId>,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        LayoutEngine { tree: TaffyTree::new(), nodes: HashMap::new() }
    }

    pub fn create(&mut self, id: u32, style: &LayoutStyle) -> Result<(), LayoutError> {
        let node = self.tree.new_leaf(style.0.clone())?;
        self.nodes.insert(id, node);
        Ok(())
    }

    pub fn destroy(&mut self, id: u32) -> Result<(), LayoutError> {
        if let Some(node) = self.nodes.remove(&id) {
            self.tree.remove(node)?;
        }
        Ok(())
    }

    pub fn set_style(&mut self, id: u32, style: &LayoutStyle) -> Result<(), LayoutError> {
        let node = self.node(id)?;
        self.tree.set_style(node, style.0.clone())?;
        Ok(())
    }

    /// Adjunta o quita el contexto de medición. Un nodo con contexto es hoja:
    /// taffy no baja de ahí aunque tenga hijos.
    pub fn set_measure(&mut self, id: u32, ctx: Option<MeasureCtx>) -> Result<(), LayoutError> {
        let node = self.node(id)?;
        self.tree.set_node_context(node, ctx)?;
        Ok(())
    }

    /// Reemplaza la lista completa de hijos. El shadow tree ya conoce el orden
    /// final, así que no merece la pena diferenciar insert/remove aquí.
    pub fn set_children(&mut self, id: u32, children: &[u32]) -> Result<(), LayoutError> {
        let parent = self.node(id)?;
        let mut mapped = Vec::with_capacity(children.len());
        for child in children {
            mapped.push(self.node(*child)?);
        }
        self.tree.set_children(parent, &mapped)?;
        Ok(())
    }

    pub fn mark_dirty(&mut self, id: u32) -> Result<(), LayoutError> {
        let node = self.node(id)?;
        self.tree.mark_dirty(node)?;
        Ok(())
    }

    pub fn compute(
        &mut self,
        root: u32,
        viewport: (f32, f32),
        measurer: &dyn TextMeasurer,
    ) -> Result<(), LayoutError> {
        let node = self.node(root)?;
        let available = taffy::Size {
            width: AvailableSpace::Definite(viewport.0),
            height: AvailableSpace::Definite(viewport.1),
        };
        // En taffy 0.14 la función de medida ya no recibe las medidas sueltas
        // ni devuelve un tamaño: recibe la entrada entera del layout y
        // devuelve su salida. `compute_leaf_layout` hace el trabajo de en
        // medio —aplicar estilos, bordes y relleno— y solo nos pide el tamaño
        // del contenido, que es lo que sabemos.
        self.tree.compute_layout_with_measure(node, available, |inputs, _node_id, ctx, style| {
            taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, available| {
                measure_leaf(known, available, ctx, measurer)
            })
        })?;
        Ok(())
    }

    pub fn layout(&self, id: u32) -> Result<Rect, LayoutError> {
        let node = self.node(id)?;
        let l = self.tree.layout(node)?;
        Ok(Rect { x: l.location.x, y: l.location.y, width: l.size.width, height: l.size.height })
    }

    /// Tamaño que ocupan los hijos, que puede pasarse del nodo. Es lo que un
    /// `UIScrollView` necesita como `contentSize`.
    pub fn content_size(&self, id: u32) -> Result<(f32, f32), LayoutError> {
        let node = self.node(id)?;
        let l = self.tree.layout(node)?;
        // En 0.14 esto es el rectángulo de desbordamiento desplazable, medido
        // desde el origen del scroll: su borde derecho e inferior son justo lo
        // que ocupa el contenido, que es lo que quiere un `UIScrollView`.
        Ok((l.scrollable_overflow_rect.right, l.scrollable_overflow_rect.bottom))
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn node(&self, id: u32) -> Result<taffy::NodeId, LayoutError> {
        self.nodes.get(&id).copied().ok_or(LayoutError::UnknownNode(id))
    }
}

fn measure_leaf(
    known: taffy::Size<Option<f32>>,
    available: taffy::Size<AvailableSpace>,
    ctx: Option<&mut MeasureCtx>,
    measurer: &dyn TextMeasurer,
) -> taffy::Size<f32> {
    // Si el layout ya fijó ambas dimensiones no hay nada que medir.
    if let (Some(width), Some(height)) = (known.width, known.height) {
        return taffy::Size { width, height };
    }
    let Some(ctx) = ctx else {
        return taffy::Size { width: known.width.unwrap_or(0.0), height: known.height.unwrap_or(0.0) };
    };

    match ctx {
        MeasureCtx::Text { text, font } => {
            // El mínimo intrínseco tiene su propia pregunta. Pedirlo como
            // "ancho disponible cero" parece equivalente y no lo es: las dos
            // plataformas contestan cero a eso, y entonces el texto se encoge
            // a nada en cuanto nadie le impone un ancho.
            let (w, h) = match (known.width, available.width) {
                (None, AvailableSpace::MinContent) => {
                    measurer.measure_text_min_content(text, font)
                }
                _ => {
                    let max_width = known.width.or(match available.width {
                        AvailableSpace::Definite(w) => Some(w),
                        AvailableSpace::MinContent => Some(0.0),
                        AvailableSpace::MaxContent => None,
                    });
                    measurer.measure_text(text, font, max_width)
                }
            };
            taffy::Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
        }
        MeasureCtx::Control { name } => {
            let available = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let (w, h) = measurer.measure_control(name, available);
            taffy::Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
        }
        MeasureCtx::Image { intrinsic } => {
            let (iw, ih) = *intrinsic;
            let ratio = if ih > 0.0 { iw / ih } else { 1.0 };
            match (known.width, known.height) {
                (Some(w), None) => taffy::Size { width: w, height: w / ratio },
                (None, Some(h)) => taffy::Size { width: h * ratio, height: h },
                _ => taffy::Size { width: iw, height: ih },
            }
        }
    }
}
