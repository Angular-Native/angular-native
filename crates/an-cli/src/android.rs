//! Builds the APK and takes it to the emulator.
//!
//! No Gradle, for the same reason iOS goes without an `.xcodeproj`: the SDK's
//! tools already do all the work and the process fits in a file you can read end
//! to end. `javac` compiles, `d8` dexes, `aapt2` links the manifest, and the
//! rest is a signed zip.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::plugins::{self, Platform, Plugin};
use crate::workspace::Workspace;

/// The shell's Java package. It does not change: it is the one in
/// `shells/android/java`, written out in every `package dev.angularnative;`.
/// What does change per project is the application identifier, and that is what
/// `--rename-manifest-package` is for: it rewrites the manifest and leaves the
/// classes where they were.
const PACKAGE: &str = "dev.angularnative";
const ACTIVITY: &str = "dev.angularnative.MainActivity";
const ABI: &str = "arm64-v8a";
const RUST_TARGET: &str = "aarch64-linux-android";

/// Phone or watch. Both are Android and both share a host; what changes is the
/// manifest and which device it gets installed on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Phone,
    Watch,
}

impl Form {
    /// Each form's manifest. It is not one file with conditionals because the
    /// format has none: `aapt2` knows nothing about variants.
    /// The bare name, for finding the one a project from outside puts there.
    fn manifest_name(self) -> &'static str {
        match self {
            Form::Phone => "AndroidManifest.xml",
            Form::Watch => "AndroidManifest.wear.xml",
        }
    }

    fn manifest(self) -> &'static str {
        match self {
            Form::Phone => "shells/android/AndroidManifest.xml",
            Form::Watch => "shells/android/AndroidManifest.wear.xml",
        }
    }

    /// What to call this when it has to be said on screen.
    pub fn label(self) -> &'static str {
        match self {
            Form::Phone => "teléfono",
            Form::Watch => "reloj",
        }
    }
}
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
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Android/sdk")
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
        Ok(Sdk {
            root,
            build_tools,
            android_jar,
        })
    }

    fn tool(&self, name: &str) -> PathBuf {
        self.build_tools.join(name)
    }

    pub fn adb(&self) -> PathBuf {
        self.root.join("platform-tools/adb")
    }
}

/// Returns the subdirectory with the highest name, which is the newest
/// version.
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

