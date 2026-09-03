//! What this host paints and what it does not, said once and in one place.
//!
//! The rest of the project has the rule that nothing fails in silence, and in
//! a host the easiest way to break it is a `_ => {}` in `create`'s `match`:
//! the primitive mounts as an empty box, takes its place in the layout and is
//! not seen. No error, no trace, nothing to look at.
//!
//! So the inventory is here, outside `cfg(target_os = "macos")`, and not
//! inside the `match`. That buys three things:
//!
//! 1. `create` can walk the whole enum with no wildcard: if somebody adds a
//!    `NodeKind` to the core, this file stops compiling.
//! 2. A primitive macOS does not cover says so through the error output the
//!    first time it turns up, with the reason, instead of not being seen.
//! 3. `scripts/check-macos.sh` reads this table and compares it with the
//!    core's enum, so the list does not fall behind unnoticed either.

use an_core::NodeKind;

/// What macOS draws a primitive with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Support {
    /// There is a system control that is exactly this. The text is the name
    /// of the AppKit class, which is what comes out in the report.
    Native(&'static str),
    /// It does not exist as such in AppKit and is assembled out of system
    /// views. Which is which gets said, the same way Android does for the tab
    /// bar.
    Assembled(&'static str),
    /// macOS does not ship it and it is not imitated. The text explains why.
    Missing(&'static str),
    /// What the primitive asks for is honoured, but not with a view in the
    /// tree: macOS puts it somewhere else. The text says where.
    ///
    /// This is not a `Missing` dressed in kind words. A `Missing` leaves the
    /// template without what it asked for; this gives it to it where the
    /// platform keeps it, which on a Mac is nearly always outside the content
    /// window: the title bar, the menu bar, the Dock.
    Elsewhere(&'static str),
}

/// Every `NodeKind` that can be mounted, with what macOS puts behind it.
///
/// `RawText` is not here: it is internal and never becomes a view.
pub const SUPPORT: &[(NodeKind, Support)] = &[
    (NodeKind::View, Support::Native("NSView")),
    (NodeKind::Text, Support::Native("NSTextField")),
    (NodeKind::Image, Support::Native("NSImageView")),
    (NodeKind::Icon, Support::Native("NSImageView + SF Symbols")),
    (NodeKind::ScrollView, Support::Native("NSScrollView")),
    (NodeKind::TextInput, Support::Native("NSTextField")),
    (NodeKind::TextEditor, Support::Native("NSTextView")),
    (NodeKind::StackView, Support::Native("NSView")),
    (NodeKind::Button, Support::Native("NSButton")),
    (NodeKind::Switch, Support::Native("NSSwitch")),
    (NodeKind::Slider, Support::Native("NSSlider")),
    (NodeKind::ActivityIndicator, Support::Native("NSProgressIndicator (spinning)")),
    (NodeKind::ProgressBar, Support::Native("NSProgressIndicator (bar)")),
    (NodeKind::SegmentedControl, Support::Native("NSSegmentedControl")),
    (NodeKind::Stepper, Support::Native("NSStepper")),
    (NodeKind::SearchBar, Support::Native("NSSearchField")),
    (NodeKind::Picker, Support::Native("NSPopUpButton")),
    (NodeKind::DatePicker, Support::Native("NSDatePicker")),
    (NodeKind::Alert, Support::Native("NSAlert")),
    (NodeKind::WebView, Support::Native("WKWebView")),
    // A layer above the content, not a separate window. On macOS what is
    // really modal is a sheet (`beginSheet:`) or an `NSPanel`, and both take
    // the content out of the window where the layout placed it. Since the core
    // already lays the modal out full screen, a view on top gives the same
    // result without fighting two coordinate systems. The system dialog
    // —`<an-alert>`— is a real `NSAlert`.
    (NodeKind::Modal, Support::Assembled("NSView above the root")),
    // macOS has no tab bar. The nearest thing the system has is an
    // `NSSegmentedControl`, which is exactly what macOS apps use to change
    // section, so it is assembled out of that and said so. `NSTabView` will
    // not do: that is the document tab, with its own frame and background.
    (NodeKind::TabBar, Support::Assembled("NSSegmentedControl")),
    // A Mac's navigation header is the window's title bar. No second one is
    // drawn inside the content —there would be two of them— but neither is
    // what the template wrote thrown away: the `[title]` ends up as the
    // window's title, which is where a Mac user looks for it.
    // `AppKitHost::apply_window_title` puts it there. The node measures zero,
    // so it leaves no gap where there is nothing. See `controls.rs`.
    (
        NodeKind::NavigationBar,
        Support::Elsewhere("the window's title bar: the [title] ends up there"),
    ),
    // Native MapKit asks for no key —that is MapKit JS, a different product—
    // and on macOS `MKMapView` inherits from `NSView`, so it goes into the
    // tree as one more view. Showing where you are does ask for permission,
    // and that is `showsUser`.
    (NodeKind::MapView, Support::Native("MKMapView")),
    // AppKit does have a video view, which UIKit does not: `AVPlayerView`
    // **is** an `NSView` and brings the system's controls with it. It works
    // out cheaper than on iOS, where an `AVPlayerViewController` has to be
    // contained.
    (NodeKind::VideoView, Support::Native("AVPlayerView")),
];

/// What macOS puts behind a primitive.
pub fn support(kind: NodeKind) -> Option<Support> {
    SUPPORT.iter().find(|(k, _)| *k == kind).map(|(_, s)| *s)
}

/// The event names the framework knows how to send.
///
/// It is needed because two kinds of name reach the host. One is
/// `nativeEvent()`'s in `packages/primitives` —`press`, `change`, `scroll`—
/// and that one really is a request. The other is the name of the directive's
/// *output* —`onChange`, `valueChange`—: Angular also registers an element
/// listener for every `(output)` that appears in a template, and that name
/// corresponds to no platform event in any of the hosts.
///
/// Warning about the second kind would mean warning on every startup about
/// something that works, and a warning that always comes out is a warning
/// nobody reads. Only the first kind is warned about: what somebody genuinely
/// asked for and this platform does not give.
pub const KNOWN_EVENTS: &[&str] = &[
    "press", "doublePress", "longPress", "pan", "pinch", "rotate", "swipeLeft", "swipeRight",
    "swipeUp", "swipeDown", "hover", "layout", "safeArea", "back", "refresh", "scroll", "load",
    "change", "input", "focus", "blur", "submit", "select", "dismiss",
];

pub fn is_known_event(event: &str) -> bool {
    KNOWN_EVENTS.contains(&event)
}

/// Events this platform cannot deliver, with the reason.
///
/// It is consulted on subscription and not on firing: a template that asks for
/// `(swipeLeft)` on a Mac has to find out at mount time, not sit waiting for
/// an event that is never going to arrive.
pub fn unsupported_event(kind: NodeKind, event: &str) -> Option<&'static str> {
    match (kind, event) {
        (kind, "swipeLeft" | "swipeRight" | "swipeUp" | "swipeDown") if !catches_swipe(kind) => {
            Some(
                "AppKit's swipe is not a recogniser hung off a view: it is an event that goes up \
                 the responder chain, and only a view of this host's can catch it. A system \
                 control cannot be subclassed with the app already running; put it on the \
                 <an-view> wrapping it, which does receive it",
            )
        }
        (NodeKind::NavigationBar, "back") => Some(
            "this host's header is the window's title bar, and a title bar has no back button: \
             on a Mac you go back through the menu or through a button of the app's",
        ),
        (NodeKind::ScrollView, "refresh") => Some(
            "there is no pull-to-refresh on the desktop: you reload with a button or with a \
             shortcut, and that is the app's business",
        ),
        (NodeKind::Modal, "dismiss") => Some(
            "this host's modal is a layer shown and hidden with `visible`, not a presentation of \
             the system's: it never closes by itself, so there is nothing to announce",
        ),
        (NodeKind::StackView, "back") => Some(
            "the swipe-from-the-edge back gesture is iOS's; on the desktop you go back through \
             the menu or through a button",
        ),
        // The safe area is the screen's cut-outs —the notch, the home
        // indicator— and a desktop window has none of that. Zero on all four
        // sides is the correct answer, not a failure: it is answered and not
        // warned about.
        _ => None,
    }
}

/// One subscribed swipe direction.
///
/// They are kept in a bit mask because each direction is its own output:
/// listening only for `swipeLeft` has no business delivering the other three.
pub const SWIPE_LEFT: u8 = 1 << 0;
pub const SWIPE_RIGHT: u8 = 1 << 1;
pub const SWIPE_UP: u8 = 1 << 2;
pub const SWIPE_DOWN: u8 = 1 << 3;

/// The bit each event name gets, if it is one of the four.
pub fn swipe_bit(event: &str) -> Option<u8> {
    Some(match event {
        "swipeLeft" => SWIPE_LEFT,
        "swipeRight" => SWIPE_RIGHT,
        "swipeUp" => SWIPE_UP,
        "swipeDown" => SWIPE_DOWN,
        _ => return None,
    })
}

/// Which way a swipe went, from the `NSEvent`'s deltas.
///
/// The correspondence between sign and direction is not a guess and cannot be
/// checked without a trackpad and a hand on it, so it sits where it can be
/// read and tested without either. Apple writes it in `NSEvent.h`, in the
/// comment on `deltaX`:
///
/// > A non-0 deltaX will represent a horizontal swipe, -1 for swipe right and
/// > 1 for swipe left. A non-0 deltaY will represent a vertical swipe, -1 for
/// > swipe down and 1 for swipe up.
///
/// The horizontal wins over the vertical when both arrive, which in practice
/// does not happen: the system sends one axis per gesture.
pub fn swipe_direction(delta_x: f64, delta_y: f64) -> Option<(u8, &'static str)> {
    if delta_x != 0.0 {
        return Some(if delta_x < 0.0 {
            (SWIPE_RIGHT, "swipeRight")
        } else {
            (SWIPE_LEFT, "swipeLeft")
        });
    }
    if delta_y != 0.0 {
        return Some(if delta_y < 0.0 {
            (SWIPE_DOWN, "swipeDown")
        } else {
            (SWIPE_UP, "swipeUp")
        });
    }
    None
}

/// Primitives whose view in this host is created by the host itself, and not
/// by AppKit.
///
/// It matters for one thing only, which is why it is here and not tucked away
/// in `host.rs`: **the swipe**. AppKit has no swipe recogniser, but it does
/// have the gesture: it arrives as `swipeWithEvent:` down the responder chain,
/// and an Objective-C class can only handle it if the method is on it. The
/// views in this list are `AnFlippedView` —see `flipped.rs`— and they have it;
/// an `NSButton` belongs to the system and a method cannot be added to it with
/// the app already running.
///
/// This is not a hole: a `swipeWithEvent:` a control does not handle goes up
/// to the next responder in the chain, which is its parent view. Which means a
/// swipe over a button ends up reaching the `<an-view>` wrapping it, which is
/// where a template puts it nearly always.
pub fn catches_swipe(kind: NodeKind) -> bool {
    matches!(
        kind,
        // The `<an-scroll-view>` catches it through its document view, which
        // is ours as well; the `NSScrollView` around it is the system's.
        NodeKind::View | NodeKind::StackView | NodeKind::ScrollView | NodeKind::Modal
    )
}
