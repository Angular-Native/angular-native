//! Builds the `.app` for the UIKit families and takes it to the simulator.
//!
//! No `.xcodeproj`: `cargo` compiles the core into a staticlib, `swiftc` links
//! the shell against it, and the bundle is put together by hand. An Xcode
//! project here would only add a 2,000-line file nobody can review in a diff.
//!
//! iOS, tvOS and visionOS share a crate (`an-ios`), a shell
//! (`shells/ios/Sources`) and a C surface. What differs between them fits in
//! [`Family`], and it is not much: the triple, the SDK, the `Info.plist` and
//! whether Rust's `std` arrives prebuilt or has to be built on the spot. That is
//! why there is no `tvos.rs` and no `visionos.rs`: they would be two copies of
//! this same `swiftc`, and a copy is exactly what falls behind the day somebody
//! fixes something in only one of them.
//!
//! Splitting by family here, and not in a separate file, is also what lets tvOS
//! inherit for free everything this module already knew how to do:
//! `workspace.build_dir()`, the `Info.plist` the project supplies, and the check
//! that this plist says the same thing as the project.
//!
//! `watchos.rs` is separate, and for a reason: the watch has no `UIView`, so it
//! shares neither the crate nor the shell. None of that applies here.
//!
//! What really is different on visionOS is where the window comes from: there is
//! no `UIScreen`, so there is no screen size to work it out from and it has to
//! be born of a `UIWindowScene`. The shell already does that, which is why that
//! family's `Info.plist` declares a `UIApplicationSceneManifest` the other two
//! do not need.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::{run, run_in};
use crate::plugins::{self, Platform, Plugin};
use crate::signing::{self, Apple};
use crate::workspace::Workspace;

/// The families built on `UIView` with absolute frames.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Ios,
    TvOs,
    VisionOs,
}

/// A simulator or a device plugged into this Mac. It changes the Rust target,
/// the SDK, swiftc's triple and —the part that is not a flag— whether the app
/// has to be signed at all.
///
/// On the simulator it does not: the entitlements go inside the binary and
/// nobody checks a signature. On a device every one of those is the opposite,
/// and getting it wrong produces an app that installs and is killed on launch.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Simulator,
    Device,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Ios => "iOS",
            Family::TvOs => "tvOS",
            Family::VisionOs => "visionOS",
        }
    }

    /// The platform's name as the user writes it: the project's directory
    /// (`ios/Info.plist`, `tvos/Info.plist`), the build's subdirectory, and the
    /// argument to `an add`.
    pub fn slug(self) -> &'static str {
        match self {
            Family::Ios => "ios",
            Family::TvOs => "tvos",
            Family::VisionOs => "visionos",
        }
    }

    /// What gets appended to the app's name and to the bundle identifier.
    ///
    /// All three families can be installed at once, on different simulators,
    /// from the same project. Sharing a name would have `an tvos` overwrite the
    /// `.app` `an ios` had just left; sharing an identifier would have
    /// installing one uninstall the other.
    pub fn suffix(self) -> (&'static str, &'static str) {
        match self {
            Family::Ios => ("", ""),
            Family::TvOs => ("TV", ".tv"),
            Family::VisionOs => ("Vision", ".vision"),
        }
    }

    fn target(self, to: Destination) -> &'static str {
        match (self, to) {
            (Family::Ios, Destination::Simulator) => "aarch64-apple-ios-sim",
            (Family::Ios, Destination::Device) => "aarch64-apple-ios",
            (Family::TvOs, Destination::Simulator) => "aarch64-apple-tvos-sim",
            (Family::TvOs, Destination::Device) => "aarch64-apple-tvos",
            (Family::VisionOs, Destination::Simulator) => "aarch64-apple-visionos-sim",
            (Family::VisionOs, Destination::Device) => "aarch64-apple-visionos",
        }
    }

    /// The SDK it asks `xcrun` for.
    fn sdk(self, to: Destination) -> &'static str {
        match (self, to) {
            (Family::Ios, Destination::Simulator) => "iphonesimulator",
            (Family::Ios, Destination::Device) => "iphoneos",
            (Family::TvOs, Destination::Simulator) => "appletvsimulator",
            (Family::TvOs, Destination::Device) => "appletvos",
            (Family::VisionOs, Destination::Simulator) => "xrsimulator",
            (Family::VisionOs, Destination::Device) => "xros",
        }
    }

    /// `swiftc`'s triple, which is not Rust's. visionOS is still called `xros`
    /// here: the marketing name changed and the compiler's did not.
    fn swift_target(self, to: Destination) -> String {
        let version = self.deployment();
        // A device triple is the simulator's without the suffix. Leaving the
        // `-simulator` on while linking against the device SDK produces a
        // binary the device refuses with "mach-o file, but is an incompatible
        // architecture", which names neither the SDK nor the triple.
        let suffix = match to {
            Destination::Simulator => "-simulator",
            Destination::Device => "",
        };
        match self {
            Family::Ios => format!("arm64-apple-ios{version}{suffix}"),
            Family::TvOs => format!("arm64-apple-tvos{version}{suffix}"),
            Family::VisionOs => format!("arm64-apple-xros{version}{suffix}"),
        }
    }

    /// The variable that tells `cc` —the one that builds QuickJS— which minimum
    /// version it is compiling for. Without it, it uses the SDK's minimum and
    /// the Swift link step complains about the mismatch.
    fn deployment_env(self) -> &'static str {
        match self {
            Family::Ios => "IPHONEOS_DEPLOYMENT_TARGET",
            Family::TvOs => "TVOS_DEPLOYMENT_TARGET",
            Family::VisionOs => "XROS_DEPLOYMENT_TARGET",
        }
    }

    fn deployment(self) -> &'static str {
        match self {
            // tvOS 17 is contemporary with iOS 17 and brings the same UIKit.
            Family::Ios | Family::TvOs => "17.0",
            // visionOS starts at 1.0: there are no earlier versions.
            Family::VisionOs => "1.0",
        }
    }

    /// Whether Rust's `std` arrives prebuilt or has to be built on the spot.
    ///
    /// `aarch64-apple-ios-sim` is tier 2 and rustup ships it compiled.
    /// `aarch64-apple-tvos-sim` and `aarch64-apple-visionos-sim` are tier 3:
    /// rustup lists them, but with no `std`. It is the same thing that already
    /// happened with watchOS, which is why those two call `cargo +nightly`.
    fn needs_build_std(self) -> bool {
        !matches!(self, Family::Ios)
    }

    /// The shell's `Info.plist`, for when the project supplies none of its own.
    /// It is the only part of the shell that is not shared: the keys each family
    /// asks for look nothing alike.
    fn resources(self) -> &'static str {
        match self {
            Family::Ios => "shells/ios/Resources",
            Family::TvOs => "shells/tvos/Resources",
            Family::VisionOs => "shells/visionos/Resources",
        }
    }

    /// What has to appear in `simctl`'s runtime name for a device to count as
    /// belonging to this family. visionOS shows up as `xrOS`, same as in
    /// swiftc's triple.
    fn runtime_marker(self) -> &'static str {
        match self {
            Family::Ios => "iOS",
            Family::TvOs => "tvOS",
            Family::VisionOs => "xrOS",
        }
    }

    /// What to run to get the simulator's runtime, if it is missing. Having the
    /// SDK is not enough to start anything: they are two separate downloads.
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
    /// The identifier `simctl` installs, launches and uninstalls with. It comes
    /// from the project: two different apps cannot share it or each would
    /// uninstall the other.
    pub bundle_id: String,
    /// The app's name without the `.app`, which is also the executable's. The
    /// archive and the `.ipa` are named after it.
    pub app_name: String,
    /// The platform's build directory, `build/ios` and friends. The archive,
    /// the `.ipa` and the generated entitlements go next to the `.app`.
    pub out: PathBuf,
    family: Family,
}

