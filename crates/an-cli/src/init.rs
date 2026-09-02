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
        None => std::env::current_dir().context("no se pudo leer el directorio actual")?,
    };
    let sdk = workspace::sdk_root()?;

    // ---- Everything that can say no, before anything is touched ----------
    check_angular(&root)?;
    let already = root.join(MARKER).is_file();
    if already && !force {
        eprintln!("==> {} ya está inicializado; solo se añade lo que falte", root.display());
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

    eprintln!("==> proyecto Angular en {}", root.display());
    eprintln!("==> SDK de angular-native en {}", sdk.display());

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
    eprintln!("Listo. {app_name} ({bundle_id})");
    eprintln!("  an add ios        crea ios/Info.plist, que a partir de ahí es tuyo");
    eprintln!("  an build          solo el bundle JS");
    eprintln!("  an ios            compila, arma el .app y lo lanza en el simulador");
    eprintln!("  an dev            lo mismo, recargando al guardar");
    eprintln!();
    eprintln!(
        "La app nativa arranca en {} y su componente raíz es src/app/app-native.ts.\n\
         Tu app web sigue como estaba: las plantillas no se comparten, porque una es HTML\n\
         y la otra son vistas nativas.",
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
            "no sé añadir {other:?}. `an add` conoce ios, tvos, visionos y android; \
             las demás plataformas todavía no tienen nada que el proyecto deba guardar."
        ),
    };

    let path = project.root.join(dir).join(file_name);
    if path.is_file() {
        eprintln!("==> {} ya existe; no se toca", path.display());
    } else {
        write_file(&path, &contents, false)?;
    }
    register_platform(&project.root, platform)?;
    eprintln!();
    eprintln!(
        "{} es tuyo a partir de ahora: `an` lo copia al build y no lo reescribe nunca.\n\
         Lo que sí se rehace entero en cada compilación es {}, que no es fuente.",
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
        bail!("{} no existe", root.display());
    }
    let mut missing: Vec<&str> = Vec::new();
    if !root.join("angular.json").is_file() {
        missing.push("angular.json");
    }
    if !workspace::depends_on_angular(root) {
        missing.push("@angular/core en las dependencias del package.json");
    }
    if missing.is_empty() {
        return Ok(());
    }
    bail!(
        "{} no parece un proyecto Angular: falta {}.\n\
         `an init` se ejecuta dentro de un proyecto ya creado; si aún no lo tienes:\n\
         \x20   npx @angular/cli new mi-app",
        root.display(),
        missing.join(" y ")
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
            "{id:?} no vale como identificador de app: tienen que ser al menos dos tramos \
             separados por puntos, cada uno empezando por letra y sin guiones ni subrayados. \
             Por ejemplo: com.ejemplo.miapp"
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
    eprintln!("==> falta @angular/compiler-cli; instalándolo");
    npm(root, &["install", "--save-dev", "--no-audit", "--no-fund", "@angular/compiler-cli"])
        .context("no se pudo instalar @angular/compiler-cli")?;
    if !has_ngc(root) {
        bail!("npm terminó bien pero sigue sin haber un node_modules/.bin/ngc alcanzable");
    }
    Ok(())
}

