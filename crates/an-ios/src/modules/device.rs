//! Information about the device.
//!
//! The values are read once only, at startup and on the main thread: `UIDevice`
//! and `UIScreen` can only be touched there, and the module lives on the
//! engine's thread. Since on top of that they do not change over the life of
//! the process, there is nothing to lose by capturing them.
//!
//! A module that does need to talk to UIKit on every call will have to queue
//! the work on the main thread and answer from there with its `Responder`,
//! which can be kept and resolved later for exactly that purpose.

use an_bridge::native_module;

/// Where this is running. It comes from the `cfg` and not from a run-time
/// check: the family is settled at compile time, and asking about it later
/// would be room to get it wrong.
#[cfg(target_os = "tvos")]
const PLATFORM: &str = "tvos";
#[cfg(target_os = "visionos")]
const PLATFORM: &str = "visionos";
#[cfg(not(any(target_os = "tvos", target_os = "visionos")))]
const PLATFORM: &str = "ios";
use objc2::MainThreadMarker;
use objc2_foundation::NSLocale;
use objc2_ui_kit::UIDevice;
// `UIScreen` is marked `API_UNAVAILABLE(visionos)`: over there an app does not
// live on a screen, it lives in a window the user places in the room and
// resizes whenever they like. See `docs/visionos.md`.
#[cfg(not(target_os = "visionos"))]
use objc2_ui_kit::UIScreen;
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
        let device = UIDevice::currentDevice(mtm);
        let locale = NSLocale::currentLocale();

        // The screen's scale. On visionOS there is none to ask for: the app
        // is drawn for two eyes and at whatever distance the user puts the
        // window, and the system exposes no number that means the same thing
        // as this one. It comes out 0 and that is said in the documentation,
        // rather than inventing a 2.0 somebody would end up computing pixels
        // with.
        #[cfg(not(target_os = "visionos"))]
        let scale = UIScreen::mainScreen(mtm).scale();
        #[cfg(target_os = "visionos")]
        let scale = 0.0;

        DeviceModule {
            info: DeviceInfo {
                // The three families share a host, but they are not the same
                // place: on a television nothing is touched, in the headset
                // the window is not a screen, and an app that wants to adapt
                // needs to tell them apart. The client's type declares all
                // three.
                platform: PLATFORM,
                system_version: device.systemVersion().to_string(),
                model: device.model().to_string(),
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
