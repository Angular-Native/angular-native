//! Dónde está el SDK, dónde está el proyecto, y dónde va cada cosa.
//!
//! `an` sirve en dos sitios: dentro de este monorepo, donde la app es
//! `examples/<algo>` y todo cuelga de la misma raíz, y dentro de un proyecto
//! Angular cualquiera —uno de `ng new`— que solo tiene su `package.json` y su
//! `src/`. Los dos casos necesitan las mismas dos raíces, y son distintas:
//!
//!   · la **raíz del SDK** es de dónde salen los crates, los shells nativos y
//!     `scripts/bundle.mjs`. Es este repo, siempre.
//!   · la **raíz del proyecto** es dónde vive el código de la app y dónde se
//!     escriben los artefactos. En el monorepo coincide con la del SDK; fuera,
//!     no.
//!
//! Distinguirlas es todo el trabajo de este módulo. El resto del CLI pide
//! `workspace.root` cuando quiere el SDK y `workspace.build_dir()` cuando
//! quiere escribir, y no se entera de en cuál de los dos mundos está.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// El fichero que marca un proyecto de fuera como ya inicializado. Que exista
/// es la única señal de que `an init` terminó: se escribe el último.
pub const MARKER: &str = "angular-native.json";

/// El nombre por defecto de la app dentro del monorepo. Es el que llevan el
/// `Info.plist` del shell y el manifiesto de Android, así que en el monorepo
/// no se toca.
pub const APP_NAME_POR_DEFECTO: &str = "AngularNative";
pub const BUNDLE_ID_POR_DEFECTO: &str = "dev.angularnative.playground";

/// La configuración de un proyecto de fuera: lo que dice su
/// `angular-native.json`.
///
/// No guarda la ruta del SDK a propósito. Sería absoluta, y absoluta en el
/// disco de quien ejecutó `an init`; el fichero se commitea y el siguiente que
/// clone el repo tendría el SDK en otro sitio. La ruta del SDK la resuelve el
/// binario (ver [`sdk_root`]), que es lo único que sabe de qué máquina es.
pub struct Project {
    pub root: PathBuf,
    /// Nombre de la app: el del ejecutable dentro del `.app` y el que se ve
    /// bajo el icono.
    pub name: String,
    pub bundle_id: String,
    /// Punto de entrada nativo, relativo a la raíz del proyecto.
    pub entry: PathBuf,
    /// Las plataformas que se añadieron con `an add`.
    pub platforms: Vec<String>,
}

impl Project {
    /// El tsconfig que usa `ngc` para el build nativo. Lo escribe `an init`; no
    /// es el del proyecto web, que compila plantillas de HTML contra el DOM.
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
        let leer = |clave: &str| -> Option<String> {
            app.and_then(|app| app.get(clave)).and_then(Value::as_str).map(str::to_owned)
        };

