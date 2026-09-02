//! Builds the watch `.app` and takes it to the watchOS simulator.
//!
//! The same thing `ios.rs` does —no `.xcodeproj`, `swiftc` linking against
//! Rust's staticlib— with three differences that are not cosmetic:
//!
//! 1. **Nightly is required.** `aarch64-apple-watchos-sim` is a tier 3 target
//!    and ships no precompiled `std`, so it has to be built on the spot with
//!    `-Z build-std`. That is why this subcommand calls `cargo +nightly` rather
//!    than the `cargo` from `rust-toolchain.toml`.
//! 2. **`-parse-as-library`.** The watch shell starts at an `@main` on a SwiftUI
//!    `App`. Without this flag `swiftc` treats the first file as a top-level
//!    script and the `@main` goes unused.
//! 3. **The bundle is a watch's.** `WKApplication` in the `Info.plist` and
//!    device family 4; without those `simctl` installs something it then cannot
//!    launch.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::ios::swift_sources;
use crate::plugins::Plugin;
use crate::workspace::Workspace;

/// The watch does not load plugins yet: `an-watch` has none of the registry
/// `an-ios` and `an-android` have, and its shell is not a port of the iOS one.
///
/// It is said here and it stops. Building the `.app` anyway would leave an app
/// in which the module does not exist and every call is turned down at runtime,
/// and that is exactly what this system must never do.
pub fn reject_plugins(plugins: &[Plugin]) -> Result<()> {
    if plugins.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.package.as_str()).collect();
    bail!(
        "esta app no se puede compilar para watchOS: el reloj todavía no carga plugins, \
         y depende de {}. Ver docs/plugins.md.",
        names.join(", ")
    )
}

const APP_NAME: &str = "AngularNativeWatch";
const BUNDLE_ID: &str = "dev.angularnative.playground.watchkitapp";
const TARGET: &str = "aarch64-apple-watchos-sim";
/// watchOS 11 is the oldest one where `@Observable` and the SwiftUI additions
/// the shell uses are available without `@available` sprinkled everywhere.
const DEPLOYMENT: &str = "11.0";

pub struct Package {
    pub dir: PathBuf,
}

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
) -> Result<Package> {
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_dir = workspace.build_dir().join("watchos").join(format!("{APP_NAME}.app"));

    eprintln!("==> core Rust ({profile}, {TARGET})");
    // `+nightly` and `build-std`: see the header. If this fails because the
    // component is missing, cargo's message already says which one, so it is let
    // through as-is rather than guessed at.
    let mut cargo_args = vec![
        "+nightly",
        "build",
        "-Z",
        "build-std=std,panic_abort",
        "--target",
        TARGET,
        "-p",
        "an-watch",
    ];
    if release {
        cargo_args.push("--release");
    }
    // QuickJS is built with `cc`, which without this uses the SDK's minimum and
    // the Swift link step complains about the mismatch.
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env("WATCHOS_DEPLOYMENT_TARGET", DEPLOYMENT)
        .current_dir(root)
        .status()
        .context("no se pudo ejecutar cargo")?;
    if !status.success() {
        bail!(
            "la compilación del core para watchOS falló. Hace falta nightly con rust-src: \
             rustup toolchain install nightly && rustup component add rust-src --toolchain nightly"
        );
    }

    eprintln!("==> shell SwiftUI");
    let sdk = capture("xcrun", &["--sdk", "watchsimulator", "--show-sdk-path"])?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir)?;

    // `shells/shared` brings what does not depend on the platform —the dev
    // server's client—, compiled by both shells.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/watchos/Sources"))?;
    if sources.is_empty() {
        bail!("no hay fuentes Swift en shells/watchos/Sources");
    }
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    let lib_dir = workspace.target_dir().join(TARGET).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        format!("arm64-apple-watchos{DEPLOYMENT}-simulator"),
        // Without this the `@main` goes unused: swiftc would treat a file as a
        // script.
        "-parse-as-library".into(),
        "-import-objc-header".into(),
        root.join("shells/watchos/Sources/Bridging-Header.h").to_string_lossy().into_owned(),
        "-I".into(),
        root.join("crates/an-watch/include").to_string_lossy().into_owned(),
        "-L".into(),
        lib_dir.to_string_lossy().into_owned(),
        "-lan_watch".into(),
        "-Xclang-linker".into(),
        "-isysroot".into(),
        "-Xclang-linker".into(),
        sdk,
        "-o".into(),
        app_dir.join(APP_NAME).to_string_lossy().into_owned(),
    ];
    if release {
        args.push("-O".into());
    }
    args.extend(sources);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "xcrun", &borrowed, "el enlazado del shell falló")?;

    std::fs::copy(root.join("shells/watchos/Resources/Info.plist"), app_dir.join("Info.plist"))?;
    std::fs::copy(bundle, app_dir.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(app_dir.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(app_dir.join("dev-server.txt"));
        }
    }

    Ok(Package { dir: app_dir })
}

