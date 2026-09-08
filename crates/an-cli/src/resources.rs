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

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

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

/// Copies the app's resources into `into`, keeping whatever directory structure
/// they had, and returns their paths relative to it in the order they were
/// copied. Nothing there, nothing said, empty list.
///
/// `reserved` is the list of names the build writes into that same directory
/// itself. A resource that would land on one of them stops the build.
pub fn copy(
    workspace: &Workspace,
    app: &Path,
    into: &Path,
    reserved: &[&str],
) -> Result<Vec<String>> {
    let from = dir(workspace, app);
    if !from.is_dir() {
        return Ok(Vec::new());
    }
    let mut copied = Vec::new();
    walk(&from, &from, into, reserved, &mut copied)?;
    if copied.is_empty() {
        return Ok(copied);
    }
    eprintln!("==> resources: {} from {}", copied.len(), from.display());
    for name in &copied {
        eprintln!("    {name}");
    }
    Ok(copied)
}

fn walk(
    root: &Path,
    from: &Path,
    into: &Path,
    reserved: &[&str],
    copied: &mut Vec<String>,
) -> Result<()> {
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
        let relative = path
            .strip_prefix(root)
            .expect("the walk never leaves the resources directory")
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            std::fs::create_dir_all(into.join(&relative))?;
            walk(root, &path, into, reserved, copied)?;
            continue;
        }
        if reserved.contains(&relative.as_str()) {
            bail!(
                "{} cannot be a resource: {relative} is the name the build writes into the app \
                 itself, and the copy would replace it.\n\
                 Rename it — a resource keeps the name it has here, and that is the name the \
                 template asks for.",
                path.display()
            );
        }
        let destination = into.join(&relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&path, &destination)
            .with_context(|| format!("{} could not be copied into the app", path.display()))?;
        copied.push(relative);
    }
    Ok(())
}
