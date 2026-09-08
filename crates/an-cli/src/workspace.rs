//! Where the SDK is, where the project is, and where each thing goes.
//!
//! `an` is useful in two places: inside this monorepo, where the app is
//! `examples/<something>` and everything hangs off the same root, and inside any
//! Angular project —one from `ng new`— that has nothing but its `package.json`
//! and its `src/`. Both cases need the same two roots, and they are different
//! ones:
//!
//!   · the **SDK root** is where the crates, the native shells and
//!     `scripts/bundle.mjs` come from.
//!   · the **project root** is where the app's code lives and where the
//!     artefacts are written. In the monorepo it is the same as the SDK's;
//!     outside, it is not.
//!
//! Telling them apart is this module's entire job. The rest of the CLI asks for
//! `workspace.root` when it wants the SDK and `workspace.build_dir()` when it
//! wants to write, and never finds out which of the two worlds it is in.
//!
//! The SDK root used to be "this repo, always", and it is not any more. It is
//! one of three things, and [`sdk_root`] is where the three are told apart:
//!
//!   1. a git checkout of this repository, found through `AN_HOME` or through
//!      the path the binary was compiled from;
//!   2. the payload of `@angular-native/cli` under a `node_modules`, which is
//!      what `npm install -g @angular-native/cli` leaves behind;
//!   3. the monorepo the current directory is already inside, which is the
//!      case [`Workspace::discover`] settles without asking anybody.
//!
//! The npm payload is laid out **exactly** like the checkout —`crates/`,
//! `shells/`, `packages/`, `scripts/bundle.mjs`— on purpose: `validate_sdk`
//! asks for the same four things whichever it is, and no other module has to
//! learn that there is now a third world. The one place it does show is
//! [`Workspace::sdk_is_npm`]: a global npm install sits somewhere the user
//! cannot write, so `cargo` cannot leave `target/` beside the crates it is
//! compiling. `main` sends it to the project instead.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// The file that marks a project from outside as already initialised. Its
/// existence is the only sign that `an init` finished: it is written last.
pub const MARKER: &str = "angular-native.json";

/// The app's default name inside the monorepo. It is the one the shell's
/// `Info.plist` and the Android manifest carry, so inside the monorepo it is not
/// to be touched.
pub const DEFAULT_APP_NAME: &str = "AngularNative";
pub const DEFAULT_BUNDLE_ID: &str = "dev.angularnative.playground";

/// The configuration of a project from outside: whatever its
/// `angular-native.json` says.
///
/// It does not keep the SDK's path, on purpose. It would be absolute, and
/// absolute on the disk of whoever ran `an init`; the file gets committed and
/// the next person to clone the repo would have the SDK somewhere else. The
/// SDK's path is resolved by the binary (see [`sdk_root`]), the only thing that
/// knows which machine it is on.
pub struct Project {
    pub root: PathBuf,
    /// The app's name: the executable's inside the `.app` and the one that
    /// shows under the icon.
    pub name: String,
    pub bundle_id: String,
    /// The native entry point, relative to the project's root.
    pub entry: PathBuf,
    /// The platforms added with `an add`.
    pub platforms: Vec<String>,
    /// Light, dark, or whatever the device is set to.
    pub appearance: Appearance,
}

/// What the app looks like, and who decides.
///
/// It is the app's decision and not the shell's, which is the whole reason
/// this exists: the Android shell used to force dark on every app built with
/// it, process-wide, because an example painting a dark background under a
/// system in light mode came out with a white navigation bar. That fixed the
/// symptom by taking the choice away from everybody.
///
/// It is declared once, in `angular-native.json`, and each platform is told in
/// its own words — `UIUserInterfaceStyle` on the Apple ones, `setDefaultNightMode`
/// on Android. A key only one platform honoured would be worse than no key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Appearance {
    /// Follow the device. Light phone, light app.
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// The word as it is written in the manifest, and as it travels to a
    /// platform that wants a string.
    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    /// `None` for a word that is none of the three. The caller says so rather
    /// than quietly picking one: a typo that silently means `system` is a
    /// setting that does nothing for a reason nobody can see.
    pub fn parse(word: &str) -> Option<Appearance> {
        match word {
            "system" => Some(Appearance::System),
            "light" => Some(Appearance::Light),
            "dark" => Some(Appearance::Dark),
            _ => None,
        }
    }
}

impl Project {
    /// The tsconfig `ngc` uses for the native build. `an init` writes it; it is
    /// not the web project's, which compiles HTML templates against the DOM.
    pub fn tsconfig(&self) -> PathBuf {
        self.root.join(".angular-native/tsconfig.json")
    }

