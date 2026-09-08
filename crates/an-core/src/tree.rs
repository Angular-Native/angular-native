//! Shadow tree: the authoritative copy of the UI tree, the one that lives in
//! Rust.
//!
//! JS assigns the ids (monotonic) and sends mutations. The tree piles them up
//! without touching anything native. On `commit()` it runs layout and returns a
//! `Frame` with the smallest set of operations the host has to apply.

use an_layout::{
    LayoutEngine, LayoutStyle, MeasureCtx, Rect, StyleKey, StyleValue, TextMeasurer,
};

use crate::props::{affects_measure, font_from_props, NodeKind, PropValue};

/// Node id. The JS side assigns it, the same way Fabric does with tags, so
/// that creating a node needs no round trip to the core.
pub type NodeId = u32;

#[derive(Debug, PartialEq)]
pub enum Error {
    UnknownNode(NodeId),
    DuplicateNode(NodeId),
    /// An id so far ahead of what has been created that it cannot come from
    /// JS. See `MAX_ID_GAP`.
    IdOutOfRange(NodeId),
    /// Hanging a node off itself, or off one of its own descendants.
    Cycle { parent: NodeId, child: NodeId },
    /// Hanging off a new parent a node that still hangs off another one.
    /// Moving it means detaching it first; without that detach the tree keeps
    /// the child in two places and each host believes a different one.
    AlreadyAttached { parent: NodeId, child: NodeId, current: NodeId },
    NoRoot,
    Layout(String),
}

/// How far ahead of what has already been created an id is allowed to run.
///
/// The node table is indexed by id, so creating node 1,000 reserves a thousand
/// slots even if none of the earlier ones exist. With the ids JS hands out
/// —one at a time, starting at 1— that never happens; with a corrupted buffer
/// it does, and an id anywhere near `u32::MAX` asked for four billion slots:
/// the system killed the process for running out of memory, with no trace, no
/// error and no screen. A generous margin lets any real traffic through and
/// turns the other case into an error you can actually show someone.
const MAX_ID_GAP: usize = 1024;

/// An operation the host must apply to real native views.
#[derive(Clone, Debug, PartialEq)]
pub enum MountOp {
    Create { id: NodeId, kind: NodeKind },
    Destroy { id: NodeId },
    /// `index` counts mountable siblings only.
    Insert { parent: NodeId, child: NodeId, index: u32 },
    Remove { parent: NodeId, child: NodeId },
    SetProp { id: NodeId, key: String, value: PropValue },
    SetText { id: NodeId, text: String },
    SetListener { id: NodeId, event: String, enabled: bool },
    /// Frame relative to the parent, in logical points.
    SetLayout { id: NodeId, frame: Rect },
    /// Content size of a scrollable node, when it overflows its frame.
    SetContentSize { id: NodeId, width: f32, height: f32 },
    /// Whether this node keeps its children inside its own frame.
    ///
    /// Without this the host never learns that a node clips: `overflow` is
    /// resolved in taffy and stops there, so a view whose children stick out
    /// paints them over its neighbours and the app behaves like a canvas
    /// rather than a page. The root always clips — it is the body.
    SetClip { id: NodeId, clip: bool },
    SetRoot { id: NodeId },
}

/// The result of a commit. Empty = nothing to do, the host never wakes up.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub ops: Vec<MountOp>,
}

