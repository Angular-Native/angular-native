//! `an` — la herramienta de línea de comandos de angular-native.
//!
//! Hace lo que hacían los scripts de shell, pero como programa: compilar el
//! bundle con AOT, armar el `.app` sin `.xcodeproj`, y levantar un servidor de
//! desarrollo que recarga la app al guardar.

mod android;
mod build;
mod dev;
mod ios;
mod workspace;

use clap::{Parser, Subcommand};

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
        #[arg(long, default_value = "iPhone 17 Pro")]
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
    /// Servidor de desarrollo: vigila los ficheros y recarga la app al guardar.
    Dev {
        app: Option<String>,
        #[arg(long, default_value = "iPhone 17 Pro")]
        device: String,
        #[arg(long, default_value_t = 8420)]
        port: u16,
        /// Lanza en el emulador de Android en vez de en el simulador de iOS.
        #[arg(long)]
        android: bool,
        /// No lanza nada; solo sirve el bundle.
        #[arg(long)]
        no_launch: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let workspace = workspace::Workspace::discover()?;

    match cli.command {
        Command::Build { app, release } => {
            let app = workspace.app(app.as_deref())?;
            let bundle = build::bundle(&workspace, &app, release)?;
            println!("{}", bundle.display());
            Ok(())
        }
        Command::Ios { app, device, release } => {
            let app = workspace.app(app.as_deref())?;
            let bundle = build::bundle(&workspace, &app, release)?;
            let package = ios::assemble(&workspace, &bundle, release, None)?;
            ios::launch(&package, &device)
        }
        Command::Android { app, release, no_launch } => {
            let app = workspace.app(app.as_deref())?;
            let bundle = build::bundle(&workspace, &app, release)?;
            let apk = android::assemble(&workspace, &bundle, release, None)?;
            if no_launch {
                println!("{}", apk.display());
                return Ok(());
            }
            android::install_and_launch(&workspace, &apk)
        }
        Command::Dev { app, device, port, android, no_launch } => {
            let app = workspace.app(app.as_deref())?;
            let target = if android { dev::Target::Android } else { dev::Target::Ios { device } };
            dev::run(workspace, app, target, port, no_launch)
        }
    }
}
