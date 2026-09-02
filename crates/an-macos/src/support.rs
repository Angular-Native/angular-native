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
    /// Lo que la primitiva pide sí se cumple, pero no con una vista del árbol:
    /// macOS lo pone en otro sitio. El texto dice dónde.
    ///
    /// No es un `Missing` con buenas palabras. Un `Missing` deja a la
    /// plantilla sin lo que pidió; esto se lo da donde la plataforma lo tiene,
    /// que en un Mac casi siempre está fuera de la ventana de contenido: la
    /// barra de título, la barra de menús, el Dock.
    Elsewhere(&'static str),
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
    // La cabecera de navegación de un Mac es la barra de título de la ventana.
    // No se dibuja otra dentro del contenido —serían dos—, pero tampoco se
    // tira lo que la plantilla escribió: el `[title]` va a parar al título de
    // la ventana, que es donde un usuario de Mac lo busca. Lo pone
    // `AppKitHost::apply_window_title`. El nodo mide cero, así que no deja
    // hueco donde no hay nada. Ver `controls.rs`.
    (
        NodeKind::NavigationBar,
        Support::Elsewhere("la barra de título de la ventana: el [title] acaba ahí"),
    ),
    // MapKit nativo no pide clave —esa es MapKit JS, que es otro producto— y
    // en macOS `MKMapView` hereda de `NSView`, así que entra en el árbol como
    // una vista más. Enseñar dónde estás sí pide permiso, y eso es `showsUser`.
    (NodeKind::MapView, Support::Native("MKMapView")),
    // En AppKit sí hay vista de vídeo, cosa que en UIKit no: `AVPlayerView`
    // **es** una `NSView` y trae los controles del sistema. Sale más barato
    // que en iOS, donde hay que contener un `AVPlayerViewController`.
    (NodeKind::VideoView, Support::Native("AVPlayerView")),
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
    "swipeUp", "swipeDown", "hover", "layout", "safeArea", "back", "refresh", "scroll", "load",
    "change", "input", "focus", "blur", "submit", "select", "dismiss",
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
        (kind, "swipeLeft" | "swipeRight" | "swipeUp" | "swipeDown") if !catches_swipe(kind) => {
            Some(
                "el deslizamiento de AppKit no es un reconocedor que se le cuelgue a una vista: \
                 es un evento que sube por la cadena de responder, y solo lo puede recoger una \
                 vista de este host. Un control del sistema no se puede subclasear con la app en \
                 marcha; ponlo en el <an-view> que lo envuelve, que sí lo recibe",
            )
        }
        (NodeKind::NavigationBar, "back") => Some(
            "la cabecera de este host es la barra de título de la ventana, y una barra de título \
             no tiene botón de atrás: en un Mac se vuelve con el menú o con un botón de la app",
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

/// Una dirección de deslizamiento suscrita.
///
/// Se guardan en un mapa de bits porque cada dirección es su propia salida:
/// escuchar solo `swipeLeft` no tiene por qué entregar las otras tres.
pub const SWIPE_LEFT: u8 = 1 << 0;
pub const SWIPE_RIGHT: u8 = 1 << 1;
pub const SWIPE_UP: u8 = 1 << 2;
pub const SWIPE_DOWN: u8 = 1 << 3;

/// El bit que le toca a cada nombre de evento, si es uno de los cuatro.
pub fn swipe_bit(event: &str) -> Option<u8> {
    Some(match event {
        "swipeLeft" => SWIPE_LEFT,
        "swipeRight" => SWIPE_RIGHT,
        "swipeUp" => SWIPE_UP,
        "swipeDown" => SWIPE_DOWN,
        _ => return None,
    })
}

/// Hacia dónde fue un deslizamiento, a partir de los deltas del `NSEvent`.
///
/// La correspondencia entre el signo y la dirección no es una suposición y no
/// se puede comprobar sin un trackpad y una mano encima, así que está donde se
/// puede leer y probar sin ninguna de las dos cosas. La escribe Apple en
/// `NSEvent.h`, en el comentario de `deltaX`:
///
/// > A non-0 deltaX will represent a horizontal swipe, -1 for swipe right and
/// > 1 for swipe left. A non-0 deltaY will represent a vertical swipe, -1 for
/// > swipe down and 1 for swipe up.
///
/// El horizontal manda sobre el vertical cuando llegan los dos, que en la
/// práctica no pasa: el sistema manda un eje por gesto.
pub fn swipe_direction(delta_x: f64, delta_y: f64) -> Option<(u8, &'static str)> {
    if delta_x != 0.0 {
        return Some(if delta_x < 0.0 {
            (SWIPE_RIGHT, "swipeRight")
        } else {
            (SWIPE_LEFT, "swipeLeft")
        });
    }
    if delta_y != 0.0 {
        return Some(if delta_y < 0.0 {
            (SWIPE_DOWN, "swipeDown")
        } else {
            (SWIPE_UP, "swipeUp")
        });
    }
    None
}

/// Primitivas cuya vista en este host la crea el propio host, y no AppKit.
///
/// Importa para una sola cosa, y por eso está aquí y no escondida en `host.rs`:
/// **el deslizamiento**. AppKit no tiene reconocedor de deslizamiento, pero sí
/// tiene el gesto: llega como `swipeWithEvent:` a la cadena de responder, y una
/// clase de Objective-C solo puede atenderlo si el método está en ella. Las
/// vistas de esta lista son `AnFlippedView` —ver `flipped.rs`— y lo tienen; un
/// `NSButton` es del sistema y no se le puede añadir un método con la app en
/// marcha.
///
/// No es un agujero: un `swipeWithEvent:` que un control no atiende sube al
/// siguiente en la cadena, que es su vista padre. O sea que un deslizamiento
/// encima de un botón acaba llegando al `<an-view>` que lo envuelve, que es
/// donde una plantilla lo pone casi siempre.
pub fn catches_swipe(kind: NodeKind) -> bool {
    matches!(
        kind,
        // El `<an-scroll-view>` lo recoge por su documento, que también es
        // nuestro; el `NSScrollView` de fuera es del sistema.
        NodeKind::View | NodeKind::StackView | NodeKind::ScrollView | NodeKind::Modal
    )
}
