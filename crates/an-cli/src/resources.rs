//! The files an app carries besides its code: images, fonts, JSON, anything an
//! `an-image` or a plugin asks for by name.
//!
//! Until this existed the build copied the `Info.plist` and `main.js` into the
//! bundle and nothing else, so `<an-image [source]="'logo.png'">` —a path with
//! no scheme, which every host reads as "a file that travelled with the app"—
//! could only ever come up empty. An `http` URL worked; a file did not, which
//! is the wrong way round for the case people reach for first.
//!
//! ## Where they live
//!
//! `<app>/resources/`. Inside the monorepo that is `examples/<name>/resources`;
//! outside it is the project's own root, beside the `ios/` and `macos/`
//! directories `an add` writes.
//!
//! It is the same place in both worlds on purpose, and it is deliberately *not*
//! one of these:
//!
//!   · **`src/assets/` or `public/`** belong to the web build. `angular.json`
//!     decides what goes in them and `an` never touches `angular.json` — see
//!     the rule in `workspace.rs` — so reading them would make the native build
//!     depend on a file it has promised to leave alone. It would also put every
//!     favicon, web font and `.svg` the browser needs into a phone binary that
//!     has no browser to need them.
//!   · **a per-platform `ios/Resources/`** would be four copies of the same PNG
//!     for anybody who ships on four platforms. The hosts all look the file up
//!     by the same name, so one directory is enough; a platform that needs its
//!     own artwork can still say so with a different name.
//!
//! What it *is* is the same kind of thing as `ios/Info.plist`: source, kept in
//! the user's repository, copied into the build and never rewritten by `an`.
//! That is why it sits next to it rather than under `.angular-native/`, which
//! holds what is generated.
//!
//! ## Where they land
//!
//! Beside `main.js`, wherever `main.js` goes — the flat `.app` root on iOS and
//! on the watch, `Contents/Resources` on macOS, `assets/` in the APK. Not a
//! choice so much as a report: that is where each host already looks.
//! `UIImage(named:)` and `NSImage(named:)` search the bundle,
//! `getAssets().open` reads `assets/`, and all three were looking there long
//! before anything put a file in front of them.
//!
//! The watch is the one that arrives at the same place by another road, and
//! `watchos.rs` says so where it copies: watchOS resolves an `imageNamed:`
//! against an asset catalogue, which a bundle nobody built with Xcode has none
//! of, so the shell reads the file by path out of `resourcePath` — and a watch
//! bundle being flat, that is the `.app` root again.
//!
//! ## The plugins' share of it
//!
//! A plugin ships methods and one kind of view, and neither of those is a `.png`,
//! a `.strings` or a sound. Anything it has to read by name travels the same
//! road: `angularNative.resources` in its `package.json` names a directory
//! inside the package, and its contents land in the very same place the app's
//! own do — the flat `.app` root, `Contents/Resources`, `assets/`. That is the
//! only place they *can* land: a plugin's Swift reaches its files through
//! `Bundle.main` and its Java through the `Activity`'s `getAssets()`, and
//! neither of the two has a compartment of its own to look in.
//!
//! **Android's `res/` is not that place, and cannot be.** A drawable under `res/`
//! is compiled by `aapt2` and reached through a generated `R` class whose
//! package belongs to the app; a plugin's Java would have to name
//! `dev.angularnative.R.drawable.something` for an id linked into a package it
//! does not own, and `an` would have to grow a per-plugin resource-compilation
//! and id-merging step to make it true. `assets/` needs none of that: the name
//! in the manifest is the name `getAssets().open` takes. What a plugin gives up
//! by living there is what `res/` is actually for — density buckets, locale
//! folders, theme attributes — and a plugin that needs those is shipping an
//! Android library, not a resource.
//!
//! **The namespace is flat and shared, so a collision is an error.** Two plugins
//! that each ship `icon.png` cannot both have it: whichever were copied second
//! would replace the other's, the plugin that lost would read the wrong file at
//! run time, and nothing anywhere would say so. The same goes for a plugin
//! landing on a name the app itself uses. There is no prefixing scheme instead of
//! this on purpose — the name in the manifest is the name the code asks for, and
//! a build that quietly renamed it would break the plugin it was trying to
//! protect. So the build stops and names both owners, which is the only thing
//! whoever reads it can act on: they wrote neither package.
//!
//! ## What is not there
//!
//! A name with nothing behind it is the host's to report, not the build's: only
//! the device knows which strings a template actually produced, and half of them
//! are computed. So the hosts warn once with the name — `images.rs` on iOS and
//! macOS, `AnImageStore` on the watch, `AnHost.loadImage` on Android — and this
//! module has one job it *can* do at build time, which is to refuse a resource
//! whose name collides with a file the build itself writes into the bundle.
//! Copying `resources/main.js` over the app's own bundle produces an app that
//! launches into nothing, and the reason is not visible anywhere on the device.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::plugins::Plugin;
use crate::workspace::Workspace;