fn has_ngc(from: &Path) -> bool {
    let mut dir = from.to_owned();
    loop {
        if dir.join("node_modules/.bin/ngc").exists() {
            return true;
        }
        if !dir.pop() {
            return false;
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
        eprintln!("==> los paquetes del framework ya están instalados");
        return Ok(());
    }

    let vendor = root.join(".angular-native/vendor");
    std::fs::create_dir_all(&vendor)?;
    let staging = root.join(".angular-native/build/packages");
    let mut tarballs: Vec<String> = Vec::new();
    for (name, short) in PACKAGES {
        let source = sdk.join("packages").join(short);
        if !source.join("src/public-api.ts").is_file() {
            bail!("el SDK no trae {name}: falta {}", source.join("src/public-api.ts").display());
        }
        let destination = staging.join(short);
        let _ = std::fs::remove_dir_all(&destination);
        std::fs::create_dir_all(&destination)?;

        eprintln!("==> compilando {name}");
        std::fs::write(destination.join("tsconfig.json"), package_tsconfig(&source, &staging))?;
        npm(root, &["exec", "--", "ngc", "-p", &destination.join("tsconfig.json").to_string_lossy()])
            .with_context(|| format!("no se pudo compilar {name}"))?;
        let api = destination.join("dist/public-api.js");
        if !api.is_file() {
            bail!("`ngc` terminó bien pero no dejó {}", api.display());
        }
        std::fs::write(destination.join("package.json"), package_json(name, &source)?)?;

        // `npm pack` writes the file's name on standard output, and that is the
        // only place the version lives: putting it together by hand here would
        // be guesswork.
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
        .with_context(|| format!("no se pudo empaquetar {name}"))?;
        let file_name = output
            .lines()
            .rfind(|line| line.trim().ends_with(".tgz"))
            .with_context(|| format!("npm pack no dijo qué fichero escribió para {name}"))?
            .trim()
            .to_owned();
        if !vendor.join(&file_name).is_file() {
            bail!("npm pack dijo haber escrito {file_name}, pero no está en {}", vendor.display());
        }
        // Relative: it is what ends up in the user's `package.json`, and an
        // absolute path would only work on this machine.
        tarballs.push(format!("file:.angular-native/vendor/{file_name}"));
    }

    eprintln!("==> npm install {}", tarballs.join(" "));
    let mut args: Vec<&str> = vec!["install", "--save", "--save-exact", "--no-audit", "--no-fund"];
    args.extend(tarballs.iter().map(String::as_str));
    npm(root, &args).context("no se pudieron instalar los paquetes del framework")?;

    for (name, _) in PACKAGES {
        let dir = root.join("node_modules").join(name);
        if !dir.join("package.json").is_file() {
            bail!("npm terminó bien pero {name} no está instalado");
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
        .with_context(|| format!("no se pudo leer {}", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} no es JSON válido", manifest.display()))?;
    let version = parsed
        .get("version")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: falta version", manifest.display()))?;
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
        peers = serde_json::to_string_pretty(&peers).expect("un objeto siempre serializa")
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
        eprintln!("    ya estaba  {short_name}  ({})", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    eprintln!("    escrito     {short_name}  ({})", path.display());
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

/// Adds to the `.gitignore` the one thing angular-native generates that is not a
/// source.
///
/// Only `.angular-native/build`. The tsconfig and the tarballs in
/// `.angular-native/vendor` do get committed: the first is configuration and the
/// second is what makes `npm ci` reinstall exactly the same framework packages
/// on the machine next door.
fn extend_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let line = "/.angular-native/build/";
    if current.lines().any(|l| l.trim() == line) {
        eprintln!("    ya estaba  .gitignore");
        return Ok(());
    }
    let mut updated = current;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&format!(
        "\n# angular-native: el .app, el APK y el JS compilado se rehacen enteros.\n{line}\n"
    ));
    std::fs::write(&path, updated)
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    eprintln!("    ampliado    .gitignore");
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
        r#"// Generado por `an init`. Es el tsconfig del build nativo: `ngc` lo usa para
// compilar las plantillas a vistas nativas, no a DOM.
//
// Se puede editar —`an` no lo reescribe si ya existe— pero dos cosas tienen que
// quedarse como están: `rootDir` en la raíz del proyecto y `outDir` dentro de
// `.angular-native/build/js`. De ahí saca `an build` el punto de entrada.
{{
  "compilerOptions": {{
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    // `dom` entra solo por los tipos: los .d.ts de Angular referencian Document,
    // Element y Event. En tiempo de ejecución no existe ninguno de los tres.
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

const ENTRY_POINT: &str = r#"// El arranque de la app nativa. El de la web sigue siendo src/main.ts.
import { bootstrapNativeApplication } from '@angular-native/platform'

import { AppNative } from './app/app-native'

bootstrapNativeApplication(AppNative).catch((error) => {
  console.error('el arranque falló:', error)
})
"#;

fn root_component(name: &str) -> String {
    format!(
        r#"import {{ ChangeDetectionStrategy, Component, signal }} from '@angular/core'
import {{ NATIVE_PRIMITIVES }} from '@angular-native/primitives'

/**
 * El componente raíz de la app nativa.
 *
 * Es un componente de Angular normal: señales, `@if`, `@for` y bindings, los de
 * siempre. Lo único distinto es que los elementos no son HTML: `<an-view>` acaba
 * siendo una `UIView` en iOS y un `AnViewGroup` en Android.
 *
 * Las plantillas no se comparten con la app web. Un `<div>` no tiene
 * equivalente nativo y `<an-view>` no se puede pintar en un navegador, así que
 * hay dos raíces y cada una con lo suyo.
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
        Corriendo sobre vistas nativas. Sin DOM y sin WebView.
      </an-text>
      <an-button [title]="'Van ' + toques() + ' toques'" [variant]="'filled'" (press)="toca()" />
    </an-view>
  `
}})
export class AppNative {{
  readonly toques = signal(0)

  toca(): void {{
    this.toques.update((valor) => valor + 1)
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
        .with_context(|| format!("no se pudo leer {}", source.display()))?;
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
        .with_context(|| format!("no se pudo leer {}", source.display()))?;
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
                "{}: se esperaba encontrar {needle:?} {times} vez/veces y aparece {found}. \
                 El shell ha cambiado y esta plantilla se ha quedado atrás.",
                source.display()
            );
        }
        output = output.replace(needle, replacement);
    }
    Ok(output)
}

const PLIST_HEADER: &str = "<!--\n  \
    Creado por `an add`. A partir de aquí es tuyo: `an` lo copia dentro del\n  \
    .app en cada compilación y no lo reescribe nunca.\n\n  \
    CFBundleExecutable y CFBundleIdentifier tienen que seguir coincidiendo con\n  \
    app.name y app.bundleId de angular-native.json. Si dejan de coincidir, el\n  \
    build se para y lo dice.\n-->\n";

const MANIFEST_HEADER: &str = "<!--\n  \
    Creado por `an add android`. A partir de aquí es tuyo: aquí van los permisos\n  \
    y lo que la app declare.\n\n  \
    El atributo package no se cambia: es el paquete de las clases del shell. El\n  \
    identificador con el que Android instala la app sale de app.bundleId de\n  \
    angular-native.json.\n-->\n";

// ---------------------------------------------------------------------------
// Odds and ends
// ---------------------------------------------------------------------------

/// The app's name, taken from the `package.json`. It ends up being the
/// executable's inside the `.app` and the one read under the icon, so it is
/// turned into PascalCase: `mi-app` is «MiApp».
fn app_name_from_package_json(root: &Path) -> Result<String> {
    let manifest = root.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("no se pudo leer {}", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} no es JSON válido", manifest.display()))?;
    let raw = parsed
        .get("name")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: falta name", manifest.display()))?;
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
        bail!("{}: del name {raw:?} no sale ningún nombre de app; pasa --name", manifest.display());
    }
    Ok(name)
}

fn slug(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

fn npm(cwd: &Path, args: &[&str]) -> Result<()> {
    crate::build::run_in(cwd, "npm", args, "npm falló")
}

fn capture(cwd: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !output.status.success() {
        bail!("{program} falló:\n{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