/// `dev_server` is the dev server's URL, if there is one. It is written inside
/// the `.app`: the app reads it on startup and, if it is there, subscribes to
/// reloads.
///
/// `signing` decides everything else. `None` is the simulator: the device
/// target is not used, nothing is signed, and the entitlements —if any plugin
/// asks for one— go inside the binary. `Some` is a real device: the device
/// SDK, the device triple, the profile embedded in the bundle and a real
/// signature over the lot.
pub fn assemble(
    workspace: &Workspace,
    family: Family,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
    signing: Option<&Apple>,
) -> Result<Package> {
    let to = match signing {
        Some(_) => Destination::Device,
        None => Destination::Simulator,
    };
    // Before compiling anything: if some plugin does not bring its iOS half,
    // the build stops here and says which one.
    //
    // tvOS and visionOS ask for the same key, `ios`, and use the same Swift
    // sources: it is the same shell and the same `AnPlugin` protocol. If that
    // Swift uses something that only exists on the phone, the link step stops
    // with swiftc's error, which says which symbol and on which line. The
    // warning comes first so that error does not arrive as a surprise.
    plugins::require(plugins, Platform::Ios)?;
    if family != Family::Ios && !plugins.is_empty() {
        eprintln!(
            "==> warning: the plugins are compiled with their iOS sources, which is all they \
             declare. If one of them uses an API {} does not have, the link step stops and says so.",
            family.label()
        );
    }

    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let (name_suffix, id_suffix) = family.suffix();
    let app_name = format!("{}{name_suffix}", workspace.app_name());
    let bundle_id = format!("{}{id_suffix}", workspace.bundle_id());
    let out = workspace.build_dir().join(family.slug());
    let app_dir = out.join(format!("{app_name}.app"));
    // The project's `Info.plist` overrides the shell's if there is one: it is
    // what `an add ios` / `an add tvos` writes, and from then on it belongs to
    // the user. It is checked before anything is compiled: that is half a minute
    // of `cargo` and `swiftc` there is no reason to burn only to say the name
    // does not line up.
    let plist = workspace
        .overlay(family.slug(), "Info.plist")
        .unwrap_or_else(|| root.join(family.resources()).join("Info.plist"));
    check_plist(&plist, &app_name, &bundle_id, family, workspace)?;

    eprintln!("==> core Rust ({profile}, {})", family.target(to));
    let mut cargo_args: Vec<&str> = Vec::new();
    if family.needs_build_std() {
        // See `needs_build_std`. If this fails because the component is
        // missing, cargo's message already says which one, so it is let through
        // as-is rather than guessed at.
        cargo_args.extend(["+nightly", "build", "-Z", "build-std=std,panic_abort"]);
    } else {
        cargo_args.push("build");
    }
    cargo_args.extend(["--target", family.target(to), "-p", "an-ios"]);
    if release {
        cargo_args.push("--release");
    }
    // QuickJS is built with `cc`, which without this uses the SDK's minimum and
    // the Swift link step complains about the mismatch.
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env(family.deployment_env(), family.deployment())
        .current_dir(root)
        .status()
        .context("cargo could not be run")?;
    if !status.success() {
        if family.needs_build_std() {
            bail!(
                "the core build for {} failed. It needs nightly with rust-src: \
                 rustup toolchain install nightly && \
                 rustup component add rust-src --toolchain nightly",
                family.label()
            );
        }
        bail!("the core build failed");
    }

    eprintln!("==> shell Swift ({})", family.label());
    let sdk = capture("xcrun", &["--sdk", family.sdk(to), "--show-sdk-path"]).with_context(|| {
        format!(
            "the {} SDK is not there. Xcode installs it with: {}",
            family.label(),
            family.download_hint()
        )
    })?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir)?;

    // The shell is the same for all three families. What differs between them
    // is inside, in `#if os(...)`, and it is two things: how the window is born
    // —visionOS has no `UIScreen`, so its own comes from a `UIWindowScene`— and
    // what colour the root's background is.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/ios/Sources"))?;
    if sources.is_empty() {
        bail!("there are no Swift sources in shells/ios/Sources");
    }
    // `shells/shared` brings what does not depend on the platform —the dev
    // server's client—, compiled by every shell.
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    // The plugins: their Swift sources and the registry that hooks them up. It
    // all goes into the same `swiftc` invocation as the shell, so a plugin sees
    // `AnPlugin` and `AnPluginCall` without importing anything.
    for plugin in plugins {
        let contributed = plugins::sources(plugin, Platform::Ios)?;
        eprintln!("==> plugin {} ({} Swift sources)", plugin.module, contributed.len());
        sources.extend(contributed);
    }
    sources.push(
        plugins::generate_ios(plugins, &out.join("generated"))?
            .to_string_lossy()
            .into_owned(),
    );

    let lib_dir = workspace.target_dir().join(family.target(to)).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        family.swift_target(to),
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
    // On a device the entitlements go in the signature and not in the binary,
    // so this section is only written for the simulator. See
    // `write_entitlements`.
    let simulator_entitlements = match to {
        Destination::Simulator => write_entitlements(plugins, &bundle_id, &out)?,
        Destination::Device => None,
    };
    if let Some(entitlements) = simulator_entitlements {
        // The way to get a section into the binary from `swiftc`: four
        // `-Xlinker`s in a row, one per argument `ld` is to receive.
        for flag in ["-sectcreate", "__TEXT", "__entitlements"] {
            args.push("-Xlinker".into());
            args.push(flag.into());
        }
        args.push("-Xlinker".into());
        args.push(entitlements.to_string_lossy().into_owned());
    }
    if release {
        args.push("-O".into());
    }
    args.extend(sources);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "xcrun", &borrowed, "the shell link step failed")?;

    write_plist(&plist, &app_dir.join("Info.plist"), plugins)?;
    std::fs::copy(bundle, app_dir.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(app_dir.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(app_dir.join("dev-server.txt"));
        }
    }

    if let Some(apple) = signing {
        sign(&app_dir, &bundle_id, apple, plugins, &out)?;
    }

    Ok(Package { dir: app_dir, bundle_id, app_name, out, family })
}

