//! Host de watchOS: un `HostRenderer` que no tiene vistas.
//!
//! Es la diferencia de fondo con iOS y con Android, y por eso esto es un crate
//! aparte y no un `cfg` dentro de `an-ios`. En watchOS no hay jerarquía de
//! `UIView`: la interfaz es SwiftUI, y SwiftUI es declarativo. No se le puede
//! decir «crea esta vista, muévela aquí, cámbiale el color»; se le describe el
//! estado y él decide qué redibujar.
//!
//! Así que este host aplica las `MountOp` sobre un modelo en memoria —el mismo
//! árbol que mantiene el core, pero con las props ya resueltas— y marca que
//! cambió. El shell de Swift lo lee una vez por frame, lo vuelca en su modelo
//! observable, y SwiftUI se encarga del resto.
//!
//! El layout lo sigue calculando taffy en Rust. Cada nodo lleva su marco
//! *relativo al padre*, y el shell lo coloca con `.offset` dentro de un
//! `ZStack`. Los `VStack`/`HStack` de SwiftUI no se usan para posicionar: si se
//! usaran habría dos motores de layout decidiendo lo mismo y peleándose.

use std::collections::{HashMap, HashSet};

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{push_event, EventQueue, HostEvent, HostRenderer};

/// Un nodo del modelo que ve Swift. Es lo que el core sabe del nodo una vez
/// aplicadas todas las ops del frame: ni más ni menos.
#[derive(Clone, Debug, Default)]
pub struct WatchNode {
    pub kind: Option<NodeKind>,
    pub parent: Option<NodeId>,
    /// Solo hijos montables, en el orden en que hay que pintarlos.
    pub children: Vec<NodeId>,
    pub props: HashMap<String, PropValue>,
    /// Texto de un `RawText`. El `<Text>` que lo contiene lo concatena al
    /// serializar: SwiftUI quiere la cadena entera, no un nodo por trozo.
    pub text: Option<String>,
    pub frame: Rect,
    pub content_size: Option<(f32, f32)>,
    pub listeners: HashSet<String>,
}

pub struct WatchHost {
    nodes: HashMap<NodeId, WatchNode>,
    root: Option<NodeId>,
    events: EventQueue,
    /// Sube en cada frame que trajo algo. El shell solo vuelve a serializar
    /// —y SwiftUI solo redibuja— cuando este número cambia: un frame quieto no
    /// cuesta nada.
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

    /// Texto completo de un `<Text>`: la concatenación de sus hijos `RawText`.
    ///
    /// El core parte el texto en nodos porque `Renderer2.createText()` crea uno
    /// por trozo, y una interpolación de Angular son varios. SwiftUI quiere una
    /// cadena.
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

    /// Un nodo `RawText` no tiene vista: cuelga del `<Text>` que lo contiene.
    /// El árbol que ve Swift no lo lleva.
    fn is_mountable(&self, id: NodeId) -> bool {
        self.nodes.get(&id).and_then(|n| n.kind).is_some_and(NodeKind::is_mountable)
    }

    fn entry(&mut self, id: NodeId) -> &mut WatchNode {
        self.nodes.entry(id).or_default()
    }

    /// La llama el shell cuando ya se ha llevado la revisión actual.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Un toque en el reloj. Lo empuja el shell desde SwiftUI, que es quien
    /// tiene el gesto; llega a JS en el tick siguiente.
    pub fn dispatch(&self, target: NodeId, name: &str, payload: Vec<(String, PropValue)>) {
        // Un evento sobre un nodo que ya no existe, o que nadie escucha, no se
        // encola: cruzaría al motor para que este lo tirase.
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
        // El `index` del core cuenta solo hermanos montables, que es justo lo
        // que guarda `children`, así que entra tal cual.
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
            // Una prop a null es una prop que se quita, no una prop con valor
            // nulo: si se guardara, el shell la leería como «pon null aquí».
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

/// Recorre el árbol montable desde la raíz, en orden de pintado.
pub fn mountable_children<'a>(host: &'a WatchHost, id: NodeId) -> Vec<NodeId> {
    let Some(node) = host.node(id) else { return Vec::new() };
    node.children.iter().copied().filter(|c| host.is_mountable(*c)).collect()
}
