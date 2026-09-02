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
//! - **El deslizamiento no es un reconocedor.** AppKit no tiene
//!   `NSSwipeGestureRecognizer`, pero sí tiene el gesto: llega como
//!   `swipeWithEvent:` por la cadena de responder, así que no se engancha a
//!   una vista cualquiera, hay que atenderlo en la clase. Por eso vive en
//!   `flipped.rs` y no aquí. No se imita con un `pan` con umbral: los umbrales
//!   son los del sistema.
//! - **Y hay algo que en un teléfono no existe: el puntero.** Estar encima de
//!   una vista es un evento —`hover`— y la forma del cursor es una prop. Los
//!   dos se montan con un `NSTrackingArea`, que a diferencia de un reconocedor
//!   no tiene que ser la vista quien lo atienda: el dueño del área es un
//!   objeto aparte, y por eso funcionan igual sobre un `NSButton` del sistema
//!   que sobre una vista nuestra.
//!
//! Lo que no cambia es cuándo se despachan: el evento se encola y se entrega al
//! principio del frame siguiente, para que todo lo que pasó entre dos vsync se
//! procese junto.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadOnly, Message};
use objc2_app_kit::{
    NSClickGestureRecognizer, NSControl, NSCursor, NSDatePicker, NSEvent, NSGestureRecognizer,
    NSGestureRecognizerState, NSMagnificationGestureRecognizer, NSPanGestureRecognizer,
    NSControlTextEditingDelegate, NSPopUpButton, NSPressGestureRecognizer,
    NSRotationGestureRecognizer, NSSegmentedControl, NSSlider, NSStepper, NSSwitch, NSTextField,
    NSTextFieldDelegate, NSTrackingArea, NSTrackingAreaOptions, NSView,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
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

/// El puntero, que en un teléfono no existe.
///
/// Es el dueño de un `NSTrackingArea`, no la vista: `NSTrackingArea` acepta
/// cualquier objeto como dueño y le manda a él las entradas y las salidas. Eso
/// es lo que hace que `(hover)` funcione igual encima de un `NSButton` del
/// sistema que encima de una vista nuestra, sin subclasear nada.
pub struct HoverIvars {
    node: NodeId,
    queue: EventQueue,
    /// La vista vigilada. Hace falta para dar el punto en sus coordenadas: el
    /// evento trae el de la ventana, y `NSTrackingArea` no dice de quién es.
    ///
    /// Retenerla no deja un ciclo: la vista retiene el área, el área apunta al
    /// dueño en débil, y quien retiene al dueño es el host, que suelta los dos
    /// al destruir el nodo.
    view: Retained<NSView>,
}

define_class!(
    // SAFETY: igual que GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnMacHoverTarget"]
    #[ivars = HoverIvars]
    pub struct HoverTarget;

    unsafe impl NSObjectProtocol for HoverTarget {}

    impl HoverTarget {
        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            self.emit(event, true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, event: &NSEvent) {
            self.emit(event, false);
        }
    }
);

impl HoverTarget {
    fn new(
        mtm: objc2::MainThreadMarker,
        node: NodeId,
        queue: EventQueue,
        view: Retained<NSView>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(HoverIvars { node, queue, view });
        unsafe { msg_send![super(this), init] }
    }

    /// Un solo evento con un booleano, que es como lo declara la primitiva.
    ///
    /// El punto va en coordenadas de la vista, igual que el de un `press`. Al
    /// salir es el último por el que pasó el puntero, o sea el borde por donde
    /// se fue.
    fn emit(&self, event: &NSEvent, hovered: bool) {
        let ivars = self.ivars();
        let point = ivars.view.convertPoint_fromView(unsafe { event.locationInWindow() }, None);
        emit(
            &ivars.queue,
            ivars.node,
            "hover",
            vec![
                ("hovered".to_owned(), PropValue::Bool(hovered)),
                ("x".to_owned(), PropValue::Number(point.x)),
                ("y".to_owned(), PropValue::Number(point.y)),
            ],
        );
    }
}

/// La forma del puntero encima de una vista.
///
/// También es dueño de un `NSTrackingArea`, y por la misma razón: poner un
/// cursor con `addCursorRect:cursor:` exige sobrescribir `resetCursorRects` en
/// la vista, y las vistas de este host son en su mayoría controles del sistema.
/// Con `NSTrackingCursorUpdate` el sistema pregunta al dueño del área justo
/// cuando el puntero entra, que es cuando hay que contestar.
pub struct CursorIvars {
    cursor: Retained<NSCursor>,
}

define_class!(
    // SAFETY: igual que GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnMacCursorTarget"]
    #[ivars = CursorIvars]
    pub struct CursorTarget;

    unsafe impl NSObjectProtocol for CursorTarget {}

    impl CursorTarget {
        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, _event: &NSEvent) {
            self.ivars().cursor.set();
        }
    }
);

