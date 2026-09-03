//! Finding plugins and linking them in.
//!
//! A plugin is an npm package that brings native sources along with its
//! TypeScript. There is nothing to install here and no separate file to keep up
//! to date: what the app declares as a dependency is what gets linked, and the
//! manifest lives in the plugin's own `package.json`.
//!
//! That this fits in one file is the payoff for having neither an `.xcodeproj`
//! nor Gradle. In Capacitor this part is one script that edits the Xcode project
//! and another that writes a `settings.gradle`; here it is reading a JSON,
//! adding a few paths to the `swiftc` line and writing a registry file.
//!
//! The one thing this module never does is keep quiet: a plugin that does not
//! cover the platform being compiled stops the build. An app never ships with a
//! method that swallows the call.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::workspace::Workspace;

/// Every platform an app can be built for, which is every platform a plugin can
/// be asked to cover.
///
/// The four Apple ones do not collapse into one. `ios` covers the phone, the TV
/// and the headset because those three share a shell and a `UIKit`; macOS and
/// watchOS do not, and that is not a packaging detail: `NSPasteboard` is not
/// `UIPasteboard`, and a watch has no pasteboard at all. A plugin that declared
/// one Apple half for all four would compile for the Mac and fail to link, or
/// —worse— link and answer nonsense.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Ios,
    Android,
    Macos,
    Watchos,
}

impl Platform {
    /// Its key inside `angularNative`.
    pub fn key(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Android => "android",
            Platform::Macos => "macos",
            Platform::Watchos => "watchos",
        }
    }

    /// The extension its sources carry.
    fn extension(self) -> &'static str {
        match self {
            Platform::Ios | Platform::Macos | Platform::Watchos => "swift",
            Platform::Android => "java",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Ios => "iOS",
            Platform::Android => "Android",
            Platform::Macos => "macOS",
            Platform::Watchos => "watchOS",
        }
    }
}

/// A plugin's native half for one platform.
pub struct Native {
    /// The directory holding the sources, absolute.
    pub sources: PathBuf,
    /// The type that implements `AnPlugin`. On Android, with its package in
    /// front.
    pub register: String,
    /// What it contributes to its platform's manifest.
    pub contributes: Contributions,
}

/// What a plugin contributes to the platform's manifest.
///
/// They do not share a shape because they are not the same thing: an
/// `Info.plist` key has a value and no element name, and a `<uses-permission>`
/// is the other way round. A common type would force a translation between the
/// two to be invented, and that translation would be a lie in both directions.
pub enum Contributions {
    /// Any Apple platform: `plist` and `entitlements` under its own key.
    ///
    /// Both are dictionaries and both merge the same way, but they end up in
    /// different files inside the `.app` and they are for different things: the
    /// `Info.plist` says what the app tells the user, and the entitlements say
    /// what the system lets it do.
    ///
    /// The shape is shared by iOS, macOS and watchOS; the values are not, and
    /// they are read from each platform's own section. A Mac wants
    /// `com.apple.security.*` where a phone wants none of them, and a watch
    /// wants neither. See [`Platform`].
    Apple {
        plist: BTreeMap<String, Value>,
        entitlements: BTreeMap<String, Value>,
    },
    /// `angularNative.android.manifest`.
    Manifest(ManifestEntries),
}

/// The elements a plugin puts into the `AndroidManifest.xml`.
#[derive(Default)]
pub struct ManifestEntries {
    /// `<uses-permission android:name="…"/>`. It is a set: asking twice for the
    /// same permission is asking once.
    pub permissions: BTreeSet<String>,
    /// `<uses-feature android:name="…" android:required="…"/>`, from the name to
    /// the `required`. Here there can be a clash: two plugins asking for the
    /// same feature, one required and one not, are not saying the same thing.
    pub features: BTreeMap<String, bool>,
}

impl ManifestEntries {
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty() && self.features.is_empty()
    }
}

