//! Compilación del bundle JS: `ngc` para las plantillas, esbuild para empaquetar.
//!
//! Nada del CLI de Angular. `ng build` produce un bundle de navegador, con sus
//! polyfills y sus suposiciones sobre el DOM; aquí hace falta lo contrario.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::plugins::Plugin;
use crate::workspace::Workspace;

pub fn bundle(
    workspace: &Workspace,
    app: &Path,
    release: bool,
    plugins: &[Plugin],
) -> Result<PathBuf> {
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
    // El empaquetado va por un script de Node y no por el binario de esbuild:
    // hace falta el Angular Linker, que es un plugin de Babel.
    let mut args: Vec<String> = vec![
        "scripts/bundle.mjs".into(),
        entry.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        format!(
            // Absolutas: el `alias` de esbuild resuelve contra el importador,
            // no contra el directorio de trabajo.
            "--alias=@angular-native/platform={}",
            workspace
                .root
                .join(&js_dir)
                .join("packages/platform-native/src/public-api.js")
                .display()
        ),
        format!(
            "--alias=@angular-native/primitives={}",
            workspace
                .root
                .join(&js_dir)
                .join("packages/primitives/src/public-api.js")
                .display()
        ),
    ];
    // Y uno por plugin que traiga su API en TypeScript: `ngc` la ha dejado en
    // `build/js`, junto a la de la app, y el import del paquete tiene que
    // apuntar ahí y no a lo que haya en `node_modules`.
    args.extend(crate::plugins::aliases(workspace, app, plugins));
    if release {
        // `ngDevMode` a false quita las comprobaciones de desarrollo de Angular,
        // que son casi la mitad del bundle.
        args.push("--release".into());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(workspace, "node", &borrowed, "el empaquetado falló")?;

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
