//! The dev server.
//!
//! It watches the files, rebuilds the bundle when you save and tells the app
//! over a WebSocket. The app downloads the new bundle and restarts on the fly,
//! with no second trip through Xcode or the simulator.
//!
//! The reload is a hot one whenever it can be: the development bundle is split
//! in two halves —the framework on top, the app underneath— and only the bottom
//! one is re-evaluated, on top of the one already running. Angular keeps each
//! component's instance and swaps its definition, so the state survives: you are
//! still on the same screen with whatever you had typed.
//!
//! If what changed is in the top half there is no refresh worth attempting —only
//! one copy of Angular fits in the interpreter— and it restarts whole. That
//! restart is not something anyone has to do by hand: the engine thread throws
//! the interpreter away, stands a new one up and evaluates the bundle it has
//! just been handed, so the developer sees the new framework code on the next
//! frame and loses the component state, nothing else. Same if the component
//! tree no longer lines up. See `packages/platform-native/src/hot-refresh.ts`.
//!
//! One part of `packages/` never travels in the bundle: the prelude,
//! `packages/runtime/runtime.js`, is `include_str!`-ed into `an-bridge` and so
//! lives inside the native binary. Rebuilding the bundle after saving it
//! produces a byte-identical top half, the stamp matches, the reload is
//! reported as hot and nothing on screen has changed — a save that says it
//! worked and did not. See `native_only`.

use std::path::{Path, PathBuf};
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
use crate::{build, ios, macos, watchos};

/// Changes arrive in bursts: saving in an editor fires several events, and
/// `ngc` writes dozens of files. It waits for things to die down.
const DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct Server {
    bundle: Arc<tokio::sync::RwLock<String>>,
    reloads: broadcast::Sender<()>,
}

/// Where to launch the app that is going to be reloaded.
pub enum Target {
    Ios { device: String },
    TvOs { device: String },
    VisionOs { device: String },
    WatchOs { device: String },
    /// The Mac. It is the only target that is not a simulator: the app runs on
    /// the machine doing the building, which is why it carries no device —there
    /// is nothing to look up— and why the URL needs no translating.
    MacOs,
    Android,
    /// The Android watch. Same emulator and same server as `Android`; what
    /// changes is the manifest the APK is built with and the shape of the device
    /// it goes to, which are precisely the two things neither can be worked out
    /// from the other.
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

    // One address for everything now. The Android targets used to need
    // `10.0.2.2` — the host machine seen from inside the emulator — and
    // `adb reverse` makes that unnecessary: the port is opened on the device,
    // pointing back here, so a phone over USB reaches the server exactly as an
    // emulator does. See `android::reverse_dev_port`.
    let url = match target {
        // The watch simulator shares the Mac's network the same way the
        // phone's does, so the same address works for it. And the Mac needs no
        // translation at all, for a stronger reason: the app is not inside
        // anything. It runs on this very machine, so 127.0.0.1 is not a bridge
        // to the host — it *is* the host.
        Target::Ios { .. }
        | Target::TvOs { .. }
        | Target::VisionOs { .. }
        | Target::WatchOs { .. }
        | Target::MacOs => {
            format!("http://127.0.0.1:{port}")
        }
        Target::Android | Target::Wear { .. } => {
            format!("http://127.0.0.1:{port}")
        }
    };

    // The port is opened before anything is taken for granted: if there is
    // another `an dev` already running, better to say so here than to start up
    // halfway.
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
        launch_app(&workspace, &app, &target, &bundle_path, &url, port, &plugins)?;
    }
    eprintln!(
        "==> vigilando {}{}",
        app.join("src").display(),
        if workspace.project.is_none() { " y packages/" } else { "" }
    );

    watch(workspace, app, Launch { target, url, port, no_launch }, server, runtime, plugins)
}

