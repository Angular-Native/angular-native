//! Gestos nativos de vuelta hacia JavaScript.
//!
//! UIKit entrega los gestos por target-action, que exige un objeto
//! Objective-C de verdad como destino. Aquí se define uno: guarda el id del
//! nodo y la cola de eventos, y al dispararse encola un `HostEvent`.
//!
//! El evento no se despacha en el momento: espera al principio del frame
//! siguiente. Así todo lo que pasó entre dos vsync se procesa junto, y la
//! respuesta a un toque se ve en el mismo frame en que se procesa.

use an_core::{NodeId, PropValue};
use an_host::{EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_ui_kit::{UIGestureRecognizer, UITapGestureRecognizer, UIView};

pub struct TargetIvars {
    node: NodeId,
    name: &'static str,
    queue: EventQueue,
}

define_class!(
    // SAFETY:
    // - NSObject no impone requisitos a sus subclases.
    // - AnGestureTarget no implementa Drop.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnGestureTarget"]
    #[ivars = TargetIvars]
    pub struct GestureTarget;

    impl GestureTarget {
        #[unsafe(method(handleGesture:))]
        fn handle_gesture(&self, recognizer: &UIGestureRecognizer) {
            let ivars = self.ivars();
            let point = recognizer.locationInView(recognizer.view().as_deref());
            ivars.queue.borrow_mut().push(HostEvent {
                target: ivars.node,
                name: ivars.name.to_owned(),
                payload: vec![
                    ("x".to_owned(), PropValue::Number(point.x)),
                    ("y".to_owned(), PropValue::Number(point.y)),
                ],
            });
        }
    }
);

impl GestureTarget {
    fn new(mtm: objc2::MainThreadMarker, node: NodeId, name: &'static str, queue: EventQueue) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { node, name, queue });
        unsafe { msg_send![super(this), init] }
    }

    fn action() -> Sel {
        sel!(handleGesture:)
    }
}

/// Un gesto enganchado a una vista, con su destino vivo mientras dure.
pub struct AttachedGesture {
    recognizer: Retained<UIGestureRecognizer>,
    /// El reconocedor guarda el target con referencia débil: si se suelta,
    /// UIKit dispara contra un objeto liberado.
    _target: Retained<GestureTarget>,
}

impl AttachedGesture {
    pub fn detach(&self, view: &UIView) {
        view.removeGestureRecognizer(&self.recognizer);
    }
}

/// Nombres de evento que esta plataforma sabe reconocer. El resto se ignoran
/// en silencio: una plantilla puede traer `(click)` heredado de web y no es
/// motivo para reventar la app.
pub fn attach(
    mtm: objc2::MainThreadMarker,
    view: &UIView,
    node: NodeId,
    event: &str,
    queue: EventQueue,
) -> Option<AttachedGesture> {
    let (name, taps): (&'static str, usize) = match event {
        "press" | "click" | "tap" => ("press", 1),
        "doublePress" => ("doublePress", 2),
        _ => return None,
    };

    let target = GestureTarget::new(mtm, node, name, queue);
    let recognizer = unsafe {
        UITapGestureRecognizer::initWithTarget_action(
            UITapGestureRecognizer::alloc(mtm),
            Some(&target),
            Some(GestureTarget::action()),
        )
    };
    recognizer.setNumberOfTapsRequired(taps);

    // UILabel y UIImageView vienen con la interacción apagada de fábrica:
    // sin esto el gesto se registra y no se dispara nunca.
    view.setUserInteractionEnabled(true);
    view.addGestureRecognizer(&recognizer);

    Some(AttachedGesture {
        recognizer: Retained::into_super(recognizer),
        _target: target,
    })
}
