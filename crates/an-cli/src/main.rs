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
mod signing;
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
        /// The simulator's name — or, with `--physical`, the device's name or
        /// UDID.
        #[arg(long, default_value = DEFAULT_PHONE)]
        device: String,
        #[arg(long)]
        release: bool,
        /// Only builds the .app, without installing it. It compiles the shell
        /// and the plugins, which is what can be checked with no simulator.
        #[arg(long)]
        no_launch: bool,
        /// Builds for a device plugged into this Mac, signs it and installs it
        /// with `devicectl`.
        ///
        /// It needs the signing settings: a development certificate in the
        /// keychain and a provisioning profile that lists this device. See
        /// https://angular-native.github.io/guide/signing-and-distribution/.
        #[arg(long)]
        physical: bool,
        /// Builds a signed `.xcarchive` and the `.ipa` that comes out of it,
        /// for TestFlight or the App Store. It implies `--release`.
        ///
        /// Which of those the `.ipa` can go to is decided by the certificate
        /// and the profile it was signed with, not by a flag here.
        #[arg(long, conflicts_with = "physical")]
        archive: bool,
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
        /// Signs with a Developer ID certificate and the hardened runtime,
        /// instead of the ad-hoc signature that only works on this machine.
        #[arg(long)]
        sign: bool,
        /// Sends the .app to Apple, waits for the answer and staples the
        /// ticket to it. It implies `--sign`.
        #[arg(long)]
        notarize: bool,
        /// Puts the .app in a .dmg. With `--sign` the image is signed too, and
        /// with `--notarize` it is notarised and stapled in its own right.
        #[arg(long)]
        dmg: bool,
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
        /// Signs with the release keystore. A Wear app goes to the same Play
        /// listing as the phone's, so it is the same key.
        #[arg(long)]
        sign: bool,
        /// Builds an `.aab` for Google Play instead of an `.apk`.
        #[arg(long)]
        aab: bool,
    },
    /// Compiles, builds the APK and launches it in the Android emulator.
    Android {
        app: Option<String>,
        #[arg(long)]
        release: bool,
        /// Only builds the APK, without installing it.
        #[arg(long)]
        no_launch: bool,
        /// Signs with the release keystore from the signing settings instead of
        /// the debug one, which no store accepts.
        ///
        /// `--release` is about the compiler; this is about the key. A build
        /// for Play wants both.
        #[arg(long)]
        sign: bool,
        /// Builds an `.aab` for Google Play instead of an `.apk`. It implies
        /// `--sign`: Play takes nothing signed with a debug key.
        #[arg(long)]
        aab: bool,
        /// The adb serial to install on, as `adb devices` prints it.
        ///
        /// Without it the single phone-shaped device is taken, and with an
        /// emulator and a real phone both plugged in there are two: a device
        /// is named here, not guessed.
        #[arg(long)]
        device: Option<String>,
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
    Macos,
    Watchos,
}

