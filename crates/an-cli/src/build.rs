//! Compilación del bundle JS: `ngc` para las plantillas, esbuild para empaquetar.
//!
//! Nada del CLI de Angular. `ng build` produce un bundle de navegador, con sus
//! polyfills y sus suposiciones sobre el DOM; aquí hace falta lo contrario.
//!
//! Los dos mundos —el monorepo y un proyecto de fuera— pasan por el mismo
//! camino; lo único que cambia son cuatro rutas y desde dónde se ejecuta.
//! Fuera, `ngc` y esbuild corren con el directorio de trabajo en el proyecto,
//! así que `@angular/core` sale de las dependencias del proyecto y no de las
//! del SDK: la app se compila contra la versión de Angular que el usuario
//! tiene instalada, que es la única que tiene sentido.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::plugins::Plugin;
use crate::workspace::Workspace;

/// Los dos paquetes del framework, con el directorio del que salen dentro del
/// monorepo. Ahí son TypeScript suelto y esbuild no los puede resolver por
/// `node_modules`: hay que apuntarle al `.js` que `ngc` acaba de escribir.
///
/// En un proyecto de fuera no hace falta ninguno de estos alias: los dos
/// paquetes están instalados, compilados y con sus `.d.ts`, y se resuelven como
/// cualquier otra dependencia.
const PAQUETES: [(&str, &str); 2] = [
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
                    "falta {}, que es el tsconfig con el que se compila la app.\n\
                     Lo escribe `an init`; vuelve a ejecutarlo en {}.",
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
            // Relativas: en el monorepo el directorio de trabajo es la raíz, y
            // las rutas cortas son las que salen por pantalla.
            let entry = PathBuf::from("build/js").join(&name).join(app).join("src/main.js");
            let out = workspace.root.join("build/bundle").join(&name).join("main.js");
            (tsconfig, entry, out, workspace.root.clone())
        }
    };

    eprintln!("==> ngc (AOT) {}", app.display());
    run_in(&cwd, "npx", &["ngc", "-p", &tsconfig.to_string_lossy()], "la compilación AOT falló")?;

    eprintln!("==> esbuild{}", if release { " (release)" } else { "" });
    std::fs::create_dir_all(out.parent().expect("la salida tiene padre"))?;

    // El empaquetado va por un script de Node y no por el binario de esbuild:
    // hace falta el Angular Linker, que es un plugin de Babel. El script vive
    // en el SDK, así que sus propios `import` se resuelven contra el
    // `node_modules` del SDK aunque el directorio de trabajo sea otro.
    let mut args: Vec<String> = vec![
        workspace.root.join("scripts/bundle.mjs").to_string_lossy().into_owned(),
        entry.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
    ];
    if workspace.project.is_none() {
        for (paquete, origen) in PAQUETES {
            // Absolutas: el `alias` de esbuild resuelve contra el importador, no
            // contra el directorio de trabajo.
            let compilado = js_dir.join(origen).join("src/public-api.js");
            args.push(format!("--alias={paquete}={}", compilado.display()));
        }
    }
    // Y uno por plugin que traiga su API en TypeScript: `ngc` la ha dejado en
    // el mismo sitio que la de la app, y el import del paquete tiene que
    // apuntar ahí y no a lo que haya en `node_modules`.
    args.extend(crate::plugins::aliases(workspace, &js_dir, plugins));
    if release {
        // `ngDevMode` a false quita las comprobaciones de desarrollo de Angular,
        // que son casi la mitad del bundle.
        args.push("--release".into());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_in(&cwd, "node", &borrowed, "el empaquetado falló")?;

    let size = std::fs::metadata(&out)?.len();
    eprintln!("==> {} KB en {}", size / 1024, out.display());
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
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}
