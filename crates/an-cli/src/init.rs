//! `an init` y `an add`: llevar angular-native a un proyecto Angular ajeno.
//!
//! La idea es la de `ng add` o `npx cap init`: alguien tiene su proyecto de
//! `ng new` y quiere llevarlo al móvil sin aprenderse la estructura de este
//! repositorio. Después de `an init`, en su proyecto hay cuatro cosas nuevas y
//! todas se pueden leer en un minuto: el manifiesto `angular-native.json`, un
//! tsconfig para el build nativo, un punto de entrada y un componente raíz.
//!
//! Dos reglas gobiernan este módulo:
//!
//!   · **Ningún fichero del usuario se pisa.** Lo que ya existe se deja y se
//!     dice que se ha dejado. `--force` reescribe solo lo que generamos
//!     nosotros, nunca el código de la app.
//!   · **O termina, o no ha empezado.** Todo lo que puede fallar —que esto sea
//!     un proyecto Angular, que el SDK esté entero, que npm instale— se
//!     comprueba y se hace antes de escribir el manifiesto, que va el último.
//!     Si algo se tuerce, `angular-native.json` no llega a existir, `an` sigue
//!     diciendo que el proyecto no está inicializado, y volver a ejecutar
//!     `an init` es seguro.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::ios::Family;
use crate::workspace::{self, Project, Workspace, MARKER};

/// Los paquetes del framework que un proyecto de fuera necesita, con el
/// directorio del que salen dentro del SDK.
///
/// En este orden: `platform-native` importa `StackView` de `primitives`, así
/// que necesita sus `.d.ts` ya escritos para compilar.
const PAQUETES: [(&str, &str); 2] = [
    ("@angular-native/primitives", "primitives"),
    ("@angular-native/platform", "platform-native"),
];

pub fn init(dir: Option<&str>, name: Option<&str>, id: Option<&str>, force: bool) -> Result<()> {
    let root = match dir {
        Some(dir) => workspace::absoluta(dir),
        None => std::env::current_dir().context("no se pudo leer el directorio actual")?,
    };
    let sdk = workspace::sdk_root()?;

    // ---- Todo lo que puede decir que no, antes de tocar nada -------------
    comprobar_angular(&root)?;
    let ya = root.join(MARKER).is_file();
    if ya && !force {
        eprintln!("==> {} ya está inicializado; solo se añade lo que falte", root.display());
    }

    let previo = ya.then(|| Project::read(&root)).transpose()?;
    let app_name = match (name, &previo) {
        (Some(name), _) => name.to_owned(),
        (None, Some(previo)) => previo.name.clone(),
        (None, None) => nombre_de_app(&root)?,
    };
    let bundle_id = match (id, &previo) {
        (Some(id), _) => id.to_owned(),
        (None, Some(previo)) => previo.bundle_id.clone(),
        (None, None) => format!("dev.angularnative.{}", slug(&app_name)),
    };
    comprobar_identificador(&bundle_id)?;
    let entry = previo
        .as_ref()
        .map(|previo| previo.entry.clone())
        .unwrap_or_else(|| PathBuf::from("src/main.native.ts"));
    let plataformas = previo.map(|previo| previo.platforms).unwrap_or_default();

    eprintln!("==> proyecto Angular en {}", root.display());
    eprintln!("==> SDK de angular-native en {}", sdk.display());

    // ---- Dependencias ----------------------------------------------------
    asegurar_compilador(&root)?;
    instalar_paquetes(&sdk, &root, force)?;

    // ---- Ficheros --------------------------------------------------------
    escribir(&root.join(".angular-native/tsconfig.json"), &tsconfig(&entry), force)?;
    escribir(&root.join(&entry), PUNTO_DE_ENTRADA, false)?;
    escribir(&root.join("src/app/app-native.ts"), &componente_raiz(&app_name), false)?;
    ignorar(&root)?;

    // Y el manifiesto el último: es lo que hace que el proyecto cuente como
    // inicializado.
    escribir_manifiesto(&root, &app_name, &bundle_id, &entry, &plataformas)?;

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
/// Crea lo único que un proyecto necesita tener suyo de cada plataforma: el
/// fichero de configuración nativo. El resto —el `.app`, el APK— es producto
/// del build, se rehace entero en cada compilación y vive en
/// `.angular-native/build`, que está en el `.gitignore`.
pub fn add(workspace: &Workspace, platform: &str) -> Result<()> {
    let project = workspace.project()?;
    let (dir, fichero, contenido) = match platform {
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
            manifiesto(workspace, &project.name)?,
        ),
        otra => bail!(
            "no sé añadir {otra:?}. `an add` conoce ios, tvos, visionos y android; \
             las demás plataformas todavía no tienen nada que el proyecto deba guardar."
        ),
    };

    let path = project.root.join(dir).join(fichero);
    if path.is_file() {
        eprintln!("==> {} ya existe; no se toca", path.display());
    } else {
        escribir(&path, &contenido, false)?;
    }
    registrar_plataforma(&project.root, platform)?;
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
// Comprobaciones
// ---------------------------------------------------------------------------

fn comprobar_angular(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("{} no existe", root.display());
    }
    let mut faltan: Vec<&str> = Vec::new();
    if !root.join("angular.json").is_file() {
        faltan.push("angular.json");
    }
    if !workspace::depende_de_angular(root) {
        faltan.push("@angular/core en las dependencias del package.json");
    }
    if faltan.is_empty() {
        return Ok(());
    }
    bail!(
        "{} no parece un proyecto Angular: falta {}.\n\
         `an init` se ejecuta dentro de un proyecto ya creado; si aún no lo tienes:\n\
         \x20   npx @angular/cli new mi-app",
        root.display(),
        faltan.join(" y ")
    )
}

