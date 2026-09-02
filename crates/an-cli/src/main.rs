//! `an` — la herramienta de línea de comandos de angular-native.
//!
//! Hace lo que hacían los scripts de shell, pero como programa: compilar el
//! bundle con AOT, armar el `.app` sin `.xcodeproj`, y levantar un servidor de
//! desarrollo que recarga la app al guardar.

mod android;
mod build;
mod dev;
mod ios;
mod macos;
mod plugins;
mod watchos;
mod workspace;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "an", version, about = "Angular sobre vistas nativas", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compila una app a un bundle JS (ngc + esbuild).
    Build {
        /// Directorio de la app. Por defecto, examples/hello-angular.
        app: Option<String>,
        /// Compila con ngDevMode desactivado y minificado.
        #[arg(long)]
        release: bool,
    },
    /// Compila, arma el .app y lo lanza en el simulador de iOS.
    Ios {
        app: Option<String>,
        /// Nombre del simulador.
        #[arg(long, default_value = TELEFONO_POR_DEFECTO)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Solo arma el .app, sin instalarlo. Compila el shell y los plugins,
        /// que es lo que se puede comprobar sin simulador.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compila, arma el .app de la tele y lo lanza en el simulador de tvOS.
    ///
    /// Necesita nightly con `rust-src`: `aarch64-apple-tvos-sim` es un target
    /// de nivel 3 y su `std` se construye en el momento.
    Tvos {
        app: Option<String>,
        /// Nombre del simulador de tvOS.
        #[arg(long, default_value = TELE_POR_DEFECTO)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Solo arma el .app, sin instalarlo.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compila, arma el .app y lo lanza en el simulador de visionOS.
    ///
    /// Necesita nightly con `rust-src`, por lo mismo que tvOS.
    Visionos {
        app: Option<String>,
        /// Nombre del simulador de visionOS.
        #[arg(long, default_value = VISOR_POR_DEFECTO)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Solo arma el .app, sin instalarlo.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compila, arma el .app de escritorio y lo lanza en este Mac.
    ///
    /// No hay simulador: la app corre aquí mismo. Por defecto lleva el ejemplo
    /// de los controles, que es el que enseña de un vistazo qué pinta AppKit y
    /// qué no.
    Macos {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Solo arma el .app, sin lanzarlo.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compila, arma el .app del reloj y lo lanza en el simulador de watchOS.
    ///
    /// Necesita nightly con `rust-src`: `aarch64-apple-watchos-sim` es un
    /// target de nivel 3 y su `std` se construye en el momento.
    Watchos {
        app: Option<String>,
        /// Nombre del simulador de reloj.
        #[arg(long, default_value = RELOJ_POR_DEFECTO)]
        device: String,
        #[arg(long)]
        release: bool,
    },
    /// Compila, arma el APK y lo lanza en el emulador de Android.
    Android {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Solo arma el APK, sin instalarlo.
        #[arg(long)]
        no_launch: bool,
        /// Número de serie del aparato (`adb devices`). Por defecto, el único
        /// que haya con forma de teléfono.
        #[arg(long)]
        device: Option<String>,
    },
    /// Compila, arma el APK del reloj y lo lanza en un emulador de Wear OS.
    ///
    /// Es un comando aparte y no un `--wear` de `an android` por lo mismo que
    /// `an watchos` no es un `--watch` de `an ios`: cambia el manifiesto,
    /// cambia el tema, cambia el ejemplo por defecto y cambia el aparato al
    /// que va. Con un flag habría que repetir las cuatro cosas en cada
    /// invocación.
    Wearos {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Solo arma el APK, sin instalarlo.
        #[arg(long)]
        no_launch: bool,
        /// Número de serie del reloj (`adb devices`). Por defecto, el único
        /// que haya con forma de reloj.
        #[arg(long)]
        device: Option<String>,
    },
    /// Enseña los plugins de los que depende una app.
    ///
    /// Con `--platform` además comprueba que todos cubran esa plataforma, que
    /// es lo mismo que hace el build antes de compilar nada.
    Plugins {
        app: Option<String>,
        #[arg(long)]
        platform: Option<PlatformArg>,
    },
    /// Servidor de desarrollo: vigila los ficheros y recarga la app al guardar.
    Dev {
        app: Option<String>,
        #[arg(long, default_value = TELEFONO_POR_DEFECTO)]
        device: String,
        #[arg(long, default_value_t = 8420)]
        port: u16,
        /// Lanza en el emulador de Android en vez de en el simulador de iOS.
        #[arg(long)]
        android: bool,
        /// Lanza en el simulador del reloj en vez de en el del teléfono.
        #[arg(long)]
        watchos: bool,
        /// Lanza como app de escritorio en este Mac.
        #[arg(long)]
        macos: bool,
        /// Lanza en un emulador de Wear OS. Aquí sí es un flag y no un
        /// comando: `an dev` ya elige aparato con flags y el ejemplo lo pone
        /// quien lo invoca.
        #[arg(long)]
        wearos: bool,
        /// No lanza nada; solo sirve el bundle.
        #[arg(long)]
        no_launch: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum PlatformArg {
    Ios,
    Android,
}

impl From<PlatformArg> for plugins::Platform {
    fn from(value: PlatformArg) -> Self {
        match value {
            PlatformArg::Ios => plugins::Platform::Ios,
            PlatformArg::Android => plugins::Platform::Android,
        }
    }
}

/// Simuladores por defecto. `an dev --watchos` no lleva su propio `--device`:
/// si el que hay es el del teléfono, es que nadie lo eligió, y lo que quiere
/// es el reloj.
const TELEFONO_POR_DEFECTO: &str = "iPhone 17 Pro";
const RELOJ_POR_DEFECTO: &str = "Apple Watch Series 11 (46mm)";
const TELE_POR_DEFECTO: &str = "Apple TV 4K (3rd generation)";
const VISOR_POR_DEFECTO: &str = "Apple Vision Pro";

/// Lo que hacen igual las tres familias de UIKit: descubrir plugins, compilar
/// el bundle, armar el `.app` y —si se pide— lanzarlo.
///
/// `por_defecto` es la app que se usa cuando no se nombra ninguna. iOS se
/// queda con la del workspace; tvOS y visionOS traen la suya porque una
/// pantalla de tele y una ventana volumétrica no se parecen a un teléfono.
#[allow(clippy::too_many_arguments)]
fn uikit(
    workspace: &workspace::Workspace,
    family: ios::Family,
    app: Option<String>,
    device: String,
    release: bool,
    no_launch: bool,
    por_defecto: Option<&str>,
) -> anyhow::Result<()> {
    let elegida = app.as_deref().or(por_defecto);
    let app = workspace.app(elegida)?;
    let found = plugins::discover(workspace, &app)?;
    let bundle = build::bundle(workspace, &app, release, &found)?;
    let package = ios::assemble(workspace, family, &bundle, release, None, &found)?;
    if no_launch {
        println!("{}", package.dir.display());
        return Ok(());
    }
    ios::launch(&package, &device)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let workspace = workspace::Workspace::discover()?;

    match cli.command {
        Command::Build { app, release } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            println!("{}", bundle.display());
            Ok(())
        }
        Command::Plugins { app, platform } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            plugins::list(&workspace, &found);
            match platform {
                Some(platform) => plugins::require(&found, platform.into()),
                None => Ok(()),
            }
        }
        Command::Ios { app, device, release, no_launch } => {
            uikit(&workspace, ios::Family::Ios, app, device, release, no_launch, None)
        }
        Command::Tvos { app, device, release, no_launch } => {
            // El ejemplo por defecto de la tele no es el del teléfono: en una
            // pantalla de 1920x1080 vista desde el sofá, `hello-angular` sale
            // en una esquina y con la letra ilegible. Y sobre todo, en tvOS no
            // hay toques: lo que hay que enseñar es el foco del mando.
            uikit(
                &workspace,
                ios::Family::TvOs,
                app,
                device,
                release,
                no_launch,
                Some("examples/hello-tv"),
            )
        }
        Command::Visionos { app, device, release, no_launch } => uikit(
            &workspace,
            ios::Family::VisionOs,
            app,
            device,
            release,
            no_launch,
            Some("examples/hello-vision"),
        ),
        Command::Macos { app, release, no_launch } => {
            // El ejemplo por defecto del escritorio no es el de todos los
            // demás: `controls` enseña los controles del sistema uno detrás de
            // otro, que es lo que hay que mirar para saber qué pinta este host.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/controls")))?;
            let found = plugins::discover(&workspace, &app)?;
            macos::reject_plugins(&found)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package = macos::assemble(&workspace, &bundle, release, None)?;
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            macos::launch(&package)
        }
        Command::Watchos { app, device, release } => {
            // El ejemplo por defecto del reloj no es el de todos los demás:
            // `hello-angular` está pensado para un teléfono y en 205 puntos de
            // ancho no se lee.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/hello-watch")))?;
            let found = plugins::discover(&workspace, &app)?;
            watchos::reject_plugins(&found)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package = watchos::assemble(&workspace, &bundle, release, None)?;
            watchos::launch(&package, &device)
        }
        Command::Android { app, release, no_launch, device } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let apk =
                android::assemble(&workspace, &bundle, release, None, &found, android::Form::Phone)?;
            if no_launch {
                println!("{}", apk.display());
                return Ok(());
            }
            android::install_and_launch(&workspace, &apk, android::Form::Phone, device.as_deref())
        }
        Command::Wearos { app, release, no_launch, device } => {
            // Igual que en el reloj de Apple: el ejemplo por defecto no puede
            // ser el del teléfono. En 227 puntos de ancho, y redondos, no se
            // lee.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/hello-wear")))?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let apk =
                android::assemble(&workspace, &bundle, release, None, &found, android::Form::Watch)?;
            if no_launch {
                println!("{}", apk.display());
                return Ok(());
            }
            android::install_and_launch(&workspace, &apk, android::Form::Watch, device.as_deref())
        }
        Command::Dev { app, device, port, android, watchos, wearos, macos, no_launch } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            if macos {
                macos::reject_plugins(&found)?;
                return dev::run(workspace, app, dev::Target::MacOs, port, no_launch, found);
            }
            let target = match (android, watchos, wearos) {
                (_, _, true) => dev::Target::Android { form: crate::android::Form::Watch },
                (true, _, _) => dev::Target::Android { form: crate::android::Form::Phone },
                (_, true, _) => dev::Target::WatchOs {
                    device: if device == TELEFONO_POR_DEFECTO {
                        RELOJ_POR_DEFECTO.to_owned()
                    } else {
                        device
                    },
                },
                _ => dev::Target::Ios { device },
            };
            if matches!(target, dev::Target::WatchOs { .. }) {
                watchos::reject_plugins(&found)?;
            }
            dev::run(workspace, app, target, port, no_launch, found)
        }
    }
}
