//! The Rust tree, in the shape SwiftUI knows how to read.
//!
//! Why a whole snapshot and not one op per call: crossing the boundary once
//! per `MountOp` is hundreds of crossings per frame, which is exactly what the
//! bridge's binary protocol exists to avoid. And why JSON rather than that
//! binary protocol: a watch screen is ten or fifteen nodes, `Codable` decodes
//! it without anyone writing a parser, and the place where this would stop
//! paying off —a long list— does not exist on watchOS yet. When it does, what
//! has to change is this file and Swift's `Decodable`, not the host.
//!
//! Colours come out already resolved into 0..1 channels. Swift never parses
//! `#0b1020` again: if it did there would be two parsers to keep in agreement.
//!
//! Two things do not travel inside the tree and come out separately, in
//! `overlays`: the `Alert` and the `Modal`. In SwiftUI they are not views that
//! get placed, they are modifiers —`.alert`, `.sheet`, `.fullScreenCover`—
//! hung off the root, and the system decides where they go. Leaving them
//! inside would force the shell to hunt for them through the tree on every
//! frame.

use std::cell::RefCell;
use std::collections::HashSet;

use an_core::accessibility::{parse_state, Checked, Role};
use an_core::{NodeId, NodeKind, PropValue};
use serde::Serialize;

use crate::host::WatchHost;

#[derive(Serialize)]
pub struct Snapshot {
    pub revision: u64,
    /// `None` while the app has not mounted anything yet.
    pub root: Option<Node>,
    /// What the system presents on top: dialogs and sheets. They come out of
    /// the tree because in SwiftUI they are not placed, they are declared on
    /// the root.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<Node>,
}

/// A node exactly as the shell paints it. Everything that does not apply to
/// the node's kind is left out, so that one screen's JSON fits in a single
/// glance when it has to be debugged.
#[derive(Serialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: &'static str,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    /// Why this node is not painted on the watch. `unsupported()` fills it in,
    /// and it is what the shell shows —and what the host says through the log—
    /// instead of leaving a gap nobody can account for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported: Option<&'static str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_radius: Option<f32>,
    /// The four corners, and only when the template did not give them all the
    /// same radius. See `corner_radii_of`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_radii: Option<[f32; 4]>,
    /// One width for the whole outline. There is no per-side border here and
    /// there is none on any other host either: `borderTopWidth` and its three
    /// siblings are layout styles, they never leave taffy, and what they do is
    /// inset the children.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Only travels when the template switches it off: a control being live
    /// is the normal case, and sending it always would fatten every node for
    /// nothing.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_id: Option<String>,

    // -------------------------------------------------------- accessibility
    //
    // The six props of the contract, already translated into SwiftUI's
    // vocabulary. The translation happens here and not in the shell for the
    // same reason the icon one does: this is where the decision lives and
    // where what has no equivalent can be said. The shell only attaches the
    // modifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_hint: Option<String>,
    /// What the reader says the value is. It arrives already resolved: either
    /// the one the template set, or the `"1"`/`"0"` a `checked` maps to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_value: Option<String>,
    /// The `AccessibilityTraits` to add, by name.
    ///
    /// It travels as a list of names and not as separate fields for the same
    /// reason `listens` does: role and state both end up here —`isButton`
    /// comes from the role, `isSelected` from the state— and adding a new one
    /// is adding a string, not a field in three places.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accessibility_traits: Vec<&'static str>,
    /// Whether this is **one** element for the reader or a container. `false`
    /// hides the whole branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessible: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_decoration: Option<String>,
    /// `numberOfLines`. Zero or absent means "as many as it takes".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u16>,

    // ------------------------------------------------------------- controls
    /// `Switch`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
    /// `Slider` and `Stepper`; on a `DatePicker`, milliseconds since 1970.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// `ProgressBar`, from 0 to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f32>,
    /// `ActivityIndicator`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animating: Option<bool>,
    /// `TextInput`: the text the template sends. It is not the same field as
    /// `text`, which is the label of a `Text` or of a `Button`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub secure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<String>,
    /// `Picker`: the labels, already taken out of the JSON they travel in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_index: Option<i64>,
    /// `DatePicker`: `date`, `time` or `dateAndTime`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_mode: Option<String>,
    /// `Icon`: the SF Symbol name, already translated from the common names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_weight: Option<u16>,
    /// `Image`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resize_mode: Option<String>,

    // -------------------------------------------------------- presentations
    /// `Alert` and `Modal`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buttons: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<String>,
    /// `StackView`: which way the next transition goes. Whoever navigates
    /// decides it, being the only one that knows whether this is a step
    /// forward or a step back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,

    /// Whether this node keeps its children inside its own frame. SwiftUI's
    /// `.clipped()` on the other side. Omitted when false, which is the
    /// default, so the snapshot does not grow a key per node for nothing.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub clip: bool,
    /// `ScrollView`: which way the content may overflow. Omitted when it is
    /// downwards, which is what every scroll view that never asked does.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub horizontal: bool,
    /// Only on a `ScrollView`, and only when the content overflows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_height: Option<f32>,

    /// What the template is listening for on this node.
    ///
    /// It goes as a list rather than one boolean per gesture because the shell
    /// only asks whether a name is in it: adding `crown` was adding a string,
    /// not a field in three places. And without the list the shell would
    /// attach recognisers nobody is listening to, which shows on a watch —the
    /// system highlights whatever can be touched— and would on top of that eat
    /// the gestures of the `ScrollView` underneath.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub listens: Vec<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