/// Signs the `.app` for a real device: the profile goes in the bundle, the
/// entitlements go in the signature.
///
/// Both halves have to be there and both have to agree. The profile alone gives
/// an app that installs and is killed on launch; the entitlements alone give one
/// `installd` refuses with `ApplicationVerificationFailed`, and neither message
/// reaches the terminal this was typed in.
fn sign(
    app_dir: &Path,
    bundle_id: &str,
    apple: &Apple,
    plugins: &[Plugin],
    out: &Path,
) -> Result<()> {
    eprintln!("==> embedded.mobileprovision ({})", apple.profile_name);
    std::fs::copy(&apple.profile, app_dir.join("embedded.mobileprovision")).with_context(|| {
        format!("{} could not be copied into the .app", apple.profile.display())
    })?;

    let entitlements = device_entitlements(apple, bundle_id, plugins, out)?;
    eprintln!("==> codesign ({})", apple.identity_name);
    let signed = Command::new("codesign")
        .args(["--force", "--sign", &apple.identity, "--entitlements"])
        .arg(&entitlements)
        // Xcode's own flag for an iOS bundle. It is what puts the DER form of
        // the entitlements in beside the plist form, which iOS 15 and later
        // want; without it the app installs on some devices and is refused on
        // others, and the ones that refuse say only "invalid entitlements".
        .arg("--generate-entitlement-der")
        .arg(app_dir)
        .output()
        .context("codesign could not be run")?;
    if !signed.status.success() {
        // A raw `codesign` exit code is the failure this whole module exists to
        // prevent, so what it said is repeated with the two things it never
        // mentions: which identity was used and which file it was signing.
        bail!(
            "codesign refused to sign {} with {:?}.\n{}\n\
             The usual causes, in order: the certificate's private key is not in \
             this keychain (the .cer alone is not enough — the .p12 that carries the \
             key is), the keychain is locked, or an entitlement in \
             {} is one the profile {} does not grant.\nSee {}",
            app_dir.display(),
            apple.identity_name,
            String::from_utf8_lossy(&signed.stderr).trim(),
            entitlements.display(),
            apple.profile_name,
            signing::DOCS
        );
    }
    Ok(())
}