/// A manifest entry along with the package that asked for it.
///
/// The owner is not decoration: when two plugins ask for the same thing with
/// different values, the only thing that helps is knowing which two they are.
pub struct Contributed {
    pub value: Value,
    pub package: String,
}

pub struct Plugin {
    /// The npm name, which is how the app declared it.
    pub package: String,
    /// The package's root, absolute and resolved (npm's workspace links point at
    /// the real directory).
    pub dir: PathBuf,
    /// The name JS calls it by.
    pub module: String,
    /// The `.ts` that exports its API, if it ships one in source.
    pub entry: Option<PathBuf>,
    pub ios: Option<Native>,
    pub android: Option<Native>,
    pub macos: Option<Native>,
    pub watchos: Option<Native>,
    /// Per platform key, the plugin's own words for why it cannot exist there.
    ///
    /// It is written as `angularNative.<platform>.unsupported`, and it is the
    /// difference between a build that stops saying "this plugin does not cover
    /// watchOS" —which anybody could have guessed from the `package.json`— and
    /// one that says "watchOS has no `UIPasteboard`". Only the person who wrote
    /// the plugin knows the second, so only they can say it.
    pub unsupported: BTreeMap<&'static str, String>,
}

impl Plugin {
    pub fn native(&self, platform: Platform) -> Option<&Native> {
        match platform {
            Platform::Ios => self.ios.as_ref(),
            Platform::Android => self.android.as_ref(),
            Platform::Macos => self.macos.as_ref(),
            Platform::Watchos => self.watchos.as_ref(),
        }
    }

    /// Why this plugin says it cannot cover that platform, if it says anything.
    pub fn unsupported(&self, platform: Platform) -> Option<&str> {
        self.unsupported.get(platform.key()).map(String::as_str)
    }

    /// Which platforms it covers, for showing.
    pub fn coverage(&self) -> String {
        let covered: Vec<&str> = ALL_PLATFORMS
            .iter()
            .filter(|platform| self.native(**platform).is_some())
            .map(|platform| platform.key())
            .collect();
        if covered.is_empty() {
            return "none".to_owned();
        }
        covered.join(" + ")
    }
}

/// Every platform, in the order they are listed in. It is the order `an plugins`
/// prints and the order the messages name them in, so it is written down once.
pub const ALL_PLATFORMS: [Platform; 4] =
    [Platform::Ios, Platform::Android, Platform::Macos, Platform::Watchos];

/// An app's plugins: those of its dependencies that carry a manifest.
///
/// The order is alphabetical by module name, not the `package.json`'s: an npm
/// `HashMap` promises no order and the generated registry file would change from
/// one run to the next without anything having changed.
pub fn discover(workspace: &Workspace, app: &Path) -> Result<Vec<Plugin>> {
    let app_dir = workspace.root.join(app);
    let manifest = app_dir.join("package.json");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        // An app does not have to be an npm package. If it is not, it has no
        // dependencies, so it has no plugins.
        return Ok(Vec::new());
    };
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", manifest.display()))?;

    let mut by_module: BTreeMap<String, Plugin> = BTreeMap::new();
    let dependencies = parsed.get("dependencies").and_then(Value::as_object);
    for name in dependencies.into_iter().flatten().map(|(name, _)| name) {
        let Some(dir) = resolve_package(workspace, &app_dir, name) else {
            // A missing ordinary dependency is not our business: `ngc` will say
            // so. Only the ones that exist and are plugins matter here.
            continue;
        };
        let Some(plugin) = read_manifest(name, &dir)? else {
            continue;
        };
        if let Some(previous) = by_module.insert(plugin.module.clone(), plugin) {
            let module = previous.module;
            bail!(
                "two plugins claim to be called {module:?}: {} and another one. \
                 The module name has to be unique within the app.",
                previous.package
            );
        }
    }
    Ok(by_module.into_values().collect())
}

