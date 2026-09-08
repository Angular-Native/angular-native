//! `HostRenderer` over AppKit. One native view per mountable node, placed with
//! a direct `frame`: taffy has already worked the layout out, and bringing Auto
//! Layout in here would only set a second layout engine competing with the
//! first.
//!
//! It is the same approach as the iOS host's. What differs is where it shows,
//! and it is commented there: the coordinate origin (`flipped.rs`), the mouse
//! gestures (`events.rs`), and the fact that there is no `_ => {}` at the end
//! of the props `match` here.
//!
//! **Why there is no `_ => {}`.** A prop a host does not look at raises no
//! error, leaves no trace and changes nothing: it is exactly the failure this
//! project is after. The iOS and Android hosts are covered by
//! `scripts/check-wrapper.sh`, which checks that the name appears in the file.
//! Here, on top of that, what is not applied is said through the error output
//! the first time it arrives, with the reason: `IGNORED` is the list of props
//! macOS cannot honour, and anything that is neither implemented nor in that
//! list comes out on screen as an "unknown prop".

use std::collections::{HashMap, HashSet};

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{EventQueue, HostRenderer};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, Message};
use objc2_app_kit::{
    NSAnimationContext, NSBezelStyle, NSButton, NSControl, NSControlStateValueOff,
    NSControlStateValueOn, NSDatePicker, NSDatePickerMode, NSFont, NSForegroundColorAttributeName,
    NSAutoresizingMaskOptions, NSImageView, NSKernAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSPopUpButton, NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSScrollerStyle,
    NSSearchField, NSSegmentedControl, NSSlider, NSStepper, NSStrikethroughStyleAttributeName,
    NSSwitch, NSTextAlignment, NSTextField, NSTextView, NSUnderlineStyleAttributeName,
    NSUserInterfaceItemIdentification, NSView,
};
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::CGMutablePath;
use objc2_foundation::{
    NSAttributedString, NSMutableAttributedString, NSNumber, NSRange, NSString,
};

use crate::flipped::FlippedView;
use crate::support::{is_known_event, unsupported_event};

/// A JSON list of strings, without pulling in a whole parser for it. Identical
/// to the iOS host's, and for the same reason: it only has to understand what
/// the JS side generates.
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

/// Props that reach this host and that macOS cannot honour, with the reason.
///
/// Being in this list is a decision, not an oversight: the prop is knowingly
/// dropped and without noise. Whatever is neither in here nor implemented goes
/// out through the error output, which is what keeps the list from falling
/// behind.
const IGNORED: &[(&str, &str)] = &[
    // Angular writes it on the app's root element; it comes out of no template
    // and no primitive.
    ("ng-version", "Angular writes it on its root, it is no primitive's prop"),
    // The core consumes them to reserve the room for an image; they never
    // become view props on any host.
    ("intrinsicWidth", "the layout consumes it, not the host"),
    ("intrinsicHeight", "the layout consumes it, not the host"),
    // There is no on-screen keyboard on a Mac, so there is nothing to
    // configure: the keyboard is hardware and the view does not pick it.
    ("keyboardType", "a Mac has no on-screen keyboard"),
    ("returnKeyType", "a Mac has no on-screen keyboard"),
    ("autoCapitalize", "a Mac has no on-screen keyboard"),
    ("autoCorrect", "correction on macOS is a system setting, not a view's"),
    // `NSSecureTextField` is a different class, and a view cannot change class
    // once it is created. The prop arrives after the field has been made.
    (
        "secureTextEntry",
        "in AppKit the password field is a different class (NSSecureTextField) and cannot be \
         changed on the fly",
    ),
    // `NSSlider` tints the travelled stretch and nothing else.
    ("thumbColor", "NSSlider does not expose the thumb's colour"),
    ("maximumTrackColor", "NSSlider only tints the travelled stretch"),
    // Pull to refresh is a finger gesture. On the desktop you reload with a
    // button or with ⌘R, which are the app's business.
    ("refreshing", "there is no pull-to-refresh on the desktop"),
    ("bounces", "NSScrollView does not bounce at the end the way iOS's does"),
    // The screen stack is mounted and unmounted, but not animated: the
    // transitions of `an-native-stack` are not ported to this host.
    ("transition", "the stack transitions are not ported to this host yet"),
    // A Mac's header is the window's title bar, and that is where the `[title]`
    // ends up (see `apply_window_title`). What a title bar does not have is a
    // back button: a Mac goes back through the menu or through a button of the
    // app's, not through an arrow in the header.
    ("backTitle", "a macOS title bar carries no back button"),
    ("showsBack", "a macOS title bar carries no back button"),
    // This host's modal is a layer above the content, not a sheet and not a
    // panel: there are no two presentations to choose between.
    ("presentation", "this host's modal is a layer, there is no sheet to choose"),
    // `NSSegmentedControl` tints the whole control alike.
    ("unselectedColor", "NSSegmentedControl does not tint the unselected segments separately"),
];

fn ignored_reason(key: &str) -> Option<&'static str> {
    IGNORED.iter().find(|(k, _)| *k == key).map(|(_, reason)| *reason)
}

/// A node's native view, with its concrete type: a label's props are not
/// applied the way a box's are.
enum HostView {
    View(Retained<FlippedView>),
    Stack(Retained<FlippedView>),
    Label(Retained<NSTextField>),
    Field(Retained<NSTextField>),
    Area(Retained<NSTextView>),
    Image(Retained<NSImageView>),
    Icon(Retained<NSImageView>),
    Scroll(Retained<NSScrollView>),
    Button(Retained<NSButton>),
    Toggle(Retained<NSSwitch>),
    Slide(Retained<NSSlider>),
    Spinner(Retained<NSProgressIndicator>),
    Progress(Retained<NSProgressIndicator>),
    Segments(Retained<NSSegmentedControl>),
    /// macOS's tab bar is a segmented control (see `support.rs`).
    Tabs(Retained<NSSegmentedControl>),
    Step(Retained<NSStepper>),
    Search(Retained<NSSearchField>),
    Menu(Retained<NSPopUpButton>),
    Date(Retained<NSDatePicker>),
    Web(Retained<crate::web::WKWebView>),
    Map(Retained<crate::map::MKMapView>),
    /// In AppKit video really is a view: `AVPlayerView` inherits from
    /// `NSView` and brings the system's controls. In UIKit there is none,
    /// which is why over there a whole controller has to be contained.
    Video(Retained<crate::video::AVPlayerView>),
    /// The navigation header is not drawn: the `[title]` goes to the window's
    /// title bar. The view exists so the tree has something to hang the node
    /// off, and it measures zero, so it leaves no gap. See `support.rs`.
    Nav(Retained<FlippedView>),
    /// A layer above the root.
    Overlay(Retained<FlippedView>),
    /// A dialog has no view: the system presents it. An empty one is mounted so
    /// the tree has something to hang the node off.
    Dialog(Retained<FlippedView>),
}

impl HostView {
    fn as_view(&self) -> &NSView {
        match self {
            HostView::View(v)
            | HostView::Stack(v)
            | HostView::Overlay(v)
            | HostView::Dialog(v)
            | HostView::Nav(v) => v,
            HostView::Label(v) | HostView::Field(v) => v,
            HostView::Area(v) => v,
            HostView::Image(v) | HostView::Icon(v) => v,
            HostView::Scroll(v) => v,
            HostView::Button(v) => v,
            HostView::Toggle(v) => v,
            HostView::Slide(v) => v,
            HostView::Spinner(v) | HostView::Progress(v) => v,
            HostView::Segments(v) | HostView::Tabs(v) => v,
            HostView::Step(v) => v,
            HostView::Search(v) => v,
            HostView::Menu(v) => v,
            HostView::Date(v) => v,
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
            HostView::Field(_) => NodeKind::TextInput,
            HostView::Area(_) => NodeKind::TextEditor,
            HostView::Image(_) => NodeKind::Image,
            HostView::Icon(_) => NodeKind::Icon,
            HostView::Scroll(_) => NodeKind::ScrollView,
            HostView::Button(_) => NodeKind::Button,
            HostView::Toggle(_) => NodeKind::Switch,
            HostView::Slide(_) => NodeKind::Slider,
            HostView::Spinner(_) => NodeKind::ActivityIndicator,
            HostView::Progress(_) => NodeKind::ProgressBar,
            HostView::Segments(_) => NodeKind::SegmentedControl,
            HostView::Tabs(_) => NodeKind::TabBar,
            HostView::Step(_) => NodeKind::Stepper,
            HostView::Search(_) => NodeKind::SearchBar,
            HostView::Menu(_) => NodeKind::Picker,
            HostView::Date(_) => NodeKind::DatePicker,
            HostView::Web(_) => NodeKind::WebView,
            HostView::Map(_) => NodeKind::MapView,
            HostView::Video(_) => NodeKind::VideoView,
            HostView::Nav(_) => NodeKind::NavigationBar,
            HostView::Overlay(_) => NodeKind::Modal,
            HostView::Dialog(_) => NodeKind::Alert,
        }
    }

    /// Where this view's children go. For nearly all of them it is the view
    /// itself; an `NSScrollView` is the exception: its children hang off the
    /// document view, not off the frame that shows it.
    fn content_view(&self) -> Retained<NSView> {
        match self {
            HostView::Scroll(scroll) => unsafe { scroll.documentView() }
                .unwrap_or_else(|| self.as_view().retain()),
            _ => self.as_view().retain(),
        }
    }
}

