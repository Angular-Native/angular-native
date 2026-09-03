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
