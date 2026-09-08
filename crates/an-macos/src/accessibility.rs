//! The six accessibility props of the contract, on AppKit.
//!
//! **AppKit's roles are a different set.** Not UIKit's with other names: in
//! UIKit a role is one bit of a mask and several things coexist —"button" and
//! "selected" at once—; here a role is *one* string, a view has exactly one,
//! and what in UIKit are state traits are separate properties of the
//! `NSAccessibility` protocol: `setAccessibilityEnabled:`,
//! `setAccessibilitySelected:`, `setAccessibilityExpanded:`. That is why this
//! file rebuilds no mask and the iOS one does.
//!
//! Two things about AppKit shape everything below, and neither is visible from
//! inside the process. Both came out of walking the tree from outside, which is
//! what `scripts/check-accessibility.sh` does.
//!
//! **A view that is not an accessibility element is never published.** An
//! `NSView` is not one by default, so a plain `an-view` given
//! `accessibilityRole="slider"` and nothing else had its role set correctly and
//! came back from the walk as nothing at all. A role AppKit can honour
//! therefore makes the view an element, and so does a label: naming something
//! is saying it is worth reaching. `accessible` overrules both, because it is
//! the prop that exists to say exactly this.
//!
//! **Overriding anything at all costs the view the role AppKit was computing
//! for it.** The moment `setAccessibilityLabel:` is called on an `NSButton`,
//! AppKit stops working that view's accessibility out and starts serving the
//! overrides — and the role, which nobody overrode, comes back `AXUnknown`. A
//! button given nothing but a better name stops being announced as a button.
//!
//! And the role cannot simply be saved and put back, because it cannot be read:
//! in-process, `NSButton.accessibilityRole()` answers `AXUnknown` while an
//! assistive client is told `AXButton`. AppKit computes it in the cell, on
//! demand, for the client. So what goes back is not a saved value but the role
//! the primitive genuinely has — an `an-button` **is** a button — which is
//! `implied_role()` below. It is a table of facts, not a guess: the same facts
//! `support.rs` already states as AppKit class names.
//!
//! Because the four props interact, they are not four independent writes: role,
//! state and `accessible` decide together what the view publishes. Rather than
//! ordering them —props of one frame arrive in whatever order the template
//! wrote them— everything is kept and everything is rewritten on every change,
//! which is what the iOS host does with its mask and for the same reason.
//!
//! **And what AppKit does not have is said out loud.** `summary` has no role
//! —it is a VoiceOver-on-iOS idea, the element that sums a screen up— and
//! `busy` has no property: the `NSAccessibility` protocol does not carry one.
//! Neither is rounded to the one next door.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use an_core::accessibility::{parse_state, Checked, Role, State};
use an_core::{NodeId, NodeKind, PropValue};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityBusyIndicatorRole, NSAccessibilityButtonRole,
    NSAccessibilityCheckBoxRole, NSAccessibilityHeadingRole, NSAccessibilityImageRole,
    NSAccessibilityLinkRole,
    NSAccessibilityPopUpButtonRole, NSAccessibilityProgressIndicatorRole,
    NSAccessibilityRadioButtonRole, NSAccessibilityRole, NSAccessibilityScrollAreaRole,
    NSAccessibilitySearchFieldSubrole, NSAccessibilitySliderRole, NSAccessibilityStaticTextRole,
    NSAccessibilitySubrole, NSAccessibilitySwitchSubrole, NSAccessibilityTextAreaRole,
    NSAccessibilityTextFieldRole, NSAccessibilityUnknownRole, NSView,
};
use objc2_foundation::{NSArray, NSNumber, NSString};

/// A role plus, where AppKit needs one, its subrole.
type AppKitRole = (&'static NSAccessibilityRole, Option<&'static NSAccessibilitySubrole>);

/// The six props of the contract.
pub fn handles(key: &str) -> bool {
    matches!(
        key,
        "accessibilityLabel"
            | "accessibilityHint"
            | "accessibilityRole"
            | "accessibilityValue"
            | "accessibilityState"
            | "accessible"
    )
}

