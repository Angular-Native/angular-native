//! Eventos nativos de vuelta hacia JavaScript.
//!
//! Igual que en iOS, AppKit entrega gestos y acciones por target-action, que
//! exige un objeto de Objective-C de verdad como destino: aquí se definen dos,
//! uno para gestos y otro para controles, cada uno con el id del nodo y la cola
//! de eventos dentro.
//!
//! **Lo que cambia en el escritorio.** Aquí no hay dedos, hay ratón y trackpad,
//! y los reconocedores de AppKit no son los mismos que los de UIKit:
//!
//! - `press` es un clic, no un toque: `NSClickGestureRecognizer`. `doublePress`
//!   es el mismo con dos clics.
//! - `longPress` es `NSPressGestureRecognizer`, que es mantener pulsado el
//!   botón del ratón, no el dedo.
//! - `pan` es `NSPanGestureRecognizer`, arrastrar con el botón pulsado.
//! - `pinch` es `NSMagnificationGestureRecognizer` y `rotate` es
//!   `NSRotationGestureRecognizer`: los dos son de trackpad, con un ratón no
//!   pasan nunca. Se enganchan igual, porque un Mac con trackpad sí los da.
//! - **Los deslizamientos no existen.** AppKit no tiene reconocedor de
//!   deslizamiento; el de dos dedos del trackpad llega como scroll. No se
//!   imita con un `pan` con umbral: se dice al suscribirse. Ver
//!   `support::unsupported_event`.
//!
//! Lo que no cambia es cuándo se despachan: el evento se encola y se entrega al
//! principio del frame siguiente, para que todo lo que pasó entre dos vsync se
//! procese junto.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSClickGestureRecognizer, NSControl, NSDatePicker, NSGestureRecognizer,
    NSGestureRecognizerState, NSMagnificationGestureRecognizer, NSPanGestureRecognizer,
    NSControlTextEditingDelegate, NSPopUpButton, NSPressGestureRecognizer,
    NSRotationGestureRecognizer, NSSegmentedControl, NSSlider, NSStepper, NSSwitch, NSTextField,
    NSTextFieldDelegate, NSView,
};
use objc2_foundation::NSObjectProtocol;

fn emit(queue: &EventQueue, target: NodeId, name: &str, payload: Vec<(String, PropValue)>) {
    push_event(queue, HostEvent { target, name: name.to_owned(), payload });
}

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
    #[name = "AnMacGestureTarget"]
    #[ivars = TargetIvars]
    pub struct GestureTarget;

    unsafe impl NSObjectProtocol for GestureTarget {}

    impl GestureTarget {
        /// Clic y doble clic. Un `NSClickGestureRecognizer` solo se dispara
        /// cuando el gesto ya ha terminado, así que no hay que filtrar estado.
        #[unsafe(method(handleClick:))]
        fn handle_click(&self, recognizer: &NSGestureRecognizer) {
            let ivars = self.ivars();
            let point = self.point_in_view(recognizer);
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.0)),
                    ("y".to_owned(), PropValue::Number(point.1)),
                ],
            );
        }

        /// Arrastrar con el botón pulsado. Lleva desplazamiento y velocidad,
        /// que es lo que hace falta para mover algo con el ratón y para decidir
        /// si al soltar sigue por inercia.
        #[unsafe(method(handlePan:))]
        fn handle_pan(&self, recognizer: &NSPanGestureRecognizer) {
            let ivars = self.ivars();
            let view = unsafe { recognizer.view() };
            let translation = unsafe { recognizer.translationInView(view.as_deref()) };
            let velocity = unsafe { recognizer.velocityInView(view.as_deref()) };
            let point = self.point_in_view(recognizer);
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.0)),
                    ("y".to_owned(), PropValue::Number(point.1)),
                    ("translationX".to_owned(), PropValue::Number(translation.x)),
                    // AppKit da el desplazamiento en coordenadas de la vista, y
                    // las vistas de este host van con el origen arriba (ver
                    // `flipped.rs`), así que la `y` ya crece hacia abajo y
                    // coincide con la de iOS y la de Android.
                    ("translationY".to_owned(), PropValue::Number(translation.y)),
                    ("velocityX".to_owned(), PropValue::Number(velocity.x)),
                    ("velocityY".to_owned(), PropValue::Number(velocity.y)),
                    (
                        "state".to_owned(),
                        PropValue::Str(state_name(unsafe { recognizer.state() })),
                    ),
                ],
            );
        }

        /// Mantener pulsado. Solo se avisa al empezar, igual que en iOS: el
        /// sistema ya decidió que el gesto cuenta.
        #[unsafe(method(handleLongPress:))]
        fn handle_long_press(&self, recognizer: &NSGestureRecognizer) {
            if unsafe { recognizer.state() } != NSGestureRecognizerState::Began {
                return;
            }
            let ivars = self.ivars();
            let point = self.point_in_view(recognizer);
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.0)),
                    ("y".to_owned(), PropValue::Number(point.1)),
                ],
            );
        }

        #[unsafe(method(handlePinch:))]
        fn handle_pinch(&self, recognizer: &NSMagnificationGestureRecognizer) {
            let ivars = self.ivars();
            // AppKit da la ampliación como incremento sobre 1 y UIKit da la
            // escala. Se manda escala, que es lo que la plantilla espera.
            let scale = 1.0 + unsafe { recognizer.magnification() };
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("scale".to_owned(), PropValue::Number(scale)),
                    // El trackpad no da velocidad de ampliación; cero es
                    // honesto y no rompe a quien la lea.
                    ("velocity".to_owned(), PropValue::Number(0.0)),
                    (
                        "state".to_owned(),
                        PropValue::Str(state_name(unsafe { recognizer.state() })),
                    ),
                ],
            );
        }

        #[unsafe(method(handleRotate:))]
        fn handle_rotate(&self, recognizer: &NSRotationGestureRecognizer) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("rotation".to_owned(), PropValue::Number(unsafe { recognizer.rotation() } as f64)),
                    ("velocity".to_owned(), PropValue::Number(0.0)),
                    (
                        "state".to_owned(),
                        PropValue::Str(state_name(unsafe { recognizer.state() })),
                    ),
                ],
            );
        }
    }
);

