//! Arma el `.app` y lo lleva al simulador.
//!
//! Sin `.xcodeproj`: `cargo` compila el core a un staticlib, `swiftc` enlaza el
//! shell contra él, y el bundle se monta a mano. Un proyecto de Xcode aquí solo
//! añadiría un fichero de 2.000 líneas que nadie puede revisar en un diff.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::workspace::Workspace;

const APP_NAME: &str = "AngularNative";
const BUNDLE_ID: &str = "dev.angularnative.playground";
const TARGET: &str = "aarch64-apple-ios-sim";
const DEPLOYMENT: &str = "17.0";

pub struct Package {
    pub dir: PathBuf,
}

/// `dev_server` es la URL del servidor de desarrollo, si lo hay. Se escribe
/// dentro del `.app`: la app la lee al arrancar y, si está, se suscribe a
/// recargas.
pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
) -> Result<Package> {
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_dir = root.join("build/ios").join(format!("{APP_NAME}.app"));

    eprintln!("==> core Rust ({profile})");
    let mut cargo_args = vec!["build", "--target", TARGET, "-p", "an-ios"];
    if release {
        cargo_args.push("--release");
    }
    // QuickJS se compila con `cc`, que sin esto usa el mínimo del SDK y el
    // enlazado con Swift avisa de la discrepancia.
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env("IPHONEOS_DEPLOYMENT_TARGET", DEPLOYMENT)
        .current_dir(root)
        .status()
        .context("no se pudo ejecutar cargo")?;
    if !status.success() {
        bail!("la compilación del core falló");
    }

    eprintln!("==> shell Swift");
    let sdk = capture("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-path"])?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir)?;

    let sources: Vec<String> = std::fs::read_dir(root.join("shells/ios/Sources"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "swift"))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    if sources.is_empty() {
        bail!("no hay fuentes Swift en shells/ios/Sources");
    }

    let lib_dir = root.join("target").join(TARGET).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        format!("arm64-apple-ios{DEPLOYMENT}-simulator"),
        "-import-objc-header".into(),
        root.join("shells/ios/Sources/Bridging-Header.h").to_string_lossy().into_owned(),
        "-I".into(),
        root.join("crates/an-ios/include").to_string_lossy().into_owned(),
        "-L".into(),
        lib_dir.to_string_lossy().into_owned(),
        "-lan_ios".into(),
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

    std::fs::copy(root.join("shells/ios/Resources/Info.plist"), app_dir.join("Info.plist"))?;
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

    // Cerrar antes de instalar. Instalar sobre una app en marcha deja la copia
    // vieja corriendo y el bundle nuevo sin cargar: la app parece no haber
    // cambiado. Falla con "no such process" si no estaba corriendo, que es lo
    // normal, así que se descarta la salida.
    let _ = Command::new("xcrun")
        .args(["simctl", "terminate", &udid, BUNDLE_ID])
        .output();
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

fn find_device(name: &str) -> Result<String> {
    let json = capture("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    // Sin dependencia de JSON: se busca el nombre exacto y se lee el udid que
    // aparece en el mismo objeto, unas líneas más abajo.
    let needle = format!("\"name\" : \"{name}\"");
    let at = json
        .find(&needle)
        .with_context(|| format!("no hay ningún simulador llamado {name:?}"))?;
    let tail = &json[at..];
    let udid_at = tail.find("\"udid\" : \"").context("el simulador no trae udid")?;
    let rest = &tail[udid_at + 10..];
    let end = rest.find('"').context("udid mal formado")?;
    Ok(rest[..end].to_owned())
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
