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
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_foundation::NSObjectProtocol;
use objc2_ui_kit::{
    UITabBarController, UITabBarControllerDelegate, UIViewController,
    UIControl, UIControlEvents, UIGestureRecognizer, UIGestureRecognizerState,
    UILongPressGestureRecognizer, UIPanGestureRecognizer, UIPinchGestureRecognizer, UIRectEdge,
    UIRefreshControl, UIRotationGestureRecognizer, UIScreenEdgePanGestureRecognizer, UIScrollView,
    UIScrollViewDelegate, UISlider, UISwipeGestureRecognizer,
    UISwipeGestureRecognizerDirection, UISwitch, UITabBar, UITabBarDelegate, UITabBarItem,
    UITapGestureRecognizer, UITextField, UIView,
};

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
    #[name = "AnGestureTarget"]
    #[ivars = TargetIvars]
    pub struct GestureTarget;

    impl GestureTarget {
        /// Gesto de volver atrás desde el borde. Solo cuenta al soltar: un
        /// arrastre que se cancela no debe navegar.
        #[unsafe(method(handleEdgePan:))]
        fn handle_edge_pan(&self, recognizer: &UIGestureRecognizer) {
            if recognizer.state() != UIGestureRecognizerState::Ended {
                return;
            }
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, ivars.name, Vec::new());
        }

        /// Arrastrar. Lleva el desplazamiento acumulado y la velocidad, que
        /// es lo que hace falta para mover algo con el dedo y para decidir si
        /// al soltar sigue por inercia.
        #[unsafe(method(handlePan:))]
        fn handle_pan(&self, recognizer: &UIPanGestureRecognizer) {
            let ivars = self.ivars();
            let view = recognizer.view();
            let translation = recognizer.translationInView(view.as_deref());
            let velocity = recognizer.velocityInView(view.as_deref());
            let point = recognizer.locationInView(view.as_deref());
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.x)),
                    ("y".to_owned(), PropValue::Number(point.y)),
                    ("translationX".to_owned(), PropValue::Number(translation.x)),
                    ("translationY".to_owned(), PropValue::Number(translation.y)),
                    ("velocityX".to_owned(), PropValue::Number(velocity.x)),
                    ("velocityY".to_owned(), PropValue::Number(velocity.y)),
                    ("state".to_owned(), PropValue::Str(state_name(recognizer.state()))),
                ],
            );
        }

        /// Mantener pulsado. Solo se avisa al empezar: el sistema ya decidió
        /// que el gesto cuenta, y avisar también al soltar solo daría un
        /// segundo evento que nadie espera.
        #[unsafe(method(handleLongPress:))]
        fn handle_long_press(&self, recognizer: &UIGestureRecognizer) {
            if recognizer.state() != UIGestureRecognizerState::Began {
                return;
            }
            let ivars = self.ivars();
            let point = recognizer.locationInView(recognizer.view().as_deref());
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.x)),
                    ("y".to_owned(), PropValue::Number(point.y)),
                ],
            );
        }

        #[unsafe(method(handleSwipe:))]
        fn handle_swipe(&self, recognizer: &UIGestureRecognizer) {
            let ivars = self.ivars();
            let point = recognizer.locationInView(recognizer.view().as_deref());
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.x)),
                    ("y".to_owned(), PropValue::Number(point.y)),
                ],
            );
        }

        #[unsafe(method(handlePinch:))]
        fn handle_pinch(&self, recognizer: &UIPinchGestureRecognizer) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("scale".to_owned(), PropValue::Number(recognizer.scale())),
                    ("velocity".to_owned(), PropValue::Number(recognizer.velocity())),
                    ("state".to_owned(), PropValue::Str(state_name(recognizer.state()))),
                ],
            );
        }

        #[unsafe(method(handleRotate:))]
        fn handle_rotate(&self, recognizer: &UIRotationGestureRecognizer) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("rotation".to_owned(), PropValue::Number(recognizer.rotation())),
                    ("velocity".to_owned(), PropValue::Number(recognizer.velocity())),
                    ("state".to_owned(), PropValue::Str(state_name(recognizer.state()))),
                ],
            );
        }

        #[unsafe(method(handleGesture:))]
        fn handle_gesture(&self, recognizer: &UIGestureRecognizer) {
            let ivars = self.ivars();
            let point = recognizer.locationInView(recognizer.view().as_deref());
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("x".to_owned(), PropValue::Number(point.x)),
                    ("y".to_owned(), PropValue::Number(point.y)),
                ],
            );
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

    fn edge_action() -> Sel {
        sel!(handleEdgePan:)
    }
}

