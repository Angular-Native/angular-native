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
use crate::signing;
use crate::workspace::{Appearance, Workspace};

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
            Form::Phone => "phone",
            Form::Watch => "watch",
        }
    }
}
/// What comes out of the build and what signs it.
///
/// Three flags that used to be one `Form` argument, kept together because they
/// are decided at the same moment and read as a sentence at the call site:
/// a phone APK signed with the debug key, a watch APK, a phone bundle signed
/// with the release key.
pub struct Packaging<'a> {
    pub form: Form,
    /// The release keystore, or `None` for the debug one — the same one Android
    /// Studio generates, which no store accepts.
    pub signing: Option<&'a signing::Android>,
    /// An `.aab` instead of an `.apk`. Google Play has taken nothing else since
    /// August 2021.
    pub aab: bool,
    /// Where `bundletool` is, when `aab` is on. It is looked up before anything
    /// is compiled —see [`bundletool`]— because it is not part of the Android
    /// SDK and its absence is the one thing here that a two-minute build cannot
    /// fix.
    pub bundletool: Option<PathBuf>,
}

impl Packaging<'_> {
    /// Debug builds go to a device; a release-signed one is an artefact for a
    /// store, and it is what the name of the file has to say.
    fn suffix(&self) -> &'static str {
        match self.signing {
            Some(_) => "-release",
            None => "",
        }
    }
}

pub struct Sdk {
    pub root: PathBuf,
    pub build_tools: PathBuf,
    pub android_jar: PathBuf,
    /// The `toolchains/llvm/prebuilt/<host>` directory inside the NDK: the
    /// clang, the archiver and the sysroot that cross-compile for Android.
    ///
    /// `None` when there is no NDK. That is not fatal here — everything except
    /// the Rust core builds without one, and saying so where the core is built
    /// is better than refusing a command that would have worked.
    pub ndk_toolchain: Option<PathBuf>,
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
            bail!("I cannot find the Android SDK; set ANDROID_HOME");
        }
        let build_tools = newest_dir(&root.join("build-tools"))
            .context("the SDK has no build-tools installed")?;
        let platform = newest_dir(&root.join("platforms"))
            .context("the SDK has no platform installed")?;
        let android_jar = platform.join("android.jar");
        if !android_jar.is_file() {
            bail!("there is no android.jar in {}", platform.display());
        }
        let ndk_toolchain = find_ndk_toolchain(&root);
        Ok(Sdk {
            root,
            build_tools,
            android_jar,
            ndk_toolchain,
        })
    }

    /// What `cargo` needs in its environment to cross-compile for Android.
    ///
    /// This used to live in `.cargo/config.toml`, committed, with one
    /// machine's home directory, one NDK version and one host operating system
    /// written into every line of it. It worked for exactly the person who
    /// wrote it: anybody else got `cc` looking for an
    /// `aarch64-linux-android-clang` that does not exist, and QuickJS never
    /// built.
    ///
    /// None of the three is knowable at commit time and all three are knowable
    /// here, so they are worked out and handed to the `cargo` this spawns.
    /// What stays in `.cargo/config.toml` is the alias, which is the only part
    /// of it that belongs to the repository rather than to a laptop.
    pub fn cargo_env(&self) -> Vec<(String, String)> {
        let Some(toolchain) = &self.ndk_toolchain else {
            return Vec::new();
        };
        let bin = toolchain.join("bin");
        let sysroot = toolchain.join("sysroot");
        let mut env = Vec::new();
        // Two ABIs, and the compiler's name is not the target triple in either
        // of them: the NDK spells the 32-bit one `armv7a-linux-androideabi`
        // where Rust says `armv7-linux-androideabi`, and both carry the API
        // level in the middle of the file name.
        for (triple, compiler) in [
            ("aarch64-linux-android", format!("aarch64-linux-android{ANDROID_API}-clang")),
            ("armv7-linux-androideabi", format!("armv7a-linux-androideabi{ANDROID_API}-clang")),
        ] {
            let cc = bin.join(&compiler).to_string_lossy().into_owned();
            let upper = triple.to_uppercase().replace('-', "_");
            // The triple exactly as Rust writes it, hyphens and all. `cc`
            // would take either form; bindgen takes only this one, and a
            // sysroot it does not receive is Apple's clang being asked to
            // compile for Android and failing to find `stdio.h`.
            //
            // That has a consequence for `an env android`, which prints this
            // same list: a shell cannot `export` a name with hyphens in it —
            // `export CC_aarch64-linux-android=…` is not a valid identifier —
            // so what it prints is an `env` command prefix, which takes
            // `NAME=VALUE` as arguments and does not care.
            //
            // `[target.X] linker` in the old file was this variable.
            env.push((format!("CARGO_TARGET_{upper}_LINKER"), cc.clone()));
            env.push((format!("CC_{triple}"), cc));
            env.push((
                format!("AR_{triple}"),
                bin.join("llvm-ar").to_string_lossy().into_owned(),
            ));
            // bindgen uses libclang, which by default is Xcode's and knows
            // nothing about Android: without the NDK sysroot it cannot even
            // find <stdint.h>.
            let ndk_target = compiler.trim_end_matches("-clang").to_owned();
            env.push((
                format!("BINDGEN_EXTRA_CLANG_ARGS_{triple}"),
                format!("--sysroot={} --target={ndk_target}", sysroot.display()),
            ));
        }
        env
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
/// The oldest API level NDK 27 supports, and the one the core is built against.
const ANDROID_API: &str = "24";

/// The NDK's toolchain directory, wherever it is on this machine.
///
/// Three things vary and all three used to be written out in a committed file:
/// where the NDK is, which version of it, and what the host is called. The
/// first two are `ANDROID_NDK_HOME` or the newest under `<sdk>/ndk`; the third
/// is read rather than guessed, because `prebuilt` holds exactly one directory
/// and its name is `darwin-x86_64` on a Mac and `linux-x86_64` on Linux —
/// including on Apple silicon, where the NDK still ships the x86_64 name and
/// runs under Rosetta.
fn find_ndk_toolchain(sdk: &Path) -> Option<PathBuf> {
    let ndk = std::env::var("ANDROID_NDK_HOME")
        .or_else(|_| std::env::var("NDK_HOME"))
        .map(PathBuf::from)
        .ok()
        .filter(|path| path.is_dir())
        .or_else(|| newest_dir(&sdk.join("ndk")))?;
    let prebuilt = ndk.join("toolchains/llvm/prebuilt");
    // One entry, whatever it is called here.
    std::fs::read_dir(&prebuilt)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.join("bin").is_dir())
}

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

