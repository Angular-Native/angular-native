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

/// Dónde corre esto. Sale del `cfg` y no de una comprobación en tiempo de
/// ejecución: la familia se decide al compilar, y preguntarlo luego sería
/// poder equivocarse.
#[cfg(target_os = "tvos")]
const PLATAFORMA: &str = "tvos";
#[cfg(target_os = "visionos")]
const PLATAFORMA: &str = "visionos";
#[cfg(not(any(target_os = "tvos", target_os = "visionos")))]
const PLATAFORMA: &str = "ios";
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
                // Las tres familias comparten host, pero no son el mismo
                // sitio: en una tele no se toca, en el visor la ventana no es
                // una pantalla, y una app que quiera adaptarse necesita
                // distinguirlas. El tipo del cliente las declara todas.
                platform: PLATAFORMA,
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