/// Nombre del estado, tal cual lo verá la plantilla.
fn state_name(state: UIGestureRecognizerState) -> String {
    match state {
        UIGestureRecognizerState::Began => "begin",
        UIGestureRecognizerState::Changed => "move",
        UIGestureRecognizerState::Ended => "end",
        UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => "cancel",
        _ => "possible",
    }
    .to_owned()
}

/// Destino de las acciones de un `UIControl`: escribir, entrar y salir de un
/// campo de texto.
pub struct ControlIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: igual que GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnControlTarget"]
    #[ivars = ControlIvars]
    pub struct ControlTarget;

    impl ControlTarget {
        #[unsafe(method(handleChange:))]
        fn handle_change(&self, sender: &UITextField) {
            self.emit_with_value("change", sender);
        }

        #[unsafe(method(handleFocus:))]
        fn handle_focus(&self, sender: &UITextField) {
            self.emit_with_value("focus", sender);
        }

        #[unsafe(method(handleBlur:))]
        fn handle_blur(&self, sender: &UITextField) {
            self.emit_with_value("blur", sender);
        }

        #[unsafe(method(handleSubmit:))]
        fn handle_submit(&self, sender: &UITextField) {
            self.emit_with_value("submit", sender);
        }

        #[unsafe(method(handleSwitch:))]
        fn handle_switch(&self, sender: &UISwitch) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Bool(sender.isOn()))],
            );
        }

        #[unsafe(method(handleSlider:))]
        fn handle_slider(&self, sender: &UISlider) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(sender.value() as f64))],
            );
        }

        #[unsafe(method(handleSegments:))]
        fn handle_segments(&self, sender: &objc2_ui_kit::UISegmentedControl) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![(
                    "index".to_owned(),
                    PropValue::Number(sender.selectedSegmentIndex() as f64),
                )],
            );
        }

        #[unsafe(method(handleStepper:))]
        fn handle_stepper(&self, sender: &objc2_ui_kit::UIStepper) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(unsafe { sender.value() }))],
            );
        }

        #[unsafe(method(handleDate:))]
        fn handle_date(&self, sender: &objc2_ui_kit::UIDatePicker) {
            let ivars = self.ivars();
            // Milisegundos desde 1970, que es lo que entiende `Date` en JS sin
            // que nadie tenga que convertir nada.
            let seconds = unsafe { sender.date().timeIntervalSince1970() };
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(seconds * 1000.0))],
            );
        }

        /// El botón de atrás de una cabecera.
        ///
        /// Fuera de un `UINavigationController` no hay atrás automático: el
        /// botón se pone a mano y quien navega es el router, así que aquí solo
        /// se avisa.
        #[unsafe(method(handleNavBack:))]
        fn handle_nav_back(&self, _sender: &objc2::runtime::AnyObject) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "back", Vec::new());
        }

        #[unsafe(method(handleButton:))]
        fn handle_button(&self, _sender: &UIControl) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "press", Vec::new());
        }

        #[unsafe(method(handleRefresh:))]
        fn handle_refresh(&self, _sender: &UIControl) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "refresh", Vec::new());
        }
    }
);

/// Delegado de la barra de pestañas. UIKit lo guarda con referencia débil.
pub struct TabIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: igual que GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnTabBarDelegate"]
    #[ivars = TabIvars]
    pub struct TabDelegate;

    unsafe impl NSObjectProtocol for TabDelegate {}

    unsafe impl UITabBarControllerDelegate for TabDelegate {
        #[unsafe(method(tabBarController:didSelectViewController:))]
        fn did_select(
            &self,
            _controller: &UITabBarController,
            selected: &UIViewController,
        ) {
            let ivars = self.ivars();
            let index = unsafe { selected.tabBarItem() }.map(|i| i.tag()).unwrap_or(0);
            emit(
                &ivars.queue,
                ivars.node,
                "select",
                // El `tag` es el índice: se le puso al construir la pestaña.
                vec![("index".to_owned(), PropValue::Number(index as f64))],
            );
        }
    }
);

impl TabDelegate {
    pub fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TabIvars { node, queue });
        unsafe { msg_send![super(this), init] }
    }
}