/// Opens the dev server's port on the device, back towards this machine.
///
/// It replaces a special case rather than adding one. The address used to be
/// `10.0.2.2`, which is the host machine seen from inside the Android
/// emulator and means nothing anywhere else: baked into the APK, hot refresh
/// reached an emulator and silently never arrived on a phone plugged in over
/// USB — the client polled an address that was not there and said nothing,
/// because nothing had failed.
///
/// `adb reverse` works on both, so every Android target now uses the same
/// `127.0.0.1` the Mac and the simulators use, and the translation is gone.
///
/// It is not fatal. A device that refuses it — an old `adb`, a reverse already
/// held by something else — still gets the app; what it does not get is hot
/// refresh, and that is said here rather than discovered as a save that never
/// arrives.
fn reverse_dev_port(adb: &Path, serial: &str, port: u16) -> Result<()> {
    let spec = format!("tcp:{port}");
    let output = Command::new(adb)
        .args(["-s", serial, "reverse", &spec, &spec])
        .output()
        .context("adb could not be run")?;
    if output.status.success() {
        eprintln!("==> adb reverse {spec} (the dev server, from the device)");
        return Ok(());
    }
    eprintln!(
        "==> warning: `adb reverse {spec}` was turned down by {serial}, so the app will not \
         reach the development server and saving will change nothing on screen.\n    {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
    packaging: Packaging<'_>,
) -> Result<PathBuf> {
    let form = packaging.form;
    // Before compiling anything: if some plugin does not bring its Android
    // half, the build stops here and says which one.
    plugins::require(plugins, Platform::Android, &std::collections::BTreeSet::new())?;
    let sdk = Sdk::discover()?;
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_name = workspace.app_name();
    let application_id = application_id(workspace);
    // The project's manifest overrides the shell's if there is one: it is what
    // `an add android` writes, and from then on it belongs to the user. It has
    // to go on declaring the shell's package, because that is where the classes
    // are; the application identifier is put there by
    // `--rename-manifest-package`.
    let manifest = workspace
        .overlay("android", form.manifest_name())
        .unwrap_or_else(|| root.join(form.manifest()));
    // Up here with the plugin coverage and not down beside the `aapt2` line
    // that consumes it: reading a file costs nothing, and everything below is
    // a cargo build and a resource compilation. It used to run after both,
    // which made "before anything is built" a comment rather than a fact.
    check_manifest(&manifest, packaging.aab, &application_id)?;

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
    if sdk.ndk_toolchain.is_none() {
        bail!(
            "the Rust core cannot be cross-compiled without the NDK, and there is none under \
             {}/ndk.\n\
             Install it —`sdkmanager \"ndk;27.1.12297006\"`, or Android Studio's SDK Manager— \
             or point ANDROID_NDK_HOME at one you already have.",
            sdk.root.display()
        );
    }
    let status = Command::new("cargo")
        .args(&cargo_args)
        // Where the NDK is, which version and what this host is called: three
        // things that used to be written out in a committed `.cargo/config.toml`
        // and are worked out here instead. See `Sdk::cargo_env`.
        .envs(sdk.cargo_env())
        .current_dir(root)
        .status()
        .context("cargo could not be run")?;
    if !status.success() {
        bail!("the core build for Android failed");
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
        .with_context(|| format!("{asset} could not be copied"))?;
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
            "the Android dependencies are missing; run \
             python3 scripts/prepare-android-deps.py"
        );
    }

    // The shell's own resources —the app's theme— are compiled here and not in
    // `prepare-android-deps.py` because they are ours and they change; the
    // libraries' never change, which is why those get cached.
    eprintln!("==> aapt2 compile (the shell's resources)");
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
        "aapt2 compile of the shell's resources failed",
    )?;

    // Linking the resources comes before `javac`: this is where the `R` classes
    // the libraries need to find their own resources come from.
    eprintln!("==> aapt2 link");
    let unsigned = out.join("unsigned.apk");
    let generated = out.join("gen");
    let _ = std::fs::remove_dir_all(&generated);
    std::fs::create_dir_all(&generated)?;
    // And the one handed to `aapt2` is the project's plus whatever the plugins
    // ask for. The original is left alone: it is the user's.
    let manifest = write_manifest(
        &manifest,
        &out.join("AndroidManifest.merged.xml"),
        plugins,
        &application_id,
        workspace.appearance(),
    )?;
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
    if packaging.aab {
        // A bundle's manifest and resources are protobuf, not the binary XML an
        // APK carries. `bundletool` refuses a module built the other way with a
        // message about a "proto" it does not explain, and it is aapt2 —not
        // bundletool— that can produce them.
        link.push("--proto-format".into());
    }
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
        "aapt2 link failed",
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
        bail!("there are no Java sources in shells/android");
    }
    // The plugins: their Java sources and the registry that hooks them up. It
    // all goes into the same `javac` invocation as the shell, so a plugin sees
    // `AnPlugin` and `AnPluginCall` with no extra classpath.
    for plugin in plugins {
        let contributed = plugins::sources(plugin, Platform::Android)?;
        eprintln!(
            "==> plugin {} ({} Java sources)",
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
    run(root, "javac", &javac, "the Java shell build failed")?;

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
    run(root, &sdk.tool("d8").to_string_lossy(), &d8, "d8 failed")?;

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
        bail!("d8 left no .dex behind");
    }

    // From here the two artefacts part company: a bundle is not a signed APK
    // with a different extension, it is a different layout that a different
    // tool assembles and a different tool signs.
    if let Some(bundletool) = &packaging.bundletool {
        let android = packaging.signing.expect("--aab always resolves a release keystore");
        return build_aab(
            workspace, &out, &staging, &unsigned, &dexes, bundletool, android, &app_name,
            packaging.suffix(),
        );
    }

    eprintln!("==> packing the APK");
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
        .context("zip could not be run")?;
    if !status.success() {
        bail!("the APK could not be packed");
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
        "zipalign failed",
    )?;

    // One name per form and per key: both APKs carry the same package, and if
    // they shared a file, building the watch's would leave the phone's pointing
    // at an APK that is no longer its own — and a release build overwriting the
    // debug one is how the wrong artefact reaches a store.
    let suffix = packaging.suffix();
    let apk = out.join(match form {
        Form::Phone => format!("{app_name}{suffix}.apk"),
        Form::Watch => format!("{app_name}-wear{suffix}.apk"),
    });
    let _ = std::fs::remove_file(&apk);
    sign_apk(&sdk, &aligned, &apk, packaging.signing)?;

    let size = std::fs::metadata(&apk)?.len();
    eprintln!("==> {} MB in {}", size / (1024 * 1024), apk.display());
    Ok(apk)
}

