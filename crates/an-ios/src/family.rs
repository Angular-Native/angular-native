//! What each UIKit family actually ships.
//!
//! `an-ios` compiles for iOS, tvOS and visionOS. All three ship UIKit and all
//! three mount `UIView`s with absolute frames, so the host is a single one.
//! What is not a single one is the catalogue of controls: `UISwitch`,
//! `UISlider`, `UIStepper`, `UIDatePicker` and `WKWebView` are marked
//! `API_UNAVAILABLE(tvos)` in the SDK.
//!
//! The compiler is no help in noticing. `objc2-ui-kit` generates the bindings
//! for every Apple platform without looking at the availability annotation, so
//! `UISwitch::new(mtm)` **compiles** for tvOS and what fails is the class
//! lookup at run time, by then inside the simulator and with the process
//! aborting. Hence this list being written by hand: it is the SDK's annotation
//! brought over into Rust, and it is what separates "this cannot be done" from
//! "it quits without saying why".

use an_core::NodeKind;

/// The family's name, for the messages.
pub const NAME: &str = if cfg!(target_os = "tvos") {
    "tvOS"
} else if cfg!(target_os = "visionos") {
    "visionOS"
} else {
    "iOS"
};

/// Why this family cannot mount this primitive, or `None` if it can.
///
/// The reason is written out in full because it ends up in the device's log,
/// which is where somebody is going to read it without this file in front of
/// them.
pub fn missing_kind(kind: NodeKind) -> Option<&'static str> {
    #[cfg(target_os = "tvos")]
    {
        match kind {
            // iOS's three value controls do not exist in the tvOS SDK. It is
            // not that they look different: the class is not in UIKit.
            NodeKind::Switch => Some(
                "UISwitch does not exist on tvOS. A television's screen is driven with the \
                 remote, and there a switch is a focusable row you press; there is no \
                 equivalent system control",
            ),
            NodeKind::Slider => Some(
                "UISlider does not exist on tvOS. The nearest thing the platform does ship is \
                 UIProgressView, which only shows a value: it cannot be dragged",
            ),
            NodeKind::Stepper => Some("UIStepper does not exist on tvOS"),
            NodeKind::DatePicker => Some(
                "UIDatePicker does not exist on tvOS. The system asks for dates with a screen \
                 of its own, not with a control that fits in a frame",
            ),
            // The whole of WebKit: the tvOS SDK does not ship the framework.
            NodeKind::WebView => {
                Some("WebKit is not part of the tvOS SDK: there is no WKWebView to mount")
            }
            _ => None,
        }
    }
    #[cfg(not(target_os = "tvos"))]
    {
        let _ = kind;
        None
    }
}

/// Says it once and does not repeat it.
///
/// The host calls in here from `create`, and `create` runs every time the tree
/// registers a node: without the filter, an `@for` over twenty switches would
/// fill the log twenty times and the warning would stop being read.
///
/// The state is `thread_local` and not a `Mutex` because all of this lives on
/// the main thread —UIKit will have it no other way— and so there is no lock
/// to take in the middle of a frame.
pub fn report(what: &str, why: &str) {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::io::Write;

    thread_local! {
        static SAID: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    }

    let fresh = SAID.with(|said| said.borrow_mut().insert(what.to_owned()));
    if !fresh {
        return;
    }
    // By hand and with a `flush`, for the same reason as `ffi.rs`'s panic
    // hook: if the process quits right afterwards, whatever was left in the
    // buffer never gets out and the warning is lost precisely when it is most
    // needed.
    let mut out = std::io::stderr().lock();
    let _ = writeln!(out, "angular-native: {what} is not available on {NAME}: {why}");
    let _ = out.flush();
}

