//! Arma el `.app` de macOS y lo lanza.
//!
//! Lo mismo que hace `ios.rs` —sin `.xcodeproj`, `swiftc` enlazando contra el
//! staticlib de Rust— con tres diferencias, y ninguna es cosmética:
//!
//! 1. **No hay simulador.** Un `.app` de macOS se lanza en la máquina que lo
//!    compiló, así que aquí no hay `simctl`, ni `bootstatus`, ni buscar un
//!    dispositivo por nombre: se abre y ya. Es la única de las cuatro
//!    plataformas de la que se puede hacer una captura sin arrancar nada más.
//! 2. **Hay que terminar la instancia anterior.** En un simulador cada
//!    lanzamiento reemplaza al de antes; aquí, si la app ya está corriendo,
//!    `open` se limita a traer la ventana vieja al frente y parece que el
//!    cambio no ha llegado. Es el mismo fallo que en iOS obliga a desinstalar
//!    antes de instalar, con otra cara.
//! 3. **Todavía no carga plugins.** Igual que el reloj: `an-macos` no tiene el
//!    registro que sí tienen `an-ios` y `an-android`, así que el build se para
//!    y lo dice en vez de dejar un módulo que se traga cada llamada.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::ios::swift_sources;
use crate::plugins::Plugin;
use crate::workspace::Workspace;

const APP_NAME: &str = "AngularNativeMac";
const BUNDLE_ID: &str = "dev.angularnative.playground.mac";
const TARGET: &str = "aarch64-apple-darwin";
/// Sonoma es lo más antiguo donde `NSView.displayLink(target:selector:)`
/// existe, y ese es el reloj que usa el shell: en un Mac con varias pantallas
/// a refrescos distintos, el bueno es el de la pantalla donde está la ventana,
/// y solo la vista lo sabe.
const DEPLOYMENT: &str = "14.0";

pub struct Package {
    pub dir: PathBuf,
}

/// Este host no carga plugins todavía. Se dice aquí y se para: armar el `.app`
/// igualmente dejaría una app en la que el módulo no existe y cada llamada se
/// rechaza en tiempo de ejecución, que es justo lo que este sistema no hace.
pub fn reject_plugins(plugins: &[Plugin]) -> Result<()> {
    if plugins.is_empty() {
        return Ok(());
    }
    let nombres: Vec<&str> = plugins.iter().map(|plugin| plugin.package.as_str()).collect();
    bail!(
        "esta app no se puede compilar para macOS: el host de escritorio todavía no carga \
         plugins, y depende de {}. Ver docs/plugins.md.",
        nombres.join(", ")
    )
}

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
) -> Result<Package> {
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_dir = root.join("build/macos").join(format!("{APP_NAME}.app"));
    // Un `.app` de macOS no es plano como el de iOS: el ejecutable va en
    // `Contents/MacOS`, el `Info.plist` en `Contents` y los recursos en
    // `Contents/Resources`. Poner un `.app` de iPhone en un Mac tal cual da un
    // bundle que Finder abre y `open` rechaza sin decir por qué.
    let contents = app_dir.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");

    eprintln!("==> core Rust ({profile}, {TARGET})");
    let mut cargo_args = vec!["build", "--target", TARGET, "-p", "an-macos"];
    if release {
        cargo_args.push("--release");
    }
    // QuickJS se compila con `cc`, que sin esto usa el mínimo del SDK y el
    // enlazado con Swift avisa de la discrepancia.
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env("MACOSX_DEPLOYMENT_TARGET", DEPLOYMENT)
        .current_dir(root)
        .status()
        .context("no se pudo ejecutar cargo")?;
    if !status.success() {
        bail!("la compilación del core para macOS falló");
    }

    eprintln!("==> shell AppKit");
    let sdk = capture("xcrun", &["--sdk", "macosx", "--show-sdk-path"])?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(&resources)?;

    // `shells/shared` trae lo que no depende de la plataforma —el cliente del
    // servidor de desarrollo—, que compilan los tres shells de Apple.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/macos/Sources"))?;
    if sources.is_empty() {
        bail!("no hay fuentes Swift en shells/macos/Sources");
    }
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    let lib_dir = root.join("target").join(TARGET).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        format!("arm64-apple-macos{DEPLOYMENT}"),
        "-import-objc-header".into(),
        root.join("shells/macos/Sources/Bridging-Header.h").to_string_lossy().into_owned(),
        "-I".into(),
        root.join("crates/an-macos/include").to_string_lossy().into_owned(),
        "-L".into(),
        lib_dir.to_string_lossy().into_owned(),
        "-lan_macos".into(),
        "-Xclang-linker".into(),
        "-isysroot".into(),
        "-Xclang-linker".into(),
        sdk,
        "-o".into(),
        macos_dir.join(APP_NAME).to_string_lossy().into_owned(),
    ];
    if release {
        args.push("-O".into());
    }
    args.extend(sources);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "xcrun", &borrowed, "el enlazado del shell falló")?;

    std::fs::copy(root.join("shells/macos/Resources/Info.plist"), contents.join("Info.plist"))?;
    std::fs::copy(bundle, resources.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(resources.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(resources.join("dev-server.txt"));
        }
    }

    // Sin firmar, macOS mata la app al primer `mmap` de código generado —que es
    // lo que hace QuickJS— con un `Killed: 9` y ninguna explicación. Una firma
    // ad-hoc es suficiente para desarrollo y no necesita cuenta de nadie.
    let signed = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&app_dir)
        .status()
        .context("no se pudo ejecutar codesign")?;
    if !signed.success() {
        bail!("la firma ad-hoc del .app falló");
    }

    Ok(Package { dir: app_dir })
}

pub fn launch(package: &Package) -> Result<()> {
    // Si ya hay una instancia, `open` traería la vieja al frente y el cambio
    // parecería no haber llegado.
    let _ = Command::new("killall").args(["-9", APP_NAME]).output();

    eprintln!("==> lanzando {}", package.dir.display());
    let launched = Command::new("open")
        .arg("-n")
        .arg(&package.dir)
        .status()
        .context("no se pudo lanzar la app")?;
    if !launched.success() {
        bail!("el lanzamiento falló");
    }
    let _ = BUNDLE_ID;
    Ok(())
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
