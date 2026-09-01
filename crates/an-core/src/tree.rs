//! Shadow tree: la copia autoritativa del árbol de UI que vive en Rust.
//!
//! JS asigna los ids (monotónicos) y manda mutaciones. El árbol las acumula sin
//! tocar nada nativo. En `commit()` corre el layout y devuelve un `Frame` con
//! las operaciones mínimas que el host tiene que aplicar.

use an_layout::{
    LayoutEngine, LayoutStyle, MeasureCtx, Rect, StyleKey, StyleValue, TextMeasurer,
};

use crate::props::{affects_measure, font_from_props, NodeKind, PropValue};

/// Id de nodo. Lo asigna el lado JS, igual que los tags de Fabric, para que
/// crear un nodo no necesite viaje de ida y vuelta al core.
pub type NodeId = u32;

#[derive(Debug, PartialEq)]
pub enum Error {
    UnknownNode(NodeId),
    DuplicateNode(NodeId),
    NoRoot,
    Layout(String),
}

/// Operación que el host debe aplicar sobre vistas nativas reales.
#[derive(Clone, Debug, PartialEq)]
pub enum MountOp {
    Create { id: NodeId, kind: NodeKind },
    Destroy { id: NodeId },
    /// `index` cuenta solo hermanos montables.
    Insert { parent: NodeId, child: NodeId, index: u32 },
    Remove { parent: NodeId, child: NodeId },
    SetProp { id: NodeId, key: String, value: PropValue },
    SetText { id: NodeId, text: String },
    SetListener { id: NodeId, event: String, enabled: bool },
    /// Marco relativo al padre, en puntos lógicos.
    SetLayout { id: NodeId, frame: Rect },
    /// Tamaño del contenido de un nodo scrollable, cuando desborda su marco.
    SetContentSize { id: NodeId, width: f32, height: f32 },
    SetRoot { id: NodeId },
}

/// Resultado de un commit. Vacío = nada que hacer, el host no despierta.
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
    /// Solo para `RawText`.
    text: String,
    frame: Rect,
    /// Solo para nodos scrollables.
    content: (f32, f32),
    /// `false` hasta el primer layout: fuerza un `SetLayout` inicial aunque
    /// el marco calculado sea (0,0,0,0).
    laid_out: bool,
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
            laid_out: false,
        }
    }

    fn prop(&self, key: &str) -> Option<PropValue> {
        self.props.iter().find(|(k, _)| &**k == key).map(|(_, v)| v.clone())
    }
}