/// The event names the framework knows how to send.
///
/// Two kinds of name reach a host. One is `nativeEvent()`'s in
/// `packages/primitives` —`press`, `change`, `scroll`— and that one really is a
/// request. The other is the name of the directive's *output* —`onChange`,
/// `valueChange`—: Angular registers an element listener for every `(output)`
/// that appears in a template, and that name corresponds to no platform event
/// on any host.
///
/// Warning about the second kind would mean warning on every startup about
/// something that works, and a warning that always comes out is a warning
/// nobody reads. Only the first kind is warned about: what somebody genuinely
/// asked for and this family does not give. The list is `an-macos`'s
/// `support::KNOWN_EVENTS` plus the two the watch has, because the outputs are
/// declared once, on the base directive, and every platform sees all of them.
pub const KNOWN_EVENTS: &[&str] = &[
    "press", "doublePress", "longPress", "pan", "pinch", "rotate", "swipeLeft", "swipeRight",
    "swipeUp", "swipeDown", "hover", "layout", "safeArea", "back", "refresh", "scroll", "load",
    "change", "input", "focus", "blur", "submit", "select", "dismiss", "crown", "crownIdle",
];

pub fn is_known_event(event: &str) -> bool {
    KNOWN_EVENTS.contains(&event)
}

/// Events this family cannot deliver, with the reason.
///
/// It is consulted on subscription and not on firing, for the same reason
/// `missing_kind` is consulted on creation: an output that never arrives gives
/// nobody anything to look at. `scripts/check-platform-gaps.sh` reads this
/// function and requires every event named here to be named on the platform's
/// page, in both languages.
pub fn unsupported_event(kind: NodeKind, event: &str) -> Option<&'static str> {
    on_every_family(kind, event).or_else(|| on_this_family(kind, event))
}

/// What none of the three UIKit families delivers.
fn on_every_family(_kind: NodeKind, event: &str) -> Option<&'static str> {
    match event {
        // The decision, so it is not made again from scratch: UIKit *does*
        // ship `UIHoverGestureRecognizer`, from iOS 13 and tvOS 16, and it is
        // in the SDK all three families compile against. It is not attached
        // anyway.
        //
        // What it reports is a pointer — an iPad trackpad, a mouse turned on
        // through AssistiveTouch, an Apple Pencil held above the glass — and
        // none of the three is on the device the app is written for. An output
        // that fires on the reviewer's iPad and never on the user's phone is
        // worse than one that does not exist: it is tested once and shipped
        // broken. And `[cursor]`, the prop that goes with `(hover)`, has no
        // meaning here at all, so half the pair could arrive and half could
        // not.
        //
        // An interface that only answers to a pointer cannot be used with a
        // finger. `(hover)` stays a desktop event, and the way to say so is to
        // say so.
        "hover" => Some(
            "the pointer is the desktop's. UIHoverGestureRecognizer is in the SDK, but what it \
             reports is a trackpad, a mouse or a Pencil held above the glass — hardware most of \
             these devices do not have — and [cursor], the prop that goes with (hover), has no \
             meaning here at all. Use (press) and (longPress), which every one of them has",
        ),
        // The crown is the Apple Watch's, and the output is declared on the
        // base directive, so every primitive on every platform carries it.
        "crown" | "crownIdle" => Some(
            "the digital crown belongs to the Apple Watch. There is no wheel to turn on this \
             device, so this output would never fire",
        ),
        _ => None,
    }
}

#[cfg(target_os = "tvos")]
fn on_this_family(kind: NodeKind, event: &str) -> Option<&'static str> {
    match (kind, event) {
        // Both classes are marked `API_UNAVAILABLE(tvos)`: asking objc2 for
        // them would close the app. `pan` and the four `swipe`s do go through
        // — the remote's surface sends indirect touches and UIKit recognises
        // them like a finger's.
        (_, "pinch" | "rotate") => Some(
            "the remote has a single-touch surface, and UIPinchGestureRecognizer and \
             UIRotationGestureRecognizer are not in the SDK",
        ),
        (NodeKind::ScrollView, "refresh") => Some(
            "UIRefreshControl is not in the SDK, and on a television there is nothing to pull",
        ),
        _ => None,
    }
}

#[cfg(target_os = "visionos")]
fn on_this_family(kind: NodeKind, event: &str) -> Option<&'static str> {
    match (kind, event) {
        (NodeKind::StackView, "back") => Some(
            "there is no back gesture: UIScreenEdgePanGestureRecognizer is not in the SDK and \
             the window has no edges to drag from. The way back has to be a button in the \
             template",
        ),
        _ => None,
    }
}

#[cfg(target_os = "ios")]
fn on_this_family(_kind: NodeKind, _event: &str) -> Option<&'static str> {
    None
}
