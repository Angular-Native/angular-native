//! Compilación del bundle JS: `ngc` para las plantillas, esbuild para empaquetar.
//!
//! Nada del CLI de Angular. `ng build` produce un bundle de navegador, con sus
//! polyfills y sus suposiciones sobre el DOM; aquí hace falta lo contrario.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::workspace::Workspace;

pub fn bundle(workspace: &Workspace, app: &Path, release: bool) -> Result<PathBuf> {
    let name = Workspace::name(app);
    let tsconfig = app.join("tsconfig.json");
    let js_dir = PathBuf::from("build/js").join(&name);
    let out = workspace.root.join("build/bundle").join(&name).join("main.js");

    eprintln!("==> ngc (AOT) {}", app.display());
    run(
        workspace,
        "npx",
        &["ngc", "-p", &tsconfig.to_string_lossy()],
        "la compilación AOT falló",
    )?;

    eprintln!("==> esbuild{}", if release { " (release)" } else { "" });
    std::fs::create_dir_all(out.parent().expect("la salida tiene padre"))?;

    let entry = js_dir.join(app).join("src/main.js");
    let alias_platform = format!(
        "--alias:@angular-native/platform=./{}/packages/platform-native/src/public-api.js",
        js_dir.display()
    );
    let alias_primitives = format!(
        "--alias:@angular-native/primitives=./{}/packages/primitives/src/public-api.js",
        js_dir.display()
    );
    let outfile = format!("--outfile={}", out.display());

    let mut args: Vec<String> = vec![
        "esbuild".into(),
        entry.to_string_lossy().into_owned(),
        "--bundle".into(),
        alias_platform,
        alias_primitives,
        "--format=iife".into(),
        "--platform=neutral".into(),
        "--target=es2022".into(),
        "--main-fields=module,main".into(),
        "--conditions=module".into(),
        outfile,
        "--log-level=warning".into(),
    ];
    if release {
        // `ngDevMode` a false quita las comprobaciones de desarrollo de Angular,
        // que son casi la mitad del bundle.
        args.push("--define:ngDevMode=false".into());
        args.push("--define:ngJitMode=false".into());
        args.push("--minify".into());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "npx", &borrowed, "el empaquetado falló")?;

    let size = std::fs::metadata(&out)?.len();
    eprintln!("==> {} KB en {}", size / 1024, out.display());
    Ok(out)
}

pub fn run(workspace: &Workspace, program: &str, args: &[&str], context: &str) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(&workspace.root)
        .status()
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}
