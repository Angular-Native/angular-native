//! Arma el APK y lo lleva al emulador.
//!
//! Sin Gradle, por el mismo motivo que iOS va sin `.xcodeproj`: las
//! herramientas del SDK ya hacen todo el trabajo y el proceso cabe en un
//! fichero que se puede leer entero. `javac` compila, `d8` dexa, `aapt2` enlaza
//! el manifiesto, y el resto es un zip firmado.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::plugins::{self, Platform, Plugin};
use crate::workspace::Workspace;

/// El paquete Java del shell. No cambia: es el de `shells/android/java`, y va
/// escrito en cada `package dev.angularnative;`. Lo que sí cambia por proyecto
/// es el identificador de la aplicación, y para eso está
/// `--rename-manifest-package`, que reescribe el manifiesto y deja las clases
/// donde estaban.
const PACKAGE: &str = "dev.angularnative";
const ACTIVITY: &str = "dev.angularnative.MainActivity";
const ABI: &str = "arm64-v8a";
const RUST_TARGET: &str = "aarch64-linux-android";

/// Teléfono o reloj. Los dos son Android y comparten host; lo que cambia
/// es el manifiesto y en qué aparato se instala.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Phone,
    Watch,
}

impl Form {
    /// El manifiesto de cada forma. No es un fichero con condicionales porque
    /// el formato no tiene ninguno: `aapt2` no sabe de variantes.
    /// El nombre suelto, para encontrar el que ponga un proyecto de fuera.
    fn manifest_name(self) -> &'static str {
        match self {
            Form::Phone => "AndroidManifest.xml",
            Form::Watch => "AndroidManifest.wear.xml",
        }
    }

    fn manifest(self) -> &'static str {
        match self {
            Form::Phone => "shells/android/AndroidManifest.xml",
            Form::Watch => "shells/android/AndroidManifest.wear.xml",
        }
    }

    /// Cómo se llama esto cuando hay que decirlo por pantalla.
    pub fn nombre(self) -> &'static str {
        match self {
            Form::Phone => "teléfono",
            Form::Watch => "reloj",
        }
    }
}
pub struct Sdk {
    pub root: PathBuf,
    pub build_tools: PathBuf,
    pub android_jar: PathBuf,
}

impl Sdk {
    pub fn discover() -> Result<Self> {
        let root = std::env::var("ANDROID_HOME")
            .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Android/sdk")
            });
        if !root.is_dir() {
            bail!("no encuentro el SDK de Android; define ANDROID_HOME");
        }
        let build_tools = newest_dir(&root.join("build-tools"))
            .context("el SDK no tiene build-tools instaladas")?;
        let platform = newest_dir(&root.join("platforms"))
            .context("el SDK no tiene ninguna plataforma instalada")?;
        let android_jar = platform.join("android.jar");
        if !android_jar.is_file() {
            bail!("no encuentro android.jar en {}", platform.display());
        }
        Ok(Sdk {
            root,
            build_tools,
            android_jar,
        })
    }

    fn tool(&self, name: &str) -> PathBuf {
        self.build_tools.join(name)
    }

    pub fn adb(&self) -> PathBuf {
        self.root.join("platform-tools/adb")
    }
}

/// Devuelve el subdirectorio de nombre más alto, que es la versión más nueva.
fn newest_dir(parent: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    entries.pop()
}

/// Desde el emulador, la máquina anfitriona no es `localhost`.
pub const EMULATOR_HOST: &str = "10.0.2.2";