/// Un identificador de paquete que iOS y Android acepten. Los dos son
/// exigentes y ninguno de los dos se queja pronto: Android falla al instalar y
/// iOS al firmar, media hora después.
fn comprobar_identificador(id: &str) -> Result<()> {
    let partes: Vec<&str> = id.split('.').collect();
    let valido = partes.len() >= 2
        && partes.iter().all(|parte| {
            parte.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && parte.chars().all(|c| c.is_ascii_alphanumeric())
        });
    if !valido {
        bail!(
            "{id:?} no vale como identificador de app: tienen que ser al menos dos tramos \
             separados por puntos, cada uno empezando por letra y sin guiones ni subrayados. \
             Por ejemplo: com.ejemplo.miapp"
        );
    }
    Ok(())
}

/// `ngc` es quien compila las plantillas, y es del proyecto, no del SDK: así la
/// app se compila contra la versión de Angular que el usuario tiene instalada.
///
/// Se busca subiendo por los directorios, que es lo que hará `npx` cuando llegue
/// el momento de ejecutarlo. En un monorepo con workspaces los binarios están
/// arriba y no en el paquete, y exigirlo aquí abajo sería instalar una segunda
/// copia sin necesidad.
fn asegurar_compilador(root: &Path) -> Result<()> {
    if hay_ngc(root) {
        return Ok(());
    }
    eprintln!("==> falta @angular/compiler-cli; instalándolo");
    npm(root, &["install", "--save-dev", "--no-audit", "--no-fund", "@angular/compiler-cli"])
        .context("no se pudo instalar @angular/compiler-cli")?;
    if !hay_ngc(root) {
        bail!("npm terminó bien pero sigue sin haber un node_modules/.bin/ngc alcanzable");
    }
    Ok(())
}

fn hay_ngc(desde: &Path) -> bool {
    let mut dir = desde.to_owned();
    loop {
        if dir.join("node_modules/.bin/ngc").exists() {
            return true;
        }
        if !dir.pop() {
            return false;
        }
    }
}