/// A transform's parts, uncomposed. The scale starts at 1 and not at 0: a view
/// with no `scale` has to look the way it did before the prop existed, not
/// disappear.
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
        Transform { translate_x: 0.0, translate_y: 0.0, scale_x: 1.0, scale_y: 1.0, rotate: 0.0 }
    }
}

impl Transform {
    fn is_identity(&self) -> bool {
        self.scale_x == 1.0 && self.scale_y == 1.0 && self.rotate == 0.0
    }

    /// Scale and rotate, in that order. The translation does not go in here:
    /// it is applied by adding it to the frame in `set_layout`, which in
    /// AppKit is exact and saves having to compensate for the layer's anchor
    /// point twice.
    fn matrix(&self) -> CGAffineTransform {
        let (sin, cos) = self.rotate.sin_cos();
        CGAffineTransform {
            a: self.scale_x * cos,
            b: self.scale_x * sin,
            c: -self.scale_y * sin,
            d: self.scale_y * cos,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

/// How a view animates its changes.
#[derive(Clone, Copy, Default)]
struct Animation {
    /// Seconds. Zero switches the animation off without wiping the rest of the
    /// settings.
    duration: f64,
    delay: f64,
}

pub struct AppKitHost {
    mtm: MainThreadMarker,
    /// The view the shell hands over. The root of the tree hangs off it.
    container: Retained<NSView>,
    views: HashMap<NodeId, HostView>,
    fonts: HashMap<NodeId, an_layout::FontSpec>,
    /// Radii per corner: top-left, top-right, bottom-right, bottom-left.
    corners: HashMap<NodeId, [f64; 4]>,
    /// The frame the core sent, without the translation. It has to be kept
    /// because `translateX` may arrive after the frame and the view then has
    /// to be placed again.
    frames: HashMap<NodeId, Rect>,
    transforms: HashMap<NodeId, Transform>,
    animations: HashMap<NodeId, Animation>,
    /// Each node's icon name, size and weight, which arrive separately.
    icons: HashMap<NodeId, (String, f32, u16)>,
    /// Each tab bar's and each segmented control's titles and icons.
    segments: HashMap<NodeId, (Vec<String>, Vec<String>)>,
    /// Each label's underline or strikethrough.
    decorations: HashMap<NodeId, String>,
    placeholders: HashMap<NodeId, String>,
    placeholder_colors: HashMap<NodeId, String>,
    /// Each button's title, colour, variant and icon: changing any one of them
    /// means redoing all four, just as on iOS.
    button_titles: HashMap<NodeId, String>,
    button_colors: HashMap<NodeId, String>,
    button_variants: HashMap<NodeId, String>,
    button_icons: HashMap<NodeId, (String, String)>,
    /// The value asked of the slider and of the `Stepper`. They are kept
    /// because the value and the range arrive as separate props and in any
    /// order: setting the value before the maximum clamps it against the old
    /// range.
    slider_values: HashMap<NodeId, f64>,
    stepper_values: HashMap<NodeId, f64>,
    alerts: HashMap<NodeId, crate::alert::AlertState>,
    dirty_alerts: Vec<NodeId>,
    /// Each map's centre and zoom. They go together because MapKit has not
    /// three properties but one region, and the three props arrive separately.
    maps: HashMap<NodeId, (f64, f64, f64)>,
    /// Each `<an-video-view>`'s player. It comes into being with the URL,
    /// which arrives after the view.
    videos: HashMap<NodeId, Retained<crate::video::AVPlayer>>,
    /// The ones that ought to be playing. See `flush`.
    video_playing: HashSet<NodeId>,
    /// The title the last mounted `<an-navigation-bar>` asked for, and the one
    /// the window had before any of them asked.
    window_title: Option<String>,
    original_title: Option<String>,
    dirty_title: bool,
    /// The cursor area of each view that asked for one. The owner is kept
    /// because `NSTrackingArea` references it weakly.
    cursors: HashMap<
        NodeId,
        (
            Retained<objc2_app_kit::NSTrackingArea>,
            Retained<crate::events::CursorTarget>,
        ),
    >,
    listeners: HashMap<(NodeId, String), crate::events::AttachedListener>,
    /// The role AppKit gave each view and the state the template asked for.
    /// See `accessibility.rs`.
    accessibility: crate::accessibility::Accessibility,
    /// What has already been warned about, so as not to repeat it sixty times
    /// a second.
    warned: HashSet<String>,
    events: EventQueue,
}

impl AppKitHost {
    /// # Safety
    /// `container` has to be a live `NSView` and this has to be called from
    /// the main thread.
    pub fn new(mtm: MainThreadMarker, container: Retained<NSView>, events: EventQueue) -> Self {
        AppKitHost {
            mtm,
            container,
            views: HashMap::new(),
            fonts: HashMap::new(),
            corners: HashMap::new(),
            frames: HashMap::new(),
            transforms: HashMap::new(),
            animations: HashMap::new(),
            icons: HashMap::new(),
            segments: HashMap::new(),
            decorations: HashMap::new(),
            placeholders: HashMap::new(),
            placeholder_colors: HashMap::new(),
            button_titles: HashMap::new(),
            button_colors: HashMap::new(),
            button_variants: HashMap::new(),
            button_icons: HashMap::new(),
            slider_values: HashMap::new(),
            stepper_values: HashMap::new(),
            alerts: HashMap::new(),
            dirty_alerts: Vec::new(),
            maps: HashMap::new(),
            videos: HashMap::new(),
            video_playing: HashSet::new(),
            window_title: None,
            original_title: None,
            dirty_title: false,
            cursors: HashMap::new(),
            listeners: HashMap::new(),
            accessibility: crate::accessibility::Accessibility::new(),
            warned: HashSet::new(),
            events,
        }
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    /// Warns once and keeps quiet thereafter. The core sends the same prop on
    /// every change, so without this one warning would be thousands of lines.
    fn warn_once(&mut self, key: String, message: impl FnOnce()) {
        if self.warned.insert(key) {
            message();
        }
    }

    /// The `NSFont` a `FontSpec` asks for.
    fn build_font(&self, spec: &an_layout::FontSpec) -> Retained<NSFont> {
        crate::measure::AppKitMeasurer::nsfont(spec)
    }

    fn font_mut(&mut self, id: NodeId) -> &mut an_layout::FontSpec {
        self.fonts.entry(id).or_default()
    }

    fn apply_font(&mut self, id: NodeId) {
        let Some(spec) = self.fonts.get(&id).cloned() else { return };
        let font = self.build_font(&spec);
        let Some(view) = self.views.get(&id) else { return };
        match view {
            HostView::Label(label) | HostView::Field(label) => unsafe {
                label.setFont(Some(&font));
                // Zero means "as many as it takes", just as in UIKit.
                label.setMaximumNumberOfLines(spec.max_lines.unwrap_or(0) as isize);
            },
            HostView::Area(area) => unsafe { area.setFont(Some(&font)) },
            HostView::Search(search) => unsafe { search.setFont(Some(&font)) },
            HostView::Button(_) => {
                self.refresh_button(id);
                return;
            }
            _ => return,
        }
        self.apply_text_attributes(id);
    }

    /// Line height, letter spacing and underline, which `NSTextField` does not
    /// have as properties. The core already measures with the first two, so
    /// without this the layout would reserve room the text does not fill.
    fn apply_text_attributes(&self, id: NodeId) {
        let Some(HostView::Label(label)) = self.views.get(&id) else { return };
        let spec = self.fonts.get(&id);
        let decoration = self.decorations.get(&id).map(String::as_str).unwrap_or("none");
        let spacing = spec.map(|s| s.letter_spacing).unwrap_or(0.0);
        let line_height = spec.and_then(|s| s.line_height);
        if spacing == 0.0 && line_height.is_none() && decoration == "none" {
            return;
        }

        let text = unsafe { label.stringValue() };
        let attributed = unsafe {
            NSMutableAttributedString::initWithString(
                NSMutableAttributedString::alloc(),
                &text,
            )
        };
        let range = NSRange { location: 0, length: text.len_utf16() };
        unsafe {
            if spacing != 0.0 {
                attributed.addAttribute_value_range(
                    NSKernAttributeName,
                    &NSNumber::new_f64(spacing as f64),
                    range,
                );
            }
            if let Some(height) = line_height {
                let style = NSMutableParagraphStyle::new();
                style.setMinimumLineHeight(height as f64);
                style.setMaximumLineHeight(height as f64);
                // Without this, long text stops wrapping the moment a
                // paragraph style is put on it: a fresh style's default mode
                // is to clip, not to wrap.
                style.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByWordWrapping);
                style.setAlignment(label.alignment());
                attributed.addAttribute_value_range(
                    NSParagraphStyleAttributeName,
                    &style,
                    range,
                );
            }
            let underline = match decoration {
                "underline" => Some(NSUnderlineStyleAttributeName),
                "line-through" => Some(NSStrikethroughStyleAttributeName),
                _ => None,
            };
            if let Some(key) = underline {
                attributed.addAttribute_value_range(key, &NSNumber::new_i64(1), range);
            }
            // The font and the colour are left alone: with neither of them in
            // the attributes, `NSTextField` goes on using its own and
            // `[color]`/`[fontSize]` work as before.
            label.setAttributedStringValue(&attributed);
        }
    }

    /// Puts the placeholder text back with its colour. With no colour it goes
    /// in plain: an attributed string with no attributes draws differently
    /// from the one AppKit puts there.
    fn apply_placeholder(&self, id: NodeId) {
        let field = match self.views.get(&id) {
            Some(HostView::Field(field)) => &**field,
            Some(HostView::Search(search)) => &***search,
            _ => return,
        };
        let Some(placeholder) = self.placeholders.get(&id) else { return };
        let string = NSString::from_str(placeholder);
        let Some(color) =
            self.placeholder_colors.get(&id).and_then(|raw| crate::color::to_nscolor(raw))
        else {
            unsafe { field.setPlaceholderString(Some(&string)) };
            return;
        };
        let attrs = objc2_foundation::NSDictionary::from_slices(
            &[unsafe { NSForegroundColorAttributeName }],
            &[&*color as &objc2::runtime::AnyObject],
        );
        // SAFETY: the dictionary carries an `NSColor` under the colour key,
        // which is the type that attribute expects.
        let attributed = unsafe { NSAttributedString::new_with_attributes(&string, &attrs) };
        unsafe { field.setPlaceholderAttributedString(Some(&attributed)) };
    }

    /// The view of this host's that can catch a node's swipe.
    ///
    /// Nearly always it is the node's own. The exception is the
    /// `<an-scroll-view>`: its view is a system `NSScrollView`, and ours is
    /// the document view inside it, which is also the one covering all of the
    /// scrollable content.
    ///
    /// The kinds here are the same ones `support::catches_swipe` names, and
    /// they have to be: that function is what decides whether a warning is
    /// issued, so a primitive it passed and this one failed to find would end
    /// up with neither a subscription nor a warning. The caller checks it.
    fn swipe_view(&self, id: NodeId) -> Option<Retained<FlippedView>> {
        match self.views.get(&id)? {
            HostView::View(view) | HostView::Stack(view) | HostView::Overlay(view) => {
                Some(view.retain())
            }
            HostView::Scroll(scroll) => unsafe { scroll.documentView() }
                .and_then(|document| document.downcast::<FlippedView>().ok()),
            _ => None,
        }
    }

    /// Writes what an `<an-navigation-bar>` asked for into the window's title
    /// bar.
    ///
    /// This is what macOS puts in place of a header inside the content, and it
    /// is no workaround: on a Mac the title of the screen you are on lives up
    /// there, in the window's bar, and drawing another one below it would make
    /// two. See `support.rs`.
    ///
    /// It is called from `flush` and not from `set_prop` because when the prop
    /// arrives the view may not be inside a window yet; while it is not, the
    /// request stays pending and is retried on the next frame.
    fn apply_window_title(&mut self) {
        let Some(window) = self.container.window() else { return };
        // What the window came with, so it can be given back when the screen
        // that asked for the title is unmounted.
        if self.original_title.is_none() {
            self.original_title = Some(window.title().to_string());
        }
        let title = self
            .window_title
            .clone()
            .or_else(|| self.original_title.clone())
            .unwrap_or_default();
        window.setTitle(&NSString::from_str(&title));
        self.dirty_title = false;
    }

    /// Runs a change inside an animation if the node asked for one.
    ///
    /// `allowsImplicitAnimation` is what makes the ordinary setters animate:
    /// in AppKit the usual way is to write through `view.animator()`, but that
    /// requires having the concrete type's proxy in every place. With the
    /// group open and implicit animations switched on, a plain `setFrame` is
    /// already animated, and the same closure works for every control.
    fn animated(&self, id: NodeId, change: impl Fn()) {
        let animation = self.animations.get(&id).copied().unwrap_or_default();
        if animation.duration <= 0.0 {
            change();
            return;
        }
        // `NSAnimationContext` has no delay: it is obtained by putting the
        // group on the main queue later. Since the closure cannot cross over
        // there without being `'static`, a requested delay is applied as total
        // duration and that gets said.
        let duration = animation.duration + animation.delay;
        let block = RcBlock::new(move |context: core::ptr::NonNull<NSAnimationContext>| {
            let context = unsafe { context.as_ref() };
            unsafe {
                context.setDuration(duration);
                context.setAllowsImplicitAnimation(true);
            }
            change();
        });
        unsafe { NSAnimationContext::runAnimationGroup(&block) };
    }

    /// Rebuilds the whole button: title, colour, variant and icon arrive
    /// separately, and changing the variant takes the rest down with it.
    fn refresh_button(&self, id: NodeId) {
        let Some(HostView::Button(button)) = self.views.get(&id) else { return };
        let title = self.button_titles.get(&id).cloned().unwrap_or_default();
        let variant = self.button_variants.get(&id).map(String::as_str).unwrap_or("text");
        let color = self.button_colors.get(&id).and_then(|raw| crate::color::to_nscolor(raw));

        unsafe {
            button.setTitle(&NSString::from_str(&title));
            button.setBezelStyle(NSBezelStyle::Push);
            if let Some(spec) = self.fonts.get(&id) {
                button.setFont(Some(&self.build_font(spec)));
            }
            match variant {
                // No border: it is macOS's text-only button, the one used in
                // bars and in a sheet's links.
                "text" => {
                    button.setBordered(false);
                    button.setBezelColor(None);
                    button.setContentTintColor(color.as_deref());
                }
                // Filled: the colour goes on the bezel and the label is
                // painted in whatever contrasts, which the core works out.
                "filled" => {
                    button.setBordered(true);
                    button.setBezelColor(color.as_deref());
                    let contrast = self
                        .button_colors
                        .get(&id)
                        .and_then(|raw| crate::color::contrasting(raw));
                    button.setContentTintColor(contrast.as_deref());
                }
                // Tonal: the same colour, dimmed. `NSColor` knows how to
                // blend with the background, so there is no second shade to
                // invent.
                "tonal" => {
                    button.setBordered(true);
                    let tinted = color.as_ref().map(|c| {
                        c.colorWithAlphaComponent(0.25)
                    });
                    button.setBezelColor(tinted.as_deref());
                    button.setContentTintColor(color.as_deref());
                }
                // Outlined: the system's border and a label in the colour
                // asked for. AppKit does not let the bezel's edge be painted
                // separately, so the outline is the standard one and the
                // colour goes on the text.
                _ => {
                    button.setBordered(true);
                    button.setBezelColor(None);
                    button.setContentTintColor(color.as_deref());
                }
            }

            match self.button_icons.get(&id) {
                Some((name, position)) if !name.is_empty() => {
                    let size = self.fonts.get(&id).map(|f| f.size).unwrap_or(0.0);
                    button.setImage(crate::icons::symbol(name, size, 400).as_deref());
                    button.setImagePosition(if position == "trailing" {
                        objc2_app_kit::NSCellImagePosition::ImageTrailing
                    } else {
                        objc2_app_kit::NSCellImagePosition::ImageLeading
                    });
                }
                _ => button.setImage(None),
            }
        }
    }

    fn set_corner(&mut self, id: NodeId, corner: usize, radius: Option<f32>) {
        let entry = self.corners.entry(id).or_insert([0.0; 4]);
        entry[corner] = radius.unwrap_or(0.0) as f64;
        self.apply_corners(id);
    }

    /// Rounds the corners.
    ///
    /// Three cases, and the cheapest one that will do is the one taken.
    ///
    /// A `CALayer` has **one** radius, so all four the same is the whole of it
    /// and costs nothing. Several the same plus some at zero is still one
    /// radius: `maskedCorners` says which of them carry it, which covers the
    /// common shape of a card rounded along its top.
    ///
    /// What a layer cannot do is two different radii at once, and until now
    /// that was rounded with the largest and merely said out loud. It is drawn
    /// now, the way the iOS host draws it: the outline goes into a
    /// `CAShapeLayer` used as the layer's mask. That costs a layer per view
    /// that asks for it and nothing at all for every view that does not, and it
    /// has to be redrawn on every resize — a mask does not stretch — which is
    /// why `place` comes back here.
    fn apply_corners(&mut self, id: NodeId) {
        let Some(radii) = self.corners.get(&id).copied() else { return };
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        native.setWantsLayer(true);
        let Some(layer) = (unsafe { native.layer() }) else { return };

        let max = radii.iter().cloned().fold(0.0_f64, f64::max);
        // How many distinct radii there are once the corners that are simply
        // square are set aside. One means a layer can do it; more means it
        // cannot.
        let rounded: Vec<f64> = radii.iter().copied().filter(|r| *r > 0.0).collect();
        let several = rounded
            .iter()
            .any(|r| (*r - rounded[0]).abs() > f64::EPSILON);

        if several {
            let bounds = native.bounds();
            if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
                // No size yet. The frame will arrive and `place` calls back.
                return;
            }
            layer.setCornerRadius(0.0);
            let shape = objc2_quartz_core::CAShapeLayer::new();
            let path = rounded_path(bounds.size.width, bounds.size.height, radii);
            unsafe { shape.setPath(Some(&path)) };
            unsafe { layer.setMask(Some(&shape)) };
            layer.setMasksToBounds(true);
            return;
        }

        // Back to no mask, in case this node had one a moment ago: a template
        // can go from four different radii to one, and a stale mask would
        // outlive the shape that asked for it.
        unsafe { layer.setMask(None) };
        layer.setCornerRadius(max);
        if radii.iter().any(|r| *r <= 0.0) && max > 0.0 {
            use objc2_quartz_core::CACornerMask;
            let mut mask = CACornerMask::empty();
            // The core's order is top-left, top-right, bottom-right,
            // bottom-left, and the layer's is in *unflipped* coordinates: what
            // the layer calls "MinY" is the top of a flipped view.
            if radii[0] > 0.0 {
                mask |= CACornerMask::LayerMinXMinYCorner;
            }
            if radii[1] > 0.0 {
                mask |= CACornerMask::LayerMaxXMinYCorner;
            }
            if radii[2] > 0.0 {
                mask |= CACornerMask::LayerMaxXMaxYCorner;
            }
            if radii[3] > 0.0 {
                mask |= CACornerMask::LayerMinXMaxYCorner;
            }
            layer.setMaskedCorners(mask);
        } else {
            use objc2_quartz_core::CACornerMask;
            layer.setMaskedCorners(
                CACornerMask::LayerMinXMinYCorner
                    | CACornerMask::LayerMaxXMinYCorner
                    | CACornerMask::LayerMaxXMaxYCorner
                    | CACornerMask::LayerMinXMaxYCorner,
            );
        }
        layer.setMasksToBounds(max > 0.0);
    }

    /// The core's frame plus the translation asked for, which here is added
    /// rather than routed through the matrix: in AppKit that is exact and
    /// saves having to compensate for the layer's anchor point.
    fn place(&self, id: NodeId) {
        let (Some(view), Some(frame)) = (self.views.get(&id), self.frames.get(&id)) else {
            return;
        };
        let transform = self.transforms.get(&id).copied().unwrap_or_default();
        let native = view.as_view().retain();
        let rect = CGRect {
            origin: CGPoint {
                x: frame.x as f64 + transform.translate_x,
                y: frame.y as f64 + transform.translate_y,
            },
            size: CGSize { width: frame.width as f64, height: frame.height as f64 },
        };
        self.animated(id, || native.setFrame(rect));

        if transform.is_identity() {
            return;
        }
        // Scale and rotation do go through the layer. The anchor is put at
        // the centre so it turns about itself and not about its corner; the
        // layer's `setFrame` derives the position from the anchor, so setting
        // it here throws nothing out of place.
        native.setWantsLayer(true);
        if let Some(layer) = unsafe { native.layer() } {
            layer.setAnchorPoint(CGPoint { x: 0.5, y: 0.5 });
            layer.setAffineTransform(transform.matrix());
        }
    }
}

impl HostRenderer for AppKitHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        let mtm = self.mtm;
        // The `match` carries no wildcard on purpose: if the core adds a
        // `NodeKind`, this file stops compiling and somebody has to decide
        // what macOS does with it. See `support.rs`'s header.
        let view = match kind {
            NodeKind::View => HostView::View(FlippedView::new(mtm)),
            NodeKind::StackView => {
                let stack = FlippedView::new(mtm);
                // Screens on their way in and out spill past the frame.
                stack.clip_to_bounds();
                HostView::Stack(stack)
            }
            NodeKind::Text => {
                let label = NSTextField::new(mtm);
                unsafe {
                    // AppKit has no `NSLabel`: a label is a text field with
                    // no border, no background and no editing. It is what
                    // `NSTextField.labelWithString:` does, written out by hand
                    // because the text arrives later.
                    label.setEditable(false);
                    label.setSelectable(false);
                    label.setBordered(false);
                    label.setBezeled(false);
                    label.setDrawsBackground(false);
                    label.setUsesSingleLineMode(false);
                    label.setMaximumNumberOfLines(0);
                    label.cell().inspect(|cell| cell.setWraps(true));
                    // The default font has to be the one the layout measured
                    // with, or the box comes out narrow and the text is cut
                    // off with nothing raising an error.
                    let default_size = an_layout::FontSpec::default().size as f64;
                    label.setFont(Some(&NSFont::systemFontOfSize(default_size)));
                }
                HostView::Label(label)
            }
            NodeKind::TextInput => {
                let field = NSTextField::new(mtm);
                unsafe { field.setUsesSingleLineMode(true) };
                HostView::Field(field)
            }
            NodeKind::TextEditor => {
                let area = NSTextView::new(mtm);
                unsafe {
                    area.setDrawsBackground(false);
                    area.setTextContainerInset(CGSize { width: 0.0, height: 0.0 });
                }
                HostView::Area(area)
            }
            NodeKind::Image => HostView::Image(NSImageView::new(mtm)),
            NodeKind::Icon => HostView::Icon(NSImageView::new(mtm)),
            NodeKind::ScrollView => {
                let scroll = NSScrollView::new(mtm);
                // The document view is what carries the children, and it is
                // flipped for the same reason as everything else: the core
                // places things from the top down.
                let document = FlippedView::new(mtm);
                unsafe {
                    scroll.setDocumentView(Some(&document));
                    scroll.setDrawsBackground(false);
                    scroll.setHasVerticalScroller(true);
                    // Scrollers that hide themselves are macOS's normal
                    // behaviour since Lion; the legacy style takes up room and
                    // would throw off the layout taffy has already worked
                    // out.
                    scroll.setScrollerStyle(NSScrollerStyle::Overlay);
                }
                HostView::Scroll(scroll)
            }
            NodeKind::Button => {
                let button = NSButton::new(mtm);
                unsafe { button.setBezelStyle(NSBezelStyle::Push) };
                HostView::Button(button)
            }
            NodeKind::Switch => HostView::Toggle(NSSwitch::new(mtm)),
            NodeKind::Slider => HostView::Slide(NSSlider::new(mtm)),
            NodeKind::ActivityIndicator => {
                let spinner = NSProgressIndicator::new(mtm);
                spinner.setStyle(NSProgressIndicatorStyle::Spinning);
                spinner.setDisplayedWhenStopped(false);
                HostView::Spinner(spinner)
            }
            NodeKind::ProgressBar => {
                let bar = NSProgressIndicator::new(mtm);
                bar.setStyle(NSProgressIndicatorStyle::Bar);
                bar.setIndeterminate(false);
                bar.setMinValue(0.0);
                bar.setMaxValue(1.0);
                HostView::Progress(bar)
            }
            NodeKind::SegmentedControl => {
                let segments = NSSegmentedControl::new(mtm);
                unsafe {
                    segments.setSegmentStyle(objc2_app_kit::NSSegmentStyle::Automatic);
                    segments.setTrackingMode(
                        objc2_app_kit::NSSegmentSwitchTracking::SelectOne,
                    );
                }
                HostView::Segments(segments)
            }
            NodeKind::TabBar => {
                let tabs = NSSegmentedControl::new(mtm);
                unsafe {
                    tabs.setSegmentStyle(objc2_app_kit::NSSegmentStyle::Automatic);
                    tabs.setTrackingMode(objc2_app_kit::NSSegmentSwitchTracking::SelectOne);
                }
                HostView::Tabs(tabs)
            }
            NodeKind::Stepper => HostView::Step(NSStepper::new(mtm)),
            NodeKind::SearchBar => HostView::Search(NSSearchField::new(mtm)),
            NodeKind::Picker => {
                // macOS's drop-down does exist as a control: it is an
                // `NSPopUpButton`, and there is no assembling it out of a
                // button and a menu the way iOS needs.
                let menu = NSPopUpButton::new(mtm);
                unsafe { menu.setPullsDown(false) };
                HostView::Menu(menu)
            }
            NodeKind::DatePicker => {
                let picker = NSDatePicker::new(mtm);
                unsafe {
                    picker.setDatePickerElements(
                        objc2_app_kit::NSDatePickerElementFlags::YearMonthDay,
                    );
                    picker.setDatePickerStyle(
                        objc2_app_kit::NSDatePickerStyle::TextFieldAndStepper,
                    );
                }
                HostView::Date(picker)
            }
            NodeKind::WebView => HostView::Web(crate::web::WKWebView::new(mtm)),
            NodeKind::MapView => HostView::Map(crate::map::MKMapView::new(mtm)),
            NodeKind::Custom => {
                // A plain container to begin with. The name arrives as the
                // `an:view` prop in the same frame, and the plugin is asked
                // then: nothing can ask before the tree has said which view.
                HostView::View(FlippedView::new(mtm))
            }
            NodeKind::VideoView => {
                let player = crate::video::AVPlayerView::new(mtm);
                // The system's controls, inside the view. It is what
                // `AVPlayerView` ships with and what lets macOS's video be
                // paused without the app putting a button there.
                player.setControlsStyle(crate::video::CONTROLS_INLINE);
                HostView::Video(player)
            }
            // The header is not drawn here: the `[title]` ends up in the
            // window's title bar, which is a Mac's header. See `support.rs`
            // and `apply_window_title`.
            NodeKind::NavigationBar => HostView::Nav(FlippedView::new(mtm)),
            NodeKind::Alert => {
                let placeholder = FlippedView::new(mtm);
                placeholder.setHidden(true);
                self.alerts.insert(id, crate::alert::AlertState::default());
                HostView::Dialog(placeholder)
            }
            NodeKind::Modal => {
                let overlay = FlippedView::new(mtm);
                overlay.setHidden(true);
                HostView::Overlay(overlay)
            }
            // Never arrives: the core sends no `Create` for a raw text node.
            NodeKind::RawText => return,
        };
        self.views.insert(id, view);
    }