pub fn assemble(
    workspace: &Workspace,
    bundle: &Path,
    release: bool,
    dev_server: Option<&str>,
    plugins: &[Plugin],
    form: Form,
) -> Result<PathBuf> {
    // Antes de compilar nada: si algún plugin no trae su parte de Android, el
    // build se para aquí y dice cuál.
    plugins::require(plugins, Platform::Android)?;
    let sdk = Sdk::discover()?;
    let root = &workspace.root;
    let profile = if release { "release" } else { "debug" };
    let app_name = workspace.app_name();
    let application_id = application_id(workspace);
    let out = workspace.build_dir().join("android");
    let staging = out.join("apk");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("assets"))?;
    std::fs::create_dir_all(staging.join(format!("lib/{ABI}")))?;

    eprintln!("==> core Rust ({profile}, {ABI})");
    let mut cargo_args = vec!["build", "--target", RUST_TARGET, "-p", "an-android"];
    if release {
        cargo_args.push("--release");
    }
    let status = Command::new("cargo")
        .args(&cargo_args)
        .current_dir(root)
        .status()
        .context("no se pudo ejecutar cargo")?;
    if !status.success() {
        bail!("la compilación del core para Android falló");
    }
    std::fs::copy(
        workspace
            .target_dir()
            .join(RUST_TARGET)
            .join(profile)
            .join("liban_android.so"),
        staging.join(format!("lib/{ABI}/liban_android.so")),
    )?;
    std::fs::copy(bundle, staging.join("assets/main.js"))?;
    // Los iconos de Material. Van en la app y no en la plataforma porque el
    // juego que trae Android —`android.R.drawable`— está congelado desde 2011
    // por compatibilidad: no es el de Material 3 ni se parece.
    for asset in ["material-symbols.ttf", "material-symbols.codepoints"] {
        std::fs::copy(
            root.join("shells/android/assets").join(asset),
            staging.join("assets").join(asset),
        )
        .with_context(|| format!("no se pudo copiar {asset}"))?;
    }
    if let Some(url) = dev_server {
        std::fs::write(staging.join("assets/dev-server.txt"), url)?;
    }

    // Las librerías de Android —Material y todo lo que arrastra— vienen
    // resueltas y con sus recursos ya compilados por
    // `scripts/prepare-android-deps.py`. Aquí solo se leen las listas.
    let vendor = root.join("vendor/android/build");
    let leer_lista = |nombre: &str| -> Vec<String> {
        std::fs::read_to_string(vendor.join(nombre))
            .unwrap_or_default()
            .lines()
            .filter(|linea| !linea.trim().is_empty())
            .map(str::to_owned)
            .collect()
    };
    let jars = leer_lista("classpath.txt");
    let recursos = leer_lista("resources.txt");
    let paquetes = leer_lista("packages.txt");
    if jars.is_empty() {
        bail!(
            "faltan las dependencias de Android; ejecuta \
             python3 scripts/prepare-android-deps.py"
        );
    }

    // Los recursos propios del shell —el tema de la app— se compilan aquí y no
    // en `prepare-android-deps.py` porque son nuestros y cambian; los de las
    // librerías no cambian nunca y por eso aquellos se cachean.
    eprintln!("==> aapt2 compile (recursos del shell)");
    let app_res = out.join("shell-res.zip");
    let _ = std::fs::remove_file(&app_res);
    run(
        root,
        &sdk.tool("aapt2").to_string_lossy(),
        &[
            "compile",
            "--dir",
            &root.join("shells/android/res").to_string_lossy(),
            "-o",
            &app_res.to_string_lossy(),
        ],
        "aapt2 compile de los recursos del shell falló",
    )?;

    // El enlace de recursos va antes que `javac`: de aquí salen las clases
    // `R` que las librerías necesitan para encontrar sus propios recursos.
    eprintln!("==> aapt2 link");
    let unsigned = out.join("unsigned.apk");
    let generado = out.join("gen");
    let _ = std::fs::remove_dir_all(&generado);
    std::fs::create_dir_all(&generado)?;
    // El manifiesto del proyecto pisa al del shell si lo hay: es lo que escribe
    // `an add android`, y a partir de ahí es del usuario. Tiene que seguir
    // declarando el paquete del shell, porque ahí están las clases; el
    // identificador de la aplicación lo pone `--rename-manifest-package`.
    let manifest = workspace
        .overlay("android", form.manifest_name())
        .unwrap_or_else(|| root.join(form.manifest()));
    comprobar_manifiesto(&manifest)?;
    // Y el que se le pasa a `aapt2` es el del proyecto más lo que piden los
    // plugins. El original no se toca: es del usuario.
    let manifest =
        escribir_manifiesto(&manifest, &out.join("AndroidManifest.merged.xml"), plugins)?;
    let mut link: Vec<String> = vec![
        "link".into(),
        "-I".into(),
        sdk.android_jar.to_string_lossy().into_owned(),
        "--manifest".into(),
        manifest.to_string_lossy().into_owned(),
        "--java".into(),
        generado.to_string_lossy().into_owned(),
        // Cada librería quiere su propia clase `R`, y sus identificadores no
        // pueden ser constantes: se resuelven al enlazar la app.
        "--extra-packages".into(),
        paquetes.join(":"),
        "--non-final-ids".into(),
        // Los recursos de las librerías se solapan a propósito —unas
        // redefinen estilos de otras— y sin esto aapt2 lo toma por un error.
        "--auto-add-overlay".into(),
        "-o".into(),
        unsigned.to_string_lossy().into_owned(),
    ];
    // En el monorepo el identificador ya es el del manifiesto y no hay nada que
    // renombrar; renombrarlo igualmente cambiaría el paquete instalado sin que
    // nadie lo haya pedido.
    if application_id != PACKAGE {
        link.push("--rename-manifest-package".into());
        link.push(application_id.clone());
    }
    for recurso in &recursos {
        link.push("-R".into());
        link.push(recurso.clone());
    }
    // Los del shell van los últimos: `--auto-add-overlay` deja que lo de aquí
    // redefina lo que traigan las librerías, y no al revés.
    link.push("-R".into());
    link.push(app_res.to_string_lossy().into_owned());
    run(
        root,
        &sdk.tool("aapt2").to_string_lossy(),
        &link,
        "aapt2 link falló",
    )?;

    eprintln!("==> shell Java");
    let classes = out.join("classes");
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::create_dir_all(&classes)?;
    let mut sources: Vec<String> =
        std::fs::read_dir(root.join("shells/android/java/dev/angularnative"))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "java"))
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
    if sources.is_empty() {
        bail!("no hay fuentes Java en shells/android");
    }
    // Los plugins: sus fuentes Java y el registro que las engancha. Todo entra
    // en la misma invocación de `javac` que el shell, así que un plugin ve
    // `AnPlugin` y `AnPluginCall` sin classpath adicional.
    for plugin in plugins {
        let aportadas = plugins::sources(plugin, Platform::Android)?;
        eprintln!(
            "==> plugin {} ({} fuentes Java)",
            plugin.module,
            aportadas.len()
        );
        sources.extend(aportadas);
    }
    sources.push(
        plugins::generate_android(plugins, &out.join("gen-plugins"))?
            .to_string_lossy()
            .into_owned(),
    );
    // `--release` en vez de `-source/-target`: con los modernos, javac
    // rechaza `-bootclasspath`, y aquí hace falta compilar contra android.jar
    // y no contra el JDK.
    let mut classpath = vec![sdk.android_jar.to_string_lossy().into_owned()];
    classpath.extend(jars.iter().cloned());
    let mut javac: Vec<String> = vec![
        "-nowarn".into(),
        "-source".into(),
        "17".into(),
        "-target".into(),
        "17".into(),
        "-classpath".into(),
        classpath.join(":"),
        "-d".into(),
        classes.to_string_lossy().into_owned(),
    ];
    javac.extend(sources);
    // Las clases `R` que acaba de escribir aapt2, una por paquete.
    javac.extend(
        walk(&generado)
            .into_iter()
            .filter(|path| path.extension().is_some_and(|e| e == "java"))
            .map(|path| path.to_string_lossy().into_owned()),
    );
    run(root, "javac", &javac, "la compilación del shell Java falló")?;

    eprintln!("==> d8");
    let class_files: Vec<String> = walk(&classes)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "class"))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let mut d8: Vec<String> = vec![
        "--min-api".into(),
        "24".into(),
        "--lib".into(),
        sdk.android_jar.to_string_lossy().into_owned(),
        "--output".into(),
        staging.to_string_lossy().into_owned(),
    ];
    if release {
        d8.push("--release".into());
    }
    d8.extend(class_files);
    // Y las librerías: sus clases tienen que acabar en el mismo dex.
    d8.extend(jars.iter().cloned());
    run(root, &sdk.tool("d8").to_string_lossy(), &d8, "d8 falló")?;

    eprintln!("==> firma");
    // `aapt2` solo mete el manifiesto: el dex, la biblioteca nativa y los
    // assets se añaden al zip después, con las rutas que espera Android.
    // Con las librerías de Material dentro, `d8` parte el dex en varios:
    // `classes.dex`, `classes2.dex`… Desde API 21 Android los carga todos, pero
    // hay que meterlos todos en el zip.
    let mut dexes: Vec<String> = std::fs::read_dir(&staging)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|nombre| nombre.starts_with("classes") && nombre.ends_with(".dex"))
        .collect();
    dexes.sort();
    if dexes.is_empty() {
        bail!("d8 no dejó ningún .dex");
    }
    let mut entries = dexes;
    entries.extend([
        format!("lib/{ABI}/liban_android.so"),
        "assets/main.js".to_owned(),
        "assets/material-symbols.ttf".to_owned(),
        "assets/material-symbols.codepoints".to_owned(),
    ]);
    if dev_server.is_some() {
        entries.push("assets/dev-server.txt".to_owned());
    }
    let mut zip_args: Vec<String> = vec![
        "-q".into(),
        "-X".into(),
        unsigned.to_string_lossy().into_owned(),
    ];
    zip_args.extend(entries.iter().cloned());
    let status = Command::new("zip")
        .args(&zip_args)
        .current_dir(&staging)
        .status()
        .context("no se pudo ejecutar zip")?;
    if !status.success() {
        bail!("no se pudo empaquetar el APK");
    }

    let aligned = out.join("aligned.apk");
    let _ = std::fs::remove_file(&aligned);
    run(
        root,
        &sdk.tool("zipalign").to_string_lossy(),
        &[
            "-p",
            "4",
            &unsigned.to_string_lossy(),
            &aligned.to_string_lossy(),
        ],
        "zipalign falló",
    )?;

    let keystore = debug_keystore()?;
    // Un nombre por forma: los dos APK llevan el mismo paquete, y si
    // compartieran fichero, armar el del reloj dejaría al del teléfono
    // apuntando a un APK que ya no es el suyo.
    let apk = out.join(match form {
        Form::Phone => format!("{app_name}.apk"),
        Form::Watch => format!("{app_name}-wear.apk"),
    });
    let _ = std::fs::remove_file(&apk);
    run(
        root,
        &sdk.tool("apksigner").to_string_lossy(),
        &[
            "sign",
            "--ks",
            &keystore.to_string_lossy(),
            "--ks-pass",
            "pass:android",
            "--key-pass",
            "pass:android",
            "--ks-key-alias",
            "androiddebugkey",
            "--out",
            &apk.to_string_lossy(),
            &aligned.to_string_lossy(),
        ],
        "la firma falló",
    )?;

    let size = std::fs::metadata(&apk)?.len();
    eprintln!("==> {} MB en {}", size / (1024 * 1024), apk.display());
    Ok(apk)
}