impl Frame {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

struct Node {
    kind: NodeKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    style: LayoutStyle,
    props: Vec<(Box<str>, PropValue)>,
    /// `RawText` only.
    text: String,
    frame: Rect,
    /// Scrollable nodes only.
    content: (f32, f32),
    /// What was last sent to the host. `None` until the first frame, so the
    /// initial value is always sent even when it is `false`.
    clip: Option<bool>,
    /// `false` until the first layout: forces an initial `SetLayout` even when
    /// the frame that came out is (0,0,0,0).
    laid_out: bool,
    /// Scrollable nodes only: which axis the content is allowed to overflow on.
    horizontal: bool,
    /// Whether the app itself set `flexDirection` on this node.
    ///
    /// `[horizontal]` turns a scroll view's children sideways, the way it does
    /// in React Native, because a horizontal scroll view whose children still
    /// stack downwards has nothing to scroll and says nothing about it. That is
    /// a default and not an override: a template that wrote the direction out
    /// keeps it, whichever of the two arrives last.
    styled_direction: bool,
    /// Whether the app itself set `flexBasis`, or the `flex` shorthand that
    /// writes it. The basis a scroll view gets by default is resolved before
    /// every layout, and it must not overwrite one the template asked for —
    /// that would be swapping one silent override for another.
    styled_basis: bool,
}

impl Node {
    fn new(kind: NodeKind) -> Self {
        Node {
            kind,
            parent: None,
            children: Vec::new(),
            style: LayoutStyle::default(),
            props: Vec::new(),
            text: String::new(),
            frame: Rect::default(),
            content: (0.0, 0.0),
            clip: None,
            laid_out: false,
            horizontal: false,
            styled_direction: false,
            styled_basis: false,
        }
    }

