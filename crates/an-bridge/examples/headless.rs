//! A screenless renderer: it evaluates a bundle, steps through frames on a fake
//! clock and prints the resolved tree.
//!
//! It stands up the whole pipeline except UIKit: shadow tree, layout, diff and
//! the event queue. The host is a pretend one, so `onLayout` and the rest of the
//! events the core produces behave exactly as they do on the device.
//!
//! Usage: cargo run -p an-bridge --example headless -- bundle.js [frames] [ms]

// The strings this example prints are matched by `scripts/check-*.sh`, so they
// stay in Spanish until those scripts are translated too.

use std::collections::HashMap;

use an_bridge::modules::{NativeModule, Responder};
use an_bridge::{apply, JsRuntime, QuickJsRuntime};
use an_core::{NaiveMeasurer, NodeId, NodeKind, PropValue, Rect};
use an_host::{new_event_queue, HostEvent, HostRenderer, Renderer};

/// A host that does nothing but write the structure down so it can be
/// printed.
#[derive(Default)]
struct TreeRecorder {
    kinds: HashMap<NodeId, NodeKind>,
    parents: HashMap<NodeId, NodeId>,
    order: HashMap<NodeId, Vec<NodeId>>,
    frames: HashMap<NodeId, Rect>,
    texts: HashMap<NodeId, String>,
    content: HashMap<NodeId, (f32, f32)>,
    root: Option<NodeId>,
    /// The transforms applied to each view. They are kept apart from the rest
    /// of the props because they are printed composed and to two decimals: what
    /// gets checked about a drag is the exact figure that arrived.
    transforms: HashMap<NodeId, HashMap<String, f32>>,
    /// Everything else the host was handed, exactly as it came.
    ///
    /// Without this, a prop that arrives fine and one that never arrives look
    /// identical in the dump: neither the frame nor the text changes. Writing
    /// them down is what makes it possible to check, with no simulator, that
    /// `[variant]` or `[ios]` travelled.
    props: HashMap<NodeId, HashMap<String, String>>,
    pressable: Vec<NodeId>,
    pannable: Vec<NodeId>,
    scrollable: Vec<NodeId>,
    backable: Vec<NodeId>,
    /// So it can be claimed that scrolling creates no views.
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
        self.props.remove(&id);
        for children in self.order.values_mut() {
            children.retain(|child| *child != id);
        }
    }
    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        // Moving a node does not come with a `remove` in front of it: on both
        // platforms, putting a view into another parent already takes it out of
        // where it was. Here it has to be done by hand or the node shows up
        // twice in the tree.
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
            return;
        }
        self.props.entry(id).or_default().insert(key.to_owned(), show_prop(value));
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

/// Answers what iOS would answer, with fixed values.
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

/// A pretend plugin, with its answers written out by hand.
///
/// Real plugins are Swift and Java, and here there is neither. What can be
/// tested with no simulator is everything else: that the module registers under
/// the name its `package.json` gives it, that the call arrives, that the answer
/// resolves the promise, and that a method that is not declared rejects it
/// instead of swallowing it.
///
/// It is declared with `AN_PLUGINS`, a JSON of `{ module: { method: answer } }`:
///
/// ```text
/// AN_PLUGINS='{"clipboard":{"read":"hola","write":null}}'
/// ```
struct CannedPlugin {
    name: &'static str,
    answers: serde_json::Map<String, serde_json::Value>,
}

impl NativeModule for CannedPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn call(&mut self, method: &str, _args: serde_json::Value, respond: Responder) {
        match self.answers.get(method) {
            Some(value) => respond.resolve(value.clone()),
            // The same thing the real plugin would do: say which method was
            // asked for.
            None => respond.reject(format!(
                "el plugin {} no tiene ninguna respuesta preparada para {method:?}",
                self.name
            )),
        }
    }
}

