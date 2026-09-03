//! Native gestures on their way back to JavaScript.
//!
//! UIKit delivers gestures through target-action, which demands a real
//! Objective-C object as the target. One is defined here: it keeps the node's
//! id and the event queue, and on firing it queues a `HostEvent`.
//!
//! The event is not dispatched on the spot: it waits for the start of the next
//! frame. That way everything that happened between two vsyncs is processed
//! together, and the response to a touch is seen in the same frame it is
//! processed in.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_foundation::NSObjectProtocol;
use objc2_ui_kit::{
    UITabBarController, UITabBarControllerDelegate, UIViewController,
    UIControl, UIControlEvents, UIGestureRecognizer, UIGestureRecognizerState,
    UILongPressGestureRecognizer, UIPanGestureRecognizer, UIPinchGestureRecognizer,
    UIRotationGestureRecognizer, UIScrollView,
    UIScrollViewDelegate, UISlider, UISwipeGestureRecognizer,
    UISwipeGestureRecognizerDirection, UISwitch, UITabBar, UITapGestureRecognizer, UITextField,
    UIView,
};
// The drag in from the edge and the refresh control are iOS's: the SDK marks
// them `API_UNAVAILABLE(tvos, visionos)` and `API_UNAVAILABLE(tvos)`. See
// `family.rs` and the part of `attach` that stands in for them.
#[cfg(not(any(target_os = "tvos", target_os = "visionos")))]
use objc2_ui_kit::{UIRectEdge, UIScreenEdgePanGestureRecognizer};
#[cfg(not(target_os = "tvos"))]
use objc2_ui_kit::UIRefreshControl;

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
    // - NSObject places no requirements on its subclasses.
    // - AnGestureTarget does not implement Drop.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnGestureTarget"]
    #[ivars = TargetIvars]
    pub struct GestureTarget;

    impl GestureTarget {
        /// The drag-in-from-the-edge back gesture. It only counts on
        /// release: a drag that gets cancelled must not navigate.
        #[unsafe(method(handleEdgePan:))]
        fn handle_edge_pan(&self, recognizer: &UIGestureRecognizer) {
            if recognizer.state() != UIGestureRecognizerState::Ended {
                return;
            }
            let ivars = self.ivars();
            emit(&ivars.queue, ivars.node, ivars.name, Vec::new());
        }

        /// Dragging. It carries the accumulated translation and the
        /// velocity, which is what it takes to move something with a finger
        /// and to decide whether it keeps going under its own momentum when
        /// released.
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

        /// Press and hold. It is only reported at the start: the system has
        /// already decided the gesture counts, and reporting on release as
        /// well would only give a second event nobody expects.
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

/// The state's name, exactly as the template will see it.
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

/// The target of a `UIControl`'s actions: typing, and entering and leaving a
/// text field.
pub struct ControlIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: the same as GestureTarget.
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
            // Milliseconds since 1970, which is what `Date` understands in
            // JS with nobody having to convert anything.
            let seconds = unsafe { sender.date().timeIntervalSince1970() };
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(seconds * 1000.0))],
            );
        }

        /// A header's back button.
        ///
        /// Outside a `UINavigationController` there is no automatic back: the
        /// button is put there by hand and what navigates is the router, so
        /// all that happens here is a notification.
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

/// The tab bar's delegate. UIKit holds it by weak reference.
pub struct TabIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: the same as GestureTarget.
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
                // The `tag` is the index: it was set when the tab was
                // built.
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
    /// A loose target, to attach to something that is not a `UIControl` —a
    /// `UIBarButtonItem`, for instance.
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

/// The scroll delegate. UIKit holds it by weak reference, so it has to be kept
/// alive here for as long as the view exists.
pub struct ScrollIvars {
    node: NodeId,
    queue: EventQueue,
}

define_class!(
    // SAFETY: the same as GestureTarget.
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
    /// The system's refresh control, if this subscription brought one.
    #[cfg(not(target_os = "tvos"))]
    pub fn refresh_control(&self) -> Option<&UIRefreshControl> {
        match self {
            AttachedListener::Refresh { control, .. } => Some(control),
            _ => None,
        }
    }
}

/// A live subscription. It holds what UIKit references weakly, which is
/// exactly what gets freed on its own if nobody retains it.
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
    /// Pull to refresh. It does not exist on tvOS: `UIRefreshControl` is not
    /// in its SDK, and with no touches there would be nothing to pull
    /// anyway.
    #[cfg(not(target_os = "tvos"))]
    Refresh {
        _target: Retained<ControlTarget>,
        control: Retained<UIRefreshControl>,
    },
    /// A view that only wants to know when the remote is looking at it. It
    /// carries no recogniser: the view itself gives the notification when it
    /// takes focus, and this exists only so it can be switched off on
    /// unsubscribing.
    #[cfg(target_os = "tvos")]
    Focus,
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
            #[cfg(not(target_os = "tvos"))]
            AttachedListener::Refresh { .. } => {
                let scroll: *const UIView = view;
                unsafe { (*scroll.cast::<UIScrollView>()).setRefreshControl(None) };
            }
            #[cfg(target_os = "tvos")]
            AttachedListener::Focus => {
                if let Some(focusable) = crate::focus::focusable(view) {
                    focusable.set_wants_focus(false);
                }
            }
            AttachedListener::Tabs { .. } => {
                let bar: *const UIView = view;
                let bar = bar.cast::<UITabBar>();
                unsafe { (*bar).setDelegate(None) };
            }
        }
    }
}