/// A stable name for the node's kind. `Debug` is not used: the Swift shell
/// switches over these strings, and renaming a variant in Rust must not be
/// able to break the watch in silence.
fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::View => "View",
        NodeKind::Text => "Text",
        NodeKind::RawText => "RawText",
        NodeKind::ScrollView => "ScrollView",
        NodeKind::Button => "Button",
        NodeKind::Image => "Image",
        NodeKind::TextInput => "TextInput",
        NodeKind::StackView => "StackView",
        NodeKind::Switch => "Switch",
        NodeKind::Slider => "Slider",
        NodeKind::ActivityIndicator => "ActivityIndicator",
        NodeKind::ProgressBar => "ProgressBar",
        NodeKind::Stepper => "Stepper",
        NodeKind::Picker => "Picker",
        NodeKind::DatePicker => "DatePicker",
        NodeKind::Icon => "Icon",
        NodeKind::Alert => "Alert",
        NodeKind::Modal => "Modal",
        NodeKind::TabBar => "TabBar",
        NodeKind::NavigationBar => "NavigationBar",
        NodeKind::SegmentedControl => "SegmentedControl",
        NodeKind::SearchBar => "SearchBar",
        NodeKind::TextEditor => "TextEditor",
        NodeKind::WebView => "WebView",
        NodeKind::MapView => "MapView",
        NodeKind::VideoView => "VideoView",
        NodeKind::Custom => "Custom",
    }
}

/// What the watch cannot draw, and why.
///
/// The reason is not an opinion: nearly all of them are what the watchOS SDK
/// says, and they are written here so that the gap explains itself. It is the
/// same decision tvOS took in `an-ios/src/family.rs`: a hand-written list,
/// because Swift's availability annotation cannot be read from Rust.
pub fn unsupported(kind: NodeKind) -> Option<&'static str> {
    Some(match kind {
        NodeKind::TabBar => {
            "a tab bar does not fit in 205 points: on a watch sections are swiped \
             through full screen, which is a container and not a bar with a frame"
        }
        NodeKind::NavigationBar => {
            "the strip at the top of a watch already belongs to the system —the time \
             and the app's title— and a bar of our own would paint below it or over it"
        }
        NodeKind::SegmentedControl => {
            "SegmentedPickerStyle is marked @available(watchOS, unavailable) in \
             SwiftUI; what the watch uses in its place is an-select"
        }
        NodeKind::SearchBar => {
            "on a watch, searching is a screen of the system's and not a field with a \
             magnifying glass: .searchable does exist, but it is a navigation \
             modifier, not a view with a frame"
        }
        NodeKind::TextEditor => {
            "TextEditor is marked @available(watchOS, unavailable) in SwiftUI; long \
             text is dictated or scribbled, and an-text-input already gives that"
        }
        NodeKind::WebView => "WebKit is not in the watchOS SDK",
        NodeKind::VideoView => {
            "AVKit on watchOS ships neither AVPlayerViewController nor VideoPlayer: \
             its headers only declare types, no playback view at all"
        }
        NodeKind::Custom => {
            "a watch has no view hierarchy: this host mirrors the tree into a model \
             SwiftUI redraws, and a plugin view is a native view somebody else built. \
             There is nowhere here to put one"
        }
        NodeKind::MapView => {
            "SwiftUI's Map does exist on watchOS, but it takes neither a centre nor a \
             zoom from the app: it would show a place the template did not choose"
        }
        _ => return None,
    })
}