impl GestureTarget {
    fn new(
        mtm: objc2::MainThreadMarker,
        node: NodeId,
        name: &'static str,
        queue: EventQueue,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { node, name, queue });
        unsafe { msg_send![super(this), init] }
    }

    fn point_in_view(&self, recognizer: &NSGestureRecognizer) -> (f64, f64) {
        let view = unsafe { recognizer.view() };
        let point = unsafe { recognizer.locationInView(view.as_deref()) };
        (point.x, point.y)
    }
}

/// Nombre del estado, tal cual lo verá la plantilla. Los mismos cuatro que en
/// iOS: la plantilla no tiene por qué saber en qué plataforma corre.
fn state_name(state: NSGestureRecognizerState) -> String {
    match state {
        NSGestureRecognizerState::Began => "begin",
        NSGestureRecognizerState::Changed => "move",
        NSGestureRecognizerState::Ended => "end",
        NSGestureRecognizerState::Cancelled | NSGestureRecognizerState::Failed => "cancel",
        _ => "possible",
    }
    .to_owned()
}

pub struct ControlIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: igual que GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnMacControlTarget"]
    #[ivars = ControlIvars]
    pub struct ControlTarget;

    unsafe impl NSObjectProtocol for ControlTarget {}

    impl ControlTarget {
        #[unsafe(method(handleButton:))]
        fn handle_button(&self, _sender: &NSControl) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "press", Vec::new());
        }

        /// El interruptor de macOS avisa por acción, no por `ValueChanged`:
        /// `NSSwitch` es un `NSControl` y su estado es `1` o `0`.
        #[unsafe(method(handleSwitch:))]
        fn handle_switch(&self, sender: &NSSwitch) {
            let ivars = self.ivars();
            let on = unsafe { sender.state() } == objc2_app_kit::NSControlStateValueOn;
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Bool(on))],
            );
        }

        #[unsafe(method(handleSlider:))]
        fn handle_slider(&self, sender: &NSSlider) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(unsafe { sender.doubleValue() }))],
            );
        }

        #[unsafe(method(handleSegments:))]
        fn handle_segments(&self, sender: &NSSegmentedControl) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![(
                    "index".to_owned(),
                    PropValue::Number(unsafe { sender.selectedSegment() } as f64),
                )],
            );
        }

        /// La barra de pestañas de macOS es un segmentado (ver `support.rs`),
        /// así que la selección llega por el mismo camino pero se llama
        /// `select`, que es como la nombra la primitiva.
        #[unsafe(method(handleTabs:))]
        fn handle_tabs(&self, sender: &NSSegmentedControl) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "select",
                vec![(
                    "index".to_owned(),
                    PropValue::Number(unsafe { sender.selectedSegment() } as f64),
                )],
            );
        }

        #[unsafe(method(handleStepper:))]
        fn handle_stepper(&self, sender: &NSStepper) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(unsafe { sender.doubleValue() }))],
            );
        }

        #[unsafe(method(handleMenu:))]
        fn handle_menu(&self, sender: &NSPopUpButton) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![(
                    "index".to_owned(),
                    PropValue::Number(unsafe { sender.indexOfSelectedItem() } as f64),
                )],
            );
        }

        #[unsafe(method(handleDate:))]
        fn handle_date(&self, sender: &NSDatePicker) {
            let ivars = self.ivars();
            // Milisegundos desde 1970, que es lo que entiende `Date` en JS.
            let seconds = unsafe { sender.dateValue().timeIntervalSince1970() };
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(seconds * 1000.0))],
            );
        }

        /// Un campo de texto de AppKit manda su acción al pulsar Intro, no en
        /// cada tecla; lo de cada tecla va por el delegado, que es otro camino
        /// y bastante más caro. `submit` es lo que esta acción significa.
        #[unsafe(method(handleSubmit:))]
        fn handle_submit(&self, sender: &NSTextField) {
            let ivars = self.ivars();
            let value = unsafe { sender.stringValue() }.to_string();
            emit(
                &ivars.queue,
                ivars.node,
                "submit",
                vec![("value".to_owned(), PropValue::Str(value))],
            );
        }

    }

    /// Lo que un campo cuenta mientras se escribe no llega por acción sino por
    /// delegado: la acción de un `NSTextField` solo se dispara al pulsar Intro.
    unsafe impl NSControlTextEditingDelegate for ControlTarget {
        #[unsafe(method(controlTextDidChange:))]
        fn controlTextDidChange(&self, notification: &objc2_foundation::NSNotification) {
            let ivars = self.ivars();
            let Some(object) = (unsafe { notification.object() }) else { return };
            let field: *const objc2::runtime::AnyObject = &*object;
            let value = unsafe { (*field.cast::<NSTextField>()).stringValue() }.to_string();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Str(value))],
            );
        }

        #[unsafe(method(controlTextDidBeginEditing:))]
        fn controlTextDidBeginEditing(&self, _notification: &objc2_foundation::NSNotification) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "focus", Vec::new());
        }

        #[unsafe(method(controlTextDidEndEditing:))]
        fn controlTextDidEndEditing(&self, _notification: &objc2_foundation::NSNotification) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "blur", Vec::new());
        }
    }

    unsafe impl NSTextFieldDelegate for ControlTarget {}
);