/// The entitlements that get signed into a device build.
///
/// The base is **the profile's own dictionary**, and that is not a shortcut: the
/// system grants nothing the profile does not carry, so anything added on top of
/// it produces an app that installs and dies on launch. The plugins' keys are
/// merged in, and the profile wins every collision — it is the authority, and a
/// plugin cannot know which team it was signed for.
fn device_entitlements(
    apple: &Apple,
    bundle_id: &str,
    plugins: &[Plugin],
    out: &Path,
) -> Result<PathBuf> {
    let mut entitlements = apple.profile_entitlements.clone();
    for (key, contributed) in &plugins::entitlement_entries(plugins)? {
        if entitlements.contains_key(key) {
            continue;
        }
        eprintln!("==> entitlements: {key} (from {})", contributed.package);
        entitlements.insert(key.clone(), substitute(&contributed.value, bundle_id));
    }
    // A keychain group on a device is `TEAMID.group`, and on the simulator it is
    // just `group`. A plugin writes the one it can know about, so the team is
    // put in front here — and only when it is not already there, so a plugin
    // that spells it out in full is not given it twice.
    if let Some(serde_json::Value::Array(groups)) = entitlements.get_mut("keychain-access-groups") {
        for group in groups.iter_mut() {
            if let serde_json::Value::String(name) = group {
                if !name.starts_with(&format!("{}.", apple.team)) {
                    *name = format!("{}.{name}", apple.team);
                }
            }
        }
    }

    std::fs::create_dir_all(out)?;
    let json = out.join("device-entitlements.json");
    let plist = out.join("device.entitlements");
    std::fs::write(&json, serde_json::Value::Object(entitlements).to_string())?;
    run_in(
        out,
        "plutil",
        &["-convert", "xml1", "-o", &plist.to_string_lossy(), &json.to_string_lossy()],
        "the entitlements file could not be written",
    )?;
    Ok(plist)
}

/// Writes the entitlements file the link step asks for, if any plugin asks for
/// one. `None` when there are none and there is nothing to embed.
///
/// Entitlements are what lets the app ask the system for something. The case
/// that forced this into existence is the keychain: without
/// `keychain-access-groups` or `application-identifier`, `SecItemAdd` answers
/// −34018 —"the client has neither of the two"— because the app belongs to no
/// keychain group and there is nowhere to save. From the outside it looks like a
/// keychain bug.
///
/// **On the simulator the entitlements do not go in the signature, they go
/// inside the binary**, in the `__TEXT,__entitlements` section asked of the
/// linker a little further down. Signing them —neither ad hoc nor with a
/// development identity— does not work: `keychain-access-groups` is a restricted
/// entitlement, and macOS refuses to run a binary that carries it in its
/// signature without a provisioning profile backing it up. The symptom is that
/// the app stops starting, with a "request denied by SBMainWorkspace" that
/// mentions entitlements nowhere. Xcode does exactly this same thing for the
/// simulator.
///
/// A real device would need the other route —identity and profile— and that is
/// not here: `an` installs on the simulator. See https://angular-native.dev/extending/plugins/.
fn write_entitlements(
    plugins: &[Plugin],
    bundle_id: &str,
    out: &Path,
) -> Result<Option<PathBuf>> {
    let requested = plugins::entitlement_entries(plugins)?;
    if requested.is_empty() {
        return Ok(None);
    }

    let mut entitlements = serde_json::Map::new();
    // The application identifier is put there by `an` and not by the plugin: a
    // plugin does not know —and has no reason to— which app it is going to be
    // dropped into. It is also what gives the `$(BUNDLE_ID)` below its value.
    entitlements.insert(
        "application-identifier".to_owned(),
        serde_json::Value::String(bundle_id.to_owned()),
    );
    for (key, contributed) in &requested {
        eprintln!("==> entitlements: {key} (from {})", contributed.package);
        entitlements.insert(key.clone(), substitute(&contributed.value, bundle_id));
    }

    std::fs::create_dir_all(out)?;
    let json = out.join("entitlements.json");
    let plist = out.join("angular-native.entitlements");
    std::fs::write(&json, serde_json::Value::Object(entitlements).to_string())?;
    // The linker wants a plist, not a JSON. Converting it with `plutil` saves
    // writing XML by hand and escaping the plugin's values along the way.
    run_in(
        out,
        "plutil",
        &["-convert", "xml1", "-o", &plist.to_string_lossy(), &json.to_string_lossy()],
        "the entitlements file could not be written",
    )?;
    Ok(Some(plist))
}