pub struct ShadowTree {
    /// Indexado por id. `None` = hueco de un nodo destruido.
    nodes: Vec<Option<Node>>,
    root: Option<NodeId>,
    layout: LayoutEngine,
    /// Ops estructurales y de props, en orden de llegada.
    pending: Vec<MountOp>,
    /// Padres cuya lista de hijos hay que resincronizar con taffy.
    children_dirty: Vec<NodeId>,
    /// Nodos hoja que hay que volver a medir.
    measure_dirty: Vec<NodeId>,
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
            root: None,
            layout: LayoutEngine::new(),
            pending: Vec::new(),
            children_dirty: Vec::new(),
            measure_dirty: Vec::new(),
            needs_layout: false,
        }
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    // ---------------------------------------------------------------- mutaciones

    pub fn create_node(&mut self, id: NodeId, kind: NodeKind) -> Result<(), Error> {
        let idx = id as usize;
        if idx >= self.nodes.len() {
            self.nodes.resize_with(idx + 1, || None);
        }
        if self.nodes[idx].is_some() {
            return Err(Error::DuplicateNode(id));
        }
        let mut node = Node::new(kind);
        // Un ScrollView no se dimensiona por su contenido: para eso está el
        // scroll. Sin estos defaults, una lista de cinco mil filas produce un
        // ScrollView de 280.000 puntos de alto y el layout del padre revienta.
        // Es lo mismo que hace React Native, donde los hijos de un ScrollView
        // no cuentan para el tamaño del propio ScrollView.
        // Una pila se comporta como un contenedor a pantalla completa: sus
        // hijos van uno encima de otro, no en fila.
        if kind.is_stack() {
            node.style.set(StyleKey::Position, StyleValue::Keyword(an_layout::Keyword::Relative));
            node.style.set(StyleKey::Overflow, StyleValue::Keyword(an_layout::Keyword::Hidden));
            node.style.set(StyleKey::FlexGrow, StyleValue::Number(1.0));
            node.style.set(StyleKey::FlexBasis, StyleValue::Points(0.0));
            node.style.set(StyleKey::MinHeight, StyleValue::Points(0.0));
        }
        // Un diálogo lo presenta el sistema encima de todo: no participa en
        // el layout, así que se le quita del flujo y se le deja sin tamaño.
        if kind.is_dialog() {
            node.style.set(StyleKey::Position, StyleValue::Keyword(an_layout::Keyword::Absolute));
            node.style.set(StyleKey::Width, StyleValue::Points(0.0));
            node.style.set(StyleKey::Height, StyleValue::Points(0.0));
        }
        if kind.is_scrollable() {
            node.style.set(StyleKey::Overflow, StyleValue::Keyword(an_layout::Keyword::Scroll));
            node.style.set(StyleKey::FlexBasis, StyleValue::Points(0.0));
            node.style.set(StyleKey::FlexShrink, StyleValue::Number(1.0));
            node.style.set(StyleKey::MinHeight, StyleValue::Points(0.0));
            node.style.set(StyleKey::MinWidth, StyleValue::Points(0.0));
        }
        if kind.is_mountable() {
            self.layout
                .create(id, &node.style)
                .map_err(|e| Error::Layout(format!("{e:?}")))?;
            self.pending.push(MountOp::Create { id, kind });
        }
        self.nodes[idx] = Some(node);
        // Un control tiene tamaño propio desde que nace: sus props no dicen
        // nada del tamaño, así que si no se marca aquí no se mide nunca y sale
        // de cero.
        if kind.is_control() {
            self.mark_measure_dirty(id);
        }
        self.needs_layout = true;
        Ok(())
    }

    /// Destruye el nodo y todo su subárbol. El host recibe un `Destroy` por
    /// nodo montable, hijos antes que padres.
    /// Destruir es idempotente: un nodo que ya no está no es un error.
    /// Angular puede mandar el borrado y la desvinculación en cualquier orden,
    /// y un búfer que aborta a mitad deja la pantalla rota.
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

    /// `index` es la posición en la lista completa de hijos, incluidos los no
    /// montables. La conversión al índice del host se hace aquí.
    pub fn insert_child(&mut self, parent: NodeId, child: NodeId, index: usize) -> Result<(), Error> {
        self.node(parent)?;
        self.node(child)?;
        let index = index.min(self.node(parent)?.children.len());
        self.node_mut(parent)?.children.insert(index, child);
        self.node_mut(child)?.parent = Some(parent);

        // Los hijos de una pila son pantallas: se superponen y ocupan todo.
        // Es la definición de lo que hace una pila, no una preferencia de
        // estilo, así que lo impone el core y no cada página.
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

    /// Igual que `destroy_node`: quitar algo que ya no cuelga de ahí no es un
    /// error, es la misma situación final.
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

    /// `Renderer2.setStyle`. Nombre en camelCase o kebab-case; valor ya en texto.
    pub fn set_style(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), Error> {
        let Some(key) = StyleKey::from_name(name) else {
            // No es layout: viaja como prop de host (color, backgroundColor...).
            //
            // Y con el nombre en camello, no como llegó. Angular pasa los
            // nombres de estilo a guiones, así que `[style.fontSize]` llega
            // aquí como `font-size`; mandarlo tal cual al host, que busca
            // `fontSize`, era pedirle algo que nunca iba a reconocer. No
            // fallaba: simplemente el texto se medía con una letra y se
            // dibujaba con otra.
            let camel = an_layout::camelize(name);
            return self.set_prop(id, &camel, PropValue::Str(value.to_owned()));
        };
        let parsed = StyleValue::parse(value);
        let node = self.node_mut(id)?;
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
        if kind.is_mountable() {
            self.pending.push(MountOp::SetProp { id, key: key.to_owned(), value });
        }
        if affects_measure(key) && kind.is_measured_leaf() {
            self.mark_measure_dirty(id);
        }
        Ok(())
    }

    /// `Renderer2.setValue` sobre un nodo de texto.
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

    /// Dar de baja un oyente de un nodo que ya no existe no es un error: es lo
    /// que pasa siempre que se destruye una vista con suscripciones vivas, y
    /// el orden en que llegan las dos cosas no está garantizado.
    pub fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) -> Result<(), Error> {
        let Ok(node) = self.node(id) else { return Ok(()) };
        if !node.kind.is_mountable() {
            return Ok(());
        }
        self.pending.push(MountOp::SetListener { id, event: event.to_owned(), enabled });
        Ok(())
    }

    // ------------------------------------------------------------------- commit

    /// Corre el layout y devuelve las operaciones para el host.
    /// Es el único momento en que el árbol produce trabajo nativo.
    pub fn commit(
        &mut self,
        viewport: (f32, f32),
        measurer: &dyn TextMeasurer,
    ) -> Result<Frame, Error> {
        let Some(root) = self.root else {
            // Sin raíz solo tienen sentido las ops ya acumuladas.
            return Ok(Frame { ops: std::mem::take(&mut self.pending) });
        };

        self.flush_children()?;
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

    /// Sincroniza con taffy las listas de hijos que cambiaron, filtrando los
    /// nodos que no participan en el layout.
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

    /// Reconstruye el contexto de medición de las hojas sucias y emite el
    /// `SetText` correspondiente al host.
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
                    // Un campo vacío tiene que seguir midiendo el alto de una
                    // línea, así que se mide el marcador si no hay valor.
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

    /// Recorre el árbol y emite `SetLayout` solo donde el marco cambió.
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
                self.layout.content_size(id).map_err(|e| Error::Layout(format!("{e:?}")))?
            } else {
                (0.0, 0.0)
            };
            let node = self.nodes[id as usize].as_mut().expect("comprobado arriba");
            if !node.laid_out || node.frame != frame {
                node.frame = frame;
                node.laid_out = true;
                ops.push(MountOp::SetLayout { id, frame });
            }
            if scrollable && node.content != content {
                node.content = content;
                ops.push(MountOp::SetContentSize { id, width: content.0, height: content.1 });
            }
            // En orden inverso para que el `pop` recorra en preorden:
            // el host recibe siempre padres antes que hijos.
            stack.extend(children.into_iter().rev());
        }
        Ok(())
    }

    // -------------------------------------------------------------------- utilidades

    /// Índice entre hermanos montables, que es el que entiende el host.
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

    /// Si el padre es un `<Text>`, cualquier cambio en sus hijos crudos obliga
    /// a recomponer y remedir el texto.
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

    /// Concatena el texto crudo del subárbol, en orden.
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