/// Signs the aligned APK, with the release keystore if there is one and with the
/// debug one if there is not.
///
/// The passwords go to `apksigner` through the **environment** and not on the
/// command line. `--ks-pass pass:hunter2` puts the keystore password in the
/// process table, where every other user on the machine can read it with `ps`,
/// and into the shell history of anybody who copies the command out of a log.
/// `env:NAME` is the form apksigner offers for exactly this, and the variable is
/// set on the child alone.
fn sign_apk(
    sdk: &Sdk,
    aligned: &Path,
    apk: &Path,
    android: Option<&signing::Android>,
) -> Result<()> {
    let (keystore, alias, store_password, key_password) = match android {
        Some(android) => (
            android.keystore.clone(),
            android.key_alias.clone(),
            android.store_password.clone(),
            android.key_password.clone(),
        ),
        None => (
            debug_keystore()?,
            "androiddebugkey".to_owned(),
            "android".to_owned(),
            "android".to_owned(),
        ),
    };
    eprintln!(
        "==> apksigner ({})",
        match android {
            Some(_) => "release keystore",
            None => "debug keystore — no store accepts this",
        }
    );
    let signed = Command::new(sdk.tool("apksigner"))
        .args([
            "sign",
            "--ks",
            &keystore.to_string_lossy(),
            "--ks-pass",
            "env:AN_KS_PASS",
            "--key-pass",
            "env:AN_KEY_PASS",
            "--ks-key-alias",
            &alias,
            "--out",
            &apk.to_string_lossy(),
            &aligned.to_string_lossy(),
        ])
        .env("AN_KS_PASS", &store_password)
        .env("AN_KEY_PASS", &key_password)
        .output()
        .context("apksigner could not be run")?;
    if signed.status.success() {
        return Ok(());
    }
    // apksigner reports a wrong password as a Java exception about a MAC check
    // that mentions neither the keystore nor the password. Everybody who has
    // ever seen it had to look it up.
    let said = String::from_utf8_lossy(&signed.stderr);
    let hint = if said.contains("password was incorrect") || said.contains("mac check failed") {
        "\nThe keystore password is wrong. It is read from the environment variable \
         named in signing.android.storePasswordEnv."
    } else if said.contains("No key with alias") || said.contains("does not contain a key") {
        "\nThe alias is not in that keystore. `keytool -list -keystore <file>` prints \
         the aliases it holds."
    } else {
        ""
    };
    bail!(
        "apksigner could not sign {} with {}.\n{}{hint}\nSee {}",
        apk.display(),
        keystore.display(),
        said.trim(),
        signing::DOCS
    )
}

