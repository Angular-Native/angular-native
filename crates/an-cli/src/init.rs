//! `an init` and `an add`: bringing angular-native to somebody else's Angular
//! project.
//!
//! The idea is `ng add`'s, or `npx cap init`'s: somebody has their `ng new`
//! project and wants to take it to a phone without learning this repository's
//! layout. After `an init` there are four new things in their project and all
//! four can be read in a minute: the `angular-native.json` manifest, a tsconfig
//! for the native build, an entry point and a root component.
//!
//! Two rules govern this module:
//!
//!   · **No file of the user's is overwritten.** What is already there is left
//!     alone, and it says it was left alone. `--force` rewrites only what we
//!     generate, never the app's code.
//!   · **Either it finishes or it never started.** Everything that can fail
//!     —that this is an Angular project, that the SDK is whole, that npm
//!     installs— is checked and done before the manifest is written, and the
//!     manifest goes last. If something goes wrong, `angular-native.json` never
//!     comes into existence, `an` goes on saying the project is not initialised,
//!     and running `an init` again is safe.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::ios::Family;
use crate::workspace::{self, Project, Workspace, MARKER};

/// The framework packages a project from outside needs, with the directory they
/// come from inside the SDK.
///
/// In this order: `platform-native` imports `StackView` from `primitives`, so it
/// needs its `.d.ts` already written in order to compile.
const PACKAGES: [(&str, &str); 2] = [
    ("@angular-native/primitives", "primitives"),
    ("@angular-native/platform", "platform-native"),
];

pub fn init(dir: Option<&str>, name: Option<&str>, id: Option<&str>, force: bool) -> Result<()> {
    let root = match dir {
        Some(dir) => workspace::absolute(dir),
        None => std::env::current_dir().context("the current directory could not be read")?,
    };
    let sdk = workspace::sdk_root()?;

    // ---- Everything that can say no, before anything is touched ----------
    check_angular(&root)?;
    let already = root.join(MARKER).is_file();
    if already && !force {
        eprintln!("==> {} is already initialised; only what is missing is added", root.display());
    }

    let previous = already.then(|| Project::read(&root)).transpose()?;
    let app_name = match (name, &previous) {
        (Some(name), _) => name.to_owned(),
        (None, Some(previous)) => previous.name.clone(),
        (None, None) => app_name_from_package_json(&root)?,
    };
    let bundle_id = match (id, &previous) {
        (Some(id), _) => id.to_owned(),
        (None, Some(previous)) => previous.bundle_id.clone(),
        (None, None) => format!("dev.angularnative.{}", slug(&app_name)),
    };
    check_identifier(&bundle_id)?;
    let entry = previous
        .as_ref()
        .map(|previous| previous.entry.clone())
        .unwrap_or_else(|| PathBuf::from("src/main.native.ts"));
    let platforms_list = previous.map(|previous| previous.platforms).unwrap_or_default();

    eprintln!("==> Angular project at {}", root.display());
    eprintln!("==> angular-native SDK at {}", sdk.display());

    // ---- Dependencies ----------------------------------------------------
    ensure_compiler(&root)?;
    install_packages(&sdk, &root, force)?;

    // ---- Files -----------------------------------------------------------
    write_file(&root.join(".angular-native/tsconfig.json"), &tsconfig(&entry), force)?;
    write_file(&root.join(&entry), ENTRY_POINT, false)?;
    write_file(&root.join("src/app/app-native.ts"), &root_component(&app_name), false)?;
    extend_gitignore(&root)?;

    // And the manifest last: it is what makes the project count as
    // initialised.
    write_project_manifest(&root, &app_name, &bundle_id, &entry, &platforms_list)?;

    eprintln!();
    eprintln!("Done. {app_name} ({bundle_id})");
    eprintln!("  an add ios        creates ios/Info.plist, which is yours from then on");
    eprintln!("  an build          the JS bundle alone");
    eprintln!("  an ios            compiles, puts the .app together and launches it on the simulator");
    eprintln!("  an dev            the same, reloading when you save");
    eprintln!();
    eprintln!(
        "The native app starts at {} and its root component is src/app/app-native.ts.\n\
         Your web app stays as it was: the templates are not shared, because one of them is\n\
         HTML and the other one is native views.",
        entry.display()
    );
    Ok(())
}

