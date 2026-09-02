//! Where the SDK is, where the project is, and where each thing goes.
//!
//! `an` is useful in two places: inside this monorepo, where the app is
//! `examples/<something>` and everything hangs off the same root, and inside any
//! Angular project —one from `ng new`— that has nothing but its `package.json`
//! and its `src/`. Both cases need the same two roots, and they are different
//! ones:
//!
//!   · the **SDK root** is where the crates, the native shells and
//!     `scripts/bundle.mjs` come from. It is this repo, always.
//!   · the **project root** is where the app's code lives and where the
//!     artefacts are written. In the monorepo it is the same as the SDK's;
//!     outside, it is not.
//!
//! Telling them apart is this module's entire job. The rest of the CLI asks for
//! `workspace.root` when it wants the SDK and `workspace.build_dir()` when it
//! wants to write, and never finds out which of the two worlds it is in.

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
            .with_context(|| format!("no se pudo leer {}", path.display()))?;
        let parsed: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} no es JSON válido", path.display()))?;
        let app = parsed.get("app").and_then(Value::as_object);
        let read = |key: &str| -> Option<String> {
            app.and_then(|app| app.get(key)).and_then(Value::as_str).map(str::to_owned)
        };

        let name = read("name").with_context(|| {
            format!("{}: falta app.name; vuelve a ejecutar `an init`", path.display())
        })?;
        let bundle_id = read("bundleId").with_context(|| {
            format!("{}: falta app.bundleId; vuelve a ejecutar `an init`", path.display())
        })?;
        let entry = read("entry").unwrap_or_else(|| "src/main.native.ts".to_owned());
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
        let cwd = std::env::current_dir().context("no se pudo leer el directorio actual")?;
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
        // often, and saying «I cannot find the root» when the answer is «run
        // `an init`» sends people off to read the CLI's source.
        if let Some(angular) = uninitialised_angular(&cwd) {
            bail!(
                "{} es un proyecto Angular, pero no está inicializado para angular-native.\n\
                 Ejecuta `an init` ahí dentro.",
                angular.display()
            );
        }
        bail!(
            "aquí no hay ni un proyecto de angular-native ni el monorepo.\n\
             Desde un proyecto Angular, ejecuta primero `an init`."
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
                        "fuera del monorepo `an` trabaja sobre el proyecto en el que se ejecuta \
                         ({}), y se le ha pedido {}.\n\
                         Ejecuta `an` desde ese otro proyecto.",
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
            bail!("{} no parece una app: falta tsconfig.json", absolute.display());
        }
        let relative = absolute
            .strip_prefix(&self.root)
            .context("la app tiene que estar dentro del proyecto")?;
        Ok(relative.to_path_buf())
    }

    /// The project from outside, or an error saying what is missing. For the
    /// commands that only make sense in there.
    pub fn project(&self) -> Result<&Project> {
        self.project.as_ref().context(
            "este comando es para un proyecto Angular de fuera del monorepo, \
             y `an` se está ejecutando dentro del monorepo",
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

    /// Where `cargo` leaves what it compiles.
    ///
    /// Almost always `target/` at the SDK's root, but `CARGO_TARGET_DIR` exists
    /// and whoever has it set does not expect the CLI to go looking for the
    /// static library where it no longer is.
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

/// The SDK's root when `an` runs outside the monorepo.
///
/// Two places, in this order, and neither of them guessed at:
///
///   1. `AN_HOME`, for whoever has several checkouts or installed the binary by
///      hand.
///   2. The path this binary was compiled from. `cargo install --path
///      crates/an-cli` bakes it in, so an `an` on the PATH knows how to find its
///      way back to its repo.
///
/// If the place exists but has not got what is needed, it says what is missing.
/// A half-finished SDK would produce a `swiftc` with no sources or a bundle with
/// no runtime, and that turns up twenty seconds later and in another language.
pub fn sdk_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("AN_HOME") {
        let dir = absolute(&dir.to_string_lossy());
        return validate_sdk(&dir).with_context(|| {
            format!("AN_HOME apunta a {}, que no es un SDK de angular-native", dir.display())
        });
    }
    let compiled_from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compiled_from = compiled_from.canonicalize().unwrap_or(compiled_from);
    validate_sdk(&compiled_from).with_context(|| {
        format!(
            "este `an` se compiló desde {}, y ahí ya no está el SDK.\n\
             Define AN_HOME con la ruta del repositorio de angular-native.",
            compiled_from.display()
        )
    })
}

fn validate_sdk(dir: &Path) -> Result<PathBuf> {
    for needed in ["packages/runtime/runtime.js", "scripts/bundle.mjs", "shells", "crates"] {
        if !dir.join(needed).exists() {
            bail!("falta {needed}");
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
