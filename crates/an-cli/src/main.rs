//! `an` — angular-native's command line tool.
//!
//! It does what the shell scripts used to do, but as a program: compile the
//! bundle with AOT, put the `.app` together without an `.xcodeproj`, and stand
//! up a dev server that reloads the app when you save.

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
#[command(name = "an", version, about = "Angular on native views", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compiles an app into a JS bundle (ngc + esbuild).
    Build {
        /// The app's directory. Defaults to examples/hello-angular.
        app: Option<String>,
        /// Compiles with ngDevMode off, and minified.
        #[arg(long)]
        release: bool,
    },
    /// Compiles, builds the .app and launches it in the iOS simulator.
    Ios {
        app: Option<String>,
        /// The simulator's name.
        #[arg(long, default_value = DEFAULT_PHONE)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Only builds the .app, without installing it. It compiles the shell
        /// and the plugins, which is what can be checked with no simulator.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compiles, builds the tvOS .app and launches it in the Apple TV
    /// simulator.
    ///
    /// It needs nightly with `rust-src`: `aarch64-apple-tvos-sim` is a tier 3
    /// target and its `std` is built on the spot.
    Tvos {
        app: Option<String>,
        /// The Apple TV simulator's name.
        #[arg(long, default_value = DEFAULT_TV)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Only builds the .app, without installing it.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compiles, builds the visionOS .app and launches it in the headset
    /// simulator.
    ///
    /// It needs nightly with `rust-src`: `aarch64-apple-visionos-sim` is a
    /// tier 3 target and its `std` is built on the spot.
    Visionos {
        app: Option<String>,
        /// The headset simulator's name.
        #[arg(long, default_value = DEFAULT_HEADSET)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Only builds the .app, without installing it.
        #[arg(long)]
        no_launch: bool,
    },
    /// There is no simulator: the app runs right here. It defaults to the
    /// controls example, the one that shows at a glance what AppKit draws and
    /// what it does not.
    Macos {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Only builds the .app, without launching it.
        #[arg(long)]
        no_launch: bool,
    },
    /// Compiles, builds the watch .app and launches it in the watchOS
    /// simulator.
    ///
    /// It needs nightly with `rust-src`: `aarch64-apple-watchos-sim` is a
    /// tier 3 target and its `std` is built on the spot.
    Watchos {
        app: Option<String>,
        /// The watch simulator's name.
        #[arg(long, default_value = DEFAULT_WATCH)]
        device: String,
        #[arg(long)]
        release: bool,
    },
    /// `an watchos` is not a `--watch` on `an ios`: it changes the manifest,
    /// it changes the theme, it changes the default example and it changes the
    /// device it goes to. With a flag those four things would have to be
    /// repeated on every invocation.
    Wearos {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Only builds the APK, without installing it.
        #[arg(long)]
        no_launch: bool,
        /// The watch's serial number (`adb devices`). Defaults to the only
        /// watch-shaped one around.
        #[arg(long)]
        device: Option<String>,
    },
    /// Compiles, builds the APK and launches it in the Android emulator.
    Android {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Only builds the APK, without installing it.
        #[arg(long)]
        no_launch: bool,
    },
    /// Shows the plugins an app depends on.
    ///
    /// With `--platform` it also checks that they all cover that platform,
    /// which is what the build does before compiling anything.
    Plugins {
        app: Option<String>,
        #[arg(long)]
        platform: Option<PlatformArg>,
    },
    /// Dev server: it watches the files and reloads the app when you save.
    Dev {
        app: Option<String>,
        #[arg(long, default_value = DEFAULT_PHONE)]
        device: String,
        #[arg(long, default_value_t = 8420)]
        port: u16,
        /// Launches in the Android emulator instead of the iOS simulator.
        #[arg(long)]
        android: bool,
        /// Launches in the Wear OS emulator instead of the phone's. It builds
        /// the watch APK, not the phone's aimed somewhere else.
        #[arg(long)]
        wearos: bool,
        /// Launches in the watch simulator instead of the phone's.
        #[arg(long)]
        watchos: bool,
        /// Launches in the Apple TV simulator instead of the phone's.
        #[arg(long)]
        tvos: bool,
        /// Launches in the headset simulator instead of the phone's.
        #[arg(long)]
        visionos: bool,
        /// Opens a window on this Mac instead of the phone's simulator. There
        /// is no simulator here: the app runs on the machine doing the
        /// building, so the dev server is on the same computer as the app.
        #[arg(long)]
        macos: bool,
        /// Launches nothing; it only serves the bundle.
        #[arg(long)]
        no_launch: bool,
    },
    /// Gets an existing Angular project ready to compile to native.
    ///
    /// It is run inside the project —one from `ng new`— and adds the
    /// dependencies, the tsconfig for the native build and an entry point to
    /// it. It touches nothing that is already there.
    Init {
        /// The project's directory. Defaults to the current one.
        dir: Option<String>,
        /// The app's name. Defaults to the `name` in package.json.
        #[arg(long)]
        name: Option<String>,
        /// The package identifier, e.g. com.example.myapp.
        #[arg(long)]
        id: Option<String>,
        /// Rewrites what `an init` generates and reinstalls the packages.
        /// It never touches the app's code.
        #[arg(long)]
        force: bool,
    },
    /// Adds a platform to the project: `an add ios`, `an add android`.
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

/// The default simulators. `an dev --watchos` has no `--device` of its own: if
/// the one in hand is the phone's, then nobody chose it, and what is wanted is
/// the watch.
const DEFAULT_PHONE: &str = "iPhone 17 Pro";
const DEFAULT_WATCH: &str = "Apple Watch Series 11 (46mm)";
/// The third-generation Apple TV 4K, which is the one the runtime ships with.
/// The other one on the list, "Apple TV", is the same thing at 1080p.
const DEFAULT_TV: &str = "Apple TV 4K (3rd generation)";
/// The only headset there is: the visionOS runtime ships one model.
const DEFAULT_HEADSET: &str = "Apple Vision Pro";

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // `an init` is the only one that runs where there is nothing to discover
    // yet: the project is not initialised, which is exactly why it is being
    // run.
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
            let package =
                ios::assemble(&workspace, ios::Family::Ios, &bundle, release, None, &found)?;
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            ios::launch(&package, &device)
        }
        Command::Visionos {
            app,
            device,
            release,
            no_launch,
        } => {
            // The headset's default example cannot be the phone's either: a
            // visionOS window has no screen size, and `hello-vision` is the one
            // written with no fixed points and no background of its own, which
            // is what that window asks for.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/hello-vision")))?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package =
                ios::assemble(&workspace, ios::Family::VisionOs, &bundle, release, None, &found)?;
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            ios::launch(&package, &device)
        }
        Command::Tvos {
            app,
            device,
            release,
            no_launch,
        } => {
            // The TV's default example is not everybody else's: `hello-angular`
            // is meant for a phone and cannot be read from three metres away on
            // a sofa. `hello-tv` also shows the one thing that cannot be shown
            // anywhere else: that without focus there is no press.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/hello-tv")))?;
            let found = plugins::discover(&workspace, &app)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package =
                ios::assemble(&workspace, ios::Family::TvOs, &bundle, release, None, &found)?;
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            ios::launch(&package, &device)
        }
        Command::Macos { app, release, no_launch } => {
            // The desktop's default example is not everybody else's:
            // `controls` shows the system controls one after another, which is
            // what to look at to know what this host draws.
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
            // The watch's default example is not everybody else's:
            // `hello-angular` is meant for a phone and cannot be read at 205
            // points wide.
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
            // Same as on the Apple watch: the default example cannot be the
            // phone's. At 227 points wide, and round, it cannot be read.
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
        Command::Dev {
            app,
            device,
            port,
            android,
            wearos,
            watchos,
            tvos,
            visionos,
            macos,
            no_launch,
        } => {
            // Same as in `an wearos` and `an macos`: the phone's example is not
            // the right default anywhere else. It cannot be read at 227 round
            // points, and on the desktop `controls` is the one that shows at a
            // glance what AppKit draws and what it does not.
            let default_app = if wearos {
                Some("examples/hello-wear")
            } else if macos {
                Some("examples/controls")
            } else {
                None
            };
            let app = workspace.app(app.as_deref().or(default_app))?;
            let found = plugins::discover(&workspace, &app)?;
            // If `--device` is still the phone's, then nobody chose it, and
            // what is wanted is the device the flag asks for.
            let chosen = |other: &str| {
                if device == DEFAULT_PHONE {
                    other.to_owned()
                } else {
                    device.clone()
                }
            };
            let target = match (android, wearos, watchos, tvos, visionos, macos) {
                (true, _, _, _, _, _) => dev::Target::Android,
                // An Android watch is identified by its `adb` serial number,
                // not by a simulator's name, so it does not go through `chosen`:
                // with no `--device` it picks one by asking for the shape.
                (_, true, _, _, _, _) => dev::Target::Wear {
                    device: (device != DEFAULT_PHONE).then(|| device.clone()),
                },
                (_, _, true, _, _, _) => dev::Target::WatchOs {
                    device: chosen(DEFAULT_WATCH),
                },
                (_, _, _, true, _, _) => dev::Target::TvOs {
                    device: chosen(DEFAULT_TV),
                },
                (_, _, _, _, true, _) => dev::Target::VisionOs {
                    device: chosen(DEFAULT_HEADSET),
                },
                // The Mac takes no `--device`: there is no simulator to name.
                (_, _, _, _, _, true) => dev::Target::MacOs,
                _ => dev::Target::Ios { device },
            };
            // The two hosts with no plugin registry refuse before compiling
            // anything, exactly as their own subcommands do: an app whose every
            // plugin call would be turned down at runtime is not an app worth
            // building.
            if matches!(target, dev::Target::WatchOs { .. }) {
                watchos::reject_plugins(&found)?;
            }
            if matches!(target, dev::Target::MacOs) {
                macos::reject_plugins(&found)?;
            }
            dev::run(workspace, app, target, port, no_launch, found)
        }
        Command::Add { platform } => init::add(&workspace, &platform),
        // Already handled before the project was discovered.
        Command::Init { .. } => unreachable!(),
    }
}
