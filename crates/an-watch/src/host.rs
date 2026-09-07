//! The watchOS host: a `HostRenderer` with no views.
//!
//! That is the underlying difference from iOS and from Android, and it is why
//! this is a crate of its own rather than a `cfg` inside `an-ios`. On watchOS
//! there is no `UIView` hierarchy: the interface is SwiftUI, and SwiftUI is
//! declarative. You cannot tell it "create this view, move it here, change its
//! colour"; you describe the state to it and it decides what to redraw.
//!
//! So this host applies the `MountOp`s onto an in-memory model —the same tree
//! the core keeps, but with the props already resolved— and marks it as
//! changed. The Swift shell reads it once per frame, pours it into its
//! observable model, and SwiftUI takes care of the rest.
//!
//! The layout is still worked out by taffy in Rust. Every node carries its
//! frame *relative to its parent*, and the shell places it with `.offset`
//! inside a `ZStack`. SwiftUI's `VStack`/`HStack` are not used to position
//! anything: if they were there would be two layout engines deciding the same
//! thing and fighting over it.

use std::collections::{HashMap, HashSet};

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{push_event, EventQueue, HostEvent, HostRenderer};

/// A node of the model Swift sees. It is what the core knows about the node
/// once every op of the frame has been applied: no more and no less.
#[derive(Clone, Debug, Default)]
pub struct WatchNode {
    pub kind: Option<NodeKind>,
    pub parent: Option<NodeId>,
    /// Mountable children only, in the order they have to be painted in.
    pub children: Vec<NodeId>,
    pub props: HashMap<String, PropValue>,
    /// The text of a `RawText`. The `<Text>` containing it concatenates it when
    /// serialising: SwiftUI wants the whole string, not one node per piece.
    pub text: Option<String>,
    pub frame: Rect,
    pub content_size: Option<(f32, f32)>,
    /// Whether this node keeps its children inside its own frame. It travels in
    /// the snapshot and the SwiftUI side turns it into `.clipped()`.
    pub clip: bool,
    pub listeners: HashSet<String>,
}

pub struct WatchHost {
    nodes: HashMap<NodeId, WatchNode>,
    root: Option<NodeId>,
    events: EventQueue,
    /// Rises on every frame that brought something. The shell only serialises
    /// again —and SwiftUI only redraws— when this number changes: a frame in
    /// which nothing moved costs nothing.
    revision: u64,
    dirty: bool,
}

impl WatchHost {
    pub fn new(events: EventQueue) -> Self {
        WatchHost {
            nodes: HashMap::new(),
            root: None,
            events,
            revision: 0,
            dirty: false,
        }
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn node(&self, id: NodeId) -> Option<&WatchNode> {
        self.nodes.get(&id)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The full text of a `<Text>`: its `RawText` children concatenated.
    ///
    /// The core splits text into nodes because `Renderer2.createText()` makes
    /// one per piece, and an Angular interpolation is several of them. SwiftUI
    /// wants one string.
    pub fn text_of(&self, id: NodeId) -> String {
        let Some(node) = self.nodes.get(&id) else { return String::new() };
        if let Some(own) = &node.text {
            return own.clone();
        }
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        let Some(node) = self.nodes.get(&id) else { return };
        if let Some(text) = &node.text {
            out.push_str(text);
        }
        for child in &node.children {
            self.collect_text(*child, out);
        }
    }

    /// A `RawText` node has no view: it hangs off the `<Text>` that contains
    /// it. The tree Swift sees does not carry it.
    fn is_mountable(&self, id: NodeId) -> bool {
        self.nodes.get(&id).and_then(|n| n.kind).is_some_and(NodeKind::is_mountable)
    }

    fn entry(&mut self, id: NodeId) -> &mut WatchNode {
        self.nodes.entry(id).or_default()
    }

    /// Called by the shell once it has taken the current revision away.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// A tap on the watch. The shell pushes it in from SwiftUI, which is what
    /// holds the gesture; it reaches JS on the next tick.
    pub fn dispatch(&self, target: NodeId, name: &str, payload: Vec<(String, PropValue)>) {
        // An event on a node that no longer exists, or that nobody is
        // listening to, is not queued: it would cross into the engine only for
        // the engine to throw it away.
        let listening = self
            .nodes
            .get(&target)
            .is_some_and(|node| node.listeners.contains(name));
        if !listening {
            return;
        }
        push_event(&self.events, HostEvent { target, name: name.to_owned(), payload });
    }
}

impl HostRenderer for WatchHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        let node = self.entry(id);
        node.kind = Some(kind);
        self.dirty = true;
    }

    fn destroy(&mut self, id: NodeId) {
        self.nodes.remove(&id);
        if self.root == Some(id) {
            self.root = None;
        }
        self.dirty = true;
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        // The core's `index` counts mountable siblings only, which is exactly
        // what `children` holds, so it goes in as it is.
        let index = (index as usize).min(self.nodes.get(&parent).map_or(0, |n| n.children.len()));
        self.entry(parent).children.insert(index, child);
        self.entry(child).parent = Some(parent);
        self.dirty = true;
    }

    fn remove(&mut self, parent: NodeId, child: NodeId) {
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.retain(|c| *c != child);
        }
        if let Some(node) = self.nodes.get_mut(&child) {
            node.parent = None;
        }
        self.dirty = true;
    }

    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        match value {
            // A prop set to null is a prop being removed, not a prop whose
            // value is null: were it stored, the shell would read it as "put a
            // null here".
            PropValue::Null => {
                self.entry(id).props.remove(key);
            }
            _ => {
                self.entry(id).props.insert(key.to_owned(), value.clone());
            }
        }
        self.dirty = true;
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        self.entry(id).text = Some(text.to_owned());
        self.dirty = true;
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let node = self.entry(id);
        if enabled {
            node.listeners.insert(event.to_owned());
        } else {
            node.listeners.remove(event);
        }
        self.dirty = true;
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        self.entry(id).frame = frame;
        self.dirty = true;
    }

    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        self.entry(id).content_size = Some((width, height));
        self.dirty = true;
    }

    fn set_clip(&mut self, id: NodeId, clip: bool) {
        // This host mounts no views: the tree is mirrored into a model SwiftUI
        // redraws, so the flag travels in the snapshot and the Swift side turns
        // it into `.clipped()`. Storing it here is the whole of the work.
        self.entry(id).clip = clip;
        self.dirty = true;
    }

    fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
        self.dirty = true;
    }

    fn flush(&mut self) {
        if self.dirty {
            self.revision += 1;
        }
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.dirty = true;
        self.revision += 1;
    }
}

/// Walks the mountable tree from the root, in painting order.
pub fn mountable_children<'a>(host: &'a WatchHost, id: NodeId) -> Vec<NodeId> {
    let Some(node) = host.node(id) else { return Vec::new() };
    node.children.iter().copied().filter(|c| host.is_mountable(*c)).collect()
}