/// Swaps `$(BUNDLE_ID)` for this app's identifier.
///
/// It is the only substitution there is, and it exists because the value that is
/// nearly always asked for —the keychain group— is the app's identifier, and the
/// plugin cannot know it. Xcode does the same with `$(AppIdentifierPrefix)`.
fn substitute(value: &serde_json::Value, bundle_id: &str) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(text.replace("$(BUNDLE_ID)", bundle_id))
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(|item| substitute(item, bundle_id)).collect())
        }
        other => other.clone(),
    }
}

/// Writes the `.app`'s `Info.plist`: the project's plus whatever the plugins
/// ask for.
///
/// A plugin cannot ask for the camera, Face ID or the microphone without a usage
/// key: iOS neither warns nor returns an error, it kills the process the moment
/// the permission is evaluated, and from the outside it looks as though the app
/// closed itself. The key coming with the plugin is what saves whoever installs
/// it from having to know that list by heart.
///
/// The app outranks the plugin. Its `Info.plist` is its own —`an add ios` writes
/// it and from then on it is untouched—, so if it already declares the key, its
/// own stays; but not silently: it says which one was ignored and whose it was.
fn write_plist(base: &Path, destination: &Path, plugins: &[Plugin]) -> Result<()> {
    let contributed_keys = plugins::plist_entries(plugins)?;
    std::fs::copy(base, destination)?;
    if contributed_keys.is_empty() {
        return Ok(());
    }
    let already_there = plist_keys(base)?;
    for (key, contributed) in &contributed_keys {
        if let Some(current) = already_there.get(key) {
            if current != &contributed.value {
                eprintln!(
                    "==> Info.plist: {key} is already declared by the app\n    \
                     ({current}); ignoring {}'s ({})",
                    contributed.package, contributed.value
                );
            }
            continue;
        }
        eprintln!("==> Info.plist: {key} (from {})", contributed.package);
        run_in(
            destination.parent().unwrap_or(destination),
            "plutil",
            &[
                "-replace",
                key,
                "-json",
                &contributed.value.to_string(),
                &destination.to_string_lossy(),
            ],
            "the key a plugin asks for could not be written into the Info.plist",
        )?;
    }
    Ok(())
}

/// The top-level keys of an `Info.plist`, actually read.
///
/// It is converted to JSON with `plutil` instead of grepping the XML for
/// `<key>`: a plist can come in binary form, and looking for text inside a
/// binary finds nothing and would have you believe the app declares no keys at
/// all.
fn plist_keys(plist: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    let json = capture("plutil", &["-convert", "json", "-o", "-", &plist.to_string_lossy()])
        .with_context(|| format!("{}: it could not be read", plist.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&json)
        .with_context(|| format!("{}: plutil returned something that is not JSON", plist.display()))?;
    match parsed {
        serde_json::Value::Object(map) => Ok(map),
        _ => bail!("{}: the root of an Info.plist has to be a dictionary", plist.display()),
    }
}

/// That the `Info.plist` says the same thing as the project.
///
/// `CFBundleExecutable` has to be the name of the binary that was just linked,
/// and `CFBundleIdentifier` the same one it later gets installed with. If
/// somebody changes `app.name` in `angular-native.json` and leaves the plist
/// alone, the app installs and vanishes on opening without a word: the system
/// goes looking for an executable that is not there. It is exactly the kind of
/// silent failure that must not happen, so it is compared here.
fn check_plist(
    plist: &Path,
    app_name: &str,
    bundle_id: &str,
    family: Family,
    workspace: &Workspace,
) -> Result<()> {
    for (key, expected) in
        [("CFBundleExecutable", app_name), ("CFBundleIdentifier", bundle_id)]
    {
        let read = capture(
            "plutil",
            &["-extract", key, "raw", "-o", "-", &plist.to_string_lossy()],
        )
        .with_context(|| format!("{}: {key} could not be read", plist.display()))?;
        if read != expected {
            // Outside the monorepo and with no overlay, what is being compared
            // is the SDK's plist against the project's name: they never match,
            // and the way out is not fixing a file that is not theirs.
            let way_out = if workspace.project.is_some()
                && workspace.overlay(family.slug(), "Info.plist").is_none()
            {
                format!(
                    "This project has not got one of its own yet: run `an add {}`.",
                    family.slug()
                )
            } else {
                "Either the plist is fixed or angular-native.json is; with the two of \
                 them different the app installs and does not open."
                    .to_owned()
            };
            bail!(
                "{}: {key} is {read:?} and the project says {expected:?}.\n{way_out}",
                plist.display()
            );
        }
    }
    Ok(())
}

/// The `.swift`s in a directory, in a stable order: `read_dir` hands them back
/// in whatever order the filesystem feels like, and with that the `swiftc`
/// command would differ between machines without anything having changed.
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
    let udid = find_device(family, device)?;
    eprintln!("==> simulator: {device}");
    let _ = Command::new("xcrun").args(["simctl", "boot", &udid]).output();
    let _ = Command::new("open")
        .args(["-a", "Simulator", "--args", "-CurrentDeviceUDID", &udid])
        .status();
    // Installing onto a half-booted simulator leaves the command hanging
    // without a word. `bootstatus` waits for the boot to really finish.
    let ready = Command::new("xcrun")
        .args(["simctl", "bootstatus", &udid, "-b"])
        .status()
        .context("waiting for the simulator to boot was not possible")?;
    if !ready.success() {
        bail!("the simulator {device} never got as far as booting");
    }

    // Quit and uninstall before installing.
    //
    // `simctl install` over an app that is already installed does not replace
    // the bundle reliably: the app starts with the old code and it looks as
    // though the change never landed. Uninstalling also wipes the app's data,
    // which in a development cycle is what anyone expects anyway. Both commands
    // fail if there was nothing there, which is the normal case the first time
    // round, so their output is thrown away.
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
        .context("the app could not be installed")?;
    if !install.success() {
        bail!("installing on the simulator failed");
    }

    let launch = Command::new("xcrun")
        .args(["simctl", "launch", &udid, &package.bundle_id])
        .status()
        .context("the app could not be launched")?;
    if !launch.success() {
        bail!("the launch failed");
    }
    Ok(())
}