/// What the template asked for on one node.
#[derive(Default)]
struct Entry {
    role: Option<Role>,
    state: State,
    /// What `[accessible]` said. `None` is "said nothing", which is not the
    /// same as `Some(false)`.
    accessible: Option<bool>,
    /// Whether the value sitting on the view was written by the template.
    /// `checked` only fills a value nobody claimed.
    explicit_value: bool,
    /// Whether the template gave this view a name. A name is a reason to be a
    /// stop.
    labelled: bool,
    /// Whether the template ever mentioned the state. Without it, a node that
    /// never said `checked` would have its value cleared anyway, and clearing
    /// a value is not the same as leaving it alone.
    state_touched: bool,
}

#[derive(Default)]
pub struct Accessibility {
    nodes: HashMap<NodeId, Entry>,
}

impl Accessibility {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn forget(&mut self, id: NodeId) {
        self.nodes.remove(&id);
    }

    pub fn apply(
        &mut self,
        id: NodeId,
        view: &Retained<NSView>,
        kind: NodeKind,
        key: &str,
        value: &PropValue,
    ) {
        let text = value.as_str().filter(|s| !s.is_empty());
        let entry = self.nodes.entry(id).or_default();

        match key {
            // An empty string or a null does not write an empty name: it
            // removes ours. What it cannot do is bring back the one AppKit was
            // computing — see the note at the top — so `write()` supplies the
            // primitive's own role either way.
            "accessibilityLabel" => {
                view.setAccessibilityLabel(text.map(NSString::from_str).as_deref());
                entry.labelled = text.is_some();
            }
            // The hint on iOS is the help on macOS: both are what is read
            // *after* the name and only when the name is not enough. AppKit
            // has no other — `AXHelp` is what the help tag shows and what
            // VoiceOver announces last.
            "accessibilityHint" => {
                view.setAccessibilityHelp(text.map(NSString::from_str).as_deref());
            }
            "accessibilityValue" => match text {
                Some(v) => {
                    entry.explicit_value = true;
                    let value = NSString::from_str(v);
                    unsafe { view.setAccessibilityValue(Some(&value)) };
                }
                None => entry.explicit_value = false,
            },
            "accessibilityRole" => match text {
                Some(raw) => match Role::parse(raw) {
                    Some(role) => entry.role = Some(role),
                    None => {
                        // A typo in the template. Nothing is changed: applying
                        // `none` would hide it under something that looks like
                        // it works.
                        warn_once(
                            &format!("role:{raw}"),
                            &format!(
                                "`[accessibilityRole]=\"{raw}\"` on <{kind:?}> is none of the \
                                 roles in the contract; the role stays as it was"
                            ),
                        );
                        return;
                    }
                },
                None => entry.role = None,
            },
            "accessibilityState" => {
                let raw = value.as_str().unwrap_or("{}");
                let (state, unknown) = parse_state(raw);
                for u in unknown {
                    warn_once(
                        &format!("state:{}", u.key),
                        &format!(
                            "`[accessibilityState]` on <{kind:?}> carries `{}: {}`, which is not \
                             in the contract; that key was not applied",
                            u.key, u.value
                        ),
                    );
                }
                entry.state = state;
                entry.state_touched = true;
            }
            "accessible" => entry.accessible = flag(value),
            _ => return,
        }

        self.write(id, view, kind);
    }

