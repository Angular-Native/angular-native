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
use crate::signing::{self, Macos};
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

/// `signing` decides how the `.app` is signed at the end. `None` is ad hoc,
/// which is enough to run on the machine that built it and needs nobody's
/// account; `Some` is a Developer ID signature with the hardened runtime, which
/// is what another Mac will open.
pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    signing: Option<&Macos>,
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

    match signing {
        // Unsigned, macOS kills the app on the first `mmap` of generated code
        // —which is what QuickJS does— with a `Killed: 9` and no explanation.
        // An ad-hoc signature is enough for development and needs nobody's
        // account.
        None => {
            let signed = Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&app_dir)
                .status()
                .context("codesign could not be run")?;
            if !signed.success() {
                bail!("the ad-hoc signing of the .app failed");
            }
        }
        Some(macos) => sign(&app_dir, macos)?,
    }

    Ok(Package { dir: app_dir })
}

/// Signs the `.app` with a Developer ID certificate and the hardened runtime.
///
/// Both are required by notarisation, and the hardened runtime is what makes
/// this more than a flag change: it turns off the ability to map writable,
/// executable memory, and the app dies on startup without a single word about
/// entitlements. Which is why one is written here — see [`hardened_entitlements`].
fn sign(app_dir: &Path, macos: &Macos) -> Result<()> {
    let entitlements = hardened_entitlements(app_dir)?;
    eprintln!("==> codesign ({})", macos.identity_name);
    let signed = Command::new("codesign")
        .args([
            "--force",
            // The hardened runtime. Notarisation refuses anything without it,
            // and it is the reason the entitlements above exist.
            "--options",
            "runtime",
            // A secure timestamp, from Apple's server. Without one the
            // signature stops being valid the day the certificate expires,
            // rather than staying valid for what was signed while it was.
            // Notarisation refuses that too.
            "--timestamp",
            "--sign",
            &macos.identity,
            "--entitlements",
        ])
        .arg(&entitlements)
        .arg(app_dir)
        .output()
        .context("codesign could not be run")?;
    if !signed.status.success() {
        let said = String::from_utf8_lossy(&signed.stderr);
        let hint = if said.contains("Timestamp service") || said.contains("timestamp") {
            "\nThe timestamp comes from Apple over the network: this needs to be online."
        } else if said.contains("no identity found") || said.contains("ambiguous") {
            "\n`security find-identity -v -p codesigning` lists what this keychain has."
        } else {
            ""
        };
        bail!(
            "codesign refused to sign {} with {:?}.\n{}{hint}\nSee {}",
            app_dir.display(),
            macos.identity_name,
            said.trim(),
            signing::DOCS
        );
    }
    Ok(())
}

/// The entitlements a hardened-runtime build needs, and why each one is there.
///
/// The hardened runtime forbids by default the two things a JavaScript engine
/// does. QuickJS is an interpreter and does not compile machine code, but it
/// does map its bytecode and its stacks the way a JIT would, and the runtime
/// does not tell the two apart: without `allow-jit` the app is killed on the
/// first allocation, with a `Killed: 9` in the console and nothing about
/// entitlements anywhere. It is the same failure the ad-hoc signature exists to
/// prevent, wearing the one face nobody recognises.
///
/// Notarisation allows both of these. They are declared here and not left to the
/// project because getting them wrong produces an app that opens on the machine
/// that built it —where the hardened runtime is not enforced the same way— and
/// dies on everybody else's.
fn hardened_entitlements(app_dir: &Path) -> Result<PathBuf> {
    let path = app_dir
        .parent()
        .unwrap_or(app_dir)
        .join("hardened.entitlements");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>com.apple.security.cs.allow-jit</key>
	<true/>
	<key>com.apple.security.cs.allow-unsigned-executable-memory</key>
	<true/>
</dict>
</plist>
"#,
    )
    .with_context(|| format!("{} could not be written", path.display()))?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Notarising, stapling, and the .dmg
// ---------------------------------------------------------------------------