/// El identificador con el que Android instala la app.
///
/// En un proyecto de fuera es el `app.bundleId`, el mismo que en iOS. En el
/// monorepo es el paquete del shell tal cual: aquí no hay proyecto que
/// consultar y cambiarlo movería de sitio la app de ejemplo que ya está
/// instalada en el emulador de todo el mundo.
fn application_id(workspace: &Workspace) -> String {
    match &workspace.project {
        Some(project) => project.bundle_id.clone(),
        None => PACKAGE.to_owned(),
    }
}

/// Escribe el manifiesto que ve `aapt2`: el del proyecto más los permisos y
/// las características que piden los plugins.
///
/// El fichero de partida no se toca nunca. Es del usuario —lo escribe `an add
/// android` y a partir de ahí es suyo—, y un build que edita fuentes deja al
/// siguiente sin saber qué escribió él y qué escribió la herramienta.
///
/// Lo que ya declare la app no se repite: `aapt2` acepta dos
/// `<uses-permission>` iguales, pero un manifiesto con la misma línea dos
/// veces es un manifiesto que nadie sabe leer.
fn escribir_manifiesto(base: &Path, destino: &Path, plugins: &[Plugin]) -> Result<PathBuf> {
    let entradas = plugins::manifest_entries(plugins)?;
    let texto = std::fs::read_to_string(base)
        .with_context(|| format!("no se pudo leer {}", base.display()))?;
    if entradas.is_empty() {
        return Ok(base.to_owned());
    }

    let mut lineas = String::new();
    let ya_permisos = nombres_declarados(&texto, "uses-permission");
    for permiso in &entradas.permissions {
        if ya_permisos.contains_key(permiso) {
            // La app ya lo pide. No hay nada que decidir: un permiso no tiene
            // valor, así que pedirlo dos veces es pedirlo una.
            continue;
        }
        eprintln!("==> AndroidManifest.xml: uses-permission {permiso}");
        lineas.push_str(&format!(
            "    <uses-permission android:name=\"{permiso}\" />\n"
        ));
    }
    let ya_features = nombres_declarados(&texto, "uses-feature");
    for (nombre, required) in &entradas.features {
        if let Some(actual) = ya_features.get(nombre) {
            let declarado = actual.as_deref().unwrap_or("true");
            if declarado != required.to_string() {
                eprintln!(
                    "==> AndroidManifest.xml: {nombre} ya la declara la app con \
                     android:required=\"{declarado}\"; se queda la suya"
                );
            }
            continue;
        }
        eprintln!("==> AndroidManifest.xml: uses-feature {nombre} (required={required})");
        lineas.push_str(&format!(
            "    <uses-feature android:name=\"{nombre}\" android:required=\"{required}\" />\n"
        ));
    }

    let salida = if lineas.is_empty() {
        texto
    } else {
        // Delante de `<application>`, que es donde van en cualquier manifiesto
        // y donde el que lo abra los va a buscar.
        let corte = texto.find("<application").with_context(|| {
            format!(
                "{}: no encuentro <application>, y ahí es donde van los permisos",
                base.display()
            )
        })?;
        // Hasta el principio de su línea, para no partir la sangría.
        let corte = texto[..corte]
            .rfind('\n')
            .map(|salto| salto + 1)
            .unwrap_or(corte);
        format!(
            "{}    <!-- De los plugins. Lo escribe `an` al armar el APK. -->\n{}\n{}",
            &texto[..corte],
            lineas.trim_end(),
            &texto[corte..]
        )
    };
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre)?;
    }
    std::fs::write(destino, salida)
        .with_context(|| format!("no se pudo escribir {}", destino.display()))?;
    Ok(destino.to_owned())
}