    fn prop(&self, key: &str) -> Option<PropValue> {
        self.props.iter().find(|(k, _)| &**k == key).map(|(_, v)| v.clone())
    }
}

pub struct ShadowTree {
    /// Indexed by id. `None` = the slot of a destroyed node.
    nodes: Vec<Option<Node>>,
    /// How many nodes have ever been created, alive or not. It is the yardstick
    /// an id is measured against to tell whether it comes too far ahead.
    created: usize,
    root: Option<NodeId>,
    layout: LayoutEngine,
    /// Structural and prop ops, in arrival order.
    pending: Vec<MountOp>,
    /// Parents whose child list has to be resynced with taffy.
    children_dirty: Vec<NodeId>,
    /// Leaf nodes that have to be measured again.
    measure_dirty: Vec<NodeId>,
    /// Every live scrollable node. Their `flex-basis` depends on the direction
    /// their parent lays out in, which nothing tells them when it changes, so
    /// it is resolved for all of them before each layout rather than tracked
    /// through every way a parent's style or a reparenting can move it.
    scrollables: Vec<NodeId>,
    needs_layout: bool,
}

impl Default for ShadowTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowTree {
    pub fn new() -> Self {
        ShadowTree {
            nodes: Vec::new(),
            created: 0,
            root: None,
            layout: LayoutEngine::new(),
            pending: Vec::new(),
            children_dirty: Vec::new(),
            measure_dirty: Vec::new(),
            scrollables: Vec::new(),
            needs_layout: false,
        }
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    // ----------------------------------------------------------------- mutations

    pub fn create_node(&mut self, id: NodeId, kind: NodeKind) -> Result<(), Error> {
        let idx = id as usize;
        if idx > self.created + MAX_ID_GAP {
            return Err(Error::IdOutOfRange(id));
        }
        if idx >= self.nodes.len() {
            self.nodes.resize_with(idx + 1, || None);
        }
        if self.nodes[idx].is_some() {
            return Err(Error::DuplicateNode(id));
        }
        self.created += 1;
        let mut node = Node::new(kind);
        // A stack behaves like a full-screen container: its children sit on top
        // of one another, not in a row.
        if kind.is_stack() {
            node.style.set(StyleKey::Position, StyleValue::Keyword(an_layout::Keyword::Relative));
            node.style.set(StyleKey::Overflow, StyleValue::Keyword(an_layout::Keyword::Hidden));
            node.style.set(StyleKey::FlexGrow, StyleValue::Number(1.0));
            node.style.set(StyleKey::FlexBasis, StyleValue::Points(0.0));
            node.style.set(StyleKey::MinHeight, StyleValue::Points(0.0));
        }
        // A dialog is presented by the system on top of everything: it takes no
        // part in layout, so it is pulled out of the flow and left with no size.
        if kind.is_dialog() {
            node.style.set(StyleKey::Position, StyleValue::Keyword(an_layout::Keyword::Absolute));
            node.style.set(StyleKey::Width, StyleValue::Points(0.0));
            node.style.set(StyleKey::Height, StyleValue::Points(0.0));
        }
        // A ScrollView is not sized by its content: that is what the scrolling
        // is for. Without these defaults, a list of five thousand rows gives a
        // ScrollView 280,000 points tall and the parent's layout blows up. It is
        // what React Native does too, where a ScrollView's children do not count
        // towards the size of the ScrollView itself.
        //
        // The `flex-basis` that says so is *not* set here: it is the main
        // axis's, so it depends on the parent, and a fixed zero was quietly
        // beating any `[style.height]` the app wrote. `flush_scroll_basis`
        // resolves it before every layout.
        if kind.is_scrollable() {
            node.style.set(StyleKey::Overflow, StyleValue::Keyword(an_layout::Keyword::Scroll));
            node.style.set(StyleKey::FlexShrink, StyleValue::Number(1.0));
            node.style.set(StyleKey::MinHeight, StyleValue::Points(0.0));
            node.style.set(StyleKey::MinWidth, StyleValue::Points(0.0));
            self.scrollables.push(id);
        }
        if kind.is_mountable() {
            self.layout
                .create(id, &node.style)
                .map_err(|e| Error::Layout(format!("{e:?}")))?;
            self.pending.push(MountOp::Create { id, kind });
        }
        self.nodes[idx] = Some(node);
        // A control has a size of its own from the moment it is born: its props
        // say nothing about size, so if it is not marked here it never gets
        // measured and comes out at zero.
        if kind.is_control() {
            self.mark_measure_dirty(id);
        }
        self.needs_layout = true;
        Ok(())
    }

    /// Destroys the node and its whole subtree. The host gets one `Destroy` per
    /// mountable node, children before parents.
    /// Destroying is idempotent: a node that is already gone is not an error.
    /// Angular may send the removal and the detach in either order, and a buffer
    /// that aborts halfway through leaves the screen broken.
    pub fn destroy_node(&mut self, id: NodeId) -> Result<(), Error> {
        if self.nodes.get(id as usize).and_then(Option::as_ref).is_none() {
            return Ok(());
        }
        if let Some(parent) = self.node(id)?.parent {
            self.remove_child(parent, id)?;
        }
        let mut stack = vec![id];
        let mut order = Vec::new();
        while let Some(current) = stack.pop() {
            order.push(current);
            if let Some(node) = self.nodes.get(current as usize).and_then(Option::as_ref) {
                stack.extend(node.children.iter().copied());
            }
        }
        for current in order.into_iter().rev() {
            let Some(node) = self.nodes[current as usize].take() else { continue };
            if node.kind.is_scrollable() {
                self.scrollables.retain(|scrollable| *scrollable != current);
            }
            if node.kind.is_mountable() {
                self.layout
                    .destroy(current)
                    .map_err(|e| Error::Layout(format!("{e:?}")))?;
                self.pending.push(MountOp::Destroy { id: current });
            }
        }
        if self.root == Some(id) {
            self.root = None;
        }
        self.needs_layout = true;
        Ok(())
    }

    pub fn set_root(&mut self, id: NodeId) -> Result<(), Error> {
        self.node(id)?;
        self.root = Some(id);
        self.pending.push(MountOp::SetRoot { id });
        self.needs_layout = true;
        Ok(())
    }

    /// Whether `node` is `possible_ancestor` itself, or hangs off it.
    ///
    /// It climbs through the parents instead of descending through the children
    /// because the chain upwards is the depth of the tree —a handful of hops—
    /// and the one downwards is the entire subtree.
    fn descends_from(&self, node: NodeId, possible_ancestor: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            if id == possible_ancestor {
                return true;
            }
            current = self.nodes.get(id as usize).and_then(Option::as_ref).and_then(|n| n.parent);
        }
        false
    }

