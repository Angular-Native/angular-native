//! Arma el APK y lo lleva al emulador.
//!
//! Sin Gradle, por el mismo motivo que iOS va sin `.xcodeproj`: las
//! herramientas del SDK ya hacen todo el trabajo y el proceso cabe en un
//! fichero que se puede leer entero. `javac` compila, `d8` dexa, `aapt2` enlaza
//! el manifiesto, y el resto es un zip firmado.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::workspace::Workspace;

const PACKAGE: &str = "dev.angularnative";
const ACTIVITY: &str = "dev.angularnative.MainActivity";
const ABI: &str = "arm64-v8a";
const RUST_TARGET: &str = "aarch64-linux-android";

pub struct Sdk {
    pub root: PathBuf,
    pub build_tools: PathBuf,
    pub android_jar: PathBuf,
}

impl Sdk {
    pub fn discover() -> Result<Self> {
        let root = std::env::var("ANDROID_HOME")
            .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join("Library/Android/sdk")
            });
        if !root.is_dir() {
            bail!("no encuentro el SDK de Android; define ANDROID_HOME");
        }
        let build_tools = newest_dir(&root.join("build-tools"))
            .context("el SDK no tiene build-tools instaladas")?;
        let platform = newest_dir(&root.join("platforms"))
            .context("el SDK no tiene ninguna plataforma instalada")?;
        let android_jar = platform.join("android.jar");
        if !android_jar.is_file() {
            bail!("no encuentro android.jar en {}", platform.display());
        }
        Ok(Sdk { root, build_tools, android_jar })
    }

    fn tool(&self, name: &str) -> PathBuf {
        self.build_tools.join(name)
    }

    pub fn adb(&self) -> PathBuf {
        self.root.join("platform-tools/adb")
    }
}

/// Devuelve el subdirectorio de nombre más alto, que es la versión más nueva.
fn newest_dir(parent: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    entries.pop()
}

/// Desde el emulador, la máquina anfitriona no es `localhost`.
pub const EMULATOR_HOST: &str = "10.0.2.2";

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
) -> Result<PathBuf> {
    let sdk = Sdk::discover()?;
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let out = root.join("build/android");
    let staging = out.join("apk");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("assets"))?;
    std::fs::create_dir_all(staging.join(format!("lib/{ABI}")))?;

    eprintln!("==> core Rust ({profile}, {ABI})");
    let mut cargo_args = vec!["build", "--target", RUST_TARGET, "-p", "an-android"];
    if release {
        cargo_args.push("--release");
    }
    let status = Command::new("cargo")
        .args(&cargo_args)
        .current_dir(root)
        .status()
        .context("no se pudo ejecutar cargo")?;
    if !status.success() {
        bail!("la compilación del core para Android falló");
    }
    std::fs::copy(
        root.join("target").join(RUST_TARGET).join(profile).join("liban_android.so"),
        staging.join(format!("lib/{ABI}/liban_android.so")),
    )?;
    std::fs::copy(bundle, staging.join("assets/main.js"))?;
    // Los iconos de Material. Van en la app y no en la plataforma porque el
    // juego que trae Android —`android.R.drawable`— está congelado desde 2011
    // por compatibilidad: no es el de Material 3 ni se parece.
    for asset in ["material-symbols.ttf", "material-symbols.codepoints"] {
        std::fs::copy(
            root.join("shells/android/assets").join(asset),
            staging.join("assets").join(asset),
        )
        .with_context(|| format!("no se pudo copiar {asset}"))?;
    }
    if let Some(url) = dev_server {
        std::fs::write(staging.join("assets/dev-server.txt"), url)?;
    }

    eprintln!("==> shell Java");
    let classes = out.join("classes");
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::create_dir_all(&classes)?;
    let sources: Vec<String> = std::fs::read_dir(root.join("shells/android/java/dev/angularnative"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "java"))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    if sources.is_empty() {
        bail!("no hay fuentes Java en shells/android");
    }
    // `--release` en vez de `-source/-target`: con los modernos, javac
    // rechaza `-bootclasspath`, y aquí hace falta compilar contra android.jar
    // y no contra el JDK.
    let mut javac: Vec<String> = vec![
        "-nowarn".into(),
        "-source".into(),
        "17".into(),
        "-target".into(),
        "17".into(),
        "-classpath".into(),
        sdk.android_jar.to_string_lossy().into_owned(),
        "-d".into(),
        classes.to_string_lossy().into_owned(),
    ];
    javac.extend(sources);
    run(root, "javac", &javac, "la compilación del shell Java falló")?;

    eprintln!("==> d8");
    let class_files: Vec<String> = walk(&classes)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "class"))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let mut d8: Vec<String> = vec![
        "--min-api".into(),
        "24".into(),
        "--lib".into(),
        sdk.android_jar.to_string_lossy().into_owned(),
        "--output".into(),
        staging.to_string_lossy().into_owned(),
    ];
    if release {
        d8.push("--release".into());
    }
    d8.extend(class_files);
    run(root, &sdk.tool("d8").to_string_lossy(), &d8, "d8 falló")?;

    eprintln!("==> aapt2 + firma");
    let unsigned = out.join("unsigned.apk");
    run(
        root,
        &sdk.tool("aapt2").to_string_lossy(),
        &[
            "link",
            "-I",
            &sdk.android_jar.to_string_lossy(),
            "--manifest",
            &root.join("shells/android/AndroidManifest.xml").to_string_lossy(),
            "-o",
            &unsigned.to_string_lossy(),
        ],
        "aapt2 link falló",
    )?;

    // `aapt2` solo mete el manifiesto: el dex, la biblioteca nativa y los
    // assets se añaden al zip después, con las rutas que espera Android.
    let mut entries = vec![
        "classes.dex".to_owned(),
        format!("lib/{ABI}/liban_android.so"),
        "assets/main.js".to_owned(),
        "assets/material-symbols.ttf".to_owned(),
        "assets/material-symbols.codepoints".to_owned(),
    ];
    if dev_server.is_some() {
        entries.push("assets/dev-server.txt".to_owned());
    }
    let mut zip_args: Vec<String> = vec!["-q".into(), "-X".into(), unsigned.to_string_lossy().into_owned()];
    zip_args.extend(entries.iter().cloned());
    let status = Command::new("zip")
        .args(&zip_args)
        .current_dir(&staging)
        .status()
        .context("no se pudo ejecutar zip")?;
    if !status.success() {
        bail!("no se pudo empaquetar el APK");
    }

    let aligned = out.join("aligned.apk");
    let _ = std::fs::remove_file(&aligned);
    run(
        root,
        &sdk.tool("zipalign").to_string_lossy(),
        &["-p", "4", &unsigned.to_string_lossy(), &aligned.to_string_lossy()],
        "zipalign falló",
    )?;

    let keystore = debug_keystore()?;
    let apk = out.join("AngularNative.apk");
    let _ = std::fs::remove_file(&apk);
    run(
        root,
        &sdk.tool("apksigner").to_string_lossy(),
        &[
            "sign",
            "--ks",
            &keystore.to_string_lossy(),
            "--ks-pass",
            "pass:android",
            "--key-pass",
            "pass:android",
            "--ks-key-alias",
            "androiddebugkey",
            "--out",
            &apk.to_string_lossy(),
            &aligned.to_string_lossy(),
        ],
        "la firma falló",
    )?;

    let size = std::fs::metadata(&apk)?.len();
    eprintln!("==> {} MB en {}", size / (1024 * 1024), apk.display());
    Ok(apk)
}

