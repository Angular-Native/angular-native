//! Builds the watch `.app` and takes it to the watchOS simulator.
//!
//! The same thing `ios.rs` does —no `.xcodeproj`, `swiftc` linking against
//! Rust's staticlib— with three differences that are not cosmetic:
//!
//! 1. **Nightly is required.** `aarch64-apple-watchos-sim` is a tier 3 target
//!    and ships no precompiled `std`, so it has to be built on the spot with
//!    `-Z build-std`. That is why this subcommand calls `cargo +nightly` rather
//!    than the `cargo` from `rust-toolchain.toml`.
//! 2. **`-parse-as-library`.** The watch shell starts at an `@main` on a SwiftUI
//!    `App`. Without this flag `swiftc` treats the first file as a top-level
//!    script and the `@main` goes unused.
//! 3. **The bundle is a watch's.** `WKApplication` in the `Info.plist` and
//!    device family 4; without those `simctl` installs something it then cannot
//!    launch.
//! 4. **A plugin has to declare watchOS on purpose.** The watch runs the same
//!    registry as the phone —see `crates/an-watch/src/plugins.rs`— but it is
//!    not the same platform in the manifest, and that is not bureaucracy: a
//!    watch has no pasteboard and no biometric sensor, so half the plugins that
//!    build for a phone cannot exist here. The refusal names the plugin and the
//!    reason it gave; see `plugins::require`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::run;
use crate::ios::swift_sources;
use crate::plugins::{self, Platform, Plugin};
use crate::workspace::Workspace;

/// Every plugin has to bring its watchOS half, and the ones that do not are
/// named along with whatever they said about it.
///
/// The watch used to refuse plugins outright, and the refusal was honest but
/// blunt: it said the host loads none, which stopped being true the moment
/// `an-watch` grew a registry. What is refused now is narrower and truer — the
/// plugin that cannot run on a watch, by name, with its own sentence about why.
pub fn require_plugins(plugins: &[Plugin]) -> Result<()> {
    plugins::require(plugins, Platform::Watchos)
}

const APP_NAME: &str = "AngularNativeWatch";
const BUNDLE_ID: &str = "dev.angularnative.playground.watchkitapp";
const TARGET: &str = "aarch64-apple-watchos-sim";
/// watchOS 11 is the oldest one where `@Observable` and the SwiftUI additions
/// the shell uses are available without `@available` sprinkled everywhere.
const DEPLOYMENT: &str = "11.0";

