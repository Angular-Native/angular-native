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
use objc2_ui_kit::UIDevice;
// `UIScreen` está marcado `API_UNAVAILABLE(visionos)`: allí una app no vive en
// una pantalla, vive en una ventana que el usuario coloca en la habitación y
// redimensiona cuando quiere. Ver `docs/visionos.md`.
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
    /// Se llama desde el hilo principal, antes de arrancar el worker.
    pub fn capture(mtm: MainThreadMarker) -> Self {
        let device = UIDevice::currentDevice(mtm);
        let locale = NSLocale::currentLocale();

        // La escala de pantalla. En visionOS no hay ninguna que preguntar: la
        // app se dibuja para dos ojos y a la distancia a la que el usuario
        // ponga la ventana, y el sistema no expone un número que signifique lo
        // mismo que aquí. Sale 0 y se dice en la documentación, en vez de
        // inventarse un 2.0 que alguien acabaría usando para calcular píxeles.
        #[cfg(not(target_os = "visionos"))]
        let scale = UIScreen::mainScreen(mtm).scale();
        #[cfg(target_os = "visionos")]
        let scale = 0.0;

        DeviceModule {
            info: DeviceInfo {
                // Sigue diciendo "ios" en las tres familias, y eso es un
                // hueco conocido: una app que quiera adaptarse a la tele no
                // tiene hoy forma de saber que está en una.
                //
                // No se arregla aquí porque el arreglo no está aquí: el tipo
                // `NativeDeviceInfo.platform` de `packages/primitives` declara
                // la unión `'ios' | 'android'`, y devolver "tvos" sería
                // devolver algo que el tipo del cliente dice que no puede
                // llegar. Ver docs/tvos.md.
                platform: "ios",
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