impl ControlTarget {
    /// Un destino suelto, para engancharlo a algo que no es un `UIControl`
    /// —un `UIBarButtonItem`, por ejemplo—.
    pub fn standalone(
        mtm: objc2::MainThreadMarker,
        node: NodeId,
        queue: EventQueue,
    ) -> Retained<Self> {
        Self::new(mtm, node, queue)
    }

    pub fn nav_back_action() -> Sel {
        sel!(handleNavBack:)
    }

    fn emit_with_value(&self, name: &str, field: &UITextField) {
        let ivars = self.ivars();
        let value = field.text().map(|t| t.to_string()).unwrap_or_default();
        emit(&ivars.queue, ivars.node, name, vec![("value".to_owned(), PropValue::Str(value))]);
    }

    fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ControlIvars { node, queue });
        unsafe { msg_send![super(this), init] }
    }
}

/// Delegado de scroll. UIKit lo guarda con referencia débil, así que hay que
/// conservarlo vivo aquí mientras la vista exista.
pub struct ScrollIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: igual que GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnScrollDelegate"]
    #[ivars = ScrollIvars]
    pub struct ScrollDelegate;

    unsafe impl NSObjectProtocol for ScrollDelegate {}

    unsafe impl UIScrollViewDelegate for ScrollDelegate {
        #[unsafe(method(scrollViewDidScroll:))]
        fn scroll_view_did_scroll(&self, scroll_view: &UIScrollView) {
            let ivars = self.ivars();
            let offset = scroll_view.contentOffset();
            emit(
                &ivars.queue,
                ivars.node,
                "scroll",
                vec![
                    ("x".to_owned(), PropValue::Number(offset.x)),
                    ("y".to_owned(), PropValue::Number(offset.y)),
                ],
            );
        }
    }
);

impl ScrollDelegate {
    fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ScrollIvars { node, queue });
        unsafe { msg_send![super(this), init] }
    }
}

impl AttachedListener {
    /// El control de recarga del sistema, si esta suscripción lo trajo.
    pub fn refresh_control(&self) -> Option<&UIRefreshControl> {
        match self {
            AttachedListener::Refresh { control, .. } => Some(control),
            _ => None,
        }
    }
}

/// Una suscripción viva. Guarda lo que UIKit referencia débilmente, que es
/// justo lo que se libera solo si no lo retiene nadie.
pub enum AttachedListener {
    Gesture {
        recognizer: Retained<UIGestureRecognizer>,
        _target: Retained<GestureTarget>,
    },
    Control {
        events: UIControlEvents,
        action: Sel,
        target: Retained<ControlTarget>,
    },
    Scroll {
        _delegate: Retained<ScrollDelegate>,
    },
    Tabs {
        _delegate: Retained<TabDelegate>,
    },
    Refresh {
        _target: Retained<ControlTarget>,
        control: Retained<UIRefreshControl>,
    },
}

impl AttachedListener {
    pub fn detach(&self, view: &UIView) {
        match self {
            AttachedListener::Gesture { recognizer, .. } => {
                view.removeGestureRecognizer(recognizer);
            }
            AttachedListener::Control { events, action, target } => {
                let control: *const UIView = view;
                let control = control.cast::<UIControl>();
                unsafe {
                    (*control).removeTarget_action_forControlEvents(
                        Some(&**target),
                        Some(*action),
                        *events,
                    )
                };
            }
            AttachedListener::Scroll { .. } => {
                let scroll: *const UIView = view;
                let scroll = scroll.cast::<UIScrollView>();
                unsafe { (*scroll).setDelegate(None) };
            }
            AttachedListener::Refresh { .. } => {
                let scroll: *const UIView = view;
                unsafe { (*scroll.cast::<UIScrollView>()).setRefreshControl(None) };
            }
            AttachedListener::Tabs { .. } => {
                let bar: *const UIView = view;
                let bar = bar.cast::<UITabBar>();
                unsafe { (*bar).setDelegate(None) };
            }
        }
    }
}