    pub fn read(root: &Path) -> Result<Self> {
        let path = root.join(MARKER);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("{} could not be read", path.display()))?;
        let parsed: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
        // Before anything is read out of it: a password written into this file
        // stops every command, not only the ones that sign. See
        // `signing::check_manifest_secrets`.
        crate::signing::check_manifest_secrets(&path, &parsed)?;
        let app = parsed.get("app").and_then(Value::as_object);
        let read = |key: &str| -> Option<String> {
            app.and_then(|app| app.get(key)).and_then(Value::as_str).map(str::to_owned)
        };

        let name = read("name").with_context(|| {
            format!("{}: app.name is missing; run `an init` again", path.display())
        })?;
        let bundle_id = read("bundleId").with_context(|| {
            format!("{}: app.bundleId is missing; run `an init` again", path.display())
        })?;
        let entry = read("entry").unwrap_or_else(|| "src/main.native.ts".to_owned());
        let appearance = match read("appearance") {
            Some(word) => Appearance::parse(&word).with_context(|| {
                format!(
                    "{}: app.appearance is {word:?}, and it has to be \"system\", \"light\" or \
                     \"dark\". \"system\" follows the device, which is the default.",
                    path.display()
                )
            })?,
            None => Appearance::default(),
        };
        let platforms = parsed
            .get("platforms")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Project {
            root: root.to_owned(),
            name,
            bundle_id,
            entry: PathBuf::from(entry),
            platforms,
            appearance,
        })
    }
}

pub struct Workspace {
    /// The SDK's root: crates, shells, packages and `scripts/bundle.mjs`.
    pub root: PathBuf,
    /// The project from outside, if `an` is running inside one. `None` means we
    /// are in the monorepo.
    pub project: Option<Project>,
}

