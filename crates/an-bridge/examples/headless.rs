//! Renderer sin pantalla: evalúa un bundle, avanza frames con un reloj falso e
//! imprime el árbol resuelto.
//!
//! Es la forma rápida de depurar el lado JS sin simulador — y el embrión de lo
//! que `an-cli` acabará ofreciendo como `an render`.
//!
//! Uso: cargo run -p an-bridge --example headless -- bundle.js [frames] [ms]

use std::collections::HashMap;

use an_bridge::{apply, JsRuntime, QuickJsRuntime};
use an_core::{MountOp, NaiveMeasurer, Rect, ShadowTree};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        eprintln!("uso: headless <bundle.js> [frames] [ms-por-frame]");
        std::process::exit(2)
    });
    let frames: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(3);
    let step: f64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1000.0);

    let code = std::fs::read_to_string(&path).expect("no se pudo leer el bundle");
    let mut js = QuickJsRuntime::new().expect("no arrancó el motor JS");
    if let Err(error) = js.eval(&path, &code) {
        eprintln!("{error}");
        std::process::exit(1);
    }

    let viewport = (393.0_f32, 852.0_f32);
    let mut tree = ShadowTree::new();
    let mut layout: HashMap<u32, Rect> = HashMap::new();
    let mut labels: HashMap<u32, String> = HashMap::new();
    let mut parents: HashMap<u32, u32> = HashMap::new();
    let mut kinds: HashMap<u32, String> = HashMap::new();
    let mut root = None;

    for frame in 0..frames {
        let now = frame as f64 * step;
        let commands = match js.tick(now) {
            Ok(commands) => commands,
            Err(error) => {
                eprintln!("frame {frame}: {error}");
                std::process::exit(1);
            }
        };
        if let Err(error) = apply(&commands, &mut tree) {
            eprintln!("frame {frame}: búfer inválido: {error:?}");
            std::process::exit(1);
        }
        let result = tree.commit(viewport, &NaiveMeasurer).expect("commit");
        println!("-- frame {frame} (t={now}ms): {} operaciones", result.ops.len());
        for op in &result.ops {
            match op {
                MountOp::SetLayout { id, frame } => {
                    layout.insert(*id, *frame);
                }
                MountOp::SetText { id, text } => {
                    labels.insert(*id, text.clone());
                }
                MountOp::Insert { parent, child, .. } => {
                    parents.insert(*child, *parent);
                }
                MountOp::Create { id, kind } => {
                    kinds.insert(*id, format!("{kind:?}"));
                }
                MountOp::SetRoot { id } => root = Some(*id),
                _ => {}
            }
        }
    }

    println!("\n== árbol resuelto ==");
    let Some(root) = root else {
        println!("(sin raíz: la app no llegó a montar nada)");
        return;
    };
    print_node(root, 0, &kinds, &layout, &labels, &parents);
}

fn print_node(
    id: u32,
    depth: usize,
    kinds: &HashMap<u32, String>,
    layout: &HashMap<u32, Rect>,
    labels: &HashMap<u32, String>,
    parents: &HashMap<u32, u32>,
) {
    let kind = kinds.get(&id).map(String::as_str).unwrap_or("?");
    let rect = layout.get(&id).copied().unwrap_or_default();
    let text = labels
        .get(&id)
        .map(|t| format!("  {:?}", truncate(t, 48)))
        .unwrap_or_default();
    println!(
        "{:indent$}{kind}#{id} [{:.0},{:.0} {:.0}x{:.0}]{text}",
        "",
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        indent = depth * 2
    );
    let mut children: Vec<u32> = parents
        .iter()
        .filter(|(_, parent)| **parent == id)
        .map(|(child, _)| *child)
        .collect();
    children.sort_unstable();
    for child in children {
        print_node(child, depth + 1, kinds, layout, labels, parents);
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    text.chars().take(max).collect::<String>() + "…"
}