impl From<PlatformArg> for plugins::Platform {
    fn from(value: PlatformArg) -> Self {
        match value {
            PlatformArg::Ios => plugins::Platform::Ios,
            PlatformArg::Android => plugins::Platform::Android,
            PlatformArg::Macos => plugins::Platform::Macos,
            PlatformArg::Watchos => plugins::Platform::Watchos,
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
                Some(platform) => {
                    plugins::require(&found, platform.into(), &std::collections::BTreeSet::new())
                }
                None => Ok(()),
            }
        }
        Command::Ios { app, device, release, no_launch, physical, archive } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            if !physical && !archive {
                let bundle = build::bundle(&workspace, &app, release, &found)?;
                let package = ios::assemble(
                    &workspace,
                    ios::Family::Ios,
                    &bundle,
                    release,
                    None,
                    &found,
                    None,
                )?;
                if no_launch {
                    println!("{}", package.dir.display());
                    return Ok(());
                }
                return ios::launch(&package, &device);
            }
            // Signing first, and before the bundle: it is the only step here
            // that can fail for a reason nobody can guess at, and finding out
            // after two minutes of `cargo` that the profile expired is exactly
            // the failure this is meant to stop happening.
            let settings = signing::Settings::read(&workspace)?;
            let purpose = if archive {
                signing::Purpose::Distribution
            } else {
                signing::Purpose::Development
            };
            let apple = signing::apple(&settings, "ios", &workspace.bundle_id(), purpose)?;
            eprintln!("==> signing as {} (team {})", apple.identity_name, apple.team);
            // An archive is always a release build: an `.ipa` with ngDevMode on
            // is twice the size and runs Angular's development checks on
            // somebody else's phone.
            let bundle = build::bundle(&workspace, &app, release || archive, &found)?;
            let package = ios::assemble(
                &workspace,
                ios::Family::Ios,
                &bundle,
                release || archive,
                None,
                &found,
                Some(&apple),
            )?;
            if archive {
                let (_, ipa) = ios::archive(&package, &apple)?;
                println!("{}", ipa.display());
                return Ok(());
            }
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            // Same rule as `an dev`: if `--device` is still the simulator's
            // default, nobody chose it, and what is wanted is the only device
            // plugged in.
            ios::install_on_device(&package, (device != DEFAULT_PHONE).then_some(device.as_str()))
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
                ios::assemble(&workspace, ios::Family::VisionOs, &bundle, release, None, &found, None)?;
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
                ios::assemble(&workspace, ios::Family::TvOs, &bundle, release, None, &found, None)?;
            if no_launch {
                println!("{}", package.dir.display());
                return Ok(());
            }
            ios::launch(&package, &device)
        }
        Command::Macos { app, release, no_launch, sign, notarize, dmg } => {
            // The desktop's default example is not everybody else's:
            // `controls` shows the system controls one after another, which is
            // what to look at to know what this host draws.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/controls")))?;
            let found = plugins::discover(&workspace, &app)?;
            macos::require_plugins(&found)?;
            // Before the bundle, before cargo: notarising asks for a keychain
            // profile that either exists or does not, and it is not going to
            // start existing because a compilation ran first.
            let identity = if sign || notarize {
                let settings = signing::Settings::read(&workspace)?;
                let macos = signing::macos(&settings, notarize)?;
                eprintln!("==> signing as {}", macos.identity_name);
                Some(macos)
            } else {
                None
            };
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package =
                macos::assemble(&workspace, &bundle, release, None, &found, identity.as_ref())?;
            if notarize {
                macos::notarize(&package, identity.as_ref().expect("--notarize implies --sign"))?;
            }
            if dmg {
                if identity.is_none() {
                    // Said, and not refused: a .dmg of an ad-hoc build is a
                    // perfectly good way to check the packaging works. It is
                    // only useless as a download, and that is the part nobody
                    // finds out until somebody else tries to open it.
                    eprintln!(
                        "==> warning: this .dmg holds an ad-hoc signed app, so another Mac \
                         will refuse to open it. Add --sign --notarize for one that opens \
                         anywhere."
                    );
                }
                let image = macos::dmg(&package, identity.as_ref(), notarize)?;
                println!("{}", image.display());
                return Ok(());
            }
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
            watchos::require_plugins(&found)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let package = watchos::assemble(&workspace, &bundle, release, None, &found)?;
            watchos::launch(&package, &device)
        }
        Command::Android { app, release, no_launch, sign, aab, device } => {
            let app = workspace.app(app.as_deref())?;
            let found = plugins::discover(&workspace, &app)?;
            let (keystore, bundletool) = android_signing(&workspace, sign, aab)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let artefact = android::assemble(
                &workspace,
                &bundle,
                release,
                None,
                &found,
                android::Packaging {
                    form: android::Form::Phone,
                    signing: keystore.as_ref(),
                    aab,
                    bundletool,
                },
            )?;
            // A bundle is not something a device installs: Play turns it into
            // APKs. It is the one artefact here with nowhere to be launched.
            if no_launch || aab {
                println!("{}", artefact.display());
                return Ok(());
            }
            android::install_and_launch(&workspace, &artefact, android::Form::Phone, device.as_deref())
        }
        Command::Wearos { app, release, no_launch, device, sign, aab } => {
            // Same as on the Apple watch: the default example cannot be the
            // phone's. At 227 points wide, and round, it cannot be read.
            let app = workspace.app(Some(app.as_deref().unwrap_or("examples/hello-wear")))?;
            let found = plugins::discover(&workspace, &app)?;
            let (keystore, bundletool) = android_signing(&workspace, sign, aab)?;
            let bundle = build::bundle(&workspace, &app, release, &found)?;
            let artefact = android::assemble(
                &workspace,
                &bundle,
                release,
                None,
                &found,
                android::Packaging {
                    form: android::Form::Watch,
                    signing: keystore.as_ref(),
                    aab,
                    bundletool,
                },
            )?;
            if no_launch || aab {
                println!("{}", artefact.display());
                return Ok(());
            }
            android::install_and_launch(
                &workspace,
                &artefact,
                android::Form::Watch,
                device.as_deref(),
            )
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
                watchos::require_plugins(&found)?;
            }
            if matches!(target, dev::Target::MacOs) {
                macos::require_plugins(&found)?;
            }
            dev::run(workspace, app, target, port, no_launch, found)
        }
        Command::Add { platform } => init::add(&workspace, &platform),
        // Already handled before the project was discovered.
        Command::Init { .. } => unreachable!(),
    }
}

/// The release keystore for an Android build, or `None` for the debug one.
///
/// `--aab` implies `--sign` and is not asked to say so twice: Play takes nothing
/// signed with a debug key, so a bundle signed with one is an artefact with
/// nowhere to go.
fn android_signing(
    workspace: &workspace::Workspace,
    sign: bool,
    aab: bool,
) -> anyhow::Result<(Option<signing::Android>, Option<std::path::PathBuf>)> {
    if !sign && !aab {
        return Ok((None, None));
    }
    // Both looked up here, before the bundle is compiled: the keystore and
    // bundletool are the two things that can be missing, and neither of them
    // becomes present because `ngc` ran for a minute first.
    let bundletool = aab.then(|| android::bundletool(workspace)).transpose()?;
    let settings = signing::Settings::read(workspace)?;
    let android = signing::android(&settings)?;
    eprintln!(
        "==> signing with {} (alias {})",
        android.keystore.display(),
        android.key_alias
    );
    Ok((Some(android), bundletool))
}