    /// `index` is the position in the full list of children, non-mountable ones
    /// included. The conversion to the host's index happens here.
    pub fn insert_child(&mut self, parent: NodeId, child: NodeId, index: usize) -> Result<(), Error> {
        self.node(parent)?;
        self.node(child)?;
        // A tree with a cycle stops being a tree, and layout is the one that
        // pays for it: it walks children all the way down, and down here there
        // is no bottom, so it goes round and round and never hands back the
        // frame. No trace, no error: the app just sits there. Angular never
        // sends this; a buffer with one flipped bit does.
        if self.descends_from(parent, child) {
            return Err(Error::Cycle { parent, child });
        }
        // Hanging off a new parent something that already hangs off another one
        // is not moving it: the child stays in both lists and its `parent`
        // points only at the last one, so layout walks it twice and the frame
        // carries an `Insert` with no `Remove`. The three hosts react
        // differently —UIKit and AppKit move the view without a word,
        // `ViewGroup.addView` throws and leaves the subtree unmounted—, so the
        // same tree ends up looking like three different things. Moving is
        // detaching and attaching again, and that call is JS's, because JS is
        // the one who knows where it came from.
        if let Some(current) = self.node(child)?.parent {
            return Err(Error::AlreadyAttached { parent, child, current });
        }
        let index = index.min(self.node(parent)?.children.len());
        self.node_mut(parent)?.children.insert(index, child);
        self.node_mut(child)?.parent = Some(parent);

        // The children of a stack are screens: they overlap and they fill
        // everything. That is the definition of what a stack does, not a styling
        // preference, so the core imposes it and not each page.
        if self.node(parent)?.kind.is_stack() && self.node(child)?.kind.is_mountable() {
            for (key, value) in [
                (StyleKey::Position, StyleValue::Keyword(an_layout::Keyword::Absolute)),
                (StyleKey::Top, StyleValue::Points(0.0)),
                (StyleKey::Left, StyleValue::Points(0.0)),
                (StyleKey::Width, StyleValue::Percent(100.0)),
                (StyleKey::Height, StyleValue::Percent(100.0)),
            ] {
                let node = self.node_mut(child)?;
                if node.style.set(key, value) {
                    let style = node.style.clone();
                    self.layout
                        .set_style(child, &style)
                        .map_err(|e| Error::Layout(format!("{e:?}")))?;
                }
            }
        }

        let child_kind = self.node(child)?.kind;
        if child_kind.is_mountable() && self.node(parent)?.kind.is_mountable() {
            let host_index = self.host_index(parent, index)?;
            self.pending.push(MountOp::Insert { parent, child, index: host_index });
            self.mark_children_dirty(parent);
        }
        self.mark_text_host(parent);
        self.needs_layout = true;
        Ok(())
    }

    /// Same as `destroy_node`: taking away something that no longer hangs there
    /// is not an error, it is the very same end state.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), Error> {
        let Ok(node) = self.node(parent) else { return Ok(()) };
        let Some(position) = node.children.iter().position(|c| *c == child) else {
            return Ok(());
        };
        self.node_mut(parent)?.children.remove(position);
        self.node_mut(child)?.parent = None;

        if self.node(child)?.kind.is_mountable() && self.node(parent)?.kind.is_mountable() {
            self.pending.push(MountOp::Remove { parent, child });
            self.mark_children_dirty(parent);
        }
        self.mark_text_host(parent);
        self.needs_layout = true;
        Ok(())
    }

