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
//! 3. **The entitlements are always written.** On iOS they only exist when a
//!    plugin asks for one; here there is a floor nobody can go under. The
//!    hardened runtime needs `allow-jit` or QuickJS is killed on its first
//!    allocation, so there is always a dictionary to write, and the plugins'
//!    keys are merged into that same one. It is also the platform where they
//!    matter most: a Mac app is sandboxed and signed, and an entitlement it does
//!    not carry is a system call it does not get, with an error that names the
//!    keychain or the network and never names the signature.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::build::run;
use crate::ios::swift_sources;
use crate::plugins::{self, Platform, Plugin};
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

/// Every plugin has to bring its macOS half, and the ones that do not are named
/// along with whatever they said about it.
///
/// This is the same rule `an ios` and `an android` enforce, and it replaces the
/// blanket refusal this host used to have. What changed is not the strictness —a
/// plugin the `.app` cannot serve still stops the build— but what is being
/// checked: it used to be "there is a plugin", and now it is "there is a plugin
/// with no Mac half".
pub fn require_plugins(plugins: &[Plugin]) -> Result<()> {
    plugins::require(plugins, Platform::Macos)
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
    plugins: &[Plugin],
    signing: Option<&Macos>,
) -> Result<Package> {
    // Before compiling anything: if some plugin does not bring its macOS half,
    // the build stops here and says which one and what it said about it.
    require_plugins(plugins)?;
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

    // The plugins: their Swift sources and the registry that hooks them up. It
    // all goes into the same `swiftc` invocation as the shell, so a plugin sees
    // `AnPlugin` and `AnPluginCall` without importing anything.
    for plugin in plugins {
        let contributed = plugins::sources(plugin, Platform::Macos)?;
        eprintln!("==> plugin {} ({} Swift sources)", plugin.module, contributed.len());
        sources.extend(contributed);
    }
    sources.push(
        plugins::generate_swift(plugins, Platform::Macos, &app_dir.parent()
            .unwrap_or(&app_dir)
            .join("generated"))?
            .to_string_lossy()
            .into_owned(),
    );

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

    write_plist(
        &root.join("shells/macos/Resources/Info.plist"),
        &contents.join("Info.plist"),
        plugins,
    )?;
    std::fs::copy(bundle, resources.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(resources.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(resources.join("dev-server.txt"));
        }
    }

    // One entitlements file for both signing paths. It is what carries the
    // plugins' keys, and a plugin that only got them on the signed path would
    // work for whoever ships the app and fail for whoever develops it, which is
    // the wrong way round.
    let entitlements = write_entitlements(&app_dir, plugins, BUNDLE_ID, signing.is_some())?;
    match signing {
        // Unsigned, macOS kills the app on the first `mmap` of generated code
        // —which is what QuickJS does— with a `Killed: 9` and no explanation.
        // An ad-hoc signature is enough for development and needs nobody's
        // account.
        None => {
            let signed = Command::new("codesign")
                .args(["--force", "--sign", "-", "--entitlements"])
                .arg(&entitlements)
                .arg(&app_dir)
                .status()
                .context("codesign could not be run")?;
            if !signed.success() {
                bail!("the ad-hoc signing of the .app failed");
            }
        }
        Some(macos) => sign(&app_dir, &entitlements, macos)?,
    }

    Ok(Package { dir: app_dir })
}

