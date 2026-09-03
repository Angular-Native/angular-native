//! Information about the Mac.
//!
//! It answers the same five fields as `an-ios`'s module, because the contract
//! —`DeviceInfo` in `packages/platform-native/src/native-modules.ts`— is one for
//! every host and an app that reads it should not have to know which host it is
//! talking to. What it does not do is answer them as if this were a phone.
//!
//! Two of the five mean something different on a desktop, and rather than
//! quietly returning the nearest-looking number, each is said out loud where it
//! is read:
//!
//! - **`model`** is the hardware identifier —`Mac15,3`, `MacBookPro18,3`—
//!   because AppKit has no `UIDevice.model`, which on a phone answers the class
//!   of device ("iPhone", "iPad") and here would have to be the word "Mac" for
//!   every Mac ever made. The identifier is the closest thing that is actually
//!   information.
//! - **`scale`** is the backing factor of the screen the app *started on*. A Mac
//!   window is not stuck to one screen: drag it to a non-Retina display and the
//!   real factor becomes 1.0 while this still says 2.0. Reading it on every call
//!   would not help either —`NSScreen` may only be touched on the main thread
//!   and this module lives on the engine's— so what is done is what can be done
//!   honestly: it is captured once and the caveat is in the documentation. With
//!   no screen at all —a Mac running headless— it comes back 0.0 rather than an
//!   invented 2.0, the same way visionOS answers it.
//!
//! The other three —`platform`, `systemVersion`, `locale`— mean exactly what
//! they mean everywhere else.
//!
//! Everything is read once, at startup and on the main thread, and travels
//! already resolved: none of it changes over the life of the process, so there
//! is nothing to lose by capturing it.

use std::ffi::{c_char, c_void, CString};

use an_bridge::native_module;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSLocale, NSProcessInfo};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    platform: &'static str,
    system_version: String,
    model: String,
    scale: f64,
    locale: String,
}

pub struct DeviceModule {
    info: DeviceInfo,
}

impl DeviceModule {
    /// Called from the main thread, before the worker is started.
    pub fn capture(mtm: MainThreadMarker) -> Self {
        let process = NSProcessInfo::processInfo();
        let version = process.operatingSystemVersion();
        let locale = NSLocale::currentLocale();

        // Built from the three numbers and not from
        // `operatingSystemVersionString`, which reads "Version 15.3.1 (Build
        // 24D70)": that string is meant for a human and iOS's `systemVersion`
        // is "18.3", so an app comparing the two would be comparing prose with
        // a version number.
        let system_version =
            format!("{}.{}.{}", version.majorVersion, version.minorVersion, version.patchVersion);

        // No screen means no factor. See the module header: a 2.0 made up here
        // is a 2.0 somebody computes pixels with later.
        let scale = NSScreen::mainScreen(mtm).map(|screen| screen.backingScaleFactor()).unwrap_or(0.0);

        DeviceModule {
            info: DeviceInfo {
                platform: "macos",
                system_version,
                model: hardware_model().unwrap_or_default(),
                scale,
                locale: unsafe { locale.localeIdentifier() }.to_string(),
            },
        }
    }
}

native_module! {
    DeviceModule as "device" {
        fn info(&mut self, _args: ()) -> Result<DeviceInfo, String> {
            Ok(self.info.clone())
        }
    }
}

// `hw.model` is not in AppKit or in Foundation: it is a `sysctl`, which is where
// the Mac's model identifier has always lived. It is declared here rather than
// pulling in `libc` for one call — it is in libSystem, which every macOS binary
// already links.
unsafe extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> i32;
}

/// The model identifier, `Mac15,3` and the like.
///
/// `None` if the kernel does not answer, which should not happen and is not
/// covered up if it does: the field comes back empty and an empty string is
/// visibly not a model.
fn hardware_model() -> Option<String> {
    let name = CString::new("hw.model").ok()?;
    // Asked twice, which is how `sysctl` is used: the first call with a null
    // buffer only fills in the length.
    let mut length: usize = 0;
    let asked = unsafe {
        sysctlbyname(name.as_ptr(), std::ptr::null_mut(), &mut length, std::ptr::null_mut(), 0)
    };
    if asked != 0 || length == 0 {
        return None;
    }
    let mut buffer = vec![0u8; length];
    let read = unsafe {
        sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast::<c_void>(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }
    // It comes back nul-terminated and the length includes the nul.
    buffer.truncate(length);
    while buffer.last() == Some(&0) {
        buffer.pop();
    }
    String::from_utf8(buffer).ok()
}

#[cfg(test)]
mod tests {
    /// The model identifier really comes out of the kernel.
    ///
    /// It is the one of the five fields that is not a Foundation call, and it
    /// is the one that can come back empty without anything failing: a `sysctl`
    /// that does not answer gives a `None` here and an empty string on the other
    /// side, which looks exactly like a Mac that will not say what it is. It
    /// needs no window and no main thread, so it runs in `cargo test`.
    #[test]
    fn the_kernel_says_what_this_mac_is() {
        let model = super::hardware_model().expect("sysctl hw.model has to answer on a Mac");
        // "MacBookPro18,3", "Mac15,3", "iMac19,1": what they share is the
        // comma, not the prefix.
        assert!(model.contains(','), "hw.model came back as {model:?}");
    }
}
