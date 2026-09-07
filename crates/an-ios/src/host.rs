//! `HostRenderer` over UIKit. One native view per mountable node, placed with
//! a direct `frame`: taffy has already worked the layout out, and Auto Layout
//! here would only add a second layout engine competing with the first.

use std::collections::HashMap;

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{EventQueue, HostRenderer};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use core::ptr::NonNull;
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSNumber, NSString, NSValue};
use block2::RcBlock;
use objc2_quartz_core::CAShapeLayer;
use objc2_ui_kit::{
    UIKeyboardAnimationCurveUserInfoKey, UIKeyboardAnimationDurationUserInfoKey,
    UIKeyboardFrameEndUserInfoKey, UIKeyboardWillChangeFrameNotification,
    UIKeyboardWillHideNotification,
    UIViewAnimationOptions,
    NSLineBreakMode, NSTextAlignment, UIAccessibilityIdentification, UIActivityIndicatorView,
    UIBezierPath, UIButton, UIControlState, UIFont, UIImageView, UILabel, UIProgressView,
    UIScrollView, UISlider, UISwitch, UITextField, UITextInputTraits, UIView,
};

/// A JSON list of strings, without pulling in a whole parser for it.
///
/// It only has to understand what the JS side generates: `["one","two"]`, with
/// escaped quotes if it comes to that.
fn parse_string_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut item = String::new();
        while let Some(inner) = chars.next() {
            match inner {
                '"' => break,
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        item.push(escaped);
                    }
                }
                other => item.push(other),
            }
        }
        out.push(item);
    }
    out
}

/// A field's or an editor's keyboard quirks.
///
/// The message is sent by first resolving its implementation through the
/// runtime, and not with the setter objc2 generates. This is not fussiness:
///
/// objc2 checks that the method exists before sending it —a check that has
/// caught badly declared signatures in this project more than once— by looking
/// at the class's method table. On iOS 26, `-[UITextField setKeyboardType:]`
/// **is not in that table**: UIKit resolves it the first time somebody asks
/// for it. `respondsToSelector:` says yes and `class_getInstanceMethod` says
/// no, and objc2 believes the second and aborts the process. The upshot: any
/// app with a text field quit the moment it started.
///
/// `class_getMethodImplementation` is the runtime function that **provokes**
/// that resolution, so it returns the real implementation. The object is asked
/// whether it responds first: if it ever stopped responding, this says so
/// instead of sending a message blind.
fn apply_text_traits(traits: &objc2_foundation::NSObject, key: &str, text: Option<&str>, value: &PropValue) {
    use objc2::runtime::{NSObjectProtocol, Sel};

    let (selector, setting) = match key {
        "keyboardType" => (
            objc2::sel!(setKeyboardType:),
            match text {
                Some("numeric") => objc2_ui_kit::UIKeyboardType::NumberPad,
                Some("decimal") => objc2_ui_kit::UIKeyboardType::DecimalPad,
                Some("email") => objc2_ui_kit::UIKeyboardType::EmailAddress,
                Some("phone") => objc2_ui_kit::UIKeyboardType::PhonePad,
                Some("url") => objc2_ui_kit::UIKeyboardType::URL,
                _ => objc2_ui_kit::UIKeyboardType::Default,
            }
            .0,
        ),
        "returnKeyType" => (
            objc2::sel!(setReturnKeyType:),
            match text {
                Some("done") => objc2_ui_kit::UIReturnKeyType::Done,
                Some("go") => objc2_ui_kit::UIReturnKeyType::Go,
                Some("next") => objc2_ui_kit::UIReturnKeyType::Next,
                Some("search") => objc2_ui_kit::UIReturnKeyType::Search,
                Some("send") => objc2_ui_kit::UIReturnKeyType::Send,
                _ => objc2_ui_kit::UIReturnKeyType::Default,
            }
            .0,
        ),
        "autoCapitalize" => (
            objc2::sel!(setAutocapitalizationType:),
            match text {
                Some("none") => objc2_ui_kit::UITextAutocapitalizationType::None,
                Some("words") => objc2_ui_kit::UITextAutocapitalizationType::Words,
                Some("characters") => objc2_ui_kit::UITextAutocapitalizationType::AllCharacters,
                _ => objc2_ui_kit::UITextAutocapitalizationType::Sentences,
            }
            .0,
        ),
        _ => (
            objc2::sel!(setAutocorrectionType:),
            if matches!(value, PropValue::Bool(false)) {
                objc2_ui_kit::UITextAutocorrectionType::No
            } else {
                objc2_ui_kit::UITextAutocorrectionType::Yes
            }
            .0,
        ),
    };

    if !traits.respondsToSelector(selector) {
        eprintln!(
            "angular-native: {} does not handle {selector:?}; the `{key}` prop was not applied",
            traits.class().name().to_string_lossy()
        );
        return;
    }

    // `objc2` declares this symbol untyped, so each caller gives it its own:
    // it is the runtime function that resolves the method —provoking the lazy
    // resolution— and returns its implementation.
    unsafe extern "C" {
        fn class_getMethodImplementation(
            class: *const objc2::runtime::AnyClass,
            selector: Sel,
        ) -> Option<unsafe extern "C" fn()>;
    }

    // All four properties take a single `NSInteger`, so the signature is the
    // same for every one of them.
    type Setter = unsafe extern "C" fn(&objc2_foundation::NSObject, Sel, isize);
    let Some(implementation) = (unsafe { class_getMethodImplementation(traits.class(), selector) })
    else {
        return;
    };
    let setter: Setter = unsafe { std::mem::transmute(implementation) };
    unsafe { setter(traits, selector, setting) };
}

/// How much of the container the keyboard is covering, and the way it gets
/// there.
///
/// **`safeAreaInsets` never counts the keyboard.** Those insets are the
/// system's own furniture — the notch, the status bar, the home indicator —
/// and the keyboard is not furniture: it comes and goes with the focus. UIKit
/// says so through the notification centre and nowhere else, so a host that
/// only reads `safeAreaInsets` leaves a field at the bottom of a form sitting
/// underneath it, which is the layout problem this platform has most of.
///
/// The notification carries three things: the frame the keyboard is **going**
/// to have, and the duration and the curve of the animation it has already
/// started. The last two are why this is a view and not a number. Jumping to
/// the final inset the moment the notification lands moves the form a fifth of
/// a second before the keyboard arrives, and the eye reads that as a glitch
/// rather than as a layout.
///
/// So the number is handed to UIKit and read back: an invisible one-point view
/// is animated with that very duration, that very curve and
/// `beginFromCurrentState`, and every frame Core Animation's in-flight copy of
/// its layer — the *presentation* layer — is asked where it has got to. The
/// curve the keyboard uses is 7, which is not one of the four public ones and
/// has no published control points; any easing written here would be a guess,
/// and this way the number being followed is the one UIKit is computing
/// itself.
struct Keyboard {
    /// Animated, never seen: one point, fully transparent and deaf to
    /// touches. Its `y` **is** the inset, in points.
    probe: Retained<UIView>,
    /// The notification-centre tokens. Nothing reads them; they are held
    /// because dropping one unregisters its observer, and an observer that
    /// outlives the host would run a block over a container that is gone.
    _observers: Vec<Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>>,
}

impl Keyboard {
    /// Subscribes to the two notifications that between them cover every way
    /// the keyboard can move.
    ///
    /// `willChangeFrame` is the one that carries the work: it fires when the
    /// keyboard comes up, when it goes down, when it changes height because
    /// the language changed or the predictive bar appeared, when the device
    /// rotates and when the window is resized in split screen. `willHide` is
    /// added because it is the only one guaranteed on some dismissals, and a
    /// second report of a frame that has not moved costs nothing: the report
    /// is compared before it is sent.
    fn install(mtm: MainThreadMarker, container: &UIView) -> Self {
        let probe = UIView::initWithFrame(
            mtm.alloc::<UIView>(),
            CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize { width: 1.0, height: 1.0 },
            },
        );
        probe.setAlpha(0.0);
        probe.setUserInteractionEnabled(false);
        // In the hierarchy on purpose: a layer outside a window is not
        // guaranteed to be given a presentation layer, and without one there
        // is nothing to read mid-animation.
        container.addSubview(&probe);

        let center = NSNotificationCenter::defaultCenter();
        let mut observers = Vec::new();
        for name in [
            unsafe { UIKeyboardWillChangeFrameNotification },
            unsafe { UIKeyboardWillHideNotification },
        ] {
            let container = container.retain();
            let probe = probe.clone();
            let block = RcBlock::new(move |note: NonNull<NSNotification>| {
                // The queue is nil, so the block runs on whatever thread
                // posted — and UIKit posts these on the main one.
                Keyboard::follow(&container, &probe, unsafe { note.as_ref() });
            });
            observers.push(unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
            });
        }
        Keyboard { probe, _observers: observers }
    }

    /// Starts the probe on the same journey the keyboard has just started.
    fn follow(container: &UIView, probe: &UIView, note: &NSNotification) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let Some(info) = note.userInfo() else { return };

        let end = unsafe { info.objectForKey(UIKeyboardFrameEndUserInfoKey) }
            .and_then(|value| value.downcast::<NSValue>().ok())
            .and_then(|value| value.get_rect());
        let Some(end) = end else { return };

        // **The frame is intersected with the view, never used as a height.**
        // A hardware keyboard leaves the software one off screen with only the
        // shortcuts bar showing, an iPad's undocked keyboard floats in the
        // middle, and a view that does not reach the bottom of the window is
        // covered by less than the keyboard is tall. All three come out right
        // from the overlap and wrong from `end.size.height`.
        let local = container.convertRect_fromView(end, None);
        let bounds = container.bounds();
        let covered = (bounds.origin.y + bounds.size.height - local.origin.y)
            .clamp(0.0, bounds.size.height);

        let duration = unsafe { info.objectForKey(UIKeyboardAnimationDurationUserInfoKey) }
            .and_then(|value| value.downcast::<NSNumber>().ok())
            .map(|value| value.doubleValue())
            .unwrap_or(0.0);
        let curve = unsafe { info.objectForKey(UIKeyboardAnimationCurveUserInfoKey) }
            .and_then(|value| value.downcast::<NSNumber>().ok())
            .map(|value| value.integerValue())
            .unwrap_or(0);

        let mut frame = probe.frame();
        if (frame.origin.y - covered).abs() < f64::EPSILON {
            return;
        }
        frame.origin.y = covered;

        // A keyboard dragged away with the finger, and a hardware one being
        // attached, both arrive with a duration of zero: there is nothing to
        // follow, and animating over zero seconds would still take a frame to
        // land. Whatever animation was in flight is stopped first, or the
        // number would carry on towards a target that no longer applies.
        if duration <= 0.0 {
            probe.layer().removeAllAnimations();
            probe.setFrame(frame);
            return;
        }

        // The curve travels as the number that goes in the top half of the
        // options mask; this is the shift UIKit's own header documents, and it
        // is what makes curve 7 —the keyboard's, which has no name— reachable
        // at all. `beginFromCurrentState` is what makes a second notification
        // arriving mid-flight carry on from where the eye can see the layout,
        // instead of snapping back to where the last one started.
        let options = UIViewAnimationOptions((curve as usize) << 16)
            | UIViewAnimationOptions::BeginFromCurrentState;
        let animated = probe.retain();
        let animations = RcBlock::new(move || animated.setFrame(frame));
        UIView::animateWithDuration_delay_options_animations_completion(
            duration,
            0.0,
            options,
            &animations,
            None,
            mtm,
        );
    }

    /// What the keyboard is covering **right now**, in points: while it is
    /// moving this is where it has got to, not where it is going.
    fn covered(&self) -> f32 {
        let layer = self.probe.layer();
        let live = unsafe { layer.presentationLayer() };
        // With no animation running there is no presentation layer, and the
        // model value is the answer.
        let y = live.map(|l| l.frame().origin.y).unwrap_or_else(|| self.probe.frame().origin.y);
        y.max(0.0) as f32
    }
}

/// The view immediately below another inside a container.
fn previous_sibling(parent: &UIView, view: &UIView) -> Option<Retained<UIView>> {
    let subviews = parent.subviews().to_vec();
    let index = subviews.iter().position(|sibling| &**sibling == view)?;
    if index == 0 {
        return None;
    }
    subviews.get(index - 1).cloned()
}

/// The outline of a rectangle with a different radius per corner.
///
/// The corners go in the order top-left, top-right, bottom-right, bottom-left,
/// the same one CSS uses and the same one Android expects.
fn rounded_path(width: f64, height: f64, radii: [f64; 4]) -> Retained<UIBezierPath> {
    use std::f64::consts::{FRAC_PI_2, PI};

    let limit = (width.min(height)) / 2.0;
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0.0, limit));
    let path = UIBezierPath::new();
    let point = |x: f64, y: f64| CGPoint { x, y };

    path.moveToPoint(point(tl, 0.0));
    path.addLineToPoint(point(width - tr, 0.0));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(width - tr, tr),
        tr,
        -FRAC_PI_2,
        0.0,
        true,
    );
    path.addLineToPoint(point(width, height - br));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(width - br, height - br),
        br,
        0.0,
        FRAC_PI_2,
        true,
    );
    path.addLineToPoint(point(bl, height));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(bl, height - bl),
        bl,
        FRAC_PI_2,
        PI,
        true,
    );
    path.addLineToPoint(point(0.0, tl));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(tl, tl),
        tl,
        PI,
        PI + FRAC_PI_2,
        true,
    );
    path.closePath();
    path
}

