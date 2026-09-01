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
    UIControl, UIControlEvents, UIGestureRecognizer, UIGestureRecognizerState, UIRectEdge,
    UIScreenEdgePanGestureRecognizer, UIScrollView, UIScrollViewDelegate, UISlider, UISwitch,
    UITabBar, UITabBarDelegate, UITabBarItem, UITapGestureRecognizer, UITextField, UIView,
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

        #[unsafe(method(handleButton:))]
        fn handle_button(&self, _sender: &UIControl) {
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, "press", Vec::new());
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

    unsafe impl UITabBarDelegate for TabDelegate {
        #[unsafe(method(tabBar:didSelectItem:))]
        fn tab_bar_did_select(&self, _bar: &UITabBar, item: &UITabBarItem) {
            let ivars = self.ivars();
            emit(
                &ivars.queue,
                ivars.node,
                "select",
                // El `tag` es el índice: se le puso al construir el ítem.
                vec![("index".to_owned(), PropValue::Number(item.tag() as f64))],
            );
        }
    }
);

impl TabDelegate {
    fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TabIvars { node, queue });
        unsafe { msg_send![super(this), init] }
    }
}

impl ControlTarget {
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
            AttachedListener::Tabs { .. } => {
                let bar: *const UIView = view;
                let bar = bar.cast::<UITabBar>();
                unsafe { (*bar).setDelegate(None) };
            }
        }
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
        _ => None,
    };
    if let Some((events, action)) = control_action {
        let target = ControlTarget::new(mtm, node, queue);
        let control: *const UIView = view;
        let control = control.cast::<UIControl>();
        unsafe { (*control).addTarget_action_forControlEvents(Some(&*target), action, events) };
        return Some(AttachedListener::Control { events, action, target });
    }

    if kind == NodeKind::TabBar && event == "select" {
        let delegate = TabDelegate::new(mtm, node, queue);
        let bar: *const UIView = view;
        let bar = bar.cast::<UITabBar>();
        unsafe { (*bar).setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        return Some(AttachedListener::Tabs { _delegate: delegate });
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