pub fn launch(package: &Package, device: &str) -> Result<()> {
    let udid = find_device(device)?;
    eprintln!("==> simulador: {device}");
    let _ = Command::new("xcrun").args(["simctl", "boot", &udid]).output();
    let _ = Command::new("open")
        .args(["-a", "Simulator", "--args", "-CurrentDeviceUDID", &udid])
        .status();
    // Installing onto a half-booted simulator leaves the command hanging
    // without a word. `bootstatus` waits for the boot to really finish.
    let ready = Command::new("xcrun")
        .args(["simctl", "bootstatus", &udid, "-b"])
        .status()
        .context("no se pudo esperar al arranque del simulador")?;
    if !ready.success() {
        bail!("el simulador {device} no llegó a arrancar");
    }

    // Quit and uninstall before installing: `simctl install` over an app that is
    // already there does not replace the bundle reliably. Both fail if there was
    // nothing, which is the normal case the first time round.
    let _ = Command::new("xcrun").args(["simctl", "terminate", &udid, BUNDLE_ID]).output();
    let _ = Command::new("xcrun").args(["simctl", "uninstall", &udid, BUNDLE_ID]).output();
    let install = Command::new("xcrun")
        .args(["simctl", "install", &udid])
        .arg(&package.dir)
        .status()
        .context("no se pudo instalar la app")?;
    if !install.success() {
        bail!("la instalación en el simulador falló");
    }

    let launch = Command::new("xcrun")
        .args(["simctl", "launch", &udid, BUNDLE_ID])
        .status()
        .context("no se pudo lanzar la app")?;
    if !launch.success() {
        bail!("el lanzamiento falló");
    }
    Ok(())
}

/// Looks a watch up by name and returns its udid.
///
/// The JSON is really parsed, for the same reason as on iOS: `simctl` puts the
/// `udid` *before* the `name`, so grepping for the name and reading the next
/// `udid` gives you the one belonging to the device after it.
fn find_device(name: &str) -> Result<String> {
    let json = capture("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).context("simctl devolvió un JSON que no se entiende")?;
    let runtimes = parsed
        .get("devices")
        .and_then(serde_json::Value::as_object)
        .context("el JSON de simctl no trae dispositivos")?;

    let mut fallback = None;
    for (runtime, devices) in runtimes {
        // Watches only: there are iPhones and iPads with similar names, and
        // putting a watchOS app on an iPhone fails much later and confusingly.
        if !runtime.contains("watchOS") {
            continue;
        }
        for device in devices.as_array().into_iter().flatten() {
            if device.get("name").and_then(serde_json::Value::as_str) != Some(name) {
                continue;
            }
            let Some(udid) = device.get("udid").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if device.get("state").and_then(serde_json::Value::as_str) == Some("Booted") {
                return Ok(udid.to_owned());
            }
            fallback.get_or_insert_with(|| udid.to_owned());
        }
    }
    fallback.with_context(|| format!("no hay ningún simulador de reloj llamado {name:?}"))
}

fn capture(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !output.status.success() {
        bail!("{program} {args:?} falló");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