/// Builds the app and puts it on the device. It runs at startup and again
/// whenever a save has changed something the bundle cannot carry: the same call,
/// because a rebuilt-and-relaunched app is exactly what the developer would
/// otherwise do by hand.
fn launch_app(
    workspace: &Workspace,
    app: &Path,
    target: &Target,
    bundle_path: &Path,
    url: &str,
    port: u16,
    plugins: &[Plugin],
) -> Result<()> {
    match target {
        Target::Ios { device } => {
            let package = ios::assemble(
                workspace,
                app,
                ios::Family::Ios,
                bundle_path,
                false,
                Some(url),
                plugins,
                // The dev server goes to a simulator, always: a device
                // build is signed, and a signature is not something to put
                // in a loop that rebuilds on every save.
                None,
            )?;
            ios::launch(&package, device)?;
        }
        Target::TvOs { device } => {
            let package = ios::assemble(
                workspace,
                app,
                ios::Family::TvOs,
                bundle_path,
                false,
                Some(url),
                plugins,
                // The dev server goes to a simulator, always: a device
                // build is signed, and a signature is not something to put
                // in a loop that rebuilds on every save.
                None,
            )?;
            ios::launch(&package, device)?;
        }
        Target::VisionOs { device } => {
            let package = ios::assemble(
                workspace,
                app,
                ios::Family::VisionOs,
                bundle_path,
                false,
                Some(url),
                plugins,
                // The dev server goes to a simulator, always: a device
                // build is signed, and a signature is not something to put
                // in a loop that rebuilds on every save.
                None,
            )?;
            ios::launch(&package, device)?;
        }
        Target::WatchOs { device } => {
            let package =
                watchos::assemble(workspace, app, bundle_path, false, Some(url), plugins)?;
            watchos::launch(&package, device)?;
        }
        // No `simctl` and no device: `launch` kills whatever instance was
        // already up —otherwise `open` only brings the old window to the
        // front and the change looks as though it never landed— and opens
        // the new one. See `macos.rs`.
        Target::MacOs => {
            let package = macos::assemble(
                workspace,
                app,
                bundle_path,
                false,
                Some(url),
                plugins,
                None,
            )?;
            macos::launch(&package)?;
        }
        Target::Android | Target::Wear { .. } => {
            let (form, device) = match target {
                Target::Wear { device } => {
                    (crate::android::Form::Watch, device.as_deref())
                }
                _ => (crate::android::Form::Phone, None),
            };
            let apk = crate::android::assemble(
                workspace,
                app,
                bundle_path,
                false,
                Some(url),
                plugins,
                // The dev loop is always the debug key: a release-signed
                // APK is an artefact for a store, not something to rebuild
                // every time a file is saved.
                crate::android::Packaging {
                    form,
                    signing: None,
                    aab: false,
                    // The APK default, arm64-v8a alone. The dev loop has
                    // no --abi of its own: every extra ABI is another
                    // cross-compilation of the core between saving a file
                    // and seeing it, and the device on the other end is
                    // one machine whose architecture does not change.
                    abis: crate::android::abis(&[], false),
                    bundletool: None,
                },
            )?;
            crate::android::install_and_launch(workspace, &apk, form, device, Some(port))?;
        }
    }
    Ok(())
}

async fn serve_bundle(State(server): State<Server>) -> impl IntoResponse {
    let source = server.bundle.read().await.clone();
    ([("content-type", "application/javascript; charset=utf-8")], source)
}

/// A long wait: the client asks and the server does not answer until there is a
/// reload. It is for Android, where the platform ships no WebSocket client and
/// pulling in OkHttp just for this is not worth it.
///
/// It returns a 204 after a while so the connection does not hang around for
/// ever in a proxy or in the emulator.
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

/// Everything the watcher needs to put the app back on the device. It is the
/// same set `run` used to launch it the first time.
struct Launch {
    target: Target,
    url: String,
    port: u16,
    no_launch: bool,
}

/// Whether a saved file is one the bundle cannot carry.
///
/// `packages/runtime/runtime.js` is the prelude, and `an-bridge` takes it in
/// with `include_str!` (`crates/an-bridge/src/quickjs.rs`): it is compiled into
/// the native binary and never appears in the JavaScript the server serves.
/// Everything else under `packages/` is TypeScript that `ngc` and esbuild put in
/// the bundle's top half, where a reload does reach it.
fn native_only(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|rel| rel.starts_with("packages/runtime"))
}

/// The extensions a save has to carry to be worth a rebuild. What the build
/// itself writes is ignored, which would avoid a loop if `build/` ever fell
/// inside what is watched.
fn sources(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ts" | "js" | "html" | "json")
            )
        })
        .cloned()
        .collect()
}