// ---------------------------------------------------------------------------
// The Android App Bundle
// ---------------------------------------------------------------------------

/// Puts the `.aab` together and signs it.
///
/// A bundle is not an APK. It is a zip of *modules*, each one a zip with its own
/// layout —`manifest/`, `dex/`, `res/`, `lib/`, `assets/`, `resources.pb`— and
/// with the manifest and the resources in protobuf rather than in binary XML.
/// There is one module here, `base`, because there is no dynamic feature to
/// split off, and there will not be one until something asks for it.
///
/// It is signed with `jarsigner` and not with `apksigner`: a bundle is a signed
/// jar, and apksigner refuses it. Play re-signs the APKs it generates from this
/// with its own key anyway — what is signed here is the upload, which is how
/// Play knows the bundle came from you.
#[allow(clippy::too_many_arguments)]
fn build_aab(
    workspace: &Workspace,
    out: &Path,
    staging: &Path,
    proto_apk: &Path,
    dexes: &[String],
    bundletool: &Path,
    android: &signing::Android,
    app_name: &str,
    suffix: &str,
) -> Result<PathBuf> {
    eprintln!("==> the base module");
    let module = out.join("bundle/base");
    let _ = std::fs::remove_dir_all(out.join("bundle"));
    std::fs::create_dir_all(module.join("manifest"))?;
    std::fs::create_dir_all(module.join("dex"))?;

    // What aapt2 wrote in proto form: the manifest and the compiled resources,
    // which have to be taken out of the linked APK and put where a module keeps
    // them.
    let extracted = out.join("bundle/linked");
    std::fs::create_dir_all(&extracted)?;
    run(
        &extracted,
        "unzip",
        &["-q", "-o", &proto_apk.to_string_lossy()],
        "the linked resources could not be unpacked",
    )?;
    std::fs::rename(
        extracted.join("AndroidManifest.xml"),
        module.join("manifest/AndroidManifest.xml"),
    )
    .context("aapt2 --proto-format left no AndroidManifest.xml")?;
    let resources = extracted.join("resources.pb");
    if resources.is_file() {
        std::fs::rename(resources, module.join("resources.pb"))?;
    }
    if extracted.join("res").is_dir() {
        std::fs::rename(extracted.join("res"), module.join("res"))?;
    }

    for dex in dexes {
        std::fs::copy(staging.join(dex), module.join("dex").join(dex))?;
    }
    for directory in ["lib", "assets"] {
        if staging.join(directory).is_dir() {
            copy_tree(&staging.join(directory), &module.join(directory))?;
        }
    }

    // Zipped with no compression: bundletool recompresses everything anyway, and
    // a `.so` stored twice is the difference between a bundle that opens in a
    // second and one that takes ten.
    let module_zip = out.join("bundle/base.zip");
    let _ = std::fs::remove_file(&module_zip);
    run(
        &module,
        "zip",
        &["-q", "-X", "-r", "-0", &module_zip.to_string_lossy(), "."],
        "the base module could not be packed",
    )?;

    let aab = out.join(format!("{app_name}{suffix}.aab"));
    let _ = std::fs::remove_file(&aab);
    eprintln!("==> bundletool build-bundle");
    let built = Command::new("java")
        .arg("-jar")
        .arg(bundletool)
        .args([
            "build-bundle",
            &format!("--modules={}", module_zip.display()),
            &format!("--output={}", aab.display()),
        ])
        .current_dir(&workspace.root)
        .output()
        .context("java could not be run; bundletool is a jar and needs a JDK")?;
    if !built.status.success() {
        bail!(
            "bundletool could not build {}.\n{}\nSee {}",
            aab.display(),
            String::from_utf8_lossy(&built.stderr).trim(),
            signing::DOCS
        );
    }

    // And signed as the jar it is. `-storepass:env` for the same reason
    // apksigner gets `env:`: a password on a command line is a password in the
    // process table.
    eprintln!("==> jarsigner (upload key)");
    let signed = Command::new("jarsigner")
        .args([
            "-verbose:false",
            "-sigalg",
            "SHA256withRSA",
            "-digestalg",
            "SHA-256",
            "-keystore",
            &android.keystore.to_string_lossy(),
            "-storepass:env",
            "AN_KS_PASS",
            "-keypass:env",
            "AN_KEY_PASS",
            &aab.to_string_lossy(),
            &android.key_alias,
        ])
        .env("AN_KS_PASS", &android.store_password)
        .env("AN_KEY_PASS", &android.key_password)
        .output()
        .context("jarsigner could not be run")?;
    if !signed.status.success() {
        bail!(
            "jarsigner could not sign {} with {}.\n{}\n\
             A bundle is signed as a jar, so it is jarsigner and not apksigner that \
             does it; the keystore and the alias are the same ones.\nSee {}",
            aab.display(),
            android.keystore.display(),
            String::from_utf8_lossy(&signed.stderr).trim(),
            signing::DOCS
        );
    }

    let size = std::fs::metadata(&aab)?.len();
    eprintln!("==> {} MB in {}", size / (1024 * 1024), aab.display());
    Ok(aab)
}

