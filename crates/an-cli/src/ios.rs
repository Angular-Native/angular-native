//! Arma el `.app` de las tres familias de UIKit y lo lleva al simulador.
//!
//! Sin `.xcodeproj`: `cargo` compila el core a un staticlib, `swiftc` enlaza el
//! shell contra él, y el bundle se monta a mano. Un proyecto de Xcode aquí solo
//! añadiría un fichero de 2.000 líneas que nadie puede revisar en un diff.
//!
//! iOS, tvOS y visionOS comparten crate (`an-ios`), shell (`shells/ios`) y
//! superficie C. Lo que cambia entre ellas cabe en `Family`, y es poco: el
//! triple, el SDK, el `Info.plist` y si el `std` de Rust viene hecho o hay que
//! construirlo en el momento. Por eso no hay un `tvos.rs` ni un `visionos.rs`:
//! un fichero por familia serían tres copias del mismo `swiftc`, y la copia es
//! justo lo que se queda atrás el día que alguien arregla algo en una sola.
//!
//! `watchos.rs` sí está aparte, y con motivo: el reloj no tiene `UIView`, así
//! que ni comparte crate ni comparte shell. Aquí no hay nada de eso.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::plugins::{self, Platform, Plugin};
use crate::workspace::Workspace;

/// Las tres familias que se montan sobre `UIView` con marcos absolutos.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Ios,
    TvOs,
    VisionOs,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Ios => "iOS",
            Family::TvOs => "tvOS",
            Family::VisionOs => "visionOS",
        }
    }

    /// El nombre del ejecutable y del `.app`. Uno por familia: los tres se
    /// pueden instalar a la vez en simuladores distintos, y compartir nombre
    /// haría que un `an tvos` pisara el `.app` que acaba de dejar `an ios`.
    fn app_name(self) -> &'static str {
        match self {
            Family::Ios => "AngularNative",
            Family::TvOs => "AngularNativeTV",
            Family::VisionOs => "AngularNativeVision",
        }
    }

    fn bundle_id(self) -> &'static str {
        match self {
            Family::Ios => "dev.angularnative.playground",
            Family::TvOs => "dev.angularnative.playground.tv",
            Family::VisionOs => "dev.angularnative.playground.vision",
        }
    }

    fn target(self) -> &'static str {
        match self {
            Family::Ios => "aarch64-apple-ios-sim",
            Family::TvOs => "aarch64-apple-tvos-sim",
            Family::VisionOs => "aarch64-apple-visionos-sim",
        }
    }

    /// El SDK que le pide `xcrun`.
    fn sdk(self) -> &'static str {
        match self {
            Family::Ios => "iphonesimulator",
            Family::TvOs => "appletvsimulator",
            Family::VisionOs => "xrsimulator",
        }
    }

    /// El triple de `swiftc`, que no es el de Rust. visionOS se sigue llamando
    /// `xros` aquí: el nombre comercial cambió y el del compilador no.
    fn swift_target(self) -> String {
        let version = self.deployment();
        match self {
            Family::Ios => format!("arm64-apple-ios{version}-simulator"),
            Family::TvOs => format!("arm64-apple-tvos{version}-simulator"),
            Family::VisionOs => format!("arm64-apple-xros{version}-simulator"),
        }
    }

    /// La variable que le dice a `cc` —el que compila QuickJS— para qué
    /// versión mínima compila. Sin ella usa el mínimo del SDK y el enlazado
    /// con Swift avisa de la discrepancia.
    fn deployment_env(self) -> &'static str {
        match self {
            Family::Ios => "IPHONEOS_DEPLOYMENT_TARGET",
            Family::TvOs => "TVOS_DEPLOYMENT_TARGET",
            Family::VisionOs => "XROS_DEPLOYMENT_TARGET",
        }
    }

    fn deployment(self) -> &'static str {
        match self {
            // tvOS 17 es contemporáneo de iOS 17 y trae el mismo UIKit.
            Family::Ios | Family::TvOs => "17.0",
            // visionOS empieza en 1.0: no hay versiones anteriores.
            Family::VisionOs => "1.0",
        }
    }

    /// Si el `std` de Rust viene hecho o hay que construirlo en el momento.
    ///
    /// `aarch64-apple-ios-sim` es de nivel 2 y rustup lo trae compilado.
    /// `aarch64-apple-tvos-sim` y `aarch64-apple-visionos-sim` son de nivel 3:
    /// rustup los lista, pero sin `std`. Es lo mismo que ya pasaba con watchOS,
    /// y por eso estas dos llaman a `cargo +nightly`.
    fn needs_build_std(self) -> bool {
        !matches!(self, Family::Ios)
    }

    /// El directorio del `Info.plist`. Es lo único del shell que no se
    /// comparte: las claves que pide cada familia no se parecen.
    fn resources(self) -> &'static str {
        match self {
            Family::Ios => "shells/ios/Resources",
            Family::TvOs => "shells/tvos/Resources",
            Family::VisionOs => "shells/visionos/Resources",
        }
    }

    fn out_dir(self) -> &'static str {
        match self {
            Family::Ios => "build/ios",
            Family::TvOs => "build/tvos",
            Family::VisionOs => "build/visionos",
        }
    }

    /// Lo que tiene que aparecer en el nombre del runtime de `simctl` para que
    /// un dispositivo cuente como de esta familia. visionOS sale como `xrOS`,
    /// igual que en el triple de swiftc.
    fn runtime_marker(self) -> &'static str {
        match self {
            Family::Ios => "iOS",
            Family::TvOs => "tvOS",
            Family::VisionOs => "xrOS",
        }
    }

    /// Lo que hay que ejecutar para tener el runtime del simulador, si falta.
    /// Tener el SDK no basta para arrancar nada: son dos descargas distintas.
    fn download_hint(self) -> &'static str {
        match self {
            Family::Ios => "xcodebuild -downloadPlatform iOS",
            Family::TvOs => "xcodebuild -downloadPlatform tvOS",
            Family::VisionOs => "xcodebuild -downloadPlatform visionOS",
        }
    }
}