/// A node's native view. It is kept with its concrete type because a
/// `<Text>`'s props are not applied the way a `<View>`'s are.
enum HostView {
    View(Retained<UIView>),
    Stack(Retained<UIView>),
    Label(Retained<UILabel>),
    Image(Retained<UIImageView>),
    Scroll(Retained<UIScrollView>),
    Field(Retained<UITextField>),
    /// A `UITabBarController`'s view, that controller being what draws the
    /// bar.
    TabsHost(Retained<UIView>),
    Toggle(Retained<UISwitch>),
    Slide(Retained<UISlider>),
    Spinner(Retained<UIActivityIndicatorView>),
    Progress(Retained<UIProgressView>),
    Button(Retained<UIButton>),
    /// A layer above everything. On iOS the proper thing would be to present
    /// a controller, but there is no one-per-screen here: it is a view mounted
    /// over the root that animates in.
    Overlay(Retained<UIView>),
    /// A dialog has no view of its own: the system presents it. An empty view
    /// is mounted so the tree has something to hang the node off.
    Dialog(Retained<UIView>),
    Segments(Retained<objc2_ui_kit::UISegmentedControl>),
    Step(Retained<objc2_ui_kit::UIStepper>),
    Search(Retained<objc2_ui_kit::UISearchBar>),
    /// A drop-down: a button that opens a system menu.
    Menu(Retained<UIButton>),
    Date(Retained<objc2_ui_kit::UIDatePicker>),
    Area(Retained<objc2_ui_kit::UITextView>),
    Nav(Retained<objc2_ui_kit::UINavigationBar>),
    /// tvOS does not have it: WebKit is not part of its SDK, so not even the
    /// linker would find it. See `family.rs`.
    #[cfg(not(target_os = "tvos"))]
    Web(Retained<crate::web::WKWebView>),
    Map(Retained<crate::map::MKMapView>),
    Video(Retained<UIView>),
}

impl HostView {
    fn as_view(&self) -> &UIView {
        match self {
            HostView::View(v) => v,
            HostView::Stack(v) => v,
            HostView::Label(v) => v,
            HostView::Image(v) => v,
            HostView::Scroll(v) => v,
            HostView::Field(v) => v,
            HostView::TabsHost(v) => v,
            HostView::Toggle(v) => v,
            HostView::Slide(v) => v,
            HostView::Spinner(v) => v,
            HostView::Progress(v) => v,
            HostView::Button(v) => v,
            HostView::Overlay(v) => v,
            HostView::Dialog(v) => v,
            HostView::Segments(v) => v,
            HostView::Step(v) => v,
            HostView::Search(v) => v,
            HostView::Menu(v) => v,
            HostView::Date(v) => v,
            HostView::Area(v) => v,
            HostView::Nav(v) => v,
            #[cfg(not(target_os = "tvos"))]
            HostView::Web(v) => v,
            HostView::Map(v) => v,
            HostView::Video(v) => v,
        }
    }

    fn kind(&self) -> NodeKind {
        match self {
            HostView::View(_) => NodeKind::View,
            HostView::Stack(_) => NodeKind::StackView,
            HostView::Label(_) => NodeKind::Text,
            HostView::Image(_) => NodeKind::Image,
            HostView::Scroll(_) => NodeKind::ScrollView,
            HostView::Field(_) => NodeKind::TextInput,
            HostView::TabsHost(_) => NodeKind::TabBar,
            HostView::Toggle(_) => NodeKind::Switch,
            HostView::Slide(_) => NodeKind::Slider,
            HostView::Spinner(_) => NodeKind::ActivityIndicator,
            HostView::Progress(_) => NodeKind::ProgressBar,
            HostView::Button(_) => NodeKind::Button,
            HostView::Overlay(_) => NodeKind::Modal,
            HostView::Dialog(_) => NodeKind::Alert,
            HostView::Segments(_) => NodeKind::SegmentedControl,
            HostView::Step(_) => NodeKind::Stepper,
            HostView::Search(_) => NodeKind::SearchBar,
            HostView::Menu(_) => NodeKind::Picker,
            HostView::Date(_) => NodeKind::DatePicker,
            HostView::Area(_) => NodeKind::TextEditor,
            HostView::Nav(_) => NodeKind::NavigationBar,
            #[cfg(not(target_os = "tvos"))]
            HostView::Web(_) => NodeKind::WebView,
            HostView::Map(_) => NodeKind::MapView,
            HostView::Video(_) => NodeKind::VideoView,
        }
    }

    fn as_label(&self) -> Option<&UILabel> {
        match self {
            HostView::Label(v) => Some(v),
            _ => None,
        }
    }
}

/// A transform's parts, uncomposed.
///
/// The scale starts at 1 and not at 0: a view with no `scale` has to look the
/// way it did before the prop existed, not disappear.
#[derive(Clone, Copy)]
struct Transform {
    translate_x: f64,
    translate_y: f64,
    scale_x: f64,
    scale_y: f64,
    rotate: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self { translate_x: 0.0, translate_y: 0.0, scale_x: 1.0, scale_y: 1.0, rotate: 0.0 }
    }
}

impl Transform {
    /// Whether this view is being drawn where layout put it.
    ///
    /// It matters because UIKit says so: with a transform applied, `frame` is
    /// documented as undefined and must not be assigned. See `set_layout`.
    fn is_identity(&self) -> bool {
        self.translate_x == 0.0
            && self.translate_y == 0.0
            && self.scale_x == 1.0
            && self.scale_y == 1.0
            && self.rotate == 0.0
    }

    /// The order is scale, rotate, then translate.
    ///
    /// The other way round does not come out the same: if the translation goes
    /// in before the rotation, rotating rotates the translation too, and
    /// dragging something tilted goes off diagonally instead of following the
    /// finger.
    fn matrix(&self) -> CGAffineTransform {
        let scale = CGAffineTransform {
            a: self.scale_x,
            b: 0.0,
            c: 0.0,
            d: self.scale_y,
            tx: 0.0,
            ty: 0.0,
        };
        let (sin, cos) = self.rotate.sin_cos();
        let rotate = CGAffineTransform { a: cos, b: sin, c: -sin, d: cos, tx: 0.0, ty: 0.0 };
        let translate = CGAffineTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: self.translate_x,
            ty: self.translate_y,
        };
        concat(concat(scale, rotate), translate)
    }
}

/// `a` first, then `b`. `CGAffineTransformConcat` is not in the bindings, and
/// multiplying two 3x2 affine matrices is six products: doing it here works out
/// cheaper than linking against Core Graphics for this.
fn concat(a: CGAffineTransform, b: CGAffineTransform) -> CGAffineTransform {
    CGAffineTransform {
        a: a.a * b.a + a.b * b.c,
        b: a.a * b.b + a.b * b.d,
        c: a.c * b.a + a.d * b.c,
        d: a.c * b.b + a.d * b.d,
        tx: a.tx * b.a + a.ty * b.c + b.tx,
        ty: a.tx * b.b + a.ty * b.d + b.ty,
    }
}

/// How a view animates its changes.
#[derive(Clone, Copy)]
struct Animation {
    /// Seconds. Zero switches the animation off without wiping the rest of the
    /// settings.
    duration: f64,
    delay: f64,
    curve: UIViewAnimationOptions,
}

impl Default for Animation {
    fn default() -> Self {
        // Off fast and easing to a stop. It is how things move, and it is
        // the default curve for nearly everything on iOS.
        Animation { duration: 0.0, delay: 0.0, curve: UIViewAnimationOptions::CurveEaseOut }
    }
}

pub struct UikitHost {
    mtm: MainThreadMarker,
    /// The view the Xcode shell hands over. The root of the tree hangs off
    /// it.
    container: Retained<UIView>,
    views: HashMap<NodeId, HostView>,
    /// The font pending per node: `fontSize` and `fontWeight` arrive as
    /// separate props and the `UIFont` has to be rebuilt from both.
    fonts: HashMap<NodeId, an_layout::FontSpec>,
    /// Radii per corner: top-left, top-right, bottom-right, bottom-left.
    /// UIKit only knows about a single radius, so when they differ the shape
    /// has to be drawn by hand and used as a mask.
    corners: HashMap<NodeId, [f64; 4]>,
    /// The value asked of each slider. It is kept because `value`,
    /// `minimumValue` and `maximumValue` arrive as separate props and in any
    /// order: setting the value before the maximum clamps it against the old
    /// range.
    slider_values: HashMap<NodeId, f32>,
    /// Each view's transform. As with the slider, the parts arrive as
    /// separate props: they have to be kept so the whole matrix can be rebuilt
    /// every time one of them changes.
    transforms: HashMap<NodeId, Transform>,
    /// The nodes that animate their changes, and how. While there is an entry
    /// here, moving, scaling, changing the opacity of or repositioning that
    /// view does not jump to the new value: it travels to it.
    animations: HashMap<NodeId, Animation>,
    /// Each node's icon name, size and weight. As with the font, the three
    /// parts arrive separately and the whole symbol has to be rebuilt every
    /// time one of them changes.
    icons: HashMap<NodeId, (String, f32, u16)>,
    /// Each tab bar's titles and icons, which arrive separately.
    tabs: HashMap<NodeId, (Vec<String>, Vec<String>)>,
    /// Each bar's tab controller.
    tab_controllers: HashMap<NodeId, Retained<objc2_ui_kit::UITabBarController>>,
    /// Each bar's delegate. A delegate is not retained, so if it is not kept
    /// here it dies and the tabs stop reporting.
    tab_delegates: HashMap<NodeId, Retained<crate::events::TabDelegate>>,
    /// Each label's underline or strikethrough. It goes separately from the
    /// `FontSpec` because the core does not need it: it does not change what
    /// the text measures.
    decorations: HashMap<NodeId, String>,
    /// Each field's placeholder text and colour. They go together because
    /// `UITextField` has no placeholder colour: it has to be given one as an
    /// attributed string, and that needs the text as well.
    placeholders: HashMap<NodeId, String>,
    placeholder_colors: HashMap<NodeId, String>,
    /// Each drop-down's options, so the chosen one's title can be set.
    menus: HashMap<NodeId, Vec<String>>,
    /// Each button's title and colour. Changing the variant rebuilds UIKit's
    /// configuration, which takes both of them down with it.
    button_titles: HashMap<NodeId, String>,
    button_colors: HashMap<NodeId, String>,
    button_variants: HashMap<NodeId, String>,
    /// The button's second line, which only exists on iOS.
    button_subtitles: HashMap<NodeId, String>,
    /// Each button's icon and which side it goes on, which also arrive
    /// separately.
    button_icons: HashMap<NodeId, (String, String)>,
    /// Each header's title, back label and whether it is shown.
    navs: HashMap<NodeId, (String, String, bool)>,
    /// Each header's live back button, so the event can be attached to it: it
    /// is rebuilt every time the title changes.
    nav_backs: HashMap<NodeId, Retained<objc2_ui_kit::UIBarButtonItem>>,
    nav_targets: HashMap<NodeId, Retained<crate::events::ControlTarget>>,
    /// Each map's centre and zoom, which arrive as separate props.
    maps: HashMap<NodeId, (f64, f64, f64)>,
    /// Each video's player and layer. The layer has to be resized by hand: a
    /// layer does not stretch with its view.
    videos: HashMap<
        NodeId,
        (Retained<crate::video::AVPlayer>, Retained<crate::video::AVPlayerViewController>),
    >,
    /// The videos that ought to be playing.
    video_playing: std::collections::HashSet<NodeId>,
    /// The value asked of each `Stepper`, for the same reason as the slider's:
    /// the range and the value arrive separately and in any order.
    stepper_values: HashMap<NodeId, f64>,
    /// Which way each stack's next transition goes: `push`, `pop` or nothing.
    /// Angular decides it, being the one that knows whether this is a step
    /// forward or a step back.
    transitions: HashMap<NodeId, String>,
    /// Screens that have just entered a stack and have not been animated yet.
    /// The animation cannot be launched on insertion because the frame is not
    /// worked out yet: it happens in `flush`, once the layout has been
    /// through.
    entering: Vec<(NodeId, NodeId)>,
    /// Screens on their way out. They stay in the hierarchy until the
    /// animation finishes, so they have to be retained even once the tree has
    /// forgotten them.
    leaving: Vec<(NodeId, Retained<UIView>)>,
    /// Nodes whose view is animating out: `destroy` must not touch them.
    animating_out: std::collections::HashSet<NodeId>,
    /// Plugin view names that were asked for and are not registered. Said once
    /// each, not once per node.
    warned_views: std::collections::HashSet<String>,
    /// What layout asked for, per node: whether the node keeps its children
    /// inside its own frame. A corner radius clips too, so the value actually
    /// given to UIKit is the two of them together — kept here so that setting
    /// one never silently undoes the other.
    clips: HashMap<NodeId, bool>,
    /// The nodes subscribed to the safe area, with the last insets they were
    /// told about. They are only notified when those really change.
    safe_area: HashMap<NodeId, [f32; 4]>,
    /// The keyboard, which is part of the safe area and is the one part of it
    /// UIKit does not put in `safeAreaInsets`.
    keyboard: Keyboard,
    /// The dialogs that have been declared. They are presented as the frame
    /// closes, once all of their props have arrived: presenting the moment
    /// `visible` changes would show a dialog with no title.
    alerts: HashMap<NodeId, crate::alert::AlertState>,
    /// The dialogs whose state changed in this frame.
    dirty_alerts: Vec<NodeId>,
    /// Each `<Modal>`'s presentation state.
    modals: HashMap<NodeId, crate::modal::ModalState>,
    dirty_modals: Vec<NodeId>,
    /// Live subscriptions, indexed by node and event. They are kept because it
    /// has to be possible to take them away: an `@if` that unmounts its branch
    /// destroys the view, but a `(press)` that stops being bound does not.
    listeners: HashMap<(NodeId, String), crate::events::AttachedListener>,
    /// Accessibility role and state per node, plus the traits the view
    /// carried from the system. They go together because
    /// `accessibilityTraits` is a mask: writing one bit means knowing the
    /// others. See `accessibility.rs`.
    accessibility: crate::accessibility::Accessibility,
    events: EventQueue,
}