/// Where `bundletool` is.
///
/// It is **not** part of the Android SDK: Gradle downloads it as a dependency,
/// and there is no Gradle here. Three places are looked at, in this order, and
/// if none of them has it the message says all three and how to get it — because
/// "bundletool: command not found" sends people to a package manager that does
/// not carry it.
pub fn bundletool(workspace: &Workspace) -> Result<PathBuf> {
    if let Some(given) = std::env::var_os("AN_BUNDLETOOL") {
        let path = PathBuf::from(given);
        if path.is_file() {
            return Ok(path);
        }
        bail!(
            "AN_BUNDLETOOL points at {}, and there is no file there.\nSee {}",
            path.display(),
            signing::DOCS
        );
    }
    let vendored = workspace.root.join("vendor/android/tools/bundletool.jar");
    if vendored.is_file() {
        return Ok(vendored);
    }
    bail!(
        "`--aab` needs bundletool, and it is not part of the Android SDK: Gradle \
         downloads it as a dependency, and there is no Gradle here.\n\n\
         Get it once —it is a single jar, about 25 MB:\n\n\
         \x20   python3 scripts/fetch-android-deps.py\n\n\
         which leaves it in {}. Or point AN_BUNDLETOOL at a copy you already have.\n\
         `an android --sign` with no `--aab` needs none of this: an APK is signed by \
         apksigner, which does come with the SDK.\nSee {}",
        vendored.display(),
        signing::DOCS
    )
}