    fn destroy(&mut self, id: NodeId) {
        if let Some(view) = self.views.remove(&id) {
            // The screen that set the title is going away: the window gets
            // its own back. Without this, closing a screen would leave its
            // name up there for good.
            if view.kind() == NodeKind::NavigationBar {
                self.window_title = None;
                self.dirty_title = true;
            }
            if let Some((area, _)) = self.cursors.remove(&id) {
                view.as_view().removeTrackingArea(&area);
            }
            view.as_view().removeFromSuperview();
        }
        self.fonts.remove(&id);
        self.corners.remove(&id);
        self.frames.remove(&id);
        self.transforms.remove(&id);
        self.animations.remove(&id);
        self.icons.remove(&id);
        self.segments.remove(&id);
        self.decorations.remove(&id);
        self.placeholders.remove(&id);
        self.placeholder_colors.remove(&id);
        self.button_titles.remove(&id);
        self.button_colors.remove(&id);
        self.button_variants.remove(&id);
        self.button_icons.remove(&id);
        self.slider_values.remove(&id);
        self.stepper_values.remove(&id);
        self.alerts.remove(&id);
        self.maps.remove(&id);
        self.videos.remove(&id);
        self.video_playing.remove(&id);
        self.listeners.retain(|(node, _), _| *node != id);
        self.accessibility.forget(id);
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        let (Some(parent_view), Some(child_view)) =
            (self.views.get(&parent), self.views.get(&child))
        else {
            return;
        };
        let host = parent_view.content_view();
        let child_native = child_view.as_view();

        // AppKit has no `insertSubview:atIndex:`: a view is placed above or
        // below a sibling. It is the same thing said differently, because the
        // order of `subviews` is the drawing order.
        let siblings = host.subviews().to_vec();
        let index = index as usize;
        unsafe {
            if index == 0 {
                match siblings.first() {
                    Some(first) => host.addSubview_positioned_relativeTo(
                        child_native,
                        objc2_app_kit::NSWindowOrderingMode::Below,
                        Some(first),
                    ),
                    None => host.addSubview(child_native),
                }
            } else {
                match siblings.get(index - 1) {
                    Some(previous) => host.addSubview_positioned_relativeTo(
                        child_native,
                        objc2_app_kit::NSWindowOrderingMode::Above,
                        Some(previous),
                    ),
                    None => host.addSubview(child_native),
                }
            }
        }
    }