impl UikitHost {
    /// # Safety
    /// `container` has to be a live `UIView` and this has to be called from
    /// the main thread.
    pub fn new(mtm: MainThreadMarker, container: Retained<UIView>, events: EventQueue) -> Self {
        let keyboard = Keyboard::install(mtm, &container);
        UikitHost {
            mtm,
            container,
            views: HashMap::new(),
            fonts: HashMap::new(),
            corners: HashMap::new(),
            slider_values: HashMap::new(),
            transforms: HashMap::new(),
            animations: HashMap::new(),
            icons: HashMap::new(),
            tabs: HashMap::new(),
            tab_controllers: HashMap::new(),
            tab_delegates: HashMap::new(),
            menus: HashMap::new(),
            decorations: HashMap::new(),
            placeholders: HashMap::new(),
            placeholder_colors: HashMap::new(),
            button_titles: HashMap::new(),
            button_colors: HashMap::new(),
            button_variants: HashMap::new(),
            button_subtitles: HashMap::new(),
            button_icons: HashMap::new(),
            navs: HashMap::new(),
            nav_backs: HashMap::new(),
            nav_targets: HashMap::new(),
            maps: HashMap::new(),
            videos: HashMap::new(),
            video_playing: std::collections::HashSet::new(),
            stepper_values: HashMap::new(),
            transitions: HashMap::new(),
            entering: Vec::new(),
            leaving: Vec::new(),
            animating_out: std::collections::HashSet::new(),
            warned_views: std::collections::HashSet::new(),
            clips: HashMap::new(),
            safe_area: HashMap::new(),
            keyboard,
            alerts: HashMap::new(),
            dirty_alerts: Vec::new(),
            modals: HashMap::new(),
            dirty_modals: Vec::new(),
            listeners: HashMap::new(),
            accessibility: crate::accessibility::Accessibility::new(),
            events,
        }
    }