/// A directory, copied. `ditto` keeps the permission bits on the `.so`, which a
/// naive copy does not, and a `liban_android.so` that arrives without its
/// executable bit is a bundle Play accepts and a device refuses to start.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let copied = Command::new("ditto")
        .arg(from)
        .arg(to)
        .status()
        .context("ditto could not be run")?;
    if !copied.success() {
        bail!("{} could not be copied to {}", from.display(), to.display());
    }
    Ok(())
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
fn write_manifest(
    base: &Path,
    destination: &Path,
    plugins: &[Plugin],
    application_id: &str,
    appearance: Appearance,
) -> Result<PathBuf> {
    let entries = plugins::manifest_entries(plugins)?;
    let text = std::fs::read_to_string(base)
        .with_context(|| format!("{} could not be read", base.display()))?;
    let text = substitute_application_id(&text, application_id);
    // What the app looks like, in the one place an Android app can be told
    // something at build time and read it at run time. It goes in always —
    // there is no "no appearance", the default is to follow the device — so
    // unlike the plugin entries it is not a reason to skip the copy, it is one
    // to always make it.
    let text = with_appearance(&text, appearance)?;

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
                    "==> AndroidManifest.xml: {name} is already declared by the app with \
                     android:required=\"{declared}\"; the app's one stays"
                );
            }
            continue;
        }
        eprintln!("==> AndroidManifest.xml: uses-feature {name} (required={required})");
        lines.push_str(&format!(
            "    <uses-feature android:name=\"{name}\" android:required=\"{required}\" />\n"
        ));
    }

    // A service goes *inside* `<application>` and not before it, so it is built
    // separately and injected at the other end.
    let mut services = String::new();
    for (name, attributes) in &entries.services {
        if text.contains(&format!("android:name=\"{name}\"")) {
            eprintln!("==> AndroidManifest.xml: {name} is already declared by the app; the app's one stays");
            continue;
        }
        eprintln!("==> AndroidManifest.xml: service {name}");
        services.push_str(&format!("        <service android:name=\"{name}\""));
        for (key, value) in attributes {
            services.push_str(&format!(" {key}=\"{value}\""));
        }
        services.push_str(" />\n");
    }

    let text = if services.is_empty() {
        text
    } else {
        // Immediately after the `<application …>` opening tag: a service is a
        // child of it, and putting it beside the permissions would produce a
        // manifest `aapt2` refuses.
        let open = text.find("<application").with_context(|| {
            format!(
                "{}: I cannot find <application>, and that is what a service goes inside",
                base.display()
            )
        })?;
        let close = text[open..].find('>').map(|at| open + at + 1).with_context(|| {
            format!("{}: <application> is never closed", base.display())
        })?;
        format!(
            "{}\n        <!-- From the plugins. Written by `an` when it puts the APK together. -->\n{}{}",
            &text[..close],
            services.trim_end(),
            &text[close..]
        )
    };

    let output = if lines.is_empty() {
        text
    } else {
        // In front of `<application>`, which is where they go in any manifest
        // and where whoever opens it will go looking for them.
        let cut = text.find("<application").with_context(|| {
            format!(
                "{}: I cannot find <application>, and that is where the permissions go",
                base.display()
            )
        })?;
        // Back to the start of its line, so as not to break the indentation.
        let cut = text[..cut]
            .rfind('\n')
            .map(|newline| newline + 1)
            .unwrap_or(cut);
        format!(
            "{}    <!-- From the plugins. Written by `an` when it puts the APK together. -->\n{}\n{}",
            &text[..cut],
            lines.trim_end(),
            &text[cut..]
        )
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(destination, output)
        .with_context(|| format!("{} could not be written", destination.display()))?;
    Ok(destination.to_owned())
}

/// The one placeholder a manifest may carry.
///
/// `${applicationId}` is Gradle's spelling and this understands the same one,
/// because an Android manifest is the one file in this project people arrive at
/// already knowing. There is no general templating here and there will not be:
/// a manifest full of substitutions is a manifest whose meaning depends on a
/// build nobody is reading at the time.
///
/// It exists for exactly one thing. A `<provider>` authority is unique across
/// the whole **device**, not the app, so a literal one in a shell every app
/// shares means the second angular-native app to be installed fails with
/// `INSTALL_FAILED_CONFLICTING_PROVIDER`. `--rename-manifest-package` does not
/// help: it rewrites the package and qualifies relative class names, and leaves
/// every attribute value exactly as it found it.
fn substitute_application_id(text: &str, application_id: &str) -> String {
    text.replace("${applicationId}", application_id)
}

/// Puts `app.appearance` into the manifest as `<meta-data>`.
///
/// It has to reach the shell at run time and the shell is compiled once for
/// every app, so it cannot be a constant. `<meta-data>` is where Android puts
/// exactly this kind of thing: a build-time value the app reads back through
/// its own `PackageManager`.
///
/// It replaces the entry rather than adding a second one, because this runs on
/// a copy that may already carry it — the shell's own manifest ships the key so
/// that the monorepo has something to read too.
fn with_appearance(text: &str, appearance: Appearance) -> Result<String> {
    let element = format!(
        "        <meta-data android:name=\"{APPEARANCE_KEY}\" android:value=\"{}\" />",
        appearance.as_str()
    );
    if let Some(at) = text.find(&format!("android:name=\"{APPEARANCE_KEY}\"")) {
        // Rewrite the line it is on, whatever it said.
        let start = text[..at].rfind('\n').map(|n| n + 1).unwrap_or(0);
        let end = text[at..].find('\n').map(|n| at + n).unwrap_or(text.len());
        return Ok(format!("{}{element}{}", &text[..start], &text[end..]));
    }
    // Inside `<application>`, like a service: `aapt2` turns down a `<meta-data>`
    // that is a sibling of it rather than a child.
    let open = text
        .find("<application")
        .context("the manifest has no <application>, and that is what meta-data goes inside")?;
    let close = text[open..]
        .find('>')
        .map(|at| open + at + 1)
        .context("the manifest's <application> is never closed")?;
    Ok(format!("{}\n{element}{}", &text[..close], &text[close..]))
}

