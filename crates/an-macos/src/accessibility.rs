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
//! What it does share with iOS is the rule of not overwriting what is already
//! right. An `NSButton` arrives with role `AXButton` and with its title as
//! label, both put there by AppKit; an `NSSwitch`, with `AXCheckBox` and
//! subrole `AXSwitch`. Writing over that without looking would make what was
//! there worse. So:
//!
//! - an empty label **removes** ours instead of writing an empty one, and then
//!   the system's comes back;
//! - the system role is read and kept the first time it has to be overwritten,
//!   and comes back when the template says `none` or drops the prop.
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

#[derive(Default)]
pub struct Accessibility {
    /// The role AppKit gave the view, saved the first time one is about to be
    /// written over it. It is what comes back with `none`.
    base: HashMap<NodeId, Option<Retained<NSAccessibilityRole>>>,
    /// Nodes whose value was written by the template: `checked` does not
    /// overwrite it.
    explicit_value: HashSet<NodeId>,
    state: HashMap<NodeId, State>,
}

impl Accessibility {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn forget(&mut self, id: NodeId) {
        self.base.remove(&id);
        self.explicit_value.remove(&id);
        self.state.remove(&id);
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
        match key {
            "accessibilityLabel" => {
                view.setAccessibilityLabel(text.map(NSString::from_str).as_deref());
            }
            // The hint on iOS is the help on macOS: both are what is read
            // *after* the name and only when the name is not enough. AppKit
            // has no other: `AXHelp` is the one the help tag shows and the one
            // VoiceOver announces last.
            "accessibilityHint" => {
                view.setAccessibilityHelp(text.map(NSString::from_str).as_deref());
            }
            "accessibilityValue" => match text {
                Some(v) => {
                    self.explicit_value.insert(id);
                    let value = NSString::from_str(v);
                    unsafe { view.setAccessibilityValue(Some(&value)) };
                }
                None => {
                    self.explicit_value.remove(&id);
                    unsafe { view.setAccessibilityValue(None) };
                    self.write_state_value(id, view);
                }
            },
            "accessibilityRole" => {
                let role = match text {
                    Some(raw) => match Role::parse(raw) {
                        Some(role) => Some(role),
                        None => {
                            warn_once(
                                &format!("role:{raw}"),
                                &format!(
                                    "`[accessibilityRole]=\"{raw}\"` on <{kind}> is none of the \
                                     roles in the contract; the role stays as it was"
                                ),
                            );
                            return;
                        }
                    },
                    None => None,
                };
                self.write_role(id, view, kind, role);
            }
            "accessibilityState" => {
                let raw = value.as_str().unwrap_or("{}");
                let (state, unknown) = parse_state(raw);
                for entry in unknown {
                    warn_once(
                        &format!("state:{}", entry.key),
                        &format!(
                            "`[accessibilityState]` on <{kind}> carries `{}: {}`, which is not in \
                             the contract; that key was not applied",
                            entry.key, entry.value
                        ),
                    );
                }
                if state.is_empty() {
                    self.state.remove(&id);
                } else {
                    self.state.insert(id, state);
                }
                self.write_state(view, kind, &state);
                self.write_state_value(id, view);
            }
            "accessible" => match flag(value) {
                Some(true) => view.setAccessibilityElement(true),
                Some(false) => {
                    // Dropping the element does not hide its own: AppKit keeps
                    // publishing the children, and a decorative view with
                    // three labels inside would still be three stops. Emptying
                    // the children list is what cuts the whole branch, which
                    // is what the contract asks for.
                    view.setAccessibilityElement(false);
                    unsafe { view.setAccessibilityChildren(Some(&NSArray::new())) };
                }
                None => unsafe { view.setAccessibilityChildren(None) },
            },
            _ => {}
        }
    }

    /// Writes the role, saving first the one the view had.
    fn write_role(&mut self, id: NodeId, view: &Retained<NSView>, kind: &str, role: Option<Role>) {
        let base =
            self.base.entry(id).or_insert_with(|| view.accessibilityRole()).clone();

        // `none` and "said nothing" are the same thing: hand the view back the
        // role AppKit gave it.
        let Some(role) = role.filter(|r| *r != Role::None) else {
            view.setAccessibilityRole(base.as_deref());
            view.setAccessibilitySubrole(None);
            return;
        };

        let Some((role_name, subrole)) = role_of(role) else {
            warn_once(
                &format!("role-without-role:{}", role.name()),
                &format!(
                    "`[accessibilityRole]=\"{}\"` on <{kind}>: AppKit has no role for that, so it \
                     is not applied. The one the system gave it stays",
                    role.name()
                ),
            );
            return;
        };

        view.setAccessibilityRole(Some(role_name));
        view.setAccessibilitySubrole(subrole);
    }

    /// The three states AppKit knows how to say, each with its own property.
    fn write_state(&self, view: &Retained<NSView>, kind: &str, state: &State) {
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
    }

    /// `checked` in AppKit's shape: the value, as a number.
    ///
    /// It is what a macOS checkbox does —an `NSButton` of switch type
    /// publishes `AXValue` 0, 1 or 2— and that is why the halfway one fits
    /// here and does not fit in UIKit. Only written if the template set no
    /// value.
    fn write_state_value(&self, id: NodeId, view: &Retained<NSView>) {
        if self.explicit_value.contains(&id) {
            return;
        }
        let Some(checked) = self.state.get(&id).and_then(|s| s.checked) else { return };
        let number = NSNumber::new_i64(match checked {
            Checked::No => 0,
            Checked::Yes => 1,
            Checked::Mixed => 2,
        });
        unsafe { view.setAccessibilityValue(Some(&number)) };
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