fn watch(
    workspace: Workspace,
    app: PathBuf,
    launch: Launch,
    server: Server,
    runtime: tokio::runtime::Runtime,
    plugins: Vec<Plugin>,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;

    // `packages/` in the monorepo only: in there the framework's sources are
    // what gets compiled. In a project from outside what gets compiled is the
    // packaged copy in its `node_modules`, so watching the SDK would set off
    // rebuilds that change nothing about what is running.
    let mut watched = vec![workspace.root.join(app.join("src"))];
    if workspace.project.is_none() {
        watched.push(workspace.root.join("packages"));
    }
    for path in &watched {
        if path.is_dir() {
            watcher.watch(path, RecursiveMode::Recursive)?;
        }
    }

    let mut last = Instant::now() - DEBOUNCE;
    loop {
        let Ok(event) = rx.recv() else { break };
        let Ok(event) = event else { continue };
        let mut changed = sources(&event.paths);
        if changed.is_empty() || last.elapsed() < DEBOUNCE {
            continue;
        }
        // Drain the burst before compiling, or it compiles once per file. The
        // paths are kept rather than thrown away: which files were saved is
        // what decides whether a new bundle is enough.
        while let Ok(more) = rx.recv_timeout(DEBOUNCE) {
            if let Ok(more) = more {
                changed.extend(sources(&more.paths));
            }
        }
        last = Instant::now();

        let native = changed.iter().any(|path| native_only(&workspace.root, path));

        eprintln!("\n==> cambio detectado, recompilando");
        let path = match build::bundle(&workspace, &app, false, &plugins) {
            Ok(path) => path,
            // A compilation error must not take the server down: it is reported
            // and the watching goes on, which is what anyone expects after
            // making a mistake.
            Err(error) => {
                eprintln!("==> la compilación falló: {error}");
                continue;
            }
        };
        match std::fs::read_to_string(&path) {
            Ok(source) => runtime.block_on(async {
                *server.bundle.write().await = source;
            }),
            Err(error) => {
                eprintln!("==> no se pudo leer el bundle: {error}");
                continue;
            }
        }

        if native {
            // Telling the app to download the bundle again would change
            // nothing and would report a reload that did not happen. The
            // prelude only travels inside the binary, so the app is built
            // again and put back on the device: the same thing the developer
            // would do by hand, without having to notice that it was needed.
            eprintln!(
                "==> packages/runtime/ va dentro del binario, no del bundle: \
                 reconstruyendo la app"
            );
            if launch.no_launch {
                eprintln!(
                    "==> con --no-launch no hay app que reemplazar; \
                     el nuevo runtime entra en la siguiente compilación"
                );
            } else if let Err(error) = launch_app(
                &workspace,
                &app,
                &launch.target,
                &path,
                &launch.url,
                launch.port,
                &plugins,
            ) {
                eprintln!("==> no se pudo reconstruir la app: {error}");
            }
            continue;
        }

        let clients = server.reloads.send(()).unwrap_or(0);
        eprintln!("==> recargado en {clients} cliente(s)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prelude_is_not_in_the_bundle() {
        let root = Path::new("/sdk");
        assert!(native_only(root, Path::new("/sdk/packages/runtime/runtime.js")));
        // The rest of `packages/` is TypeScript, and it does travel.
        assert!(!native_only(root, Path::new("/sdk/packages/platform-native/src/platform.ts")));
        assert!(!native_only(root, Path::new("/sdk/packages/primitives/src/primitives.ts")));
        assert!(!native_only(root, Path::new("/sdk/examples/hello-angular/src/app.component.ts")));
        // A project of the user's that happens to have that shape is not the
        // SDK's prelude: the path is anchored to the root, not searched for.
        assert!(!native_only(root, Path::new("/elsewhere/packages/runtime/runtime.js")));
    }

    #[test]
    fn only_sources_set_off_a_rebuild() {
        let paths = vec![
            PathBuf::from("/sdk/packages/runtime/runtime.js"),
            PathBuf::from("/sdk/build/bundle/hello-angular/main.js.map"),
            PathBuf::from("/sdk/examples/hello-angular/src/app.component.ts"),
        ];
        let kept = sources(&paths);
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().any(|path| native_only(Path::new("/sdk"), path)));
    }
}