/// Writes the `.app`'s `Info.plist`: the shell's plus whatever the plugins ask
/// for.
///
/// It is the same merge `an ios` does and for the same reason: a plugin that
/// needs a usage key and does not get it produces an app the system kills the
/// moment the permission is evaluated, with nothing in the log about the key. On
/// the Mac the list is shorter than the phone's —there is no Face ID here— but
/// it is not empty: `NSMicrophoneUsageDescription` and the Apple-events one are
/// exactly as fatal.
///
/// The shell's own plist outranks the plugin, and not silently: what was ignored
/// and whose it was gets said.
fn write_plist(base: &Path, destination: &Path, plugins: &[Plugin]) -> Result<()> {
    let contributed = plugins::plist_entries(plugins, Platform::Macos)?;
    std::fs::copy(base, destination)?;
    if contributed.is_empty() {
        return Ok(());
    }
    let already_there = plist_keys(base)?;
    for (key, entry) in &contributed {
        if let Some(current) = already_there.get(key) {
            if current != &entry.value {
                eprintln!(
                    "==> Info.plist: {key} is already declared by the app\n    \
                     ({current}); ignoring {}'s ({})",
                    entry.package, entry.value
                );
            }
            continue;
        }
        eprintln!("==> Info.plist: {key} (from {})", entry.package);
        let status = Command::new("plutil")
            .args(["-replace", key, "-json", &entry.value.to_string()])
            .arg(destination)
            .status()
            .context("plutil could not be run")?;
        if !status.success() {
            bail!("the key {key} a plugin asks for could not be written into the Info.plist");
        }
    }
    Ok(())
}

/// The top-level keys of an `Info.plist`, actually read.
///
/// Converted to JSON with `plutil` and not grepped for `<key>`: a plist can come
/// in binary form, and looking for text inside a binary finds nothing and would
/// have you believe the app declares no keys at all.
fn plist_keys(plist: &Path) -> Result<serde_json::Map<String, Value>> {
    let json = capture("plutil", &["-convert", "json", "-o", "-", &plist.to_string_lossy()])
        .with_context(|| format!("{}: it could not be read", plist.display()))?;
    let parsed: Value = serde_json::from_str(&json).with_context(|| {
        format!("{}: plutil returned something that is not JSON", plist.display())
    })?;
    match parsed {
        Value::Object(map) => Ok(map),
        _ => bail!("{}: the root of an Info.plist has to be a dictionary", plist.display()),
    }
}

