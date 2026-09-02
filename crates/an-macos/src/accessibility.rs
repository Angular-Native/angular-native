//! The six accessibility props of the contract, on AppKit.
//!
//! **AppKit's roles are a different set.** Not UIKit's with other names: in
//! UIKit a role is one bit of a mask and several things coexist —"button" and
//! "selected" at once—; here a role is *one* string, a view has exactly one,
//! and what in UIKit are state traits are separate properties of the
//! `NSAccessibility` protocol: `setAccessibilityEnabled:`,
//! `setAccessibilitySelected:`, `setAccessibilityExpanded:`.
//!
//! **A role is not enough to be read.** This is the part that only showed up
//! once the tree was walked from outside: AppKit publishes a view to an
//! assistive client only if it is an accessibility element, and an `NSView` is
//! not one by default. A plain `an-view` given `accessibilityRole="slider"`
//! and nothing else came back from the walk as *nothing at all* — the role was
//! set, the reader never saw it. So a real role also makes the view an
//! element, unless the template said `accessible="false"`, in which case the
//! template wins.
//!
//! Because of that the six props are not six independent writes: the role, the
//! state and `accessible` decide together what the view publishes. Rather than
//! ordering them —props of one frame arrive in whatever order the template
//! wrote them— everything is kept and everything is rewritten on every change,
//! which is what the iOS host does with its mask and for the same reason.
//!
//! What it shares with iOS is the rule of not overwriting what is already
//! right. An `NSButton` arrives with role `AXButton` and with its title as
//! label, both put there by AppKit; an `NSSwitch`, with `AXCheckBox` and
//! subrole `AXSwitch`. So an empty label **removes** ours instead of writing
//! an empty one —and then the system's comes back— and the system role is
//! saved the first time it has to be overwritten, and returns with `none`.
//!
//! **And what AppKit does not have is said out loud.** `summary` has no role
//! —it is a VoiceOver-on-iOS idea, the element that sums a screen up— and
//! `busy` has no property: the `NSAccessibility` protocol does not carry one.
//! Neither is rounded to the one next door.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use an_core::accessibility::{parse_state, Checked, Role, State};
use an_core::{NodeId, PropValue};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityButtonRole, NSAccessibilityCheckBoxRole,
    NSAccessibilityHeadingRole, NSAccessibilityImageRole, NSAccessibilityLinkRole,
    NSAccessibilityRadioButtonRole, NSAccessibilityRole, NSAccessibilitySearchFieldSubrole,
    NSAccessibilitySliderRole, NSAccessibilityStaticTextRole, NSAccessibilitySubrole,
    NSAccessibilitySwitchSubrole, NSAccessibilityTextFieldRole, NSView,
};
use objc2_foundation::{NSArray, NSNumber, NSString};

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