    fn remove(&mut self, _parent: NodeId, child: NodeId) {
        let Some(view) = self.views.get(&child) else { return };
        view.as_view().removeFromSuperview();
    }

    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        let Some(view) = self.views.get(&id) else { return };
        let kind = view.kind();
        let native = view.as_view().retain();
        let text = value.as_str().map(str::to_owned);
        let number = value.as_f32();

        match key {
            // Another platform's props. They travel with their prefix, so
            // this host drops them at a glance without having to know what
            // they are.
            _ if key.starts_with("ios:") || key.starts_with("android:") => {}

            // The name of the view a plugin brings. It arrives on the same
            // frame the node was created on, which is why `create` mounts an
            // empty container: a plugin cannot be asked for a view before the
            // tree has said which one.
            "an:view" => {
                let Some(name) = text.as_deref() else { return };
                let Some(brought) = crate::plugin_views::make(name) else {
                    // Once per name and not once per node: a list of five
                    // hundred rows with the same missing view is one mistake,
                    // not five hundred.
                    let name = name.to_owned();
                    self.warn_once(format!("plugin-view:{name}"), || {
                        eprintln!(
                            "angular-native: no plugin registers a view called {name:?}, so \
                             <an-custom [view]=\"{name}\"> mounts nothing. The name is the one \
                             the plugin passes to AnPluginViews.register."
                        );
                    });
                    return;
                };
                // It fills the node, whose size layout decided: a plugin view
                // is never measured by its content, so the frame is the answer
                // and not a starting point.
                brought.setFrame(native.bounds());
                unsafe {
                    brought.setAutoresizingMask(
                        NSAutoresizingMaskOptions::ViewWidthSizable
                            | NSAutoresizingMaskOptions::ViewHeightSizable,
                    );
                }
                native.addSubview(&brought);
            }

            "backgroundColor" | "background-color" => {
                let color = text.as_deref().and_then(crate::color::to_nscolor);
                match self.views.get(&id) {
                    // A field and a label paint their background through a
                    // property of their own; the layer would draw it
                    // underneath them.
                    Some(HostView::Field(field)) | Some(HostView::Label(field)) => unsafe {
                        field.setDrawsBackground(color.is_some());
                        field.setBackgroundColor(color.as_deref());
                    },
                    _ => {
                        native.setWantsLayer(true);
                        if let Some(layer) = unsafe { native.layer() } {
                            layer.setBackgroundColor(
                                color.as_ref().map(|c| c.CGColor()).as_deref(),
                            );
                        }
                    }
                }
            }
            "opacity" => {
                if let Some(v) = number {
                    let view = native.clone();
                    self.animated(id, move || unsafe { view.setAlphaValue(v as f64) });
                }
            }
            "enabled" => {
                let on = !matches!(value, PropValue::Bool(false));
                let control: *const NSView = &*native;
                match kind {
                    NodeKind::Button
                    | NodeKind::Switch
                    | NodeKind::Slider
                    | NodeKind::SegmentedControl
                    | NodeKind::TabBar
                    | NodeKind::Stepper
                    | NodeKind::Picker
                    | NodeKind::DatePicker
                    | NodeKind::TextInput
                    | NodeKind::SearchBar => unsafe {
                        (*control.cast::<NSControl>()).setEnabled(on)
                    },
                    // What is not a control does not know how to grey
                    // itself out, and AppKit has no `userInteractionEnabled`
                    // the way UIKit does: a view that is not a control cannot
                    // be switched off without imitating it. That gets said
                    // rather than pretending it was switched off.
                    _ => {
                        let _ = on;
                        self.warn_once(format!("enabled:{kind:?}"), || {
                            eprintln!(
                                "angular-native: `enabled` on <{kind:?}> does not apply on \
                                 macOS: AppKit only knows how to switch controls off, not \
                                 arbitrary views"
                            );
                        });
                    }
                }
            }
            "testID" | "accessibilityIdentifier" => {
                if let Some(t) = &text {
                    unsafe { native.setIdentifier(Some(&NSString::from_str(t))) };
                }
            }
            // The six of the contract, all together in `accessibility.rs`:
            // role and state are written through different properties of the
            // NSAccessibility protocol, but `checked` ends up in the value,
            // and that means knowing whether the template set one.
            _ if crate::accessibility::handles(key) => {
                self.accessibility.apply(id, &native, kind, key, value);
            }

            // --- the view's own geometry
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
                if let Some(color) = text.as_deref().and_then(crate::color::to_cgcolor) {
                    native.setWantsLayer(true);
                    if let Some(layer) = unsafe { native.layer() } {
                        layer.setBorderColor(Some(&color));
                    }
                }
            }
            "borderWidth" | "border-width" => {
                if let Some(v) = number {
                    native.setWantsLayer(true);
                    if let Some(layer) = unsafe { native.layer() } {
                        layer.setBorderWidth(v as f64);
                    }
                }
            }

