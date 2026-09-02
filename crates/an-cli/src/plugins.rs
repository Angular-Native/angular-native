//! Descubrimiento y enlazado de plugins.
//!
//! Un plugin es un paquete npm que además de TypeScript trae fuentes nativas.
//! Aquí no hay nada que instalar ni ningún fichero aparte que mantener: lo que
//! la app declara como dependencia es lo que se enlaza, y el manifiesto vive
//! en el `package.json` del propio plugin.
//!
//! Que esto quepa en un fichero es la ventaja de no tener `.xcodeproj` ni
//! Gradle. En Capacitor esta parte es un script que edita el proyecto de Xcode
//! y otro que escribe un `settings.gradle`; aquí es leer un JSON, añadir unas
//! rutas a la línea de `swiftc` y escribir un fichero de registro.
//!
//! Lo único que este módulo no hace nunca es callarse: un plugin que no cubre
//! la plataforma que se está compilando detiene el build. Nunca sale una app
//! con un método que se traga la llamada.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::workspace::Workspace;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Ios,
    Android,
}

impl Platform {
    /// La clave dentro de `angularNative`.
    fn key(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Android => "android",
        }
    }

    /// La extensión de sus fuentes.
    fn extension(self) -> &'static str {
        match self {
            Platform::Ios => "swift",
            Platform::Android => "java",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Ios => "iOS",
            Platform::Android => "Android",
        }
    }
}

/// La mitad nativa de un plugin para una plataforma.
pub struct Native {
    /// Directorio con las fuentes, absoluto.
    pub sources: PathBuf,
    /// El tipo que implementa `AnPlugin`. En Android, con su paquete delante.
    pub register: String,
    /// Lo que aporta al manifiesto de su plataforma.
    pub contributes: Contributions,
}

/// Lo que un plugin aporta al manifiesto de la plataforma.
///
/// No comparten forma porque no son la misma cosa: una clave del `Info.plist`
/// tiene valor y no tiene nombre de elemento, y un `<uses-permission>` es al
/// revés. Un tipo común obligaría a inventar una traducción entre los dos, y
/// esa traducción sería mentira en las dos direcciones.
pub enum Contributions {
    /// `angularNative.ios.plist`: clave del `Info.plist` a valor.
    Plist(BTreeMap<String, Value>),
    /// `angularNative.android.manifest`.
    Manifest(ManifestEntries),
}

/// Los elementos que un plugin mete en el `AndroidManifest.xml`.
#[derive(Default)]
pub struct ManifestEntries {
    /// `<uses-permission android:name="…"/>`. Es un conjunto: pedir dos veces
    /// el mismo permiso es pedirlo una.
    pub permissions: BTreeSet<String>,
    /// `<uses-feature android:name="…" android:required="…"/>`, del nombre al
    /// `required`. Aquí sí puede haber choque: dos plugins que pidan la misma
    /// característica, uno obligatoria y otro no, no dicen lo mismo.
    pub features: BTreeMap<String, bool>,
}

impl ManifestEntries {
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty() && self.features.is_empty()
    }
}

/// Una entrada del manifiesto con el paquete que la pidió.
///
/// El dueño no es un adorno: cuando dos plugins piden lo mismo con valores
/// distintos, lo único que sirve para arreglarlo es saber cuáles son.
pub struct Contributed {
    pub value: Value,
    pub package: String,
}

pub struct Plugin {
    /// El nombre npm, que es como lo declaró la app.
    pub package: String,
    /// Raíz del paquete, absoluta y resuelta (los enlaces de los workspaces de
    /// npm apuntan al directorio de verdad).
    pub dir: PathBuf,
    /// El nombre con el que JS lo invoca.
    pub module: String,
    /// El `.ts` que exporta su API, si la trae en fuente.
    pub entry: Option<PathBuf>,
    pub ios: Option<Native>,
    pub android: Option<Native>,
}

impl Plugin {
    fn native(&self, platform: Platform) -> Option<&Native> {
        match platform {
            Platform::Ios => self.ios.as_ref(),
            Platform::Android => self.android.as_ref(),
        }
    }

