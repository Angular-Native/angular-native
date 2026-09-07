//! Native events on their way back to JavaScript.
//!
//! Just as on iOS, AppKit delivers gestures and actions through target-action,
//! which demands a real Objective-C object as the target: two are defined
//! here, one for gestures and one for controls, each with the node's id and
//! the event queue inside it.
//!
//! **What changes on the desktop.** There are no fingers here, there is a
//! mouse and a trackpad, and AppKit's recognisers are not UIKit's:
//!
//! - `press` is a click, not a tap: `NSClickGestureRecognizer`. `doublePress`
//!   is the same one with two clicks.
//! - `longPress` is `NSPressGestureRecognizer`, which is holding the mouse
//!   button down, not the finger.
//! - `pan` is `NSPanGestureRecognizer`, dragging with the button held down.
//! - `pinch` is `NSMagnificationGestureRecognizer` and `rotate` is
//!   `NSRotationGestureRecognizer`: both belong to the trackpad and never
//!   happen with a mouse. They are attached all the same, because a Mac with a
//!   trackpad does give them.
//! - **The swipe is not a recogniser.** AppKit has no
//!   `NSSwipeGestureRecognizer`, but it does have the gesture: it arrives as
//!   `swipeWithEvent:` down the responder chain, so it is not attached to just
//!   any view, it has to be handled on the class. Hence it living in
//!   `flipped.rs` and not here. It is not imitated with a `pan` and a
//!   threshold: the thresholds are the system's.
//! - **And there is something a phone does not have: the pointer.** Being over
//!   a view is an event —`hover`— and the cursor's shape is a prop. Both are
//!   set up with an `NSTrackingArea`, which unlike a recogniser does not have
//!   to be handled by the view itself: the area's owner is a separate object,
//!   which is why they work the same over a system `NSButton` as over a view
//!   of ours.
//!
//! What does not change is when they are dispatched: the event is queued and
//! delivered at the start of the next frame, so that everything that happened
//! between two vsyncs is processed together.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadOnly, Message};
use objc2_app_kit::{
    NSClickGestureRecognizer, NSControl, NSCursor, NSDatePicker, NSEvent, NSGestureRecognizer,
    NSGestureRecognizerState, NSMagnificationGestureRecognizer, NSPanGestureRecognizer,
    NSControlTextEditingDelegate, NSPopUpButton, NSPressGestureRecognizer,
    NSRotationGestureRecognizer, NSScrollView, NSSegmentedControl, NSSlider, NSStepper, NSSwitch,
    NSTextField,
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
    // - NSObject places no requirements on its subclasses.
    // - AnGestureTarget does not implement Drop.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnMacGestureTarget"]
    #[ivars = TargetIvars]
    pub struct GestureTarget;

    unsafe impl NSObjectProtocol for GestureTarget {}

    impl GestureTarget {
        /// Click and double click. An `NSClickGestureRecognizer` only fires
        /// once the gesture is over, so there is no state to filter on.
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

        /// Dragging with the button held down. It carries translation and
        /// velocity, which is what it takes to move something with the mouse
        /// and to decide whether it keeps going under its own momentum when
        /// released.
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
                    // AppKit gives the translation in the view's coordinates,
                    // and this host's views have their origin at the top (see
                    // `flipped.rs`), so `y` already grows downwards and agrees
                    // with iOS's and Android's.
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

        /// Press and hold. It is only reported at the start, just as on iOS:
        /// the system has already decided the gesture counts.
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
            // AppKit gives magnification as an increment over 1 and UIKit
            // gives the scale. The scale is what is sent, because that is what
            // the template expects.
            let scale = 1.0 + unsafe { recognizer.magnification() };
            emit(
                &ivars.queue,
                ivars.node,
                ivars.name,
                vec![
                    ("scale".to_owned(), PropValue::Number(scale)),
                    // The trackpad gives no magnification velocity; zero is
                    // honest and breaks nobody who reads it.
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

/// The state's name, exactly as the template will see it. The same four as on
/// iOS: a template has no business knowing which platform it runs on.
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
    // SAFETY: the same as GestureTarget.
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

        /// macOS's switch reports through an action and not through
        /// `ValueChanged`: `NSSwitch` is an `NSControl` and its state is `1`
        /// or `0`.
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

        /// macOS's tab bar is a segmented control (see `support.rs`), so the
        /// selection arrives down the same path but is called `select`, which
        /// is how the primitive names it.
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
            // Milliseconds since 1970, which is what `Date` understands in
            // JS.
            let seconds = unsafe { sender.dateValue().timeIntervalSince1970() };
            emit(
                &ivars.queue,
                ivars.node,
                "change",
                vec![("value".to_owned(), PropValue::Number(seconds * 1000.0))],
            );
        }

        /// An AppKit text field sends its action on Return, not on every
        /// keystroke; the per-keystroke part goes through the delegate, which
        /// is a different path and a good deal more expensive. `submit` is
        /// what this action means.
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

    /// What a field reports while it is being typed into does not arrive
    /// through an action but through a delegate: an `NSTextField`'s action
    /// only fires on Return.
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

/// The pointer, which on a phone does not exist.
///
/// This is an `NSTrackingArea`'s owner, not the view: `NSTrackingArea` takes
/// any object as its owner and sends the entries and the exits to it. That is
/// what makes `(hover)` work the same over a system `NSButton` as over a view
/// of ours, without subclassing anything.
pub struct HoverIvars {
    node: NodeId,
    queue: EventQueue,
    /// The view being watched. It is needed to give the point in its own
    /// coordinates: the event carries the window's, and `NSTrackingArea` does
    /// not say whose it is.
    ///
    /// Retaining it leaves no cycle: the view retains the area, the area
    /// points at the owner weakly, and what retains the owner is the host,
    /// which releases both when the node is destroyed.
    view: Retained<NSView>,
}

define_class!(
    // SAFETY: the same as GestureTarget.
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

    /// One single event with a boolean, which is how the primitive declares
    /// it.
    ///
    /// The point goes in the view's coordinates, just as a `press`'s does. On
    /// exit it is the last one the pointer passed through — that is, the edge
    /// it left by.
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

/// A scroll view's offset, on its way to `(scroll)`.
///
/// On iOS this is a delegate method. AppKit has no scroll delegate: an
/// `NSScrollView` reports movement by posting `NSViewBoundsDidChange` from its
/// **clip view**, and only if that clip view has been asked to
/// (`postsBoundsChangedNotifications`, which is off by default — the one line
/// whose absence makes this look like AppKit simply not telling anyone).
///
/// The offset is the clip view's `bounds.origin`, and it counts downwards
/// because the document view is flipped, the same as `crates/an-macos/src/flipped.rs`
/// explains for everything else this host mounts. So the numbers reaching a
/// template are the ones iOS sends, and a component listening to `(scroll)`
/// needs to know nothing about which desktop it is on.
pub struct ScrollIvars {
    node: NodeId,
    queue: EventQueue,
    clip: Retained<NSView>,
}

define_class!(
    // SAFETY: the same as GestureTarget.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnMacScrollTarget"]
    #[ivars = ScrollIvars]
    pub struct ScrollTarget;

    unsafe impl NSObjectProtocol for ScrollTarget {}

    impl ScrollTarget {
        #[unsafe(method(boundsDidChange:))]
        fn bounds_did_change(&self, _notification: &objc2_foundation::NSNotification) {
            let ivars = self.ivars();
            let origin = ivars.clip.bounds().origin;
            emit(
                &ivars.queue,
                ivars.node,
                "scroll",
                vec![
                    ("x".to_owned(), PropValue::Number(origin.x)),
                    ("y".to_owned(), PropValue::Number(origin.y)),
                ],
            );
        }
    }
);

impl ScrollTarget {
    fn new(
        mtm: objc2::MainThreadMarker,
        node: NodeId,
        queue: EventQueue,
        clip: Retained<NSView>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ScrollIvars { node, queue, clip });
        unsafe { msg_send![super(this), init] }
    }
}

/// The pointer's shape over a view.
///
/// This too owns an `NSTrackingArea`, and for the same reason: setting a
/// cursor with `addCursorRect:cursor:` requires overriding `resetCursorRects`
/// on the view, and this host's views are mostly system controls. With
/// `NSTrackingCursorUpdate` the system asks the area's owner right when the
/// pointer enters, which is when the answer is due.
pub struct CursorIvars {
    cursor: Retained<NSCursor>,
}

define_class!(
    // SAFETY: the same as GestureTarget.
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

/// The system cursor each of the primitive's names gets.
///
/// `None` means a name that is not in the vocabulary: the caller warns. None
/// of them is drawn by hand; they are all the system's, looking however they
/// look on that version of macOS.
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

/// The area covering a whole view, now and after every change of size.
///
/// `InVisibleRect` is what saves having to rebuild it on every `set_layout`:
/// with it AppKit keeps the rectangle glued to the view's and the one passed
/// in here is ignored. Without it, an area would stay the size the view was
/// when somebody subscribed, and on resizing the window —which on the desktop
/// happens constantly— the pointer would enter and leave over ground where
/// there is nothing any more.
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

/// Sets this view's cursor, taking away whatever was there.
///
/// It returns what has to be kept alive: AppKit holds the area's owner by weak
/// reference, so releasing it here would leave an area that does not answer
/// and a pointer that does not change, with no error at all.
pub fn attach_cursor(
    mtm: objc2::MainThreadMarker,
    view: &NSView,
    cursor: Retained<NSCursor>,
) -> (Retained<NSTrackingArea>, Retained<CursorTarget>) {
    let target = CursorTarget::new(mtm, cursor);
    // `ActiveInKeyWindow` is what AppKit does with its own cursor rects: the
    // pointer's shape is the business of the window being worked in, not of
    // one sitting behind it.
    let area = tracking_area(
        view,
        NSTrackingAreaOptions::CursorUpdate | NSTrackingAreaOptions::ActiveInKeyWindow,
        &target,
    );
    (area, target)
}

/// A live subscription. It holds what AppKit references weakly, which is
/// exactly what gets freed on its own if nobody retains it: an action's target
/// and a field's delegate.
pub enum AttachedListener {
    Gesture { recognizer: Retained<NSGestureRecognizer>, _target: Retained<GestureTarget> },
    /// An `NSControl`'s action. AppKit allows only **one** per control, so
    /// two subscriptions on the same control would tread on each other; in
    /// practice that does not happen because each control has a single event
    /// to give.
    Action { _target: Retained<ControlTarget> },
    /// A text field's delegate, which is where the three things a field
    /// reports while it is being typed into come from.
    FieldDelegate { _target: Retained<ControlTarget> },
    /// The pointer hovering. It is not a recogniser: it is an
    /// `NSTrackingArea` with a separate owner, which is what makes watching a
    /// system control possible.
    Hover { area: Retained<NSTrackingArea>, _target: Retained<HoverTarget> },
    /// One swipe direction. There is nothing to attach: the method is already
    /// on the view's class (see `flipped.rs`); what is kept is who has to be
    /// told and which direction stops being listened for on release.
    Swipe { view: Retained<crate::flipped::FlippedView>, bit: u8 },
    /// A scroll view's offset. What is kept is the observer, because the
    /// notification centre holds it **unowned**: dropping it without removing
    /// it first leaves the centre posting to freed memory.
    Scroll { _target: Retained<ScrollTarget>, clip: Retained<NSView> },
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
            AttachedListener::Scroll { _target, clip } => unsafe {
                objc2_foundation::NSNotificationCenter::defaultCenter()
                    .removeObserver_name_object(
                        &**_target,
                        Some(objc2_app_kit::NSViewBoundsDidChangeNotification),
                        Some(clip),
                    );
            },
        }
    }
}

/// Which `NSControl` action each (primitive, event) pair gets.
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

/// Attaches an event to a view. `None` means this platform does not know how
/// to deliver it; the caller decides whether that is worth a warning.
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

    // What a field reports while it is being typed into arrives through a
    // delegate. `change`/`input`, `focus` and `blur` are `NSControl`'s three
    // notifications, and the same target handles them all: attaching it once
    // covers the three.
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

    // Scrolling. AppKit has no delegate for it: the clip view posts a
    // notification, and only once it has been told to.
    if kind == NodeKind::ScrollView && event == "scroll" {
        let scroll: *const NSView = view;
        let scroll = scroll.cast::<NSScrollView>();
        let clip = unsafe { (*scroll).contentView() };
        // Off by default. Without it nothing is ever posted and the whole
        // subscription is silence with no error anywhere.
        unsafe { clip.setPostsBoundsChangedNotifications(true) };
        let clip: Retained<NSView> = Retained::into_super(clip);
        let target = ScrollTarget::new(mtm, node, queue, clip.clone());
        unsafe {
            objc2_foundation::NSNotificationCenter::defaultCenter()
                .addObserver_selector_name_object(
                    &*target,
                    sel!(boundsDidChange:),
                    Some(objc2_app_kit::NSViewBoundsDidChangeNotification),
                    Some(&clip),
                );
        }
        return Some(AttachedListener::Scroll { _target: target, clip });
    }

    // The pointer hovering. It goes before the gestures because it is not one
    // of them: there is no recogniser, there is a watched area, and the area's
    // owner is a separate object. Which is why it works over any view, the
    // system's or ours.
    if event == "hover" {
        let target = HoverTarget::new(mtm, node, queue, view.retain());
        // `ActiveInActiveApp` and not `ActiveAlways`: on a Mac controls only
        // light up under the pointer when the app is in front, and this one is
        // not going to be the exception that behaves differently from the rest
        // of the desktop.
        let area = tracking_area(
            view,
            NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveInActiveApp,
            &target,
        );
        return Some(AttachedListener::Hover { area, _target: target });
    }

    // Continuous gestures.
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

    // Click and double click.
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