/// From inside the emulator, the host machine is not `localhost`.
pub const EMULATOR_HOST: &str = "10.0.2.2";

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
    form: Form,
) -> Result<PathBuf> {
    // Before compiling anything: if some plugin does not bring its Android
    // half, the build stops here and says which one.
    plugins::require(plugins, Platform::Android)?;
    let sdk = Sdk::discover()?;
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_name = workspace.app_name();
    let application_id = application_id(workspace);
    let out = workspace.build_dir().join("android");
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
        workspace
            .target_dir()
            .join(RUST_TARGET)
            .join(profile)
            .join("liban_android.so"),
        staging.join(format!("lib/{ABI}/liban_android.so")),
    )?;
    std::fs::copy(bundle, staging.join("assets/main.js"))?;
    // The Material icons. They go in the app and not in the platform because the
    // set Android ships —`android.R.drawable`— has been frozen since 2011 for
    // compatibility: it is not Material 3's and does not resemble it.
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

    // The Android libraries —Material and everything it drags along— arrive
    // resolved and with their resources already compiled by
    // `scripts/prepare-android-deps.py`. All that happens here is reading the
    // lists.
    let vendor = root.join("vendor/android/build");
    let read_list = |name: &str| -> Vec<String> {
        std::fs::read_to_string(vendor.join(name))
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect()
    };
    let jars = read_list("classpath.txt");
    let resources = read_list("resources.txt");
    let packages = read_list("packages.txt");
    if jars.is_empty() {
        bail!(
            "faltan las dependencias de Android; ejecuta \
             python3 scripts/prepare-android-deps.py"
        );
    }

    // The shell's own resources —the app's theme— are compiled here and not in
    // `prepare-android-deps.py` because they are ours and they change; the
    // libraries' never change, which is why those get cached.
    eprintln!("==> aapt2 compile (recursos del shell)");
    let app_res = out.join("shell-res.zip");
    let _ = std::fs::remove_file(&app_res);
    run(
        root,
        &sdk.tool("aapt2").to_string_lossy(),
        &[
            "compile",
            "--dir",
            &root.join("shells/android/res").to_string_lossy(),
            "-o",
            &app_res.to_string_lossy(),
        ],
        "aapt2 compile de los recursos del shell falló",
    )?;

    // Linking the resources comes before `javac`: this is where the `R` classes
    // the libraries need to find their own resources come from.
    eprintln!("==> aapt2 link");
    let unsigned = out.join("unsigned.apk");
    let generated = out.join("gen");
    let _ = std::fs::remove_dir_all(&generated);
    std::fs::create_dir_all(&generated)?;
    // The project's manifest overrides the shell's if there is one: it is what
    // `an add android` writes, and from then on it belongs to the user. It has
    // to go on declaring the shell's package, because that is where the classes
    // are; the application identifier is put there by
    // `--rename-manifest-package`.
    let manifest = workspace
        .overlay("android", form.manifest_name())
        .unwrap_or_else(|| root.join(form.manifest()));
    check_manifest(&manifest)?;
    // And the one handed to `aapt2` is the project's plus whatever the plugins
    // ask for. The original is left alone: it is the user's.
    let manifest =
        write_manifest(&manifest, &out.join("AndroidManifest.merged.xml"), plugins)?;
    let mut link: Vec<String> = vec![
        "link".into(),
        "-I".into(),
        sdk.android_jar.to_string_lossy().into_owned(),
        "--manifest".into(),
        manifest.to_string_lossy().into_owned(),
        "--java".into(),
        generated.to_string_lossy().into_owned(),
        // Every library wants an `R` class of its own, and its identifiers
        // cannot be constants: they are resolved when the app is linked.
        "--extra-packages".into(),
        packages.join(":"),
        "--non-final-ids".into(),
        // The libraries' resources overlap on purpose —some redefine others'
        // styles— and without this aapt2 takes it for an error.
        "--auto-add-overlay".into(),
        "-o".into(),
        unsigned.to_string_lossy().into_owned(),
    ];
    // In the monorepo the identifier is already the manifest's and there is
    // nothing to rename; renaming it anyway would change the installed package
    // without anybody having asked.
    if application_id != PACKAGE {
        link.push("--rename-manifest-package".into());
        link.push(application_id.clone());
    }
    for resource in &resources {
        link.push("-R".into());
        link.push(resource.clone());
    }
    // The shell's go last: `--auto-add-overlay` lets what is here redefine
    // whatever the libraries bring, and not the other way round.
    link.push("-R".into());
    link.push(app_res.to_string_lossy().into_owned());
    run(
        root,
        &sdk.tool("aapt2").to_string_lossy(),
        &link,
        "aapt2 link falló",
    )?;

    eprintln!("==> shell Java");
    let classes = out.join("classes");
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::create_dir_all(&classes)?;
    let mut sources: Vec<String> =
        std::fs::read_dir(root.join("shells/android/java/dev/angularnative"))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "java"))
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
    if sources.is_empty() {
        bail!("no hay fuentes Java en shells/android");
    }
    // The plugins: their Java sources and the registry that hooks them up. It
    // all goes into the same `javac` invocation as the shell, so a plugin sees
    // `AnPlugin` and `AnPluginCall` with no extra classpath.
    for plugin in plugins {
        let contributed = plugins::sources(plugin, Platform::Android)?;
        eprintln!(
            "==> plugin {} ({} fuentes Java)",
            plugin.module,
            contributed.len()
        );
        sources.extend(contributed);
    }
    sources.push(
        plugins::generate_android(plugins, &out.join("gen-plugins"))?
            .to_string_lossy()
            .into_owned(),
    );
    // `--release` instead of `-source/-target`: with the modern ones javac turns
    // `-bootclasspath` down, and here the compilation has to go against
    // android.jar and not against the JDK.
    let mut classpath = vec![sdk.android_jar.to_string_lossy().into_owned()];
    classpath.extend(jars.iter().cloned());
    let mut javac: Vec<String> = vec![
        "-nowarn".into(),
        "-source".into(),
        "17".into(),
        "-target".into(),
        "17".into(),
        "-classpath".into(),
        classpath.join(":"),
        "-d".into(),
        classes.to_string_lossy().into_owned(),
    ];
    javac.extend(sources);
    // The `R` classes aapt2 has just written, one per package.
    javac.extend(
        walk(&generated)
            .into_iter()
            .filter(|path| path.extension().is_some_and(|e| e == "java"))
            .map(|path| path.to_string_lossy().into_owned()),
    );
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
    // And the libraries: their classes have to end up in the same dex.
    d8.extend(jars.iter().cloned());
    run(root, &sdk.tool("d8").to_string_lossy(), &d8, "d8 falló")?;

    eprintln!("==> firma");
    // `aapt2` only puts the manifest in: the dex, the native library and the
    // assets are added to the zip afterwards, at the paths Android expects.
    // With the Material libraries in there, `d8` splits the dex into several:
    // `classes.dex`, `classes2.dex`… Since API 21 Android loads them all, but
    // they all have to go into the zip.
    let mut dexes: Vec<String> = std::fs::read_dir(&staging)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("classes") && name.ends_with(".dex"))
        .collect();
    dexes.sort();
    if dexes.is_empty() {
        bail!("d8 no dejó ningún .dex");
    }
    let mut entries = dexes;
    entries.extend([
        format!("lib/{ABI}/liban_android.so"),
        "assets/main.js".to_owned(),
        "assets/material-symbols.ttf".to_owned(),
        "assets/material-symbols.codepoints".to_owned(),
    ]);
    if dev_server.is_some() {
        entries.push("assets/dev-server.txt".to_owned());
    }
    let mut zip_args: Vec<String> = vec![
        "-q".into(),
        "-X".into(),
        unsigned.to_string_lossy().into_owned(),
    ];
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
        &[
            "-p",
            "4",
            &unsigned.to_string_lossy(),
            &aligned.to_string_lossy(),
        ],
        "zipalign falló",
    )?;

    let keystore = debug_keystore()?;
    // One name per form: both APKs carry the same package, and if they shared a
    // file, building the watch's would leave the phone's pointing at an APK that
    // is no longer its own.
    let apk = out.join(match form {
        Form::Phone => format!("{app_name}.apk"),
        Form::Watch => format!("{app_name}-wear.apk"),
    });
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