    /// Qué plataformas cubre, para enseñarlo.
    pub fn coverage(&self) -> String {
        let mut cubiertas: Vec<&str> = Vec::new();
        if self.ios.is_some() {
            cubiertas.push("ios");
        }
        if self.android.is_some() {
            cubiertas.push("android");
        }
        if cubiertas.is_empty() {
            return "ninguna".to_owned();
        }
        cubiertas.join(" + ")
    }
}

/// Los plugins de una app: sus dependencias que traen manifiesto.
///
/// El orden es el alfabético del nombre de módulo, no el del `package.json`:
/// un `HashMap` de npm no promete orden y el fichero de registro generado
/// cambiaría entre ejecuciones sin que cambie nada.
pub fn discover(workspace: &Workspace, app: &Path) -> Result<Vec<Plugin>> {
    let app_dir = workspace.root.join(app);
    let manifest = app_dir.join("package.json");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        // Una app no tiene por qué ser un paquete npm. Si no lo es, no tiene
        // dependencias, así que no tiene plugins.
        return Ok(Vec::new());
    };
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} no es JSON válido", manifest.display()))?;

    let mut por_modulo: BTreeMap<String, Plugin> = BTreeMap::new();
    let dependencies = parsed.get("dependencies").and_then(Value::as_object);
    for name in dependencies.into_iter().flatten().map(|(name, _)| name) {
        let Some(dir) = resolve_package(workspace, &app_dir, name) else {
            // No es cosa nuestra que falte una dependencia cualquiera: eso lo
            // dirá `ngc`. Solo nos interesan las que existen y son plugins.
            continue;
        };
        let Some(plugin) = read_manifest(name, &dir)? else {
            continue;
        };
        if let Some(previo) = por_modulo.insert(plugin.module.clone(), plugin) {
            let module = previo.module;
            bail!(
                "dos plugins dicen llamarse {module:?}: {} y otro. \
                 El nombre de módulo tiene que ser único en la app.",
                previo.package
            );
        }
    }
    Ok(por_modulo.into_values().collect())
}

/// Resuelve un paquete como lo haría Node: primero el `node_modules` de la
/// app, después el de la raíz —que es donde npm pone los de los workspaces—.
///
/// La raíz solo cuenta en el monorepo. Para un proyecto de fuera, el
/// `node_modules` del SDK no es un sitio del que su app pueda depender: enlazar
/// desde ahí un plugin que su `package.json` no declara sería enlazar algo que
/// en la máquina de al lado no está.
fn resolve_package(workspace: &Workspace, app_dir: &Path, name: &str) -> Option<PathBuf> {
    let mut bases: Vec<&Path> = vec![app_dir];
    if workspace.project.is_none() {
        bases.push(workspace.root.as_path());
    }
    for base in bases {
        let candidate = base.join("node_modules").join(name);
        if candidate.join("package.json").is_file() {
            // Los workspaces de npm son enlaces; interesa el directorio real,
            // que es el que está dentro del repo y el que `ngc` compila.
            return candidate.canonicalize().ok().or(Some(candidate));
        }
    }
    None
}

/// Lee el manifiesto de un paquete. `Ok(None)` si no es un plugin.
fn read_manifest(package: &str, dir: &Path) -> Result<Option<Plugin>> {
    let manifest = dir.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("no se pudo leer {}", manifest.display()))?;
    let parsed: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} no es JSON válido", manifest.display()))?;
    let Some(declared) = parsed.get("angularNative") else {
        return Ok(None);
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative tiene que ser un objeto",
            manifest.display()
        )
    })?;

    let module = declared
        .get("module")
        .and_then(Value::as_str)
        .with_context(|| format!("{}: falta angularNative.module", manifest.display()))?;
    check_module_name(module, &manifest)?;

    let entry = match declared.get("entry").and_then(Value::as_str) {
        Some(relative) => {
            let path = dir.join(relative);
            if !path.is_file() {
                bail!(
                    "{}: angularNative.entry apunta a {}, que no existe",
                    manifest.display(),
                    path.display()
                );
            }
            Some(path)
        }
        None => None,
    };

    Ok(Some(Plugin {
        package: package.to_owned(),
        dir: dir.to_owned(),
        module: module.to_owned(),
        entry,
        ios: read_native(declared.get("ios"), dir, Platform::Ios, &manifest)?,
        android: read_native(declared.get("android"), dir, Platform::Android, &manifest)?,
    }))
}

