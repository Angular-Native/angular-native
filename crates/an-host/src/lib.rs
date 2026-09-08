//! The border with the platform. Everything iOS or Android have to bring is in
//! two traits: `HostRenderer` (mounting views) and `TextMeasurer` (measuring
//! text in the system's real typeface).
//!
//! The rest of the core does not know UIKit or Android exist.
//!
//! The work is split into two halves that can live on different threads:
//!
//! - `ShadowSide` owns the tree, the layout and the measuring. It makes `Frame`s.
//! - `MountSide` owns the native views. It consumes `Frame`s.
//!
//! The only thing that travels between them is `Frame`, which is `Send`. It is
//! the same split Fabric makes between its shadow thread and its UI thread, and
//! it is what lets the JS engine sit on a thread with a big stack without
//! dragging UIKit off the main one.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use an_core::{Frame, MountOp, NodeId, NodeKind, PropValue, Rect, ShadowTree, TextMeasurer};

/// Each platform implements it over its own native views.
///
/// The calls always arrive on the UI thread and in the order the core emitted
/// them: structure, props, and layout last.
pub trait HostRenderer {
    fn create(&mut self, id: NodeId, kind: NodeKind);
    fn destroy(&mut self, id: NodeId);
    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32);
    fn remove(&mut self, parent: NodeId, child: NodeId);
    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue);
    fn set_text(&mut self, id: NodeId, text: &str);
    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool);
    fn set_layout(&mut self, id: NodeId, frame: Rect);
    /// Only comes for scrollable nodes, and only when the content changes
    /// size.
    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        let _ = (id, width, height);
    }
    /// Whether this node keeps its children inside its own frame.
    ///
    /// It arrives once per node with the first layout and again whenever it
    /// changes. The root always arrives as `true`: it is the app's body, and a
    /// body that does not clip is a canvas — content dragged past its edge
    /// keeps existing off-screen, which is how a list can be scrolled until
    /// there is nothing left to look at.
    ///
    /// There is no default here on purpose. A host that ignores this paints
    /// children outside their parents and nothing says so, which is exactly the
    /// class of silent gap this project refuses everywhere else.
    fn set_clip(&mut self, id: NodeId, clip: bool);
    fn set_root(&mut self, id: NodeId);
    /// Called once per frame, after all the ops have been applied.
    fn flush(&mut self) {}

    /// Unmounts everything. Only hot reload uses it: the JS tree is about to be
    /// rebuilt from scratch and the views standing there no longer match it.
    fn clear(&mut self) {}
}

/// A native event on its way back to JS. The host queues them; the runtime
/// drains them at the start of the next tick.
#[derive(Clone, Debug, PartialEq)]
pub struct HostEvent {
    pub target: NodeId,
    pub name: String,
    /// The event's key/value pairs (`x`, `y`, `text`...). No nested objects:
    /// whatever does not fit here is a call to a native module, not an event.
    pub payload: Vec<(String, PropValue)>,
}

/// The queue of native events. It is shared by the host, which pushes from the
/// platform's callbacks, and the runtime, which empties it when the frame
/// starts.
///
/// It is an `Arc<Mutex<..>>` because the two ends can be on different threads:
/// events are born on the UI one and consumed on the engine's.
pub type EventQueue = Arc<Mutex<Vec<HostEvent>>>;

