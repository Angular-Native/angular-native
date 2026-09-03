//! Building the JS bundle: `ngc` for the templates, esbuild for the packaging.
//!
//! Nothing from Angular's CLI. `ng build` produces a browser bundle, with its
//! polyfills and its assumptions about the DOM; what is needed here is the
//! opposite.
//!
//! Both worlds —the monorepo and a project from outside— go down the same road;
//! all that changes is four paths and where it runs from. Outside, `ngc` and
//! esbuild run with the working directory set to the project, so `@angular/core`
//! comes from the project's dependencies and not the SDK's: the app is compiled
//! against the version of Angular the user has installed, which is the only one
//! that makes any sense.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::plugins::Plugin;
use crate::workspace::Workspace;

/// The framework's two packages, with the directory they come from inside the
/// monorepo. In there they are loose TypeScript and esbuild cannot resolve them
/// through `node_modules`: it has to be pointed at the `.js` `ngc` has just
/// written.
///
/// In a project from outside none of these aliases is needed: both packages are
/// installed, compiled and carrying their `.d.ts`, and they resolve like any
/// other dependency.
const PACKAGES: [(&str, &str); 2] = [
    ("@angular-native/platform", "packages/platform-native"),
    ("@angular-native/primitives", "packages/primitives"),
];

pub fn bundle(
    workspace: &Workspace,
    app: &Path,
    release: bool,
    plugins: &[Plugin],
) -> Result<PathBuf> {
    let js_dir = workspace.js_dir(app);
    let (tsconfig, entry, out, cwd) = match &workspace.project {
        Some(project) => {
            let tsconfig = project.tsconfig();
            if !tsconfig.is_file() {
                bail!(
                    "{} is missing, and it is the tsconfig the app is compiled with.\n\
                     `an init` writes it; run it again in {}.",
                    tsconfig.display(),
                    project.root.display()
                );
            }
            let entry = js_dir.join(&project.entry).with_extension("js");
            let out = workspace.build_dir().join("bundle/main.js");
            (tsconfig, entry, out, project.root.clone())
        }
        None => {
            let name = Workspace::name(app);
            let tsconfig = workspace.root.join(app).join("tsconfig.json");
            // Relative: in the monorepo the working directory is the root, and
            // short paths are what shows up on screen.
            let entry = PathBuf::from("build/js").join(&name).join(app).join("src/main.js");
            let out = workspace.root.join("build/bundle").join(&name).join("main.js");
            (tsconfig, entry, out, workspace.root.clone())
        }
    };

    eprintln!("==> ngc (AOT) {}", app.display());
    run_in(&cwd, "npx", &["ngc", "-p", &tsconfig.to_string_lossy()], "the AOT compilation failed")?;

    eprintln!("==> esbuild{}", if release { " (release)" } else { "" });
    std::fs::create_dir_all(out.parent().expect("the output has a parent"))?;

    // The packaging goes through a Node script and not esbuild's binary: the
    // Angular Linker is needed, and that is a Babel plugin. The script lives in
    // the SDK, so its own `import`s resolve against the SDK's `node_modules`
    // even when the working directory is somewhere else.
    let mut args: Vec<String> = vec![
        workspace.root.join("scripts/bundle.mjs").to_string_lossy().into_owned(),
        entry.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
    ];
    if workspace.project.is_none() {
        for (package, source) in PACKAGES {
            // Absolute: esbuild's `alias` resolves against the importer, not
            // against the working directory.
            let compiled = js_dir.join(source).join("src/public-api.js");
            args.push(format!("--alias={package}={}", compiled.display()));
        }
    }
    // And one per plugin that brings its API in TypeScript: `ngc` has left it
    // in the same place as the app's, and the package's import has to point
    // there and not at whatever is in `node_modules`.
    args.extend(crate::plugins::aliases(workspace, &js_dir, plugins));
    if release {
        // `ngDevMode` set to false strips Angular's development checks, which
        // are close to half the bundle.
        args.push("--release".into());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_in(&cwd, "node", &borrowed, "the bundling failed")?;

    let size = std::fs::metadata(&out)?.len();
    eprintln!("==> {} KB in {}", size / 1024, out.display());
    Ok(out)
}

pub fn run(workspace: &Workspace, program: &str, args: &[&str], context: &str) -> Result<()> {
    run_in(&workspace.root, program, args, context)
}

pub fn run_in(cwd: &Path, program: &str, args: &[&str], context: &str) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .with_context(|| format!("{program} could not be run"))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}