/// The directory's name inside the app, in both worlds.
pub const DIR: &str = "resources";

/// Where this app's resources are, whether or not there is anything in there.
///
/// The two worlds are spelled out rather than leaning on `Path::join`
/// discarding a base when what it is given is absolute — which is what
/// `workspace.app()` returns outside the monorepo. It would work, and it would
/// be one line, and nobody reading it would know why.
pub fn dir(workspace: &Workspace, app: &Path) -> PathBuf {
    match &workspace.project {
        Some(project) => project.root.join(DIR),
        None => workspace.root.join(app).join(DIR),
    }
}

/// Whose file this is, for the sentence a collision produces.
#[derive(Clone)]
enum Owner {
    App,
    /// The npm package name, which is how the app declared the plugin and the
    /// only handle anybody has on it.
    Plugin(String),
}

impl Owner {
    fn describe(&self, path: &Path) -> String {
        match self {
            Owner::App => format!("the app's own {}", path.display()),
            Owner::Plugin(package) => format!("{package}, from {}", path.display()),
        }
    }
}

/// One file on its way into the bundle: where it is, what it will be called in
/// there, and who put it there.
struct Carried {
    from: PathBuf,
    /// The path relative to the bundle directory, with `/` separators. It is the
    /// name a template's `[source]` or a plugin's `Bundle.main` asks for.
    relative: String,
    owner: Owner,
}

/// Everything that is going into the bundle, with every collision already
/// refused. Nothing is read or written; this is the part that can run before a
/// single byte is compiled.
///
/// `reserved` is the list of names the build writes into that same directory
/// itself. A resource that would land on one of them stops the build.
fn plan(
    workspace: &Workspace,
    app: &Path,
    plugins: &[Plugin],
    reserved: &[&str],
) -> Result<Vec<Carried>> {
    let mut planned: Vec<Carried> = Vec::new();
    // By the name inside the bundle, which is the thing that can collide.
    let mut taken: BTreeMap<String, (Owner, PathBuf)> = BTreeMap::new();

    // The app's own first, so that a plugin landing on one of its names is
    // reported as the plugin arriving late rather than the other way round: the
    // app is the thing being built, and the plugin is the one that can be
    // swapped for another.
    let mut sources: Vec<(Owner, PathBuf)> = vec![(Owner::App, dir(workspace, app))];
    for plugin in plugins {
        if let Some(from) = &plugin.resources {
            sources.push((Owner::Plugin(plugin.package.clone()), from.clone()));
        }
    }

    for (owner, from) in sources {
        if !from.is_dir() {
            continue;
        }
        let mut found = Vec::new();
        collect(&from, &from, &mut found)?;
        for (path, relative) in found {
            if reserved.contains(&relative.as_str()) {
                bail!(
                    "{} cannot be a resource: {relative} is the name the build writes into the \
                     app itself, and the copy would replace it.\n\
                     Rename it — a resource keeps the name it has here, and that is the name \
                     the template asks for.{}",
                    path.display(),
                    match &owner {
                        Owner::App => String::new(),
                        Owner::Plugin(package) => format!(
                            "\nIt is {package} that ships it, so the rename is that plugin's \
                             to make."
                        ),
                    }
                );
            }
            if let Some((first, first_path)) = taken.get(&relative) {
                bail!(
                    "two of the things this app carries ship a resource called {relative}:\n\
                     \x20 · {}\n\
                     \x20 · {}\n\n\
                     They land in one directory and are asked for by that one name, so the \
                     second would replace the first and the code that reads it would get the \
                     other one's file with nothing said.\n\
                     One of the two has to rename it; the name in the package.json is the name \
                     the code asks for, so `an` cannot rename it for them.",
                    first.describe(first_path),
                    owner.describe(&path),
                );
            }
            taken.insert(relative.clone(), (owner.clone(), path.clone()));
            planned.push(Carried { from: path, relative, owner: owner.clone() });
        }
    }
    Ok(planned)
}

