//! La vista contenedora de este host: una `NSView` con el origen arriba.
//!
//! Es la diferencia más grande entre AppKit y UIKit, y no es cosmética. En
//! UIKit el origen de una vista está arriba a la izquierda y la `y` crece hacia
//! abajo; en AppKit está **abajo** a la izquierda y la `y` crece hacia arriba.
//! El núcleo calcula el layout con taffy, que es CSS, y CSS es como UIKit.
//!
//! Hay dos formas de arreglarlo:
//!
//! 1. Dar la vuelta a cada marco en `set_layout`, restando de la altura del
//!    padre. Exige que el host sepa el alto del padre de cada nodo en el
//!    momento de colocarlo, y ese alto puede llegar *después* que el del hijo:
//!    el core manda los marcos en el orden del árbol, no de fuera adentro. El
//!    resultado sería una vista bien colocada y otra descolocada según el
//!    orden, que es la clase de fallo que no se ve hasta que se ve.
//! 2. Decirle a AppKit que estas vistas van al revés. `isFlipped` es
//!    exactamente eso, y es una propiedad de la vista *padre*: quien decide
//!    cómo se interpretan los marcos de los hijos es el contenedor.
//!
//! Se hace lo segundo. Como cada contenedor que monta este host es de esta
//! clase —vista, pila, contenido del scroll, capa del modal— cualquier hijo,
//! sea un `NSButton` del sistema o no, recibe su marco en coordenadas de arriba
//! abajo sin que haya que convertir nada en ningún sitio.
//!
//! El recorte va aparte, con `clipsToBounds`: en UIKit es una propiedad de la
//! vista y en AppKit hay que pedir capa y decírselo a ella.

use objc2::rc::Retained;
use objc2::{define_class, msg_send, MainThreadOnly};
use objc2_app_kit::NSView;
use objc2_foundation::NSObjectProtocol;

define_class!(
    // SAFETY:
    // - `NSView` no impone requisitos a sus subclases más allá de vivir en el
    //   hilo principal, que el `MainThreadOnly` garantiza.
    // - `AnFlippedView` no implementa `Drop`.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnFlippedView"]
    pub struct FlippedView;

    unsafe impl NSObjectProtocol for FlippedView {}

    impl FlippedView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl FlippedView {
    pub fn new(mtm: objc2::MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        unsafe { msg_send![this, init] }
    }

    /// Recorta a los hijos que se salgan. En UIKit es `clipsToBounds`; aquí hay
    /// que darle capa a la vista y decírselo a la capa.
    pub fn clip_to_bounds(&self) {
        self.setWantsLayer(true);
        if let Some(layer) = unsafe { self.layer() } {
            layer.setMasksToBounds(true);
        }
    }
}