/// El nombre de módulo viaja tal cual hasta un literal de Swift y de Java. Se
/// comprueba aquí para que un nombre raro dé un error legible en vez de un
/// fallo de compilación dentro de un fichero generado.
fn check_module_name(module: &str, manifest: &Path) -> Result<()> {
    let valido = module
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && module
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valido {
        bail!(
            "{}: angularNative.module es {module:?}; tiene que empezar por minúscula \
             y llevar solo letras, cifras y guiones",
            manifest.display()
        );
    }
    Ok(())
}

fn read_native(
    declared: Option<&Value>,
    dir: &Path,
    platform: Platform,
    manifest: &Path,
) -> Result<Option<Native>> {
    let Some(declared) = declared else {
        return Ok(None);
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative.{} tiene que ser un objeto",
            manifest.display(),
            platform.key()
        )
    })?;
    let sources = declared
        .get("sources")
        .and_then(Value::as_str)
        .with_context(|| {
            format!(
                "{}: falta angularNative.{}.sources",
                manifest.display(),
                platform.key()
            )
        })?;
    let register = declared
        .get("register")
        .and_then(Value::as_str)
        .with_context(|| {
            format!(
                "{}: falta angularNative.{}.register",
                manifest.display(),
                platform.key()
            )
        })?;
    let sources = dir.join(sources);
    if !sources.is_dir() {
        bail!(
            "{}: angularNative.{}.sources apunta a {}, que no es un directorio",
            manifest.display(),
            platform.key(),
            sources.display()
        );
    }
    let contributes = match platform {
        Platform::Ios => Contributions::Plist(read_plist(declared.get("plist"), manifest)?),
        Platform::Android => {
            Contributions::Manifest(read_manifest_entries(declared.get("manifest"), manifest)?)
        }
    };
    Ok(Some(Native {
        sources,
        register: register.to_owned(),
        contributes,
    }))
}

/// `angularNative.ios.plist`: las claves que este plugin necesita en el
/// `Info.plist`.
///
/// Sin esto, un plugin de Face ID compila, se instala y mata la app la primera
/// vez que se autentica: iOS exige `NSFaceIDUsageDescription` y sin ella no
/// avisa, cierra. Que el plugin declare aquí lo que necesita es lo que impide
/// que el que lo instala tenga que saberlo.
fn read_plist(declared: Option<&Value>, manifest: &Path) -> Result<BTreeMap<String, Value>> {
    let Some(declared) = declared else {
        return Ok(BTreeMap::new());
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative.ios.plist tiene que ser un objeto",
            manifest.display()
        )
    })?;
    let mut entries = BTreeMap::new();
    for (key, value) in declared {
        // La clave viaja a `plutil -replace`, que trata el punto como
        // separador de camino: `a.b` no sería una clave llamada «a.b» sino la
        // «b» de dentro de la «a». Ninguna clave del sistema lleva punto, así
        // que se para aquí en vez de escribir en un sitio que nadie pidió.
        if key.is_empty() || key.contains('.') {
            bail!(
                "{}: angularNative.ios.plist tiene la clave {key:?};                  solo se admiten claves de primer nivel y sin puntos",
                manifest.display()
            );
        }
        if !plist_value_ok(value) {
            bail!(
                "{}: angularNative.ios.plist[{key:?}] es {value};                  solo se admiten cadenas, booleanos, números y listas de cadenas.                  Un diccionario anidado todavía no se funde. Ver docs/plugins.md.",
                manifest.display()
            );
        }
        entries.insert(key.clone(), value.clone());
    }
    Ok(entries)
}

/// Lo que se sabe escribir en el plist con `plutil -replace … -json`, y nada
/// más. Aceptar un diccionario aquí y fundirlo mal más tarde sería peor que
/// no aceptarlo.
fn plist_value_ok(value: &Value) -> bool {
    match value {
        Value::String(_) | Value::Bool(_) | Value::Number(_) => true,
        Value::Array(items) => items.iter().all(Value::is_string),
        _ => false,
    }
}