/// Deja `@angular-native/primitives` y `@angular-native/platform` instalados en
/// el proyecto.
///
/// Los dos son paquetes de este repositorio y todavía no están publicados en
/// npm. De las tres maneras de meterlos en un proyecto de fuera:
///
///   · una **ruta local** (`file:../angular-native/packages/primitives`) es la
///     más cómoda de escribir y la peor de todas: npm la instala como enlace
///     simbólico, la ruta es la del disco de quien ejecutó `an init`, y el
///     `package-lock.json` que se commitea no le sirve a nadie más.
///   · un **tarball vendorizado** —`npm pack`, dentro del proyecto, con
///     versión— es una copia exacta de lo que el SDK tenía ese día. Se commitea
///     con el proyecto, `npm ci` lo reinstala sin red y sin el SDK delante, y
///     el build no se puede desincronizar del framework sin que alguien lo vea
///     en un diff.
///   · **publicarlos en npm** es lo que hará falta el día que esto salga del
///     cajón, y entonces solo cambia el especificador: en `node_modules` queda
///     exactamente lo mismo, así que nada de lo que hay por encima se entera.
///
/// Elegido el segundo. Y lo que se empaqueta no son las fuentes: son los dos
/// paquetes **compilados**, con sus `.d.ts` y en modo parcial, que es como se
/// publica cualquier librería de Angular. No es un capricho de pureza —
/// TypeScript no emite JavaScript para las fuentes que encuentra bajo
/// `node_modules`, las da por librería externa ya compilada, así que meter ahí
/// los `.ts` produce un bundle al que le falta medio framework y ni `ngc` ni
/// esbuild dicen nada—. Compilarlos primero convierte ese silencio en el caso
/// normal: se resuelven como cualquier otra dependencia y el Angular Linker de
/// `scripts/bundle.mjs` hace el resto.
///
/// Se compilan con el `ngc` del proyecto, no con el del SDK: así los `.d.ts` y
/// las declaraciones parciales salen de la misma versión de Angular contra la
/// que se compila la app.
fn instalar_paquetes(sdk: &Path, root: &Path, force: bool) -> Result<()> {
    let instalados = PAQUETES
        .iter()
        .all(|(nombre, _)| root.join("node_modules").join(nombre).join("package.json").is_file());
    if instalados && !force {
        eprintln!("==> los paquetes del framework ya están instalados");
        return Ok(());
    }

    let vendor = root.join(".angular-native/vendor");
    std::fs::create_dir_all(&vendor)?;
    let staging = root.join(".angular-native/build/packages");
    let mut tarballs: Vec<String> = Vec::new();
    for (nombre, corto) in PAQUETES {
        let origen = sdk.join("packages").join(corto);
        if !origen.join("src/public-api.ts").is_file() {
            bail!("el SDK no trae {nombre}: falta {}", origen.join("src/public-api.ts").display());
        }
        let destino = staging.join(corto);
        let _ = std::fs::remove_dir_all(&destino);
        std::fs::create_dir_all(&destino)?;

        eprintln!("==> compilando {nombre}");
        std::fs::write(destino.join("tsconfig.json"), tsconfig_paquete(&origen, &staging))?;
        npm(root, &["exec", "--", "ngc", "-p", &destino.join("tsconfig.json").to_string_lossy()])
            .with_context(|| format!("no se pudo compilar {nombre}"))?;
        let api = destino.join("dist/public-api.js");
        if !api.is_file() {
            bail!("`ngc` terminó bien pero no dejó {}", api.display());
        }
        std::fs::write(destino.join("package.json"), package_json(nombre, &origen)?)?;

        // `npm pack` escribe el nombre del fichero por la salida estándar, y es
        // el único sitio donde está la versión: componerlo aquí a mano sería
        // adivinar.
        let salida = capturar(
            root,
            "npm",
            &[
                "pack",
                &destino.to_string_lossy(),
                "--pack-destination",
                &vendor.to_string_lossy(),
                "--silent",
            ],
        )
        .with_context(|| format!("no se pudo empaquetar {nombre}"))?;
        let fichero = salida
            .lines()
            .rfind(|linea| linea.trim().ends_with(".tgz"))
            .with_context(|| format!("npm pack no dijo qué fichero escribió para {nombre}"))?
            .trim()
            .to_owned();
        if !vendor.join(&fichero).is_file() {
            bail!("npm pack dijo haber escrito {fichero}, pero no está en {}", vendor.display());
        }
        // Relativa: es lo que acaba en el `package.json` del usuario, y una ruta
        // absoluta solo valdría en esta máquina.
        tarballs.push(format!("file:.angular-native/vendor/{fichero}"));
    }

    eprintln!("==> npm install {}", tarballs.join(" "));
    let mut args: Vec<&str> = vec!["install", "--save", "--save-exact", "--no-audit", "--no-fund"];
    args.extend(tarballs.iter().map(String::as_str));
    npm(root, &args).context("no se pudieron instalar los paquetes del framework")?;

    for (nombre, _) in PAQUETES {
        let dir = root.join("node_modules").join(nombre);
        if !dir.join("package.json").is_file() {
            bail!("npm terminó bien pero {nombre} no está instalado");
        }
    }
    Ok(())
}

/// El tsconfig con el que se compila uno de los paquetes del framework.
///
/// `compilationMode: partial` es lo que hace que esto sea una librería
/// publicable: los decoradores quedan como declaraciones `ɵɵngDeclare*` que
/// resuelve el Angular Linker al empaquetar, en vez de código atado a la
/// versión exacta del compilador.
fn tsconfig_paquete(origen: &Path, staging: &Path) -> String {
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
        src = origen.join("src").display(),
        api = origen.join("src/public-api.ts").display(),
        primitives = primitives.display()
    )
}