    /// `Renderer2.setStyle`. Name in camelCase or kebab-case; value already as
    /// text.
    pub fn set_style(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), Error> {
        let Some(key) = StyleKey::from_name(name) else {
            // Not layout: it travels as a host prop (color, backgroundColor...).
            //
            // And with the name in camel case, not as it arrived. Angular turns
            // style names into hyphenated ones, so `[style.fontSize]` reaches
            // here as `font-size`; forwarding that as-is to the host, which
            // looks for `fontSize`, was asking it for something it was never
            // going to recognise. Nothing failed: the text was simply measured
            // with one font and drawn with another.
            let camel = an_layout::camelize(name);
            return self.set_prop(id, &camel, PropValue::Str(value.to_owned()));
        };
        let parsed = StyleValue::parse(value);
        // The other half of the branch above, and the one that used to be
        // silent: this name *is* layout, so it stops here and no host is ever
        // told about it. For four of them that is not what the author meant.
        // Unsetting one is not asking for anything, so it says nothing.
        if parsed != StyleValue::Unset {
            warn_border_side(key);
        }
        let node = self.node_mut(id)?;
        // Written down before the "nothing changed" way out: a template that
        // asks for the direction it already has is still a template that asked,
        // and `[horizontal]` must not go on to overrule it.
        if key == StyleKey::FlexDirection && parsed != StyleValue::Unset {
            node.styled_direction = true;
        }
        if matches!(key, StyleKey::FlexBasis | StyleKey::Flex) && parsed != StyleValue::Unset {
            node.styled_basis = true;
        }
        if !node.style.set(key, parsed) {
            return Ok(());
        }
        let style = node.style.clone();
        let kind = node.kind;
        if kind.is_mountable() {
            self.layout
                .set_style(id, &style)
                .map_err(|e| Error::Layout(format!("{e:?}")))?;
            self.needs_layout = true;
        }
        Ok(())
    }

    pub fn remove_style(&mut self, id: NodeId, name: &str) -> Result<(), Error> {
        self.set_style(id, name, "")
    }

    /// `Renderer2.setProperty` / `setAttribute`.
    pub fn set_prop(&mut self, id: NodeId, key: &str, value: PropValue) -> Result<(), Error> {
        let node = self.node_mut(id)?;
        match node.props.iter_mut().find(|(k, _)| &**k == key) {
            Some((_, slot)) if *slot == value => return Ok(()),
            Some((_, slot)) => *slot = value.clone(),
            None => node.props.push((key.into(), value.clone())),
        }
        let kind = node.kind;
        let asked_sideways = matches!(value, PropValue::Bool(true));
        if kind.is_mountable() {
            self.pending.push(MountOp::SetProp { id, key: key.to_owned(), value });
        }
        if affects_measure(key) && kind.is_measured_leaf() {
            self.mark_measure_dirty(id);
        }
        // The one prop the core reads as well as forwards. Which axis a scroll
        // view may overflow on is a layout decision before it is a host one:
        // it is what says whether the content is allowed to come out wider than
        // the frame, and the clamp in `collect_layout` is the only thing
        // standing between a horizontal scroll view and a `contentSize` cut
        // back to the width of the screen.
        if kind.is_scrollable() && key == "horizontal" {
            self.set_horizontal(id, asked_sideways)?;
        }
        Ok(())
    }

    /// Turns a scroll view sideways: the axis the content may overflow on, and
    /// — unless the template said otherwise — the direction its children run in.
    fn set_horizontal(&mut self, id: NodeId, horizontal: bool) -> Result<(), Error> {
        let node = self.node_mut(id)?;
        if node.horizontal == horizontal {
            return Ok(());
        }
        node.horizontal = horizontal;
        // The content size is recomputed and re-sent from the layout pass, so
        // the axis change reaches the host by the same road as any other frame.
        self.needs_layout = true;
        if self.node(id)?.styled_direction {
            return Ok(());
        }
        let direction = if horizontal { an_layout::Keyword::Row } else { an_layout::Keyword::Column };
        let node = self.node_mut(id)?;
        if !node.style.set(StyleKey::FlexDirection, StyleValue::Keyword(direction)) {
            return Ok(());
        }
        let style = node.style.clone();
        self.layout.set_style(id, &style).map_err(|e| Error::Layout(format!("{e:?}")))?;
        Ok(())
    }

    /// `Renderer2.setValue` on a text node.
    pub fn set_text(&mut self, id: NodeId, text: &str) -> Result<(), Error> {
        let node = self.node_mut(id)?;
        if node.text == text {
            return Ok(());
        }
        node.text = text.to_owned();
        let parent = node.parent;
        if let Some(parent) = parent {
            self.mark_text_host(parent);
        }
        self.needs_layout = true;
        Ok(())
    }

