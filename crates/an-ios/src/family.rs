//! Qué trae de verdad cada familia de UIKit.
//!
//! `an-ios` compila para iOS, tvOS y visionOS. Las tres traen UIKit y las tres
//! montan `UIView` con marcos absolutos, así que el host es uno solo. Lo que no
//! es uno solo es el catálogo de controles: `UISwitch`, `UISlider`, `UIStepper`,
//! `UIDatePicker` y `WKWebView` están marcados `API_UNAVAILABLE(tvos)` en el
//! SDK.
//!
//! El compilador no ayuda a notarlo. `objc2-ui-kit` genera los enlaces para
//! todas las plataformas Apple sin mirar la anotación de disponibilidad, así
//! que `UISwitch::new(mtm)` **compila** para tvOS y lo que falla es la búsqueda
//! de la clase en tiempo de ejecución, ya dentro del simulador y con el proceso
//! abortando. Por eso esta lista está escrita a mano: es la anotación del SDK
//! traída al Rust, y es lo que separa «no se puede» de «se cierra sin decir por
//! qué».

use an_core::NodeKind;

/// El nombre de la familia, para los mensajes.
pub const NAME: &str = if cfg!(target_os = "tvos") {
    "tvOS"
} else if cfg!(target_os = "visionos") {
    "visionOS"
} else {
    "iOS"
};

/// Por qué esta familia no puede montar esta primitiva, o `None` si sí puede.
///
/// El motivo se escribe entero porque acaba en el log del dispositivo, que es
/// donde alguien lo va a leer sin este fichero delante.
pub fn missing_kind(kind: NodeKind) -> Option<&'static str> {
    #[cfg(target_os = "tvos")]
    {
        match kind {
            // Los tres controles de valor de iOS no existen en el SDK de tvOS.
            // No es que se vean distintos: la clase no está en UIKit.
            NodeKind::Switch => Some(
                "UISwitch no existe en tvOS. La pantalla de una tele se maneja con el mando, y \
                 ahí un interruptor es una fila enfocable que se pulsa; no hay control del \
                 sistema equivalente",
            ),
            NodeKind::Slider => Some(
                "UISlider no existe en tvOS. Lo más cercano que sí trae la plataforma es \
                 UIProgressView, que solo enseña un valor: no se puede arrastrar",
            ),
            NodeKind::Stepper => Some("UIStepper no existe en tvOS"),
            NodeKind::DatePicker => Some(
                "UIDatePicker no existe en tvOS. El sistema pide las fechas con una pantalla \
                 propia, no con un control que quepa en un marco",
            ),
            // WebKit entero: el SDK de tvOS no trae el framework.
            NodeKind::WebView => {
                Some("WebKit no forma parte del SDK de tvOS: no hay WKWebView que montar")
            }
            _ => None,
        }
    }
    #[cfg(not(target_os = "tvos"))]
    {
        let _ = kind;
        None
    }
}

/// Lo dice una vez y no lo repite.
///
/// El host llama aquí desde `create`, y `create` corre cada vez que el árbol
/// da de alta un nodo: sin el filtro, un `@for` de veinte interruptores
/// llenaría el log veinte veces y el aviso dejaría de leerse.
///
/// El estado es `thread_local` y no un `Mutex` porque todo esto vive en el hilo
/// principal —UIKit no admite otra cosa— y así no hay ningún candado que tomar
/// en mitad de un frame.
pub fn report(what: &str, why: &str) {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::io::Write;

    thread_local! {
        static DICHO: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    }

    let nuevo = DICHO.with(|dicho| dicho.borrow_mut().insert(what.to_owned()));
    if !nuevo {
        return;
    }
    // A mano y con `flush`, por lo mismo que el gancho de pánico de `ffi.rs`:
    // si el proceso se cierra justo después, lo que quedó en el búfer no llega
    // a salir y el aviso se pierde justo cuando más falta hace.
    let mut salida = std::io::stderr().lock();
    let _ = writeln!(salida, "angular-native: {what} no está disponible en {NAME}: {why}");
    let _ = salida.flush();
}