fn color_of(host: &WatchHost, id: NodeId, key: &str) -> Option<[f32; 4]> {
    let value = host.node(id)?.props.get(key)?;
    let (r, g, b, a) = match value {
        PropValue::Str(raw) => an_core::color::parse(raw)?,
        // Packed RGBA, 8 bits per channel, the way the protocol sends it.
        PropValue::Color(packed) => {
            let byte = |shift: u32| ((packed >> shift) & 0xff) as f64 / 255.0;
            (byte(24), byte(16), byte(8), byte(0))
        }
        _ => return None,
    };
    Some([r as f32, g as f32, b as f32, a as f32])
}

fn number_of(host: &WatchHost, id: NodeId, key: &str) -> Option<f32> {
    host.node(id)?.props.get(key)?.as_f32()
}

/// The four corner radii, and only when they are not all the same.
///
/// Nearly every rounded node has one radius for the four corners, and for that
/// `border_radius` on its own is enough; sending an array as well would add a
/// key to every node in the snapshot to say what the number already said. The
/// four only travel when the template really set them apart. It is the same
/// call `an-ios` makes before reaching for a mask layer, and for the same
/// reason: the cheap path covers almost everything.
///
/// The order is the one the props are written in —top left, top right, bottom
/// right, bottom left— and not SwiftUI's leading/trailing. Renaming them on the
/// way across would mean the shell reading position 0 had to remember it was no
/// longer the top left, and the prop names are the ones an app author sees.
fn corner_radii_of(host: &WatchHost, id: NodeId) -> Option<[f32; 4]> {
    // A per-corner radius overrides `borderRadius` on that corner alone: that
    // is what `[borderRadius]="8" [borderTopLeftRadius]="0"` has to mean, and
    // it is what `set_corner` does on iOS.
    let all = number_of(host, id, "borderRadius").unwrap_or(0.0);
    let radii = [
        number_of(host, id, "borderTopLeftRadius").unwrap_or(all),
        number_of(host, id, "borderTopRightRadius").unwrap_or(all),
        number_of(host, id, "borderBottomRightRadius").unwrap_or(all),
        number_of(host, id, "borderBottomLeftRadius").unwrap_or(all),
    ];
    radii.iter().any(|r| *r != radii[0]).then_some(radii)
}

fn f64_of(host: &WatchHost, id: NodeId, key: &str) -> Option<f64> {
    match host.node(id)?.props.get(key)? {
        PropValue::Number(n) => Some(*n),
        PropValue::Str(s) => s.parse().ok(),
        _ => None,
    }
}

fn string_of(host: &WatchHost, id: NodeId, key: &str) -> Option<String> {
    host.node(id)?.props.get(key)?.as_str().map(str::to_owned)
}

