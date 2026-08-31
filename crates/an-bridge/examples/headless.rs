//! Renderer sin pantalla: evalúa un bundle, avanza frames con un reloj falso e
//! imprime el árbol resuelto.
//!
//! Monta el pipeline entero salvo UIKit: shadow tree, layout, diff y cola de
//! eventos. El host es de mentira, así que `onLayout` y el resto de eventos
//! que produce el core funcionan igual que en el dispositivo.
//!
//! Uso: cargo run -p an-bridge --example headless -- bundle.js [frames] [ms]

use std::collections::HashMap;

use an_bridge::{apply, JsRuntime, QuickJsRuntime};
use an_core::{NaiveMeasurer, NodeId, NodeKind, PropValue, Rect};
use an_host::{new_event_queue, HostEvent, HostRenderer, Renderer};

/// Host que se limita a apuntar la estructura para poder imprimirla.
#[derive(Default)]
struct TreeRecorder {
    kinds: HashMap<NodeId, NodeKind>,
    parents: HashMap<NodeId, NodeId>,
    order: HashMap<NodeId, Vec<NodeId>>,
    frames: HashMap<NodeId, Rect>,
    texts: HashMap<NodeId, String>,
    content: HashMap<NodeId, (f32, f32)>,
    root: Option<NodeId>,
    pressable: Vec<NodeId>,
}

impl HostRenderer for TreeRecorder {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        self.kinds.insert(id, kind);
    }
    fn destroy(&mut self, id: NodeId) {
        self.kinds.remove(&id);
        self.parents.remove(&id);
        self.frames.remove(&id);
        self.texts.remove(&id);
        for children in self.order.values_mut() {
            children.retain(|child| *child != id);
        }
    }
    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        self.parents.insert(child, parent);
        let children = self.order.entry(parent).or_default();
        let at = (index as usize).min(children.len());
        children.insert(at, child);
    }
    fn remove(&mut self, parent: NodeId, child: NodeId) {
        self.parents.remove(&child);
        if let Some(children) = self.order.get_mut(&parent) {
            children.retain(|current| *current != child);
        }
    }
    fn set_prop(&mut self, _id: NodeId, _key: &str, _value: &PropValue) {}
    fn set_text(&mut self, id: NodeId, text: &str) {
        self.texts.insert(id, text.to_owned());
    }
    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        if event != "press" {
            return;
        }
        if enabled {
            self.pressable.push(id);
        } else {
            self.pressable.retain(|node| *node != id);
        }
    }
    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        self.frames.insert(id, frame);
    }
    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        self.content.insert(id, (width, height));
    }
    fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Responde lo mismo que respondería iOS, con valores fijos.
struct FakeDevice;

an_bridge::native_module! {
    FakeDevice as "device" {
        fn info(&mut self, _args: ()) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "platform": "headless",
                "systemVersion": "0.0",
                "model": "sin dispositivo",
                "scale": 3.0,
                "locale": "es-ES"
            }))
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        eprintln!("uso: headless <bundle.js> [frames] [ms-por-frame]");
        std::process::exit(2)
    });
    let frames: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(3);
    let step: f64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1000.0);

    let code = std::fs::read_to_string(&path).expect("no se pudo leer el bundle");
    // `AN_STACK` permite tantear el límite de pila que necesita una app.
    let stack = std::env::var("AN_STACK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(QuickJsRuntime::DEFAULT_STACK_SIZE);
    let mut js = QuickJsRuntime::with_options(std::rc::Rc::new(an_bridge::runtime::StderrLog), stack)
        .expect("no arrancó el motor JS");
    // Un `device` de mentira: permite probar el camino completo de un módulo
    // nativo —promesa en JS, registro, respuesta, resolución— sin simulador.
    js.register_module(Box::new(FakeDevice));
    if let Err(error) = js.eval(&path, &code) {
        eprintln!("{error}");
        std::process::exit(1);
    }

    let events = new_event_queue();
    let mut renderer =
        Renderer::new(TreeRecorder::default(), NaiveMeasurer, (393.0, 852.0), events);
    let mut tapped = false;

    for frame in 0..frames {
        let now = frame as f64 * step;

        // Los eventos que produjo el frame anterior —incluidos los `layout`
        // que emite el core— entran antes que nada.
        let pending = renderer.drain_events();
        if !pending.is_empty() {
            js.dispatch_events(&pending).expect("despacho de eventos");
        }

        // A mitad de la ejecución, un toque en el primer nodo que escuche.
        if !tapped && frame >= frames / 2 {
            if let Some(target) = renderer.host().pressable.first().copied() {
                println!("-- toque simulado en #{target}");
                js.dispatch_events(&[HostEvent {
                    target,
                    name: "press".to_owned(),
                    payload: vec![
                        ("x".to_owned(), PropValue::Number(40.0)),
                        ("y".to_owned(), PropValue::Number(20.0)),
                    ],
                }])
                .expect("despacho de eventos");
                tapped = true;
            }
        }

        let commands = match js.tick(now) {
            Ok(commands) => commands,
            Err(error) => {
                eprintln!("frame {frame}: {error}");
                std::process::exit(1);
            }
        };
        if let Err(error) = apply(&commands, &mut renderer.tree) {
            eprintln!("frame {frame}: búfer inválido: {error:?}");
            std::process::exit(1);
        }
        let applied = renderer.render_frame().expect("commit");
        println!("-- frame {frame} (t={now}ms): {applied} operaciones");
    }

    println!("\n== árbol resuelto ==");
    let host = renderer.host();
    match host.root {
        Some(root) => print_node(host, root, 0),
        None => println!("(sin raíz: la app no llegó a montar nada)"),
    }
    println!("\nvistas nativas montadas: {}", host.kinds.len());
}

fn print_node(host: &TreeRecorder, id: NodeId, depth: usize) {
    let kind = host.kinds.get(&id).map(|k| format!("{k:?}")).unwrap_or_else(|| "?".into());
    let rect = host.frames.get(&id).copied().unwrap_or_default();
    let text = host
        .texts
        .get(&id)
        .map(|t| format!("  {:?}", truncate(t, 40)))
        .unwrap_or_default();
    let content = host
        .content
        .get(&id)
        .map(|(w, h)| format!("  contenido {w:.0}x{h:.0}"))
        .unwrap_or_default();
    println!(
        "{:indent$}{kind}#{id} [{:.0},{:.0} {:.0}x{:.0}]{content}{text}",
        "",
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        indent = depth * 2
    );
    if let Some(children) = host.order.get(&id) {
        for child in children {
            print_node(host, *child, depth + 1);
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    text.chars().take(max).collect::<String>() + "…"
}
