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
//!
//! ## Y aquí vive el deslizamiento
//!
//! AppKit no tiene `NSSwipeGestureRecognizer`, y eso llevó mucho tiempo a que
//! `(swipeLeft)` y sus tres hermanos avisaran de que no iban a llegar nunca.
//! Pero el gesto **sí existe**: no es un reconocedor que se le cuelgue a una
//! vista, es un evento —`NSEventTypeSwipe`— que el sistema manda por la cadena
//! de responder con `swipeWithEvent:`. Lo produce el mismo trackpad y con el
//! mismo criterio que usa cualquier app de Mac para pasar de página, así que el
//! umbral, el número de dedos y si el gesto cuenta o no los decide el sistema,
//! que es la regla de la casa.
//!
//! Lo que hace falta para recogerlo es que el método esté en la clase, y eso
//! solo se puede hacer con una clase nuestra. Por eso está aquí y no en
//! `events.rs` con los reconocedores: los demás gestos se enganchan a cualquier
//! vista, este solo a las de este fichero. Un control del sistema no lo recoge,
//! pero tampoco lo pierde: al no atenderlo, el evento sube al siguiente de la
//! cadena, que es su vista padre. Ver `support::catches_swipe`.

use std::cell::{Cell, RefCell};

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSView};
use objc2_foundation::NSObjectProtocol;

use crate::support::swipe_direction;

pub struct FlippedIvars {
    /// A quién avisar, cuando alguien ha pedido algún deslizamiento.
    swipe: RefCell<Option<(NodeId, EventQueue)>>,
    /// Direcciones suscritas. Cero es «nadie escucha», y entonces el evento se
    /// pasa a la cadena de responder tal cual llegó.
    swipe_mask: Cell<u8>,
}

define_class!(
    // SAFETY:
    // - `NSView` no impone requisitos a sus subclases más allá de vivir en el
    //   hilo principal, que el `MainThreadOnly` garantiza.
    // - `AnFlippedView` no implementa `Drop`.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnFlippedView"]
    #[ivars = FlippedIvars]
    pub struct FlippedView;

    unsafe impl NSObjectProtocol for FlippedView {}

    impl FlippedView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        /// El deslizamiento del trackpad.
        ///
        /// Qué dirección es cada signo lo dice `support::swipe_direction`, que
        /// está fuera de la plataforma justo para poder probarlo sin trackpad.
        #[unsafe(method(swipeWithEvent:))]
        fn swipe_with_event(&self, event: &NSEvent) {
            let ivars = self.ivars();
            let deltas = (unsafe { event.deltaX() }, unsafe { event.deltaY() });
            let Some((bit, name)) = swipe_direction(deltas.0, deltas.1) else {
                // Un deslizamiento sin dirección no es de nadie. Se pasa, que
                // es lo que haría la vista si no tuviera este método.
                return unsafe { msg_send![super(self), swipeWithEvent: event] };
            };

            if ivars.swipe_mask.get() & bit == 0 {
                // Esta vista no escucha esa dirección. Dejarlo aquí sería
                // tragárselo: el `<an-view>` de fuera dejaría de recibirlo solo
                // porque el de dentro existe.
                return unsafe { msg_send![super(self), swipeWithEvent: event] };
            }

            let Some((node, queue)) = ivars.swipe.borrow().clone() else {
                return unsafe { msg_send![super(self), swipeWithEvent: event] };
            };
            // Dónde estaba el puntero, en coordenadas de esta vista, igual que
            // en un `press`. La ventana da el punto en las suyas.
            let point = self.convertPoint_fromView(unsafe { event.locationInWindow() }, None);
            push_event(
                &queue,
                HostEvent {
                    target: node,
                    name: name.to_owned(),
                    payload: vec![
                        ("x".to_owned(), PropValue::Number(point.x)),
                        ("y".to_owned(), PropValue::Number(point.y)),
                    ],
                },
            );
        }
    }
);

impl FlippedView {
    pub fn new(mtm: objc2::MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm)
            .set_ivars(FlippedIvars { swipe: RefCell::new(None), swipe_mask: Cell::new(0) });
        unsafe { msg_send![super(this), init] }
    }

    /// Recorta a los hijos que se salgan. En UIKit es `clipsToBounds`; aquí hay
    /// que darle capa a la vista y decírselo a la capa.
    pub fn clip_to_bounds(&self) {
        self.setWantsLayer(true);
        if let Some(layer) = unsafe { self.layer() } {
            layer.setMasksToBounds(true);
        }
    }

    /// Empieza a entregar una dirección de deslizamiento.
    pub fn listen_swipe(&self, node: NodeId, queue: EventQueue, bit: u8) {
        let ivars = self.ivars();
        *ivars.swipe.borrow_mut() = Some((node, queue));
        ivars.swipe_mask.set(ivars.swipe_mask.get() | bit);
    }

    /// Deja de entregarla. Cuando no queda ninguna, se suelta también la cola:
    /// una vista que ya no escucha no tiene por qué retener nada.
    pub fn unlisten_swipe(&self, bit: u8) {
        let ivars = self.ivars();
        let mask = ivars.swipe_mask.get() & !bit;
        ivars.swipe_mask.set(mask);
        if mask == 0 {
            *ivars.swipe.borrow_mut() = None;
        }
    }
}