/// Construye el reconocedor de un gesto continuo o de dirección, si el nombre
/// es de uno.
fn continuous_gesture(
    mtm: objc2::MainThreadMarker,
    event: &str,
    node: NodeId,
    queue: &EventQueue,
) -> Option<(Retained<UIGestureRecognizer>, Retained<GestureTarget>)> {
    let (action, name): (Sel, &'static str) = match event {
        "pan" => (sel!(handlePan:), "pan"),
        "longPress" => (sel!(handleLongPress:), "longPress"),
        "pinch" => (sel!(handlePinch:), "pinch"),
        "rotate" => (sel!(handleRotate:), "rotate"),
        "swipeLeft" | "swipeRight" | "swipeUp" | "swipeDown" => (sel!(handleSwipe:), "swipe"),
        _ => return None,
    };
    let _ = name;
    let target = GestureTarget::new(mtm, node, leak_event_name(event), queue.clone());

    let recognizer: Retained<UIGestureRecognizer> = match event {
        "pan" => Retained::into_super(unsafe {
            UIPanGestureRecognizer::initWithTarget_action(
                UIPanGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(action),
            )
        }),
        "longPress" => Retained::into_super(unsafe {
            UILongPressGestureRecognizer::initWithTarget_action(
                UILongPressGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(action),
            )
        }),
        "pinch" => Retained::into_super(unsafe {
            UIPinchGestureRecognizer::initWithTarget_action(
                UIPinchGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(action),
            )
        }),
        "rotate" => Retained::into_super(unsafe {
            UIRotationGestureRecognizer::initWithTarget_action(
                UIRotationGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(action),
            )
        }),
        _ => {
            let swipe = unsafe {
                UISwipeGestureRecognizer::initWithTarget_action(
                    UISwipeGestureRecognizer::alloc(mtm),
                    Some(&target),
                    Some(action),
                )
            };
            swipe.setDirection(match event {
                "swipeRight" => UISwipeGestureRecognizerDirection::Right,
                "swipeUp" => UISwipeGestureRecognizerDirection::Up,
                "swipeDown" => UISwipeGestureRecognizerDirection::Down,
                _ => UISwipeGestureRecognizerDirection::Left,
            });
            Retained::into_super(swipe)
        }
    };
    Some((recognizer, target))
}

/// El nombre del evento tiene que vivir tanto como el destino del gesto, y los
/// nombres son un conjunto cerrado y conocido.
fn leak_event_name(event: &str) -> &'static str {
    match event {
        "pan" => "pan",
        "longPress" => "longPress",
        "pinch" => "pinch",
        "rotate" => "rotate",
        "swipeLeft" => "swipeLeft",
        "swipeRight" => "swipeRight",
        "swipeUp" => "swipeUp",
        "swipeDown" => "swipeDown",
        _ => "gesture",
    }
}

