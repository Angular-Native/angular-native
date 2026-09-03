//! macOS's native modules.
//!
//! There is one, `device`, and it is compiled into the core. What there is not
//! —and it is the difference from `an-ios` and `an-android`— is the plugin
//! registry: nothing here carries a call out to Swift, so the modules an npm
//! package brings do not exist on this host.
//!
//! That absence is stated twice, on purpose and in two different moments.
//! `an-cli`'s `macos::reject_plugins` stops the build of an app that depends on
//! a plugin, which is while it can still be fixed cheaply. And `ABSENT_NOTE` is
//! handed to the registry so that a call to a module that is not here —a name
//! typed straight into `callNative`, a plugin reached without declaring it— is
//! turned down naming what was asked for *and* saying why it is not there.
//! Without the second one the rejection reads like a typo, and whoever gets it
//! goes looking for a spelling mistake that is not there.

mod device;

pub use device::DeviceModule;

/// Why a name that is not in the registry may still be a name somebody wrote in
/// good faith. It is appended to the rejection; see the module header.
pub const ABSENT_NOTE: &str = "the macOS host does not load plugins yet, so only the modules \
     compiled into the core exist here. See https://angular-native.dev/extending/plugins/";