fn bool_of(host: &WatchHost, id: NodeId, key: &str) -> Option<bool> {
    match host.node(id)?.props.get(key)? {
        PropValue::Bool(b) => Some(*b),
        PropValue::Number(n) => Some(*n != 0.0),
        PropValue::Str(s) => match s.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Lists of labels travel as JSON because the bridge's protocol carries no
/// lists. They are taken apart here and not in Swift: were they taken apart
/// there, two places would have to know this was JSON.
fn strings_of(host: &WatchHost, id: NodeId, key: &str) -> Option<Vec<String>> {
    let raw = string_of(host, id, key)?;
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(items) => Some(items),
        Err(error) => {
            eprintln!("angular-native: [{key}] is not a list of labels: {error}");
            None
        }
    }
}

pub fn snapshot(host: &WatchHost) -> Snapshot {
    let mut overlays = Vec::new();
    let root = host.root().and_then(|root| node(host, root, &mut overlays));
    Snapshot { revision: host.revision(), root, overlays }
}

fn node(host: &WatchHost, id: NodeId, overlays: &mut Vec<Node>) -> Option<Node> {
    let source = host.node(id)?;
    let kind = source.kind?;
    if !kind.is_mountable() {
        return None;
    }
    let frame = source.frame;
    let name = kind_name(kind);
    let unsupported = unsupported(kind);
    if let Some(reason) = unsupported {
        warn_once(name, "", &format!("{name} is not painted on watchOS: {reason}"));
    }

    // A `<Text>`'s text is its `RawText` children concatenated; those do not
    // go down to the shell, because SwiftUI wants the whole string. A `Button`
    // carries its label in a prop, just as on iOS.
    let text = match kind {
        NodeKind::Text => Some(host.text_of(id)),
        NodeKind::Button => string_of(host, id, "title"),
        _ => None,
    };

    let children = if kind == NodeKind::Text {
        // A `<Text>`'s children have already been fused into `text`.
        Vec::new()
    } else {
        crate::host::mountable_children(host, id)
            .into_iter()
            .filter_map(|child| node(host, child, overlays))
            .collect()
    };

    let content = source.content_size.filter(|_| kind.is_scrollable());
    let mut listens: Vec<String> = source.listeners.iter().cloned().collect();
    // A stable order: the snapshot goes into a test and into a `diff`, and a
    // `HashSet` would shuffle it on every frame.
    listens.sort();
    warn_unheard(name, &listens);

    // Whether this kind has an edge to draw on. `unpaintable` is the one place
    // that decides it, and it is asked here so the snapshot cannot promise the
    // shell a border on the very node the host has just refused one on: a field
    // in the snapshot is a promise something will be drawn with it.
    let outline = unpaintable(kind, "borderWidth").is_none();

    let built = Node {
        id,
        kind: name,
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
        unsupported,
        background: color_of(host, id, "backgroundColor"),
        color: color_of(host, id, "color"),
        border_radius: outline.then(|| number_of(host, id, "borderRadius")).flatten(),
        border_radii: outline.then(|| corner_radii_of(host, id)).flatten(),
        // A zero-width border is no border: sending it would make the shell
        // stroke a hairline SwiftUI still antialiases into a visible edge.
        border_width: outline
            .then(|| number_of(host, id, "borderWidth").filter(|w| *w > 0.0))
            .flatten(),
        border_color: outline.then(|| color_of(host, id, "borderColor")).flatten(),
        opacity: number_of(host, id, "opacity"),
        // A switched-off control is the exception, not the rule: only the
        // `false` travels.
        disabled: bool_of(host, id, "enabled") == Some(false)
            || bool_of(host, id, "editable") == Some(false),
        test_id: string_of(host, id, "testID"),
        accessibility_label: string_of(host, id, "accessibilityLabel").filter(|l| !l.is_empty()),
        accessibility_hint: string_of(host, id, "accessibilityHint").filter(|h| !h.is_empty()),
        accessibility_value: accessibility_value_of(host, id, name),
        accessibility_traits: accessibility_traits_of(host, id, name),
        accessible: bool_of(host, id, "accessible"),
        text,
        font_size: number_of(host, id, "fontSize"),
        font_weight: font_weight_of(host, id),
        italic: string_of(host, id, "fontStyle").as_deref() == Some("italic"),
        font_family: string_of(host, id, "fontFamily"),
        letter_spacing: number_of(host, id, "letterSpacing"),
        text_align: string_of(host, id, "textAlign"),
        text_decoration: string_of(host, id, "textDecoration").filter(|d| d != "none"),
        max_lines: number_of(host, id, "numberOfLines").map(|n| n as u16).filter(|n| *n > 0),
        on: bool_of(host, id, "on"),
        value: value_of(host, id, kind),
        minimum: f64_of(host, id, "minimumValue"),
        maximum: f64_of(host, id, "maximumValue"),
        step: f64_of(host, id, "stepValue").filter(|s| *s > 0.0),
        progress: number_of(host, id, "progress").map(|p| p.clamp(0.0, 1.0)),
        animating: bool_of(host, id, "animating"),
        field: string_of(host, id, "value").filter(|_| kind == NodeKind::TextInput),
        placeholder: string_of(host, id, "placeholder"),
        secure: bool_of(host, id, "secureTextEntry") == Some(true),
        keyboard: string_of(host, id, "keyboardType"),
        items: strings_of(host, id, "items"),
        selected_index: f64_of(host, id, "selectedIndex").map(|i| i as i64),
        date_mode: string_of(host, id, "mode"),
        symbol: string_of(host, id, "name").map(|raw| an_core::icons::translate(&raw).to_owned()),
        symbol_size: number_of(host, id, "iconSize"),
        symbol_weight: number_of(host, id, "iconWeight").map(|w| w as u16),
        source: string_of(host, id, "source"),
        resize_mode: string_of(host, id, "resizeMode"),
        visible: bool_of(host, id, "visible"),
        title: string_of(host, id, "title").filter(|_| kind == NodeKind::Alert),
        message: string_of(host, id, "message"),
        buttons: strings_of(host, id, "buttons"),
        presentation: string_of(host, id, "presentation"),
        transition: string_of(host, id, "transition"),
        clip: source.clip,
        horizontal: kind.is_scrollable() && bool_of(host, id, "horizontal") == Some(true),
        content_width: content.map(|c| c.0),
        content_height: content.map(|c| c.1),
        listens,
        children,
    };
    warn_unread(host, id, kind);

    // `Alert` and `Modal` are not placed: the system presents them. They come
    // out of the tree and are hung off the root, which is where SwiftUI expects
    // the modifier.
    if kind.is_overlay() {
        overlays.push(built);
        return None;
    }
    Some(built)
}

/// The value a numeric control starts at.
///
/// `DatePicker` sends it in milliseconds since 1970 —what `Date` gives and
/// takes— and that is how it travels to Swift: converting it here would force
/// converting it back when the `change` is sent.
fn value_of(host: &WatchHost, id: NodeId, kind: NodeKind) -> Option<f64> {
    match kind {
        NodeKind::Slider | NodeKind::Stepper | NodeKind::DatePicker => f64_of(host, id, "value"),
        _ => None,
    }
}

/// `fontWeight` arrives either as a number (`700`) or as a name (`bold`), just
/// as in CSS. It is normalised here so the shell only ever sees numbers.
fn font_weight_of(host: &WatchHost, id: NodeId) -> Option<u16> {
    let value = host.node(id)?.props.get("fontWeight")?;
    match value {
        PropValue::Number(n) => Some(*n as u16),
        PropValue::Str(s) => match s.as_str() {
            "normal" => Some(400),
            "bold" => Some(700),
            other => other.parse().ok(),
        },
        _ => None,
    }
}

/// The contract's role and state, as SwiftUI `AccessibilityTraits`.
///
/// The watch is the only declarative host of Apple's four: there is no setter
/// to call when the prop arrives, there is a view built out of every snapshot.
/// So the translation lives in the snapshot and not in a `set_prop`: by the
/// time the shell draws, it is already done.
///
/// SwiftUI's set is shorter than the contract's in two places —`radio` and
/// `slider`— and that gets said, not nudged towards the one next door.
fn accessibility_traits_of(host: &WatchHost, id: NodeId, kind: &'static str) -> Vec<&'static str> {
    let mut traits = Vec::new();

    if let Some(raw) = string_of(host, id, "accessibilityRole").filter(|r| !r.is_empty()) {
        match Role::parse(&raw) {
            None => warn_once(
                kind,
                "accessibilityRole",
                &format!(
                    "<{kind}> asks for [accessibilityRole]=\"{raw}\", which is none of the roles \
                     in the contract; the role is not applied"
                ),
            ),
            Some(Role::None) => {}
            Some(role) => match swiftui_trait(role) {
                Some(name) => traits.push(name),
                None => warn_once(
                    kind,
                    "accessibilityRole",
                    &format!(
                        "<{kind}> asks for [accessibilityRole]=\"{}\": SwiftUI has no \
                         AccessibilityTraits for that, so the role is not applied",
                        role.name()
                    ),
                ),
            },
        }
    }

    let Some(raw) = string_of(host, id, "accessibilityState") else { return traits };
    let (state, unknown) = parse_state(&raw);
    for entry in unknown {
        warn_once(
            kind,
            "accessibilityState",
            &format!(
                "<{kind}> carries [accessibilityState] with `{}: {}`, which is not in the \
                 contract; that key was not applied",
                entry.key, entry.value
            ),
        );
    }
    if state.selected == Some(true) {
        traits.push("isSelected");
    }
    // What SwiftUI cannot say about a state. `disabled` is not in
    // `AccessibilityTraits`: SwiftUI's way is `.disabled(true)`, which also
    // stops taking taps, and really turning a control off just to announce it
    // off would change what the view does, not what is read out of it.
    if state.disabled.is_some() {
        warn_once(
            kind,
            "accessibilityState.disabled",
            &format!(
                "<{kind}> asks for `disabled` in [accessibilityState]: SwiftUI has no such trait, \
                 and `.disabled()` would switch off the tap as well. To switch a control off \
                 there is [enabled]"
            ),
        );
    }
    if state.expanded.is_some() {
        warn_once(
            kind,
            "accessibilityState.expanded",
            &format!("<{kind}> asks for `expanded` in [accessibilityState]: SwiftUI has no such trait"),
        );
    }
    if state.busy.is_some() {
        warn_once(
            kind,
            "accessibilityState.busy",
            &format!("<{kind}> asks for `busy` in [accessibilityState]: SwiftUI has no such trait"),
        );
    }
    traits
}

/// The `AccessibilityTraits` each role gets, by name.
///
/// It returns the name and not a constant because the constants are in Swift:
/// the shell keeps the name-to-`AccessibilityTraits` table in one place, and a
/// name it does not recognise leaves through the log instead of getting lost.
fn swiftui_trait(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Button => "isButton",
        Role::Link => "isLink",
        Role::Header => "isHeader",
        Role::Image => "isImage",
        Role::Text => "isStaticText",
        // Same as UIKit: SwiftUI's trait is "this turns on and off", and that
        // describes both a checkbox and a switch.
        Role::Checkbox | Role::Switch => "isToggle",
        Role::Search => "isSearchField",
        Role::Summary => "isSummaryElement",
        // SwiftUI **has no** radio and no slider trait. Adjustable in SwiftUI
        // is not a trait but an action, `accessibilityAdjustableAction`, and
        // hanging one that did nothing would be exactly the imitation this
        // project does not do.
        Role::Radio | Role::Slider => return None,
        Role::None => return None,
    })
}