    pub fn container(&self) -> &UIView {
        &self.container
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    /// The `UIFont` a `FontSpec` asks for. Without touching any view: the
    /// label, the field, the editor and the button all need the same
    /// calculation.
    fn build_font(&self, spec: &an_layout::FontSpec) -> Retained<UIFont> {
        let size = spec.size as f64;
        if let Some(family) = &spec.family {
            let name = NSString::from_str(family);
            return UIFont::fontWithName_size(&name, size)
                .unwrap_or_else(|| UIFont::systemFontOfSize(size));
        }
        if spec.italic {
            return UIFont::italicSystemFontOfSize(size);
        }
        let weight = match spec.weight {
            0..=299 => -0.6,
            300..=399 => -0.4,
            400..=499 => 0.0,
            500..=599 => 0.23,
            600..=699 => 0.3,
            700..=799 => 0.4,
            _ => 0.6,
        };
        UIFont::systemFontOfSize_weight(size, weight)
    }

    /// Puts the placeholder text back with its colour.
    ///
    /// With no colour it goes in plain and unattributed: an attributed string
    /// with no attributes draws differently from the one UIKit puts there on
    /// its own.
    fn apply_placeholder(&self, id: NodeId) {
        let Some(HostView::Field(field)) = self.views.get(&id) else { return };
        let Some(placeholder) = self.placeholders.get(&id) else { return };
        let string = NSString::from_str(placeholder);
        let Some(color) =
            self.placeholder_colors.get(&id).and_then(|raw| crate::color::to_uicolor(raw))
        else {
            field.setPlaceholder(Some(&string));
            return;
        };
        let attributed = unsafe {
            objc2_foundation::NSMutableAttributedString::initWithString(
                self.mtm.alloc::<objc2_foundation::NSMutableAttributedString>(),
                &string,
            )
        };
        unsafe {
            attributed.addAttribute_value_range(
                objc2_ui_kit::NSForegroundColorAttributeName,
                &color,
                objc2_foundation::NSRange { location: 0, length: string.len_utf16() },
            );
            field.setAttributedPlaceholder(Some(&attributed));
        }
    }

    fn apply_font(&mut self, id: NodeId) {
        let Some(spec) = self.fonts.get(&id).cloned() else { return };
        let font = self.build_font(&spec);
        let Some(view) = self.views.get(&id) else { return };
        // These UIKit setters are marked unsafe for not being thread-safe;
        // the host's `MainThreadMarker` guarantees we are on the right
        // thread.
        match view {
            HostView::Label(label) => unsafe {
                label.setFont(Some(&font));
                label.setNumberOfLines(spec.max_lines.unwrap_or(0) as isize);
            },
            // The field, the editor and the button have type too, and until
            // now they were left with UIKit's: `[fontSize]` on a `<TextInput>`
            // was a declared prop that did nothing.
            HostView::Field(field) => unsafe { field.setFont(Some(&font)) },
            HostView::Area(area) => unsafe { area.setFont(Some(&font)) },
            HostView::Button(_) => self.refresh_button(id),
            _ => return,
        }
        self.apply_text_attributes(id);
    }

    /// Line height and letter spacing, which `UILabel` does not have as
    /// properties.
    ///
    /// The core was already measuring with both —they are in the `FontSpec` it
    /// works each line's height out with— and the host was drawing without
    /// them, so the layout reserved room the text did not fill. The only way
    /// to apply them in UIKit is with attributed text: `kern` for the spacing
    /// and an `NSParagraphStyle` for the line height.
    ///
    /// Only those two attributes are set. The font and the colour are left out
    /// on purpose: with neither of them in the attributes, `UILabel` uses its
    /// own, and so `[color]` and `[fontSize]` go on working as before.
    fn apply_text_attributes(&self, id: NodeId) {
        let Some(label) = self.views.get(&id).and_then(HostView::as_label) else { return };
        let spec = self.fonts.get(&id);
        let kern = spec.map(|s| s.letter_spacing).unwrap_or(0.0);
        let line_height = spec.and_then(|s| s.line_height);
        let decoration = self.decorations.get(&id).map(String::as_str).unwrap_or("none");
        let text = unsafe { label.text() }.map(|t| t.to_string()).unwrap_or_default();
        if text.is_empty() {
            return;
        }
        let string = NSString::from_str(&text);
        if kern == 0.0 && line_height.is_none() && decoration == "none" {
            // With nothing to add it goes back to plain text: otherwise,
            // taking the spacing away would leave the previous one in place.
            unsafe { label.setAttributedText(None) };
            label.setText(Some(&string));
            return;
        }
        let attributed = unsafe {
            objc2_foundation::NSMutableAttributedString::initWithString(
                self.mtm.alloc::<objc2_foundation::NSMutableAttributedString>(),
                &string,
            )
        };
        // In UTF-16, which is how `NSString` counts. With the length in
        // bytes, any text carrying an accent runs off the end of the range and
        // `NSAttributedString` raises an exception: the app quits on mounting
        // the first accented letter, and the headless dump does not see it
        // because there is no UIKit there.
        let range = objc2_foundation::NSRange { location: 0, length: string.len_utf16() };
        if kern != 0.0 {
            let number = objc2_foundation::NSNumber::new_f64(kern as f64);
            unsafe {
                attributed.addAttribute_value_range(
                    objc2_ui_kit::NSKernAttributeName,
                    &number,
                    range,
                )
            };
        }
        if let Some(height) = line_height {
            let style = objc2_ui_kit::NSMutableParagraphStyle::new();
            // Minimum and maximum the same: the line height is the one the
            // template asks for, neither the font's own nor a larger one.
            style.setMinimumLineHeight(height as f64);
            style.setMaximumLineHeight(height as f64);
            // The paragraph style takes the line-break mode with it too, so
            // the label's has to be given back or `numberOfLines` would stop
            // putting an ellipsis in.
            style.setLineBreakMode(label.lineBreakMode());
            unsafe {
                attributed.addAttribute_value_range(
                    objc2_ui_kit::NSParagraphStyleAttributeName,
                    &style,
                    range,
                )
            };
        }
        if decoration != "none" {
            // The 1 is `NSUnderlineStyle.single`: a plain single rule, which
            // is the only one a template ever asks for.
            let style = objc2_foundation::NSNumber::new_isize(1);
            unsafe {
                attributed.addAttribute_value_range(
                    if decoration == "lineThrough" {
                        objc2_ui_kit::NSStrikethroughStyleAttributeName
                    } else {
                        objc2_ui_kit::NSUnderlineStyleAttributeName
                    },
                    &style,
                    range,
                )
            };
        }
        unsafe { label.setAttributedText(Some(&attributed)) };
    }

    /// Applies a visual change, animated if the node asked for it.
    ///
    /// You cannot simply animate "whatever happens inside the block": UIKit
    /// needs the starting state to be in place before it is entered, and that
    /// is exactly the one the view has right now. Hence it being enough to put
    /// the change inside; what is outside is what was there.
    fn animated(&self, id: NodeId, change: impl Fn() + 'static) {
        let Some(anim) = self.animations.get(&id).copied().filter(|a| a.duration > 0.0) else {
            change();
            return;
        };
        let block = RcBlock::new(move || change());
        unsafe {
            UIView::animateWithDuration_delay_options_animations_completion(
                anim.duration,
                anim.delay,
                anim.curve,
                &block,
                None,
                self.mtm,
            );
        }
    }

    /// Rebuilds the whole button from whatever it is carrying.
    ///
    /// Variant, label, subtitle, icon, colour and type all arrive as separate
    /// props and in any order, and all of them end up in the same
    /// `UIButtonConfiguration`: changing one rebuilds the configuration and
    /// takes the other five down with it. So they are kept and it is assembled
    /// in one piece.
    fn refresh_button(&self, id: NodeId) {
        let Some(HostView::Button(button)) = self.views.get(&id) else { return };
        let variant = self.button_variants.get(&id).map(String::as_str).unwrap_or("text");
        let title = self.button_titles.get(&id).cloned().unwrap_or_default();
        let subtitle = self.button_subtitles.get(&id).cloned().unwrap_or_default();
        let icon = self.button_icons.get(&id).cloned();
        let spec = self.fonts.get(&id).cloned();
        let font = spec.as_ref().map(|spec| self.build_font(spec));
        let raw_color = self.button_colors.get(&id).and_then(|raw| crate::color::parse(raw));
        let color = raw_color.map(|(r, g, b, a)| {
            objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a)
        });
        // Filled, the label takes whatever colour reads over the background;
        // unfilled, the colour asked for. It is worked out here and not left
        // to UIKit because attributed text —the kind that carries the type—
        // does not inherit the configuration's colour: it stayed the tint
        // colour, that is, green on green, that is, invisible.
        let foreground = raw_color.map(|value| {
            let (r, g, b, a) = if variant == "filled" {
                crate::color::contrast_on(value)
            } else {
                value
            };
            objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a)
        });
        // `UIButtonConfiguration` is what gives iOS's current buttons:
        // filled, tinted, outlined or bare, with their backgrounds and their
        // corners. A subtitle, an icon or a font of one's own can only be
        // asked for through it, so the moment any of the three is there a
        // configuration is needed even if the variant is the label-only one.
        let needs_config =
            !subtitle.is_empty() || icon.is_some() || font.is_some() || variant != "text";
        let config = unsafe {
            match variant {
                "filled" => {
                    Some(objc2_ui_kit::UIButtonConfiguration::filledButtonConfiguration(self.mtm))
                }
                "tonal" => {
                    Some(objc2_ui_kit::UIButtonConfiguration::tintedButtonConfiguration(self.mtm))
                }
                // UIKit's outline: a transparent background and a line all
                // round, which is what Material's `outlined` button does.
                "outlined" => {
                    Some(objc2_ui_kit::UIButtonConfiguration::borderedButtonConfiguration(self.mtm))
                }
                _ if needs_config => {
                    Some(objc2_ui_kit::UIButtonConfiguration::plainButtonConfiguration(self.mtm))
                }
                _ => None,
            }
        };
        if let Some(config) = &config {
            unsafe {
                // First of all, wipe what the old path wrote.
                //
                // The props arrive separately and `variant` arrives after
                // `title` and `color`, so every button's first pass always
                // runs as though it were `text`: with no configuration, and
                // therefore through `setTitle:forState:` and
                // `setTitleColor:forState:`. That colour sticks to the button,
                // and when a configuration is later mounted on it UIKit goes
                // on applying it **to the title** over the top of
                // `baseForegroundColor` —not to the subtitle, which does
                // respect it. In the three variants where the label takes the
                // colour asked for this does not show, because the leftover is
                // worth the same; in `filled`, where the label takes the
                // colour that contrasts with the background, the leftover
                // painted the text the same colour as the fill. Hence the
                // whole button with no label on it.
                button.setTitleColor_forState(None, UIControlState::Normal);
                button.setTitle_forState(None, UIControlState::Normal);
                // With a configuration, everything goes through it and
                // nothing through the old calls.
                config.setTitle(Some(&NSString::from_str(&title)));
                if !subtitle.is_empty() {
                    config.setSubtitle(Some(&NSString::from_str(&subtitle)));
                }
                if let Some((name, position)) = &icon {
                    // The button's icon is asked for by name, just as in
                    // `<Icon>`: it is the system's symbol, not a drawing.
                    //
                    // And it is asked for at the label's size. Left unsaid it
                    // comes at its own, which is a loose image's, and a button
                    // with a star twice as tall as its text looks like no iOS
                    // button there is.
                    let points = spec.as_ref().map(|spec| spec.size).unwrap_or(17.0);
                    let weight = spec.as_ref().map(|spec| spec.weight).unwrap_or(400);
                    config.setImage(crate::icons::symbol(name, points, weight).as_deref());
                    config.setImagePlacement(if position == "trailing" {
                        objc2_ui_kit::NSDirectionalRectEdge::Trailing
                    } else {
                        objc2_ui_kit::NSDirectionalRectEdge::Leading
                    });
                    config.setImagePadding(6.0);
                }
                if let Some(color) = &color {
                    // Filled, the colour asked for is the background's and
                    // the label takes whatever reads over it; unfilled, it is
                    // the label's, and with it the icon's.
                    if variant == "filled" {
                        config.setBaseBackgroundColor(Some(color));
                    }
                }
                if let Some(foreground) = &foreground {
                    config.setBaseForegroundColor(Some(foreground));
                }
                if let Some(font) = &font {
                    // The type of a button with a configuration is UIKit's
                    // to resolve: asking the `titleLabel` for it is a
                    // suggestion it treads on the moment it mounts the title
                    // again. The place that really rules is this transformer,
                    // which receives the attributes UIKit was going to use and
                    // returns the ones that get used. Only the font is
                    // changed: the colour and the rest come out of the
                    // configuration and have to be let through.
                    let font = font.clone();
                    let block = RcBlock::new(
                        move |attributes: NonNull<
                            objc2_foundation::NSDictionary<
                                objc2_foundation::NSAttributedStringKey,
                                objc2::runtime::AnyObject,
                            >,
                        >| {
                            let incoming = unsafe { attributes.as_ref() };
                            let out = objc2_foundation::NSMutableDictionary::
                                dictionaryWithDictionary(incoming);
                            out.insert(objc2_ui_kit::NSFontAttributeName, font.as_ref());
                            // The block returns the dictionary at +0, so it
                            // is released into the pool: keeping it leaks it
                            // and releasing it here kills it before UIKit
                            // reads it.
                            let out: Retained<objc2_foundation::NSDictionary<_, _>> =
                                out.into_super();
                            NonNull::new(Retained::autorelease_return(out)).unwrap()
                        },
                    );
                    config.setTitleTextAttributesTransformer(RcBlock::as_ptr(&block));
                }
            }
        }
        unsafe {
            button.setConfiguration(config.as_deref());
            // The font is asked of the label in both cases. With a
            // configuration what rules is the transformer above and this is
            // redundant; without one no transformer applies and this is the
            // only path, so it is left in for both.
            if let (Some(font), Some(label)) = (&font, button.titleLabel()) {
                label.setFont(Some(font));
            }
            if config.is_none() {
                // With no configuration the button rules: label and colour
                // through the old calls.
                button.setTitle_forState(Some(&NSString::from_str(&title)), UIControlState::Normal);
                if let Some(color) = &color {
                    button.setTitleColor_forState(Some(color), UIControlState::Normal);
                    button.setTintColor(Some(color));
                }
            }
        }
    }

    fn font_mut(&mut self, id: NodeId) -> &mut an_layout::FontSpec {
        self.fonts.entry(id).or_default()
    }

    /// Animates the screens that came in or went out in this frame.
    ///
    /// It happens here and not on insertion because until the layout has been
    /// through there is no frame to animate: a freshly created screen measures
    /// zero.
    fn run_stack_animations(&mut self) {
        let entering = std::mem::take(&mut self.entering);
        let leaving = std::mem::take(&mut self.leaving);
        if entering.is_empty() && leaving.is_empty() {
            return;
        }
        let mtm = self.mtm;

        for (stack, child) in entering {
            let Some(stack_view) = self.views.get(&stack).map(|v| v.as_view().retain()) else {
                continue;
            };
            let Some(view) = self.views.get(&child).map(|v| v.as_view().retain()) else { continue };
            let width = stack_view.bounds().size.width;
            let direction = self.transitions.get(&stack).map(String::as_str).unwrap_or("none");
            if direction != "push" || width <= 0.0 {
                continue;
            }

            // It comes in from the right; the one underneath shifts by a
            // third, which is the parallax UINavigationController does.
            let target = view.frame();
            let mut start = target;
            start.origin.x = width;
            view.setFrame(start);

            let below = previous_sibling(&stack_view, &view);
            let below_target = below.as_ref().map(|v| {
                let mut frame = v.frame();
                frame.origin.x = -width / 3.0;
                (v.clone(), frame)
            });

            let animations = RcBlock::new(move || {
                view.setFrame(target);
                if let Some((below, frame)) = &below_target {
                    below.setFrame(*frame);
                }
            });
            UIView::animateWithDuration_animations_completion(0.3, &animations, None, mtm);
        }

        for (stack, view) in leaving {
            let Some(stack_view) = self.views.get(&stack).map(|v| v.as_view().retain()) else {
                view.removeFromSuperview();
                continue;
            };
            let width = stack_view.bounds().size.width;
            if width <= 0.0 {
                view.removeFromSuperview();
                continue;
            }

            let below = previous_sibling(&stack_view, &view);
            let below_target = below.as_ref().map(|v| {
                let mut frame = v.frame();
                frame.origin.x = 0.0;
                (v.clone(), frame)
            });
            let mut target = view.frame();
            target.origin.x = width;

            let animated = view.clone();
            let animations = RcBlock::new(move || {
                animated.setFrame(target);
                if let Some((below, frame)) = &below_target {
                    below.setFrame(*frame);
                }
            });
            // The view is taken away when it is done: until then it has to
            // stay mounted, which is why the block retains it.
            let completion = RcBlock::new(move |_finished: objc2::runtime::Bool| {
                view.removeFromSuperview();
            });
            UIView::animateWithDuration_animations_completion(
                0.3,
                &animations,
                Some(&completion),
                mtm,
            );
        }
        self.animating_out.clear();
    }

    /// Reports the insets if they changed since last time.
    ///
    /// The keyboard goes in the bottom one and not in an event of its own,
    /// because from the template it is the same question the notch asks —"how
    /// far can I paint?"— and a second event would be a second way of knowing
    /// one thing. `max` and not a sum: while the keyboard is up it is drawn
    /// over the home indicator, so the two do not stack.
    fn report_safe_area(&mut self, id: NodeId) {
        let Some(previous) = self.safe_area.get(&id).copied() else { return };
        let insets = self.container.safeAreaInsets();
        let current = [
            insets.top as f32,
            insets.right as f32,
            (insets.bottom as f32).max(self.keyboard.covered()),
            insets.left as f32,
        ];
        if current
            .iter()
            .zip(previous.iter())
            .all(|(a, b)| (a - b).abs() < f32::EPSILON)
        {
            return;
        }
        self.safe_area.insert(id, current);
        an_host::push_event(
            &self.events,
            an_host::HostEvent {
                target: id,
                name: "safeArea".to_owned(),
                payload: vec![
                    ("top".to_owned(), PropValue::Number(current[0] as f64)),
                    ("right".to_owned(), PropValue::Number(current[1] as f64)),
                    ("bottom".to_owned(), PropValue::Number(current[2] as f64)),
                    ("left".to_owned(), PropValue::Number(current[3] as f64)),
                ],
            },
        );
    }

    fn set_corner(&mut self, id: NodeId, corner: usize, radius: Option<f32>) {
        let radii = self.corners.entry(id).or_insert([0.0; 4]);
        radii[corner] = radius.unwrap_or(0.0) as f64;
        self.apply_corners(id);
    }

    /// Applies the radii to the node.
    ///
    /// If all four are the same, `cornerRadius` is enough: it is cheap and
    /// lets UIKit clip on its own. If they differ there is no API: the outline
    /// has to be drawn and used as a mask, and rebuilt every time the view
    /// changes size, because a mask does not stretch by itself.
    fn apply_corners(&mut self, id: NodeId) {
        let Some(radii) = self.corners.get(&id).copied() else { return };
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        let layer = native.layer();

        let uniform = radii.iter().all(|r| (*r - radii[0]).abs() < f64::EPSILON);
        if uniform {
            // As with the rest of UIKit's setters: marked unsafe for not
            // being thread-safe, and here we are always on the UI thread.
            unsafe { layer.setMask(None) };
            layer.setCornerRadius(radii[0]);
            // Either reason to clip is enough, and neither may cancel the
            // other: a rounded view still clips when layout did not ask, and a
            // square one still clips when layout did.
            let wanted = self.clips.get(&id).copied().unwrap_or(false);
            native.setClipsToBounds(radii[0] > 0.0 || wanted);
            return;
        }

        layer.setCornerRadius(0.0);
        native.setClipsToBounds(true);
        let bounds = native.bounds();
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            // It has no size yet; the frame will arrive and we will be back
            // here.
            return;
        }
        let path = rounded_path(bounds.size.width, bounds.size.height, radii);
        let shape = CAShapeLayer::new();
        unsafe { shape.setPath(Some(&path.CGPath())) };
        unsafe { layer.setMask(Some(&shape)) };
    }
}