/// Builds the recogniser for a continuous or directional gesture, if the name
/// is one of theirs.
fn continuous_gesture(
    mtm: objc2::MainThreadMarker,
    event: &str,
    node: NodeId,
    queue: &EventQueue,
) -> Option<(Retained<UIGestureRecognizer>, Retained<GestureTarget>)> {
    // Pinching and rotating ask for two fingers at once. The tvOS remote's
    // surface is single-touch and the SDK says so without hedging: both
    // classes are marked `API_UNAVAILABLE(tvos)`. Asking objc2 for the class
    // here would close the app, so it is said and nothing is attached.
    //
    // `pan` and the four `swipe`s do go through: the remote's surface sends
    // indirect touches and UIKit recognises them just like a finger's.
    #[cfg(target_os = "tvos")]
    if matches!(event, "pinch" | "rotate") {
        crate::family::report(
            &format!("({event})"),
            "the remote has a single-touch surface, and UIPinchGestureRecognizer and \
             UIRotationGestureRecognizer are not in the SDK",
        );
        return None;
    }

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
            let long_press = UILongPressGestureRecognizer::initWithTarget_action(
                UILongPressGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(action),
            );
            // On a television, holding is holding the centre button down, not
            // keeping a finger still on the screen.
            #[cfg(target_os = "tvos")]
            crate::focus::allow_press(&long_press, crate::focus::SELECT);
            long_press
        }),
        #[cfg(not(target_os = "tvos"))]
        "pinch" => Retained::into_super(unsafe {
            UIPinchGestureRecognizer::initWithTarget_action(
                UIPinchGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(action),
            )
        }),
        #[cfg(not(target_os = "tvos"))]
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

/// The event's name has to live as long as the gesture's target does, and the
/// names are a closed, known set.
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