/// `angularNative.android.manifest`: permisos y características.
fn read_manifest_entries(declared: Option<&Value>, manifest: &Path) -> Result<ManifestEntries> {
    let Some(declared) = declared else {
        return Ok(ManifestEntries::default());
    };
    let declared = declared.as_object().with_context(|| {
        format!(
            "{}: angularNative.android.manifest tiene que ser un objeto",
            manifest.display()
        )
    })?;
    let mut entries = ManifestEntries::default();
    for (clave, valor) in declared {
        match clave.as_str() {
            "uses-permission" => {
                let lista = valor.as_array().with_context(|| {
                    format!(
                        "{}: angularNative.android.manifest[\"uses-permission\"]                          tiene que ser una lista de nombres",
                        manifest.display()
                    )
                })?;
                for nombre in lista {
                    let nombre = nombre.as_str().with_context(|| {
                        format!(
                            "{}: los permisos son cadenas, y hay un {nombre}",
                            manifest.display()
                        )
                    })?;
                    entries.permissions.insert(nombre.to_owned());
                }
            }
            "uses-feature" => {
                let objeto = valor.as_object().with_context(|| {
                    format!(
                        "{}: angularNative.android.manifest[\"uses-feature\"] tiene que ser                          un objeto de nombre a si es obligatoria",
                        manifest.display()
                    )
                })?;
                for (nombre, required) in objeto {
                    let required = required.as_bool().with_context(|| {
                        format!(
                            "{}: uses-feature[{nombre:?}] tiene que ser true o false,                              que es lo que vale android:required",
                            manifest.display()
                        )
                    })?;
                    entries.features.insert(nombre.clone(), required);
                }
            }
            otro => bail!(
                "{}: angularNative.android.manifest no sabe de {otro:?};                  de momento solo se aportan \"uses-permission\" y \"uses-feature\".                  Ver docs/plugins.md.",
                manifest.display()
            ),
        }
    }
    Ok(entries)
}

/// Exige que todos los plugins cubran la plataforma que se va a compilar.
///
/// Esta es la regla que da sentido al resto. Un plugin de iOS metido en un APK
/// no puede acabar en un método que devuelve `undefined` y una pantalla que no
/// hace nada: se para el build y se dice qué falta y en qué paquete.
pub fn require(plugins: &[Plugin], platform: Platform) -> Result<()> {
    let faltan: Vec<&Plugin> = plugins
        .iter()
        .filter(|plugin| plugin.native(platform).is_none())
        .collect();
    if faltan.is_empty() {
        // Lo que aportan al manifiesto se funde aquí y no al escribirlo: así
        // un choque entre dos plugins sale en `an plugins --platform ios`, sin
        // gastar medio minuto de `swiftc` para acabar diciendo lo mismo.
        match platform {
            Platform::Ios => {
                plist_entries(plugins)?;
            }
            Platform::Android => {
                manifest_entries(plugins)?;
            }
        }
        return Ok(());
    }
    let mut message = format!(
        "esta app no se puede compilar para {}: {} de sus plugins no lo cubre{}\n",
        platform.label(),
        faltan.len(),
        if faltan.len() == 1 { "" } else { "n" }
    );
    for plugin in &faltan {
        message.push_str(&format!(
            "  · {} (módulo {:?}) solo trae {}\n",
            plugin.package,
            plugin.module,
            plugin.coverage()
        ));
    }
    message.push_str(&format!(
        "\nO el plugin añade su parte de {} —fuentes en angularNative.{} de su package.json—, \
         o la app deja de depender de él.",
        platform.label(),
        platform.key()
    ));
    bail!(message)
}

/// Funde las claves del `Info.plist` que piden todos los plugins.
///
/// Dos plugins que piden la misma clave con el **mismo** valor no son un
/// problema: dicen lo mismo, y se escribe una vez. Con valores distintos no
/// hay forma honrada de elegir —quedarse con el primero por orden alfabético
/// o por orden de dependencia sería decidir en silencio qué texto le sale al
/// usuario en el diálogo del sistema—, así que se para el build.
pub fn plist_entries(plugins: &[Plugin]) -> Result<BTreeMap<String, Contributed>> {
    let mut merged: BTreeMap<String, Contributed> = BTreeMap::new();
    for plugin in plugins {
        let Some(native) = plugin.ios.as_ref() else {
            continue;
        };
        let Contributions::Plist(entries) = &native.contributes else {
            continue;
        };
        for (key, value) in entries {
            if let Some(previo) = merged.get(key) {
                if &previo.value != value {
                    bail!(choque(
                        &format!("la clave {key:?} del Info.plist"),
                        &previo.package,
                        &previo.value.to_string(),
                        &plugin.package,
                        &value.to_string(),
                    ));
                }
                continue;
            }
            merged.insert(
                key.clone(),
                Contributed {
                    value: value.clone(),
                    package: plugin.package.clone(),
                },
            );
        }
    }
    Ok(merged)
}