/// Los `android:name` de un tipo de elemento del manifiesto, con su
/// `android:required` si lo lleva.
///
/// Se busca elemento a elemento y no por texto suelto: `contains` sobre la
/// línea entera fallaría en cuanto alguien pusiera los atributos en otro orden
/// o partiera el elemento en varias líneas, y el fallo sería un permiso
/// repetido, no un error.
fn nombres_declarados(texto: &str, elemento: &str) -> BTreeMap<String, Option<String>> {
    let mut encontrados = BTreeMap::new();
    let abre = format!("<{elemento}");
    let mut resto = texto;
    while let Some(inicio) = resto.find(&abre) {
        resto = &resto[inicio + abre.len()..];
        let Some(fin) = resto.find('>') else { break };
        let atributos = &resto[..fin];
        if let Some(nombre) = atributo(atributos, "android:name") {
            encontrados.insert(nombre, atributo(atributos, "android:required"));
        }
        resto = &resto[fin..];
    }
    encontrados
}

/// El valor de un atributo entrecomillado dentro de un elemento.
fn atributo(atributos: &str, nombre: &str) -> Option<String> {
    let inicio = atributos.find(nombre)? + nombre.len();
    let resto = atributos[inicio..].trim_start();
    let resto = resto.strip_prefix('=')?.trim_start();
    let comilla = resto.chars().next()?;
    if comilla != '"' && comilla != '\'' {
        return None;
    }
    let resto = &resto[comilla.len_utf8()..];
    let fin = resto.find(comilla)?;
    Some(resto[..fin].to_owned())
}