    /// Writes everything the props add up to, whole.
    fn write(&mut self, id: NodeId, view: &Retained<NSView>, kind: NodeKind) {
        let Some(entry) = self.nodes.get(&id) else { return };

        // --- the role
        let role = match entry.role {
            // No prop: whatever this primitive is. Not a saved value — see the
            // note at the top of the file.
            None => implied_role(kind),
            // `none` is a role, and the role it names is no role. `AXUnknown`
            // is AppKit's `android.view.View`: the answer that takes the role
            // away instead of handing one back. The element is still
            // published, so a name on it is still read; it is just announced
            // as nothing in particular.
            Some(Role::None) => Some((unsafe { NSAccessibilityUnknownRole }, None)),
            Some(asked) => match role_of(asked) {
                Some(found) => Some(found),
                None => {
                    warn_once(
                        &format!("role-without-role:{}", asked.name()),
                        &format!(
                            "`[accessibilityRole]=\"{}\"` on <{kind:?}>: AppKit has no role for \
                             that, so it is not applied. What this primitive already is stays",
                            asked.name()
                        ),
                    );
                    implied_role(kind)
                }
            },
        };
        if let Some((name, subrole)) = role {
            view.setAccessibilityRole(Some(name));
            view.setAccessibilitySubrole(subrole);
        }

        // --- whether it is a stop at all
        //
        // Anything AppKit can name is worth reaching; so is anything the
        // template named. `accessible` overrules both, in either direction.
        let named = entry.labelled || role.is_some();
        view.setAccessibilityElement(entry.accessible.unwrap_or(named));

        // --- and whether its own are stops too
        //
        // Both `true` and `false` cut the branch, and that is not a
        // coincidence: `true` says "this is **one** element", so what is inside
        // stops being separate stops, and `false` says "this is decorative", so
        // it stops too. Dropping the element on its own would not do it —
        // AppKit keeps publishing the children, and a decorative box with a
        // label inside would still be a stop.
        if entry.accessible.is_some() {
            unsafe { view.setAccessibilityChildren(Some(&NSArray::new())) };
        }

        // --- how it reads
        let state = entry.state;
        if let Some(disabled) = state.disabled {
            view.setAccessibilityEnabled(!disabled);
        }
        if let Some(selected) = state.selected {
            view.setAccessibilitySelected(selected);
        }
        if let Some(expanded) = state.expanded {
            view.setAccessibilityExpanded(expanded);
        }
        if state.busy.is_some() {
            // AppKit does not carry it. The `NSAccessibility` protocol has no
            // "busy" property at all: the closest thing is the
            // `AXBusyIndicator` role, which is *a spinner* — a different view,
            // not a state of this one. Setting it would turn a button into a
            // spinner.
            warn_once(
                "state-without-shape:busy",
                &format!(
                    "`[accessibilityState]` on <{kind:?}> asks for `busy`: the NSAccessibility \
                     protocol has no property for that, so it is not applied"
                ),
            );
        }

        // --- `checked`, in AppKit's shape: the value, as a number
        //
        // It is what a macOS checkbox does —an `NSButton` of switch type
        // publishes `AXValue` 0, 1 or 2— and that is why the halfway one fits
        // here and does not fit in UIKit. Only written over a value nobody
        // claimed: what the template wrote always wins.
        if entry.state_touched && !entry.explicit_value {
            match state.checked {
                Some(checked) => {
                    let number = NSNumber::new_i64(match checked {
                        Checked::No => 0,
                        Checked::Yes => 1,
                        Checked::Mixed => 2,
                    });
                    unsafe { view.setAccessibilityValue(Some(&number)) };
                }
                None => unsafe { view.setAccessibilityValue(None) },
            }
        }
    }
}

