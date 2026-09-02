//! Host props: everything that is not layout. Color, text, font, `source`...
//! The core does not interpret them, except the ones that affect measuring.

use an_layout::FontSpec;

/// Node type. It maps 1:1 onto a native primitive, except for `RawText`, which
/// is internal and never gets mounted as a view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    View,
    Text,
    /// Raw text node created by `Renderer2.createText()`.
    RawText,
    Image,
    ScrollView,
    TextInput,
    /// A stack of screens. As far as layout is concerned it is an ordinary
    /// container —its children pile up and fill everything— but the host treats
    /// it differently: it animates the way in and the way out, and hooks up the
    /// back gesture.
    StackView,
    /// The system tab bar.
    TabBar,
    Switch,
    Slider,
    ActivityIndicator,
    ProgressBar,
    /// A system button, with its own typeface and its own response to touch.
    Button,
    /// A layer presented on top of everything.
    Modal,
    /// A system dialog. It takes up no room: it is presented over the app.
    Alert,
    /// A system icon: an SF Symbol on iOS, a drawable on Android. No icon set
    /// is bundled: they are asked for by name and drawn by the platform, with
    /// whatever stroke that version happens to give them.
    Icon,
    /// Pick one of several options, all of them on screen. `UISegmentedControl`
    /// on iOS; Android has no platform equivalent, so it is drawn out of system
    /// views, like the tab bar.
    SegmentedControl,
    /// Up and down, one at a time. `UIStepper` on iOS; Android has none either,
    /// so it is put together out of two buttons.
    Stepper,
    /// The system search field, with its magnifier and its clear button.
    SearchBar,
    /// Pick one of several options from a list that drops down.
    Picker,
    /// The system date and time picker.
    DatePicker,
    /// A header with a title and a back button.
    NavigationBar,
    /// A multi-line text field. It is a different kind of view, not a prop on
    /// the single-line one: on iOS they are `UITextField` and `UITextView`, two
    /// separate controls, and swapping one for the other mid-flight is not a
    /// thing you can do.
    TextEditor,
    /// An embedded browser.
    WebView,
    /// A map.
    MapView,
    /// A video player.
    VideoView,
}

impl NodeKind {
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "View" | "view" => NodeKind::View,
            "Text" | "text" => NodeKind::Text,
            "Image" | "image" => NodeKind::Image,
            "ScrollView" | "scroll-view" => NodeKind::ScrollView,
            "TextInput" | "text-input" => NodeKind::TextInput,
            "StackView" | "stack-view" => NodeKind::StackView,
            "TabBar" | "tab-bar" => NodeKind::TabBar,
            "Switch" | "switch" => NodeKind::Switch,
            "Slider" | "slider" => NodeKind::Slider,
            "ActivityIndicator" | "activity-indicator" => NodeKind::ActivityIndicator,
            "ProgressBar" | "progress-bar" => NodeKind::ProgressBar,
            "Button" | "button" => NodeKind::Button,
            "Modal" | "modal" => NodeKind::Modal,
            "Alert" | "alert" => NodeKind::Alert,
            "Icon" | "icon" => NodeKind::Icon,
            "VideoView" | "video-view" => NodeKind::VideoView,
            "MapView" | "map-view" => NodeKind::MapView,
            "WebView" | "web-view" => NodeKind::WebView,
            "TextEditor" | "text-editor" => NodeKind::TextEditor,
            "NavigationBar" | "navigation-bar" => NodeKind::NavigationBar,
            "DatePicker" | "date-picker" => NodeKind::DatePicker,
            "Picker" | "picker" => NodeKind::Picker,
            "SearchBar" | "search-bar" => NodeKind::SearchBar,
            "Stepper" | "stepper" => NodeKind::Stepper,
            "SegmentedControl" | "segmented-control" => NodeKind::SegmentedControl,
            _ => return None,
        })
    }

    /// `false` only for internal nodes with no native view behind them.
    pub fn is_mountable(self) -> bool {
        self != NodeKind::RawText
    }

    /// Leaf nodes as far as layout is concerned: their size is measured, not
    /// derived from children.
    ///
    /// `TextInput` belongs here so that a field with no explicit height takes up
    /// as much room as its text does, instead of collapsing to zero.
    pub fn is_measured_leaf(self) -> bool {
        matches!(self, NodeKind::Text | NodeKind::Image | NodeKind::TextInput)
            || self.is_control()
    }

    /// System controls: the platform draws them, and the platform is what
    /// decides their natural size, not the framework.
    pub fn is_control(self) -> bool {
        matches!(
            self,
            NodeKind::TabBar
                | NodeKind::Switch
                | NodeKind::Slider
                | NodeKind::ActivityIndicator
                | NodeKind::ProgressBar
                | NodeKind::Button
                | NodeKind::Icon
                | NodeKind::SegmentedControl
                | NodeKind::Stepper
                | NodeKind::SearchBar
                | NodeKind::Picker
                | NodeKind::DatePicker
                | NodeKind::NavigationBar
        )
    }

    /// The name by which the host recognises the control when measuring it.
    pub fn control_name(self) -> &'static str {
        match self {
            NodeKind::TabBar => "TabBar",
            NodeKind::Switch => "Switch",
            NodeKind::Slider => "Slider",
            NodeKind::ActivityIndicator => "ActivityIndicator",
            NodeKind::ProgressBar => "ProgressBar",
            NodeKind::Button => "Button",
            NodeKind::Icon => "Icon",
            NodeKind::SegmentedControl => "SegmentedControl",
            NodeKind::Stepper => "Stepper",
            NodeKind::SearchBar => "SearchBar",
            NodeKind::Picker => "Picker",
            NodeKind::DatePicker => "DatePicker",
            NodeKind::NavigationBar => "NavigationBar",
            _ => "",
        }
    }

    /// Presented on top of everything, outside its parent's flow.
    pub fn is_overlay(self) -> bool {
        matches!(self, NodeKind::Modal | NodeKind::Alert)
    }

    /// Takes up no room in the layout: the system presents it on its own.
    pub fn is_dialog(self) -> bool {
        self == NodeKind::Alert
    }

    /// Nodes whose content can overflow and therefore need a `contentSize`.
    pub fn is_scrollable(self) -> bool {
        self == NodeKind::ScrollView
    }

    /// Containers whose children come in and go out with an animation.
    pub fn is_stack(self) -> bool {
        self == NodeKind::StackView
    }
}