/// The name the shell looks the appearance up under. Written here and read in
/// `MainActivity`; `check-android-appearance.sh` keeps the two the same.
pub const APPEARANCE_KEY: &str = "dev.angularnative.appearance";

/// An `android:authorities` that will not be the app's, if the manifest has one.
///
/// Either spelling is fine: `${applicationId}` because `an` substitutes it, and
/// the identifier written out because a project may have decided to say it
/// itself. Anything else belongs to somebody else's app.
fn frozen_authority<'a>(text: &'a str, application_id: &str) -> Option<&'a str> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.contains("android:authorities"))
        .find(|line| !line.contains("${applicationId}") && !line.contains(application_id))
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
fn check_manifest(manifest: &Path, aab: bool, application_id: &str) -> Result<()> {
    let text = std::fs::read_to_string(manifest)
        .with_context(|| format!("{} could not be read", manifest.display()))?;
    // A device never asks for a version, which is why a manifest can go without
    // one for years. A store does, and bundletool says so only after everything
    // has been built — so it is said here instead, before anything is.
    if aab && !declares_version(&text) {
        bail!(
            "{}: there is no android:versionCode, and Google Play will not take a \
             bundle without one.\n\
             Add it to the <manifest> element, next to the package:\n\
             \x20   android:versionCode=\"1\"\n\
             \x20   android:versionName=\"1.0\"\n\
             versionCode is an integer that has to go up on every upload; Play refuses \
             one it has already seen.\nSee {}",
            manifest.display(),
            signing::DOCS
        );
    }
    // A `<provider>` authority is unique across the **device**, not the app, so
    // a fixed one in a shell every app shares means the second angular-native
    // app installed on a phone fails with INSTALL_FAILED_CONFLICTING_PROVIDER —
    // at install time, on somebody else's phone, with nothing in this build to
    // suggest it. And `AnShare` asks the system for `getPackageName()` plus the
    // suffix, so a fixed one is already the wrong string for any app whose id
    // is not the shell's: the first `share` throws.
    //
    // The shell writes `${applicationId}` and `an` substitutes it. What this
    // catches is the copy: `an add android` hands the manifest over once and it
    // is the user's from then on, so a project set up before that changed still
    // carries the old literal and nothing else would ever mention it.
    if let Some(literal) = frozen_authority(&text, application_id) {
        bail!(
            "{}: {literal}\n\
             A provider authority is unique across the whole device, not the app, so a fixed \
             one means two angular-native apps cannot be installed at once — the second fails \
             with INSTALL_FAILED_CONFLICTING_PROVIDER. It is also not the authority this app \
             asks for at run time: `share` looks for \"{application_id}.anfiles\", so the \
             first file handed to another app throws.\n\
             Write it against the application identifier:\n\
             \x20   android:authorities=\"${{applicationId}}.anfiles\"\n\
             `an` substitutes that when it puts the manifest together.",
            manifest.display()
        );
    }
    if !text.contains(&format!("package=\"{PACKAGE}\"")) {
        bail!(
            "{}: the manifest has to declare package=\"{PACKAGE}\", which is where the \
             shell's classes are.\n\
             The application identifier does not go here: it comes from app.bundleId in \
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
        .context("keytool could not be run")?;
    if !status.success() {
        bail!("the debug keystore could not be created");
    }
    Ok(path)
}

fn devices(adb: &Path) -> Result<Vec<(String, Form)>> {
    let output = Command::new(adb)
        .args(["devices"])
        .output()
        .context("adb devices could not be run")?;
    if !output.status.success() {
        bail!("adb devices failed");
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
            .with_context(|| format!("{serial} could not be asked about"))?;
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
            "there is no device connected; start a {} emulator \
             (`emulator -avd <name>`)",
            form.label()
        ),
        [] => bail!(
            "there is no device shaped like a {}; what there is is: {}",
            form.label(),
            found
                .iter()
                .map(|(serial, shape)| format!("{serial} ({})", shape.label()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        several => bail!(
            "there are {} devices shaped like a {}: {}. Pick one with --device",
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
            "{serial} is shaped like a {}, and this is a {} APK",
            shape.label(),
            form.label()
        ),
        None => bail!(
            "there is no device {serial}; what there is is: {}",
            if found.is_empty() {
                "nothing".to_owned()
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
    // The dev server's port, when there is one. It is opened back towards this
    // machine on the device that was picked — which is why it lives here and
    // not where the APK is assembled: the device is not known until now.
    dev_port: Option<u16>,
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
    if let Some(port) = dev_port {
        reverse_dev_port(&adb, &serial, port)?;
    }
    eprintln!("==> installing on {serial}");
    // Same reason as on iOS: installing over a running app does not reload the
    // new bundle.
    let _ = Command::new(&adb)
        .args(["-s", &serial, "shell", "am", "force-stop", &application_id])
        .output();
    run(
        workspace,
        &adb.to_string_lossy(),
        &["-s", &serial, "install", "-r", &apk.to_string_lossy()],
        "adb install failed",
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
        "the app could not be launched",
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
        .with_context(|| format!("{program} could not be run"))?;
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


/// Whether a manifest declares a version code.
///
/// By attribute name and not by parsing the XML: the only thing being asked is
/// whether the attribute is written anywhere in the `<manifest>` element, and
/// there is exactly one of those.
fn declares_version(manifest: &str) -> bool {
    manifest.contains("android:versionCode")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_with_no_version_code_is_caught_before_bundletool_sees_it() {
        let without = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="dev.angularnative">
</manifest>"#;
        assert!(!declares_version(without));
        assert!(declares_version(
            r#"<manifest package="dev.angularnative" android:versionCode="7">"#
        ));
    }

    #[test]
    fn an_authority_that_will_not_be_the_apps_is_caught() {
        let stale = r#"<provider android:authorities="dev.angularnative.anfiles" />"#;
        // The literal the shells carried before this moved: right for the
        // shell's own id and wrong for every app that renames itself.
        assert_eq!(frozen_authority(stale, "com.example.notes"), Some(stale.trim()));
        assert_eq!(frozen_authority(stale, "dev.angularnative"), None);
        // The two spellings that are always fine.
        assert_eq!(
            frozen_authority(r#"<provider android:authorities="${applicationId}.anfiles" />"#, "x"),
            None
        );
        assert_eq!(
            frozen_authority(
                r#"<provider android:authorities="com.example.notes.anfiles" />"#,
                "com.example.notes"
            ),
            None
        );
        // A manifest with no provider at all has nothing to say about this.
        assert_eq!(frozen_authority("<manifest></manifest>", "com.example.notes"), None);
    }

    #[test]
    fn the_application_id_reaches_the_provider_authority() {
        let manifest = r#"<provider android:authorities="${applicationId}.anfiles" />"#;
        assert_eq!(
            substitute_application_id(manifest, "com.example.notes"),
            r#"<provider android:authorities="com.example.notes.anfiles" />"#
        );
        // And a manifest without it is handed back untouched, which is what
        // lets the merged copy be skipped entirely when there is nothing to do.
        let plain = r#"<provider android:authorities="com.example.notes.anfiles" />"#;
        assert_eq!(substitute_application_id(plain, "com.other"), plain);
    }

    /// A literal authority in a shell every app shares is the bug this is here
    /// to stop coming back: it installs, it runs, and the second angular-native
    /// app on the device is the one that cannot be installed. `AnShare` asks
    /// for `getPackageName() + ".anfiles"`, so a literal one is also simply
    /// wrong for any app whose id is not the shell's.
    #[test]
    fn neither_shell_manifest_hardcodes_the_provider_authority() {
        for manifest in [Form::Phone.manifest(), Form::Watch.manifest()] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(manifest);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{} could not be read", path.display()));
            for line in text.lines().filter(|l| l.contains("android:authorities")) {
                assert!(
                    line.contains("${applicationId}"),
                    "{manifest}: {} is a literal authority; it has to be \
                     ${{applicationId}}-based or two apps cannot be installed at once",
                    line.trim()
                );
            }
        }
    }

    /// `--` cannot appear inside an XML comment, and a manifest is the one file
    /// here whose comments are long enough for somebody to write one by
    /// accident — spelling a command-line flag is all it takes. What comes back
    /// is `aapt2` saying `not well-formed (invalid token)` and a line number,
    /// which is true and says nothing about why.
    #[test]
    fn no_shell_manifest_comment_carries_a_double_hyphen() {
        for manifest in [Form::Phone.manifest(), Form::Watch.manifest()] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(manifest);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{} could not be read", path.display()));
            let mut rest = text.as_str();
            while let Some(open) = rest.find("<!--") {
                let body = &rest[open + 4..];
                let close = body.find("-->").unwrap_or_else(|| {
                    panic!("{manifest}: a comment is never closed");
                });
                assert!(
                    !body[..close].contains("--"),
                    "{manifest}: a comment contains `--`, which XML does not allow:\n{}",
                    &body[..close]
                );
                rest = &body[close + 3..];
            }
        }
    }

    /// The shells ship one. Without this, the manifests in the repository could
    /// lose it in a merge and only the first person to build a bundle would find
    /// out.
    #[test]
    fn both_shell_manifests_declare_one() {
        for manifest in [Form::Phone.manifest(), Form::Watch.manifest()] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(manifest);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{} could not be read", path.display()));
            assert!(declares_version(&text), "{manifest} has no android:versionCode");
        }
    }
}