impl Workspace {
    /// Climbs up from the current directory until it works out which world this
    /// is.
    ///
    /// The nearest one wins: if somebody initialises a project inside the
    /// monorepo itself —which is exactly what `scripts/check-external.sh` does—
    /// the marker sits lower than the `Cargo.toml` and it is the one in
    /// charge.
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir().context("the current directory could not be read")?;
        let mut dir = cwd.clone();
        loop {
            if dir.join(MARKER).is_file() {
                let project = Project::read(&dir)?;
                return Ok(Workspace { root: sdk_root()?, project: Some(project) });
            }
            if is_monorepo(&dir) {
                return Ok(Workspace { root: dir, project: None });
            }
            if !dir.pop() {
                break;
            }
        }
        // Nothing. Before giving up it is worth looking at whether this is an
        // uninitialised Angular project: it is the error that will come up most
        // often, and saying "I cannot find the root" when the answer is "run
        // `an init`" sends people off to read the CLI's source.
        if let Some(angular) = uninitialised_angular(&cwd) {
            bail!(
                "{} is an Angular project, but it is not initialised for angular-native.\n\
                 Run `an init` in there.",
                angular.display()
            );
        }
        bail!(
            "there is neither an angular-native project nor the monorepo here.\n\
             From an Angular project, run `an init` first."
        )
    }

    /// The app's path.
    ///
    /// Inside the monorepo it is relative to the root, as always. Outside it is
    /// the project's, absolute, and there is nothing to choose: `an` works on
    /// the project it is run in.
    pub fn app(&self, given: Option<&str>) -> Result<PathBuf> {
        if let Some(project) = &self.project {
            if let Some(given) = given {
                let asked_for = absolute(given);
                if asked_for != project.root {
                    bail!(
                        "outside the monorepo `an` works on the project it is run in \
                         ({}), and it has been asked for {}.\n\
                         Run `an` from that other project.",
                        project.root.display(),
                        asked_for.display()
                    );
                }
            }
            return Ok(project.root.clone());
        }

        let raw = given.unwrap_or("examples/hello-angular");
        let absolute = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            self.root.join(raw)
        };
        if !absolute.join("tsconfig.json").is_file() {
            bail!("{} does not look like an app: tsconfig.json is missing", absolute.display());
        }
        let relative = absolute
            .strip_prefix(&self.root)
            .context("the app has to be inside the project")?;
        Ok(relative.to_path_buf())
    }

    /// The project from outside, or an error saying what is missing. For the
    /// commands that only make sense in there.
    pub fn project(&self) -> Result<&Project> {
        self.project.as_ref().context(
            "this command is for an Angular project from outside the monorepo, \
             and `an` is being run inside the monorepo",
        )
    }

    /// Where the artefacts are written: the `.app`, the APK, the compiled JS.
    ///
    /// In the monorepo, `build/` at the root, where they have always been and
    /// where the scripts look for them. In a project from outside, inside
    /// `.angular-native/`, which goes into the `.gitignore`: nothing in there is
    /// a source.
    pub fn build_dir(&self) -> PathBuf {
        match &self.project {
            Some(project) => project.root.join(".angular-native/build"),
            None => self.root.join("build"),
        }
    }

    /// Whether the SDK came from npm rather than from a checkout.
    ///
    /// The one thing the rest of the CLI has to know about the third world, and
    /// it is asked exactly once, in `main`: a global npm install lives where the
    /// user cannot write, so `cargo` is sent elsewhere. See `target_dir`.
    pub fn sdk_is_npm(&self) -> bool {
        is_npm_sdk(&self.root)
    }

    /// Where `cargo` leaves what it compiles.
    ///
    /// Almost always `target/` at the SDK's root, but `CARGO_TARGET_DIR` exists
    /// and whoever has it set does not expect the CLI to go looking for the
    /// static library where it no longer is.
    ///
    /// That variable is also how the npm install is dealt with: `main` sets it
    /// when the SDK is under `node_modules`, so what is read here and what the
    /// child `cargo` writes cannot come apart.
    pub fn target_dir(&self) -> PathBuf {
        match std::env::var_os("CARGO_TARGET_DIR") {
            Some(dir) => absolute(&dir.to_string_lossy()),
            None => self.root.join("target"),
        }
    }

    /// Where `ngc` leaves the JavaScript. It is the `outDir` of the tsconfig
    /// that compiles the app, and esbuild's entry point comes from there.
    pub fn js_dir(&self, app: &Path) -> PathBuf {
        match &self.project {
            Some(_) => self.build_dir().join("js"),
            None => self.root.join("build/js").join(Workspace::name(app)),
        }
    }

    /// The tsconfig's `rootDir`. `ngc` keeps the directory structure relative to
    /// it under the `outDir`, so it is what has to be stripped off a source path
    /// to know where its `.js` ended up.
    pub fn source_root(&self) -> PathBuf {
        let dir = match &self.project {
            Some(project) => project.root.clone(),
            None => self.root.clone(),
        };
        dir.canonicalize().unwrap_or(dir)
    }

    pub fn app_name(&self) -> String {
        match &self.project {
            Some(project) => project.name.clone(),
            None => DEFAULT_APP_NAME.to_owned(),
        }
    }

    /// What the app looks like. In the monorepo there is no manifest to read,
    /// so it is the default: follow the machine.
    pub fn appearance(&self) -> Appearance {
        match &self.project {
            Some(project) => project.appearance,
            None => Appearance::default(),
        }
    }

    pub fn bundle_id(&self) -> String {
        match &self.project {
            Some(project) => project.bundle_id.clone(),
            None => DEFAULT_BUNDLE_ID.to_owned(),
        }
    }

    /// A file the project can supply to override the shell's —the `Info.plist`,
    /// the `AndroidManifest.xml`—, if it exists.
    ///
    /// These are the only native files the user edits by hand, which is why they
    /// live in `ios/` and `android/` and not in `.angular-native/build`: they
    /// get committed, and `an` never rewrites them once they exist.
    pub fn overlay(&self, platform: &str, file: &str) -> Option<PathBuf> {
        let project = self.project.as_ref()?;
        let path = project.root.join(platform).join(file);
        path.is_file().then_some(path)
    }

    pub fn name(app: &Path) -> String {
        app.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "app".to_owned())
    }
}

fn is_monorepo(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file() && dir.join("packages/runtime/runtime.js").is_file()
}

/// The name of the npm package whose payload is an SDK. It is the name the
/// package's own `package.json` carries, and reading it is the only way to tell
/// the payload apart from any other directory that happens to have a
/// `packages/` in it.
pub const NPM_PACKAGE: &str = "@angular-native/cli";