/// Resolves a package the way Node would: the app's `node_modules` first, then
/// the root's —which is where npm puts the workspace ones—.
///
/// The root only counts inside the monorepo. For a project from outside, the
/// SDK's `node_modules` is not a place its app can depend on: linking a plugin
/// from there that its `package.json` does not declare would be linking
/// something that is not on the machine next door.
fn resolve_package(workspace: &Workspace, app_dir: &Path, name: &str) -> Option<PathBuf> {
    let mut bases: Vec<&Path> = vec![app_dir];
    if workspace.project.is_none() {
        bases.push(workspace.root.as_path());
    }
    for base in bases {
        let candidate = base.join("node_modules").join(name);
        if candidate.join("package.json").is_file() {
            // npm's workspaces are links; what matters is the real directory,
            // the one inside the repo and the one `ngc` compiles.
            return candidate.canonicalize().ok().or(Some(candidate));
        }
    }
    None
}

/// Reads a package's manifest. `Ok(None)` if it is not a plugin.
fn read_manifest(package: &str, dir: &Path) -> Result<Option<Plugin>> {
    let manifest = dir.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("{} could not be read", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", manifest.display()))?;
    let Some(declared) = parsed.get("angularNative") else {
        return Ok(None);
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative has to be an object",
            manifest.display()
        )
    })?;

    let module = declared
        .get("module")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: angularNative.module is missing", manifest.display()))?;
    check_module_name(module, &manifest)?;

    let entry = match declared.get("entry").and_then(Value::as_str) {
        Some(relative) => {
            let path = dir.join(relative);
            if !path.is_file() {
                bail!(
                    "{}: angularNative.entry points at {}, which does not exist",
                    manifest.display(),
                    path.display()
                );
            }
            Some(path)
        }
        None => None,
    };

    let mut plugin = Plugin {
        package: package.to_owned(),
        dir: dir.to_owned(),
        module: module.to_owned(),
        entry,
        ios: None,
        android: None,
        macos: None,
        watchos: None,
        unsupported: BTreeMap::new(),
    };
    for platform in ALL_PLATFORMS {
        match read_native(declared.get(platform.key()), dir, platform, &manifest)? {
            Half::Covered(native) => match platform {
                Platform::Ios => plugin.ios = Some(native),
                Platform::Android => plugin.android = Some(native),
                Platform::Macos => plugin.macos = Some(native),
                Platform::Watchos => plugin.watchos = Some(native),
            },
            Half::Unsupported(reason) => {
                plugin.unsupported.insert(platform.key(), reason);
            }
            Half::Absent => {}
        }
    }
    Ok(Some(plugin))
}

/// What a platform's section in the manifest turned out to be.
///
/// The middle one is the whole point of this enum. A plugin that says nothing
/// about watchOS and a plugin that says "watchOS has no pasteboard" are not the
/// same plugin, even though neither of them can be built for a watch: the first
/// one is unfinished and the second one is finished. Folding them together would
/// throw away the only sentence anybody reading the refusal actually wants.
enum Half {
    Covered(Native),
    Unsupported(String),
    Absent,
}

/// The module name travels untouched all the way to a Swift literal and a Java
/// one. It is checked here so that an odd name gives a readable error rather
/// than a compilation failure inside a generated file.
fn check_module_name(module: &str, manifest: &Path) -> Result<()> {
    let valid = module
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && module
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        bail!(
            "{}: angularNative.module is {module:?}; it has to start with a lower-case letter \
             and carry nothing but letters, digits and hyphens",
            manifest.display()
        );
    }
    Ok(())
}