/// El `package.json` del paquete empaquetado. La versión y los peers salen del
/// que hay en el SDK: son el mismo paquete, solo que compilado.
fn package_json(nombre: &str, origen: &Path) -> Result<String> {
    let manifest = origen.join("package.json");
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
  "name": "{nombre}",
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
// Ficheros
// ---------------------------------------------------------------------------

/// Escribe un fichero. Si ya existe y no se pidió reescribirlo, se deja y se
/// dice.
fn escribir(path: &Path, contenido: &str, force: bool) -> Result<()> {
    let corta = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if path.is_file() && !force {
        eprintln!("    ya estaba  {corta}  ({})", path.display());
        return Ok(());
    }
    if let Some(padre) = path.parent() {
        std::fs::create_dir_all(padre)?;
    }
    std::fs::write(path, contenido)
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    eprintln!("    escrito     {corta}  ({})", path.display());
    Ok(())
}

fn escribir_manifiesto(
    root: &Path,
    name: &str,
    bundle_id: &str,
    entry: &Path,
    platforms: &[String],
) -> Result<()> {
    let plataformas = platforms
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let contenido = format!(
        r#"{{
  "app": {{
    "name": "{name}",
    "bundleId": "{bundle_id}",
    "entry": "{entry}"
  }},
  "platforms": [{plataformas}]
}}
"#,
        entry = entry.display()
    );
    escribir(&root.join(MARKER), &contenido, true)
}

fn registrar_plataforma(root: &Path, platform: &str) -> Result<()> {
    let project = Project::read(root)?;
    if project.platforms.iter().any(|p| p == platform) {
        return Ok(());
    }
    let mut platforms = project.platforms.clone();
    platforms.push(platform.to_owned());
    platforms.sort();
    escribir_manifiesto(root, &project.name, &project.bundle_id, &project.entry, &platforms)
}

/// Añade al `.gitignore` lo único que angular-native genera y no es fuente.
///
/// Solo `.angular-native/build`. El tsconfig y los tarballs de
/// `.angular-native/vendor` sí se commitean: el primero es configuración y el
/// segundo es lo que hace que `npm ci` reinstale exactamente los mismos
/// paquetes del framework en la máquina de al lado.
fn ignorar(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    let actual = std::fs::read_to_string(&path).unwrap_or_default();
    let linea = "/.angular-native/build/";
    if actual.lines().any(|l| l.trim() == linea) {
        eprintln!("    ya estaba  .gitignore");
        return Ok(());
    }
    let mut nuevo = actual;
    if !nuevo.is_empty() && !nuevo.ends_with('\n') {
        nuevo.push('\n');
    }
    nuevo.push_str(&format!(
        "\n# angular-native: el .app, el APK y el JS compilado se rehacen enteros.\n{linea}\n"
    ));
    std::fs::write(&path, nuevo)
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    eprintln!("    ampliado    .gitignore");
    Ok(())
}

// ---------------------------------------------------------------------------
// Plantillas
// ---------------------------------------------------------------------------

/// El tsconfig del build nativo.
///
/// No es el del proyecto web y no puede serlo: aquí no hay DOM, la app entra
/// por otro fichero, y `paths` tiene que mandar los dos paquetes del framework
/// a su TypeScript y no al `main` del `package.json`.
///
/// Los dos paquetes del framework no llevan `paths`: están instalados y
/// compilados en `node_modules`, y se resuelven como cualquier otra
/// dependencia.
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

const PUNTO_DE_ENTRADA: &str = r#"// El arranque de la app nativa. El de la web sigue siendo src/main.ts.
import { bootstrapNativeApplication } from '@angular-native/platform'

import { AppNative } from './app/app-native'

bootstrapNativeApplication(AppNative).catch((error) => {
  console.error('el arranque falló:', error)
})
"#;