/// Que el manifiesto siga declarando el paquete del shell.
///
/// Si alguien lo cambia a mano, `javac` compila igual —las clases llevan su
/// `package` dentro— pero Android no encuentra la actividad y la app no abre.
/// Vale más pararlo aquí.
fn comprobar_manifiesto(manifest: &Path) -> Result<()> {
    let texto = std::fs::read_to_string(manifest)
        .with_context(|| format!("no se pudo leer {}", manifest.display()))?;
    if !texto.contains(&format!("package=\"{PACKAGE}\"")) {
        bail!(
            "{}: el manifiesto tiene que declarar package=\"{PACKAGE}\", que es donde están \
             las clases del shell.\n\
             El identificador de la aplicación no se pone aquí: sale de app.bundleId en \
             angular-native.json.",
            manifest.display()
        );
    }
    Ok(())
}

/// El almacén de claves de depuración estándar. Si no existe, se crea: es el
/// mismo que genera Android Studio, con la contraseña de siempre.
fn debug_keystore() -> Result<PathBuf> {
    let path =
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".android/debug.keystore");
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(path.parent().expect("tiene padre"))?;
    let status = Command::new("keytool")
        .args([
            "-genkeypair",
            "-keystore",
            &path.to_string_lossy(),
            "-storepass",
            "android",
            "-keypass",
            "android",
            "-alias",
            "androiddebugkey",
            "-keyalg",
            "RSA",
            "-keysize",
            "2048",
            "-validity",
            "10000",
            "-dname",
            "CN=Android Debug,O=Android,C=US",
        ])
        .status()
        .context("no se pudo ejecutar keytool")?;
    if !status.success() {
        bail!("no se pudo crear el almacén de claves de depuración");
    }
    Ok(path)
}