/// The identifier Android installs the app under.
///
/// In a project from outside it is the `app.bundleId`, the same one as on iOS.
/// In the monorepo it is the shell's package as-is: there is no project to
/// consult here and changing it would move the example app already installed in
/// everybody's emulator.
fn application_id(workspace: &Workspace) -> String {
    match &workspace.project {
        Some(project) => project.bundle_id.clone(),
        None => PACKAGE.to_owned(),
    }
}

/// Writes the manifest `aapt2` sees: the project's plus the permissions and
/// features the plugins ask for.
///
/// The file it starts from is never touched. It is the user's —`an add android`
/// writes it and from then on it is theirs—, and a build that edits sources
/// leaves the next person unable to tell what they wrote from what the tool
/// wrote.
///
/// Anything the app already declares is not repeated: `aapt2` accepts two
/// identical `<uses-permission>`s, but a manifest with the same line twice is a
/// manifest nobody can read.
fn write_manifest(base: &Path, destination: &Path, plugins: &[Plugin]) -> Result<PathBuf> {
    let entries = plugins::manifest_entries(plugins)?;
    let text = std::fs::read_to_string(base)
        .with_context(|| format!("no se pudo leer {}", base.display()))?;
    if entries.is_empty() {
        return Ok(base.to_owned());
    }

    let mut lines = String::new();
    let existing_permissions = declared_names(&text, "uses-permission");
    for permission in &entries.permissions {
        if existing_permissions.contains_key(permission) {
            // The app already asks for it. There is nothing to decide: a
            // permission carries no value, so asking twice is asking once.
            continue;
        }
        eprintln!("==> AndroidManifest.xml: uses-permission {permission}");
        lines.push_str(&format!(
            "    <uses-permission android:name=\"{permission}\" />\n"
        ));
    }
    let existing_features = declared_names(&text, "uses-feature");
    for (name, required) in &entries.features {
        if let Some(current) = existing_features.get(name) {
            let declared = current.as_deref().unwrap_or("true");
            if declared != required.to_string() {
                eprintln!(
                    "==> AndroidManifest.xml: {name} ya la declara la app con \
                     android:required=\"{declared}\"; se queda la suya"
                );
            }
            continue;
        }
        eprintln!("==> AndroidManifest.xml: uses-feature {name} (required={required})");
        lines.push_str(&format!(
            "    <uses-feature android:name=\"{name}\" android:required=\"{required}\" />\n"
        ));
    }

    let output = if lines.is_empty() {
        text
    } else {
        // In front of `<application>`, which is where they go in any manifest
        // and where whoever opens it will go looking for them.
        let cut = text.find("<application").with_context(|| {
            format!(
                "{}: no encuentro <application>, y ahí es donde van los permisos",
                base.display()
            )
        })?;
        // Back to the start of its line, so as not to break the indentation.
        let cut = text[..cut]
            .rfind('\n')
            .map(|newline| newline + 1)
            .unwrap_or(cut);
        format!(
            "{}    <!-- De los plugins. Lo escribe `an` al armar el APK. -->\n{}\n{}",
            &text[..cut],
            lines.trim_end(),
            &text[cut..]
        )
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(destination, output)
        .with_context(|| format!("no se pudo escribir {}", destination.display()))?;
    Ok(destination.to_owned())
}

/// The `android:name`s of one kind of manifest element, with their
/// `android:required` if they carry one.
///
/// It goes element by element and not by loose text: a `contains` over the whole
/// line would fail the moment somebody put the attributes in another order or
/// split the element across several lines, and the failure would be a repeated
/// permission, not an error.
fn declared_names(text: &str, element: &str) -> BTreeMap<String, Option<String>> {
    let mut found = BTreeMap::new();
    let opening = format!("<{element}");
    let mut rest = text;
    while let Some(start) = rest.find(&opening) {
        rest = &rest[start + opening.len()..];
        let Some(end) = rest.find('>') else { break };
        let attributes = &rest[..end];
        if let Some(name) = attribute(attributes, "android:name") {
            found.insert(name, attribute(attributes, "android:required"));
        }
        rest = &rest[end..];
    }
    found
}

/// The value of a quoted attribute inside an element.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let start = attributes.find(name)? + name.len();
    let rest = attributes[start..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_owned())
}