    /// Dropping a listener from a node that no longer exists is not an error:
    /// it is what happens every single time a view with live subscriptions is
    /// destroyed, and the order the two things arrive in is not guaranteed.
    pub fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) -> Result<(), Error> {
        let Ok(node) = self.node(id) else { return Ok(()) };
        if !node.kind.is_mountable() {
            return Ok(());
        }
        self.pending.push(MountOp::SetListener { id, event: event.to_owned(), enabled });
        Ok(())
    }

    // ------------------------------------------------------------------- commit

    /// Runs layout and returns the operations for the host.
    /// It is the only moment at which the tree produces native work.
    pub fn commit(
        &mut self,
        viewport: (f32, f32),
        measurer: &dyn TextMeasurer,
    ) -> Result<Frame, Error> {
        let Some(root) = self.root else {
            // With no root, the only thing that makes sense is the ops already
            // piled up.
            return Ok(Frame { ops: std::mem::take(&mut self.pending) });
        };

        self.flush_children()?;
        self.flush_scroll_basis()?;
        self.flush_measures()?;

        let mut ops = std::mem::take(&mut self.pending);
        if self.needs_layout {
            self.layout
                .compute(root, viewport, measurer)
                .map_err(|e| Error::Layout(format!("{e:?}")))?;
            self.needs_layout = false;
            self.collect_layout(root, &mut ops)?;
        }
        Ok(Frame { ops })
    }

    /// Gives every scroll view the `flex-basis` that keeps it from being sized
    /// by its content without swallowing the size the app asked for.
    ///
    /// It runs over all of them and not only the dirty ones because what it
    /// depends on is the *parent's* `flexDirection`, and a node is told nothing
    /// when its parent restyles or when it is moved under another one. The list
    /// is one entry per `<an-scroll-view>` on screen — the windowed list keeps
    /// one and recycles its rows — so a walk per layout is cheaper than the
    /// bookkeeping that would keep it exact.
    fn flush_scroll_basis(&mut self) -> Result<(), Error> {
        // By index, so that the list is not copied on every settled frame: the
        // loop touches `nodes` and `layout` and never the list itself.
        for index in 0..self.scrollables.len() {
            let id = self.scrollables[index];
            let Some(node) = self.nodes.get(id as usize).and_then(Option::as_ref) else { continue };
            if node.styled_basis {
                continue;
            }
            let parent_row = node
                .parent
                .and_then(|parent| self.nodes.get(parent as usize).and_then(Option::as_ref))
                .is_some_and(|parent| parent.style.is_row());
            let node = self.nodes[id as usize].as_mut().expect("checked above");
            if !node.style.set_scroll_basis(parent_row) {
                continue;
            }
            let style = node.style.clone();
            self.layout.set_style(id, &style).map_err(|e| Error::Layout(format!("{e:?}")))?;
            self.needs_layout = true;
        }
        Ok(())
    }

    /// Syncs to taffy the child lists that changed, filtering out the nodes
    /// that take no part in layout.
    fn flush_children(&mut self) -> Result<(), Error> {
        let dirty = std::mem::take(&mut self.children_dirty);
        for parent in dirty {
            let Some(node) = self.nodes.get(parent as usize).and_then(Option::as_ref) else { continue };
            if !node.kind.is_mountable() {
                continue;
            }
            let children: Vec<NodeId> = node
                .children
                .iter()
                .copied()
                .filter(|c| {
                    self.nodes
                        .get(*c as usize)
                        .and_then(Option::as_ref)
                        .is_some_and(|n| n.kind.is_mountable())
                })
                .collect();
            self.layout
                .set_children(parent, &children)
                .map_err(|e| Error::Layout(format!("{e:?}")))?;
        }
        Ok(())
    }

    /// Rebuilds the measuring context of the dirty leaves and emits the
    /// matching `SetText` for the host.
    fn flush_measures(&mut self) -> Result<(), Error> {
        let dirty = std::mem::take(&mut self.measure_dirty);
        for id in dirty {
            let Some(node) = self.nodes.get(id as usize).and_then(Option::as_ref) else { continue };
            let ctx = match node.kind {
                NodeKind::Text => {
                    let text = self.collect_text(id);
                    let font = font_from_props(|k| node.prop(k));
                    self.pending.push(MountOp::SetText { id, text: text.clone() });
                    Some(MeasureCtx::Text { text, font })
                }
                NodeKind::TextInput => {
                    // An empty field still has to measure one line tall, so the
                    // placeholder is what gets measured when there is no value.
                    let text = node
                        .prop("value")
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .filter(|t| !t.is_empty())
                        .or_else(|| node.prop("placeholder").and_then(|v| v.as_str().map(str::to_owned)))
                        .unwrap_or_else(|| " ".to_owned());
                    let font = font_from_props(|k| node.prop(k));
                    Some(MeasureCtx::Text { text, font })
                }
                kind if kind.is_control() => {
                    Some(MeasureCtx::Control { name: kind.control_name().to_owned() })
                }
                NodeKind::Image => {
                    let w = node.prop("intrinsicWidth").and_then(|v| v.as_f32()).unwrap_or(0.0);
                    let h = node.prop("intrinsicHeight").and_then(|v| v.as_f32()).unwrap_or(0.0);
                    Some(MeasureCtx::Image { intrinsic: (w, h) })
                }
                _ => None,
            };
            if ctx.is_some() {
                self.layout
                    .set_measure(id, ctx)
                    .map_err(|e| Error::Layout(format!("{e:?}")))?;
                self.layout
                    .mark_dirty(id)
                    .map_err(|e| Error::Layout(format!("{e:?}")))?;
                self.needs_layout = true;
            }
        }
        Ok(())
    }

    /// Walks the tree and emits `SetLayout` only where the frame changed.
    fn collect_layout(&mut self, root: NodeId, ops: &mut Vec<MountOp>) -> Result<(), Error> {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id as usize).and_then(Option::as_ref) else { continue };
            if !node.kind.is_mountable() {
                continue;
            }
            let children = node.children.clone();
            let frame = self.layout.layout(id).map_err(|e| Error::Layout(format!("{e:?}")))?;
            let scrollable = node.kind.is_scrollable();
            let content = if scrollable {
                let (w, h) =
                    self.layout.content_size(id).map_err(|e| Error::Layout(format!("{e:?}")))?;
                // A scroll view overflows on one axis, the one it was asked
                // for, and is clamped to its own frame on the other.
                //
                // Reporting a bigger content on the cross axis does not add a
                // feature, it takes one away. A `UIScrollView` scrolls on
                // whichever axis its content is bigger, so a row that came out
                // a few points too wide — one image reporting its intrinsic
                // size, say — lets the whole page be dragged sideways into
                // nothing, and the app looks like it emptied itself. Along the
                // asked-for axis the overflow is the point; across it it is
                // always a mistake somewhere else, and it should show up as a
                // clipped edge rather than as a screen that can be swiped away.
                if node.horizontal {
                    (w, h.min(frame.height))
                } else {
                    (w.min(frame.width), h)
                }
            } else {
                (0.0, 0.0)
            };
            // The root is the app's body, so it always clips: whatever a
            // template does, nothing it contains may be painted outside the
            // window. Everywhere else it is the resolved `overflow` that says.
            let clip = id == root || self.nodes[id as usize]
                .as_ref()
                .expect("checked above")
                .style
                .clips();
            let node = self.nodes[id as usize].as_mut().expect("checked above");
            if !node.laid_out || node.frame != frame {
                node.frame = frame;
                node.laid_out = true;
                ops.push(MountOp::SetLayout { id, frame });
            }
            if node.clip != Some(clip) {
                node.clip = Some(clip);
                ops.push(MountOp::SetClip { id, clip });
            }
            if scrollable && node.content != content {
                node.content = content;
                ops.push(MountOp::SetContentSize { id, width: content.0, height: content.1 });
            }
            // In reverse order so that the `pop` walks it in preorder: the host
            // always gets parents before children.
            stack.extend(children.into_iter().rev());
        }
        Ok(())
    }

    // --------------------------------------------------------------------- helpers

    /// Index among mountable siblings, which is the one the host understands.
    fn host_index(&self, parent: NodeId, logical_index: usize) -> Result<u32, Error> {
        let children = &self.node(parent)?.children;
        let mut host = 0_u32;
        for child in children.iter().take(logical_index) {
            if self
                .nodes
                .get(*child as usize)
                .and_then(Option::as_ref)
                .is_some_and(|n| n.kind.is_mountable())
            {
                host += 1;
            }
        }
        Ok(host)
    }

    /// If the parent is a `<Text>`, any change to its raw children forces the
    /// text to be recomposed and measured again.
    fn mark_text_host(&mut self, parent: NodeId) {
        if self
            .nodes
            .get(parent as usize)
            .and_then(Option::as_ref)
            .is_some_and(|n| n.kind == NodeKind::Text)
        {
            self.mark_measure_dirty(parent);
        }
    }

    fn mark_measure_dirty(&mut self, id: NodeId) {
        if !self.measure_dirty.contains(&id) {
            self.measure_dirty.push(id);
        }
    }

    fn mark_children_dirty(&mut self, parent: NodeId) {
        if !self.children_dirty.contains(&parent) {
            self.children_dirty.push(parent);
        }
    }

    /// Concatenates the raw text of the subtree, in order.
    fn collect_text(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text_into(id, &mut out);
        out
    }

    fn collect_text_into(&self, id: NodeId, out: &mut String) {
        let Some(node) = self.nodes.get(id as usize).and_then(Option::as_ref) else { return };
        if node.kind == NodeKind::RawText {
            out.push_str(&node.text);
        }
        for child in &node.children {
            self.collect_text_into(*child, out);
        }
    }

    fn node(&self, id: NodeId) -> Result<&Node, Error> {
        self.nodes
            .get(id as usize)
            .and_then(Option::as_ref)
            .ok_or(Error::UnknownNode(id))
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, Error> {
        self.nodes
            .get_mut(id as usize)
            .and_then(Option::as_mut)
            .ok_or(Error::UnknownNode(id))
    }
}