/// The value of a host prop. Deliberately small: whatever fits in the bridge's
/// binary protocol without serialising arbitrary objects.
#[derive(Clone, PartialEq, Debug)]
pub enum PropValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    /// Packed RGBA, 8 bits per channel.
    Color(u32),
}

impl PropValue {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            PropValue::Number(n) => Some(*n as f32),
            PropValue::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Pulls out of the props the font a `<Text>` has to be measured with.
pub fn font_from_props(get: impl Fn(&str) -> Option<PropValue>) -> FontSpec {
    let mut font = FontSpec::default();
    if let Some(v) = get("fontSize").and_then(|v| v.as_f32()) {
        font.size = v;
    }
    if let Some(v) = get("fontWeight") {
        font.weight = match &v {
            PropValue::Number(n) => *n as u16,
            PropValue::Str(s) => match s.as_str() {
                "normal" => 400,
                "bold" => 700,
                other => other.parse().unwrap_or(400),
            },
            _ => 400,
        };
    }
    if let Some(v) = get("fontStyle").and_then(|v| v.as_str().map(str::to_owned)) {
        font.italic = v == "italic";
    }
    if let Some(v) = get("fontFamily").and_then(|v| v.as_str().map(str::to_owned)) {
        font.family = Some(v);
    }
    if let Some(v) = get("lineHeight").and_then(|v| v.as_f32()) {
        font.line_height = Some(v);
    }
    if let Some(v) = get("letterSpacing").and_then(|v| v.as_f32()) {
        font.letter_spacing = v;
    }
    if let Some(v) = get("numberOfLines").and_then(|v| v.as_f32()) {
        if v >= 1.0 {
            font.max_lines = Some(v as u32);
        }
    }
    font
}

/// Props that, when they change, force the node to be measured again.
pub fn affects_measure(key: &str) -> bool {
    matches!(
        key,
        "fontSize"
            | "fontWeight"
            | "fontStyle"
            | "fontFamily"
            | "lineHeight"
            | "letterSpacing"
            | "numberOfLines"
            | "intrinsicWidth"
            | "intrinsicHeight"
            | "value"
            | "placeholder"
            | "title"
            | "items"
            | "buttons"
    )
}