fn read_native(
    declared: Option<&Value>,
    dir: &Path,
    platform: Platform,
    manifest: &Path,
) -> Result<Half> {
    let Some(declared) = declared else {
        return Ok(Half::Absent);
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative.{} has to be an object",
            manifest.display(),
            platform.key()
        )
    })?;
    // `unsupported` is a plugin owning up in its own words, and it is exclusive
    // with `sources`: a half that exists needs no excuse, and one that does not
    // exist has nothing to compile. Accepting both would leave it to whoever
    // reads the JSON to guess which of the two the plugin meant.
    if let Some(reason) = declared.get("unsupported") {
        let reason = reason.as_str().with_context(|| {
            format!(
                "{}: angularNative.{}.unsupported has to be the sentence that explains why, \
                 as a string",
                manifest.display(),
                platform.key()
            )
        })?;
        if reason.trim().is_empty() {
            bail!(
                "{}: angularNative.{}.unsupported is empty. The whole point of it is the \
                 reason; an empty one says less than leaving the section out.",
                manifest.display(),
                platform.key()
            );
        }
        if declared.contains_key("sources") {
            bail!(
                "{}: angularNative.{} declares both sources and unsupported. It is one or the \
                 other: either the half exists or it does not.",
                manifest.display(),
                platform.key()
            );
        }
        return Ok(Half::Unsupported(reason.to_owned()));
    }
    let sources = declared
        .get("sources")
        .and_then(Value::as_str)
        .with_context(|| {
            format!(
                "{}: angularNative.{}.sources is missing",
                manifest.display(),
                platform.key()
            )
        })?;
    let register = declared
        .get("register")
        .and_then(Value::as_str)
        .with_context(|| {
            format!(
                "{}: angularNative.{}.register is missing",
                manifest.display(),
                platform.key()
            )
        })?;
    let sources = dir.join(sources);
    if !sources.is_dir() {
        bail!(
            "{}: angularNative.{}.sources points at {}, which is not a directory",
            manifest.display(),
            platform.key(),
            sources.display()
        );
    }
    let contributes = match platform {
        Platform::Ios | Platform::Macos | Platform::Watchos => Contributions::Apple {
            plist: read_dict(declared.get("plist"), platform, "plist", manifest)?,
            entitlements: read_dict(
                declared.get("entitlements"),
                platform,
                "entitlements",
                manifest,
            )?,
        },
        Platform::Android => {
            Contributions::Manifest(read_manifest_entries(declared.get("manifest"), manifest)?)
        }
    };
    Ok(Half::Covered(Native {
        sources,
        register: register.to_owned(),
        contributes,
    }))
}

/// `angularNative.ios.plist`: the keys this plugin needs in the `Info.plist`.
///
/// Without this, a Face ID plugin compiles, installs and kills the app the first
/// time it authenticates: iOS demands `NSFaceIDUsageDescription` and without it
/// there is no warning, it just closes. The plugin declaring here what it needs
/// is what saves whoever installs it from having to know.
fn read_dict(
    declared: Option<&Value>,
    platform: Platform,
    section: &str,
    manifest: &Path,
) -> Result<BTreeMap<String, Value>> {
    let key_path = format!("angularNative.{}.{section}", platform.key());
    let Some(declared) = declared else {
        return Ok(BTreeMap::new());
    };
    let declared = declared.as_object().with_context(|| {
        format!("{}: {key_path} has to be an object", manifest.display())
    })?;
    let mut entries = BTreeMap::new();
    for (key, value) in declared {
        // The key travels on to `plutil -replace`, which treats the dot as a
        // path separator: `a.b` would not be a key called "a.b" but the "b"
        // inside the "a". No system key has a dot in it, so it stops here rather
        // than writing somewhere nobody asked for.
        //
        // The macOS entitlements are the exception that had to be carved out:
        // every one of them is `com.apple.security.something`, dots and all, and
        // they are what a sandboxed Mac app lives or dies by. They are not
        // written with `plutil -replace` —they go into a dictionary that is
        // serialised whole— so the path-separator problem does not reach them.
        let dots_allowed = section == "entitlements";
        if key.is_empty() || (key.contains('.') && !dots_allowed) {
            bail!(
                "{}: {key_path} has the key {key:?}; only top-level keys with \
                 no dots in them are accepted",
                manifest.display()
            );
        }
        if !plist_value_ok(value) {
            bail!(
                "{}: {key_path}[{key:?}] is {value}; only strings, booleans, \
                 numbers and lists of strings are accepted. A nested dictionary is not merged \
                 yet. See https://angular-native.dev/extending/plugins/.",
                manifest.display()
            );
        }
        entries.insert(key.clone(), value.clone());
    }
    Ok(entries)
}