/// Signs the `.app` with a Developer ID certificate and the hardened runtime.
///
/// Both are required by notarisation, and the hardened runtime is what makes
/// this more than a flag change: it turns off the ability to map writable,
/// executable memory, and the app dies on startup without a single word about
/// entitlements. Which is why one is written here — see [`hardened_entitlements`].
fn sign(app_dir: &Path, entitlements: &Path, macos: &Macos) -> Result<()> {
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

/// The `.app`'s entitlements: the two the engine cannot live without, plus
/// whatever the plugins ask for.
///
/// **The floor.** The hardened runtime forbids by default the two things a
/// JavaScript engine does. QuickJS is an interpreter and does not compile
/// machine code, but it does map its bytecode and its stacks the way a JIT
/// would, and the runtime does not tell the two apart: without `allow-jit` the
/// app is killed on the first allocation, with a `Killed: 9` in the console and
/// nothing about entitlements anywhere. Notarisation allows both. They are
/// declared here and not left to the project because getting them wrong produces
/// an app that opens on the machine that built it —where the hardened runtime is
/// not enforced the same way— and dies on everybody else's.
///
/// **The plugins' half.** This is where the Mac differs from the phone. On iOS a
/// plugin's entitlements are needed by one plugin —the keychain— and on the Mac
/// nearly every plugin has one, because the App Sandbox denies by default and
/// each capability has to be asked for by name: the network, the microphone, a
/// file the user picked, the keychain. The failure is the one that is hardest to
/// read, too. `SecItemAdd` answers −34018 and says "the client has neither of
/// the two"; nothing in that sentence mentions a signature, and the bug looks
/// like a keychain bug for as long as it takes somebody to think of entitlements.
///
/// The floor wins a collision, and it is the only place it can: a plugin that
/// asked for `allow-jit: false` would be asking for an app that does not start.
fn write_entitlements(
    app_dir: &Path,
    plugins: &[Plugin],
    bundle_id: &str,
    real_identity: bool,
) -> Result<PathBuf> {
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    for (key, entry) in &plugins::entitlement_entries(plugins, Platform::Macos)? {
        if !real_identity && needs_profile(key) {
            // See `needs_profile`. This is the loudest warning in this file
            // because it is the one that changes what the app can do, and the
            // app still starts and still looks right: whoever gets it has to be
            // told what stopped working and what to run to get it back.
            eprintln!(
                "==> warning: {key} (asked for by {}) is left out of this build.\n    \
                 An ad-hoc signature cannot carry it —macOS wants a provisioning profile behind \
                 it— and an .app that carries it anyway is killed the instant it launches, with \
                 a bare `Killed: 9`.\n    \
                 The plugin will fall back to whatever it can do without the entitlement and say \
                 so at run time. `an macos --sign` gives it the real one.",
                entry.package
            );
            continue;
        }
        eprintln!("==> entitlements: {key} (from {})", entry.package);
        entries.insert(key.clone(), substitute(&entry.value, bundle_id));
    }
    for key in ["com.apple.security.cs.allow-jit", "com.apple.security.cs.allow-unsigned-executable-memory"] {
        if let Some(previous) = entries.insert(key.to_owned(), Value::Bool(true)) {
            if previous != Value::Bool(true) {
                // Said and overruled, not silently overruled: a plugin that
                // asked for this to be false asked for an app that is killed on
                // startup, and it deserves to hear that it did not get it.
                eprintln!(
                    "==> entitlements: {key} stays true; without it QuickJS is killed on its \
                     first allocation and the app never draws a frame"
                );
            }
        }
    }

    let out = app_dir.parent().unwrap_or(app_dir);
    std::fs::create_dir_all(out)?;
    let json = out.join("entitlements.json");
    let plist = out.join("angular-native.entitlements");
    std::fs::write(
        &json,
        Value::Object(entries.into_iter().collect::<serde_json::Map<_, _>>()).to_string(),
    )
    .with_context(|| format!("{} could not be written", json.display()))?;
    // codesign wants a plist, not a JSON. Converting it with `plutil` saves
    // writing XML by hand and escaping the plugins' values along the way.
    let converted = Command::new("plutil")
        .args(["-convert", "xml1", "-o"])
        .arg(&plist)
        .arg(&json)
        .status()
        .context("plutil could not be run")?;
    if !converted.success() {
        bail!("{} could not be written", plist.display());
    }
    Ok(plist)
}

/// Whether macOS demands a provisioning profile behind this entitlement.
///
/// This is the rule that cost an afternoon, so it is written down rather than
/// discovered again. macOS splits entitlements in two. The `com.apple.security.`
/// ones —the hardened-runtime relaxations, the sandbox— are *restrictions the
/// app puts on itself*, and anybody may sign them, ad hoc included. The rest are
/// *permissions the system grants*, and the system will not grant one on the
/// word of a signature that belongs to nobody. An ad-hoc `.app` carrying
/// `keychain-access-groups` does not fail to use the keychain: **it is killed at
/// launch**, before its first line runs, with a `Killed: 9` and nothing in the
/// log about entitlements. It is indistinguishable from the JIT kill the
/// hardened-runtime entitlements exist to prevent, which is precisely how it
/// wastes an afternoon.
///
/// So an ad-hoc build leaves them out and says so, and the plugin finds out at
/// run time that it did not get them. That is why the keychain plugin probes
/// instead of assuming, and why its `backing()` has something to report.
///
/// `com.apple.security.application-groups` is on this list despite the prefix:
/// a group identifier is a name Apple hands out, not a restriction, and it needs
/// the profile like the rest of them.
fn needs_profile(key: &str) -> bool {
    key == "keychain-access-groups"
        || key == "application-identifier"
        || key == "com.apple.application-identifier"
        || key == "com.apple.security.application-groups"
        || key.starts_with("com.apple.developer.")
}

/// Swaps `$(BUNDLE_ID)` for this app's identifier.
///
/// It is the only substitution there is, and it exists because the value that is
/// nearly always asked for —the keychain group— is the app's identifier, and the
/// plugin cannot know it. `an ios` does exactly the same thing; Xcode does it
/// with `$(AppIdentifierPrefix)`.
fn substitute(value: &Value, bundle_id: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace("$(BUNDLE_ID)", bundle_id)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| substitute(item, bundle_id)).collect())
        }
        other => other.clone(),
    }
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
        // Exactly the invocation Apple's notarisation documentation gives.
        .args(["-c", "-k", "--keepParent"])
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