/// The four per-side border widths, said once each.
///
/// A style the core recognises is layout and stops here: no host is ever told
/// about it. For these four that is not what the template meant, and until this
/// existed the only sign of it was the children moving.
///
/// It takes the resolved key and not the name written in the template, so that
/// both spellings —`borderTopWidth` and `border-top-width`, which Angular
/// hands over hyphenated— land on the same entry and say the same thing once.
///
/// Thread-local rather than global because the engine thread is the only one
/// that mutates the tree, and a lock for a set that ends up holding four
/// entries would be paying for contention that cannot happen.
fn warn_border_side(key: StyleKey) {
    use std::cell::RefCell;
    use std::collections::HashSet;

    thread_local! {
        static SAID: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }

    // The uniform `borderWidth` style is deliberately not here: written next to
    // the `[borderWidth]` prop it reserves the room the prop's line is painted
    // over, which is the one honest use of taffy's border rect. There is no
    // per-side line, so there is nothing for these four to reserve room for.
    let side = match key {
        StyleKey::BorderTopWidth => "borderTopWidth",
        StyleKey::BorderRightWidth => "borderRightWidth",
        StyleKey::BorderBottomWidth => "borderBottomWidth",
        StyleKey::BorderLeftWidth => "borderLeftWidth",
        _ => return,
    };
    SAID.with(|said| {
        if !said.borrow_mut().insert(side) {
            return;
        }
        eprintln!(
            "angular-native: [style.{side}] insets this node's children and draws nothing. \
             The border that is drawn is the [borderWidth] prop — one number, all four sides, \
             on every platform — and there is no per-side one to reserve room for. If the \
             inset is what was wanted, it is padding."
        );
    });
}