pub struct Package {
    pub dir: PathBuf,
    family: Family,
}

/// `dev_server` es la URL del servidor de desarrollo, si lo hay. Se escribe
/// dentro del `.app`: la app la lee al arrancar y, si está, se suscribe a
/// recargas.
pub fn assemble(
    workspace: &Workspace,
    family: Family,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
) -> Result<Package> {
    // Antes de compilar nada: si algún plugin no trae su parte de iOS, el
    // build se para aquí y dice cuál.
    //
    // tvOS y visionOS piden la misma clave, `ios`, y usan las mismas fuentes
    // Swift: es el mismo shell y el mismo protocolo `AnPlugin`. Si ese Swift
    // usa algo que solo existe en el teléfono, el enlazado se para con el
    // error de swiftc, que dice qué símbolo y en qué línea. Se avisa antes
    // para que el error no llegue de sorpresa.
    plugins::require(plugins, Platform::Ios)?;
    if family != Family::Ios && !plugins.is_empty() {
        eprintln!(
            "==> aviso: los plugins se compilan con sus fuentes de iOS, que es lo único que \
             declaran. Si alguna usa API que {} no tiene, el enlazado se para y lo dice.",
            family.label()
        );
    }

    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_name = family.app_name();
    let app_dir = root.join(family.out_dir()).join(format!("{app_name}.app"));

    eprintln!("==> core Rust ({profile}, {})", family.target());
    let mut cargo_args: Vec<&str> = Vec::new();
    if family.needs_build_std() {
        // Ver `needs_build_std`. Si falta el componente, el mensaje de cargo
        // ya dice cuál es y se deja pasar tal cual en vez de adivinar.
        cargo_args.extend(["+nightly", "build", "-Z", "build-std=std,panic_abort"]);
    } else {
        cargo_args.push("build");
    }
    cargo_args.extend(["--target", family.target(), "-p", "an-ios"]);
    if release {
        cargo_args.push("--release");
    }
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env(family.deployment_env(), family.deployment())
        .current_dir(root)
        .status()
        .context("no se pudo ejecutar cargo")?;
    if !status.success() {
        if family.needs_build_std() {
            bail!(
                "la compilación del core para {} falló. Hace falta nightly con rust-src: \
                 rustup toolchain install nightly && \
                 rustup component add rust-src --toolchain nightly",
                family.label()
            );
        }
        bail!("la compilación del core falló");
    }

    eprintln!("==> shell Swift ({})", family.label());
    let sdk = capture("xcrun", &["--sdk", family.sdk(), "--show-sdk-path"]).with_context(|| {
        format!(
            "no está el SDK de {}. Xcode lo instala con: {}",
            family.label(),
            family.download_hint()
        )
    })?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir)?;

    // El shell es el mismo para las tres familias. Lo que cambia entre ellas
    // está dentro, en `#if os(...)`, y son dos cosas: cómo nace la ventana
    // —visionOS no tiene `UIScreen`— y de qué color es el fondo de la raíz.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/ios/Sources"))?;
    if sources.is_empty() {
        bail!("no hay fuentes Swift en shells/ios/Sources");
    }
    // `shells/shared` trae lo que no depende de la plataforma —el cliente del
    // servidor de desarrollo—, que compilan todos los shells.
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    // Los plugins: sus fuentes Swift y el registro que las engancha. Todo va
    // en la misma invocación de `swiftc` que el shell, así que un plugin ve
    // `AnPlugin` y `AnPluginCall` sin importar nada.
    for plugin in plugins {
        let aportadas = plugins::sources(plugin, Platform::Ios)?;
        eprintln!("==> plugin {} ({} fuentes Swift)", plugin.module, aportadas.len());
        sources.extend(aportadas);
    }
    sources.push(
        plugins::generate_ios(plugins, &root.join(family.out_dir()).join("generated"))?
            .to_string_lossy()
            .into_owned(),
    );

    let lib_dir = root.join("target").join(family.target()).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        family.swift_target(),
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
        app_dir.join(app_name).to_string_lossy().into_owned(),
    ];
    if release {
        args.push("-O".into());
    }
    args.extend(sources);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "xcrun", &borrowed, "el enlazado del shell falló")?;

    std::fs::copy(root.join(family.resources()).join("Info.plist"), app_dir.join("Info.plist"))?;
    std::fs::copy(bundle, app_dir.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(app_dir.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(app_dir.join("dev-server.txt"));
        }
    }

    Ok(Package { dir: app_dir, family })
}