/// `an add ios`, `an add tvos`, `an add visionos`, `an add android`.
///
/// It creates the one thing a project needs to own for each platform: the native
/// configuration file. The rest —the `.app`, the APK— is a product of the build,
/// gets rebuilt from scratch on every compilation and lives in
/// `.angular-native/build`, which is in the `.gitignore`.
pub fn add(workspace: &Workspace, platform: &str) -> Result<()> {
    let project = workspace.project()?;
    let (dir, file_name, contents) = match platform {
        "ios" => (
            "ios",
            "Info.plist",
            plist(workspace, Family::Ios, &project.name, &project.bundle_id)?,
        ),
        "tvos" => (
            "tvos",
            "Info.plist",
            plist(workspace, Family::TvOs, &project.name, &project.bundle_id)?,
        ),
        "visionos" => (
            "visionos",
            "Info.plist",
            plist(workspace, Family::VisionOs, &project.name, &project.bundle_id)?,
        ),
        "android" => (
            "android",
            "AndroidManifest.xml",
            android_manifest(workspace, &project.name)?,
        ),
        other => bail!(
            "I do not know how to add {other:?}. `an add` knows ios, tvos, visionos and android; \
             the other platforms have nothing yet the project needs to keep."
        ),
    };

    let path = project.root.join(dir).join(file_name);
    if path.is_file() {
        eprintln!("==> {} already exists; it is left alone", path.display());
    } else {
        write_file(&path, &contents, false)?;
    }
    register_platform(&project.root, platform)?;
    eprintln!();
    eprintln!(
        "{} is yours from now on: `an` copies it into the build and never rewrites it.\n\
         What does get remade from scratch on every compilation is {}, which is not source.",
        path.display(),
        workspace.build_dir().join(dir).display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

fn check_angular(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("{} does not exist", root.display());
    }
    let mut missing: Vec<&str> = Vec::new();
    if !root.join("angular.json").is_file() {
        missing.push("angular.json");
    }
    if !workspace::depends_on_angular(root) {
        missing.push("@angular/core in the package.json dependencies");
    }
    if missing.is_empty() {
        return Ok(());
    }
    bail!(
        "{} does not look like an Angular project: {} is missing.\n\
         `an init` is run inside a project that already exists; if you do not have one yet:\n\
         \x20   npx @angular/cli new my-app",
        root.display(),
        missing.join(" and ")
    )
}

/// A package identifier both iOS and Android will accept. Both are fussy and
/// neither complains early: Android fails on install and iOS on signing, half an
/// hour later.
fn check_identifier(id: &str) -> Result<()> {
    let parts: Vec<&str> = id.split('.').collect();
    let valid = parts.len() >= 2
        && parts.iter().all(|part| {
            part.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && part.chars().all(|c| c.is_ascii_alphanumeric())
        });
    if !valid {
        bail!(
            "{id:?} is no good as an app identifier: it has to be at least two parts \
             separated by dots, each one starting with a letter and with no hyphens or \
             underscores. For example: com.example.myapp"
        );
    }
    Ok(())
}

/// `ngc` is what compiles the templates, and it belongs to the project, not to
/// the SDK: that way the app is compiled against the version of Angular the user
/// has installed.
///
/// It is looked for by climbing up the directories, which is what `npx` will do
/// when the time comes to run it. In a monorepo with workspaces the binaries are
/// up top and not in the package, and demanding it down here would install a
/// second copy for nothing.
fn ensure_compiler(root: &Path) -> Result<()> {
    if has_ngc(root) {
        return Ok(());
    }
    let manager = PackageManager::detect(root);
    eprintln!("==> @angular/compiler-cli is missing; installing it with {}", manager.program());
    let args = manager.add_dev(&["@angular/compiler-cli".to_owned()]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    crate::build::run_in(root, manager.program(), &argv, &format!("{} failed", manager.program()))
        .context("@angular/compiler-cli could not be installed")?;
    if !has_ngc(root) {
        bail!(
            "{} finished fine but there is still no reachable node_modules/.bin/ngc",
            manager.program()
        );
    }
    Ok(())
}

fn has_ngc(from: &Path) -> bool {
    find_ngc(from).is_some()
}

/// Where the `ngc` that will be run actually is.
///
/// The same climb as `has_ngc`, and it is a climb for the reason
/// `ensure_compiler` gives: in a workspace the binaries are at the top and not
/// in the package. Answering the path rather than a yes/no is what lets the
/// compile step run it without going through a package manager to find it.
fn find_ngc(from: &Path) -> Option<std::path::PathBuf> {
    let mut dir = from.to_owned();
    loop {
        let candidate = dir.join("node_modules/.bin/ngc");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Leaves `@angular-native/primitives` and `@angular-native/platform` installed
/// in the project.
///
/// Both are packages from this repository and neither is published on npm yet.
/// Of the three ways of getting them into a project from outside:
///
///   · a **local path** (`file:../angular-native/packages/primitives`) is the
///     easiest to write and the worst of the three: npm installs it as a
///     symlink, the path belongs to the disk of whoever ran `an init`, and the
///     `package-lock.json` that gets committed is no use to anybody else.
///   · a **vendored tarball** —`npm pack`, inside the project, with a version—
///     is an exact copy of what the SDK held that day. It is committed with the
///     project, `npm ci` reinstalls it with no network and no SDK in sight, and
///     the build cannot drift away from the framework without somebody seeing it
///     in a diff.
///   · **publishing them on npm** is what will be needed the day this comes out
///     of the drawer, and then only the specifier changes: what lands in
///     `node_modules` is exactly the same, so nothing above it finds out.
///
/// The second one won. And what gets packed is not the sources: it is the two
/// **compiled** packages, with their `.d.ts` and in partial mode, which is how
/// any Angular library is published. This is not purity for its own sake —
/// TypeScript emits no JavaScript for sources it finds under `node_modules`, it
/// takes them for an external library already compiled, so putting the `.ts`
/// files in there produces a bundle missing half the framework and neither `ngc`
/// nor esbuild says a word—. Compiling them first turns that silence back into
/// the ordinary case: they resolve like any other dependency and the Angular
/// Linker in `scripts/bundle.mjs` does the rest.
///
/// They are compiled with the project's `ngc`, not the SDK's: that way the
/// `.d.ts` files and the partial declarations come out of the same version of
/// Angular the app is compiled against.
fn install_packages(sdk: &Path, root: &Path, force: bool) -> Result<()> {
    let installed = PACKAGES
        .iter()
        .all(|(name, _)| root.join("node_modules").join(name).join("package.json").is_file());
    if installed && !force {
        eprintln!("==> the framework packages are already installed");
        return Ok(());
    }

    let vendor = root.join(".angular-native/vendor");
    std::fs::create_dir_all(&vendor)?;
    let staging = root.join(".angular-native/build/packages");
    let mut tarballs: Vec<String> = Vec::new();
    for (name, short) in PACKAGES {
        let source = sdk.join("packages").join(short);
        if !source.join("src/public-api.ts").is_file() {
            bail!("the SDK does not carry {name}: {} is missing", source.join("src/public-api.ts").display());
        }
        let destination = staging.join(short);
        let _ = std::fs::remove_dir_all(&destination);
        std::fs::create_dir_all(&destination)?;

        eprintln!("==> compiling {name}");
        std::fs::write(destination.join("tsconfig.json"), package_tsconfig(&source, &staging))?;
        // The binary out of the project's own `node_modules`, not
        // `npm exec`: all four managers write `node_modules/.bin`, so this is
        // the one way of saying "the project's ngc" that does not first have
        // to know which of them wrote it. It is also the same ngc `npm exec`
        // would have found, so nothing about the compilation changes.
        let ngc = find_ngc(root).with_context(|| {
            format!(
                "there is no reachable node_modules/.bin/ngc, so there is nothing to compile \
                 {name} with: the project's dependencies are not installed"
            )
        })?;
        crate::build::run_in(
            root,
            &ngc.to_string_lossy(),
            &["-p", &destination.join("tsconfig.json").to_string_lossy()],
            "ngc failed",
        )
        .with_context(|| format!("{name} could not be compiled"))?;
        let api = destination.join("dist/public-api.js");
        if !api.is_file() {
            bail!("`ngc` finished fine but left no {}", api.display());
        }
        std::fs::write(destination.join("package.json"), package_json(name, &source)?)?;

        // `npm pack` writes the file's name on standard output, and that is the
        // only place the version lives: putting it together by hand here would
        // be guesswork.
        //
        // This one stays npm's whatever the project uses, and unlike the
        // install above that is safe: packing turns a directory outside
        // `node_modules` into a tarball and never reads or writes the tree
        // another manager laid out, which is the only thing they disagree
        // about. npm is there wherever Node is.
        let output = capture(
            root,
            "npm",
            &[
                "pack",
                &destination.to_string_lossy(),
                "--pack-destination",
                &vendor.to_string_lossy(),
                "--silent",
            ],
        )
        .with_context(|| format!("{name} could not be packed"))?;
        let file_name = output
            .lines()
            .rfind(|line| line.trim().ends_with(".tgz"))
            .with_context(|| format!("npm pack did not say which file it wrote for {name}"))?
            .trim()
            .to_owned();
        if !vendor.join(&file_name).is_file() {
            bail!("npm pack said it wrote {file_name}, but it is not in {}", vendor.display());
        }
        // Relative: it is what ends up in the user's `package.json`, and an
        // absolute path would only work on this machine.
        tarballs.push(format!("file:.angular-native/vendor/{file_name}"));
    }

    // Whatever wrote this `node_modules` is what writes into it now. See
    // `PackageManager::detect`: getting this wrong does not produce a message
    // about getting it wrong.
    let manager = PackageManager::detect(root);
    let args = manager.add(&tarballs);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    eprintln!("==> {} {}", manager.program(), args.join(" "));
    crate::build::run_in(
        root,
        manager.program(),
        &argv,
        &format!("{} failed", manager.program()),
    )
    .with_context(|| {
        format!(
            "the framework packages could not be installed with {}, which is what this \
             project's lockfile says laid out its node_modules",
            manager.program()
        )
    })?;

    for (name, _) in PACKAGES {
        let dir = root.join("node_modules").join(name);
        if !dir.join("package.json").is_file() {
            bail!("{} finished fine but {name} is not installed", manager.program());
        }
    }
    Ok(())
}

/// The tsconfig one of the framework's packages is compiled with.
///
/// `compilationMode: partial` is what makes this a publishable library: the
/// decorators are left as `ɵɵngDeclare*` declarations that the Angular Linker
/// resolves at packaging time, instead of code tied to the compiler's exact
/// version.
fn package_tsconfig(source: &Path, staging: &Path) -> String {
    let primitives = staging.join("primitives/dist/public-api.d.ts");
    format!(
        r#"{{
  "compilerOptions": {{
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    "lib": ["es2022", "dom"],
    "strict": true,
    "skipLibCheck": true,
    "useDefineForClassFields": false,
    "experimentalDecorators": false,
    "moduleDetection": "force",
    "declaration": true,
    "outDir": "dist",
    "rootDir": "{src}",
    "paths": {{
      "@angular-native/primitives": ["{primitives}"]
    }},
    "types": []
  }},
  "files": ["{api}"],
  "angularCompilerOptions": {{
    "strictTemplates": true,
    "compilationMode": "partial"
  }}
}}
"#,
        src = source.join("src").display(),
        api = source.join("src/public-api.ts").display(),
        primitives = primitives.display()
    )
}

/// The packed package's `package.json`. The version and the peers come from the
/// one in the SDK: it is the same package, only compiled.
fn package_json(name: &str, source: &Path) -> Result<String> {
    let manifest = source.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("{} could not be read", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", manifest.display()))?;
    let version = parsed
        .get("version")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: version is missing", manifest.display()))?;
    let peers = parsed
        .get("peerDependencies")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    Ok(format!(
        r#"{{
  "name": "{name}",
  "version": "{version}",
  "type": "module",
  "main": "dist/public-api.js",
  "module": "dist/public-api.js",
  "types": "dist/public-api.d.ts",
  "peerDependencies": {peers}
}}
"#,
        peers = serde_json::to_string_pretty(&peers).expect("an object always serialises")
    ))
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/// Writes a file. If it already exists and no rewrite was asked for, it is left
/// alone and that is said out loud.
fn write_file(path: &Path, contents: &str, force: bool) -> Result<()> {
    let short_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if path.is_file() && !force {
        eprintln!("    already there  {short_name}  ({})", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
        .with_context(|| format!("{} could not be written", path.display()))?;
    eprintln!("    written        {short_name}  ({})", path.display());
    Ok(())
}

fn write_project_manifest(
    root: &Path,
    name: &str,
    bundle_id: &str,
    entry: &Path,
    platforms: &[String],
) -> Result<()> {
    let platforms_list = platforms
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let contents = format!(
        r#"{{
  "app": {{
    "name": "{name}",
    "bundleId": "{bundle_id}",
    "entry": "{entry}"
  }},
  "platforms": [{platforms_list}]
}}
"#,
        entry = entry.display()
    );
    write_file(&root.join(MARKER), &contents, true)
}

fn register_platform(root: &Path, platform: &str) -> Result<()> {
    let project = Project::read(root)?;
    if project.platforms.iter().any(|p| p == platform) {
        return Ok(());
    }
    let mut platforms = project.platforms.clone();
    platforms.push(platform.to_owned());
    platforms.sort();
    write_project_manifest(root, &project.name, &project.bundle_id, &project.entry, &platforms)
}

/// The lines `an init` adds to the project's `.gitignore`, with the comment that
/// explains each group.
///
/// Two groups, and they are there for different reasons. The build directory is
/// noise: it is remade from scratch on every compilation. The tsconfig and the
/// tarballs in `.angular-native/vendor` are deliberately **not** in here — the
/// first is configuration and the second is what makes `npm ci` reinstall
/// exactly the same framework packages on the machine next door.
///
/// The credentials are the other reason, and it is not tidiness. A release
/// keystore in a repository is the app's whole identity on Google Play, and once
/// it is pushed the fix is to generate another one and ask Google to change the
/// upload key. A `.p12` is a certificate's private key. Neither is something to
/// notice later, so the patterns go in at `an init` time, before there is
/// anything to catch — which is the only moment this costs nothing.
const GITIGNORE: [(&str, &[&str]); 2] = [
    (
        "# angular-native: the .app, the APK and the compiled JS are remade from scratch.",
        &["/.angular-native/build/"],
    ),
    (
        "# Signing credentials. None of these belongs in a repository: a keystore is the\n         # app's identity on Play, and a .p12 carries a certificate's private key.\n         # See https://angular-native.github.io/guide/signing-and-distribution/.",
        &["*.keystore", "*.jks", "*.p12", "*.mobileprovision"],
    ),
];

/// Adds to the `.gitignore` what angular-native generates and what must never be
/// committed. See [`GITIGNORE`].
///
/// Line by line, and only what is missing: the file belongs to the project and
/// may already say some of this.
fn extend_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let mut updated = current.clone();
    let mut added = 0usize;
    for (comment, lines) in GITIGNORE {
        let missing: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| !current.lines().any(|existing| existing.trim() == *line))
            .collect();
        if missing.is_empty() {
            continue;
        }
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(&format!("\n{comment}\n{}\n", missing.join("\n")));
        added += missing.len();
    }
    if added == 0 {
        eprintln!("    already there  .gitignore");
        return Ok(());
    }
    std::fs::write(&path, updated)
        .with_context(|| format!("{} could not be written", path.display()))?;
    eprintln!("    extended       .gitignore  ({added} lines)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

/// The tsconfig for the native build.
///
/// It is not the web project's and it cannot be: there is no DOM here, the app
/// comes in through a different file, and `paths` has to send both framework
/// packages to their TypeScript and not to the `package.json`'s `main`.
///
/// The two framework packages carry no `paths`: they are installed and compiled
/// in `node_modules`, and they resolve like any other dependency.
fn tsconfig(entry: &Path) -> String {
    format!(
        r#"// Generated by `an init`. This is the tsconfig of the native build: `ngc` uses
// it to compile the templates into native views, not into DOM.
//
// It can be edited —`an` does not rewrite it if it already exists— but two things
// have to stay as they are: `rootDir` at the root of the project and `outDir`
// inside `.angular-native/build/js`. That is where `an build` picks the entry
// point up from.
{{
  "compilerOptions": {{
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    // `dom` is in for the types alone: Angular's .d.ts files reference Document,
    // Element and Event. At runtime none of the three exists.
    "lib": ["es2022", "dom"],
    "strict": true,
    "skipLibCheck": true,
    "useDefineForClassFields": false,
    "experimentalDecorators": false,
    "moduleDetection": "force",
    "outDir": "build/js",
    "rootDir": "..",
    "types": []
  }},
  "files": ["../{entry}"],
  "angularCompilerOptions": {{
    "strictTemplates": true,
    "compilationMode": "full"
  }}
}}
"#,
        entry = entry.display()
    )
}

const ENTRY_POINT: &str = r#"// The native app's startup. The web one is still src/main.ts.
import { bootstrapNativeApplication } from '@angular-native/platform'

import { AppNative } from './app/app-native'

bootstrapNativeApplication(AppNative).catch((error) => {
  console.error('the startup failed:', error)
})
"#;

fn root_component(name: &str) -> String {
    format!(
        r#"import {{ ChangeDetectionStrategy, Component, signal }} from '@angular/core'
import {{ NATIVE_PRIMITIVES }} from '@angular-native/primitives'

/**
 * The native app's root component.
 *
 * It is an ordinary Angular component: signals, `@if`, `@for` and bindings, the
 * usual ones. The only difference is that the elements are not HTML:
 * `<an-view>` ends up being a `UIView` on iOS and an `AnViewGroup` on Android.
 *
 * The templates are not shared with the web app. A `<div>` has no native
 * equivalent and an `<an-view>` cannot be painted in a browser, so there are two
 * roots and each one has its own.
 */
@Component({{
  selector: 'app-native',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.paddingTop]="'64'"
      [style.paddingHorizontal]="'24'"
      [style.gap]="'16'"
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'">
      <an-text [fontSize]="30" [fontWeight]="'bold'" [color]="'#f4f7ff'">{name}</an-text>
      <an-text [fontSize]="16" [color]="'#9fb0d4'">
        Running on native views. No DOM and no WebView.
      </an-text>
      <an-button [title]="'Taps: ' + taps()" [variant]="'filled'" (press)="tap()" />
    </an-view>
  `
}})
export class AppNative {{
  readonly taps = signal(0)

  tap(): void {{
    this.taps.update((count) => count + 1)
  }}
}}
"#
    )
}

/// The project's `Info.plist`, taken from the one the shell uses.
///
/// It starts from the SDK's so as not to keep two copies of the same list of
/// keys, and only the three that identify the app are changed. If the SDK's
/// stops carrying the values expected, it stops: a half-substituted plist
/// produces an app that installs and does not open.
fn plist(
    workspace: &Workspace,
    family: Family,
    name: &str,
    bundle_id: &str,
) -> Result<String> {
    // The name and the identifier the shell's plist carries are the monorepo's
    // with the family's decoration on; the project's get decorated the same way,
    // and so what comes out of here already passes the check the build does.
    // Both suffixes come from the same place, `Family::suffix`, so they cannot
    // drift apart.
    let (name_suffix, id_suffix) = family.suffix();
    let source = workspace.root.join(match family {
        Family::Ios => "shells/ios/Resources/Info.plist",
        Family::TvOs => "shells/tvos/Resources/Info.plist",
        Family::VisionOs => "shells/visionos/Resources/Info.plist",
    });
    let text = std::fs::read_to_string(&source)
        .with_context(|| format!("{} could not be read", source.display()))?;
    let text = substitute(
        &text,
        &source,
        &[
            (
                &format!("<string>AngularNative{name_suffix}</string>"),
                &format!("<string>{name}{name_suffix}</string>"),
                2,
            ),
            (
                &format!("<string>dev.angularnative.playground{id_suffix}</string>"),
                &format!("<string>{bundle_id}{id_suffix}</string>"),
                1,
            ),
        ],
    )?;
    // The comment goes after the XML declaration, which has to come first.
    Ok(prepend(&text, PLIST_HEADER))
}

/// The project's `AndroidManifest.xml`, taken from the shell's.
///
/// The `package` stays as it is —that is where the shell's classes live— and
/// what changes is the label. The identifier Android installs the app under
/// comes from `app.bundleId` and `aapt2` applies it at link time.
fn android_manifest(workspace: &Workspace, name: &str) -> Result<String> {
    let source = workspace.root.join("shells/android/AndroidManifest.xml");
    let text = std::fs::read_to_string(&source)
        .with_context(|| format!("{} could not be read", source.display()))?;
    let text = substitute(
        &text,
        &source,
        &[("android:label=\"AngularNative\"", &format!("android:label=\"{name}\""), 1)],
    )?;
    // The comment goes after the XML declaration, which has to come first.
    Ok(prepend(&text, MANIFEST_HEADER))
}

/// Slots a comment in right behind the XML declaration. Nothing can go in front
/// of it: an `<?xml?>` that is not the first thing in the file is not valid XML,
/// and `plutil` turns the whole plist down.
fn prepend(text: &str, header: &str) -> String {
    match text.split_once('\n') {
        Some((declaration, rest)) => format!("{declaration}\n{header}{rest}"),
        None => format!("{header}{text}"),
    }
}

/// Substitutes, checking how many times each thing was supposed to appear.
fn substitute(text: &str, source: &Path, changes: &[(&str, &str, usize)]) -> Result<String> {
    let mut output = text.to_owned();
    for (needle, replacement, times) in changes {
        let found = output.matches(needle).count();
        if found != *times {
            bail!(
                "{}: {needle:?} was expected {times} time(s) and it turns up {found}. \
                 The shell has changed and this template has been left behind.",
                source.display()
            );
        }
        output = output.replace(needle, replacement);
    }
    Ok(output)
}

const PLIST_HEADER: &str = "<!--\n  \
    Created by `an add`. From here on it is yours: `an` copies it into the .app\n  \
    on every compilation and never rewrites it.\n\n  \
    CFBundleExecutable and CFBundleIdentifier have to go on matching app.name\n  \
    and app.bundleId in angular-native.json. If they stop matching, the build\n  \
    stops and says so.\n-->\n";

const MANIFEST_HEADER: &str = "<!--\n  \
    Created by `an add android`. From here on it is yours: the permissions and\n  \
    whatever the app declares go in here.\n\n  \
    The package attribute is not changed: it is the package of the shell's\n  \
    classes. The identifier Android installs the app under comes from\n  \
    app.bundleId in angular-native.json.\n-->\n";

// ---------------------------------------------------------------------------
// Odds and ends
// ---------------------------------------------------------------------------

/// The app's name, taken from the `package.json`. It ends up being the
/// executable's inside the `.app` and the one read under the icon, so it is
/// turned into PascalCase: `my-app` becomes "MyApp".
fn app_name_from_package_json(root: &Path) -> Result<String> {
    let manifest = root.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("{} could not be read", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", manifest.display()))?;
    let raw = parsed
        .get("name")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: name is missing", manifest.display()))?;
    let name: String = raw
        .rsplit('/')
        .next()
        .unwrap_or(raw)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if name.is_empty() {
        bail!("{}: the name {raw:?} yields no app name; pass --name", manifest.display());
    }
    Ok(name)
}

fn slug(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

/// Which tool puts things in this project's `node_modules`.
///
/// It matters because `node_modules` is not a format the four of them agree
/// about, and running the wrong one over a tree another one laid out does not
/// produce a message about that. npm over bun's tree dies with
///
/// ```text
/// npm error Cannot read properties of null (reading 'isDescendantOf')
/// ```
///
/// which names neither npm's problem nor bun, and its only clue is the
/// `node_modules/.bun/…` paths buried in the peer-dependency warnings above
/// it. That is a long afternoon for somebody whose only mistake was to run
/// `bun install` before `an init`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PackageManager {
    Npm,
    Bun,
    Pnpm,
    Yarn,
}

impl PackageManager {
    /// The lockfile decides, and it is looked for **upwards**.
    ///
    /// A project inside a repository that was installed with bun has no
    /// lockfile of its own and its `node_modules` is bun's all the same:
    /// resolution walks up, so the tree that will be written to is the one up
    /// there. Looking only at the project root is what made `an init` fail on
    /// any app kept inside this very repository.
    ///
    /// With no lockfile anywhere it is npm, which is what a fresh
    /// `npx @angular/cli new` leaves behind.
    fn detect(root: &Path) -> PackageManager {
        for dir in root.ancestors() {
            // Ordered, because a project can carry more than one: a `bun.lock`
            // next to a stale `package-lock.json` is a project that moved to
            // bun and did not delete the old one, and bun is what wrote the
            // tree. npm is last for the same reason.
            for (file, manager) in [
                ("bun.lock", PackageManager::Bun),
                ("bun.lockb", PackageManager::Bun),
                ("pnpm-lock.yaml", PackageManager::Pnpm),
                ("yarn.lock", PackageManager::Yarn),
                ("package-lock.json", PackageManager::Npm),
            ] {
                if dir.join(file).is_file() {
                    return manager;
                }
            }
        }
        PackageManager::Npm
    }

    fn program(self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Bun => "bun",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
        }
    }

    /// Installing a tarball as an exact dependency, in each one's words.
    ///
    /// npm and bun are the two that have been run. pnpm's and yarn's lines are
    /// written from their documentation and have never been executed by
    /// anybody working on this — the same admission the signing paths make.
    /// They are here rather than absent because a good-faith line somebody can
    /// correct beats a refusal to try.
    /// The same, for a dependency the app builds with rather than ships.
    fn add_dev(self, specs: &[String]) -> Vec<String> {
        let mut args: Vec<String> = match self {
            PackageManager::Npm => ["install", "--save-dev", "--no-audit", "--no-fund"]
                .iter()
                .map(|a| (*a).to_owned())
                .collect(),
            PackageManager::Bun => vec!["add".into(), "--dev".into()],
            PackageManager::Pnpm => vec!["add".into(), "--save-dev".into()],
            PackageManager::Yarn => vec!["add".into(), "--dev".into()],
        };
        args.extend(specs.iter().cloned());
        args
    }

    fn add(self, specs: &[String]) -> Vec<String> {
        let mut args: Vec<String> = match self {
            PackageManager::Npm => {
                ["install", "--save", "--save-exact", "--no-audit", "--no-fund"]
                    .iter()
                    .map(|a| (*a).to_owned())
                    .collect()
            }
            PackageManager::Bun => vec!["add".into(), "--exact".into()],
            PackageManager::Pnpm => vec!["add".into(), "--save-exact".into()],
            PackageManager::Yarn => vec!["add".into(), "--exact".into()],
        };
        args.extend(specs.iter().cloned());
        args
    }
}

fn capture(cwd: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("{program} could not be run"))?;
    if !output.status.success() {
        bail!("{program} failed:\n{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
