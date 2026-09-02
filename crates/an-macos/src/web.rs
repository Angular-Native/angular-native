//! `WKWebView`, declarada a mano.
//!
//! En macOS `objc2-web-kit` sí cubre esta clase —hereda de `NSView`, que es lo
//! que el generador entiende—, pero traerse el crate entero por cuatro métodos
//! son cuatrocientas clases más que compilar en cada build del host. Se declara
//! aquí igual que en iOS, con las mismas cuatro firmas, y `objc2` sigue
//! comprobándolas contra las de verdad en tiempo de ejecución.

use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSResponder, NSView};
use objc2_foundation::{NSObject, NSString, NSURL, NSURLRequest};

// El framework hay que enlazarlo: nadie más lo hace por nosotros.
#[link(name = "WebKit", kind = "framework")]
unsafe extern "C" {}

extern_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct WKWebView;
);

impl WKWebView {
    /// Una web nueva, vacía. `new` no se puede declarar en `extern_methods!`
    /// —no lleva argumentos y el marcador de hilo no es uno de ellos—, así que
    /// se llama por el camino normal de objc2.
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let _ = mtm;
        unsafe { objc2::msg_send![<Self as ClassType>::class(), new] }
    }

    extern_methods!(
        // Los cuatro devuelven un `WKNavigation *` que aquí no hace falta para
        // nada, pero hay que declararlo: objc2 comprueba la firma contra la de
        // verdad y decir `void` donde hay un objeto aborta el proceso.
        #[unsafe(method(loadRequest:))]
        pub fn loadRequest(&self, request: &NSURLRequest) -> Option<Retained<NSObject>>;

        #[unsafe(method(loadHTMLString:baseURL:))]
        pub fn loadHTMLString_baseURL(
            &self,
            html: &NSString,
            base: Option<&NSURL>,
        ) -> Option<Retained<NSObject>>;

        #[unsafe(method(reload))]
        pub fn reload(&self) -> Option<Retained<NSObject>>;

        #[unsafe(method(goBack))]
        pub fn goBack(&self) -> Option<Retained<NSObject>>;
    );
}