fn devices(adb: &Path) -> Result<Vec<(String, Form)>> {
    let salida = Command::new(adb)
        .args(["devices"])
        .output()
        .context("no se pudo ejecutar adb devices")?;
    if !salida.status.success() {
        bail!("adb devices falló");
    }
    let listado = String::from_utf8_lossy(&salida.stdout);
    let mut encontrados = Vec::new();
    for linea in listado.lines().skip(1) {
        let mut campos = linea.split_whitespace();
        let (Some(serial), Some("device")) = (campos.next(), campos.next()) else {
            continue;
        };
        let props = Command::new(adb)
            .args(["-s", serial, "shell", "getprop", "ro.build.characteristics"])
            .output()
            .with_context(|| format!("no se pudo preguntar por {serial}"))?;
        let forma = if String::from_utf8_lossy(&props.stdout).contains("watch") {
            Form::Watch
        } else {
            Form::Phone
        };
        encontrados.push((serial.to_owned(), forma));
    }
    Ok(encontrados)
}

/// Elige a qué aparato va el APK.
///
/// Sin esto, `adb install` a secas se planta en cuanto hay más de un emulador
/// arrancado, y con un teléfono y un reloj a la vez —que es lo normal en
/// cuanto se trabaja en los dos— eso es siempre. Peor: si acertara por
/// casualidad, el APK del reloj acabaría en el teléfono sin que nada lo dijera.
fn pick_device(adb: &Path, form: Form) -> Result<String> {
    let encontrados = devices(adb)?;
    let candidatos: Vec<&String> = encontrados
        .iter()
        .filter(|(_, forma)| *forma == form)
        .map(|(serial, _)| serial)
        .collect();
    match candidatos.as_slice() {
        [uno] => Ok((*uno).clone()),
        [] if encontrados.is_empty() => bail!(
            "no hay ningún aparato conectado; arranca un emulador de {} \
             (`emulator -avd <nombre>`)",
            form.nombre()
        ),
        [] => bail!(
            "no hay ningún aparato con forma de {}; lo que hay es: {}",
            form.nombre(),
            encontrados
                .iter()
                .map(|(serial, forma)| format!("{serial} ({})", forma.nombre()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        varios => bail!(
            "hay {} aparatos con forma de {}: {}. Elige con --device",
            varios.len(),
            form.nombre(),
            varios
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Comprueba que el aparato que pidió `--device` existe y es de la forma que
/// se está armando.
///
/// Dejar pasar un serial cualquiera sería peor que no tener la opción: el APK
/// del reloj entra sin quejarse en un teléfono, arranca y pinta, y lo único
/// que no hace es ser una app de reloj. Es el fallo que solo se ve al
/// publicarla.
fn check_device(adb: &Path, serial: &str, form: Form) -> Result<String> {
    let encontrados = devices(adb)?;
    match encontrados.iter().find(|(s, _)| s == serial) {
        Some((s, forma)) if *forma == form => Ok(s.clone()),
        Some((_, forma)) => bail!(
            "{serial} tiene forma de {}, y esto es un APK de {}",
            forma.nombre(),
            form.nombre()
        ),
        None => bail!(
            "no hay ningún aparato {serial}; lo que hay es: {}",
            if encontrados.is_empty() {
                "nada".to_owned()
            } else {
                encontrados
                    .iter()
                    .map(|(s, forma)| format!("{s} ({})", forma.nombre()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
    }
}

pub fn install_and_launch(
    workspace: &Workspace,
    apk: &Path,
    form: Form,
    device: Option<&str>,
) -> Result<()> {
    let sdk = Sdk::discover()?;
    let adb = sdk.adb();
    let application_id = application_id(workspace);
    // A qué aparato va, decidido antes de tocar nada. Sin el `-s`, `adb` elige
    // por su cuenta: con uno solo acierta siempre, y en cuanto hay dos se
    // planta —o, si el otro está sin autorizar, ni siquiera se planta y manda
    // el APK del reloj al teléfono.
    let serial = match device {
        Some(pedido) => check_device(&adb, pedido, form)?,
        None => pick_device(&adb, form)?,
    };
    eprintln!("==> instalando en {serial}");
    // Mismo motivo que en iOS: instalar sobre una app en marcha no recarga el
    // bundle nuevo.
    let _ = Command::new(&adb)
        .args(["-s", &serial, "shell", "am", "force-stop", &application_id])
        .output();
    run(
        workspace,
        &adb.to_string_lossy(),
        &["-s", &serial, "install", "-r", &apk.to_string_lossy()],
        "adb install falló",
    )?;
    run(
        workspace,
        &adb.to_string_lossy(),
        // La actividad conserva el paquete del shell aunque la aplicación se
        // llame de otra forma: `--rename-manifest-package` cualifica los
        // nombres de clase con el paquete original.
        &[
            "-s",
            &serial,
            "shell",
            "am",
            "start",
            "-n",
            &format!("{application_id}/{ACTIVITY}"),
        ],
        "no se pudo lanzar la app",
    )?;
    Ok(())
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

fn run<T: AsRef<str>>(cwd: impl AsCwd, program: &str, args: &[T], context: &str) -> Result<()> {
    let status = Command::new(program)
        .args(args.iter().map(|a| a.as_ref()))
        .current_dir(cwd.cwd())
        .status()
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}

pub trait AsCwd {
    fn cwd(&self) -> PathBuf;
}

impl AsCwd for &Workspace {
    fn cwd(&self) -> PathBuf {
        self.root.clone()
    }
}

impl AsCwd for &PathBuf {
    fn cwd(&self) -> PathBuf {
        (*self).clone()
    }
}