/// The value the reader announces: the template's, or the one `checked` maps
/// to.
///
/// The `"1"`/`"0"` is the platform's own convention —it is what a `Toggle`
/// publishes— which is why no word is written here: a label of ours would come
/// out in English on a watch set to Japanese.
fn accessibility_value_of(host: &WatchHost, id: NodeId, kind: &'static str) -> Option<String> {
    if let Some(value) = string_of(host, id, "accessibilityValue").filter(|v| !v.is_empty()) {
        return Some(value);
    }
    let raw = string_of(host, id, "accessibilityState")?;
    match parse_state(&raw).0.checked? {
        Checked::Yes => Some("1".to_owned()),
        Checked::No => Some("0".to_owned()),
        Checked::Mixed => {
            warn_once(
                kind,
                "accessibilityState.checked",
                &format!(
                    "<{kind}> asks for `checked: \"mixed\"`: in SwiftUI the accessibility value \
                     only knows checked and unchecked, so it is left empty"
                ),
            );
            None
        }
    }
}

/// Gestures the template asks for that never arrive on the watch.
///
/// Keeping quiet here would be the worst of both worlds: the template writes a
/// `(pinch)`, nothing fails, and the gesture simply never responds. The reason
/// travels with the warning because nearly always it is the SDK's, not a
/// decision of this project's.
fn warn_unheard(kind: &'static str, listens: &[String]) {
    for event in listens {
        let Some(reason) = unheard(event) else { continue };
        warn_once(kind, event, &format!("({event}) does not arrive on watchOS: {reason}"));
    }
}

