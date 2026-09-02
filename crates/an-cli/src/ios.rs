//! Arma el `.app` y lo lleva al simulador.
//!
//! Sin `.xcodeproj`: `cargo` compila el core a un staticlib, `swiftc` enlaza el
//! shell contra él, y el bundle se monta a mano. Un proyecto de Xcode aquí solo
//! añadiría un fichero de 2.000 líneas que nadie puede revisar en un diff.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::plugins::{self, Platform, Plugin};
use crate::workspace::Workspace;

const TARGET: &str = "aarch64-apple-ios-sim";
const DEPLOYMENT: &str = "17.0";

pub struct Package {
    pub dir: PathBuf,
    /// El identificador con el que `simctl` instala, lanza y desinstala. Sale
    /// del proyecto: dos apps distintas no pueden compartirlo o cada una
    /// desinstalaría a la otra.
    pub bundle_id: String,
}

/// `dev_server` es la URL del servidor de desarrollo, si lo hay. Se escribe
/// dentro del `.app`: la app la lee al arrancar y, si está, se suscribe a
/// recargas.
pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
) -> Result<Package> {
    // Antes de compilar nada: si algún plugin no trae su parte de iOS, el
    // build se para aquí y dice cuál.
    plugins::require(plugins, Platform::Ios)?;
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_name = workspace.app_name();
    let bundle_id = workspace.bundle_id();
    let out = workspace.build_dir().join("ios");
    let app_dir = out.join(format!("{app_name}.app"));
    // El `Info.plist` del proyecto pisa al del shell si lo hay: es lo que
    // escribe `an add ios`, y a partir de ahí es del usuario. Se comprueba antes
    // de compilar nada: son medio minuto de `cargo` y de `swiftc` que no hay por
    // qué gastar para acabar diciendo que el nombre no cuadra.
    let plist = workspace
        .overlay("ios", "Info.plist")
        .unwrap_or_else(|| root.join("shells/ios/Resources/Info.plist"));
    comprobar_plist(&plist, &app_name, &bundle_id)?;

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

    // `shells/shared` trae lo que no depende de la plataforma —el cliente del
    // servidor de desarrollo—, que compilan los dos shells.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/ios/Sources"))?;
    if sources.is_empty() {
        bail!("no hay fuentes Swift en shells/ios/Sources");
    }
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
        plugins::generate_ios(plugins, &out.join("generated"))?
            .to_string_lossy()
            .into_owned(),
    );

    let lib_dir = workspace.target_dir().join(TARGET).join(profile);
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
        app_dir.join(&app_name).to_string_lossy().into_owned(),
    ];
    if release {
        args.push("-O".into());
    }
    args.extend(sources);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "xcrun", &borrowed, "el enlazado del shell falló")?;

    std::fs::copy(&plist, app_dir.join("Info.plist"))?;
    std::fs::copy(bundle, app_dir.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(app_dir.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(app_dir.join("dev-server.txt"));
        }
    }

    Ok(Package { dir: app_dir, bundle_id })
}

/// Que el `Info.plist` diga lo mismo que el proyecto.
///
/// `CFBundleExecutable` tiene que ser el nombre del binario que se acaba de
/// enlazar, y `CFBundleIdentifier` el mismo con el que luego se instala. Si
/// alguien cambia `app.name` en `angular-native.json` y no toca el plist, la
/// app se instala y al abrirla desaparece sin decir nada: iOS busca un
/// ejecutable que no está. Es exactamente el fallo silencioso que no puede
/// pasar, así que se compara aquí.
fn comprobar_plist(plist: &Path, app_name: &str, bundle_id: &str) -> Result<()> {
    for (clave, esperado) in
        [("CFBundleExecutable", app_name), ("CFBundleIdentifier", bundle_id)]
    {
        let leido = capture(
            "plutil",
            &["-extract", clave, "raw", "-o", "-", &plist.to_string_lossy()],
        )
        .with_context(|| format!("{}: no se pudo leer {clave}", plist.display()))?;
        if leido != esperado {
            bail!(
                "{}: {clave} es {leido:?} y el proyecto dice {esperado:?}.\n\
                 O se corrige el plist, o se corrige angular-native.json; \
                 con los dos distintos la app se instala y no abre.",
                plist.display()
            );
        }
    }
    Ok(())
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

    // Cerrar y desinstalar antes de instalar.
    //
    // `simctl install` sobre una app que ya está instalada no reemplaza el
    // bundle de forma fiable: la app arranca con el código viejo y parece que
    // el cambio no ha llegado. Desinstalar borra también los datos de la app,
    // lo cual en un ciclo de desarrollo es lo que uno espera de todas formas.
    // Los dos comandos fallan si no había nada, que es lo normal la primera
    // vez, así que se descarta su salida.
    let _ = Command::new("xcrun")
        .args(["simctl", "terminate", &udid, &package.bundle_id])
        .output();
    let _ = Command::new("xcrun")
        .args(["simctl", "uninstall", &udid, &package.bundle_id])
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
        .args(["simctl", "launch", &udid, &package.bundle_id])
        .status()
        .context("no se pudo lanzar la app")?;
    if !launch.success() {
        bail!("el lanzamiento falló");
    }
    Ok(())
}

/// Busca un simulador por nombre y devuelve su udid.
///
/// Se parsea el JSON de verdad. Buscar el nombre a pelo y leer el `udid`
/// siguiente no vale: `simctl` pone el `udid` *antes* que el `name`, así que
/// eso devuelve el del dispositivo de después y se acaba instalando en un
/// simulador que no es, sin que nada falle.
fn find_device(name: &str) -> Result<String> {
    let json = capture("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).context("simctl devolvió un JSON que no se entiende")?;
    let runtimes = parsed
        .get("devices")
        .and_then(serde_json::Value::as_object)
        .context("el JSON de simctl no trae dispositivos")?;

    // Se prefiere uno ya arrancado: si hay varios con el mismo nombre en
    // distintas versiones de iOS, el que el usuario está mirando es ese.
    let mut fallback = None;
    for devices in runtimes.values() {
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
    fallback.with_context(|| format!("no hay ningún simulador llamado {name:?}"))
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