/// Reads `AN_PLUGINS` and returns one module per declared plugin.
fn canned_plugins() -> Vec<CannedPlugin> {
    let Ok(raw) = std::env::var("AN_PLUGINS") else { return Vec::new() };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("AN_PLUGINS no es JSON válido: {error}");
            std::process::exit(2);
        }
    };
    let Some(modules) = parsed.as_object() else {
        eprintln!("AN_PLUGINS tiene que ser un objeto de módulos");
        std::process::exit(2);
    };
    modules
        .iter()
        .map(|(name, answers)| CannedPlugin {
            // Same as in the real bridge: the name arrives at runtime and
            // `NativeModule::name` wants it as a `&'static str`.
            name: Box::leak(name.clone().into_boxed_str()),
            answers: answers.as_object().cloned().unwrap_or_default(),
        })
        .collect()
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
    // `AN_HOT` points at a second bundle: the same example with something
    // changed. It is evaluated on top of the first one to test hot refresh with
    // neither a simulator nor a dev server.
    let hot = std::env::var("AN_HOT").ok();
    // `AN_STACK` makes it possible to feel out the stack limit an app needs.
    let stack = std::env::var("AN_STACK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(QuickJsRuntime::DEFAULT_STACK_SIZE);
    let mut js = QuickJsRuntime::with_options(std::rc::Rc::new(an_bridge::runtime::StderrLog), stack)
        .expect("no arrancó el motor JS");
    // A pretend `device`: it makes the whole path of a native module testable
    // —promise in JS, registry, answer, resolution— with no simulator.
    js.register_module(Box::new(FakeDevice));
    // And whatever plugins `AN_PLUGINS` declares, if any.
    for plugin in canned_plugins() {
        println!("-- plugin de mentira: {}", plugin.name);
        js.register_module(Box::new(plugin));
    }
    if let Err(error) = js.eval(&path, &code) {
        eprintln!("{error}");
        std::process::exit(1);
    }

    // The viewport. An iPhone's by default, which is where most of the examples
    // run; `AN_VIEWPORT=1920x1080` changes it without recompiling anything. A TV
    // screen is not a big phone: at 393 points wide, a screen designed for 1920
    // comes out with everything stacked and broken, and a check looking at it
    // that way checks nothing at all.
    let viewport = std::env::var("AN_VIEWPORT")
        .ok()
        .and_then(|raw| {
            let (w, h) = raw.split_once('x')?;
            Some((w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?))
        })
        .unwrap_or((393.0, 852.0));

    let events = new_event_queue();
    let mut renderer = Renderer::new(TreeRecorder::default(), NaiveMeasurer, viewport, events);
    let mut tapped = false;
    let mut scrolled = false;
    let mut went_back = false;
    let mut panned = false;
    let mut before_back = (0usize, 0usize);
    // The count taken right before scrolling, so it can be said how many views
    // the scroll cost.
    let mut before_scroll = (0usize, 0usize);

    for frame in 0..frames {
        let now = frame as f64 * step;

        // The events the previous frame produced —including the `layout` ones
        // the core emits— go in before anything else.
        let pending = renderer.drain_events();
        if !pending.is_empty() {
            js.dispatch_events(&pending).expect("despacho de eventos");
        }

        // Near the end, once there has been a tap and there is state to lose,
        // the new bundle goes in.
        if let Some(other) = hot.as_ref() {
            if frame + 2 == frames {
                let updated = std::fs::read_to_string(other).expect("no se pudo leer el bundle");
                match js.eval_hot(other, &updated) {
                    Ok(true) => println!("-- refresco en caliente: sí"),
                    Ok(false) => println!("-- refresco en caliente: no, toca reiniciar"),
                    Err(error) => println!("-- refresco en caliente: falló ({error})"),
                }
            }
        }

        // Halfway through the run, a tap on the first node that is listening.
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

        // One frame after the tap, a long scroll: it moves the whole window and
        // shows whether the list recycles or rebuilds.
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

        // A whole drag: begin, move and let go. All three states matter:
        // whoever moves something with a finger updates on `move` and settles on
        // `end`, and if only one of the two arrived it would look like it works
        // until the second drag.
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
                    // Each state in its own turn, applying whatever comes out:
                    // on the device the three do not arrive in the same frame
                    // either, and whoever is dragging updates on every one.
                    let commands = js.tick(now).expect("turno de arrastre");
                    apply(&commands, &mut renderer).expect("búfer del arrastre");
                }
                panned = true;
            }
        }

        // Near the end, the back gesture: it checks that the stack brings the
        // previous screen back rather than rebuilding it.
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
    // The transforms are printed sorted: they are a map, and unsorted the
    // output would change from one run to the next and could not be checked.
    // The props are printed sorted for the same reason as the transforms: they
    // are a map, and unsorted the output would change from one run to the next
    // and there would be nothing to check.
    let props = host
        .props
        .get(&id)
        .filter(|values| !values.is_empty())
        .map(|values| {
            let mut parts: Vec<String> =
                values.iter().map(|(key, value)| format!("{key}={value}")).collect();
            parts.sort();
            format!("  props {}", parts.join(" "))
        })
        .unwrap_or_default();
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
        "{:indent$}{kind}#{id} [{:.0},{:.0} {:.0}x{:.0}]{content}{transform}{text}{props}",
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

/// One prop on one line. Long strings get cut: a tab bar's `items`, or a
/// browser's HTML, would fill the entire dump.
fn show_prop(value: &PropValue) -> String {
    match value {
        PropValue::Null => "null".to_owned(),
        PropValue::Bool(v) => v.to_string(),
        PropValue::Number(v) => format!("{v}"),
        PropValue::Str(v) => truncate(v, 32),
        PropValue::Color(v) => format!("#{v:08x}"),
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    text.chars().take(max).collect::<String>() + "…"
}