impl HostRenderer for UikitHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        let mtm = self.mtm;

        // What this UIKit family does not ship, before the `match`.
        //
        // The trouble with an `<an-switch>` on tvOS is not how to configure
        // it: it is that `UISwitch` is not in the system, and asking objc2 for
        // the class aborts the process right there. It is said in the log
        // —once— and a marked empty view is left behind, so that the tree has
        // somewhere to hang the node's children and the rest of the screen
        // does not go out of place. The measurer does not know that control,
        // so the box measures zero: the gap shows, which is what ought to
        // happen.
        if let Some(reason) = crate::family::missing_kind(kind) {
            crate::family::report(&format!("{kind:?}"), reason);
            let gap = UIView::new(mtm);
            let mark = NSString::from_str(&format!("an-unsupported:{kind:?}"));
            gap.setAccessibilityIdentifier(Some(&mark));
            gap.setTranslatesAutoresizingMaskIntoConstraints(true);
            self.views.insert(id, HostView::View(gap));
            return;
        }

        let view = match kind {
            NodeKind::Text => {
                let label = UILabel::new(mtm);
                // The height is the layout's to decide, not UIKit's
                // auto-sizing.
                label.setNumberOfLines(0);
                label.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
                // The default font has to be the one the layout measured
                // with. A freshly made `UILabel` uses 17 points and the core
                // measures with 14: the box came out 20% narrow, the text
                // wrapped, and the parent's clipping ate the second line. It
                // showed up as text that disappears, with no error
                // anywhere.
                let default_size = an_layout::FontSpec::default().size as f64;
                unsafe { label.setFont(Some(&UIFont::systemFontOfSize(default_size))) };
                HostView::Label(label)
            }
            NodeKind::Image => {
                let image = UIImageView::new(mtm);
                // `cover` is `ScaleAspectFill`, and an aspect-fill image view
                // draws **outside** its frame unless it is told not to: the
                // overflowing edge lands on whatever the layout put next to it,
                // which in a card is the caption underneath. Layout's own
                // `overflow` does not reach here — it is resolved in taffy and
                // no host reads it — so the clip is set on the view itself,
                // where it is true of every image regardless of the style.
                image.setClipsToBounds(true);
                HostView::Image(image)
            }
            NodeKind::Icon => {
                let view = UIImageView::new(mtm);
                // `AlwaysTemplate` is what allows the symbol to be tinted
                // with `tintColor`; without it it would always come out in its
                // own colour and `[color]` would do nothing.
                unsafe { view.setContentMode(objc2_ui_kit::UIViewContentMode::ScaleAspectFit) };
                HostView::Image(view)
            }
            NodeKind::ScrollView => HostView::Scroll(UIScrollView::new(mtm)),
            NodeKind::TabBar => {
                // A real `UITabBarController`, not a loose `UITabBar`.
                //
                // Since iOS 26 a loose bar does not behave: its visual
                // provider draws it on its own and on iPad it puts it up top
                // *as well as* in the frame the layout gives it, so two of
                // them come out. It is what happens when a control that
                // expects a controller is used without one.
                //
                // With the controller, UIKit has what it needs and puts the
                // bar where it belongs on each device: at the bottom on
                // iPhone, at the top on iPad. One of them, the system's, on
                // both.
                let controller = objc2_ui_kit::UITabBarController::new(mtm);
                unsafe {
                    // `TabBar` and not `Automatic`: on iPad the automatic
                    // one may turn it into a sidebar, and that changes the
                    // whole screen out from under the layout.
                    controller.setMode(objc2_ui_kit::UITabBarControllerMode::TabBar);
                }
                let delegate = crate::events::TabDelegate::new(mtm, id, self.events.clone());
                unsafe {
                    controller.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(
                        &*delegate,
                    )))
                };
                self.tab_delegates.insert(id, delegate);
                let view = controller.view().expect("the controller comes with a view");
                // The controller's view is only the slot the bar goes in: the
                // tree supplies the content. Without this its white background
                // shows through underneath.
                view.setBackgroundColor(None);
                self.tab_controllers.insert(id, controller);
                HostView::TabsHost(view)
            }
            NodeKind::Switch => HostView::Toggle(UISwitch::new(mtm)),
            NodeKind::Slider => HostView::Slide(UISlider::new(mtm)),
            NodeKind::ActivityIndicator => {
                let spinner = UIActivityIndicatorView::new(mtm);
                spinner.setHidesWhenStopped(true);
                HostView::Spinner(spinner)
            }
            NodeKind::ProgressBar => HostView::Progress(UIProgressView::new(mtm)),
            NodeKind::Button => HostView::Button(UIButton::new(mtm)),
            NodeKind::Alert => {
                let placeholder = UIView::new(mtm);
                placeholder.setHidden(true);
                self.alerts.insert(id, crate::alert::AlertState::default());
                HostView::Dialog(placeholder)
            }
            NodeKind::Modal => {
                let overlay = UIView::new(mtm);
                overlay.setHidden(true);
                self.modals.insert(id, crate::modal::ModalState::default());
                HostView::Overlay(overlay)
            }
            NodeKind::StackView => {
                let stack = UIView::new(mtm);
                // Screens on their way in and out spill past the frame:
                // unclipped, they would be seen sliding over everything
                // else.
                stack.setClipsToBounds(true);
                HostView::Stack(stack)
            }
            NodeKind::TextInput => HostView::Field(UITextField::new(mtm)),
            NodeKind::TextEditor => {
                let text_view = objc2_ui_kit::UITextView::new(mtm);
                unsafe {
                    // No background and no insets of its own: the template
                    // supplies those, just as with a single-line field.
                    text_view.setBackgroundColor(None);
                    text_view.setTextContainerInset(objc2_ui_kit::UIEdgeInsets {
                        top: 0.0,
                        left: 0.0,
                        bottom: 0.0,
                        right: 0.0,
                    });
                    text_view.textContainer().setLineFragmentPadding(0.0);
                }
                HostView::Area(text_view)
            }
            NodeKind::NavigationBar => {
                let bar = objc2_ui_kit::UINavigationBar::new(mtm);
                HostView::Nav(bar)
            }
            #[cfg(not(target_os = "tvos"))]
            NodeKind::WebView => {
                let web = crate::web::WKWebView::new(mtm);
                HostView::Web(web)
            }
            NodeKind::MapView => HostView::Map(crate::map::MKMapView::new(mtm)),
            NodeKind::Custom => {
                // A plain container to begin with. The name arrives as the
                // `an:view` prop in the same frame, and the plugin's view is
                // added inside this one then: a plugin cannot be asked for a
                // view before the tree has said which one.
                HostView::View(UIView::new(mtm))
            }
            NodeKind::VideoView => {
                let player = UIView::new(mtm);
                HostView::Video(player)
            }
            NodeKind::SegmentedControl => {
                HostView::Segments(objc2_ui_kit::UISegmentedControl::new(mtm))
            }
            NodeKind::Stepper => HostView::Step(objc2_ui_kit::UIStepper::new(mtm)),
            NodeKind::SearchBar => {
                let search = objc2_ui_kit::UISearchBar::new(mtm);
                // `.minimal` is the field on its own. The default style draws a
                // bar behind it — a chrome background from the days when a
                // search bar sat under a navigation bar and had to match it —
                // and on any page that is not exactly that grey it reads as a
                // rectangle somebody forgot to remove. The field itself is
                // unchanged, including its placeholder, its magnifier and its
                // clear button.
                search.setSearchBarStyle(objc2_ui_kit::UISearchBarStyle::Minimal);
                HostView::Search(search)
            }
            NodeKind::Picker => {
                // A drop-down on iOS is a button that opens a menu: there is
                // no separate control, and `UIPickerView` is the full-screen
                // wheel, which is a different thing.
                let button = objc2_ui_kit::UIButton::new(mtm);
                unsafe { button.setShowsMenuAsPrimaryAction(true) };
                HostView::Menu(button)
            }
            NodeKind::DatePicker => {
                let picker = objc2_ui_kit::UIDatePicker::new(mtm);
                unsafe {
                    picker.setPreferredDatePickerStyle(objc2_ui_kit::UIDatePickerStyle::Compact);
                    // By default UIKit asks for a date *and* a time. This
                    // one asks for a date unless told otherwise, as on
                    // Android.
                    picker.setDatePickerMode(objc2_ui_kit::UIDatePickerMode::Date);
                };
                HostView::Date(picker)
            }
            // `View` lands here, which is the primitive people make
            // pressable with a `(press)`. On tvOS it cannot be just any
            // `UIView`: `canBecomeFocused` can only be changed by inheriting,
            // and without that the remote never reaches that view. See
            // `focus.rs`.
            #[cfg(target_os = "tvos")]
            _ => HostView::View(Retained::into_super(crate::focus::FocusableView::new(
                mtm,
                id,
                self.events.clone(),
            ))),
            #[cfg(not(target_os = "tvos"))]
            _ => HostView::View(UIView::new(mtm)),
        };
        // Who rules the frame.
        //
        // `false` means "my frame is decided by my constraints", and that is
        // what was here. While there was no control with constraints of its
        // own it made no difference: the Auto Layout engine never came to life
        // and the frames stayed as the core wrote them. The moment a compound
        // one came in —a `UISearchBar`, a `UIDatePicker`— the engine switched
        // on for the whole window and zeroed the frame of every view that
        // claimed to be waiting on constraints that did not exist. It showed
        // up as the entire screen piled into the corner.
        //
        // `true` is what to say when you write the frame yourself: UIKit
        // translates it into constraints and respects what it is given.
        view.as_view().setTranslatesAutoresizingMaskIntoConstraints(true);
        self.views.insert(id, view);
    }

    fn destroy(&mut self, id: NodeId) {
        if let Some(view) = self.views.remove(&id) {
            // A screen on its way out stays on screen until the animation
            // finishes: taking it away now would give a jump.
            if !self.animating_out.contains(&id) {
                view.as_view().removeFromSuperview();
            }
        }
        self.fonts.remove(&id);
        self.corners.remove(&id);
        self.safe_area.remove(&id);
        self.alerts.remove(&id);
        self.slider_values.remove(&id);
        self.transforms.remove(&id);
        self.animations.remove(&id);
        self.icons.remove(&id);
        self.tabs.remove(&id);
        self.tab_controllers.remove(&id);
        self.tab_delegates.remove(&id);
        self.menus.remove(&id);
        self.button_titles.remove(&id);
        self.button_colors.remove(&id);
        self.button_variants.remove(&id);
        self.button_subtitles.remove(&id);
        self.button_icons.remove(&id);
        self.placeholders.remove(&id);
        self.placeholder_colors.remove(&id);
        self.decorations.remove(&id);
        self.navs.remove(&id);
        self.nav_backs.remove(&id);
        self.nav_targets.remove(&id);
        self.maps.remove(&id);
        self.videos.remove(&id);
        self.video_playing.remove(&id);
        self.stepper_values.remove(&id);
        self.modals.remove(&id);
        self.listeners.retain(|(node, _), _| *node != id);
        self.accessibility.forget(id);
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        let (Some(parent_view), Some(child_view)) = (self.views.get(&parent), self.views.get(&child))
        else {
            return;
        };
        let is_stack = matches!(parent_view, HostView::Stack(_));
        // A screen coming in has to end up above the one going out, even
        // when the tree places it before.
        let index = if is_stack {
            parent_view.as_view().subviews().len() as isize
        } else {
            index as isize
        };
        parent_view.as_view().insertSubview_atIndex(child_view.as_view(), index);
        if is_stack {
            self.entering.push((parent, child));
        }
    }

    fn remove(&mut self, parent: NodeId, child: NodeId) {
        let Some(view) = self.views.get(&child) else { return };
        let native = view.as_view().retain();
        let popping = matches!(self.views.get(&parent), Some(HostView::Stack(_)))
            && self.transitions.get(&parent).map(String::as_str) == Some("pop");
        if popping {
            // It stays mounted until it has finished leaving.
            self.animating_out.insert(child);
            self.leaving.push((parent, native));
            return;
        }
        native.removeFromSuperview();
    }

    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        let text = value.as_str().map(str::to_owned);
        let number = value.as_f32();

        match key {
            // Which view a plugin should put here. It arrives once, in the same
            // frame the node was created in, and the plugin is asked then —
            // never before, because until this prop lands nothing knows which
            // view is wanted.
            "an:view" => {
                let Some(name) = text.as_deref() else { return };
                let Some(brought) = crate::plugin_views::make(name) else {
                    // Once per name and not once per node: a list of five
                    // hundred rows with the same missing view is one mistake,
                    // not five hundred.
                    if self.warned_views.insert(name.to_owned()) {
                        eprintln!(
                            "angular-native: no plugin registers a view called {name:?}, so \
                             <an-custom [view]=\"{name}\"> mounts nothing. The name is the one \
                             the plugin passes to AnPluginViews.register."
                        );
                    }
                    return;
                };
                // It fills the node, whose size layout decided: a plugin view is
                // never measured by its content, so the frame is the answer and
                // not a starting point.
                brought.setFrame(native.bounds());
                brought.setAutoresizingMask(
                    objc2_ui_kit::UIViewAutoresizing::FlexibleWidth
                        | objc2_ui_kit::UIViewAutoresizing::FlexibleHeight,
                );
                native.addSubview(&brought);
            }
            "backgroundColor" | "background-color" => {
                let color = text.as_deref().and_then(crate::color::to_uicolor);
                native.setBackgroundColor(color.as_deref());
            }
            "opacity" => {
                if let Some(v) = number {
                    let view = native.retain();
                    self.animated(id, move || view.setAlpha(v as f64));
                }
            }
            // Single-platform props. They travel with their prefix, so this
            // host drops the other's at a glance: it does not have to know
            // what they mean, only whose they are.
            _ if key.starts_with("android:") => {}
            "variant" | "icon" | "iconPosition" | "ios:subtitle"
                if matches!(view, HostView::Button(_)) =>
            {
                match key {
                    "variant" => {
                        self.button_variants
                            .insert(id, text.clone().unwrap_or_else(|| "text".to_owned()));
                    }
                    "ios:subtitle" => {
                        self.button_subtitles.insert(id, text.clone().unwrap_or_default());
                    }
                    "icon" => {
                        let entry = self.button_icons.entry(id).or_default();
                        entry.0 = text.clone().unwrap_or_default();
                    }
                    _ => {
                        let entry = self.button_icons.entry(id).or_default();
                        entry.1 = text.clone().unwrap_or_else(|| "leading".to_owned());
                    }
                }
                // A button with no icon name carries no icon, even when the
                // side is still set from before.
                if self.button_icons.get(&id).is_some_and(|(name, _)| name.is_empty()) {
                    self.button_icons.remove(&id);
                }
                self.refresh_button(id);
            }
            // Switching a control off is `UIControl`'s business: it knows how
            // to go grey and stop responding. Whatever is not one is left
            // untouchable instead, which is the nearest thing there is.
            "enabled" => {
                let on = !matches!(value, PropValue::Bool(false));
                match view {
                    HostView::Button(v) | HostView::Menu(v) => v.setEnabled(on),
                    HostView::Toggle(v) => v.setEnabled(on),
                    HostView::Slide(v) => v.setEnabled(on),
                    HostView::Segments(v) => v.setEnabled(on),
                    HostView::Step(v) => v.setEnabled(on),
                    HostView::Date(v) => v.setEnabled(on),
                    _ => native.setUserInteractionEnabled(on),
                }
            }
            // --- map
            "latitude" | "longitude" | "zoom" | "showsUser"
                if matches!(view, HostView::Map(_)) =>
            {
                let HostView::Map(map) = view else { return };
                if key == "showsUser" {
                    map.setShowsUserLocation(matches!(value, PropValue::Bool(true)));
                    return;
                }
                let entry = self.maps.entry(id).or_insert((0.0, 0.0, 12.0));
                match key {
                    "latitude" => entry.0 = number.unwrap_or(0.0) as f64,
                    "longitude" => entry.1 = number.unwrap_or(0.0) as f64,
                    _ => entry.2 = number.unwrap_or(12.0) as f64,
                }
                let (lat, lon, zoom) = *entry;
                // MapKit has no zoom levels: it has how much of the globe is
                // in view. Each level is half the previous one, and 0 spans
                // all 360 degrees of longitude, so the width is 360 / 2^zoom.
                let span = 360.0 / 2f64.powf(zoom.max(0.0));
                map.setRegion_animated(
                    crate::map::MKCoordinateRegion {
                        center: crate::map::CLLocationCoordinate2D {
                            latitude: lat,
                            longitude: lon,
                        },
                        span: crate::map::MKCoordinateSpan {
                            latitude_delta: span,
                            longitude_delta: span,
                        },
                    },
                    false,
                );
            }
            // --- video
            "url" | "playing" | "muted" if matches!(view, HostView::Video(_)) => {
                let HostView::Video(container) = view else { return };
                if key == "url" {
                    let Some(raw) = text.as_deref() else { return };
                    let Some(url) =
                        (unsafe { objc2_foundation::NSURL::URLWithString(&NSString::from_str(raw)) })
                    else {
                        return;
                    };
                    let player = crate::video::AVPlayer::with_url(&url, self.mtm);
                    let controller = crate::video::AVPlayerViewController::new(self.mtm);
                    controller.setPlayer(Some(&player));
                    controller.setShowsPlaybackControls(true);
                    // Real containment: the controller goes in as a child of
                    // the presiding one. Hanging up its view alone works until
                    // something —a rotation, full-screen mode— asks for the
                    // controller governing it and there is none.
                    if let Some(root) =
                        self.container.window().and_then(|w| w.rootViewController())
                    {
                        unsafe { root.addChildViewController(&controller) };
                        let view =
                            controller.view().expect("the controller comes with a view");
                        // The player's view brings an opaque background of its
                        // own, which would show around the picture wherever the
                        // video does not fill the frame. What is behind belongs
                        // to the template.
                        view.setBackgroundColor(None);
                        view.setFrame(container.bounds());
                        container.addSubview(&view);
                        unsafe { controller.didMoveToParentViewController(Some(&root)) };
                    }
                    self.videos.insert(id, (player, controller));
                    return;
                }
                let Some((player, _)) = self.videos.get(&id) else { return };
                match key {
                    "playing" => {
                        if matches!(value, PropValue::Bool(true)) {
                            self.video_playing.insert(id);
                            player.play();
                        } else {
                            self.video_playing.remove(&id);
                            player.pause();
                        }
                    }
                    _ => player.setMuted(matches!(value, PropValue::Bool(true))),
                }
            }
            // --- navigation header
            "title" | "backTitle" | "showsBack" if matches!(view, HostView::Nav(_)) => {
                let HostView::Nav(bar) = view else { return };
                let entry = self.navs.entry(id).or_default();
                match key {
                    "title" => entry.0 = text.clone().unwrap_or_default(),
                    "backTitle" => entry.1 = text.clone().unwrap_or_default(),
                    _ => entry.2 = matches!(value, PropValue::Bool(true)),
                }
                let (title, back_title, shows_back) = entry.clone();
                let item = objc2_ui_kit::UINavigationItem::new(self.mtm);
                unsafe { item.setTitle(Some(&NSString::from_str(&title))) };
                if shows_back {
                    // Outside a `UINavigationController` there is no
                    // automatic back button: one is put there with the same
                    // symbol in the same place, and what navigates is the
                    // router.
                    //
                    // The target lives as long as the header does: the button
                    // is rebuilt on every title change and the target is not.
                    let target = self
                        .nav_targets
                        .entry(id)
                        .or_insert_with(|| {
                            crate::events::ControlTarget::standalone(
                                self.mtm,
                                id,
                                self.events.clone(),
                            )
                        })
                        .clone();
                    let action = crate::events::ControlTarget::nav_back_action();
                    let back = if back_title.is_empty() {
                        unsafe {
                        objc2_ui_kit::UIBarButtonItem::initWithImage_style_target_action(
                            self.mtm.alloc::<objc2_ui_kit::UIBarButtonItem>(),
                            crate::icons::symbol("chevron.left", 0.0, 400).as_deref(),
                            objc2_ui_kit::UIBarButtonItemStyle::Plain,
                            Some(&*target),
                            Some(action),
                        )
                        }
                    } else {
                        unsafe {
                            objc2_ui_kit::UIBarButtonItem::initWithTitle_style_target_action(
                                self.mtm.alloc::<objc2_ui_kit::UIBarButtonItem>(),
                                Some(&NSString::from_str(&back_title)),
                                objc2_ui_kit::UIBarButtonItemStyle::Plain,
                                Some(&*target),
                                Some(action),
                            )
                        }
                    };
                    unsafe { item.setLeftBarButtonItem(Some(&back)) };
                    self.nav_backs.insert(id, back);
                }
                bar.setItems(Some(&objc2_foundation::NSArray::from_retained_slice(&[item])));
            }
            // --- multi-line text
            "value" if matches!(view, HostView::Area(_)) => {
                let HostView::Area(area) = view else { return };
                let next = text.clone().unwrap_or_default();
                // As with the single-line field: it is not rewritten if it
                // already says that, or the caret jumps to the end while the
                // user types.
                let current = unsafe { area.text() }.to_string();
                if current != next {
                    unsafe { area.setText(Some(&NSString::from_str(&next))) };
                }
            }
            "editable" if matches!(view, HostView::Area(_)) => {
                let HostView::Area(area) = view else { return };
                unsafe { area.setEditable(!matches!(value, PropValue::Bool(false))) };
            }
            // --- embedded browser. It does not exist on tvOS: with no
            // WebKit there is no view to load, and the node was never even
            // created.
            #[cfg(not(target_os = "tvos"))]
            "url" if matches!(view, HostView::Web(_)) => {
                let HostView::Web(web) = view else { return };
                let Some(raw) = text.as_deref() else { return };
                let Some(url) = (unsafe {
                    objc2_foundation::NSURL::URLWithString(&NSString::from_str(raw))
                }) else {
                    return;
                };
                let request = unsafe { objc2_foundation::NSURLRequest::requestWithURL(&url) };
                let _ = web.loadRequest(&request);
            }
            #[cfg(not(target_os = "tvos"))]
            "html" if matches!(view, HostView::Web(_)) => {
                let HostView::Web(web) = view else { return };
                let _ = web.loadHTMLString_baseURL(
                    &NSString::from_str(text.as_deref().unwrap_or("")),
                    None,
                );
            }
            // --- segmented control, drop-down and date picker
            "items" if matches!(view, HostView::Segments(_) | HostView::Menu(_)) => {
                let titles = parse_string_list(text.as_deref().unwrap_or("[]"));
                match view {
                    HostView::Segments(segments) => {
                        segments.removeAllSegments();
                        for (index, title) in titles.iter().enumerate() {
                            unsafe {
                                segments.insertSegmentWithTitle_atIndex_animated(
                                    Some(&NSString::from_str(title)),
                                    index,
                                    false,
                                );
                            }
                        }
                    }
                    HostView::Menu(button) => {
                        self.menus.insert(id, titles.clone());
                        let menu = crate::menu::build(self.mtm, id, &titles, &self.events);
                        unsafe { button.setMenu(Some(&menu)) };
                        // With no title the button cannot be seen: the first
                        // one goes in until somebody chooses.
                        let current = titles.first().cloned().unwrap_or_default();
                        unsafe {
                            button.setTitle_forState(
                                Some(&NSString::from_str(&current)),
                                UIControlState::Normal,
                            );
                        }
                    }
                    _ => {}
                }
            }
            "selectedIndex" if matches!(view, HostView::Segments(_) | HostView::Menu(_)) => {
                let index = number.unwrap_or(0.0).max(0.0) as usize;
                match view {
                    HostView::Segments(segments) => {
                        segments.setSelectedSegmentIndex(index as isize)
                    }
                    HostView::Menu(button) => {
                        let title = self
                            .menus
                            .get(&id)
                            .and_then(|titles| titles.get(index))
                            .cloned()
                            .unwrap_or_default();
                        unsafe {
                            button.setTitle_forState(
                                Some(&NSString::from_str(&title)),
                                UIControlState::Normal,
                            );
                        }
                    }
                    _ => {}
                }
            }
            "value" | "minimumValue" | "maximumValue" | "stepValue"
                if matches!(view, HostView::Step(_)) =>
            {
                let HostView::Step(stepper) = view else { return };
                let v = number.unwrap_or(0.0) as f64;
                unsafe {
                    match key {
                        // As with the slider: the range before the value, or
                        // the value gets clamped against the old range.
                        "minimumValue" => stepper.setMinimumValue(v),
                        "maximumValue" => stepper.setMaximumValue(v),
                        "stepValue" => stepper.setStepValue(if v > 0.0 { v } else { 1.0 }),
                        _ => {
                            self.stepper_values.insert(id, v);
                            stepper.setValue(v);
                        }
                    }
                    if key != "value" {
                        if let Some(wanted) = self.stepper_values.get(&id).copied() {
                            stepper.setValue(wanted);
                        }
                    }
                }
            }
            "value" if matches!(view, HostView::Date(_)) => {
                let HostView::Date(picker) = view else { return };
                // It arrives in milliseconds since 1970, which is what
                // `Date` gives in JS. `NSDate` works in seconds.
                let seconds = number.unwrap_or(0.0) as f64 / 1000.0;
                let date = unsafe {
                    objc2_foundation::NSDate::dateWithTimeIntervalSince1970(seconds)
                };
                unsafe { picker.setDate(&date) };
            }
            "mode" if matches!(view, HostView::Date(_)) => {
                let HostView::Date(picker) = view else { return };
                unsafe {
                    picker.setDatePickerMode(match text.as_deref() {
                        Some("time") => objc2_ui_kit::UIDatePickerMode::Time,
                        Some("dateAndTime") => objc2_ui_kit::UIDatePickerMode::DateAndTime,
                        _ => objc2_ui_kit::UIDatePickerMode::Date,
                    })
                };
            }
            "value" | "placeholder" if matches!(view, HostView::Search(_)) => {
                let HostView::Search(bar) = view else { return };
                let value = text.as_deref().map(NSString::from_str);
                unsafe {
                    if key == "value" {
                        bar.setText(value.as_deref());
                    } else {
                        bar.setPlaceholder(value.as_deref());
                    }
                }
            }
            // Icons. The name and the size go together: a symbol's size does
            // not scale the drawing, it picks the stroke, so it has to be
            // rebuilt when either of them changes.
            "name" | "iconSize" | "iconWeight" if matches!(view, HostView::Image(_)) => {
                let entry = self.icons.entry(id).or_default();
                match key {
                    "name" => entry.0 = text.clone().unwrap_or_default(),
                    "iconSize" => entry.1 = number.unwrap_or(24.0),
                    _ => entry.2 = number.unwrap_or(400.0) as u16,
                }
                let (name, size, weight) = entry.clone();
                if let HostView::Image(image_view) = view {
                    let symbol = crate::icons::symbol(&name, size, weight);
                    unsafe {
                        let templated = symbol.map(|image| {
                            image.imageWithRenderingMode(
                                objc2_ui_kit::UIImageRenderingMode::AlwaysTemplate,
                            )
                        });
                        image_view.setImage(templated.as_deref());
                    }
                }
            }
            // Animation. It is not a value you can see: it says how the ones
            // you can are arrived at.
            "animate" => {
                let entry = self.animations.entry(id).or_default();
                // Milliseconds are what gets written in a template; UIKit
                // works in seconds.
                entry.duration = number.unwrap_or(0.0) as f64 / 1000.0;
            }
            "animateDelay" => {
                let entry = self.animations.entry(id).or_default();
                entry.delay = number.unwrap_or(0.0) as f64 / 1000.0;
            }
            "animateEasing" => {
                let entry = self.animations.entry(id).or_default();
                entry.curve = match text.as_deref() {
                    Some("linear") => UIViewAnimationOptions::CurveLinear,
                    Some("ease-in") => UIViewAnimationOptions::CurveEaseIn,
                    Some("ease-in-out") => UIViewAnimationOptions::CurveEaseInOut,
                    // `ease-out` is what is wanted nearly always: off fast
                    // and easing to a stop, which is how things move.
                    _ => UIViewAnimationOptions::CurveEaseOut,
                };
            }
            // Transforms. They do not go through the layout, on purpose:
            // moving or scaling a view does not change the room it takes, so
            // there is nothing to recompute. It is what makes following a
            // finger at 120 Hz possible.
            "translateX" | "translateY" | "scale" | "scaleX" | "scaleY" | "rotate" => {
                let entry = self.transforms.entry(id).or_default();
                let v = number.unwrap_or(0.0) as f64;
                match key {
                    "translateX" => entry.translate_x = v,
                    "translateY" => entry.translate_y = v,
                    "scale" => {
                        entry.scale_x = if number.is_some() { v } else { 1.0 };
                        entry.scale_y = entry.scale_x;
                    }
                    "scaleX" => entry.scale_x = if number.is_some() { v } else { 1.0 },
                    "scaleY" => entry.scale_y = if number.is_some() { v } else { 1.0 },
                    _ => entry.rotate = v,
                }
                let transform = entry.matrix();
                let view = native.retain();
                self.animated(id, move || view.setTransform(transform));
            }
            "borderRadius" | "border-radius" => {
                if let Some(v) = number {
                    self.corners.insert(id, [v as f64; 4]);
                    self.apply_corners(id);
                }
            }
            "borderTopLeftRadius" => self.set_corner(id, 0, number),
            "borderTopRightRadius" => self.set_corner(id, 1, number),
            "borderBottomRightRadius" => self.set_corner(id, 2, number),
            "borderBottomLeftRadius" => self.set_corner(id, 3, number),
            "borderColor" | "border-color" => {
                if let Some(color) = text.as_deref().and_then(crate::color::to_uicolor) {
                    // `CGColor` is not guaranteed thread-safe; here we are
                    // always on the main thread.
                    let cg = unsafe { color.CGColor() };
                    native.layer().setBorderColor(Some(&cg));
                }
            }
            "borderWidth" | "border-width" => {
                if let Some(v) = number {
                    native.layer().setBorderWidth(v as f64);
                }
            }
            // --- the system's dialogs
            "title" | "message" | "buttons" | "visible" | "sheet"
                if self.alerts.contains_key(&id) =>
            {
                let Some(state) = self.alerts.get_mut(&id) else { return };
                match key {
                    "title" => state.title = text.clone().unwrap_or_default(),
                    "message" => state.message = text.clone().unwrap_or_default(),
                    "buttons" => {
                        state.buttons = parse_string_list(text.as_deref().unwrap_or("[]"))
                    }
                    "sheet" => state.sheet = matches!(value, PropValue::Bool(true)),
                    _ => state.visible = matches!(value, PropValue::Bool(true)),
                }
                if !self.dirty_alerts.contains(&id) {
                    self.dirty_alerts.push(id);
                }
            }
            "transition" => {
                if let Some(direction) = &text {
                    self.transitions.insert(id, direction.clone());
                }
            }
            "source" => {
                if let HostView::Image(image) = view {
                    crate::images::load(
                        self.mtm,
                        image,
                        id,
                        text.as_deref().unwrap_or_default(),
                        self.events.clone(),
                    );
                }
            }
            "resizeMode" => {
                if let HostView::Image(image) = view {
                    image.setContentMode(crate::images::content_mode(
                        text.as_deref().unwrap_or("contain"),
                    ));
                }
            }
            "testID" | "accessibilityIdentifier" => {
                if let Some(t) = &text {
                    native.setAccessibilityIdentifier(Some(&NSString::from_str(t)));
                }
            }
            // The six of the contract. They all go together to
            // `accessibility.rs` because they are not six independent props:
            // role and state end up in the same bit mask, and `checked` ends
            // up in the value only if the template set none.
            _ if crate::accessibility::handles(key) => {
                let kind = format!("{:?}", view.kind());
                let native = native.retain();
                self.accessibility.apply(self.mtm, id, &native, &kind, key, value);
            }
            "color" => {
                let Some(color) = text.as_deref().and_then(crate::color::to_uicolor) else {
                    return;
                };
                match view {
                    HostView::Label(label) => unsafe { label.setTextColor(Some(&color)) },
                    HostView::Field(field) => unsafe { field.setTextColor(Some(&color)) },
                    HostView::Area(area) => unsafe { area.setTextColor(Some(&color)) },
                    HostView::Toggle(toggle) => toggle.setOnTintColor(Some(&color)),
                    HostView::Slide(slider) => slider.setMinimumTrackTintColor(Some(&color)),
                    HostView::Spinner(spinner) => unsafe { spinner.setColor(Some(&color)) },
                    // A symbol is tinted, not recoloured: it is drawn as a
                    // template and the tint rules.
                    HostView::Image(image) => unsafe { image.setTintColor(Some(&color)) },
                    HostView::Progress(bar) => bar.setProgressTintColor(Some(&color)),
                    HostView::Button(_) => {
                        if let Some(raw) = text.as_deref() {
                            self.button_colors.insert(id, raw.to_owned());
                        }
                        self.refresh_button(id);
                    }
                    HostView::TabsHost(_) => {
                        if let Some(controller) = self.tab_controllers.get(&id) {
                            unsafe { controller.tabBar().setTintColor(Some(&color)) };
                        }
                    }
                    _ => {}
                }
            }
            "textAlign" | "text-align" => {
                let Some(t) = &text else { return };
                let alignment = match t.as_str() {
                    "center" => NSTextAlignment::Center,
                    "right" => NSTextAlignment::Right,
                    "justify" => NSTextAlignment::Justified,
                    _ => NSTextAlignment::Left,
                };
                match view {
                    HostView::Label(label) => label.setTextAlignment(alignment),
                    // The field and the editor align too, and until now they
                    // were left with their own.
                    HostView::Field(field) => unsafe { field.setTextAlignment(alignment) },
                    HostView::Area(area) => unsafe { area.setTextAlignment(alignment) },
                    _ => {}
                }
            }
            "fontSize" => {
                if let Some(v) = number {
                    self.font_mut(id).size = v;
                    self.apply_font(id);
                }
            }
            "fontWeight" => {
                let weight = match value {
                    PropValue::Number(n) => *n as u16,
                    PropValue::Str(s) if s == "bold" => 700,
                    PropValue::Str(s) => s.parse().unwrap_or(400),
                    _ => 400,
                };
                self.font_mut(id).weight = weight;
                self.apply_font(id);
            }
            "fontStyle" => {
                self.font_mut(id).italic = text.as_deref() == Some("italic");
                self.apply_font(id);
            }
            "fontFamily" => {
                self.font_mut(id).family = text.clone();
                self.apply_font(id);
            }
            "textDecoration" | "text-decoration" => {
                self.decorations.insert(id, text.clone().unwrap_or_else(|| "none".to_owned()));
                self.apply_text_attributes(id);
            }
            "letterSpacing" | "letter-spacing" => {
                self.font_mut(id).letter_spacing = number.unwrap_or(0.0);
                self.apply_text_attributes(id);
            }
            "lineHeight" | "line-height" => {
                self.font_mut(id).line_height = number;
                self.apply_text_attributes(id);
            }
            // --- text fields
            "value" => {
                if let (HostView::Slide(slider), Some(v)) = (view, number) {
                    self.slider_values.insert(id, v);
                    // Only if it differs: writing it while the thumb is being
                    // dragged would fight with the user's finger.
                    if (slider.value() - v).abs() > f32::EPSILON {
                        slider.setValue(v);
                    }
                }
                if let HostView::Field(field) = view {
                    // Writing the text while the user types would move their
                    // caret to the end on every keystroke: it is only applied
                    // if it really differs.
                    let current = field.text().map(|t| t.to_string()).unwrap_or_default();
                    let next = text.clone().unwrap_or_default();
                    if current != next {
                        field.setText(Some(&NSString::from_str(&next)));
                    }
                }
            }
            "placeholder" => {
                if let HostView::Field(_) = view {
                    self.placeholders.insert(id, text.clone().unwrap_or_default());
                    self.apply_placeholder(id);
                }
            }
            // The placeholder text has no reason to take the text's colour,
            // and `UITextField` has no prop for it: it has to be given as an
            // attributed string, so the colour and the text are kept
            // together.
            "placeholderColor" => {
                match &text {
                    Some(color) => self.placeholder_colors.insert(id, color.clone()),
                    None => self.placeholder_colors.remove(&id),
                };
                self.apply_placeholder(id);
            }
            // How the keyboard behaves. These are the `UITextInputTraits`,
            // which live in the protocol and hold alike for the field and for
            // the editor.
            "keyboardType" | "returnKeyType" | "autoCapitalize" | "autoCorrect" => match view {
                HostView::Field(field) => apply_text_traits(&**field, key, text.as_deref(), value),
                HostView::Area(area) => apply_text_traits(&**area, key, text.as_deref(), value),
                _ => {}
            },
            "ios:clearButtonMode" => {
                if let HostView::Field(field) = view {
                    unsafe {
                        field.setClearButtonMode(match text.as_deref() {
                            Some("whileEditing") => objc2_ui_kit::UITextFieldViewMode::WhileEditing,
                            Some("always") => objc2_ui_kit::UITextFieldViewMode::Always,
                            _ => objc2_ui_kit::UITextFieldViewMode::Never,
                        })
                    };
                }
            }
            "ios:borderStyle" => {
                if let HostView::Field(field) = view {
                    unsafe {
                        field.setBorderStyle(match text.as_deref() {
                            Some("line") => objc2_ui_kit::UITextBorderStyle::Line,
                            Some("bezel") => objc2_ui_kit::UITextBorderStyle::Bezel,
                            Some("roundedRect") => objc2_ui_kit::UITextBorderStyle::RoundedRect,
                            _ => objc2_ui_kit::UITextBorderStyle::None,
                        })
                    };
                }
            }
            "secureTextEntry" => {
                if let HostView::Field(field) = view {
                    field.setSecureTextEntry(matches!(value, PropValue::Bool(true)));
                }
            }
            "editable" => {
                if let HostView::Field(field) = view {
                    field.setEnabled(!matches!(value, PropValue::Bool(false)));
                }
            }
            // --- the system's controls
            "on" => {
                if let HostView::Toggle(toggle) = view {
                    toggle.setOn(matches!(value, PropValue::Bool(true)));
                }
            }
            "minimumValue" | "maximumValue" => {
                let (HostView::Slide(slider), Some(v)) = (view, number) else { return };
                if key == "minimumValue" {
                    slider.setMinimumValue(v);
                } else {
                    slider.setMaximumValue(v);
                }
                // The range changed: the value has to be applied again,
                // since it may have arrived earlier and been clamped.
                if let Some(wanted) = self.slider_values.get(&id).copied() {
                    slider.setValue(wanted);
                }
            }
            // The switch's and the slider's separate colours. `[color]` is
            // still the main one —what is switched on, the travelled stretch—
            // and these are the others.
            "thumbColor" => {
                let Some(color) = text.as_deref().and_then(crate::color::to_uicolor) else {
                    return;
                };
                match view {
                    HostView::Toggle(toggle) => toggle.setThumbTintColor(Some(&color)),
                    HostView::Slide(slider) => slider.setThumbTintColor(Some(&color)),
                    _ => {}
                }
            }
            "minimumTrackColor" | "maximumTrackColor" => {
                let (HostView::Slide(slider), Some(color)) =
                    (view, text.as_deref().and_then(crate::color::to_uicolor))
                else {
                    return;
                };
                if key == "minimumTrackColor" {
                    slider.setMinimumTrackTintColor(Some(&color));
                } else {
                    slider.setMaximumTrackTintColor(Some(&color));
                }
            }
            "ios:continuous" => {
                if let HostView::Slide(slider) = view {
                    // Switched off, the slider only reports on release. Good
                    // for whatever is expensive to recompute at every point.
                    slider.setContinuous(!matches!(value, PropValue::Bool(false)));
                }
            }
            "animating" => {
                if let HostView::Spinner(spinner) = view {
                    if matches!(value, PropValue::Bool(false)) {
                        spinner.stopAnimating();
                    } else {
                        spinner.startAnimating();
                    }
                }
            }
            "progress" => {
                if let (HostView::Progress(bar), Some(v)) = (view, number) {
                    bar.setProgress(v.clamp(0.0, 1.0));
                }
            }
            "title" => {
                if matches!(view, HostView::Button(_)) {
                    self.button_titles.insert(id, text.clone().unwrap_or_default());
                    self.refresh_button(id);
                }
            }
            "items" | "icons" if matches!(view, HostView::TabsHost(_)) => {
                // The titles and the icons arrive as JSON: the protocol
                // carries no lists, and a tab bar does not justify adding
                // them. They arrive as separate props, so they are kept and
                // the tabs are rebuilt from both every time.
                let entry = self.tabs.entry(id).or_default();
                let list = parse_string_list(text.as_deref().unwrap_or("[]"));
                if key == "items" {
                    entry.0 = list;
                } else {
                    entry.1 = list;
                }
                let (titles, icons) = entry.clone();
                let Some(controller) = self.tab_controllers.get(&id) else { return };
                // A `UITabBarController`'s tab is a controller with its own
                // `tabBarItem`. The ones here go in empty: the tree supplies
                // the content, not them; what is wanted from the controller is
                // that it draws and places the bar the way the system says.
                let controllers: Vec<Retained<objc2_ui_kit::UIViewController>> = titles
                    .iter()
                    .enumerate()
                    .map(|(index, title)| {
                        let vc = objc2_ui_kit::UIViewController::new(self.mtm);
                        let image =
                            icons.get(index).and_then(|name| crate::icons::symbol(name, 0.0, 400));
                        let item = unsafe {
                            objc2_ui_kit::UITabBarItem::initWithTitle_image_tag(
                                self.mtm.alloc::<objc2_ui_kit::UITabBarItem>(),
                                Some(&NSString::from_str(title)),
                                image.as_deref(),
                                index as isize,
                            )
                        };
                        unsafe { vc.setTabBarItem(Some(&item)) };
                        vc
                    })
                    .collect();
                unsafe {
                    controller.setViewControllers(Some(
                        &objc2_foundation::NSArray::from_retained_slice(&controllers),
                    ))
                };
            }
            "selectedIndex" => {
                if let (HostView::TabsHost(_), Some(index)) = (view, number) {
                    if let Some(controller) = self.tab_controllers.get(&id) {
                        unsafe { controller.setSelectedIndex(index.max(0.0) as usize) };
                    }
                }
            }
            "presentation" if self.modals.contains_key(&id) => {
                if let Some(state) = self.modals.get_mut(&id) {
                    state.sheet = text.as_deref() == Some("sheet");
                }
                if !self.dirty_modals.contains(&id) {
                    self.dirty_modals.push(id);
                }
            }
            // Only a sheet has anywhere to rest, and it is UIKit's idea:
            // Android's `Dialog` is as tall as its content and has no list of
            // stops to be given. Hence the prefix.
            "ios:detents" if self.modals.contains_key(&id) => {
                if let Some(state) = self.modals.get_mut(&id) {
                    state.set_detents(text.as_deref());
                }
                if !self.dirty_modals.contains(&id) {
                    self.dirty_modals.push(id);
                }
            }
            "visible" if self.modals.contains_key(&id) => {
                if let Some(state) = self.modals.get_mut(&id) {
                    state.visible = matches!(value, PropValue::Bool(true));
                }
                // Hidden while it is not presented. On presentation it is
                // the controller that shows it; were it left visible without
                // being presented, the modal's content would be drawn inline
                // over the page.
                let visible = self.modals.get(&id).is_some_and(|m| m.visible);
                native.setHidden(!visible);
                if !self.dirty_modals.contains(&id) {
                    self.dirty_modals.push(id);
                }
            }
            "visible" => {
                if let HostView::Overlay(overlay) = view {
                    overlay.setHidden(matches!(value, PropValue::Bool(false)));
                }
            }
            // --- scrolling
            //
            // Pull to refresh is iOS's: `UIRefreshControl` is not in the tvOS
            // SDK. On a television there is nothing to pull, so the prop has
            // nobody to talk to. The warning is given at subscription time, in
            // `events::attach`, and not here: repeating it on every change of
            // value would fill the log with the same warning.
            #[cfg(not(target_os = "tvos"))]
            "refreshing" => {
                // The subscription to `refresh` brought the control: if
                // nobody is listening, there is nothing to stop.
                if let Some(control) = self
                    .listeners
                    .get(&(id, "refresh".to_owned()))
                    .and_then(crate::events::AttachedListener::refresh_control)
                {
                    if matches!(value, PropValue::Bool(true)) {
                        control.beginRefreshing();
                    } else {
                        control.endRefreshing();
                    }
                }
            }
            "showsScrollIndicator" => {
                if let HostView::Scroll(scroll) = view {
                    let shown = !matches!(value, PropValue::Bool(false));
                    scroll.setShowsVerticalScrollIndicator(shown);
                    scroll.setShowsHorizontalScrollIndicator(shown);
                }
            }
            "bounces" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setBounces(!matches!(value, PropValue::Bool(false)));
                }
            }
            "scrollEnabled" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setScrollEnabled(!matches!(value, PropValue::Bool(false)));
                }
            }
            "ios:pagingEnabled" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setPagingEnabled(matches!(value, PropValue::Bool(true)));
                }
            }
            "ios:keyboardDismissMode" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setKeyboardDismissMode(match text.as_deref() {
                        Some("onDrag") => {
                            objc2_ui_kit::UIScrollViewKeyboardDismissMode::OnDrag
                        }
                        Some("interactive") => {
                            objc2_ui_kit::UIScrollViewKeyboardDismissMode::Interactive
                        }
                        _ => objc2_ui_kit::UIScrollViewKeyboardDismissMode::None,
                    });
                }
            }
            // --- tab bar
            "unselectedColor" => {
                let (HostView::TabsHost(_), Some(controller)) =
                    (view, self.tab_controllers.get(&id))
                else {
                    return;
                };
                let color = text.as_deref().and_then(crate::color::to_uicolor);
                unsafe { controller.tabBar().setUnselectedItemTintColor(color.as_deref()) };
            }
            "ios:translucent" => {
                if let Some(controller) = self.tab_controllers.get(&id) {
                    // With the bar opaque, what is underneath stops showing
                    // through: useful when the content behind it reads badly.
                    unsafe {
                        controller.tabBar().setTranslucent(!matches!(value, PropValue::Bool(false)))
                    };
                }
            }
            "numberOfLines" => {
                self.font_mut(id).max_lines = number.filter(|v| *v >= 1.0).map(|v| v as u32);
                self.apply_font(id);
            }
            _ => {}
        }
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        if let Some(label) = self.views.get(&id).and_then(HostView::as_label) {
            label.setText(Some(&NSString::from_str(text)));
            // `setText` throws the attributed text away, so the letter
            // spacing and the line height have to be put back with every new
            // word.
            self.apply_text_attributes(id);
        }
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let key = (id, event.to_owned());
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();

        // No gesture produces the safe area: the system knows it, and it
        // changes on rotation or when the keyboard comes up. It is reported at
        // subscription time and afterwards on every layout, which is when it
        // may have changed.
        if event == "safeArea" {
            if enabled {
                self.safe_area.insert(id, [f32::NAN; 4]);
                self.report_safe_area(id);
            } else {
                self.safe_area.remove(&id);
            }
            return;
        }

        if !enabled {
            if let Some(listener) = self.listeners.remove(&key) {
                listener.detach(native);
            }
            return;
        }
        if self.listeners.contains_key(&key) {
            return;
        }
        let kind = view.kind();
        if let Some(listener) =
            crate::events::attach(self.mtm, native, kind, id, event, self.events.clone())
        {
            self.listeners.insert(key, listener);
        }
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view().retain();
        let rect = CGRect {
            origin: CGPoint { x: frame.x as f64, y: frame.y as f64 },
            size: CGSize { width: frame.width as f64, height: frame.height as f64 },
        };
        // `frame` is only meaningful while the transform is the identity. UIKit
        // documents it plainly: with a transform applied the value of `frame`
        // is undefined and must not be set, because it is *derived* from
        // `bounds`, `center` and the transform together. Assigning it anyway
        // makes UIKit solve for a centre using the transformed size, and the
        // view ends up somewhere it was never asked to be.
        //
        // A carousel is where this shows: the row carries a `translateX`, the
        // next commit sets its frame, and from then on it is drawn from a
        // corrupted origin — the page will not move again in either direction.
        // So a transformed view is positioned the way UIKit expects, through
        // `bounds` and `center`, and the transform is left alone.
        if self.transforms.get(&id).is_some_and(|t| !t.is_identity()) {
            let mut bounds = native.bounds();
            bounds.size = rect.size;
            let centre = CGPoint {
                x: rect.origin.x + rect.size.width / 2.0,
                y: rect.origin.y + rect.size.height / 2.0,
            };
            self.animated(id, move || {
                native.setBounds(bounds);
                native.setCenter(centre);
            });
        } else {
            self.animated(id, move || native.setFrame(rect));
        }
        if self.safe_area.contains_key(&id) {
            self.report_safe_area(id);
        }
        // A mask of uneven corners does not stretch with the view: it has to
        // be redrawn at the new size.
        if self
            .corners
            .get(&id)
            .is_some_and(|radii| radii.iter().any(|r| (*r - radii[0]).abs() > f64::EPSILON))
        {
            self.apply_corners(id);
        }
    }

    fn clear(&mut self) {
        for view in self.views.values() {
            view.as_view().removeFromSuperview();
        }
        self.views.clear();
        self.fonts.clear();
        self.corners.clear();
        self.listeners.clear();
    }

    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        if let Some(HostView::Scroll(scroll)) = self.views.get(&id) {
            // Never wider than the scroll view itself. The core already clamps
            // this, and it is clamped again here because the consequence is out
            // of all proportion to the mistake: a `UIScrollView` scrolls on
            // whichever axis its content is bigger, so a content width a few
            // points over -- one image reporting its intrinsic size into a row
            // -- turns the whole page into something that can be dragged
            // sideways until the screen is empty. This engine overflows
            // downwards only, and that is enforced where the scrolling happens.
            let bounds = scroll.bounds().size.width;
            let width = if bounds > 0.0 { (width as f64).min(bounds) } else { width as f64 };
            scroll.setContentSize(CGSize { width, height: height as f64 });
        }
    }

    fn set_clip(&mut self, id: NodeId, clip: bool) {
        self.clips.insert(id, clip);
        let radii = self.corners.get(&id).copied().unwrap_or([0.0; 4]);
        let rounded = radii.iter().any(|r| *r > 0.0);
        if let Some(view) = self.views.get(&id) {
            view.as_view().setClipsToBounds(clip || rounded);
        }
    }

    fn flush(&mut self) {
        // The keyboard is asked about here and not in `set_layout`, which is
        // where the notch is asked about, because it moves while nothing else
        // does: no layout runs between the keyboard leaving the bottom of the
        // screen and arriving at its height, so `set_layout` never comes round
        // to ask. The frame does, sixty times a second, and that is what makes
        // the form travel *with* the keyboard instead of after it.
        //
        // It costs one comparison per subscribed node on a settled frame:
        // `report_safe_area` sends nothing when nothing moved.
        for id in self.safe_area.keys().copied().collect::<Vec<_>>() {
            self.report_safe_area(id);
        }

        // Ask again for whatever ought to be playing to play.
        //
        // `play()` on a player that has not loaded anything yet does not
        // catch: the `rate` stays at zero and there it stays for good, with no
        // error and the layer black. Since the `playing` prop arrives only
        // once, it has to be retried until it takes hold.
        for (id, (player, _)) in &self.videos {
            if self.video_playing.contains(id) && player.rate() == 0.0 && player.status() == 1 {
                player.play();
            }
        }

        // The player's view brought up to the size of its own.
        //
        // Doing it in `set_layout` is not enough: the player is created when
        // the video's URL arrives, which is *after* the frame has been set, so
        // it comes into being zero wide and nobody touches it again.
        for (id, (_, controller)) in &self.videos {
            let Some(view) = self.views.get(id) else { continue };
            let Some(inner) = controller.view() else { continue };
            let bounds = view.as_view().bounds();
            if inner.frame().size != bounds.size {
                inner.setFrame(bounds);
            }
        }
        self.run_stack_animations();
        for id in std::mem::take(&mut self.dirty_alerts) {
            let Some(mut state) = self.alerts.remove(&id) else { continue };
            state.sync(self.mtm, &self.container, id, &self.events);
            self.alerts.insert(id, state);
        }
        // Presenting has to come after the layout: the controller takes the
        // view exactly as it is, and if the frame is not worked out yet what
        // gets presented is an empty box.
        for id in std::mem::take(&mut self.dirty_modals) {
            let Some(mut state) = self.modals.remove(&id) else { continue };
            if let Some(content) = self.views.get(&id).map(|v| v.as_view().retain()) {
                state.sync(self.mtm, &self.container, &content, id, &self.events);
            }
            self.modals.insert(id, state);
        }
    }

    fn set_root(&mut self, id: NodeId) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        if native.superview().is_none() {
            self.container.addSubview(native);
        }
    }
}
