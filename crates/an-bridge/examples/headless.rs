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
    /// Transformaciones aplicadas a cada vista. Solo estas: son las únicas
    /// props que no se ven en el árbol —no cambian marco ni contenido— y por
    /// tanto las únicas que hay que apuntar para poder comprobarlas.
    transforms: HashMap<NodeId, HashMap<String, f32>>,
    pressable: Vec<NodeId>,
    pannable: Vec<NodeId>,
    scrollable: Vec<NodeId>,
    backable: Vec<NodeId>,
    /// Para poder afirmar que desplazarse no crea vistas.
    created: usize,
    destroyed: usize,
}

impl HostRenderer for TreeRecorder {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        self.created += 1;
        self.kinds.insert(id, kind);
    }
    fn destroy(&mut self, id: NodeId) {
        self.destroyed += 1;
        self.kinds.remove(&id);
        self.parents.remove(&id);
        self.frames.remove(&id);
        self.texts.remove(&id);
        self.transforms.remove(&id);
        for children in self.order.values_mut() {
            children.retain(|child| *child != id);
        }
    }
    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        // Mover un nodo no lleva un `remove` delante: en las dos plataformas
        // meter una vista en otro padre ya la saca de donde estaba. Aquí hay
        // que hacerlo a mano o el nodo sale dos veces en el árbol.
        if let Some(previous) = self.parents.insert(child, parent) {
            if let Some(siblings) = self.order.get_mut(&previous) {
                siblings.retain(|current| *current != child);
            }
        }
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
    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        if matches!(key, "translateX" | "translateY" | "scale" | "rotate") {
            if let Some(number) = value.as_f32() {
                self.transforms.entry(id).or_default().insert(key.to_owned(), number);
            }
        }
    }
    fn set_text(&mut self, id: NodeId, text: &str) {
        self.texts.insert(id, text.to_owned());
    }
    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let list = match event {
            "press" => &mut self.pressable,
            "pan" => &mut self.pannable,
            "scroll" => &mut self.scrollable,
            "back" => &mut self.backable,
            _ => return,
        };
        if enabled {
            list.push(id);
        } else {
            list.retain(|node| *node != id);
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
    let mut scrolled = false;
    let mut went_back = false;
    let mut panned = false;
    let mut before_back = (0usize, 0usize);
    // Recuento en el momento justo antes de desplazar, para poder decir
    // cuántas vistas costó el desplazamiento.
    let mut before_scroll = (0usize, 0usize);

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

        // Un frame después del toque, un desplazamiento largo: mueve la
        // ventana entera y deja ver si la lista recicla o rehace.
        if !scrolled && frame >= frames / 2 {
            if let Some(target) = renderer.host().scrollable.first().copied() {
                before_scroll = (renderer.host().created, renderer.host().destroyed);
                println!("-- desplazamiento simulado en #{target} hasta y=4000");
                js.dispatch_events(&[HostEvent {
                    target,
                    name: "scroll".to_owned(),
                    payload: vec![
                        ("x".to_owned(), PropValue::Number(0.0)),
                        ("y".to_owned(), PropValue::Number(4000.0)),
                    ],
                }])
                .expect("despacho de eventos");
                scrolled = true;
            }
        }

        // Un arrastre entero: empezar, mover y soltar. Los tres estados
        // importan: quien mueve algo con el dedo actualiza en `move` y fija en
        // `end`, y si solo llegase uno de los dos parecería que funciona hasta
        // el segundo arrastre.
        if !panned && frame >= frames / 2 {
            let target = renderer.host().pannable.first().copied();
            if let Some(target) = target {
                println!("-- arrastre simulado en #{target}");
                for (state, dx, dy) in
                    [("begin", 0.0, 0.0), ("move", 60.0, 25.0), ("end", 60.0, 25.0)]
                {
                    js.dispatch_events(&[HostEvent {
                        target,
                        name: "pan".to_owned(),
                        payload: vec![
                            ("x".to_owned(), PropValue::Number(10.0)),
                            ("y".to_owned(), PropValue::Number(10.0)),
                            ("translationX".to_owned(), PropValue::Number(dx)),
                            ("translationY".to_owned(), PropValue::Number(dy)),
                            ("velocityX".to_owned(), PropValue::Number(120.0)),
                            ("velocityY".to_owned(), PropValue::Number(0.0)),
                            ("state".to_owned(), PropValue::Str(state.to_owned())),
                        ],
                    }])
                    .expect("despacho de eventos");
                    // Cada estado en su propio turno, y aplicando lo que salga:
                    // en el dispositivo tampoco llegan los tres en el mismo
                    // frame, y quien arrastra actualiza en cada uno.
                    let commands = js.tick(now).expect("turno de arrastre");
                    apply(&commands, &mut renderer).expect("búfer del arrastre");
                }
                panned = true;
            }
        }

        // Cerca del final, el gesto de volver atrás: comprueba que la pila
        // recupera la pantalla anterior en vez de rehacerla.
        if !went_back && frames > 3 && frame == frames - 2 {
            if let Some(target) = renderer.host().backable.first().copied() {
                before_back = (renderer.host().created, renderer.host().destroyed);
                println!("-- atrás simulado en #{target}");
                js.dispatch_events(&[HostEvent {
                    target,
                    name: "back".to_owned(),
                    payload: Vec::new(),
                }])
                .expect("despacho de eventos");
                went_back = true;
            }
        }

        let commands = match js.tick(now) {
            Ok(commands) => commands,
            Err(error) => {
                eprintln!("frame {frame}: {error}");
                std::process::exit(1);
            }
        };
        if let Err(error) = apply(&commands, &mut renderer) {
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
    if scrolled {
        println!(
            "desplazarse costó {} vistas creadas y {} destruidas",
            host.created - before_scroll.0,
            host.destroyed - before_scroll.1
        );
    }
    if went_back {
        println!(
            "volver atrás costó {} vistas creadas y {} destruidas",
            host.created - before_back.0,
            host.destroyed - before_back.1
        );
    }
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
    // Las transformaciones se imprimen ordenadas: son un mapa, y sin ordenar
    // la salida cambiaría de una ejecución a otra y no se podría comprobar.
    let transform = host
        .transforms
        .get(&id)
        .filter(|values| !values.is_empty())
        .map(|values| {
            let mut parts: Vec<String> =
                values.iter().map(|(key, value)| format!("{key}={value:.2}")).collect();
            parts.sort();
            format!("  transform {}", parts.join(" "))
        })
        .unwrap_or_default();
    println!(
        "{:indent$}{kind}#{id} [{:.0},{:.0} {:.0}x{:.0}]{content}{transform}{text}",
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