/// What each primitive already is, in AppKit's vocabulary.
///
/// This is not a fallback for a missing role: it is the role the view would
/// have had if nothing here had ever touched it, and it has to be written back
/// by hand because touching a view at all stops AppKit computing it. An
/// `an-button` mounts an `NSButton`, and an `NSButton` is an `AXButton` — the
/// same fact `support.rs` states as a class name.
///
/// `None` is for the primitives that genuinely have no role. A container is not
/// an `AXGroup` unless somebody says it is one: `AXGroup` is a thing a reader
/// announces, and announcing every box would make a screen unusable.
fn implied_role(kind: NodeKind) -> Option<AppKitRole> {
    unsafe {
        Some(match kind {
            NodeKind::Text => (NSAccessibilityStaticTextRole, None),
            NodeKind::Image | NodeKind::Icon => (NSAccessibilityImageRole, None),
            NodeKind::Button => (NSAccessibilityButtonRole, None),
            NodeKind::Switch => (NSAccessibilityCheckBoxRole, Some(NSAccessibilitySwitchSubrole)),
            NodeKind::Slider => (NSAccessibilitySliderRole, None),
            NodeKind::TextInput => (NSAccessibilityTextFieldRole, None),
            NodeKind::TextEditor => (NSAccessibilityTextAreaRole, None),
            NodeKind::SearchBar => {
                (NSAccessibilityTextFieldRole, Some(NSAccessibilitySearchFieldSubrole))
            }
            NodeKind::Picker => (NSAccessibilityPopUpButtonRole, None),
            NodeKind::ScrollView => (NSAccessibilityScrollAreaRole, None),
            // The two mount the same class and AppKit still gives them
            // different roles, because what it looks at is the style: a bar
            // publishes `AXProgressIndicator` and a spinner publishes
            // `AXBusyIndicator`. Writing the bar's back for both is how a
            // spinner given nothing but a name stopped being a spinner and
            // started announcing progress it does not have. The walk from
            // outside says it plainly: `AXBusyIndicator` while nothing was
            // overridden, `AXProgressIndicator` the moment a label was.
            NodeKind::ProgressBar => (NSAccessibilityProgressIndicatorRole, None),
            NodeKind::ActivityIndicator => (NSAccessibilityBusyIndicatorRole, None),
            // Everything else is a box, a presentation the system owns, or a
            // control whose AppKit role is not one of the ones this project can
            // state as a fact. Nothing is written, and AppKit keeps whatever it
            // was going to say.
            _ => return None,
        })
    }
}

/// The role —and the subrole, where one is needed— that each role of the
/// contract gets.
///
/// The subrole is not decoration: in AppKit a search field *is* an
/// `AXTextField`, and the only thing telling it apart from any other field is
/// the `AXSearchField` subrole. Same for the switch, which is an `AXCheckBox`
/// with subrole `AXSwitch`. Setting only the role would leave both
/// indistinguishable from what they are not.
fn role_of(role: Role) -> Option<AppKitRole> {
    unsafe {
        Some(match role {
            Role::Button => (NSAccessibilityButtonRole, None),
            Role::Link => (NSAccessibilityLinkRole, None),
            Role::Header => (NSAccessibilityHeadingRole, None),
            Role::Image => (NSAccessibilityImageRole, None),
            Role::Text => (NSAccessibilityStaticTextRole, None),
            Role::Checkbox => (NSAccessibilityCheckBoxRole, None),
            Role::Radio => (NSAccessibilityRadioButtonRole, None),
            Role::Switch => (NSAccessibilityCheckBoxRole, Some(NSAccessibilitySwitchSubrole)),
            Role::Slider => (NSAccessibilitySliderRole, None),
            Role::Search => {
                (NSAccessibilityTextFieldRole, Some(NSAccessibilitySearchFieldSubrole))
            }
            // AppKit **has nothing** for this. `summary` is a VoiceOver-on-iOS
            // idea: the element read on its own when you enter a screen, to sum
            // it up. On a Mac there is no such moment —you do not "enter" a
            // window— and there is neither a role nor a subrole like it. It is
            // said, not invented.
            Role::Summary => return None,
            // Handled before this is called: it is not a role to look up, it is
            // the absence of one.
            Role::None => return None,
        })
    }
}

/// `accessible` can arrive as a boolean or as the string of an `[attr.]`.
fn flag(value: &PropValue) -> Option<bool> {
    match value {
        PropValue::Bool(b) => Some(*b),
        PropValue::Str(s) if s == "true" => Some(true),
        PropValue::Str(s) if s == "false" => Some(false),
        _ => None,
    }
}

thread_local! {
    static SAID: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

fn warn_once(key: &str, message: &str) {
    SAID.with(|said| {
        if said.borrow_mut().insert(key.to_owned()) {
            eprintln!("angular-native: {message}");
        }
    });
}