/// What can be written into the plist with `plutil -replace … -json`, and
/// nothing more. Accepting a dictionary here and merging it badly later would be
/// worse than not accepting it at all.
fn plist_value_ok(value: &Value) -> bool {
    match value {
        Value::String(_) | Value::Bool(_) | Value::Number(_) => true,
        Value::Array(items) => items.iter().all(Value::is_string),
        _ => false,
    }
}

/// `angularNative.android.manifest`: permissions and features.
fn read_manifest_entries(declared: Option<&Value>, manifest: &Path) -> Result<ManifestEntries> {
    let Some(declared) = declared else {
        return Ok(ManifestEntries::default());
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative.android.manifest has to be an object",
            manifest.display()
        )
    })?;
    let mut entries = ManifestEntries::default();
    for (key, value) in declared {
        match key.as_str() {
            "uses-permission" => {
                let list = value.as_array().with_context(|| {
                    format!(
                        "{}: angularNative.android.manifest[\"uses-permission\"] has to be \
                         a list of names",
                        manifest.display()
                    )
                })?;
                for name in list {
                    let name = name.as_str().with_context(|| {
                        format!(
                            "{}: permissions are strings, and there is a {name}",
                            manifest.display()
                        )
                    })?;
                    entries.permissions.insert(name.to_owned());
                }
            }
            "uses-feature" => {
                let object = value.as_object().with_context(|| {
                    format!(
                        "{}: angularNative.android.manifest[\"uses-feature\"] has to be an \
                         object from name to whether it is required",
                        manifest.display()
                    )
                })?;
                for (name, required) in object {
                    let required = required.as_bool().with_context(|| {
                        format!(
                            "{}: uses-feature[{name:?}] has to be true or false, which is what \
                             android:required is worth",
                            manifest.display()
                        )
                    })?;
                    entries.features.insert(name.clone(), required);
                }
            }
            other => bail!(
                "{}: angularNative.android.manifest knows nothing about {other:?}; for now only \
                 \"uses-permission\" and \"uses-feature\" are contributed. See https://angular-native.dev/extending/plugins/.",
                manifest.display()
            ),
        }
    }
    Ok(entries)
}

/// Demands that every plugin cover the platform about to be compiled.
///
/// This is the rule that gives the rest its point. An iOS plugin dropped into an
/// APK must not end up as a method that returns `undefined` and a screen that
/// does nothing: the build stops and says what is missing and in which package.
pub fn require(plugins: &[Plugin], platform: Platform) -> Result<()> {
    let missing: Vec<&Plugin> = plugins
        .iter()
        .filter(|plugin| plugin.native(platform).is_none())
        .collect();
    if missing.is_empty() {
        // What they contribute to the manifest is merged here and not when it is
        // written: that way a clash between two plugins turns up in
        // `an plugins --platform ios`, without burning half a minute of `swiftc`
        // only to say the same thing.
        match platform {
            Platform::Ios | Platform::Macos | Platform::Watchos => {
                plist_entries(plugins, platform)?;
                entitlement_entries(plugins, platform)?;
            }
            Platform::Android => {
                manifest_entries(plugins)?;
            }
        }
        return Ok(());
    }
    let mut message = format!(
        "this app cannot be built for {}: {} of its plugins do{} not cover it\n",
        platform.label(),
        missing.len(),
        if missing.len() == 1 { "es" } else { "" }
    );
    // Two kinds of missing, and they get two different lines. The plugin that
    // explained itself gets its own sentence quoted; the one that said nothing
    // gets the list of what it does bring, which is all anybody can say about it.
    for plugin in &missing {
        match plugin.unsupported(platform) {
            Some(reason) => message.push_str(&format!(
                "  · {} (module {:?}) says it cannot: {reason}\n",
                plugin.package, plugin.module
            )),
            None => message.push_str(&format!(
                "  · {} (module {:?}) only brings {}\n",
                plugin.package,
                plugin.module,
                plugin.coverage()
            )),
        }
    }
    // If every one of them explained itself, there is nothing to add: the app
    // depends on plugins that will never work there, and telling whoever reads
    // this to go and write the missing half would be telling them to do
    // something impossible.
    if missing.iter().all(|plugin| plugin.unsupported(platform).is_some()) {
        message.push_str(&format!(
            "\nThese are not unfinished halves: they cannot exist on {}. The app has to stop \
             depending on them for this build, or not be built for {}.",
            platform.label(),
            platform.label()
        ));
    } else {
        message.push_str(&format!(
            "\nEither the plugin adds its {} half —sources in angularNative.{} of its \
             package.json—, or it says why it cannot in angularNative.{}.unsupported, or the \
             app stops depending on it.",
            platform.label(),
            platform.key(),
            platform.key()
        ));
    }
    bail!(message)
}