/// The names every host writes into the resource directory, whichever it is.
///
/// The rest of each platform's reserved list —`Info.plist`, the executable, the
/// icon font— is that platform's own and is only known where the bundle is put
/// together. These two are written by all four, so they can be refused before a
/// platform has been named.
const ALWAYS_RESERVED: &[&str] = &["main.js", "dev-server.txt"];

/// That nothing the app carries collides with anything else it carries.
///
/// It is `plan` with only the reserved names every platform shares, because the
/// rest of them differ per platform and this runs before one is chosen. What is
/// left is everything that can be said from the manifests alone.
/// `an plugins --platform …` calls it so that two plugins shipping one file name
/// is found in the second it takes to read some JSON, and not half a minute into
/// `swiftc`.
pub fn check(workspace: &Workspace, app: &Path, plugins: &[Plugin]) -> Result<()> {
    plan(workspace, app, plugins, ALWAYS_RESERVED).map(|_| ())
}

/// Copies the app's resources —and its plugins'— into `into`, keeping whatever
/// directory structure they had, and returns their paths relative to it in the
/// order they were copied. Nothing there, nothing said, empty list.
///
/// `reserved` is the list of names the build writes into that same directory
/// itself. A resource that would land on one of them stops the build.
///
/// The plugins are discovered here rather than handed in. Every host copies its
/// resources through this one function, and the list it returns is what the APK
/// build turns into zip entries — a plugin's files that were not in it would be
/// copied into the staging directory and left out of the archive. Discovery is a
/// walk over `package.json`s that the caller has already done and that says the
/// same thing twice; asking four platform modules to remember an argument is the
/// version of this that goes wrong on the fifth.
pub fn copy(
    workspace: &Workspace,
    app: &Path,
    into: &Path,
    reserved: &[&str],
) -> Result<Vec<String>> {
    let plugins = crate::plugins::discover(workspace, app)?;
    let planned = plan(workspace, app, &plugins, reserved)?;
    if planned.is_empty() {
        return Ok(Vec::new());
    }
    let mut copied = Vec::new();
    for file in &planned {
        let destination = into.join(&file.relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&file.from, &destination)
            .with_context(|| format!("{} could not be copied into the app", file.from.display()))?;
        copied.push(file.relative.clone());
    }
    eprintln!("==> resources: {}", copied.len());
    for file in &planned {
        match &file.owner {
            Owner::App => eprintln!("    {}", file.relative),
            Owner::Plugin(package) => eprintln!("    {} (from {package})", file.relative),
        }
    }
    Ok(copied)
}