/// Looks a simulator of that family up by name and returns its udid.
///
/// The JSON is really parsed. Grepping for the name and reading the next `udid`
/// does not work: `simctl` puts the `udid` *before* the `name`, so that gives
/// you the one belonging to the device after it and you end up installing on the
/// wrong simulator, with nothing failing anywhere.
///
/// The runtime is filtered by family for the same reason: installing a tvOS app
/// on an iPhone fails much later and confusingly.
fn find_device(family: Family, name: &str) -> Result<String> {
    let json = capture("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).context("simctl returned a JSON nobody can make sense of")?;
    let runtimes = parsed
        .get("devices")
        .and_then(serde_json::Value::as_object)
        .context("simctl's JSON carries no devices")?;

    // An already-booted one is preferred: if there are several with the same
    // name on different versions of the system, that is the one the user is
    // looking at.
    let mut fallback = None;
    let mut family_present = false;
    for (runtime, devices) in runtimes {
        if !runtime.contains(family.runtime_marker()) {
            continue;
        }
        family_present = true;
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

    // With no runtime of the family at all the problem is not the device's
    // name: it is that the download is missing. Having the SDK installed —which
    // is what compiling and linking need— does not bring the runtime, which is
    // what running needs. They are separate gigabytes, and saying "there is no
    // simulator called X" would send people looking in the wrong place.
    if !family_present {
        bail!(
            "there is no {} runtime installed, so there is no simulator to boot.\n\
             The `.app` is built; to be able to run it:\n    {}",
            family.label(),
            family.download_hint()
        );
    }
    bail!("there is no {} simulator called {name:?}", family.label())
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

// ---------------------------------------------------------------------------
// A real device
// ---------------------------------------------------------------------------

/// Installs the signed `.app` on a device plugged into this Mac, and launches
/// it.
///
/// `devicectl` and not `ios-deploy`: since Xcode 15 it is the only tool Apple
/// ships that talks to a modern device, and it is already on the machine of
/// anybody who can build this at all.
pub fn install_on_device(package: &Package, wanted: Option<&str>) -> Result<()> {
    require_devicectl()?;
    let (udid, name) = find_connected(wanted)?;
    eprintln!("==> device: {name}");

    let install = Command::new("xcrun")
        .args(["devicectl", "device", "install", "app", "--device", &udid])
        .arg(&package.dir)
        .output()
        .context("devicectl could not be run")?;
    if !install.status.success() {
        let said = String::from_utf8_lossy(&install.stderr);
        // The three refusals that mean something specific, translated back into
        // what has to be done. Everything else is passed through: devicectl's
        // own message is better than a guess at what it meant.
        let hint = if said.contains("developer mode") || said.contains("DeveloperMode") {
            "\nDeveloper Mode is off on that device. Settings ▸ Privacy & Security ▸ \
             Developer Mode, switch it on, and the device restarts."
        } else if said.contains("not paired") || said.contains("Unable to connect") {
            "\nThe device is not trusted by this Mac: unlock it, and answer Trust to \
             the prompt that appears when it is plugged in."
        } else if said.contains("ApplicationVerificationFailed") || said.contains("valid provisioning") {
            "\nThe device is not in the provisioning profile. Add its UDID at \
             https://developer.apple.com/account/resources/devices/list, regenerate \
             the profile and download it again."
        } else {
            ""
        };
        bail!(
            "devicectl could not install {} on {name}.\n{}{hint}\nSee {}",
            package.dir.display(),
            said.trim(),
            signing::DOCS
        );
    }

    let launch = Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--device",
            &udid,
            &package.bundle_id,
        ])
        .status()
        .context("devicectl could not be run")?;
    if !launch.success() {
        bail!(
            "{} is installed on {name} but would not launch. Open it from the home \
             screen: if it bounces and closes, the entitlements and the profile \
             disagree.\nSee {}",
            package.bundle_id,
            signing::DOCS
        );
    }
    Ok(())
}

/// That this Xcode has `devicectl` at all.
///
/// Xcode 14 and earlier had `instruments -s devices` and nothing that installs.
/// Saying "xcrun: devicectl: command not found" would send somebody looking for
/// a tool to install, and there is none: the answer is a newer Xcode.
fn require_devicectl() -> Result<()> {
    let found = Command::new("xcrun")
        .args(["devicectl", "--version"])
        .output()
        .context("xcrun could not be run")?;
    if found.status.success() {
        return Ok(());
    }
    bail!(
        "this Xcode has no `devicectl`, and that is what installs on a device.\n\
         It arrived with Xcode 15; `xcodebuild -version` says which one this is. \
         There is nothing to install separately — it comes with Xcode.\n\
         The `.app` is built and signed either way: `an ios --physical --no-launch` \
         leaves it where you can drag it onto a device from Xcode's Devices window.\n\
         See {}",
        signing::DOCS
    )
}

/// The device to install on, by name or UDID, or the only one there is.
///
/// Same rule as `an android`: with one connected device it is used, with several
/// you are asked which, and what there is gets listed. A wrong guess here
/// installs a build on somebody's phone without saying so.
fn find_connected(wanted: Option<&str>) -> Result<(String, String)> {
    let listing = std::env::temp_dir().join("an-devicectl.json");
    let listed = Command::new("xcrun")
        .args(["devicectl", "list", "devices", "--json-output"])
        .arg(&listing)
        .output()
        .context("devicectl could not be run")?;
    if !listed.status.success() {
        bail!(
            "devicectl could not list the devices.\n{}",
            String::from_utf8_lossy(&listed.stderr).trim()
        );
    }
    let text = std::fs::read_to_string(&listing)
        .with_context(|| format!("{} could not be read", listing.display()))?;
    let _ = std::fs::remove_file(&listing);
    let found = parse_devices(&text)?;

    if let Some(wanted) = wanted {
        return found
            .iter()
            .find(|(udid, name)| udid == wanted || name == wanted)
            .cloned()
            .with_context(|| {
                format!(
                    "there is no connected device called {wanted:?}. What there is:\n{}",
                    describe(&found)
                )
            });
    }
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => bail!(
            "there is no device connected. Plug an iPhone or iPad in over USB, unlock \
             it, and switch Developer Mode on in Settings ▸ Privacy & Security.\n\
             `an ios` with no --physical goes to the simulator and needs none of \
             that.\nSee {}",
            signing::DOCS
        ),
        several => bail!(
            "there are {} devices connected:\n{}\nPick one with --device.",
            several.len(),
            describe(several)
        ),
    }
}