impl ControlTarget {
    pub fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ControlIvars { node, queue });
        unsafe { msg_send![super(this), init] }
    }
}

/// Una suscripción viva. Guarda lo que AppKit referencia débilmente, que es
/// justo lo que se libera solo si no lo retiene nadie: el destino de una acción
/// y el delegado de un campo.
pub enum AttachedListener {
    Gesture { recognizer: Retained<NSGestureRecognizer>, _target: Retained<GestureTarget> },
    /// Acción de un `NSControl`. AppKit solo admite **una** por control, así
    /// que dos suscripciones al mismo control se pisarían; en la práctica no
    /// pasa porque cada control tiene un solo evento que dar.
    Action { _target: Retained<ControlTarget> },
    /// Delegado de un campo de texto, que es por donde llegan las tres cosas
    /// que un campo cuenta mientras se escribe.
    FieldDelegate { _target: Retained<ControlTarget> },
}

impl AttachedListener {
    pub fn detach(&self, view: &NSView) {
        match self {
            AttachedListener::Gesture { recognizer, .. } => {
                unsafe { view.removeGestureRecognizer(recognizer) };
            }
            AttachedListener::Action { .. } => {
                let control: *const NSView = view;
                unsafe { (*control.cast::<NSControl>()).setTarget(None) };
            }
            AttachedListener::FieldDelegate { .. } => {
                let field: *const NSView = view;
                unsafe { (*field.cast::<NSTextField>()).setDelegate(None) };
            }
        }
    }
}

/// Qué acción de `NSControl` le toca a cada par (primitiva, evento).
fn control_action(kind: an_core::NodeKind, event: &str) -> Option<Sel> {
    use an_core::NodeKind;
    Some(match (kind, event) {
        (NodeKind::Button, "press") => sel!(handleButton:),
        (NodeKind::Switch, "change") => sel!(handleSwitch:),
        (NodeKind::Slider, "change") => sel!(handleSlider:),
        (NodeKind::SegmentedControl, "change") => sel!(handleSegments:),
        (NodeKind::TabBar, "select") => sel!(handleTabs:),
        (NodeKind::Stepper, "change") => sel!(handleStepper:),
        (NodeKind::Picker, "change") => sel!(handleMenu:),
        (NodeKind::DatePicker, "change") => sel!(handleDate:),
        (NodeKind::TextInput | NodeKind::SearchBar, "submit") => sel!(handleSubmit:),
        _ => return None,
    })
}