fn unheard(event: &str) -> Option<&'static str> {
    Some(match event {
        "pinch" => {
            "MagnifyGesture is marked @available(watchOS, unavailable), and two \
             fingers do not fit on a 40 mm screen"
        }
        "rotate" => "RotateGesture is marked @available(watchOS, unavailable)",
        "back" => {
            "outside a NavigationStack the watch gives no drag from the edge, and \
             putting one up would nest SwiftUI's layout inside taffy's"
        }
        "refresh" => {
            "on a watch you do not pull a list down to reload it: that is done with \
             the crown, which already arrives as (crown)"
        }
        "scroll" => {
            "SwiftUI's ScrollView does not publish its offset on watchOS 11, which is \
             this shell's minimum"
        }
        "safeArea" => {
            "a watch app takes the whole screen and the system reserves no margins \
             that could be asked about"
        }
        "focus" | "blur" => {
            "not yet: on a watch the focus is the same one that decides who holds the \
             crown, and giving it two owners would make the crown jump elsewhere \
             while typing"
        }
        _ => return None,
    })
}

/// Props that reach the watch and that nobody looks at.
///
/// A prop that travels, that nobody recognises and that raises no error is a
/// bug found months later, when somebody wonders why their `[borderWidth]`
/// paints nothing. It is said once per kind and key —not once per frame, which
/// at 30 Hz would be an unreadable log— and it is said from here, the only
/// place that knows what actually ended up being used.
fn warn_unread(host: &WatchHost, id: NodeId, kind: NodeKind) {
    let Some(node) = host.node(id) else { return };
    let name = kind_name(kind);
    for key in node.props.keys() {
        // The reason first, when there is one: "nobody reads this" and "this
        // cannot be drawn here" are different pieces of news, and only the
        // second one tells whoever is reading the log to stop looking for a
        // version of the SDK where it works.
        if let Some(reason) = unpaintable(kind, key) {
            let said =
                format!("<{name}> got [{key}], and it cannot be painted on watchOS: {reason}");
            warn_once(name, key, &said);
            continue;
        }
        if reads(kind, key) {
            continue;
        }
        warn_once(name, key, &format!("<{name}> got [{key}], and the watchOS host does not look at it"));
    }
}