fn componente_raiz(name: &str) -> String {
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

/// El `Info.plist` del proyecto, sacado del que usa el shell.
///
/// Se parte del del SDK para no tener dos copias de la misma lista de claves,
/// y solo se cambian las tres que identifican a la app. Si el del SDK deja de
/// llevar los valores que se esperan, se para: un plist a medio sustituir
/// produce una app que se instala y no abre.
fn plist(
    workspace: &Workspace,
    family: Family,
    name: &str,
    bundle_id: &str,
) -> Result<String> {
    // El nombre y el identificador que lleva el plist del shell son los del
    // monorepo con el adorno de la familia puesto; los del proyecto se adornan
    // igual, y así el que sale de aquí ya pasa la comprobación que hace el
    // build. Los dos sufijos salen del mismo sitio, `Family::suffix`, para que
    // no puedan separarse.
    let (name_suffix, id_suffix) = family.suffix();
    let origen = workspace.root.join(match family {
        Family::Ios => "shells/ios/Resources/Info.plist",
        Family::TvOs => "shells/tvos/Resources/Info.plist",
        Family::VisionOs => "shells/visionos/Resources/Info.plist",
    });
    let texto = std::fs::read_to_string(&origen)
        .with_context(|| format!("no se pudo leer {}", origen.display()))?;
    let texto = sustituir(
        &texto,
        &origen,
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
    // El comentario va después de la declaración XML, que tiene que ir primera.
    Ok(anteponer(&texto, CABECERA_PLIST))
}

/// El `AndroidManifest.xml` del proyecto, sacado del del shell.
///
/// El `package` se queda como está —ahí viven las clases del shell— y lo que
/// cambia es la etiqueta. El identificador con el que Android instala la app
/// sale de `app.bundleId` y lo aplica `aapt2` al enlazar.
fn manifiesto(workspace: &Workspace, name: &str) -> Result<String> {
    let origen = workspace.root.join("shells/android/AndroidManifest.xml");
    let texto = std::fs::read_to_string(&origen)
        .with_context(|| format!("no se pudo leer {}", origen.display()))?;
    let texto = sustituir(
        &texto,
        &origen,
        &[("android:label=\"AngularNative\"", &format!("android:label=\"{name}\""), 1)],
    )?;
    // El comentario va después de la declaración XML, que tiene que ir primera.
    Ok(anteponer(&texto, CABECERA_MANIFIESTO))
}

/// Mete un comentario justo detrás de la declaración XML. Delante no puede ir
/// nada: un `<?xml?>` que no sea lo primero del fichero no es XML válido, y
/// `plutil` rechaza el plist entero.
fn anteponer(texto: &str, cabecera: &str) -> String {
    match texto.split_once('\n') {
        Some((declaracion, resto)) => format!("{declaracion}\n{cabecera}{resto}"),
        None => format!("{cabecera}{texto}"),
    }
}

/// Sustituye, comprobando cuántas veces tenía que aparecer cada cosa.
fn sustituir(texto: &str, origen: &Path, cambios: &[(&str, &str, usize)]) -> Result<String> {
    let mut salida = texto.to_owned();
    for (busca, pon, veces) in cambios {
        let encontradas = salida.matches(busca).count();
        if encontradas != *veces {
            bail!(
                "{}: se esperaba encontrar {busca:?} {veces} vez/veces y aparece {encontradas}. \
                 El shell ha cambiado y esta plantilla se ha quedado atrás.",
                origen.display()
            );
        }
        salida = salida.replace(busca, pon);
    }
    Ok(salida)
}

const CABECERA_PLIST: &str = "<!--\n  \
    Creado por `an add`. A partir de aquí es tuyo: `an` lo copia dentro del\n  \
    .app en cada compilación y no lo reescribe nunca.\n\n  \
    CFBundleExecutable y CFBundleIdentifier tienen que seguir coincidiendo con\n  \
    app.name y app.bundleId de angular-native.json. Si dejan de coincidir, el\n  \
    build se para y lo dice.\n-->\n";

const CABECERA_MANIFIESTO: &str = "<!--\n  \
    Creado por `an add android`. A partir de aquí es tuyo: aquí van los permisos\n  \
    y lo que la app declare.\n\n  \
    El atributo package no se cambia: es el paquete de las clases del shell. El\n  \
    identificador con el que Android instala la app sale de app.bundleId de\n  \
    angular-native.json.\n-->\n";

// ---------------------------------------------------------------------------
// Menudencias
// ---------------------------------------------------------------------------

/// El nombre de la app, sacado del `package.json`. Acaba siendo el del
/// ejecutable dentro del `.app` y el que se lee bajo el icono, así que se
/// convierte a PascalCase: `mi-app` es «MiApp».
fn nombre_de_app(root: &Path) -> Result<String> {
    let manifest = root.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("no se pudo leer {}", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} no es JSON válido", manifest.display()))?;
    let crudo = parsed
        .get("name")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: falta name", manifest.display()))?;
    let nombre: String = crudo
        .rsplit('/')
        .next()
        .unwrap_or(crudo)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|parte| !parte.is_empty())
        .map(|parte| {
            let mut chars = parte.chars();
            match chars.next() {
                Some(primera) => primera.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if nombre.is_empty() {
        bail!("{}: del name {crudo:?} no sale ningún nombre de app; pasa --name", manifest.display());
    }
    Ok(nombre)
}

fn slug(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

fn npm(cwd: &Path, args: &[&str]) -> Result<()> {
    crate::build::run_in(cwd, "npm", args, "npm falló")
}

fn capturar(cwd: &Path, program: &str, args: &[&str]) -> Result<String> {
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
