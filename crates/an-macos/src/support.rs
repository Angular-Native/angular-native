//! Qué pinta este host y qué no, dicho una vez y en un solo sitio.
//!
//! El resto del proyecto tiene la regla de que nada falle en silencio, y en un
//! host la forma más fácil de romperla es un `_ => {}` en el `match` de
//! `create`: la primitiva se monta como una caja vacía, ocupa su sitio en el
//! layout y no se ve. Ni error, ni traza, ni nada que mirar.
//!
//! Así que el inventario está aquí, fuera de `cfg(target_os = "macos")`, y no
//! dentro del `match`. Con eso se consiguen tres cosas:
//!
//! 1. `create` puede recorrer el enum entero sin comodín: si alguien añade un
//!    `NodeKind` al núcleo, este fichero deja de compilar.
//! 2. Una primitiva que macOS no cubre lo dice por la salida de error la
//!    primera vez que aparece, con el motivo, en vez de no verse.
//! 3. `scripts/check-macos.sh` lee esta tabla y la compara con el enum del
//!    núcleo, así que la lista tampoco se queda atrás sin que nadie se entere.

use an_core::NodeKind;

/// Con qué dibuja macOS una primitiva.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Support {
    /// Hay un control del sistema que es exactamente esto. El texto es el
    /// nombre de la clase de AppKit, que es lo que sale en el informe.
    Native(&'static str),
    /// No existe tal cual en AppKit y se arma con vistas del sistema. Se dice
    /// cuál es cuál, igual que hace Android con la barra de pestañas.
    Assembled(&'static str),
    /// macOS no lo trae y no se imita. El texto explica por qué.
    Missing(&'static str),
}

/// Todos los `NodeKind` que se pueden montar, con lo que macOS pone detrás.
///
/// `RawText` no está: es interno y nunca llega a ser una vista.
pub const SUPPORT: &[(NodeKind, Support)] = &[
    (NodeKind::View, Support::Native("NSView")),
    (NodeKind::Text, Support::Native("NSTextField")),
    (NodeKind::Image, Support::Native("NSImageView")),
    (NodeKind::Icon, Support::Native("NSImageView + SF Symbols")),
    (NodeKind::ScrollView, Support::Native("NSScrollView")),
    (NodeKind::TextInput, Support::Native("NSTextField")),
    (NodeKind::TextEditor, Support::Native("NSTextView")),
    (NodeKind::StackView, Support::Native("NSView")),
    (NodeKind::Button, Support::Native("NSButton")),
    (NodeKind::Switch, Support::Native("NSSwitch")),
    (NodeKind::Slider, Support::Native("NSSlider")),
    (NodeKind::ActivityIndicator, Support::Native("NSProgressIndicator (spinning)")),
    (NodeKind::ProgressBar, Support::Native("NSProgressIndicator (bar)")),
    (NodeKind::SegmentedControl, Support::Native("NSSegmentedControl")),
    (NodeKind::Stepper, Support::Native("NSStepper")),
    (NodeKind::SearchBar, Support::Native("NSSearchField")),
    (NodeKind::Picker, Support::Native("NSPopUpButton")),
    (NodeKind::DatePicker, Support::Native("NSDatePicker")),
    (NodeKind::Alert, Support::Native("NSAlert")),
    (NodeKind::WebView, Support::Native("WKWebView")),
    // Una capa por encima del contenido, no una ventana aparte. En macOS lo
    // modal de verdad es una hoja (`beginSheet:`) o un `NSPanel`, y las dos
    // sacan el contenido de la ventana donde el layout lo colocó. Como el
    // núcleo ya calcula el modal a pantalla completa, una vista encima da el
    // mismo resultado sin pelearse con dos sistemas de coordenadas. El
    // diálogo del sistema —`<an-alert>`— sí es un `NSAlert` de verdad.
    (NodeKind::Modal, Support::Assembled("NSView por encima de la raíz")),
    // macOS no tiene barra de pestañas. Lo más parecido del sistema es un
    // `NSSegmentedControl`, que es justo lo que usan las apps de macOS para
    // cambiar de sección, así que se arma con él y se dice. `NSTabView` no
    // vale: es la pestaña de documento, con su marco y su fondo.
    (NodeKind::TabBar, Support::Assembled("NSSegmentedControl")),
    // La cabecera de navegación de macOS es la barra de título de la ventana,
    // que no vive en el árbol de vistas: la pone el shell. Poner una barra
    // dentro del contenido sería dibujar una segunda cabecera debajo de la de
    // verdad, que es lo que este proyecto no hace.
    (
        NodeKind::NavigationBar,
        Support::Missing("en macOS la cabecera es la barra de título de la ventana, no una vista"),
    ),
    // MapKit existe en macOS, pero `MKMapView` pide clave de mapa y permisos
    // que este host todavía no gestiona; montar una vista en blanco sería peor
    // que no montarla.
    (NodeKind::MapView, Support::Missing("MKMapView todavía no está portado a este host")),
    // `AVPlayerView` es de AppKit y no es el `AVPlayerViewController` de iOS:
    // no es un port, es otro control. Todavía no está hecho.
    (NodeKind::VideoView, Support::Missing("AVPlayerView todavía no está portado a este host")),
];

/// Lo que macOS pone detrás de una primitiva.
pub fn support(kind: NodeKind) -> Option<Support> {
    SUPPORT.iter().find(|(k, _)| *k == kind).map(|(_, s)| *s)
}

/// Los nombres de evento que el framework sabe mandar.
///
/// Hace falta porque al host llegan dos clases de nombre. Uno es el de
/// `nativeEvent()` en `packages/primitives` —`press`, `change`, `scroll`— y ese
/// sí es una petición de verdad. El otro es el nombre de la *salida* de la
/// directiva —`onChange`, `valueChange`—: Angular registra también un oyente de
/// elemento por cada `(salida)` que aparece en una plantilla, y ese nombre no
/// le corresponde a ningún evento de plataforma en ninguno de los hosts.
///
/// Avisar de los segundos sería avisar en cada arranque de algo que funciona, y
/// un aviso que sale siempre es un aviso que nadie lee. Se avisa solo de los
/// primeros: de lo que alguien pidió de verdad y esta plataforma no da.
pub const KNOWN_EVENTS: &[&str] = &[
    "press", "doublePress", "longPress", "pan", "pinch", "rotate", "swipeLeft", "swipeRight",
    "swipeUp", "swipeDown", "layout", "safeArea", "back", "refresh", "scroll", "load", "change",
    "input", "focus", "blur", "submit", "select", "dismiss",
];

pub fn is_known_event(event: &str) -> bool {
    KNOWN_EVENTS.contains(&event)
}

/// Eventos que esta plataforma no puede entregar, con el motivo.
///
/// Se consulta al suscribirse, no al dispararse: una plantilla que pide
/// `(swipeLeft)` en un Mac tiene que enterarse al montar, no quedarse esperando
/// un evento que nunca va a llegar.
pub fn unsupported_event(kind: NodeKind, event: &str) -> Option<&'static str> {
    match (kind, event) {
        (_, "swipeLeft" | "swipeRight" | "swipeUp" | "swipeDown") => Some(
            "AppKit no tiene reconocedor de deslizamiento: el de dos dedos del trackpad llega \
             como scroll, no como gesto",
        ),
        (NodeKind::ScrollView, "refresh") => Some(
            "no hay «tirar para recargar» en escritorio: se recarga con un botón o con un \
             atajo, y eso es cosa de la app",
        ),
        (NodeKind::Modal, "dismiss") => Some(
            "el modal de este host es una capa que se enseña y se esconde con `visible`, no una \
             presentación del sistema: nunca se cierra sola, así que no hay nada que avisar",
        ),
        (NodeKind::StackView, "back") => Some(
            "el gesto de volver atrás desde el borde es de iOS; en escritorio se vuelve con el \
             menú o con un botón",
        ),
        // El área segura es el recorte de la pantalla —la muesca, la barra de
        // inicio—, y una ventana de escritorio no tiene nada de eso. Cero por
        // los cuatro lados es la respuesta correcta, no un fallo: se contesta
        // y no se avisa.
        _ => None,
    }
}
