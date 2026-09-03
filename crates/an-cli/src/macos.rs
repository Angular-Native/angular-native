//! Builds the macOS `.app` and launches it.
//!
//! The same thing `ios.rs` does —no `.xcodeproj`, `swiftc` linking against
//! Rust's staticlib— with three differences, and not one of them is cosmetic:
//!
//! 1. **There is no simulator.** A macOS `.app` is launched on the machine that
//!    compiled it, so there is no `simctl` here, no `bootstatus`, no looking a
//!    device up by name: it opens and that is that. It is the only one of the
//!    four platforms you can take a screenshot of without starting anything
//!    else.
//! 2. **The previous instance has to be killed.** In a simulator every launch
//!    replaces the one before; here, if the app is already running, `open` does
//!    nothing but bring the old window to the front and it looks as though the
//!    change never landed. It is the same bug that forces an uninstall before
//!    an install on iOS, wearing a different face.
//! 3. **It does not load plugins yet.** Same as the watch: `an-macos` has none
//!    of the registry `an-ios` and `an-android` do have, so the build stops and
//!    says so instead of leaving a module that swallows every call.

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
/// Sonoma is the oldest one where `NSView.displayLink(target:selector:)` exists,
/// and that is the clock the shell uses: on a Mac with several screens at
/// different refresh rates, the right one belongs to the screen the window is
/// on, and only the view knows which that is.
const DEPLOYMENT: &str = "14.0";

/// Frameworks `an-macos` depends on besides AppKit, which Swift brings in on
/// its own. See `assemble` for why they have to be named.
const FRAMEWORKS: &[&str] = &["WebKit", "MapKit", "AVFoundation", "AVKit"];

pub struct Package {
    pub dir: PathBuf,
}

/// This host does not load plugins yet. It is said here and it stops: building
/// the `.app` anyway would leave an app in which the module does not exist and
/// every call is turned down at runtime, which is exactly what this system does
/// not do.
pub fn reject_plugins(plugins: &[Plugin]) -> Result<()> {
    if plugins.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.package.as_str()).collect();
    bail!(
        "this app cannot be built for macOS: the desktop host does not load plugins yet, \
         and it depends on {}. See https://angular-native.dev/extending/plugins/.",
        names.join(", ")
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
    // A macOS `.app` is not flat the way iOS's is: the executable goes in
    // `Contents/MacOS`, the `Info.plist` in `Contents` and the resources in
    // `Contents/Resources`. Dropping an iPhone `.app` on a Mac as-is gives a
    // bundle Finder opens and `open` turns down without saying why.
    let contents = app_dir.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");

    eprintln!("==> core Rust ({profile}, {TARGET})");
    let mut cargo_args = vec!["build", "--target", TARGET, "-p", "an-macos"];
    if release {
        cargo_args.push("--release");
    }
    // QuickJS is built with `cc`, which without this uses the SDK's minimum and
    // the Swift link step complains about the mismatch.
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env("MACOSX_DEPLOYMENT_TARGET", DEPLOYMENT)
        .current_dir(root)
        .status()
        .context("cargo could not be run")?;
    if !status.success() {
        bail!("the core build for macOS failed");
    }

    eprintln!("==> shell AppKit");
    let sdk = capture("xcrun", &["--sdk", "macosx", "--show-sdk-path"])?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(&resources)?;

    // `shells/shared` brings what does not depend on the platform —the dev
    // server's client—, compiled by all three Apple shells.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/macos/Sources"))?;
    if sources.is_empty() {
        bail!("there are no Swift sources in shells/macos/Sources");
    }
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    let lib_dir = workspace.target_dir().join(TARGET).join(profile);
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
    // The frameworks the host uses, named here and not only in the Rust.
    //
    // A Rust `staticlib` **does not drag its native dependencies along**:
    // `an-macos`'s `#[link(name = "…", kind = "framework")]` documents what each
    // module depends on, but the `.a` that comes out carries nothing that tells
    // the linker. The one that really links is this `swiftc`, and if they are
    // not here, the app builds, signs and starts without a single complaint: it
    // blows up later, on mounting the first view of that class, with a "class
    // AVPlayerView could not be found" that points nowhere.
    //
    // The list must not fall behind: `scripts/check-macos.py` compares it with
    // the crate's `#[link]`s.
    for framework in FRAMEWORKS {
        args.push("-framework".into());
        args.push((*framework).into());
    }
    if release {
        args.push("-O".into());
    }
    args.extend(sources);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "xcrun", &borrowed, "the shell link step failed")?;

    std::fs::copy(root.join("shells/macos/Resources/Info.plist"), contents.join("Info.plist"))?;
    std::fs::copy(bundle, resources.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(resources.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(resources.join("dev-server.txt"));
        }
    }

    // Unsigned, macOS kills the app on the first `mmap` of generated code
    // —which is what QuickJS does— with a `Killed: 9` and no explanation. An
    // ad-hoc signature is enough for development and needs nobody's account.
    let signed = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&app_dir)
        .status()
        .context("codesign could not be run")?;
    if !signed.success() {
        bail!("the ad-hoc signing of the .app failed");
    }

    Ok(Package { dir: app_dir })
}

pub fn launch(package: &Package) -> Result<()> {
    // If there is already an instance, `open` would bring the old one to the
    // front and the change would look as though it had never landed.
    let _ = Command::new("killall").args(["-9", APP_NAME]).output();

    eprintln!("==> lanzando {}", package.dir.display());
    let launched = Command::new("open")
        .arg("-n")
        .arg(&package.dir)
        .status()
        .context("the app could not be launched")?;
    if !launched.success() {
        bail!("the launch failed");
    }
    let _ = BUNDLE_ID;
    Ok(())
}

fn capture(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("{program} could not be run"))?;
    if !output.status.success() {
        bail!("{program} {args:?} failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