/// Los `.swift` de un directorio, en orden estable: `read_dir` los devuelve en
/// el que le dé el sistema de ficheros, y con eso el comando de `swiftc`
/// cambiaría entre máquinas sin que cambie nada.
pub fn swift_sources(dir: &Path) -> Result<Vec<String>> {
    let mut found: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "swift"))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    found.sort();
    Ok(found)
}

pub fn launch(package: &Package, device: &str) -> Result<()> {
    let family = package.family;
    let bundle_id = family.bundle_id();
    let udid = find_device(family, device)?;
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

    // Cerrar y desinstalar antes de instalar.
    //
    // `simctl install` sobre una app que ya está instalada no reemplaza el
    // bundle de forma fiable: la app arranca con el código viejo y parece que
    // el cambio no ha llegado. Desinstalar borra también los datos de la app,
    // lo cual en un ciclo de desarrollo es lo que uno espera de todas formas.
    // Los dos comandos fallan si no había nada, que es lo normal la primera
    // vez, así que se descarta su salida.
    let _ = Command::new("xcrun").args(["simctl", "terminate", &udid, bundle_id]).output();
    let _ = Command::new("xcrun").args(["simctl", "uninstall", &udid, bundle_id]).output();
    let install = Command::new("xcrun")
        .args(["simctl", "install", &udid])
        .arg(&package.dir)
        .status()
        .context("no se pudo instalar la app")?;
    if !install.success() {
        bail!("la instalación en el simulador falló");
    }

    let launch = Command::new("xcrun")
        .args(["simctl", "launch", &udid, bundle_id])
        .status()
        .context("no se pudo lanzar la app")?;
    if !launch.success() {
        bail!("el lanzamiento falló");
    }
    Ok(())
}

/// Busca un simulador de esa familia por nombre y devuelve su udid.
///
/// Se parsea el JSON de verdad. Buscar el nombre a pelo y leer el `udid`
/// siguiente no vale: `simctl` pone el `udid` *antes* que el `name`, así que
/// eso devuelve el del dispositivo de después y se acaba instalando en un
/// simulador que no es, sin que nada falle.
///
/// El runtime se filtra por familia por lo mismo: instalar una app de tvOS en
/// un iPhone falla mucho después y de forma confusa.
fn find_device(family: Family, name: &str) -> Result<String> {
    let json = capture("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).context("simctl devolvió un JSON que no se entiende")?;
    let runtimes = parsed
        .get("devices")
        .and_then(serde_json::Value::as_object)
        .context("el JSON de simctl no trae dispositivos")?;

    // Se prefiere uno ya arrancado: si hay varios con el mismo nombre en
    // distintas versiones del sistema, el que el usuario está mirando es ese.
    let mut fallback = None;
    let mut hay_familia = false;
    for (runtime, devices) in runtimes {
        if !runtime.contains(family.runtime_marker()) {
            continue;
        }
        hay_familia = true;
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
    if let Some(udid) = fallback {
        return Ok(udid);
    }

    // Sin ningún runtime de la familia el problema no es el nombre del
    // dispositivo: es que falta la descarga. Tener el SDK instalado —que es lo
    // que hace falta para compilar y enlazar— no trae el runtime, que es lo
    // que hace falta para ejecutar. Son gigabytes aparte, y decir «no hay
    // ningún simulador llamado X» mandaría a buscar en el sitio equivocado.
    if !hay_familia {
        bail!(
            "no hay ningún runtime de {} instalado, así que no hay simulador que arrancar.\n\
             El `.app` está armado; para poder ejecutarlo:\n    {}",
            family.label(),
            family.download_hint()
        );
    }
    bail!("no hay ningún simulador de {} llamado {name:?}", family.label())
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