/// Props that arrive on a kind that can never paint them, and why.
///
/// The distinction this draws is the whole point of it: `reads()` says "no host
/// code looks at this yet", which is a gap somebody can close, and this says
/// "there is nothing here to draw on", which nobody can. Sending the same
/// sentence for both would have an app author waiting for a release that is
/// never coming.
///
/// It is the third of these lists —`unsupported` rules out a whole primitive,
/// `unheard` an event— and it is a function rather than a table for the same
/// reason they are: the reason has to be written next to the decision.
pub(crate) fn unpaintable(kind: NodeKind, key: &str) -> Option<&'static str> {
    // Only the outline family, and only where there is no outline to draw. A
    // kind the watch does not paint at all has already said so wholesale
    // through `unsupported`, and repeating it per prop would bury it.
    if !matches!(
        key,
        "borderWidth"
            | "borderColor"
            | "borderRadius"
            | "borderTopLeftRadius"
            | "borderTopRightRadius"
            | "borderBottomRightRadius"
            | "borderBottomLeftRadius"
    ) {
        return None;
    }
    match kind {
        NodeKind::Alert | NodeKind::Modal => Some(
            "the system presents a dialog and a sheet, and the app hands it a title, a \
             message and buttons — not a frame. There is no edge of ours to round or to \
             stroke, and drawing one would mean painting a lookalike in front of the \
             real one",
        ),
        _ => None,
    }
}