/// Lo mismo para el `AndroidManifest.xml`.
///
/// Los permisos son un conjunto y no pueden chocar: `USE_BIOMETRIC` pedido dos
/// veces es `USE_BIOMETRIC`. Las características sí, porque llevan valor:
/// obligatoria y opcional no son la misma petición, y la diferencia decide si
/// Google Play enseña la app en un aparato sin ese sensor.
pub fn manifest_entries(plugins: &[Plugin]) -> Result<ManifestEntries> {
    let mut merged = ManifestEntries::default();
    let mut duenos: BTreeMap<String, String> = BTreeMap::new();
    for plugin in plugins {
        let Some(native) = plugin.android.as_ref() else {
            continue;
        };
        let Contributions::Manifest(entries) = &native.contributes else {
            continue;
        };
        for permiso in &entries.permissions {
            merged.permissions.insert(permiso.clone());
        }
        for (nombre, required) in &entries.features {
            if let Some(previo) = merged.features.get(nombre) {
                if previo != required {
                    let dueno = duenos
                        .get(nombre)
                        .map(String::as_str)
                        .unwrap_or("otro plugin");
                    bail!(choque(
                        &format!("la característica {nombre:?} del AndroidManifest.xml"),
                        dueno,
                        &format!("android:required=\"{previo}\""),
                        &plugin.package,
                        &format!("android:required=\"{required}\""),
                    ));
                }
                continue;
            }
            merged.features.insert(nombre.clone(), *required);
            duenos.insert(nombre.clone(), plugin.package.clone());
        }
    }
    Ok(merged)
}

/// El mensaje de dos plugins que piden lo mismo con valores distintos.
///
/// Dice los dos paquetes y los dos valores porque es lo único con lo que se
/// puede arreglar: quien lo lee no escribió ninguno de los dos.
fn choque(que: &str, uno: &str, valor_uno: &str, otro: &str, valor_otro: &str) -> String {
    format!(
        "dos plugins piden {que} con valores distintos:\n\
         \x20 · {uno}\n\
         \x20     {valor_uno}\n\
         \x20 · {otro}\n\
         \x20     {valor_otro}\n\n\
         Solo puede quedar uno, y elegirlo por orden sería decidir en silencio \
         algo que se ve en pantalla.\n\
         O los dos plugins se ponen de acuerdo, o la app se queda con uno de los dos."
    )
}

