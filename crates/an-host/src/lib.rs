//! Frontera con la plataforma. Todo lo que iOS o Android tienen que aportar
//! está en dos traits: `HostRenderer` (montar vistas) y `TextMeasurer` (medir
//! texto con la tipografía real del sistema).
//!
//! El resto del núcleo no sabe que existen UIKit ni Android.

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
    fn set_root(&mut self, id: NodeId);
    /// Se llama una vez por frame, después de aplicar todas las ops.
    fn flush(&mut self) {}
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

/// Une shadow tree, medidor y host. Es lo que el shell de la plataforma
/// instancia y conserva mientras la app vive.
pub struct Renderer<H: HostRenderer, M: TextMeasurer> {
    pub tree: ShadowTree,
    host: H,
    measurer: M,
    viewport: (f32, f32),
    events: Vec<HostEvent>,
}

impl<H: HostRenderer, M: TextMeasurer> Renderer<H, M> {
    pub fn new(host: H, measurer: M, viewport: (f32, f32)) -> Self {
        Renderer { tree: ShadowTree::new(), host, measurer, viewport, events: Vec::new() }
    }

    /// Cambio de tamaño de pantalla o rotación: obliga a recalcular todo.
    pub fn set_viewport(&mut self, viewport: (f32, f32)) {
        if self.viewport != viewport {
            self.viewport = viewport;
            if let Some(root) = self.tree.root() {
                let _ = self.tree.set_style(root, "width", &format!("{}", viewport.0));
                let _ = self.tree.set_style(root, "height", &format!("{}", viewport.1));
            }
        }
    }

    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    /// Un frame completo: layout, diff y montaje. Devuelve el número de ops
    /// aplicadas, que es cero en la inmensa mayoría de frames.
    pub fn render_frame(&mut self) -> Result<usize, an_core::tree::Error> {
        let frame = self.tree.commit(self.viewport, &self.measurer)?;
        let count = frame.ops.len();
        if count > 0 {
            self.apply(&frame);
            self.host.flush();
        }
        Ok(count)
    }

    fn apply(&mut self, frame: &Frame) {
        for op in &frame.ops {
            match op {
                MountOp::Create { id, kind } => self.host.create(*id, *kind),
                MountOp::Destroy { id } => self.host.destroy(*id),
                MountOp::Insert { parent, child, index } => self.host.insert(*parent, *child, *index),
                MountOp::Remove { parent, child } => self.host.remove(*parent, *child),
                MountOp::SetProp { id, key, value } => self.host.set_prop(*id, key, value),
                MountOp::SetText { id, text } => self.host.set_text(*id, text),
                MountOp::SetListener { id, event, enabled } => {
                    self.host.set_listener(*id, event, *enabled)
                }
                MountOp::SetLayout { id, frame } => self.host.set_layout(*id, *frame),
                MountOp::SetRoot { id } => self.host.set_root(*id),
            }
        }
    }

    pub fn push_event(&mut self, event: HostEvent) {
        self.events.push(event);
    }

    /// La llama el runtime JS al empezar su tick.
    pub fn drain_events(&mut self) -> Vec<HostEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
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
    fn set_root(&mut self, id: NodeId) {
        self.log.push(format!("root {id}"));
    }
}
