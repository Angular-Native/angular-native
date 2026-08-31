//! Frontera con la plataforma. Todo lo que iOS o Android tienen que aportar
//! está en dos traits: `HostRenderer` (montar vistas) y `TextMeasurer` (medir
//! texto con la tipografía real del sistema).
//!
//! El resto del núcleo no sabe que existen UIKit ni Android.
//!
//! El trabajo está partido en dos mitades que pueden vivir en hilos distintos:
//!
//! - `ShadowSide` tiene el árbol, el layout y la medición. Produce `Frame`s.
//! - `MountSide` tiene las vistas nativas. Consume `Frame`s.
//!
//! Entre las dos solo viaja `Frame`, que es `Send`. Es la misma separación que
//! hace Fabric entre su hilo de sombra y el de UI, y es lo que permite meter el
//! motor JS en un hilo con pila grande sin sacar UIKit del principal.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use an_core::{Frame, MountOp, NodeId, NodeKind, PropValue, Rect, ShadowTree, TextMeasurer};

/// Lo implementa cada plataforma sobre sus vistas nativas.
///
/// Las llamadas siempre llegan en el hilo de UI y en el orden en que el core
/// las emitió: estructura, props, y layout al final.
pub trait HostRenderer {
    fn create(&mut self, id: NodeId, kind: NodeKind);
    fn destroy(&mut self, id: NodeId);
    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32);
    fn remove(&mut self, parent: NodeId, child: NodeId);
    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue);
    fn set_text(&mut self, id: NodeId, text: &str);
    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool);
    fn set_layout(&mut self, id: NodeId, frame: Rect);
    /// Solo llega para nodos scrollables, y solo cuando el contenido cambia
    /// de tamaño.
    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        let _ = (id, width, height);
    }
    fn set_root(&mut self, id: NodeId);
    /// Se llama una vez por frame, después de aplicar todas las ops.
    fn flush(&mut self) {}

    /// Desmonta todo. Solo la usa la recarga en caliente: el árbol de JS se va
    /// a reconstruir de cero y las vistas actuales ya no le corresponden.
    fn clear(&mut self) {}
}

/// Evento nativo de vuelta hacia JS. El host los encola; el runtime los drena
/// al principio del siguiente tick.
#[derive(Clone, Debug, PartialEq)]
pub struct HostEvent {
    pub target: NodeId,
    pub name: String,
    /// Pares clave/valor del evento (`x`, `y`, `text`...). Sin objetos anidados:
    /// lo que no quepa aquí es una llamada a un módulo nativo, no un evento.
    pub payload: Vec<(String, PropValue)>,
}

/// Cola de eventos nativos. La comparten el host, que empuja desde los
/// callbacks de la plataforma, y el runtime, que la vacía al empezar el frame.
///
/// Es `Arc<Mutex<..>>` porque las dos puntas pueden estar en hilos distintos:
/// los eventos nacen en el de UI y se consumen en el del motor.
pub type EventQueue = Arc<Mutex<Vec<HostEvent>>>;

pub fn new_event_queue() -> EventQueue {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn push_event(queue: &EventQueue, event: HostEvent) {
    queue.lock().expect("cola de eventos envenenada").push(event);
}

pub fn drain_events(queue: &EventQueue) -> Vec<HostEvent> {
    std::mem::take(&mut *queue.lock().expect("cola de eventos envenenada"))
}

/// La mitad que piensa: árbol, layout y medición. No sabe nada de vistas.
pub struct ShadowSide<M: TextMeasurer> {
    pub tree: ShadowTree,
    measurer: M,
    viewport: (f32, f32),
    /// Nodos suscritos a `layout`. Se lleva aquí y no en el host porque el
    /// marco lo calcula el core: así `onLayout` funciona igual en cualquier
    /// plataforma, sin que ninguna tenga que implementarlo.
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

    /// Cambio de tamaño de pantalla o rotación: obliga a recalcular todo.
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

    /// Tira el árbol. Tras esto la app tiene que volver a construirse desde
    /// JS; es lo que hace la recarga en caliente.
    pub fn reset(&mut self) {
        self.tree = ShadowTree::new();
        self.layout_listeners.clear();
    }

    /// Corre layout y diff. Devuelve las operaciones para el host y los
    /// eventos `layout` que el propio core produce.
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

/// La mitad que monta: vistas nativas y nada más. Vive en el hilo de UI.
pub struct MountSide<H: HostRenderer> {
    host: H,
}

impl<H: HostRenderer> MountSide<H> {
    pub fn new(host: H) -> Self {
        MountSide { host }
    }

    /// Aplica un frame. Devuelve cuántas operaciones tocó.
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
                // `layout` no es un evento de plataforma: lo emite el core.
                MountOp::SetListener { event, .. } if event == "layout" => {}
                MountOp::SetListener { id, event, enabled } => {
                    self.host.set_listener(*id, event, *enabled)
                }
                MountOp::SetLayout { id, frame } => self.host.set_layout(*id, *frame),
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

/// Las dos mitades juntas en un hilo. Es lo que usan los tests y el renderer
/// sin pantalla; las plataformas las separan.
pub struct Renderer<H: HostRenderer, M: TextMeasurer> {
    shadow: ShadowSide<M>,
    mount: MountSide<H>,
    events: EventQueue,
}

impl<H: HostRenderer, M: TextMeasurer> Renderer<H, M> {
    /// `events` tiene que ser la misma cola que se le dio al host, o los
    /// eventos nativos nunca llegarán a JS.
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
        self.events.lock().expect("cola envenenada").clear();
    }

    /// Un frame completo: layout, diff y montaje.
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

    /// La llama el runtime JS al empezar su tick.
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

/// Host de mentira que solo apunta lo que le mandan. Para tests del núcleo y
/// para depurar sin simulador.
#[derive(Default, Debug)]
pub struct RecordingHost {
    pub log: Vec<String>,
    pub frames: Vec<(NodeId, Rect)>,
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
    fn set_root(&mut self, id: NodeId) {
        self.log.push(format!("root {id}"));
    }
}