/// Whether the watch does anything with that prop on that kind of node.
pub(crate) fn reads(kind: NodeKind, key: &str) -> bool {
    // Common to everything that gets painted. The outline goes here and not
    // per kind because on the watch it is not a property of a view —there are
    // no views— but a shape stroked around the frame taffy gave, and every node
    // has one of those. Where there is no frame to stroke, `unpaintable` has
    // already said so with the reason before this list is ever consulted.
    if matches!(
        key,
        "backgroundColor"
            | "borderRadius"
            | "borderTopLeftRadius"
            | "borderTopRightRadius"
            | "borderBottomRightRadius"
            | "borderBottomLeftRadius"
            | "borderWidth"
            | "borderColor"
            | "opacity"
            | "testID"
            | "enabled"
    ) {
        return true;
    }
    // The six accessibility ones hold on any node: the contract puts them on
    // the base, not on a control. Whatever of them cannot be applied on the
    // watch is already said, with the reason, by `accessibility_traits_of`, so
    // warning here as well would be saying the same thing twice.
    if matches!(
        key,
        "accessibilityLabel"
            | "accessibilityHint"
            | "accessibilityRole"
            | "accessibilityValue"
            | "accessibilityState"
            | "accessible"
    ) {
        return true;
    }
    // What only the other host looks at is never an oversight: it comes from
    // the template's `[ios]` or `[android]`, which already say who they are
    // addressed to.
    if key.starts_with("ios:") || key.starts_with("android:") {
        return true;
    }
    // The mark Angular puts on the root. It comes out of no template and no
    // host looks at it, so warning about it would be warning, in every app,
    // about something nobody wrote.
    if key == "ng-version" {
        return true;
    }
    match kind {
        NodeKind::Text => matches!(
            key,
            "color"
                | "fontSize"
                | "fontWeight"
                | "fontStyle"
                | "fontFamily"
                | "letterSpacing"
                | "textAlign"
                | "textDecoration"
                | "numberOfLines"
        ),
        NodeKind::Button => matches!(key, "title" | "color" | "fontSize" | "fontWeight"),
        NodeKind::Image => {
            matches!(key, "source" | "resizeMode" | "intrinsicWidth" | "intrinsicHeight")
        }
        NodeKind::Icon => matches!(key, "name" | "iconSize" | "iconWeight" | "color"),
        NodeKind::Switch => matches!(key, "on" | "color"),
        NodeKind::Slider => matches!(key, "value" | "minimumValue" | "maximumValue" | "color"),
        NodeKind::Stepper => matches!(key, "value" | "minimumValue" | "maximumValue" | "stepValue"),
        NodeKind::ProgressBar => matches!(key, "progress" | "color"),
        NodeKind::ActivityIndicator => matches!(key, "animating" | "color"),
        NodeKind::TextInput => matches!(
            key,
            "value"
                | "placeholder"
                | "secureTextEntry"
                | "editable"
                | "color"
                | "fontSize"
                | "fontWeight"
                | "textAlign"
                | "keyboardType"
        ),
        NodeKind::Picker => matches!(key, "items" | "selectedIndex"),
        NodeKind::DatePicker => matches!(key, "value" | "mode"),
        NodeKind::Alert => matches!(key, "visible" | "title" | "message" | "buttons" | "sheet"),
        NodeKind::Modal => matches!(key, "visible" | "presentation"),
        NodeKind::ScrollView => {
            matches!(key, "scrollEnabled" | "showsScrollIndicator" | "horizontal")
        }
        NodeKind::StackView => key == "transition",
        // What the watch does not paint was already warned about wholesale by
        // its kind: repeating its props would be saying the same thing twice.
        other => unsupported(other).is_some(),
    }
}

thread_local! {
    /// What has already been said, so as not to repeat it thirty times a
    /// second. It is `thread_local` because the snapshot is always built on the
    /// UI thread: a `Mutex` here would be a lock nobody contends for.
    static SAID: RefCell<HashSet<(&'static str, String)>> = RefCell::new(HashSet::new());
}

fn warn_once(kind: &'static str, key: &str, message: &str) {
    SAID.with(|said| {
        if said.borrow_mut().insert((kind, key.to_owned())) {
            eprintln!("angular-native: {message}");
        }
    });
}