            // --- animation and transforms
            "animate" => {
                self.animations.entry(id).or_default().duration =
                    number.unwrap_or(0.0) as f64 / 1000.0;
            }
            "animateDelay" => {
                let delay = number.unwrap_or(0.0) as f64 / 1000.0;
                self.animations.entry(id).or_default().delay = delay;
                if delay > 0.0 {
                    self.warn_once("animateDelay".to_owned(), || {
                        eprintln!(
                            "angular-native: NSAnimationContext has no delay; on macOS \
                             `animateDelay` is added to the duration"
                        );
                    });
                }
            }
            "animateEasing" => {
                // `NSAnimationContext` takes a Core Animation timing curve,
                // which is not UIKit's four. As long as only those four are
                // asked for, the system's is the one that corresponds to
                // `ease-in-out` and the other three would be got wrong.
                if text.as_deref().is_some_and(|t| t != "ease-in-out") {
                    self.warn_once("animateEasing".to_owned(), || {
                        eprintln!(
                            "angular-native: on macOS the animation curve is the system's; \
                             `animateEasing` does not apply"
                        );
                    });
                }
            }
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
                self.place(id);
            }

            // --- text
            "color" => {
                let Some(color) = text.as_deref().and_then(crate::color::to_nscolor) else {
                    return;
                };
                match self.views.get(&id) {
                    Some(HostView::Label(label)) | Some(HostView::Field(label)) => unsafe {
                        label.setTextColor(Some(&color))
                    },
                    Some(HostView::Search(search)) => unsafe {
                        search.setTextColor(Some(&color))
                    },
                    Some(HostView::Area(area)) => unsafe { area.setTextColor(Some(&color)) },
                    // A symbol is tinted, not recoloured.
                    Some(HostView::Icon(icon)) => unsafe {
                        icon.setContentTintColor(Some(&color))
                    },
                    // `NSSwitch` and `NSProgressIndicator` are not tinted:
                    // they take the accent colour the user picked in Settings,
                    // and AppKit exposes no property to change it per view.
                    // Tinting them by hand —a layer on top, a filter— would be
                    // drawing a control instead of using the system's.
                    Some(HostView::Toggle(_))
                    | Some(HostView::Spinner(_))
                    | Some(HostView::Progress(_)) => {
                        self.warn_once(format!("tint:{kind:?}"), || {
                            eprintln!(
                                "angular-native: `color` on <{kind:?}> does not apply on macOS: \
                                 these controls take the system's accent colour and AppKit does \
                                 not let it be changed per view"
                            );
                        });
                    }
                    Some(HostView::Slide(slider)) => unsafe {
                        slider.setTrackFillColor(Some(&color))
                    },
                    Some(HostView::Segments(segments)) | Some(HostView::Tabs(segments)) => unsafe {
                        segments.setSelectedSegmentBezelColor(Some(&color))
                    },
                    Some(HostView::Button(_)) => {
                        if let Some(raw) = text.as_deref() {
                            self.button_colors.insert(id, raw.to_owned());
                        }
                        self.refresh_button(id);
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
                match self.views.get(&id) {
                    Some(HostView::Label(label)) | Some(HostView::Field(label)) => unsafe {
                        label.setAlignment(alignment)
                    },
                    Some(HostView::Area(area)) => unsafe { area.setAlignment(alignment) },
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
            "numberOfLines" => {
                self.font_mut(id).max_lines = number.filter(|v| *v >= 1.0).map(|v| v as u32);
                self.apply_font(id);
            }
            "lineHeight" | "line-height" => {
                self.font_mut(id).line_height = number;
                self.apply_text_attributes(id);
            }
            "letterSpacing" | "letter-spacing" => {
                self.font_mut(id).letter_spacing = number.unwrap_or(0.0);
                self.apply_text_attributes(id);
            }
            "textDecoration" | "text-decoration" => {
                self.decorations.insert(id, text.clone().unwrap_or_else(|| "none".to_owned()));
                self.apply_text_attributes(id);
            }
            "placeholder" => {
                self.placeholders.insert(id, text.clone().unwrap_or_default());
                self.apply_placeholder(id);
            }
            "placeholderColor" => {
                match &text {
                    Some(color) => self.placeholder_colors.insert(id, color.clone()),
                    None => self.placeholder_colors.remove(&id),
                };
                self.apply_placeholder(id);
            }
            "editable" => {
                let on = !matches!(value, PropValue::Bool(false));
                match self.views.get(&id) {
                    Some(HostView::Field(field)) => unsafe { field.setEditable(on) },
                    Some(HostView::Area(area)) => unsafe { area.setEditable(on) },
                    _ => {}
                }
            }

            // --- the controls' values
            "value" => match self.views.get(&id) {
                Some(HostView::Slide(slider)) => {
                    let Some(v) = number else { return };
                    self.slider_values.insert(id, v as f64);
                    // Only if it differs: writing it while the thumb is being
                    // dragged would fight with the user's mouse.
                    if (unsafe { slider.doubleValue() } - v as f64).abs() > f64::EPSILON {
                        unsafe { slider.setDoubleValue(v as f64) };
                    }
                }
                Some(HostView::Step(stepper)) => {
                    let v = number.unwrap_or(0.0) as f64;
                    self.stepper_values.insert(id, v);
                    unsafe { stepper.setDoubleValue(v) };
                }
                Some(HostView::Date(picker)) => {
                    // It arrives in milliseconds since 1970, which is what
                    // `Date` gives in JS. `NSDate` works in seconds.
                    let seconds = number.unwrap_or(0.0) as f64 / 1000.0;
                    let date =
                        unsafe { objc2_foundation::NSDate::dateWithTimeIntervalSince1970(seconds) };
                    unsafe { picker.setDateValue(&date) };
                }
                // Writing it while the user types would move their caret to
                // the end on every keystroke: only if it really differs.
                Some(HostView::Field(field)) => {
                    let next = text.clone().unwrap_or_default();
                    if unsafe { field.stringValue() }.to_string() != next {
                        unsafe { field.setStringValue(&NSString::from_str(&next)) };
                    }
                }
                Some(HostView::Search(search)) => {
                    let next = text.clone().unwrap_or_default();
                    if unsafe { search.stringValue() }.to_string() != next {
                        unsafe { search.setStringValue(&NSString::from_str(&next)) };
                    }
                }
                Some(HostView::Area(area)) => {
                    let current = unsafe { area.string() }.to_string();
                    let next = text.clone().unwrap_or_default();
                    if current != next {
                        unsafe { area.setString(&NSString::from_str(&next)) };
                    }
                }
                _ => {}
            },
            "on" => {
                if let Some(HostView::Toggle(toggle)) = self.views.get(&id) {
                    unsafe {
                        toggle.setState(if matches!(value, PropValue::Bool(true)) {
                            NSControlStateValueOn
                        } else {
                            NSControlStateValueOff
                        })
                    };
                }
            }
            "minimumValue" | "maximumValue" | "stepValue" => {
                let v = number.unwrap_or(0.0) as f64;
                match self.views.get(&id) {
                    Some(HostView::Slide(slider)) => {
                        unsafe {
                            match key {
                                "minimumValue" => slider.setMinValue(v),
                                "maximumValue" => slider.setMaxValue(v),
                                // `stepValue` is the stepper's alone:
                                // `an-slider` declares no step, so nothing
                                // sends one here. If one ever arrives it is
                                // not applied — a macOS slider snaps only to
                                // tick marks, and tick marks are *drawn*, so
                                // the control would come out notched by a
                                // prop about values.
                                _ => return,
                            }
                        }
                        // The range changed: the value has to be applied
                        // again, since it may have arrived earlier and been
                        // clamped.
                        if let Some(wanted) = self.slider_values.get(&id).copied() {
                            unsafe { slider.setDoubleValue(wanted) };
                        }
                    }
                    Some(HostView::Step(stepper)) => {
                        unsafe {
                            match key {
                                "minimumValue" => stepper.setMinValue(v),
                                "maximumValue" => stepper.setMaxValue(v),
                                _ => stepper.setIncrement(if v > 0.0 { v } else { 1.0 }),
                            }
                        }
                        if let Some(wanted) = self.stepper_values.get(&id).copied() {
                            unsafe { stepper.setDoubleValue(wanted) };
                        }
                    }
                    _ => {}
                }
            }
            "minimumTrackColor" => {
                if let (Some(HostView::Slide(slider)), Some(color)) =
                    (self.views.get(&id), text.as_deref().and_then(crate::color::to_nscolor))
                {
                    unsafe { slider.setTrackFillColor(Some(&color)) };
                }
            }
            "animating" => {
                if let Some(HostView::Spinner(spinner)) = self.views.get(&id) {
                    if matches!(value, PropValue::Bool(false)) {
                        unsafe { spinner.stopAnimation(None) };
                    } else {
                        unsafe { spinner.startAnimation(None) };
                    }
                }
            }
            "progress" => {
                if let (Some(HostView::Progress(bar)), Some(v)) = (self.views.get(&id), number) {
                    bar.setDoubleValue(v.clamp(0.0, 1.0) as f64);
                }
            }

            // --- button
            "title" if kind == NodeKind::Button => {
                self.button_titles.insert(id, text.clone().unwrap_or_default());
                self.refresh_button(id);
            }
            "variant" if kind == NodeKind::Button => {
                self.button_variants.insert(id, text.clone().unwrap_or_else(|| "text".to_owned()));
                self.refresh_button(id);
            }
            "icon" | "iconPosition" if kind == NodeKind::Button => {
                let entry = self.button_icons.entry(id).or_default();
                if key == "icon" {
                    entry.0 = text.clone().unwrap_or_default();
                } else {
                    entry.1 = text.clone().unwrap_or_else(|| "leading".to_owned());
                }
                self.refresh_button(id);
            }

            // --- icons
            "name" | "iconSize" | "iconWeight" if kind == NodeKind::Icon => {
                let entry = self.icons.entry(id).or_default();
                match key {
                    "name" => entry.0 = text.clone().unwrap_or_default(),
                    "iconSize" => entry.1 = number.unwrap_or(24.0),
                    _ => entry.2 = number.unwrap_or(400.0) as u16,
                }
                let (name, size, weight) = entry.clone();
                if let Some(HostView::Icon(icon)) = self.views.get(&id) {
                    // As a template: that way `[color]` tints the symbol
                    // instead of it coming out in whatever colour it ships
                    // with.
                    let symbol = crate::icons::symbol(&name, size, weight);
                    if let Some(image) = &symbol {
                        unsafe { image.setTemplate(true) };
                    }
                    unsafe { icon.setImage(symbol.as_deref()) };
                }
            }

            // --- image
            "source" => {
                if let Some(HostView::Image(image)) = self.views.get(&id) {
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
                if let Some(HostView::Image(image)) = self.views.get(&id) {
                    unsafe {
                        image.setImageScaling(crate::images::image_scaling(
                            text.as_deref().unwrap_or("contain"),
                        ))
                    };
                }
            }

            // --- lists: tabs, segments and the drop-down
            "items" | "icons" => {
                let entry = self.segments.entry(id).or_default();
                let list = parse_string_list(text.as_deref().unwrap_or("[]"));
                if key == "items" {
                    entry.0 = list;
                } else {
                    entry.1 = list;
                }
                let (titles, icons) = entry.clone();
                match self.views.get(&id) {
                    Some(HostView::Segments(control)) | Some(HostView::Tabs(control)) => unsafe {
                        control.setSegmentCount(titles.len() as isize);
                        for (index, title) in titles.iter().enumerate() {
                            let index = index as isize;
                            control.setLabel_forSegment(&NSString::from_str(title), index);
                            let symbol = icons
                                .get(index as usize)
                                .and_then(|name| crate::icons::symbol(name, 0.0, 400));
                            if let Some(image) = &symbol {
                                image.setTemplate(true);
                            }
                            control.setImage_forSegment(symbol.as_deref(), index);
                        }
                    },
                    Some(HostView::Menu(menu)) => unsafe {
                        menu.removeAllItems();
                        for title in &titles {
                            menu.addItemWithTitle(&NSString::from_str(title));
                        }
                    },
                    _ => {}
                }
            }
            "selectedIndex" => {
                let index = number.unwrap_or(0.0).max(0.0) as isize;
                match self.views.get(&id) {
                    Some(HostView::Segments(control)) | Some(HostView::Tabs(control)) => unsafe {
                        if index < control.segmentCount() {
                            control.setSelectedSegment(index);
                        }
                    },
                    Some(HostView::Menu(menu)) => unsafe { menu.selectItemAtIndex(index) },
                    _ => {}
                }
            }
            "mode" if kind == NodeKind::DatePicker => {
                if let Some(HostView::Date(picker)) = self.views.get(&id) {
                    use objc2_app_kit::NSDatePickerElementFlags as Flags;
                    unsafe {
                        picker.setDatePickerElements(match text.as_deref() {
                            Some("time") => Flags::HourMinute,
                            Some("dateAndTime") => Flags::YearMonthDay | Flags::HourMinute,
                            _ => Flags::YearMonthDay,
                        });
                        // The range mode is not used here, but it has to be
                        // set or the picker remembers the previous one.
                        picker.setDatePickerMode(NSDatePickerMode::Single);
                    }
                }
            }

            // --- scroll
            "showsScrollIndicator" => {
                if let Some(HostView::Scroll(scroll)) = self.views.get(&id) {
                    let shown = !matches!(value, PropValue::Bool(false));
                    unsafe {
                        scroll.setHasVerticalScroller(shown);
                        scroll.setHasHorizontalScroller(shown);
                    }
                }
            }
            "scrollEnabled" => {
                // AppKit has no scroll switch, and hiding the scrollers is not
                // one: a scroll view scrolls because the wheel event reaches it
                // up the responder chain. The document view is ours and comes
                // first in that chain, so stopping the event there is what
                // stops the scroll — see `flipped.rs`. Whether a scroller is
                // drawn stays `showsScrollIndicator`'s business.
                let on = !matches!(value, PropValue::Bool(false));
                if let Some(HostView::Scroll(scroll)) = self.views.get(&id) {
                    if let Some(document) = unsafe { scroll.documentView() }
                        .and_then(|document| document.downcast::<FlippedView>().ok())
                    {
                        document.set_scroll_locked(!on);
                    }
                }
            }

            // --- embedded browser
            "url" if kind == NodeKind::WebView => {
                let Some(HostView::Web(web)) = self.views.get(&id) else { return };
                let Some(raw) = text.as_deref() else { return };
                let Some(url) =
                    (unsafe { objc2_foundation::NSURL::URLWithString(&NSString::from_str(raw)) })
                else {
                    return;
                };
                let request = unsafe { objc2_foundation::NSURLRequest::requestWithURL(&url) };
                let _ = web.loadRequest(&request);
            }
            "html" => {
                if let Some(HostView::Web(web)) = self.views.get(&id) {
                    let _ = web.loadHTMLString_baseURL(
                        &NSString::from_str(text.as_deref().unwrap_or("")),
                        None,
                    );
                }
            }

            // --- map
            "latitude" | "longitude" | "zoom" | "showsUser" if kind == NodeKind::MapView => {
                if key == "showsUser" {
                    // Showing where you are asks for location permission,
                    // and the system asks for the permission with the text the
                    // `.app`'s `Info.plist` declares. All that is asked for
                    // here is the dot.
                    if let Some(HostView::Map(map)) = self.views.get(&id) {
                        map.setShowsUserLocation(matches!(value, PropValue::Bool(true)));
                    }
                    return;
                }
                // All three arrive separately and in any order, and MapKit
                // has not three properties but one region: they have to be
                // kept and the whole thing rebuilt every time.
                let entry = self.maps.entry(id).or_insert((0.0, 0.0, 12.0));
                match key {
                    "latitude" => entry.0 = number.unwrap_or(0.0) as f64,
                    "longitude" => entry.1 = number.unwrap_or(0.0) as f64,
                    _ => entry.2 = number.unwrap_or(12.0) as f64,
                }
                let (lat, lon, zoom) = *entry;
                let Some(HostView::Map(map)) = self.views.get(&id) else { return };
                let span = crate::map::span_for_zoom(zoom);
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
            "url" | "playing" | "muted" if kind == NodeKind::VideoView => {
                if key == "url" {
                    let Some(raw) = text.clone() else { return };
                    let Some(url) = (unsafe {
                        objc2_foundation::NSURL::URLWithString(&NSString::from_str(&raw))
                    }) else {
                        // A URL that is not a URL must not end up as a
                        // silent player and a black box.
                        self.warn_once(format!("videourl:{raw}"), || {
                            eprintln!(
                                "angular-native: <an-video-view>'s `[url]` is not a valid URL: \
                                 {raw}"
                            );
                        });
                        return;
                    };
                    let player = crate::video::AVPlayer::with_url(&url, self.mtm);
                    if let Some(HostView::Video(view)) = self.views.get(&id) {
                        view.setPlayer(Some(&player));
                    }
                    self.videos.insert(id, player);
                    return;
                }
                let Some(player) = self.videos.get(&id).cloned() else { return };
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
            //
            // None is drawn: a Mac's header is the window's title bar. What is
            // done is to carry the `[title]` up there, which is where a Mac
            // user looks for it. See `support.rs`.
            "title" if kind == NodeKind::NavigationBar => {
                self.window_title = Some(text.clone().unwrap_or_default());
                // It is written in `flush`: when the prop arrives, the view
                // may not be inside a window yet.
                self.dirty_title = true;
            }

            // --- the pointer
            //
            // The cursor's shape is not a property of `NSView`: it is a
            // rectangle the view declares, and declaring it requires
            // overriding `resetCursorRects`, which cannot be done with a
            // system control. With an `NSTrackingArea` the owner is a separate
            // object and it works just the same over an `NSButton`. See
            // `events.rs`.
            "cursor" => {
                if let Some((area, _)) = self.cursors.remove(&id) {
                    native.removeTrackingArea(&area);
                }
                let Some(name) = text.clone().filter(|n| !n.is_empty()) else { return };
                let Some(cursor) = crate::events::system_cursor(&name) else {
                    self.warn_once(format!("cursor:{name}"), || {
                        eprintln!(
                            "angular-native: `[cursor]=\"{name}\"` is none of the system's \
                             pointers; the pointer is left as it was"
                        );
                    });
                    return;
                };
                self.cursors.insert(id, crate::events::attach_cursor(self.mtm, &native, cursor));
            }

            // --- the system's dialogs
            //
            // `sheet` asks for an action sheet: several things to do with what
            // was just tapped, rather than a question to answer. macOS has no
            // such control —the nearest is a context menu, which is a different
            // gesture in a different place— so nothing is applied, and it is
            // said once when a template really asks for one. Mapping it onto
            // the dialog's severity instead would change something the prop
            // does not mean, which reads from outside like a prop that works.
            "sheet" if self.alerts.contains_key(&id) => {
                if matches!(value, PropValue::Bool(true)) {
                    self.warn_once("alert-sheet".to_owned(), || {
                        eprintln!(
                            "angular-native: `sheet` on <Alert> does not apply on macOS: there \
                             is no action sheet on the desktop —the nearest control is a context \
                             menu, which is something else— so the dialog is the same NSAlert \
                             either way"
                        );
                    });
                }
            }
            "title" | "message" | "buttons" if self.alerts.contains_key(&id) => {
                let Some(state) = self.alerts.get_mut(&id) else { return };
                match key {
                    "title" => state.title = text.clone().unwrap_or_default(),
                    "message" => state.message = text.clone().unwrap_or_default(),
                    _ => state.buttons = parse_string_list(text.as_deref().unwrap_or("[]")),
                }
                if !self.dirty_alerts.contains(&id) {
                    self.dirty_alerts.push(id);
                }
            }
            "visible" if self.alerts.contains_key(&id) => {
                if let Some(state) = self.alerts.get_mut(&id) {
                    state.visible = matches!(value, PropValue::Bool(true));
                }
                if !self.dirty_alerts.contains(&id) {
                    self.dirty_alerts.push(id);
                }
            }
            "visible" => {
                native.setHidden(matches!(value, PropValue::Bool(false)));
            }

            // What macOS cannot honour, said on purpose.
            _ if ignored_reason(key).is_some() => {
                let reason = ignored_reason(key).unwrap_or_default();
                self.warn_once(format!("ignored:{kind:?}:{key}"), || {
                    eprintln!(
                        "angular-native: `{key}` on <{kind:?}> does not apply on macOS: {reason}"
                    );
                });
            }
            // And what is neither implemented nor declared. There is no
            // `_ => {}` here: a prop nobody looks at comes out on screen the
            // first time, which is the difference between a gap that is known
            // and one nobody sees.
            other => {
                self.warn_once(format!("unknown:{kind:?}:{other}"), || {
                    eprintln!(
                        "angular-native: unknown prop `{other}` on <{kind:?}>; the macOS host \
                         does not look at it and it is not declared in IGNORED"
                    );
                });
            }
        }
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        let Some(HostView::Label(label)) = self.views.get(&id) else { return };
        unsafe { label.setStringValue(&NSString::from_str(text)) };
        // `setStringValue` throws the attributed text away, so the letter
        // spacing, the line height and the underline have to be put back with
        // every new word.
        self.apply_text_attributes(id);
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let key = (id, event.to_owned());
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view().retain();

        // The safe area is the screen's cut-outs: the notch, the home
        // indicator. A desktop window has none of that, so the correct answer
        // is zero on all four sides, and it is answered once at subscription
        // time instead of leaving the template waiting.
        if event == "safeArea" {
            if enabled {
                an_host::push_event(
                    &self.events,
                    an_host::HostEvent {
                        target: id,
                        name: "safeArea".to_owned(),
                        payload: ["top", "right", "bottom", "left"]
                            .into_iter()
                            .map(|edge| (edge.to_owned(), PropValue::Number(0.0)))
                            .collect(),
                    },
                );
            }
            return;
        }

        if !enabled {
            if let Some(listener) = self.listeners.remove(&key) {
                listener.detach(&native);
            }
            return;
        }
        if self.listeners.contains_key(&key) {
            return;
        }

        let kind = view.kind();

        // The system dialog does not deliver its choice through the view: it
        // has no view. `NSAlert` delivers it from its completion block, so
        // attaching it here would be attaching it twice.
        if kind == NodeKind::Alert && event == "select" {
            return;
        }

        // What this platform cannot give is said at subscription time, not
        // when the event fails to arrive.
        if let Some(reason) = unsupported_event(kind, event) {
            self.warn_once(format!("event:{kind:?}:{event}"), || {
                eprintln!(
                    "angular-native: `({event})` on <{kind:?}> cannot be delivered on macOS: \
                     {reason}"
                );
            });
            return;
        }

        // The swipe is not attached: it is already on the view's class (see
        // `flipped.rs`). What has to be done is tell it who to notify and of
        // which direction. The `<an-scroll-view>` catches it through its
        // document view, which is the one that is ours; the `NSScrollView`
        // around it is the system's.
        if let Some(bit) = crate::support::swipe_bit(event) {
            let Some(flipped) = self.swipe_view(id) else {
                // This is not reached: `unsupported_event` already returned
                // a reason for anything that is not a view of ours, and this
                // subscription would not have got past it. If it is ever
                // reached, the two lists have drifted apart, and that must not
                // end in an output that never fires with nobody having said
                // so.
                self.warn_once(format!("swipe:{kind:?}"), || {
                    eprintln!(
                        "angular-native: `({event})` on <{kind:?}> could not be attached: the \
                         inventory says this primitive catches the swipe and the host cannot \
                         find where. Look at `support::catches_swipe` and `swipe_view`."
                    );
                });
                return;
            };
            flipped.listen_swipe(id, self.events.clone(), bit);
            self.listeners.insert(
                key,
                crate::events::AttachedListener::Swipe { view: flipped, bit },
            );
            return;
        }

        if let Some(listener) =
            crate::events::attach(self.mtm, &native, kind, id, event, self.events.clone())
        {
            self.listeners.insert(key, listener);
        } else if is_known_event(event) {
            // A name the framework does send and that this host does not
            // cover. The ones it does not send —the output names Angular
            // registers along the way— are dropped without noise: see
            // `support::KNOWN_EVENTS`.
            self.warn_once(format!("event:{kind:?}:{event}"), || {
                eprintln!(
                    "angular-native: the macOS host cannot deliver `({event})` on <{kind:?}>"
                );
            });
        }
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        self.frames.insert(id, frame);
        self.place(id);
        // A corner mask does not stretch with the view.
        if self.corners.contains_key(&id) {
            self.apply_corners(id);
        }
    }

    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        let Some(HostView::Scroll(scroll)) = self.views.get(&id) else { return };
        let Some(document) = (unsafe { scroll.documentView() }) else { return };
        // In AppKit the content's size *is* the document view's frame: there
        // is no separate `contentSize` the way `UIScrollView` has.
        document.setFrame(CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize { width: width as f64, height: height as f64 },
        });
    }

    fn set_clip(&mut self, id: NodeId, clip: bool) {
        // AppKit has no `clipsToBounds`: a view clips when its layer masks to
        // its bounds, which is also what a corner radius needs, so the two are
        // decided together rather than one overwriting the other.
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        native.setWantsLayer(true);
        if let Some(layer) = unsafe { native.layer() } {
            let rounded = layer.cornerRadius() > 0.0;
            layer.setMasksToBounds(clip || rounded);
        }
    }

    fn set_root(&mut self, id: NodeId) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        if unsafe { native.superview() }.is_none() {
            self.container.addSubview(native);
        }
    }

    fn flush(&mut self) {
        if self.dirty_title {
            self.apply_window_title();
        }

        // Ask again for whatever ought to be playing to play, and say so if
        // it is never going to.
        //
        // `play()` on a player that has not loaded anything yet does not
        // catch: the `rate` stays at zero and there it stays for good, with no
        // error and the view black. Since `playing` arrives only once, it has
        // to be retried until it takes hold. `1` is
        // `AVPlayerStatusReadyToPlay` and `2` is `AVPlayerStatusFailed`.
        //
        // And a video that failed stays exactly as black as one that has not
        // loaded yet. Looking at the window they cannot be told apart, so it
        // has to be said: it is the difference between "wait a moment" and
        // "that URL cannot be played".
        let mut failed: Vec<(NodeId, String)> = Vec::new();
        for (id, player) in &self.videos {
            if player.status() == 2 {
                let reason = player
                    .error()
                    .map(|error| unsafe { error.localizedDescription() }.to_string())
                    .unwrap_or_else(|| "no detail".to_owned());
                failed.push((*id, reason));
                continue;
            }
            if self.video_playing.contains(id) && player.rate() == 0.0 && player.status() == 1 {
                player.play();
            }
        }
        for (id, reason) in failed {
            self.warn_once(format!("video:{id}"), || {
                eprintln!(
                    "angular-native: <an-video-view>'s video cannot be played: {reason}"
                );
            });
        }

        // Presenting comes after the layout: a dialog is presented once all of
        // its props have arrived, or it would come out with half a title.
        for id in std::mem::take(&mut self.dirty_alerts) {
            let Some(mut state) = self.alerts.remove(&id) else { continue };
            state.sync(self.mtm, &self.container, id, &self.events);
            self.alerts.insert(id, state);
        }
    }

    fn clear(&mut self) {
        for view in self.views.values() {
            view.as_view().removeFromSuperview();
        }
        self.views.clear();
        self.fonts.clear();
        self.corners.clear();
        self.frames.clear();
        self.transforms.clear();
        self.listeners.clear();
        self.cursors.clear();
        self.alerts.clear();
        self.dirty_alerts.clear();
        self.maps.clear();
        self.videos.clear();
        self.video_playing.clear();
        // A `clear()` is a cold reload: the tree is stood up again from
        // scratch, so the window goes back to being called what it was called
        // until the new screen asks for its title.
        self.window_title = None;
        self.dirty_title = true;
    }
}

/// The outline of a rectangle with a different radius per corner.
///
/// The corners go in the order top-left, top-right, bottom-right, bottom-left,
/// the same one CSS uses, the same one Android expects and the same one the
/// iOS host draws.
///
/// It is built with `CGPath` and not `NSBezierPath`, which is what an AppKit
/// file would reach for first. `NSBezierPath`'s arc takes **degrees**, and its
/// sense of clockwise is in the unflipped coordinate system every view here
/// has turned over — two chances to get a corner subtly wrong for no gain.
/// `addArcToPoint` takes the two lines that meet and a radius, so there are no
/// angles to be wrong about.
fn rounded_path(width: f64, height: f64, radii: [f64; 4]) -> CFRetained<CGMutablePath> {
    // No corner may eat more than half the box, or the arcs cross and the
    // outline turns inside out.
    let limit = width.min(height) / 2.0;
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0.0, limit));
    let path = CGMutablePath::new();
    unsafe {
        // Start halfway along the top edge, where no corner can reach.
        CGMutablePath::move_to_point(Some(&path), std::ptr::null(), width / 2.0, 0.0);
        CGMutablePath::add_arc_to_point(Some(&path), std::ptr::null(), width, 0.0, width, height, tr);
        CGMutablePath::add_arc_to_point(Some(&path), std::ptr::null(), width, height, 0.0, height, br);
        CGMutablePath::add_arc_to_point(Some(&path), std::ptr::null(), 0.0, height, 0.0, 0.0, bl);
        CGMutablePath::add_arc_to_point(Some(&path), std::ptr::null(), 0.0, 0.0, width, 0.0, tl);
        CGMutablePath::close_subpath(Some(&path));
    }
    // A CGPath is Core Foundation and not Objective-C, so it arrives in a
    // `CFRetained` rather than a `Retained`. The mutable one derefs to the
    // immutable one, which is what `setPath` wants.
    path
}