/// What the template asked for on one node, and what the system had there
/// before anyone asked for anything.
struct Entry {
    role: Option<Role>,
    state: State,
    /// What `[accessible]` said. `None` is "said nothing", which is not the
    /// same as `Some(false)`.
    accessible: Option<bool>,
    /// Whether the value sitting on the view was written by the template.
    /// `checked` only fills a value nobody claimed.
    explicit_value: bool,
    /// Whether the template ever said anything about the role, and about the
    /// state.
    ///
    /// Without these, a node that only sets `[accessibilityState]` would still
    /// have its role written —with the very role AppKit gave it, which sounds
    /// harmless and is not: writing a role at all replaces AppKit's own
    /// computation with a fixed answer, and a view whose role was pinned to
    /// nothing drops out of the tree and takes its window with it. That is not
    /// a guess; it is what the outside walk showed, and it is the difference
    /// between "we set the same value" and "we did not touch it".
    role_touched: bool,
    state_touched: bool,
    /// The role and the element flag AppKit gave the view, read the first time
    /// either was about to be overwritten. They are what comes back when the
    /// template stops asking.
    base_role: Option<Retained<NSAccessibilityRole>>,
    base_element: bool,
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
        kind: &str,
        key: &str,
        value: &PropValue,
    ) {
        let text = value.as_str().filter(|s| !s.is_empty());
        // The label and the hint interact with nothing, so they are written
        // where they arrive. The other four are settled together below.
        match key {
            "accessibilityLabel" => {
                view.setAccessibilityLabel(text.map(NSString::from_str).as_deref());
                return;
            }
            // The hint on iOS is the help on macOS: both are what is read
            // *after* the name and only when the name is not enough. AppKit
            // has no other: `AXHelp` is what the help tag shows and what
            // VoiceOver announces last.
            "accessibilityHint" => {
                view.setAccessibilityHelp(text.map(NSString::from_str).as_deref());
                return;
            }
            _ => {}
        }

        let entry = self.nodes.entry(id).or_insert_with(|| Entry {
            role: None,
            state: State::default(),
            accessible: None,
            explicit_value: false,
            role_touched: false,
            state_touched: false,
            base_role: view.accessibilityRole(),
            base_element: view.isAccessibilityElement(),
        });

        match key {
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
                    Some(role) => {
                        entry.role = Some(role);
                        entry.role_touched = true;
                    }
                    None => {
                        warn_once(
                            &format!("role:{raw}"),
                            &format!(
                                "`[accessibilityRole]=\"{raw}\"` on <{kind}> is none of the roles \
                                 in the contract; the role stays as it was"
                            ),
                        );
                        return;
                    }
                },
                None => {
                    entry.role = None;
                    entry.role_touched = true;
                }
            },
            "accessibilityState" => {
                let raw = value.as_str().unwrap_or("{}");
                let (state, unknown) = parse_state(raw);
                for u in unknown {
                    warn_once(
                        &format!("state:{}", u.key),
                        &format!(
                            "`[accessibilityState]` on <{kind}> carries `{}: {}`, which is not in \
                             the contract; that key was not applied",
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

    /// Writes everything the four interacting props add up to.
    ///
    /// Whole and not in pieces because they decide together: the role settles
    /// what the view *is*, `accessible` settles whether it is a stop and
    /// whether its own are, and the state settles how it reads. Writing one at
    /// a time would make the outcome depend on the order the template happened
    /// to list them in.
    fn write(&mut self, id: NodeId, view: &Retained<NSView>, kind: &str) {
        let Some(entry) = self.nodes.get(&id) else { return };

        // --- the role
        //
        // Only if the template ever said anything about it. Writing back the
        // role AppKit already had is not a no-op: it pins it.
        if entry.role_touched {
            match entry.role.filter(|r| *r != Role::None) {
                None => {
                    view.setAccessibilityRole(entry.base_role.as_deref());
                    view.setAccessibilitySubrole(None);
                }
                Some(role) => match role_of(role) {
                    Some((name, subrole)) => {
                        view.setAccessibilityRole(Some(name));
                        view.setAccessibilitySubrole(subrole);
                    }
                    None => {
                        warn_once(
                            &format!("role-without-role:{}", role.name()),
                            &format!(
                                "`[accessibilityRole]=\"{}\"` on <{kind}>: AppKit has no role \
                                 for that, so it is not applied. The one the system gave it stays",
                                role.name()
                            ),
                        );
                    }
                },
            }
        }

        // --- whether it is a stop at all
        //
        // A role the template asked for and AppKit can honour is a statement
        // that this is something, and something is read: that is what makes
        // the view an element. `accessible` overrules it in both directions,
        // because it is the prop that exists to say exactly this. And with
        // neither of the two, nothing is written: an element flag pinned to
        // the value it already had is still a pin.
        let has_role = entry.role.filter(|r| *r != Role::None).and_then(role_of).is_some();
        match entry.accessible {
            Some(explicit) => view.setAccessibilityElement(explicit),
            None if has_role => view.setAccessibilityElement(true),
            None if entry.role_touched => view.setAccessibilityElement(entry.base_element),
            None => {}
        }

        // --- and whether its own are stops too
        //
        // Both `true` and `false` cut the branch, and that is not a
        // coincidence: `true` says "this is **one** element", so what is
        // inside stops being separate stops, and `false` says "this is
        // decorative", so it stops too. Dropping the element on its own would
        // not do it — AppKit keeps publishing the children, and a decorative
        // view with a label inside would still be a stop.
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
            // `AXBusyIndicator` role, which is *a spinner*, not a state of
            // some other view. Setting that role would turn a button into a
            // spinner.
            warn_once(
                "state-without-shape:busy",
                &format!(
                    "`[accessibilityState]` on <{kind}> asks for `busy`: the NSAccessibility \
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

/// The role —and the subrole, where one is needed— that each one gets.
///
/// The subrole is not decoration: in AppKit a search field *is* an
/// `AXTextField`, and what tells it apart from any other field is the
/// `AXSearchField` subrole. Same for the switch, which is an `AXCheckBox` with
/// subrole `AXSwitch`. Setting only the role would leave both
/// indistinguishable from what they are not.
fn role_of(
    role: Role,
) -> Option<(&'static NSAccessibilityRole, Option<&'static NSAccessibilitySubrole>)> {
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
            // idea: the element read on its own when you enter a screen, to
            // sum it up. On a Mac there is no such moment —you do not "enter"
            // a window— and there is neither a role nor a subrole like it. It
            // is said, not invented.
            Role::Summary => return None,
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