/// Merges the `Info.plist` keys all the plugins ask for, for one Apple platform.
pub fn plist_entries(
    plugins: &[Plugin],
    platform: Platform,
) -> Result<BTreeMap<String, Contributed>> {
    merge_dicts(plugins, platform, "the Info.plist", |plist, _| plist)
}

/// Merges the entitlements all the plugins ask for.
///
/// Entitlements are what lets the app ask the system for something: without
/// `keychain-access-groups`, Keychain Services answers
/// `errSecMissingEntitlement` and saves nothing. Nobody sees that error until
/// the app runs, and by then it looks like a keychain bug and not a signature
/// that was missing.
pub fn entitlement_entries(
    plugins: &[Plugin],
    platform: Platform,
) -> Result<BTreeMap<String, Contributed>> {
    merge_dicts(plugins, platform, "the entitlements", |_, entitlements| entitlements)
}

/// The merge both of them share.
///
/// Two plugins asking for the same key with the **same** value are no problem:
/// they are saying the same thing, and it gets written once. With different
/// values there is no honest way to choose —keeping the first one by alphabetical
/// order or by dependency order would be silently deciding what text the user
/// gets in the system's dialog— so the build stops.
fn merge_dicts<'a>(
    plugins: &'a [Plugin],
    platform: Platform,
    what: &str,
    pick: fn(
        &'a BTreeMap<String, Value>,
        &'a BTreeMap<String, Value>,
    ) -> &'a BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Contributed>> {
    let mut merged: BTreeMap<String, Contributed> = BTreeMap::new();
    for plugin in plugins {
        let Some(native) = plugin.native(platform) else { continue };
        let Contributions::Apple { plist, entitlements } = &native.contributes else { continue };
        for (key, value) in pick(plist, entitlements) {
            if let Some(previous) = merged.get(key) {
                if &previous.value != value {
                    bail!(clash(
                        &format!("the key {key:?} of {what}"),
                        &previous.package,
                        &previous.value.to_string(),
                        &plugin.package,
                        &value.to_string(),
                    ));
                }
                continue;
            }
            merged.insert(
                key.clone(),
                Contributed { value: value.clone(), package: plugin.package.clone() },
            );
        }
    }
    Ok(merged)
}