pub struct Package {
    pub dir: PathBuf,
}

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
) -> Result<Package> {
    // Before compiling anything: if some plugin does not bring its watchOS
    // half, the build stops here and says which one and why.
    require_plugins(plugins)?;
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_dir = workspace.build_dir().join("watchos").join(format!("{APP_NAME}.app"));

    eprintln!("==> core Rust ({profile}, {TARGET})");
    // `+nightly` and `build-std`: see the header. If this fails because the
    // component is missing, cargo's message already says which one, so it is let
    // through as-is rather than guessed at.
    let mut cargo_args = vec![
        "+nightly",
        "build",
        "-Z",
        "build-std=std,panic_abort",
        "--target",
        TARGET,
        "-p",
        "an-watch",
    ];
    if release {
        cargo_args.push("--release");
    }
    // QuickJS is built with `cc`, which without this uses the SDK's minimum and
    // the Swift link step complains about the mismatch.
    let status = Command::new("cargo")
        .args(&cargo_args)
        .env("WATCHOS_DEPLOYMENT_TARGET", DEPLOYMENT)
        .current_dir(root)
        .status()
        .context("cargo could not be run")?;
    if !status.success() {
        bail!(
            "the core build for watchOS failed. It needs nightly with rust-src: \
             rustup toolchain install nightly && rustup component add rust-src --toolchain nightly"
        );
    }

    eprintln!("==> shell SwiftUI");
    let sdk = capture("xcrun", &["--sdk", "watchsimulator", "--show-sdk-path"])?;
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir)?;

    // `shells/shared` brings what does not depend on the platform —the dev
    // server's client—, compiled by both shells.
    let mut sources: Vec<String> = swift_sources(&root.join("shells/watchos/Sources"))?;
    if sources.is_empty() {
        bail!("there are no Swift sources in shells/watchos/Sources");
    }
    sources.extend(swift_sources(&root.join("shells/shared"))?);

    // The plugins: their Swift sources and the registry that hooks them up,
    // into the same `swiftc` invocation as the shell so a plugin sees
    // `AnPlugin` and `AnPluginCall` without importing anything.
    let generated = app_dir.parent().unwrap_or(&app_dir).join("generated");
    for plugin in plugins {
        let contributed = plugins::sources(plugin, Platform::Watchos)?;
        eprintln!("==> plugin {} ({} Swift sources)", plugin.module, contributed.len());
        sources.extend(contributed);
    }
    sources.push(
        plugins::generate_swift(plugins, Platform::Watchos, &generated)?
            .to_string_lossy()
            .into_owned(),
    );

    let lib_dir = workspace.target_dir().join(TARGET).join(profile);
    let mut args: Vec<String> = vec![
        "swiftc".into(),
        "-sdk".into(),
        sdk.clone(),
        "-target".into(),
        format!("arm64-apple-watchos{DEPLOYMENT}-simulator"),
        // Without this the `@main` goes unused: swiftc would treat a file as a
        // script.
        "-parse-as-library".into(),
        "-import-objc-header".into(),
        root.join("shells/watchos/Sources/Bridging-Header.h").to_string_lossy().into_owned(),
        "-I".into(),
        root.join("crates/an-watch/include").to_string_lossy().into_owned(),
        "-L".into(),
        lib_dir.to_string_lossy().into_owned(),
        "-lan_watch".into(),
        "-Xclang-linker".into(),
        "-isysroot".into(),
        "-Xclang-linker".into(),
        sdk,
        "-o".into(),
        app_dir.join(APP_NAME).to_string_lossy().into_owned(),
    ];
    // The entitlements, if any plugin asks for one. **On the simulator they go
    // inside the binary and not in a signature** — the same thing `an ios` does
    // and for the same reason: `keychain-access-groups` is a restricted
    // entitlement, and a simulator binary carrying it in its signature without a
    // provisioning profile behind it is refused at launch, with a message that
    // mentions entitlements nowhere. Xcode embeds it exactly like this.
    if let Some(entitlements) = write_entitlements(plugins, BUNDLE_ID, &generated)? {
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

    write_plist(
        &root.join("shells/watchos/Resources/Info.plist"),
        &app_dir.join("Info.plist"),
        plugins,
    )?;
    std::fs::copy(bundle, app_dir.join("main.js"))?;
    match dev_server {
        Some(url) => std::fs::write(app_dir.join("dev-server.txt"), url)?,
        None => {
            let _ = std::fs::remove_file(app_dir.join("dev-server.txt"));
        }
    }

    Ok(Package { dir: app_dir })
}

pub fn launch(package: &Package, device: &str) -> Result<()> {
    let udid = find_device(device)?;
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

    // Quit and uninstall before installing: `simctl install` over an app that is
    // already there does not replace the bundle reliably. Both fail if there was
    // nothing, which is the normal case the first time round.
    let _ = Command::new("xcrun").args(["simctl", "terminate", &udid, BUNDLE_ID]).output();
    let _ = Command::new("xcrun").args(["simctl", "uninstall", &udid, BUNDLE_ID]).output();
    let install = Command::new("xcrun")
        .args(["simctl", "install", &udid])
        .arg(&package.dir)
        .status()
        .context("the app could not be installed")?;
    if !install.success() {
        bail!("installing on the simulator failed");
    }

    let launch = Command::new("xcrun")
        .args(["simctl", "launch", &udid, BUNDLE_ID])
        .status()
        .context("the app could not be launched")?;
    if !launch.success() {
        bail!("the launch failed");
    }
    Ok(())
}