        let name = leer("name").with_context(|| {
            format!("{}: falta app.name; vuelve a ejecutar `an init`", path.display())
        })?;
        let bundle_id = leer("bundleId").with_context(|| {
            format!("{}: falta app.bundleId; vuelve a ejecutar `an init`", path.display())
        })?;
        let entry = leer("entry").unwrap_or_else(|| "src/main.native.ts".to_owned());
        let platforms = parsed
            .get("platforms")
            .and_then(Value::as_array)
            .map(|lista| {
                lista.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>()
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
    /// Raíz del SDK: crates, shells, packages y `scripts/bundle.mjs`.
    pub root: PathBuf,
    /// El proyecto de fuera, si `an` se está ejecutando dentro de uno.
    /// `None` significa que estamos en el monorepo.
    pub project: Option<Project>,
}

impl Workspace {
    /// Sube desde el directorio actual hasta encontrar de qué mundo se trata.
    ///
    /// Gana lo más cercano: si alguien inicializa un proyecto dentro del propio
    /// monorepo —que es justo lo que hace `scripts/check-external.sh`—, el
    /// marcador está más abajo que el `Cargo.toml` y es el que manda.
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir().context("no se pudo leer el directorio actual")?;
        let mut dir = cwd.clone();
        loop {
            if dir.join(MARKER).is_file() {
                let project = Project::read(&dir)?;
                return Ok(Workspace { root: sdk_root()?, project: Some(project) });
            }
            if es_monorepo(&dir) {
                return Ok(Workspace { root: dir, project: None });
            }
            if !dir.pop() {
                break;
            }
        }
        // Nada. Antes de rendirse conviene mirar si esto es un proyecto Angular
        // sin inicializar: es el error que más veces va a salir, y decir «no
        // encuentro la raíz» cuando la respuesta es «ejecuta `an init`» manda a
        // la gente a leer el código del CLI.
        if let Some(angular) = angular_sin_inicializar(&cwd) {
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

    /// Ruta de la app.
    ///
    /// Dentro del monorepo es relativa a la raíz, como siempre. Fuera es la del
    /// proyecto, absoluta, y no hay nada que elegir: `an` trabaja sobre el
    /// proyecto en el que se ejecuta.
    pub fn app(&self, given: Option<&str>) -> Result<PathBuf> {
        if let Some(project) = &self.project {
            if let Some(given) = given {
                let pedida = absoluta(given);
                if pedida != project.root {
                    bail!(
                        "fuera del monorepo `an` trabaja sobre el proyecto en el que se ejecuta \
                         ({}), y se le ha pedido {}.\n\
                         Ejecuta `an` desde ese otro proyecto.",
                        project.root.display(),
                        pedida.display()
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

    /// El proyecto de fuera, o un error que dice qué falta. Para los comandos
    /// que solo tienen sentido ahí.
    pub fn project(&self) -> Result<&Project> {
        self.project.as_ref().context(
            "este comando es para un proyecto Angular de fuera del monorepo, \
             y `an` se está ejecutando dentro del monorepo",
        )
    }

    /// Dónde se escriben los artefactos: el `.app`, el APK, el JS compilado.
    ///
    /// En el monorepo, `build/` en la raíz, que es donde han estado siempre y
    /// donde los buscan los scripts. En un proyecto de fuera, dentro de
    /// `.angular-native/`, que va al `.gitignore`: nada de lo que hay ahí es
    /// fuente.
    pub fn build_dir(&self) -> PathBuf {
        match &self.project {
            Some(project) => project.root.join(".angular-native/build"),
            None => self.root.join("build"),
        }
    }

    /// Dónde deja `cargo` lo que compila.
    ///
    /// Casi siempre `target/` en la raíz del SDK, pero `CARGO_TARGET_DIR`
    /// existe y quien lo tiene puesto no espera que el CLI busque la librería
    /// estática donde ya no está.
    pub fn target_dir(&self) -> PathBuf {
        match std::env::var_os("CARGO_TARGET_DIR") {
            Some(dir) => absoluta(&dir.to_string_lossy()),
            None => self.root.join("target"),
        }
    }

    /// Dónde deja `ngc` el JavaScript. Es el `outDir` del tsconfig que compila
    /// la app, y de ahí sale la entrada de esbuild.
    pub fn js_dir(&self, app: &Path) -> PathBuf {
        match &self.project {
            Some(_) => self.build_dir().join("js"),
            None => self.root.join("build/js").join(Workspace::name(app)),
        }
    }

    /// El `rootDir` del tsconfig. `ngc` conserva bajo el `outDir` la estructura
    /// de directorios relativa a él, así que es lo que hay que quitarle a una
    /// ruta de fuente para saber dónde acabó su `.js`.
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
            None => APP_NAME_POR_DEFECTO.to_owned(),
        }
    }

    pub fn bundle_id(&self) -> String {
        match &self.project {
            Some(project) => project.bundle_id.clone(),
            None => BUNDLE_ID_POR_DEFECTO.to_owned(),
        }
    }

    /// Un fichero que el proyecto puede aportar para pisar al del shell —el
    /// `Info.plist`, el `AndroidManifest.xml`—, si existe.
    ///
    /// Estos son los únicos ficheros nativos que el usuario edita a mano, y por
    /// eso viven en `ios/` y `android/` y no en `.angular-native/build`: se
    /// commitean, y `an` no los reescribe nunca una vez creados.
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

fn es_monorepo(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file() && dir.join("packages/runtime/runtime.js").is_file()
}

/// La raíz del SDK cuando `an` corre fuera del monorepo.
///
/// Dos sitios, en este orden, y ninguno adivinado:
///
///   1. `AN_HOME`, para quien tiene varios checkouts o instaló el binario a
///      mano.
///   2. La ruta desde la que se compiló este binario. `cargo install --path
///      crates/an-cli` la deja grabada, así que un `an` en el PATH sabe volver
///      a su repo.
///
/// Si el sitio existe pero no tiene lo que hace falta, se dice qué falta. Un
/// SDK a medias produciría un `swiftc` sin fuentes o un bundle sin runtime, y
/// eso se descubre veinte segundos más tarde y en otro idioma.
pub fn sdk_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("AN_HOME") {
        let dir = absoluta(&dir.to_string_lossy());
        return validar_sdk(&dir).with_context(|| {
            format!("AN_HOME apunta a {}, que no es un SDK de angular-native", dir.display())
        });
    }
    let compilado = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compilado = compilado.canonicalize().unwrap_or(compilado);
    validar_sdk(&compilado).with_context(|| {
        format!(
            "este `an` se compiló desde {}, y ahí ya no está el SDK.\n\
             Define AN_HOME con la ruta del repositorio de angular-native.",
            compilado.display()
        )
    })
}

fn validar_sdk(dir: &Path) -> Result<PathBuf> {
    for necesario in ["packages/runtime/runtime.js", "scripts/bundle.mjs", "shells", "crates"] {
        if !dir.join(necesario).exists() {
            bail!("falta {necesario}");
        }
    }
    Ok(dir.to_owned())
}

/// Sube buscando un proyecto Angular: `angular.json` y `@angular/core` en las
/// dependencias. Las dos cosas, porque `angular.json` suelto también lo tiene
/// un workspace vacío y `@angular/core` suelto lo tiene una librería.
pub fn angular_sin_inicializar(desde: &Path) -> Option<PathBuf> {
    let mut dir = desde.to_owned();
    loop {
        if dir.join("angular.json").is_file() && depende_de_angular(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub fn depende_de_angular(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else { return false };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return false };
    ["dependencies", "devDependencies", "peerDependencies"].iter().any(|seccion| {
        parsed
            .get(seccion)
            .and_then(Value::as_object)
            .is_some_and(|deps| deps.contains_key("@angular/core"))
    })
}

/// Una ruta que el usuario escribió, resuelta contra el directorio actual.
pub fn absoluta(raw: &str) -> PathBuf {
    let path = Path::new(raw);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    path.canonicalize().unwrap_or(path)
}