/// The same thing for the `AndroidManifest.xml`.
///
/// Permissions are a set and cannot clash: `USE_BIOMETRIC` asked for twice is
/// `USE_BIOMETRIC`. Features can, because they carry a value: required and
/// optional are not the same request, and the difference decides whether Google
/// Play shows the app on a device without that sensor.
pub fn manifest_entries(plugins: &[Plugin]) -> Result<ManifestEntries> {
    let mut merged = ManifestEntries::default();
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for plugin in plugins {
        let Some(native) = plugin.android.as_ref() else {
            continue;
        };
        let Contributions::Manifest(entries) = &native.contributes else {
            continue;
        };
        for permission in &entries.permissions {
            merged.permissions.insert(permission.clone());
        }
        for (name, required) in &entries.features {
            if let Some(previous) = merged.features.get(name) {
                if previous != required {
                    let owner = owners
                        .get(name)
                        .map(String::as_str)
                        .unwrap_or("another plugin");
                    bail!(clash(
                        &format!("the feature {name:?} of the AndroidManifest.xml"),
                        owner,
                        &format!("android:required=\"{previous}\""),
                        &plugin.package,
                        &format!("android:required=\"{required}\""),
                    ));
                }
                continue;
            }
            merged.features.insert(name.clone(), *required);
            owners.insert(name.clone(), plugin.package.clone());
        }
    }
    Ok(merged)
}

/// The message for two plugins asking for the same thing with different values.
///
/// It names both packages and both values because that is the only thing anyone
/// can fix it with: whoever reads it wrote neither of the two.
fn clash(what: &str, one: &str, one_value: &str, other: &str, other_value: &str) -> String {
    format!(
        "two plugins ask for {what} with different values:\n\
         \x20 · {one}\n\
         \x20     {one_value}\n\
         \x20 · {other}\n\
         \x20     {other_value}\n\n\
         Only one can stay, and picking by order would silently decide something \
         that shows up on screen.\n\
         Either the two plugins agree, or the app keeps one of the two."
    )
}