/// Las fuentes nativas de un plugin para una plataforma, en orden estable.
pub fn sources(plugin: &Plugin, platform: Platform) -> Result<Vec<String>> {
    let native = plugin
        .native(platform)
        .with_context(|| format!("{} no cubre {}", plugin.package, platform.label()))?;
    let mut found: Vec<PathBuf> = Vec::new();
    let mut kotlin: Vec<PathBuf> = Vec::new();
    for path in walk(&native.sources) {
        match path.extension().and_then(|e| e.to_str()) {
            Some(extension) if extension == platform.extension() => found.push(path),
            Some("kt") => kotlin.push(path),
            _ => {}
        }
    }
    if !kotlin.is_empty() {
        // Aquí no hay Gradle, y sin Gradle no hay `kotlinc` que valga: el shell
        // de Android es Java y se compila con `javac` contra `android.jar`.
        // Decirlo es mejor que compilar el APK sin esos ficheros dentro.
        bail!(
            "{}: {} lleva fuentes Kotlin y todavía no se compilan; el lado Android \
             de un plugin es Java. Ver docs/plugins.md.",
            plugin.package,
            native.sources.display()
        );
    }
    if found.is_empty() {
        bail!(
            "{}: no hay ninguna fuente .{} en {}",
            plugin.package,
            platform.extension(),
            native.sources.display()
        );
    }
    found.sort();
    Ok(found
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

/// El registro que enlaza los plugins con el shell de iOS.
///
/// Se genera siempre, aunque no haya ninguno: el shell lo llama sin
/// condiciones, y un `install()` vacío es más fácil de leer que un `#if`.
pub fn generate_ios(plugins: &[Plugin], out: &Path) -> Result<PathBuf> {
    let mut code = String::from(
        "// Generado por `an` al armar el .app. No editar: se reescribe en cada build.\n\
         //\n\
         // Los nombres salen del angularNative.module del package.json de cada\n\
         // plugin, que es el único sitio donde se escriben.\n\n\
         enum AnGeneratedPlugins {\n    static func install() {\n",
    );
    if plugins.is_empty() {
        code.push_str("        // Esta app no depende de ningún plugin.\n");
    }
    for plugin in plugins {
        let native = plugin
            .native(Platform::Ios)
            .expect("require() ya lo comprobó");
        code.push_str(&format!(
            "        AnPluginRegistry.register({:?}, {}())\n",
            plugin.module, native.register
        ));
    }
    code.push_str("    }\n}\n");

    std::fs::create_dir_all(out)?;
    let path = out.join("AnGeneratedPlugins.swift");
    std::fs::write(&path, code)?;
    Ok(path)
}

/// El mismo registro, para el shell de Android.
pub fn generate_android(plugins: &[Plugin], out: &Path) -> Result<PathBuf> {
    let mut code = String::from(
        "// Generado por `an` al armar el APK. No editar: se reescribe en cada build.\n\
         //\n\
         // Los nombres salen del angularNative.module del package.json de cada\n\
         // plugin, que es el único sitio donde se escriben.\n\n\
         package dev.angularnative;\n\n\
         public final class AnGeneratedPlugins {\n\n\
         \x20   private AnGeneratedPlugins() {}\n\n\
         \x20   public static void install() {\n",
    );
    if plugins.is_empty() {
        code.push_str("        // Esta app no depende de ningún plugin.\n");
    }
    for plugin in plugins {
        let native = plugin
            .native(Platform::Android)
            .expect("require() ya lo comprobó");
        code.push_str(&format!(
            "        AnPluginRegistry.register({:?}, new {}());\n",
            plugin.module, native.register
        ));
    }
    code.push_str("    }\n}\n");

    let package_dir = out.join("dev/angularnative");
    std::fs::create_dir_all(&package_dir)?;
    let path = package_dir.join("AnGeneratedPlugins.java");
    std::fs::write(&path, code)?;
    Ok(path)
}

/// Los `--alias` de esbuild para que el import del paquete apunte al JS que
/// acaba de salir de `ngc`.
///
/// Solo para los plugins que traen su API en TypeScript dentro del repo: uno
/// publicado en npm ya viene compilado, y entonces esbuild lo resuelve por
/// `node_modules` como cualquier otra dependencia y aquí no hay nada que
/// hacer.
pub fn aliases(workspace: &Workspace, js_dir: &Path, plugins: &[Plugin]) -> Vec<String> {
    // Los directorios de los plugins vienen resueltos; la raíz puede no
    // estarlo, y entonces el prefijo no casaría.
    let root = workspace.source_root();
    plugins
        .iter()
        .filter_map(|plugin| {
            let entry = plugin.entry.as_ref()?;
            if entry.extension().and_then(|e| e.to_str()) != Some("ts") {
                return None;
            }
            // `ngc` conserva la estructura de directorios bajo el `rootDir` del
            // tsconfig, que en las apps de este repo es la raíz.
            let relative = entry.strip_prefix(&root).ok()?;
            let compiled = js_dir.join(relative).with_extension("js");
            Some(format!("--alias={}={}", plugin.package, compiled.display()))
        })
        .collect()
}

/// Lista los plugins de una app por la salida estándar.
///
/// La ruta no es un adorno: un paquete npm puede venir del repo o de
/// `node_modules`, y cuando algo no cuadra lo primero que se quiere saber es
/// cuál de los dos se enlazó.
pub fn list(workspace: &Workspace, plugins: &[Plugin]) {
    if plugins.is_empty() {
        println!("esta app no depende de ningún plugin");
        return;
    }
    let root = workspace.source_root();
    for plugin in plugins {
        let dir = plugin.dir.strip_prefix(&root).unwrap_or(&plugin.dir);
        println!(
            "{}  ({})  {}  {}",
            plugin.module,
            plugin.package,
            plugin.coverage(),
            dir.display()
        );
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}
