//! Arma el `.app` del reloj y lo lleva al simulador de watchOS.
//!
//! Lo mismo que hace `ios.rs` —sin `.xcodeproj`, `swiftc` enlazando contra el
//! staticlib de Rust— con tres diferencias que no son cosméticas:
//!
//! 1. **Hace falta nightly.** `aarch64-apple-watchos-sim` es un target de
//!    nivel 3 y no trae `std` precompilada, así que hay que construirla en el
//!    momento con `-Z build-std`. Es la razón por la que este subcomando llama
//!    a `cargo +nightly` en vez de al `cargo` del `rust-toolchain.toml`.
//! 2. **`-parse-as-library`.** El shell del reloj arranca con `@main` sobre un
//!    `App` de SwiftUI. Sin este flag `swiftc` trata el primer fichero como un
//!    script de nivel superior y `@main` no se usa.
//! 3. **El bundle es de reloj.** `WKApplication` en el `Info.plist` y familia
//!    de dispositivo 4; sin eso `simctl` instala algo que luego no sabe lanzar.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::ios::swift_sources;
use crate::workspace::Workspace;

const APP_NAME: &str = "AngularNativeWatch";
const BUNDLE_ID: &str = "dev.angularnative.playground.watchkitapp";
const TARGET: &str = "aarch64-apple-watchos-sim";
/// watchOS 11 es lo más antiguo donde `@Observable` y las novedades de SwiftUI
/// que usa el shell están disponibles sin `@available` por todas partes.
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
    let app_dir = root.join("build/watchos").join(format!("{APP_NAME}.app"));

    eprintln!("==> core Rust ({profile}, {TARGET})");
    // `+nightly` y `build-std`: ver la cabecera. Si esto falla porque falta el
    // componente, el mensaje de cargo ya dice cuál es y se deja pasar tal cual
    // en vez de adivinar.
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
    // QuickJS se compila con `cc`, que sin esto usa el mínimo del SDK y el
    // enlazado con Swift avisa de la discrepancia.
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

    // `shells/shared` trae lo que no depende de la plataforma —el cliente del
    // servidor de desarrollo—, que compilan los dos shells.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/watchos/Sources"))?;
    if sources.is_empty() {
        bail!("no hay fuentes Swift en shells/watchos/Sources");
    }
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    let lib_dir = root.join("target").join(TARGET).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        format!("arm64-apple-watchos{DEPLOYMENT}-simulator"),
        // Sin esto `@main` no se usa: swiftc trataría un fichero como script.
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
    // Instalar sobre un simulador a medio arrancar deja el comando colgado sin
    // decir nada. `bootstatus` espera a que el arranque termine de verdad.
    let ready = Command::new("xcrun")
        .args(["simctl", "bootstatus", &udid, "-b"])
        .status()
        .context("no se pudo esperar al arranque del simulador")?;
    if !ready.success() {
        bail!("el simulador {device} no llegó a arrancar");
    }

    // Cerrar y desinstalar antes de instalar: `simctl install` sobre una app
    // que ya está no reemplaza el bundle de forma fiable. Los dos fallan si no
    // había nada, que es lo normal la primera vez.
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

/// Busca un reloj por nombre y devuelve su udid.
///
/// Se parsea el JSON de verdad, por lo mismo que en iOS: `simctl` pone el
/// `udid` *antes* que el `name`, así que buscar el nombre a pelo y leer el
/// `udid` siguiente devuelve el del dispositivo de después.
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
        // Solo relojes: hay iPhones y iPads con nombres parecidos, y meter una
        // app de watchOS en un iPhone falla mucho después y de forma confusa.
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