fn describe(devices: &[(String, String)]) -> String {
    devices
        .iter()
        .map(|(udid, name)| format!("\x20   {name}  ({udid})"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The `(udid, name)` pairs out of what `devicectl list devices --json-output`
/// wrote.
///
/// The UDID that matters is `hardwareProperties.udid` and not the `identifier`
/// next to it: the second one is devicectl's own record of the pairing, it
/// changes when a device is unpaired, and `devicectl device install` takes
/// either — which is what makes picking the wrong one a bug that works on the
/// machine it was written on.
pub fn parse_devices(json: &str) -> Result<Vec<(String, String)>> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).context("devicectl returned a JSON nobody can make sense of")?;
    let mut found = Vec::new();
    let devices = parsed
        .get("result")
        .and_then(|result| result.get("devices"))
        .and_then(serde_json::Value::as_array);
    for device in devices.into_iter().flatten() {
        let hardware = device.get("hardwareProperties");
        let Some(udid) = hardware
            .and_then(|hardware| hardware.get("udid"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let name = device
            .get("deviceProperties")
            .and_then(|properties| properties.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unnamed");
        found.push((udid.to_owned(), name.to_owned()));
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The archive and the .ipa
// ---------------------------------------------------------------------------

/// Turns a signed `.app` into an `.xcarchive` and an `.ipa`.
///
/// Both are directories with a shape and nothing more: an archive is the app
/// under `Products/Applications` with an `Info.plist` describing it, and an
/// `.ipa` is a zip with the app under `Payload/`. Neither format needs Xcode to
/// produce — Xcode is needed to *sign*, and that has already happened by the
/// time this runs.
///
/// Which store the `.ipa` can go to is decided entirely by what it was signed
/// with. An App Store upload wants an Apple Distribution certificate and an App
/// Store profile; TestFlight wants the same; an ad-hoc build wants an ad-hoc
/// profile listing the devices. There is no `--method` flag here because there
/// is nothing for it to do: the profile already said which of those this is.
pub fn archive(package: &Package, apple: &Apple) -> Result<(PathBuf, PathBuf)> {
    let archive = package.out.join(format!("{}.xcarchive", package.app_name));
    let _ = std::fs::remove_dir_all(&archive);
    let applications = archive.join("Products/Applications");
    std::fs::create_dir_all(&applications)?;
    std::fs::create_dir_all(archive.join("dSYMs"))?;
    copy_tree(&package.dir, &applications.join(format!("{}.app", package.app_name)))?;

    // The debug symbols. Without them a crash report from TestFlight is a list
    // of addresses, and there is no second chance to produce them: they only
    // exist next to the binary they came out of.
    let symbols = archive.join(format!("dSYMs/{}.app.dSYM", package.app_name));
    let extracted = Command::new("dsymutil")
        .arg(package.dir.join(&package.app_name))
        .arg("-o")
        .arg(&symbols)
        .output()
        .context("dsymutil could not be run")?;
    if !extracted.status.success() {
        // Not fatal, and said out loud rather than swallowed: the archive is
        // still a valid archive, it just cannot symbolicate anything.
        eprintln!(
            "==> warning: dsymutil left no symbols, so crash reports from this build \
             will not symbolicate.\n    {}",
            String::from_utf8_lossy(&extracted.stderr).trim()
        );
    }

    let version = plist_value(&package.dir.join("Info.plist"), "CFBundleShortVersionString")
        .unwrap_or_else(|| "1.0".to_owned());
    let build = plist_value(&package.dir.join("Info.plist"), "CFBundleVersion")
        .unwrap_or_else(|| "1".to_owned());
    std::fs::write(
        archive.join("Info.plist"),
        archive_plist(package, apple, &version, &build),
    )?;
    eprintln!("==> {}", archive.display());

    // And the `.ipa`. `ditto -c -k` and not `zip`, because it is the only
    // zipper on this machine that keeps a bundle's symlinks and resource forks
    // intact; a `.app` that went through plain `zip` is refused by App Store
    // Connect for a reason that mentions neither.
    let payload = package.out.join("Payload");
    let _ = std::fs::remove_dir_all(&payload);
    std::fs::create_dir_all(&payload)?;
    copy_tree(&package.dir, &payload.join(format!("{}.app", package.app_name)))?;
    let ipa = package.out.join(format!("{}.ipa", package.app_name));
    let _ = std::fs::remove_file(&ipa);
    run_in(
        &package.out,
        "ditto",
        &["-c", "-k", "--sequesterRsrc", "--keepParent", "Payload", &ipa.to_string_lossy()],
        "the .ipa could not be packed",
    )?;
    let _ = std::fs::remove_dir_all(&payload);
    let size = std::fs::metadata(&ipa)?.len();
    eprintln!("==> {} MB in {}", size / (1024 * 1024), ipa.display());
    Ok((archive, ipa))
}

/// An archive's `Info.plist`.
///
/// Written by hand and not through `plutil` because `CreationDate` is a
/// `<date>`, and a plist holding a date is one `plutil -convert json` refuses
/// to read or write. Xcode reads this file to list the archive in the Organizer;
/// the keys are the ones it looks at.
fn archive_plist(package: &Package, apple: &Apple, version: &str, build: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>ApplicationProperties</key>
	<dict>
		<key>ApplicationPath</key>
		<string>Applications/{name}.app</string>
		<key>CFBundleIdentifier</key>
		<string>{bundle_id}</string>
		<key>CFBundleShortVersionString</key>
		<string>{version}</string>
		<key>CFBundleVersion</key>
		<string>{build}</string>
		<key>SigningIdentity</key>
		<string>{identity}</string>
		<key>Team</key>
		<string>{team}</string>
	</dict>
	<key>ArchiveVersion</key>
	<integer>2</integer>
	<key>CreationDate</key>
	<date>{now}</date>
	<key>Name</key>
	<string>{name}</string>
	<key>SchemeName</key>
	<string>{name}</string>
</dict>
</plist>
"#,
        name = package.app_name,
        bundle_id = package.bundle_id,
        identity = apple.identity_name,
        team = apple.team,
        now = signing::now_iso8601()
    )
}

/// One key out of a plist, or nothing if it is not there. Nothing is a fine
/// answer: the archive falls back to 1.0 rather than refusing to be written
/// because a version is missing.
fn plist_value(plist: &Path, key: &str) -> Option<String> {
    let output = Command::new("plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(plist)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Copies a bundle. `ditto` and not a recursive `std::fs::copy` loop: a signed
/// `.app` carries a `_CodeSignature` directory and symlinks, and a copy that
/// resolves the symlinks produces a bundle whose signature no longer verifies.
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
