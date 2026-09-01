//! `an` — la herramienta de línea de comandos de angular-native.
//!
//! Hace lo que hacían los scripts de shell, pero como programa: compilar el
//! bundle con AOT, armar el `.app` sin `.xcodeproj`, y levantar un servidor de
//! desarrollo que recarga la app al guardar.

mod android;
mod build;
mod dev;
mod ios;
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
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package = ios::assemble(&workspace, &bundle, release, None, &found)?;
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            ios::launch(&package, &device)
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
        Command::Android { app, release, no_launch } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let apk = android::assemble(&workspace, &bundle, release, None, &found)?;
            if no_launch {
                println!("{}", apk.display());
                return Ok(());
            }
            android::install_and_launch(&workspace, &apk)
        }
        Command::Dev { app, device, port, android, watchos, no_launch } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let target = match (android, watchos) {
                (true, _) => dev::Target::Android,
                (_, true) => dev::Target::WatchOs {
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