/// El almacén de claves de depuración estándar. Si no existe, se crea: es el
/// mismo que genera Android Studio, con la contraseña de siempre.
fn debug_keystore() -> Result<PathBuf> {
    let path = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".android/debug.keystore");
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(path.parent().expect("tiene padre"))?;
    let status = Command::new("keytool")
        .args([
            "-genkeypair",
            "-keystore",
            &path.to_string_lossy(),
            "-storepass",
            "android",
            "-keypass",
            "android",
            "-alias",
            "androiddebugkey",
            "-keyalg",
            "RSA",
            "-keysize",
            "2048",
            "-validity",
            "10000",
            "-dname",
            "CN=Android Debug,O=Android,C=US",
        ])
        .status()
        .context("no se pudo ejecutar keytool")?;
    if !status.success() {
        bail!("no se pudo crear el almacén de claves de depuración");
    }
    Ok(path)
}

pub fn install_and_launch(workspace: &Workspace, apk: &Path) -> Result<()> {
    let sdk = Sdk::discover()?;
    let adb = sdk.adb();
    eprintln!("==> instalando");
    // Mismo motivo que en iOS: instalar sobre una app en marcha no recarga el
    // bundle nuevo.
    let _ = Command::new(&adb).args(["shell", "am", "force-stop", PACKAGE]).output();
    run(
        workspace,
        &adb.to_string_lossy(),
        &["install", "-r", &apk.to_string_lossy()],
        "adb install falló",
    )?;
    run(
        workspace,
        &adb.to_string_lossy(),
        &["shell", "am", "start", "-n", &format!("{PACKAGE}/{ACTIVITY}")],
        "no se pudo lanzar la app",
    )?;
    Ok(())
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

fn run<T: AsRef<str>>(
    cwd: impl AsCwd,
    program: &str,
    args: &[T],
    context: &str,
) -> Result<()> {
    let status = Command::new(program)
        .args(args.iter().map(|a| a.as_ref()))
        .current_dir(cwd.cwd())
        .status()
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}

pub trait AsCwd {
    fn cwd(&self) -> PathBuf;
}

impl AsCwd for &Workspace {
    fn cwd(&self) -> PathBuf {
        self.root.clone()
    }
}

impl AsCwd for &PathBuf {
    fn cwd(&self) -> PathBuf {
        (*self).clone()
    }
}
