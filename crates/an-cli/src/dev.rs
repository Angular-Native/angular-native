//! Servidor de desarrollo.
//!
//! Vigila los ficheros, recompila el bundle al guardar y avisa a la app por
//! WebSocket. La app se descarga el bundle nuevo y se reinicia sobre la marcha,
//! sin volver a pasar por Xcode ni por el simulador.
//!
//! La recarga es completa: el estado se pierde. Preservarlo entre recargas
//! —el *fast refresh* de React Native— exige saber qué componentes cambiaron y
//! reconciliar el árbol, y es un proyecto en sí mismo.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::workspace::Workspace;
use crate::{build, ios};

/// Los cambios llegan en ráfagas: guardar en un editor dispara varios eventos,
/// y `ngc` escribe decenas de ficheros. Se espera a que amaine.
const DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct Server {
    bundle: Arc<tokio::sync::RwLock<String>>,
    reloads: broadcast::Sender<()>,
}

pub fn run(
    workspace: Workspace,
    app: PathBuf,
    device: String,
    port: u16,
    no_launch: bool,
) -> Result<()> {
    let bundle_path = build::bundle(&workspace, &app, false)?;
    let source = std::fs::read_to_string(&bundle_path)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("no se pudo arrancar el runtime asíncrono")?;

    let (reloads, _) = broadcast::channel(8);
    let server = Server { bundle: Arc::new(tokio::sync::RwLock::new(source)), reloads };

    let url = format!("http://127.0.0.1:{port}");

    // El puerto se abre antes de dar nada por bueno: si ya hay otro `an dev`
    // corriendo, más vale decirlo aquí que arrancar a medias.
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind(("127.0.0.1", port)))
        .with_context(|| format!("no se pudo abrir el puerto {port}"))?;

    let serving = server.clone();
    runtime.spawn(async move {
        let router = Router::new()
            .route("/bundle.js", get(serve_bundle))
            .route("/ws", get(upgrade))
            .with_state(serving);
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("==> el servidor se cayó: {error}");
        }
    });

    eprintln!("==> servidor de desarrollo en {url}");
    if !no_launch {
        let package = ios::assemble(&workspace, &bundle_path, false, Some(&url))?;
        ios::launch(&package, &device)?;
    }
    eprintln!("==> vigilando {} y packages/", app.display());

    watch(workspace, app, server, runtime)
}

async fn serve_bundle(State(server): State<Server>) -> impl IntoResponse {
    let source = server.bundle.read().await.clone();
    ([("content-type", "application/javascript; charset=utf-8")], source)
}

async fn upgrade(ws: WebSocketUpgrade, State(server): State<Server>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| notify_client(socket, server))
}

async fn notify_client(mut socket: WebSocket, server: Server) {
    let mut reloads = server.reloads.subscribe();
    eprintln!("==> app conectada");
    while reloads.recv().await.is_ok() {
        if socket.send(Message::Text("reload".into())).await.is_err() {
            break;
        }
    }
}

fn watch(
    workspace: Workspace,
    app: PathBuf,
    server: Server,
    runtime: tokio::runtime::Runtime,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;

    for dir in [app.join("src"), PathBuf::from("packages")] {
        let path = workspace.root.join(dir);
        if path.is_dir() {
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }
    }

    let mut last = Instant::now() - DEBOUNCE;
    loop {
        let Ok(event) = rx.recv() else { break };
        let Ok(event) = event else { continue };
        // Solo importan las fuentes: ignorar lo que el propio build escribe
        // evitaría un bucle si algún día `build/` cayera dentro de lo vigilado.
        let relevant = event.paths.iter().any(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ts" | "js" | "html" | "json")
            )
        });
        if !relevant || last.elapsed() < DEBOUNCE {
            continue;
        }
        // Vaciar la ráfaga antes de compilar, o se compila una vez por fichero.
        while rx.recv_timeout(DEBOUNCE).is_ok() {}
        last = Instant::now();

        eprintln!("\n==> cambio detectado, recompilando");
        match build::bundle(&workspace, &app, false) {
            Ok(path) => match std::fs::read_to_string(&path) {
                Ok(source) => {
                    runtime.block_on(async {
                        *server.bundle.write().await = source;
                    });
                    let clients = server.reloads.send(()).unwrap_or(0);
                    eprintln!("==> recargado en {clients} cliente(s)");
                }
                Err(error) => eprintln!("==> no se pudo leer el bundle: {error}"),
            },
            // Un error de compilación no puede tirar el servidor: se informa y
            // se sigue vigilando, que es lo que uno espera al equivocarse.
            Err(error) => eprintln!("==> la compilación falló: {error}"),
        }
    }
    Ok(())
}