/// Nombres de evento que esta plataforma sabe reconocer. El resto se ignoran
/// en silencio: una plantilla puede traer `(click)` heredado de web y no es
/// motivo para reventar la app.
///
/// `kind` decide qué mecanismo de UIKit se usa: gestos para vistas normales,
/// target-action para campos de texto, delegado para scroll.
pub fn attach(
    mtm: objc2::MainThreadMarker,
    view: &UIView,
    kind: an_core::NodeKind,
    node: NodeId,
    event: &str,
    queue: EventQueue,
) -> Option<AttachedListener> {
    use an_core::NodeKind;

    // Controles que avisan por target-action: el valor cambió, o se pulsó.
    let control_action = match (kind, event) {
        (NodeKind::Switch, "change") => Some((UIControlEvents::ValueChanged, sel!(handleSwitch:))),
        (NodeKind::Slider, "change") => Some((UIControlEvents::ValueChanged, sel!(handleSlider:))),
        (NodeKind::Button, "press") => Some((UIControlEvents::TouchUpInside, sel!(handleButton:))),
        (NodeKind::SegmentedControl, "change") => {
            Some((UIControlEvents::ValueChanged, sel!(handleSegments:)))
        }
        (NodeKind::Stepper, "change") => Some((UIControlEvents::ValueChanged, sel!(handleStepper:))),
        (NodeKind::DatePicker, "change") => Some((UIControlEvents::ValueChanged, sel!(handleDate:))),
        _ => None,
    };
    if let Some((events, action)) = control_action {
        let target = ControlTarget::new(mtm, node, queue);
        let control: *const UIView = view;
        let control = control.cast::<UIControl>();
        unsafe { (*control).addTarget_action_forControlEvents(Some(&*target), action, events) };
        return Some(AttachedListener::Control { events, action, target });
    }

    // La barra de búsqueda no es un control: lo es su campo de texto, que sí
    // es un `UITextField`. Engancharse a él evita tener que implementar el
    // delegado entero para saber que alguien escribió.
    if kind == NodeKind::SearchBar {
        let events = match event {
            "input" => UIControlEvents::EditingChanged,
            "submit" => UIControlEvents::EditingDidEndOnExit,
            "focus" => UIControlEvents::EditingDidBegin,
            "blur" => UIControlEvents::EditingDidEnd,
            _ => return None,
        };
        let action = match event {
            "input" => sel!(handleChange:),
            "submit" => sel!(handleSubmit:),
            "focus" => sel!(handleFocus:),
            _ => sel!(handleBlur:),
        };
        let bar: *const UIView = view;
        let field = unsafe { (*bar.cast::<objc2_ui_kit::UISearchBar>()).searchTextField() };
        let target = ControlTarget::new(mtm, node, queue);
        unsafe { field.addTarget_action_forControlEvents(Some(&*target), action, events) };
        return Some(AttachedListener::Control { events, action, target });
    }

    // Tirar para recargar. En iOS lo dibuja el sistema: se le engancha un
    // `UIRefreshControl` al scroll y él pone la ruedecilla y la animación.
    if kind == NodeKind::ScrollView && event == "refresh" {
        let target = ControlTarget::new(mtm, node, queue);
        let control = UIRefreshControl::new(mtm);
        unsafe {
            control.addTarget_action_forControlEvents(
                Some(&*target),
                sel!(handleRefresh:),
                UIControlEvents::ValueChanged,
            );
        }
        let scroll: *const UIView = view;
        unsafe { (*scroll.cast::<UIScrollView>()).setRefreshControl(Some(&control)) };
        return Some(AttachedListener::Refresh { _target: target, control });
    }

    // La selección de pestaña la avisa el controlador, no la vista: el host
    // engancha el delegado al crearla, porque aquí solo llega la vista.
    if kind == NodeKind::TabBar {
        return None;
    }

    if kind == NodeKind::TextInput {
        let (events, action) = match event {
            "change" | "input" => (UIControlEvents::EditingChanged, sel!(handleChange:)),
            "focus" => (UIControlEvents::EditingDidBegin, sel!(handleFocus:)),
            "blur" => (UIControlEvents::EditingDidEnd, sel!(handleBlur:)),
            "submit" => (UIControlEvents::EditingDidEndOnExit, sel!(handleSubmit:)),
            _ => return None,
        };
        let target = ControlTarget::new(mtm, node, queue);
        let control: *const UIView = view;
        let control = control.cast::<UIControl>();
        unsafe {
            (*control).addTarget_action_forControlEvents(Some(&*target), action, events);
        }
        return Some(AttachedListener::Control { events, action, target });
    }

    if kind == NodeKind::ScrollView && event == "scroll" {
        let delegate = ScrollDelegate::new(mtm, node, queue);
        let scroll: *const UIView = view;
        let scroll = scroll.cast::<UIScrollView>();
        unsafe { (*scroll).setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        return Some(AttachedListener::Scroll { _delegate: delegate });
    }

    if kind == NodeKind::StackView && event == "back" {
        // El gesto de sistema: arrastrar desde el borde izquierdo. Aquí solo
        // se avisa; deshacer la navegación es cosa del router.
        let target = GestureTarget::new(mtm, node, "back", queue);
        let recognizer = unsafe {
            UIScreenEdgePanGestureRecognizer::initWithTarget_action(
                UIScreenEdgePanGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(GestureTarget::edge_action()),
            )
        };
        recognizer.setEdges(UIRectEdge::Left);
        view.addGestureRecognizer(&recognizer);
        return Some(AttachedListener::Gesture {
            recognizer: Retained::into_super(Retained::into_super(recognizer)),
            _target: target,
        });
    }

    // Gestos continuos y de dirección. Cada uno lleva su reconocedor: UIKit
    // ya resuelve entre ellos quién gana cuando compiten.
    if let Some(recognizer) = continuous_gesture(mtm, event, node, &queue) {
        let (recognizer, target) = recognizer;
        view.setUserInteractionEnabled(true);
        view.addGestureRecognizer(&recognizer);
        return Some(AttachedListener::Gesture { recognizer, _target: target });
    }

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

    Some(AttachedListener::Gesture {
        recognizer: Retained::into_super(recognizer),
        _target: target,
    })
}