/// The event names this platform knows how to recognise. The rest are ignored
/// in silence: a template may carry a `(click)` inherited from the web, and
/// that is no reason to crash the app.
///
/// `kind` decides which UIKit mechanism is used: gestures for ordinary views,
/// target-action for text fields, a delegate for scrolling.
pub fn attach(
    mtm: objc2::MainThreadMarker,
    view: &UIView,
    kind: an_core::NodeKind,
    node: NodeId,
    event: &str,
    queue: EventQueue,
) -> Option<AttachedListener> {
    use an_core::NodeKind;

    // Controls that report through target-action: the value changed, or it
    // was pressed.
    let control_action = match (kind, event) {
        (NodeKind::Switch, "change") => Some((UIControlEvents::ValueChanged, sel!(handleSwitch:))),
        (NodeKind::Slider, "change") => Some((UIControlEvents::ValueChanged, sel!(handleSlider:))),
        // The button, and here the two families cannot be treated alike.
        //
        // On a phone it is touched, and UIKit sends `TouchUpInside`. On a
        // television there are no touches: the remote presses the centre
        // button over whatever is focused and UIKit sends
        // `PrimaryActionTriggered`; `TouchUpInside` never arrives. The
        // recogniser attaches all the same, nothing fails, and the button
        // simply does not respond. It is the textbook silent failure, and it
        // took a while to spot with the button focused and white on screen.
        #[cfg(not(target_os = "tvos"))]
        (NodeKind::Button, "press") => Some((UIControlEvents::TouchUpInside, sel!(handleButton:))),
        #[cfg(target_os = "tvos")]
        (NodeKind::Button, "press") => {
            Some((UIControlEvents::PrimaryActionTriggered, sel!(handleButton:)))
        }
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

    // The search bar is not a control: its text field is, and that one really
    // is a `UITextField`. Attaching to it saves implementing the whole
    // delegate just to learn that somebody typed.
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

    // Pull to refresh. On iOS the system draws it: a `UIRefreshControl` is
    // attached to the scroll view and it supplies the spinner and the
    // animation.
    //
    // On tvOS it does not: the class is not in the SDK, and even if it were
    // there is no finger to pull with. It is said and nothing is attached,
    // rather than leaving a `(refresh)` that never fires.
    #[cfg(target_os = "tvos")]
    if kind == NodeKind::ScrollView && event == "refresh" {
        crate::family::report(
            "(refresh)",
            "UIRefreshControl is not in the SDK, and on a television there is nothing to pull",
        );
        return None;
    }
    #[cfg(not(target_os = "tvos"))]
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

    // The controller reports the tab selection, not the view: the host
    // attaches the delegate when it creates it, because only the view gets
    // here.
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

    // Going back. It is the same event in all three families and the same
    // gesture in none of them: `UIScreenEdgePanGestureRecognizer` is marked
    // `API_UNAVAILABLE(tvos, visionos)`, and rightly so —a television has no
    // edge to drag from and neither does a volumetric window.
    #[cfg(not(any(target_os = "tvos", target_os = "visionos")))]
    if kind == NodeKind::StackView && event == "back" {
        // The system's gesture: dragging in from the left edge. All that
        // happens here is a notification; undoing the navigation is the
        // router's business.
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

    // On tvOS the system's "back" is the remote's menu button. It is not a
    // translation of the gesture on our part: it is the button the platform
    // reserves for that, and a tvOS app that does not answer it feels
    // broken.
    #[cfg(target_os = "tvos")]
    if kind == NodeKind::StackView && event == "back" {
        let target = GestureTarget::new(mtm, node, "back", queue);
        let recognizer = unsafe {
            UITapGestureRecognizer::initWithTarget_action(
                UITapGestureRecognizer::alloc(mtm),
                Some(&target),
                Some(GestureTarget::action()),
            )
        };
        crate::focus::allow_press(&recognizer, crate::focus::MENU);
        view.addGestureRecognizer(&recognizer);
        return Some(AttachedListener::Gesture {
            recognizer: Retained::into_super(recognizer),
            _target: target,
        });
    }

    // visionOS has no system gesture for going back: the window is closed
    // through its bar, and inside the app the way back is a button the
    // template puts there. It is said, because a `(back)` that never arrives
    // is exactly what this project must not let through.
    #[cfg(target_os = "visionos")]
    if kind == NodeKind::StackView && event == "back" {
        crate::family::report(
            "(back)",
            "there is no back gesture: UIScreenEdgePanGestureRecognizer is not in the SDK and \
             the window has no edges to drag from. The way back has to be a button in the \
             template",
        );
        return None;
    }

    // Continuous and directional gestures. Each carries its own recogniser:
    // UIKit already settles between them who wins when they compete.
    if let Some(recognizer) = continuous_gesture(mtm, event, node, &queue) {
        let (recognizer, target) = recognizer;
        view.setUserInteractionEnabled(true);
        view.addGestureRecognizer(&recognizer);
        return Some(AttachedListener::Gesture { recognizer, _target: target });
    }

    // On tvOS, `(focus)` and `(blur)` on a view with no press: the view
    // reports on its own when the remote reaches it, but only if it declares
    // itself focusable, and one that is listening for nothing else has no
    // recogniser to give it away. See `focus.rs`.
    #[cfg(target_os = "tvos")]
    if matches!(event, "focus" | "blur") {
        return match crate::focus::focusable(view) {
            Some(focusable) => {
                focusable.set_wants_focus(true);
                Some(AttachedListener::Focus)
            }
            None => {
                crate::family::report(
                    &format!("({event}) on <{kind:?}>"),
                    "only <an-view> can take the remote's focus: the rest of the primitives are \
                     UILabel, UIImageView and controls, and canBecomeFocused can only be changed \
                     by inheriting. Wrap it in an <an-view>",
                );
                None
            }
        };
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

    // UILabel and UIImageView come with interaction switched off out of the
    // box: without this the gesture registers and never fires.
    view.setUserInteractionEnabled(true);

    // tvOS: with no focus there is no press.
    //
    // The recogniser attaches just as on iOS and nothing fails, but the centre
    // button only reaches the view the focus engine has selected. A `UIView`
    // answers `NO` to `canBecomeFocused`, so the remote can never come to rest
    // on it and `(press)` never fires. There is no error and no warning from
    // the system: nothing happens at all. Hence `an-view` being created on
    // tvOS as an `AnFocusableView`, which answers yes as soon as it has a
    // gesture on it —precisely the one being attached here.
    #[cfg(target_os = "tvos")]
    {
        crate::focus::allow_press(&recognizer, crate::focus::SELECT);
        match crate::focus::focusable(view) {
            Some(focusable) => focusable.allow_interaction(),
            None => crate::family::report(
                &format!("({event}) on <{kind:?}>"),
                "on a television only what the remote can focus can be pressed, and of the \
                 primitives only <an-view> and the system's controls are. Wrap it in an \
                 <an-view> and put the (press) there",
            ),
        }
    }

    // visionOS: the user aims with their gaze, and without a highlight they
    // cannot see at what.
    //
    // The system draws it outside the process, but only if the view asks for
    // it: `hoverStyle` is `nil` out of the box. The controls come with it set;
    // a `UIView` with a `(press)` does not.
    #[cfg(target_os = "visionos")]
    crate::hover::mark_pressable(mtm, view);

    view.addGestureRecognizer(&recognizer);

    Some(AttachedListener::Gesture {
        recognizer: Retained::into_super(recognizer),
        _target: target,
    })
}
