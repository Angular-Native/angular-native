//! Información del dispositivo.
//!
//! Los valores se leen una sola vez, al arrancar y en el hilo principal:
//! `UIDevice` y `UIScreen` solo se pueden tocar ahí, y el módulo vive en el
//! hilo del motor. Como además no cambian durante la vida del proceso, no hay
//! nada que perder por capturarlos.
//!
//! Un módulo que sí necesite hablar con UIKit en cada llamada tendrá que
//! encolar el trabajo en el hilo principal y contestar desde allí con su
//! `Responder`, que para eso se puede guardar y resolver más tarde.

use an_bridge::native_module;
use objc2::MainThreadMarker;
use objc2_foundation::NSLocale;
use objc2_ui_kit::{UIDevice, UIScreen};
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
    /// Se llama desde el hilo principal, antes de arrancar el worker.
    pub fn capture(mtm: MainThreadMarker) -> Self {
        let device = UIDevice::currentDevice(mtm);
        let screen = UIScreen::mainScreen(mtm);
        let locale = NSLocale::currentLocale();
        DeviceModule {
            info: DeviceInfo {
                platform: "ios",
                system_version: device.systemVersion().to_string(),
                model: device.model().to_string(),
                scale: screen.scale(),
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