/// A plugin's native sources for one platform, in a stable order.
pub fn sources(plugin: &Plugin, platform: Platform) -> Result<Vec<String>> {
    let native = plugin
        .native(platform)
        .with_context(|| format!("{} does not cover {}", plugin.package, platform.label()))?;
    let mut found: Vec<PathBuf> = Vec::new();
    let mut kotlin: Vec<PathBuf> = Vec::new();
    for path in walk(&native.sources) {
        match path.extension().and_then(|e| e.to_str()) {
            Some(extension) if extension == platform.extension() => found.push(path),
            Some("kt") => kotlin.push(path),
            _ => {}
        }
    }
    if !kotlin.is_empty() {
        // There is no Gradle here, and without Gradle there is no `kotlinc`
        // worth having: the Android shell is Java and it is compiled with `javac`
        // against `android.jar`. Saying so beats building the APK without those
        // files in it.
        bail!(
            "{}: {} carries Kotlin sources and those are not compiled yet; the Android half \
             of a plugin is Java. See https://angular-native.dev/extending/plugins/.",
            plugin.package,
            native.sources.display()
        );
    }
    if found.is_empty() {
        bail!(
            "{}: there is no .{} source in {}",
            plugin.package,
            platform.extension(),
            native.sources.display()
        );
    }
    found.sort();
    Ok(found
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

/// The registry that wires the plugins to a Swift shell.
///
/// One function for the three Apple platforms, because the file it writes is the
/// same file: `AnPluginRegistry.register(name, Type())`, once per plugin. What
/// differs between iOS, macOS and watchOS is the registry on the other side —its
/// `AnPlugin` protocol hangs off `UIViewController`, `NSViewController` or
/// nothing at all— and none of that shows up here.
///
/// It is always generated, even when there are none: the shell calls it
/// unconditionally, and an empty `install()` reads better than an `#if`.
pub fn generate_swift(plugins: &[Plugin], platform: Platform, out: &Path) -> Result<PathBuf> {
    let mut code = String::from(
        "// Generated by `an` when it puts the .app together. Do not edit: it is\n\
         // rewritten on every build.\n\
         //\n\
         // The names come from the angularNative.module of each plugin\'s\n\
         // package.json, which is the one place they are written down.\n\n\
         enum AnGeneratedPlugins {\n    static func install() {\n",
    );
    if plugins.is_empty() {
        code.push_str("        // This app depends on no plugin.\n");
    }
    for plugin in plugins {
        let native = plugin
            .native(platform)
            .expect("require() already checked it");
        code.push_str(&format!(
            "        AnPluginRegistry.register({:?}, {}())\n",
            plugin.module, native.register
        ));
    }
    code.push_str("    }\n}\n");

    std::fs::create_dir_all(out)?;
    let path = out.join("AnGeneratedPlugins.swift");
    std::fs::write(&path, code)?;
    Ok(path)
}

/// The same registry, for the Android shell.
pub fn generate_android(plugins: &[Plugin], out: &Path) -> Result<PathBuf> {
    let mut code = String::from(
        "// Generated by `an` when it puts the APK together. Do not edit: it is\n\
         // rewritten on every build.\n\
         //\n\
         // The names come from the angularNative.module of each plugin\'s\n\
         // package.json, which is the one place they are written down.\n\n\
         package dev.angularnative;\n\n\
         public final class AnGeneratedPlugins {\n\n\
         \x20   private AnGeneratedPlugins() {}\n\n\
         \x20   public static void install() {\n",
    );
    if plugins.is_empty() {
        code.push_str("        // This app depends on no plugin.\n");
    }
    for plugin in plugins {
        let native = plugin
            .native(Platform::Android)
            .expect("require() already checked it");
        code.push_str(&format!(
            "        AnPluginRegistry.register({:?}, new {}());\n",
            plugin.module, native.register
        ));
    }
    code.push_str("    }\n}\n");

    let package_dir = out.join("dev/angularnative");
    std::fs::create_dir_all(&package_dir)?;
    let path = package_dir.join("AnGeneratedPlugins.java");
    std::fs::write(&path, code)?;
    Ok(path)
}

/// esbuild's `--alias`es, so that the package's import points at the JS that has
/// just come out of `ngc`.
///
/// Only for plugins that bring their API in TypeScript inside the repo: one
/// published on npm arrives compiled, and then esbuild resolves it through
/// `node_modules` like any other dependency and there is nothing to do here.
pub fn aliases(workspace: &Workspace, js_dir: &Path, plugins: &[Plugin]) -> Vec<String> {
    // The plugins' directories come already resolved; the root may not be, and
    // then the prefix would not match.
    let root = workspace.source_root();
    plugins
        .iter()
        .filter_map(|plugin| {
            let entry = plugin.entry.as_ref()?;
            if entry.extension().and_then(|e| e.to_str()) != Some("ts") {
                return None;
            }
            // `ngc` keeps the directory structure under the tsconfig's
            // `rootDir`, which in this repo's apps is the root.
            let relative = entry.strip_prefix(&root).ok()?;
            let compiled = js_dir.join(relative).with_extension("js");
            Some(format!("--alias={}={}", plugin.package, compiled.display()))
        })
        .collect()
}

/// Lists an app's plugins on standard output.
///
/// The path is not decoration: an npm package can come from the repo or from
/// `node_modules`, and when something does not add up the first thing anyone
/// wants to know is which of the two got linked.
pub fn list(workspace: &Workspace, plugins: &[Plugin]) {
    if plugins.is_empty() {
        println!("this app depends on no plugin");
        return;
    }
    let root = workspace.source_root();
    for plugin in plugins {
        let dir = plugin.dir.strip_prefix(&root).unwrap_or(&plugin.dir);
        println!(
            "{}  ({})  {}  {}",
            plugin.module,
            plugin.package,
            plugin.coverage(),
            dir.display()
        );
        // A platform the plugin has ruled out on purpose is worth a line of its
        // own here, where somebody is looking at the list precisely because they
        // are wondering what will happen on that platform. Finding it out from a
        // failed build half a minute later is finding it out too late.
        for platform in ALL_PLATFORMS {
            if let Some(reason) = plugin.unsupported(platform) {
                println!("    no {}: {reason}", platform.key());
            }
        }
    }
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
