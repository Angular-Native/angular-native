//! Servidor de desarrollo.
//!
//! Vigila los ficheros, recompila el bundle al guardar y avisa a la app por
//! WebSocket. La app se descarga el bundle nuevo y se reinicia sobre la marcha,
//! sin volver a pasar por Xcode ni por el simulador.
//!
//! La recarga es en caliente siempre que se pueda: el bundle de desarrollo va
//! partido en dos mitades —el framework arriba, la app abajo— y solo se
//! reevalúa la de abajo, encima de la que ya corre. Angular se queda con la
//! instancia de cada componente y le cambia la definición, así que el estado
//! sobrevive: sigues en la misma pantalla y con lo que llevaras escrito.
//!
//! Si lo que cambió está en la mitad de arriba, no hay refresco que valga —en
//! el intérprete solo cabe una copia de Angular— y se reinicia entero. Lo mismo
//! si el árbol de componentes ya no encaja. Ver
//! `packages/platform-native/src/hot-refresh.ts`.

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

use crate::plugins::Plugin;
use crate::workspace::Workspace;
use crate::{build, ios, watchos};

/// Los cambios llegan en ráfagas: guardar en un editor dispara varios eventos,
/// y `ngc` escribe decenas de ficheros. Se espera a que amaine.
const DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct Server {
    bundle: Arc<tokio::sync::RwLock<String>>,
    reloads: broadcast::Sender<()>,
}

/// Dónde lanzar la app que se va a recargar.
pub enum Target {
    Ios { device: String },
    TvOs { device: String },
    VisionOs { device: String },
    WatchOs { device: String },
    Android,
    /// El reloj de Android. Es el mismo emulador y el mismo servidor que
    /// `Android`; lo que cambia es el manifiesto con el que se arma el APK y
    /// la forma del aparato al que va, que son justo las dos cosas que no se
    /// pueden deducir de la otra.
    Wear { device: Option<String> },
}

pub fn run(
    workspace: Workspace,
    app: PathBuf,
    target: Target,
    port: u16,
    no_launch: bool,
    plugins: Vec<Plugin>,
) -> Result<()> {
    let bundle_path = build::bundle(&workspace, &app, false, &plugins)?;
    let source = std::fs::read_to_string(&bundle_path)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("no se pudo arrancar el runtime asíncrono")?;

    let (reloads, _) = broadcast::channel(8);
    let server = Server { bundle: Arc::new(tokio::sync::RwLock::new(source)), reloads };

    // El emulador de Android no ve `localhost`: la máquina anfitriona es
    // 10.0.2.2 desde dentro.
    let url = match target {
        // El simulador del reloj comparte la red del Mac igual que el del
        // teléfono, así que le vale la misma dirección.
        Target::Ios { .. }
        | Target::TvOs { .. }
        | Target::VisionOs { .. }
        | Target::WatchOs { .. } => {
            format!("http://127.0.0.1:{port}")
        }
        Target::Android | Target::Wear { .. } => {
            format!("http://{}:{port}", crate::android::EMULATOR_HOST)
        }
    };

    // El puerto se abre antes de dar nada por bueno: si ya hay otro `an dev`
    // corriendo, más vale decirlo aquí que arrancar a medias.
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind(("127.0.0.1", port)))
        .with_context(|| format!("no se pudo abrir el puerto {port}"))?;

    let serving = server.clone();
    runtime.spawn(async move {
        let router = Router::new()
            .route("/bundle.js", get(serve_bundle))
            .route("/wait", get(wait_for_reload))
            .route("/ws", get(upgrade))
            .with_state(serving);
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("==> el servidor se cayó: {error}");
        }
    });

    eprintln!("==> servidor de desarrollo en {url}");
    if !no_launch {
        match &target {
            Target::Ios { device } => {
                let package = ios::assemble(
                    &workspace,
                    ios::Family::Ios,
                    &bundle_path,
                    false,
                    Some(&url),
                    &plugins,
                )?;
                ios::launch(&package, device)?;
            }
            Target::TvOs { device } => {
                let package = ios::assemble(
                    &workspace,
                    ios::Family::TvOs,
                    &bundle_path,
                    false,
                    Some(&url),
                    &plugins,
                )?;
                ios::launch(&package, device)?;
            }
            Target::VisionOs { device } => {
                let package = ios::assemble(
                    &workspace,
                    ios::Family::VisionOs,
                    &bundle_path,
                    false,
                    Some(&url),
                    &plugins,
                )?;
                ios::launch(&package, device)?;
            }
            Target::WatchOs { device } => {
                let package = watchos::assemble(&workspace, &bundle_path, false, Some(&url))?;
                watchos::launch(&package, device)?;
            }
            Target::Android | Target::Wear { .. } => {
                let (form, device) = match &target {
                    Target::Wear { device } => {
                        (crate::android::Form::Watch, device.as_deref())
                    }
                    _ => (crate::android::Form::Phone, None),
                };
                let apk = crate::android::assemble(
                    &workspace,
                    &bundle_path,
                    false,
                    Some(&url),
                    &plugins,
                    form,
                )?;
                crate::android::install_and_launch(&workspace, &apk, form, device)?;
            }
        }
    }
    eprintln!(
        "==> vigilando {}{}",
        app.join("src").display(),
        if workspace.project.is_none() { " y packages/" } else { "" }
    );

    watch(workspace, app, server, runtime, plugins)
}

async fn serve_bundle(State(server): State<Server>) -> impl IntoResponse {
    let source = server.bundle.read().await.clone();
    ([("content-type", "application/javascript; charset=utf-8")], source)
}

/// Espera larga: el cliente pregunta y el servidor no contesta hasta que hay
/// una recarga. Es para Android, donde la plataforma no trae cliente de
/// WebSocket y meter OkHttp solo para esto no compensa.
///
/// Devuelve 204 al cabo de un rato para que la conexión no se quede colgada
/// indefinidamente en un proxy o en el emulador.
async fn wait_for_reload(State(server): State<Server>) -> impl IntoResponse {
    let mut reloads = server.reloads.subscribe();
    let waited = tokio::time::timeout(Duration::from_secs(30), reloads.recv()).await;
    match waited {
        Ok(Ok(())) => axum::http::StatusCode::OK,
        _ => axum::http::StatusCode::NO_CONTENT,
    }
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
    plugins: Vec<Plugin>,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;

    // `packages/` solo en el monorepo: ahí las fuentes del framework son las que
    // se compilan. En un proyecto de fuera lo que se compila es la copia
    // empaquetada que hay en su `node_modules`, así que vigilar el SDK
    // provocaría recompilaciones que no cambian nada de lo que corre.
    let mut vigilados = vec![workspace.root.join(app.join("src"))];
    if workspace.project.is_none() {
        vigilados.push(workspace.root.join("packages"));
    }
    for path in &vigilados {
        if path.is_dir() {
            watcher.watch(path, RecursiveMode::Recursive)?;
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
        match build::bundle(&workspace, &app, false, &plugins) {
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