/// Every file under `from`, as `(where it is, what it is called inside the
/// bundle)`, sorted.
fn collect(root: &Path, from: &Path, found: &mut Vec<(PathBuf, String)>) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(from)
        .with_context(|| format!("{} could not be read", from.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    // Sorted so two machines building the same tree produce the same APK: the
    // order the filesystem hands directory entries back in is not the same on
    // APFS as on ext4, and it ends up as the order of the entries in the zip.
    entries.sort();

    for path in entries {
        let Some(name) = path.file_name().map(|name| name.to_string_lossy().into_owned()) else {
            continue;
        };
        // `.DS_Store` is the one that matters —the Finder writes it into any
        // directory somebody has opened— but the rule is all dotfiles: nothing
        // whose name starts with a dot was put there to be shipped, and one of
        // them inside a signed bundle is a signature that no longer verifies on
        // the next machine.
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, found)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("the walk never leaves the resources directory")
            .to_string_lossy()
            .replace('\\', "/");
        found.push((path, relative));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app with the plugins given as `(package, resource file names)`, plus
    /// its own resources. Returns the workspace root.
    fn tree(name: &str, own: &[&str], plugins: &[(&str, &[&str])]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("an-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let app = root.join("app");
        std::fs::create_dir_all(&app).unwrap();
        let dependencies: Vec<String> =
            plugins.iter().map(|(package, _)| format!("{package:?}:\"1\"")).collect();
        std::fs::write(
            app.join("package.json"),
            format!("{{\"name\":\"app\",\"dependencies\":{{{}}}}}", dependencies.join(",")),
        )
        .unwrap();
        for file in own {
            let path = app.join(DIR).join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "the app's").unwrap();
        }
        for (package, files) in plugins {
            let dir = app.join("node_modules").join(package);
            std::fs::create_dir_all(dir.join("res")).unwrap();
            std::fs::write(
                dir.join("package.json"),
                format!(
                    "{{\"name\":{package:?},\"angularNative\":{{\"module\":{:?},\
                     \"resources\":\"res\"}}}}",
                    package.trim_start_matches('@').replace(['/', '-'], "")
                ),
            )
            .unwrap();
            for file in *files {
                let path = dir.join("res").join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, *package).unwrap();
            }
        }
        root
    }

    fn checked(root: &Path) -> Result<()> {
        let workspace = Workspace { root: root.to_owned(), project: None };
        let app = Path::new("app");
        let plugins = crate::plugins::discover(&workspace, app)?;
        check(&workspace, app, &plugins)
    }

    /// Two plugins shipping one file name. Neither of them wrote the other, so
    /// the only thing the message can be worth is naming both.
    #[test]
    fn two_plugins_shipping_one_resource_name_say_which_two() {
        let root = tree(
            "res-clash",
            &[],
            &[("@x/one", &["icon.png"] as &[&str]), ("@x/two", &["icon.png"])],
        );
        let error = checked(&root).unwrap_err().to_string();
        assert!(error.contains("@x/one"), "{error}");
        assert!(error.contains("@x/two"), "{error}");
        assert!(error.contains("icon.png"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The app's own file and a plugin's. The app is the thing being built, so
    /// it is named first, but it is still an error and not a silent win.
    #[test]
    fn a_plugin_landing_on_the_apps_own_name_is_refused() {
        let root = tree("res-app-clash", &["icon.png"], &[("@x/one", &["icon.png"] as &[&str])]);
        let error = checked(&root).unwrap_err().to_string();
        assert!(error.contains("@x/one"), "{error}");
        assert!(error.contains("the app's own"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `main.js` is written by every host into the same directory. A plugin
    /// shipping one would replace the app's code with a picture.
    #[test]
    fn a_plugin_cannot_ship_a_name_the_build_writes() {
        let root = tree("res-reserved", &[], &[("@x/one", &["main.js"] as &[&str])]);
        let error = checked(&root).unwrap_err().to_string();
        assert!(error.contains("@x/one"), "{error}");
        assert!(error.contains("main.js"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// And the case that is not an error: two plugins and an app, all with
    /// different names, all of them copied into one directory.
    #[test]
    fn the_apps_files_and_the_plugins_end_up_together() {
        let root = tree(
            "res-together",
            &["logo.png"],
            &[("@x/one", &["one.png"] as &[&str]), ("@x/two", &["nested/two.png"])],
        );
        let workspace = Workspace { root: root.clone(), project: None };
        let into = root.join("bundle");
        std::fs::create_dir_all(&into).unwrap();
        let copied = copy(&workspace, Path::new("app"), &into, &["main.js"]).unwrap();
        assert_eq!(copied, vec!["logo.png", "one.png", "nested/two.png"]);
        assert!(into.join("nested/two.png").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