/// That the manifest still declares the shell's package.
///
/// If somebody changes it by hand, `javac` compiles just the same —the classes
/// carry their `package` inside them— but Android cannot find the activity and
/// the app does not open. Better to stop it here.
fn check_manifest(manifest: &Path) -> Result<()> {
    let text = std::fs::read_to_string(manifest)
        .with_context(|| format!("no se pudo leer {}", manifest.display()))?;
    if !text.contains(&format!("package=\"{PACKAGE}\"")) {
        bail!(
            "{}: el manifiesto tiene que declarar package=\"{PACKAGE}\", que es donde están \
             las clases del shell.\n\
             El identificador de la aplicación no se pone aquí: sale de app.bundleId en \
             angular-native.json.",
            manifest.display()
        );
    }
    Ok(())
}

/// The standard debug keystore. If it does not exist it is created: it is the
/// same one Android Studio generates, with the usual password.
fn debug_keystore() -> Result<PathBuf> {
    let path =
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".android/debug.keystore");
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(path.parent().expect("it has a parent"))?;
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

fn devices(adb: &Path) -> Result<Vec<(String, Form)>> {
    let output = Command::new(adb)
        .args(["devices"])
        .output()
        .context("no se pudo ejecutar adb devices")?;
    if !output.status.success() {
        bail!("adb devices falló");
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let mut found = Vec::new();
    for line in listing.lines().skip(1) {
        let mut fields = line.split_whitespace();
        let (Some(serial), Some("device")) = (fields.next(), fields.next()) else {
            continue;
        };
        let props = Command::new(adb)
            .args(["-s", serial, "shell", "getprop", "ro.build.characteristics"])
            .output()
            .with_context(|| format!("no se pudo preguntar por {serial}"))?;
        let form = if String::from_utf8_lossy(&props.stdout).contains("watch") {
            Form::Watch
        } else {
            Form::Phone
        };
        found.push((serial.to_owned(), form));
    }
    Ok(found)
}

/// Picks which device the APK goes to.
///
/// Without this, a bare `adb install` refuses to move the moment there is more
/// than one emulator running, and with a phone and a watch at once —which is the
/// normal state of affairs as soon as you work on both— that is always. Worse:
/// if it happened to guess right, the watch's APK would end up on the phone
/// without a word from anyone.
fn pick_device(adb: &Path, form: Form) -> Result<String> {
    let found = devices(adb)?;
    let candidates: Vec<&String> = found
        .iter()
        .filter(|(_, shape)| *shape == form)
        .map(|(serial, _)| serial)
        .collect();
    match candidates.as_slice() {
        [one] => Ok((*one).clone()),
        [] if found.is_empty() => bail!(
            "no hay ningún aparato conectado; arranca un emulador de {} \
             (`emulator -avd <nombre>`)",
            form.label()
        ),
        [] => bail!(
            "no hay ningún aparato con forma de {}; lo que hay es: {}",
            form.label(),
            found
                .iter()
                .map(|(serial, shape)| format!("{serial} ({})", shape.label()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        several => bail!(
            "hay {} aparatos con forma de {}: {}. Elige con --device",
            several.len(),
            form.label(),
            several
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Checks that the device `--device` asked for exists and has the shape of the
/// one being built.
///
/// Letting any old serial through would be worse than not having the option at
/// all: the watch's APK goes onto a phone without complaint, starts and draws,
/// and the only thing it does not do is be a watch app. It is the kind of
/// failure you only see once you publish.
fn check_device(adb: &Path, serial: &str, form: Form) -> Result<String> {
    let found = devices(adb)?;
    match found.iter().find(|(s, _)| s == serial) {
        Some((s, shape)) if *shape == form => Ok(s.clone()),
        Some((_, shape)) => bail!(
            "{serial} tiene forma de {}, y esto es un APK de {}",
            shape.label(),
            form.label()
        ),
        None => bail!(
            "no hay ningún aparato {serial}; lo que hay es: {}",
            if found.is_empty() {
                "nada".to_owned()
            } else {
                found
                    .iter()
                    .map(|(s, shape)| format!("{s} ({})", shape.label()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
    }
}

pub fn install_and_launch(
    workspace: &Workspace,
    apk: &Path,
    form: Form,
    device: Option<&str>,
) -> Result<()> {
    let sdk = Sdk::discover()?;
    let adb = sdk.adb();
    let application_id = application_id(workspace);
    // Which device it goes to, settled before anything is touched. Without the
    // `-s`, `adb` picks for itself: with one it always gets it right, and the
    // moment there are two it refuses to move —or, if the other one is
    // unauthorised, it does not even refuse and sends the watch's APK to the
    // phone.
    let serial = match device {
        Some(asked_for) => check_device(&adb, asked_for, form)?,
        None => pick_device(&adb, form)?,
    };
    eprintln!("==> instalando en {serial}");
    // Same reason as on iOS: installing over a running app does not reload the
    // new bundle.
    let _ = Command::new(&adb)
        .args(["-s", &serial, "shell", "am", "force-stop", &application_id])
        .output();
    run(
        workspace,
        &adb.to_string_lossy(),
        &["-s", &serial, "install", "-r", &apk.to_string_lossy()],
        "adb install falló",
    )?;
    run(
        workspace,
        &adb.to_string_lossy(),
        // The activity keeps the shell's package even when the application is
        // called something else: `--rename-manifest-package` qualifies the class
        // names with the original package.
        &[
            "-s",
            &serial,
            "shell",
            "am",
            "start",
            "-n",
            &format!("{application_id}/{ACTIVITY}"),
        ],
        "no se pudo lanzar la app",
    )?;
    Ok(())
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
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

fn run<T: AsRef<str>>(cwd: impl AsCwd, program: &str, args: &[T], context: &str) -> Result<()> {
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
