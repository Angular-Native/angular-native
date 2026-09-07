//! macOS's native modules.
//!
//! There are two sorts and they arrive by different roads. `device` is written
//! in Rust and compiled into the core, so it is there in every app. A *plugin*
//! is Swift an npm package brings, and it is there only if the app declared the
//! dependency: `plugin` is the postman that carries the call out to it and the
//! answer back, and it is the same mechanism `an-ios` uses, down to the symbol
//! names. See `plugin.rs`.
//!
//! What is still stated out loud is the *empty* case. A call to a module that
//! is not here has to be turned down saying why, not only that the name is
//! unknown, or the rejection reads like a typo and whoever gets it goes looking
//! for a spelling mistake that is not there. The reason is not the same one it
//! used to be —there is a registry now— so [`absent_note`] builds it from what
//! this particular `.app` actually has: no plugins at all, or these ones and
//! not the one that was asked for.

mod device;
mod plugin;

pub use device::DeviceModule;
pub use plugin::{host_plugins, pump, registered_names};

/// Why a name that is not in the registry may still be a name somebody wrote in
/// good faith.
///
/// It is appended to the rejection. The two branches are different situations
/// and they get different sentences: an app with no plugins at all is one where
/// the `package.json` never declared the dependency, and an app with three
/// plugins that was asked for a fourth is one where the name is wrong or the
/// dependency is missing from that one app.
pub fn absent_note() -> String {
    let names = registered_names();
    if names.is_empty() {
        return "this .app carries no plugins, so only the modules compiled into the core exist \
                here. A plugin is an npm dependency of the app: declare it and `an` links its \
                Swift in. See https://angular-native.github.io/extending/plugins/"
            .to_owned();
    }
    format!(
        "the plugins in this .app are {}, and only those plus the modules compiled into the core \
         exist here. See https://angular-native.github.io/extending/plugins/",
        names.join(", ")
    )
}
