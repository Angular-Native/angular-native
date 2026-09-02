//! The layout tree: a mirror of the shadow tree, resolved by taffy.
//!
//! The keys are the ids JS assigns (monotonic, like Fabric's tags), not the
//! `taffy::NodeId`s. The mapping lives here and never leaves this crate.

use std::collections::HashMap;

use taffy::prelude::*;
use taffy::TaffyError;

use crate::measure::{MeasureCtx, TextMeasurer};
use crate::style::LayoutStyle;

/// A resolved rectangle, relative to the parent, in logical points.
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

    /// Attaches or removes the measuring context. A node with a context is a
    /// leaf: taffy does not go below it, children or no children.
    pub fn set_measure(&mut self, id: u32, ctx: Option<MeasureCtx>) -> Result<(), LayoutError> {
        let node = self.node(id)?;
        self.tree.set_node_context(node, ctx)?;
        Ok(())
    }

    /// Replaces the whole child list. The shadow tree already knows the final
    /// order, so telling insert from remove here is not worth the trouble.
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
        // In taffy 0.14 the measure function no longer takes the measurements
        // loose nor returns a size: it takes the whole layout input and returns
        // its output. `compute_leaf_layout` does the work in between —applying
        // styles, borders and padding— and only asks us for the size of the
        // content, which is the part we know.
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

    /// How much room the children take up, which may be more than the node
    /// itself. It is what a `UIScrollView` needs as its `contentSize`.
    pub fn content_size(&self, id: u32) -> Result<(f32, f32), LayoutError> {
        let node = self.node(id)?;
        let l = self.tree.layout(node)?;
        // In 0.14 this is the scrollable overflow rectangle, measured from the
        // scroll origin: its right and bottom edges are exactly how much room
        // the content takes, which is what a `UIScrollView` wants.
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
    // If layout already pinned both dimensions there is nothing to measure.
    if let (Some(width), Some(height)) = (known.width, known.height) {
        return taffy::Size { width, height };
    }
    let Some(ctx) = ctx else {
        return taffy::Size { width: known.width.unwrap_or(0.0), height: known.height.unwrap_or(0.0) };
    };

    match ctx {
        MeasureCtx::Text { text, font } => {
            // The intrinsic minimum gets a question of its own. Asking for it
            // as "available width zero" looks equivalent and is not: both
            // platforms answer zero to that, and then the text shrinks to
            // nothing as soon as nobody imposes a width on it.
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