pub fn new_event_queue() -> EventQueue {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn push_event(queue: &EventQueue, event: HostEvent) {
    queue.lock().expect("the event queue is poisoned").push(event);
}

pub fn drain_events(queue: &EventQueue) -> Vec<HostEvent> {
    std::mem::take(&mut *queue.lock().expect("the event queue is poisoned"))
}

/// The half that thinks: tree, layout and measuring. It knows nothing about
/// views.
pub struct ShadowSide<M: TextMeasurer> {
    pub tree: ShadowTree,
    measurer: M,
    viewport: (f32, f32),
    /// Nodes subscribed to `layout`. It is kept here and not in the host
    /// because the core is what computes the frame: that way `onLayout` behaves
    /// the same on every platform, without any of them having to implement it.
    layout_listeners: HashSet<NodeId>,
}

impl<M: TextMeasurer> ShadowSide<M> {
    pub fn new(measurer: M, viewport: (f32, f32)) -> Self {
        ShadowSide {
            tree: ShadowTree::new(),
            measurer,
            viewport,
            layout_listeners: HashSet::new(),
        }
    }

    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    /// The screen resized, or rotated: everything has to be computed again.
    pub fn set_viewport(&mut self, viewport: (f32, f32)) {
        if self.viewport == viewport {
            return;
        }
        self.viewport = viewport;
        if let Some(root) = self.tree.root() {
            let _ = self.tree.set_style(root, "width", &format!("{}", viewport.0));
            let _ = self.tree.set_style(root, "height", &format!("{}", viewport.1));
        }
    }

    /// Throws the tree away. After this the app has to be built again from JS;
    /// it is what hot reload does.
    pub fn reset(&mut self) {
        self.tree = ShadowTree::new();
        self.layout_listeners.clear();
    }

    /// Runs layout and diff. Returns the operations for the host and the
    /// `layout` events the core itself produces.
    pub fn commit(&mut self) -> Result<(Frame, Vec<HostEvent>), an_core::tree::Error> {
        let frame = self.tree.commit(self.viewport, &self.measurer)?;

        let mut events = Vec::new();
        for op in &frame.ops {
            match op {
                MountOp::SetListener { id, event, enabled } if event == "layout" => {
                    if *enabled {
                        self.layout_listeners.insert(*id);
                    } else {
                        self.layout_listeners.remove(id);
                    }
                }
                MountOp::SetLayout { id, frame } if self.layout_listeners.contains(id) => {
                    events.push(HostEvent {
                        target: *id,
                        name: "layout".to_owned(),
                        payload: vec![
                            ("x".to_owned(), PropValue::Number(frame.x as f64)),
                            ("y".to_owned(), PropValue::Number(frame.y as f64)),
                            ("width".to_owned(), PropValue::Number(frame.width as f64)),
                            ("height".to_owned(), PropValue::Number(frame.height as f64)),
                        ],
                    });
                }
                _ => {}
            }
        }
        Ok((frame, events))
    }
}

/// The half that mounts: native views and nothing else. It lives on the UI
/// thread.
pub struct MountSide<H: HostRenderer> {
    host: H,
}

impl<H: HostRenderer> MountSide<H> {
    pub fn new(host: H) -> Self {
        MountSide { host }
    }

    /// Applies a frame. Returns how many operations it touched.
    pub fn apply(&mut self, frame: &Frame) -> usize {
        if frame.ops.is_empty() {
            return 0;
        }
        for op in &frame.ops {
            match op {
                MountOp::Create { id, kind } => self.host.create(*id, *kind),
                MountOp::Destroy { id } => self.host.destroy(*id),
                MountOp::Insert { parent, child, index } => self.host.insert(*parent, *child, *index),
                MountOp::Remove { parent, child } => self.host.remove(*parent, *child),
                MountOp::SetProp { id, key, value } => self.host.set_prop(*id, key, value),
                MountOp::SetText { id, text } => self.host.set_text(*id, text),
                // `layout` is not a platform event: the core emits it.
                MountOp::SetListener { event, .. } if event == "layout" => {}
                MountOp::SetListener { id, event, enabled } => {
                    self.host.set_listener(*id, event, *enabled)
                }
                MountOp::SetLayout { id, frame } => self.host.set_layout(*id, *frame),
                MountOp::SetClip { id, clip } => self.host.set_clip(*id, *clip),
                MountOp::SetContentSize { id, width, height } => {
                    self.host.set_content_size(*id, *width, *height)
                }
                MountOp::SetRoot { id } => self.host.set_root(*id),
            }
        }
        self.host.flush();
        frame.ops.len()
    }

    pub fn clear(&mut self) {
        self.host.clear();
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }
}

/// Both halves together on one thread. It is what the tests and the screenless
/// renderer use; the platforms pull them apart.
pub struct Renderer<H: HostRenderer, M: TextMeasurer> {
    shadow: ShadowSide<M>,
    mount: MountSide<H>,
    events: EventQueue,
}

impl<H: HostRenderer, M: TextMeasurer> Renderer<H, M> {
    /// `events` has to be the very queue the host was given, or native events
    /// will never reach JS.
    pub fn new(host: H, measurer: M, viewport: (f32, f32), events: EventQueue) -> Self {
        Renderer {
            shadow: ShadowSide::new(measurer, viewport),
            mount: MountSide::new(host),
            events,
        }
    }

    pub fn set_viewport(&mut self, viewport: (f32, f32)) {
        self.shadow.set_viewport(viewport);
    }

    pub fn viewport(&self) -> (f32, f32) {
        self.shadow.viewport()
    }

    pub fn reset(&mut self) {
        self.mount.clear();
        self.shadow.reset();
        self.events.lock().expect("the queue is poisoned").clear();
    }

    /// A whole frame: layout, diff and mounting.
    pub fn render_frame(&mut self) -> Result<usize, an_core::tree::Error> {
        let (frame, layout_events) = self.shadow.commit()?;
        let applied = self.mount.apply(&frame);
        for event in layout_events {
            push_event(&self.events, event);
        }
        Ok(applied)
    }

    pub fn push_event(&mut self, event: HostEvent) {
        push_event(&self.events, event);
    }

    /// The JS runtime calls it when its tick starts.
    pub fn drain_events(&mut self) -> Vec<HostEvent> {
        drain_events(&self.events)
    }

    pub fn host(&self) -> &H {
        self.mount.host()
    }

    pub fn host_mut(&mut self) -> &mut H {
        self.mount.host_mut()
    }
}

impl<H: HostRenderer, M: TextMeasurer> std::ops::Deref for Renderer<H, M> {
    type Target = ShadowTree;

    fn deref(&self) -> &ShadowTree {
        &self.shadow.tree
    }
}

impl<H: HostRenderer, M: TextMeasurer> std::ops::DerefMut for Renderer<H, M> {
    fn deref_mut(&mut self) -> &mut ShadowTree {
        &mut self.shadow.tree
    }
}

/// A pretend host that only writes down what it is told. For core tests and for
/// debugging with no simulator.
#[derive(Default, Debug)]
pub struct RecordingHost {
    pub log: Vec<String>,
    pub frames: Vec<(NodeId, Rect)>,
    /// Counted apart from `log` because the dumps the check scripts grep are
    /// built out of `log`, and a host that starts announcing its own frame
    /// boundaries there would change every one of them.
    pub flushes: usize,
}

impl HostRenderer for RecordingHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        self.log.push(format!("create {id} {kind:?}"));
    }
    fn destroy(&mut self, id: NodeId) {
        self.log.push(format!("destroy {id}"));
    }
    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        self.log.push(format!("insert {child} into {parent} at {index}"));
    }
    fn remove(&mut self, parent: NodeId, child: NodeId) {
        self.log.push(format!("remove {child} from {parent}"));
    }
    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        self.log.push(format!("prop {id} {key}={value:?}"));
    }
    fn set_text(&mut self, id: NodeId, text: &str) {
        self.log.push(format!("text {id} {text:?}"));
    }
    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        self.log.push(format!("listener {id} {event} {enabled}"));
    }
    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        self.log.push(format!("layout {id} {frame:?}"));
        self.frames.push((id, frame));
    }
    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        self.log.push(format!("content {id} {width}x{height}"));
    }
    fn set_clip(&mut self, id: NodeId, clip: bool) {
        self.log.push(format!("clip {id} {clip}"));
    }
    fn set_root(&mut self, id: NodeId) {
        self.log.push(format!("root {id}"));
    }
    fn flush(&mut self) {
        self.flushes += 1;
    }
}
