//! Información del dispositivo. Es el módulo de referencia: pequeño, síncrono,
//! y muestra el camino completo desde una plantilla Angular hasta UIKit.

use an_bridge::native_module;
use objc2::MainThreadMarker;
use objc2_foundation::NSLocale;
use objc2_ui_kit::{UIDevice, UIScreen};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    platform: &'static str,
    system_version: String,
    model: String,
    scale: f64,
    locale: String,
}

pub struct DeviceModule {
    mtm: MainThreadMarker,
}

impl DeviceModule {
    pub fn new(mtm: MainThreadMarker) -> Self {
        DeviceModule { mtm }
    }
}

native_module! {
    DeviceModule as "device" {
        fn info(&mut self, _args: ()) -> Result<DeviceInfo, String> {
            let device = UIDevice::currentDevice(self.mtm);
            let screen = UIScreen::mainScreen(self.mtm);
            let locale = NSLocale::currentLocale();
            Ok(DeviceInfo {
                platform: "ios",
                system_version: device.systemVersion().to_string(),
                model: device.model().to_string(),
                scale: screen.scale(),
                locale: unsafe { locale.localeIdentifier() }.to_string(),
            })
        }
    }
}
