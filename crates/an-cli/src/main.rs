//! `an` — la herramienta de línea de comandos de angular-native.
//!
//! Hace lo que hacían los scripts de shell, pero como programa: compilar el
//! bundle con AOT, armar el `.app` sin `.xcodeproj`, y levantar un servidor de
//! desarrollo que recarga la app al guardar.

mod android;
mod build;
mod dev;
mod init;
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
    /// Compila, arma el .app del reloj y lo lanza en el simulador de watchOS.
    ///
    /// Necesita nightly con `rust-src`: `aarch64-apple-watchos-sim` es un
    /// target de nivel 3 y su `std` se construye en el momento.
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
    /// Prepara un proyecto Angular existente para compilar a nativo.
    ///
    /// Se ejecuta dentro del proyecto —uno de `ng new`— y le añade las
    /// dependencias, el tsconfig del build nativo y un punto de entrada. No
    /// toca nada de lo que ya haya.
    Init {
        /// Directorio del proyecto. Por defecto, el actual.
        dir: Option<String>,
        /// Nombre de la app. Por defecto sale del `name` del package.json.
        #[arg(long)]
        name: Option<String>,
        /// Identificador del paquete, p. ej. com.ejemplo.miapp.
        #[arg(long)]
        id: Option<String>,
        /// Reescribe lo que genera `an init` y reinstala los paquetes.
        /// Nunca toca el código de la app.
        #[arg(long)]
        force: bool,
    },
    /// Añade una plataforma al proyecto: `an add ios`, `an add android`.
    Add {
        platform: String,
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
    // `an init` es el único que corre donde todavía no hay nada que descubrir:
    // el proyecto no está inicializado, que es justo el motivo de ejecutarlo.
    if let Command::Init { dir, name, id, force } = &cli.command {
        return init::init(dir.as_deref(), name.as_deref(), id.as_deref(), *force);
    }
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
        Command::Android { app, release, no_launch } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let apk =
                android::assemble(&workspace, &bundle, release, None, &found, android::Form::Phone)?;
            if no_launch {
                println!("{}", apk.display());
                return Ok(());
            }
            android::install_and_launch(&workspace, &apk, android::Form::Phone, None)
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
        Command::Add { platform } => init::add(&workspace, &platform),
        // Ya se atendió antes de descubrir el proyecto.
        Command::Init { .. } => unreachable!(),
    }
}