impl CursorTarget {
    fn new(mtm: objc2::MainThreadMarker, cursor: Retained<NSCursor>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CursorIvars { cursor });
        unsafe { msg_send![super(this), init] }
    }
}

/// El cursor del sistema que le toca a cada nombre de la primitiva.
///
/// `None` es un nombre que no está en el vocabulario: quien llama avisa. No hay
/// ninguno dibujado a mano; todos son los del sistema, con el aspecto que
/// tengan en esa versión de macOS.
pub fn system_cursor(name: &str) -> Option<Retained<NSCursor>> {
    Some(match name {
        "default" => NSCursor::arrowCursor(),
        "pointer" => NSCursor::pointingHandCursor(),
        "text" => NSCursor::IBeamCursor(),
        "crosshair" => NSCursor::crosshairCursor(),
        "grab" => NSCursor::openHandCursor(),
        "grabbing" => NSCursor::closedHandCursor(),
        "not-allowed" => NSCursor::operationNotAllowedCursor(),
        _ => return None,
    })
}

/// El área que cubre a una vista entera, ahora y después de cada cambio de
/// tamaño.
///
/// `InVisibleRect` es lo que hace que no haya que rehacerla en cada
/// `set_layout`: con ella el rectángulo lo lleva AppKit pegado al de la vista y
/// el que se pasa aquí se ignora. Sin ella, un área quedaría del tamaño que
/// tenía la vista cuando alguien se suscribió, y al redimensionar la ventana
/// —que en escritorio pasa constantemente— el puntero entraría y saldría por
/// donde ya no hay nada.
fn tracking_area(
    view: &NSView,
    options: NSTrackingAreaOptions,
    owner: &objc2::runtime::AnyObject,
) -> Retained<NSTrackingArea> {
    let rect = CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize::default() };
    let area = unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            NSTrackingArea::alloc(),
            rect,
            options | NSTrackingAreaOptions::InVisibleRect,
            Some(owner),
            None,
        )
    };
    view.addTrackingArea(&area);
    area
}

/// Pone el cursor de esta vista, quitando el que hubiera.
///
/// Devuelve lo que hay que guardar vivo: AppKit se queda con el dueño del área
/// por referencia débil, así que soltarlo aquí dejaría un área que no contesta
/// y un puntero que no cambia, sin ningún error.
pub fn attach_cursor(
    mtm: objc2::MainThreadMarker,
    view: &NSView,
    cursor: Retained<NSCursor>,
) -> (Retained<NSTrackingArea>, Retained<CursorTarget>) {
    let target = CursorTarget::new(mtm, cursor);
    // `ActiveInKeyWindow` es lo que hace AppKit con sus propios rectángulos de
    // cursor: la forma del puntero es cosa de la ventana con la que se está
    // trabajando, no de una que está detrás.
    let area = tracking_area(
        view,
        NSTrackingAreaOptions::CursorUpdate | NSTrackingAreaOptions::ActiveInKeyWindow,
        &target,
    );
    (area, target)
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
    /// El puntero por encima. No es un reconocedor: es un `NSTrackingArea` con
    /// un dueño aparte, que es lo que deja vigilar un control del sistema.
    Hover { area: Retained<NSTrackingArea>, _target: Retained<HoverTarget> },
    /// Una dirección de deslizamiento. No hay nada que enganchar: el método ya
    /// está en la clase de la vista (ver `flipped.rs`); lo que se guarda es a
    /// quién hay que decírselo y qué dirección deja de escucharse al soltar.
    Swipe { view: Retained<crate::flipped::FlippedView>, bit: u8 },
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
            AttachedListener::Hover { area, .. } => view.removeTrackingArea(area),
            AttachedListener::Swipe { view, bit } => view.unlisten_swipe(*bit),
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

    // El puntero por encima. Va antes que los gestos porque no es uno: no hay
    // reconocedor, hay un área vigilada, y el dueño del área es un objeto
    // aparte. Por eso funciona sobre cualquier vista, del sistema o nuestra.
    if event == "hover" {
        let target = HoverTarget::new(mtm, node, queue, view.retain());
        // `ActiveInActiveApp` y no `ActiveAlways`: en un Mac los controles solo
        // se iluminan al pasar por encima cuando la app está delante, y esta no
        // va a ser la excepción que se comporta distinto que el resto del
        // escritorio.
        let area = tracking_area(
            view,
            NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveInActiveApp,
            &target,
        );
        return Some(AttachedListener::Hover { area, _target: target });
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