/// Looks a watch up by name and returns its udid.
///
/// The JSON is really parsed, for the same reason as on iOS: `simctl` puts the
/// `udid` *before* the `name`, so grepping for the name and reading the next
/// `udid` gives you the one belonging to the device after it.
fn find_device(name: &str) -> Result<String> {
    let json = capture("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).context("simctl returned a JSON nobody can make sense of")?;
    let runtimes = parsed
        .get("devices")
        .and_then(serde_json::Value::as_object)
        .context("simctl's JSON carries no devices")?;

    let mut fallback = None;
    for (runtime, devices) in runtimes {
        // Watches only: there are iPhones and iPads with similar names, and
        // putting a watchOS app on an iPhone fails much later and confusingly.
        if !runtime.contains("watchOS") {
            continue;
        }
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
    fallback.with_context(|| format!("there is no watch simulator called {name:?}"))
}

/// Writes the `.app`'s `Info.plist`: the shell's plus whatever the plugins ask
/// for.
///
/// A watch asks for fewer usage keys than a phone, but the ones it does ask for
/// are just as fatal when they are missing: watchOS kills the process the moment
/// the permission is evaluated and says nothing about the key. The app's own
/// plist outranks the plugin, and what was ignored gets said out loud.
fn write_plist(base: &Path, destination: &Path, plugins: &[Plugin]) -> Result<()> {
    let contributed = plugins::plist_entries(plugins, Platform::Watchos)?;
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

/// The top-level keys of an `Info.plist`, actually read. `plutil` and not a grep
/// for `<key>`: a plist can be binary, and grepping a binary finds nothing and
/// would have you believe the app declares no keys at all.
fn plist_keys(plist: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    let json = capture("plutil", &["-convert", "json", "-o", "-", &plist.to_string_lossy()])
        .with_context(|| format!("{}: it could not be read", plist.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&json).with_context(|| {
        format!("{}: plutil returned something that is not JSON", plist.display())
    })?;
    match parsed {
        serde_json::Value::Object(map) => Ok(map),
        _ => bail!("{}: the root of an Info.plist has to be a dictionary", plist.display()),
    }
}

/// The entitlements the link step embeds, if any plugin asks for one. `None`
/// when there are none and there is nothing to embed.
///
/// On a watch the one that matters is the keychain's: without
/// `keychain-access-groups` or `application-identifier` the app belongs to no
/// keychain group, `SecItemAdd` answers −34018 and there is nowhere to save. The
/// error names neither the signature nor the entitlement, so from the outside it
/// looks like a keychain that is broken rather than an app that never asked for
/// one.
///
/// This is `an ios`'s `write_entitlements` doing the same job on the same kind of
/// simulator binary. What a real watch would need is the other route —an identity
/// and a provisioning profile— and that is not here: `an watchos` installs on the
/// simulator.
fn write_entitlements(
    plugins: &[Plugin],
    bundle_id: &str,
    out: &Path,
) -> Result<Option<PathBuf>> {
    let requested = plugins::entitlement_entries(plugins, Platform::Watchos)?;
    if requested.is_empty() {
        return Ok(None);
    }
    let mut entitlements = serde_json::Map::new();
    // Put there by `an` and not by the plugin: a plugin does not know —and has
    // no reason to— which app it is going to be dropped into. It is also what
    // gives `$(BUNDLE_ID)` its value.
    entitlements.insert(
        "application-identifier".to_owned(),
        serde_json::Value::String(bundle_id.to_owned()),
    );
    for (key, entry) in &requested {
        eprintln!("==> entitlements: {key} (from {})", entry.package);
        entitlements.insert(key.clone(), substitute(&entry.value, bundle_id));
    }

    std::fs::create_dir_all(out)?;
    let json = out.join("entitlements.json");
    let plist = out.join("angular-native.entitlements");
    std::fs::write(&json, serde_json::Value::Object(entitlements).to_string())?;
    // The linker wants a plist, not a JSON. `plutil` saves writing the XML by
    // hand and escaping the plugins' values along the way.
    let converted = Command::new("plutil")
        .args(["-convert", "xml1", "-o"])
        .arg(&plist)
        .arg(&json)
        .status()
        .context("plutil could not be run")?;
    if !converted.success() {
        bail!("{} could not be written", plist.display());
    }
    Ok(Some(plist))
}

/// Swaps `$(BUNDLE_ID)` for this app's identifier: the keychain group is nearly
/// always the app's own identifier and the plugin cannot know it.
fn substitute(value: &serde_json::Value, bundle_id: &str) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(text.replace("$(BUNDLE_ID)", bundle_id))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items.iter().map(|item| substitute(item, bundle_id)).collect(),
        ),
        other => other.clone(),
    }
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