/// The SDK's root when `an` runs outside the monorepo.
///
/// Three places, in this order, and none of them guessed at:
///
///   1. `AN_HOME`, for whoever has several checkouts, installed the binary by
///      hand, or came in through the npm shim. `bin/an.mjs` sets it before
///      spawning this binary because Node's resolver is the only thing that
///      knows where the package manager put the payload: under pnpm the
///      executable's package and the payload's are not siblings, they are two
///      unrelated directories inside `node_modules/.pnpm`.
///   2. The path this binary was compiled from. `cargo install --path
///      crates/an-cli` bakes it in, so an `an` on the PATH knows how to find its
///      way back to its repo. An npm-installed binary was compiled in CI and
///      that path is somebody else's runner, which is why it has to be allowed
///      to fail rather than end the search.
///   3. Beside the executable. This is what makes the binary work when it is
///      run directly instead of through the shim — every layout but pnpm's puts
///      `@angular-native/cli` where a climb from the executable finds it.
///
/// If the place exists but has not got what is needed, it says what is missing.
/// A half-finished SDK would produce a `swiftc` with no sources or a bundle with
/// no runtime, and that turns up twenty seconds later and in another language.
pub fn sdk_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("AN_HOME") {
        let dir = absolute(&dir.to_string_lossy());
        return validate_sdk(&dir).with_context(|| {
            format!("AN_HOME points at {}, which is not an angular-native SDK", dir.display())
        });
    }
    let compiled_from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compiled_from = compiled_from.canonicalize().unwrap_or(compiled_from);
    if let Ok(root) = validate_sdk(&compiled_from) {
        return Ok(root);
    }
    if let Some(root) = beside_the_executable() {
        return Ok(root);
    }
    // The path it was compiled from is named even now: for the `cargo install`
    // case it is the whole answer, and for the npm case it is at least a clue
    // that this binary is not where it thinks it is.
    bail!(
        "this `an` was compiled from {}, the SDK is no longer there, and there is no \
         {NPM_PACKAGE} beside the executable either.\n\
         Set AN_HOME to the path of the angular-native repository, or install the CLI with \
         `npm install -g {NPM_PACKAGE}`.",
        compiled_from.display()
    )
}

/// Climbs from the executable looking for an SDK: the directory it is in, a
/// `@angular-native/cli` beside it, or one under a `node_modules` on the way up.
///
/// The middle case is the npm one. `npm install -g @angular-native/cli` leaves
/// the executable at
/// `…/node_modules/@angular-native/cli-<platform>/bin/an` and the payload at
/// `…/node_modules/@angular-native/cli`, so the climb reaches `@angular-native/`
/// and the sibling is one `join` away.
fn beside_the_executable() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let mut dir = exe.parent()?.to_owned();
    loop {
        let candidates = [
            dir.clone(),
            dir.join("@angular-native").join("cli"),
            dir.join("node_modules").join("@angular-native").join("cli"),
        ];
        if let Some(found) = candidates.into_iter().find(|dir| validate_sdk(dir).is_ok()) {
            return Some(found);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Whether this directory is the npm package's payload rather than a checkout.
///
/// The `package.json` is read rather than the layout inspected: the payload is
/// laid out exactly like the repository, deliberately, so the layout cannot tell
/// them apart and nothing else should try.
pub fn is_npm_sdk(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else { return false };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return false };
    parsed.get("name").and_then(Value::as_str) == Some(NPM_PACKAGE)
}

fn validate_sdk(dir: &Path) -> Result<PathBuf> {
    for needed in ["packages/runtime/runtime.js", "scripts/bundle.mjs", "shells", "crates"] {
        if !dir.join(needed).exists() {
            bail!("{needed} is missing");
        }
    }
    Ok(dir.to_owned())
}

/// Climbs up looking for an Angular project: `angular.json` and `@angular/core`
/// among the dependencies. Both, because an `angular.json` on its own is also
/// what an empty workspace has, and an `@angular/core` on its own is what a
/// library has.
pub fn uninitialised_angular(from: &Path) -> Option<PathBuf> {
    let mut dir = from.to_owned();
    loop {
        if dir.join("angular.json").is_file() && depends_on_angular(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub fn depends_on_angular(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else { return false };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return false };
    ["dependencies", "devDependencies", "peerDependencies"].iter().any(|section| {
        parsed
            .get(section)
            .and_then(Value::as_object)
            .is_some_and(|deps| deps.contains_key("@angular/core"))
    })
}

/// A path the user typed, resolved against the current directory.
pub fn absolute(raw: &str) -> PathBuf {
    let path = Path::new(raw);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    path.canonicalize().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The monorepo's own root has a `package.json` and a `packages/`, and it is
    /// what every check script runs in. Mistaking it for the npm payload would
    /// send `cargo`'s output somewhere `check-all.sh` does not look, so the
    /// marker is the package's **name** and nothing about the layout.
    #[test]
    fn the_monorepo_is_not_taken_for_an_npm_install() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(root.join("package.json").is_file(), "the fixture is the repository itself");
        assert!(!is_npm_sdk(&root));
    }

    /// And a directory laid out like the payload is, down to the name. This is
    /// what `scripts/build-cli.mjs` writes.
    #[test]
    fn the_payload_is_recognised_by_the_name_in_its_manifest() {
        let dir = std::env::temp_dir().join("an-cli-npm-sdk-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the temporary directory is writable");
        std::fs::write(
            dir.join("package.json"),
            format!(r#"{{ "name": "{NPM_PACKAGE}", "version": "0.0.1" }}"#),
        )
        .expect("the manifest is written");
        assert!(is_npm_sdk(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