/// Sends the `.app` to Apple, waits for the answer, and staples it.
///
/// Stapling is the step that is easy to skip and expensive to skip: without it
/// the app is notarised but the ticket lives on Apple's servers, so the first
/// person to open it offline gets Gatekeeper's "cannot be opened" and no way to
/// tell that from an app that was never notarised at all.
pub fn notarize(package: &Package, macos: &Macos) -> Result<()> {
    let profile = macos
        .notary_profile
        .as_ref()
        .expect("--notarize resolves a notarytool profile before building");
    // notarytool takes a zip, a dmg or a pkg — never a bare `.app`. `ditto` is
    // the only zipper here that keeps a bundle intact.
    let archive = package.dir.with_extension("zip");
    let _ = std::fs::remove_file(&archive);
    let zipped = Command::new("ditto")
        .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
        .arg(&package.dir)
        .arg(&archive)
        .status()
        .context("ditto could not be run")?;
    if !zipped.success() {
        bail!("{} could not be zipped for notarisation", package.dir.display());
    }

    eprintln!("==> notarytool submit (this waits on Apple, usually a minute or two)");
    let submitted = Command::new("xcrun")
        .args(["notarytool", "submit", "--keychain-profile", profile, "--wait"])
        .arg(&archive)
        .output()
        .context("notarytool could not be run")?;
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&submitted.stdout),
        String::from_utf8_lossy(&submitted.stderr)
    );
    let _ = std::fs::remove_file(&archive);
    // notarytool exits zero when the *submission* worked, which is not the same
    // as the app being accepted. The status in its output is the answer, and
    // reading only the exit code is how an unnotarised app gets stapled and
    // shipped.
    if !submitted.status.success() || !said.contains("status: Accepted") {
        let hint = if said.contains("keychain profile") || said.contains("No Keychain profile") {
            format!(
                "\nThere is no notarytool profile called {profile:?} in this keychain. \
                 Create it with `xcrun notarytool store-credentials {profile}`."
            )
        } else if said.contains("Invalid") {
            "\nApple rejected the app. `xcrun notarytool log <submission-id> \
             --keychain-profile <profile>` prints exactly which binary and which \
             requirement — nearly always a missing hardened runtime or a missing \
             secure timestamp on something nested."
                .to_owned()
        } else {
            String::new()
        };
        bail!(
            "notarisation did not come back accepted.\n{}{hint}\nSee {}",
            said.trim(),
            signing::DOCS
        );
    }

    staple(&package.dir)
}

/// Attaches the notarisation ticket to a bundle or a `.dmg`, so it opens with no
/// network.
pub fn staple(what: &Path) -> Result<()> {
    eprintln!("==> stapler staple {}", what.display());
    let stapled = Command::new("xcrun")
        .args(["stapler", "staple"])
        .arg(what)
        .output()
        .context("stapler could not be run")?;
    if !stapled.status.success() {
        bail!(
            "the notarisation ticket could not be stapled to {}.\n{}\n\
             Without it the app is notarised but the ticket only lives on Apple's \
             servers, and the first person to open it offline is told it cannot be \
             opened.\nSee {}",
            what.display(),
            String::from_utf8_lossy(&stapled.stderr).trim(),
            signing::DOCS
        );
    }
    Ok(())
}

/// Puts the `.app` in a `.dmg`, signs it, and —when asked— notarises and staples
/// that too.
///
/// The disk image is signed and notarised in its own right, and not only because
/// it can be: it is the file that gets downloaded, so it is the one Gatekeeper
/// looks at first. A `.dmg` carrying a perfectly notarised app is still an
/// unsigned download.
pub fn dmg(package: &Package, macos: Option<&Macos>, notarising: bool) -> Result<PathBuf> {
    let name = package
        .dir
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| APP_NAME.to_owned());
    let dmg = package.dir.with_extension("dmg");
    let _ = std::fs::remove_file(&dmg);
    eprintln!("==> hdiutil create {}", dmg.display());
    let created = Command::new("hdiutil")
        .args(["create", "-volname", &name, "-srcfolder"])
        .arg(&package.dir)
        .args(["-ov", "-format", "UDZO"])
        .arg(&dmg)
        .output()
        .context("hdiutil could not be run")?;
    if !created.status.success() {
        bail!(
            "the .dmg could not be created.\n{}",
            String::from_utf8_lossy(&created.stderr).trim()
        );
    }

    if let Some(macos) = macos {
        let signed = Command::new("codesign")
            .args(["--force", "--timestamp", "--sign", &macos.identity])
            .arg(&dmg)
            .output()
            .context("codesign could not be run")?;
        if !signed.status.success() {
            bail!(
                "the .dmg could not be signed with {:?}.\n{}\nSee {}",
                macos.identity_name,
                String::from_utf8_lossy(&signed.stderr).trim(),
                signing::DOCS
            );
        }
        if notarising {
            let profile = macos
                .notary_profile
                .as_ref()
                .expect("--notarize resolves a notarytool profile before building");
            eprintln!("==> notarytool submit (the .dmg)");
            let submitted = Command::new("xcrun")
                .args(["notarytool", "submit", "--keychain-profile", profile, "--wait"])
                .arg(&dmg)
                .output()
                .context("notarytool could not be run")?;
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&submitted.stdout),
                String::from_utf8_lossy(&submitted.stderr)
            );
            if !submitted.status.success() || !said.contains("status: Accepted") {
                bail!(
                    "the .dmg did not come back accepted.\n{}\nSee {}",
                    said.trim(),
                    signing::DOCS
                );
            }
            staple(&dmg)?;
        }
    }

    let size = std::fs::metadata(&dmg)?.len();
    eprintln!("==> {} MB in {}", size / (1024 * 1024), dmg.display());
    Ok(dmg)
}

pub fn launch(package: &Package) -> Result<()> {
    // If there is already an instance, `open` would bring the old one to the
    // front and the change would look as though it had never landed.
    let _ = Command::new("killall").args(["-9", APP_NAME]).output();

    eprintln!("==> launching {}", package.dir.display());
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