/// Engancha un evento a una vista. `None` significa que esta plataforma no
/// sabe entregarlo; quien llama decide si eso merece un aviso.
pub fn attach(
    mtm: objc2::MainThreadMarker,
    view: &NSView,
    kind: an_core::NodeKind,
    node: NodeId,
    event: &str,
    queue: EventQueue,
) -> Option<AttachedListener> {
    use an_core::NodeKind;

    if let Some(action) = control_action(kind, event) {
        let target = ControlTarget::new(mtm, node, queue);
        let control: *const NSView = view;
        unsafe {
            (*control.cast::<NSControl>()).setTarget(Some(&*target));
            (*control.cast::<NSControl>()).setAction(Some(action));
        }
        return Some(AttachedListener::Action { _target: target });
    }

    // Lo que un campo cuenta mientras se escribe llega por delegado.
    // `change`/`input`, `focus` y `blur` son las tres notificaciones de
    // `NSControl`, y el mismo destino las atiende todas: engancharlo una vez
    // vale para las tres.
    if matches!(kind, NodeKind::TextInput | NodeKind::SearchBar)
        && matches!(event, "change" | "input" | "focus" | "blur")
    {
        let target = ControlTarget::new(mtm, node, queue);
        let field: *const NSView = view;
        unsafe {
            (*field.cast::<NSTextField>())
                .setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*target)));
        }
        return Some(AttachedListener::FieldDelegate { _target: target });
    }

    // Gestos continuos.
    let continuous: Option<(Retained<NSGestureRecognizer>, Retained<GestureTarget>)> = match event {
        "pan" => {
            let target = GestureTarget::new(mtm, node, "pan", queue.clone());
            let recognizer = unsafe {
                NSPanGestureRecognizer::initWithTarget_action(
                    NSPanGestureRecognizer::alloc(mtm),
                    Some(&target),
                    Some(sel!(handlePan:)),
                )
            };
            Some((Retained::into_super(recognizer), target))
        }
        "longPress" => {
            let target = GestureTarget::new(mtm, node, "longPress", queue.clone());
            let recognizer = unsafe {
                NSPressGestureRecognizer::initWithTarget_action(
                    NSPressGestureRecognizer::alloc(mtm),
                    Some(&target),
                    Some(sel!(handleLongPress:)),
                )
            };
            Some((Retained::into_super(recognizer), target))
        }
        "pinch" => {
            let target = GestureTarget::new(mtm, node, "pinch", queue.clone());
            let recognizer = unsafe {
                NSMagnificationGestureRecognizer::initWithTarget_action(
                    NSMagnificationGestureRecognizer::alloc(mtm),
                    Some(&target),
                    Some(sel!(handlePinch:)),
                )
            };
            Some((Retained::into_super(recognizer), target))
        }
        "rotate" => {
            let target = GestureTarget::new(mtm, node, "rotate", queue.clone());
            let recognizer = unsafe {
                NSRotationGestureRecognizer::initWithTarget_action(
                    NSRotationGestureRecognizer::alloc(mtm),
                    Some(&target),
                    Some(sel!(handleRotate:)),
                )
            };
            Some((Retained::into_super(recognizer), target))
        }
        _ => None,
    };
    if let Some((recognizer, target)) = continuous {
        unsafe { view.addGestureRecognizer(&recognizer) };
        return Some(AttachedListener::Gesture { recognizer, _target: target });
    }

    // Clic y doble clic.
    let (name, clicks): (&'static str, isize) = match event {
        "press" | "click" | "tap" => ("press", 1),
        "doublePress" => ("doublePress", 2),
        _ => return None,
    };
    let target = GestureTarget::new(mtm, node, name, queue);
    let recognizer = unsafe {
        NSClickGestureRecognizer::initWithTarget_action(
            NSClickGestureRecognizer::alloc(mtm),
            Some(&target),
            Some(sel!(handleClick:)),
        )
    };
    unsafe { recognizer.setNumberOfClicksRequired(clicks) };
    unsafe { view.addGestureRecognizer(&recognizer) };
    Some(AttachedListener::Gesture {
        recognizer: Retained::into_super(recognizer),
        _target: target,
    })
}
