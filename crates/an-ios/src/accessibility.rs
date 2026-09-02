//! The six accessibility props of the contract, on UIKit.
//!
//! It serves the three families that mount this host: the phone, the TV and
//! the headset. There is nothing to split by family —`UIAccessibility` is the
//! same category on `NSObject` in all three— so there is not a single `cfg`
//! here.
//!
//! **The role is a bit mask, not a value.** That is what makes this file
//! different from the AppKit one. `accessibilityTraits` is a `u64` where every
//! trait is a bit, and three kinds of thing share that word: what the view
//! *is* —button, header, image—, what the view *is like* right now —not
//! enabled, selected— and what the view *does* —plays sound, turns pages—. The
//! contract splits them into `accessibilityRole` and `accessibilityState`, so
//! here they have to be put back together, and it has to be done whole every
//! time: writing one bit means knowing the other sixty-three.
//!
//! That is why role and state are kept and the mask is rebuilt:
//!
//! ```text
//!   traits = base_or_role  |  state bits
//! ```
//!
//! **And what `base` is.** A `UIButton` already ships with `.button` on it,
//! and a `UISwitch` with its own. If the template says nothing about a role,
//! that one wins: the house rule is that only what the template really set
//! gets overwritten. So the first time a view is touched, whatever it carried
//! is saved, and that is what comes back when the template says `none` or
//! drops the prop. When a role *is* given, the role wins and replaces: a
//! template writing `accessibilityRole="link"` on an `an-button` is saying
//! this reads as a link, not as "link, button".
//!
//! **A role or a name is not enough to be read.** A `UIView` is not an
//! accessibility element by default, and neither traits nor a label on a view
//! that is not an element reach any reader: VoiceOver never stops there. It
//! shows up the moment the tree is walked from outside —a plain `an-view` with
//! `accessibilityRole="slider"` and nothing else comes back as nothing at
//! all— and it is the quietest failure in the whole area, because everything
//! was set correctly.
//!
//! So a role the platform can honour makes the view an element, and so does a
//! label: naming something is saying it is worth reaching. `accessible`
//! overrules both in either direction, because it is the prop that exists to
//! say exactly this.
//!
//! **What UIKit does not have is said out loud.** The contract's vocabulary is
//! longer than the trait set in three places —`radio`, `expanded` and `busy`—
//! and none of the three is rounded to the trait next door: it goes out
//! through the log the first time, with the name of what was asked for.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use an_core::accessibility::{parse_state, Checked, Role, State};
use an_core::{NodeId, PropValue};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::NSString;
use objc2_ui_kit::{
    NSObjectUIAccessibility, UIAccessibilityTraitAdjustable, UIAccessibilityTraitButton,
    UIAccessibilityTraitHeader, UIAccessibilityTraitImage, UIAccessibilityTraitLink,
    UIAccessibilityTraitNotEnabled, UIAccessibilityTraitSearchField,
    UIAccessibilityTraitSelected, UIAccessibilityTraitStaticText,
    UIAccessibilityTraitSummaryElement, UIAccessibilityTraitToggleButton, UIAccessibilityTraits,
    UIView,
};

/// The six props of the contract. The host asks before coming in here, so the
/// name of each one does not end up spread across two files.
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

/// What has to be remembered per view in order to rebuild its mask.
#[derive(Default)]
pub struct Accessibility {
    /// The traits the view carried from the system, read the first time its
    /// role is touched and not before: reading them when the view is created
    /// would read them before UIKit has finished configuring it.
    base: HashMap<NodeId, UIAccessibilityTraits>,
    role: HashMap<NodeId, Role>,
    state: HashMap<NodeId, State>,
    /// Nodes whose `accessibilityValue` was written by the template.
    ///
    /// Needed because `checked` also ends up in the value —that is UIKit's own
    /// convention, the one a `UISwitch` uses— and without this there would be
    /// no way to tell whether the value sitting there is the one the template
    /// asked for or one we put there for the state. With it, what the template
    /// wrote is never overwritten.
    explicit_value: HashSet<NodeId>,
    /// What `[accessible]` said, where it said anything. `None` is not the
    /// same as `Some(false)`: the first leaves the decision to the role, the
    /// second is the template taking it.
    accessible: HashMap<NodeId, bool>,
    /// Nodes the template gave a name to. A name is a reason to be a stop.
    labelled: HashSet<NodeId>,
    /// Whether the view was an accessibility element before anyone touched it.
    /// A `UIButton` already is; an `an-view` is not. It is what comes back
    /// when the template stops asking for anything.
    base_element: HashMap<NodeId, bool>,
}

impl Accessibility {
    pub fn new() -> Self {
        Self::default()
    }

    /// A node that was destroyed. Without this, a recycled id would inherit
    /// the previous one's role.
    pub fn forget(&mut self, id: NodeId) {
        self.base.remove(&id);
        self.role.remove(&id);
        self.state.remove(&id);
        self.explicit_value.remove(&id);
        self.accessible.remove(&id);
        self.labelled.remove(&id);
        self.base_element.remove(&id);
    }

    /// Applies one of the six. `kind` is only used so a warning can say which
    /// primitive the thing that could not be applied was on.
    pub fn apply(
        &mut self,
        mtm: MainThreadMarker,
        id: NodeId,
        view: &Retained<UIView>,
        kind: &str,
        key: &str,
        value: &PropValue,
    ) {
        let text = value.as_str().filter(|s| !s.is_empty());
        match key {
            // An empty string or a null does not write an empty label: it
            // removes ours. And removing it in UIKit brings back the one the
            // control ships with —a `UIButton`'s is its title— which is
            // exactly what has to happen when the template stops saying
            // anything.
            "accessibilityLabel" => {
                view.setAccessibilityLabel(text.map(NSString::from_str).as_deref(), mtm);
                if text.is_some() {
                    self.labelled.insert(id);
                } else {
                    self.labelled.remove(&id);
                }
                self.write_element(mtm, id, view);
            }
            "accessibilityHint" => {
                view.setAccessibilityHint(text.map(NSString::from_str).as_deref(), mtm);
            }
            "accessibilityValue" => {
                match text {
                    Some(v) => {
                        self.explicit_value.insert(id);
                        view.setAccessibilityValue(Some(&NSString::from_str(v)), mtm);
                    }
                    None => {
                        self.explicit_value.remove(&id);
                        // The template's one goes away and whatever came out
                        // of the state comes back, if there is a state.
                        view.setAccessibilityValue(None, mtm);
                        self.write_state_value(mtm, id, view);
                    }
                }
            }
            "accessibilityRole" => {
                match text {
                    Some(raw) => match Role::parse(raw) {
                        Some(role) => {
                            self.role.insert(id, role);
                        }
                        None => {
                            // A typo in the template. Nothing is applied:
                            // applying `none` would hide it under something
                            // that looks like it works.
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
                    None => {
                        self.role.remove(&id);
                    }
                }
                self.write_traits(mtm, id, view, kind);
                self.write_element(mtm, id, view);
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
                self.write_traits(mtm, id, view, kind);
                self.write_state_value(mtm, id, view);
                self.warn_unrepresentable(id, kind);
            }
            // `true` turns the view into **one** stop for the reader, with
            // whatever is inside read in one go. `false` hides it and its own,
            // which is what decorative content needs: without
            // `accessibilityElementsHidden` the children would still be stops,
            // and hiding only the parent hides nothing.
            "accessible" => {
                match flag(value) {
                    Some(on) => self.accessible.insert(id, on),
                    None => self.accessible.remove(&id),
                };
                self.write_element(mtm, id, view);
            }
            _ => {}
        }
    }

    /// Whether this view is a stop for the reader, and whether its own are.
    ///
    /// Three things can decide it, in this order: what `[accessible]` said,
    /// which always wins because it is the prop that exists to say exactly
    /// this; a real role, which is a statement that the view is something, and
    /// something gets read; and, failing both, whatever the view already was.
    fn write_element(&mut self, mtm: MainThreadMarker, id: NodeId, view: &Retained<UIView>) {
        let base = *self
            .base_element
            .entry(id)
            .or_insert_with(|| view.isAccessibilityElement(mtm));
        // `none` counts as a role here on purpose: a template that says "this
        // has no role" and gives it a name still wants the name read.
        let has_role =
            self.role.get(&id).is_some_and(|r| *r == Role::None || trait_of(*r).is_some());
        let named = self.labelled.contains(&id);

        let (element, hidden) = match self.accessible.get(&id).copied() {
            Some(true) => (true, false),
            Some(false) => (false, true),
            None if has_role || named => (true, false),
            None => (base, false),
        };
        // Only if it differs from what the view already reported. Writing a
        // flag the view already had is a write nobody asked for, and on the
        // AppKit side the same line cost a button its role — different API,
        // same rule.
        if element != base {
            view.setIsAccessibilityElement(element, mtm);
        }
        view.setAccessibilityElementsHidden(hidden, mtm);
    }

    /// Rebuilds the whole mask. It is the only way to write one trait without
    /// taking the others down with it.
    fn write_traits(
        &mut self,
        mtm: MainThreadMarker,
        id: NodeId,
        view: &Retained<UIView>,
        kind: &str,
    ) {
        let base = *self.base.entry(id).or_insert_with(|| view.accessibilityTraits(mtm));

        let mut traits = match self.role.get(&id).copied() {
            // No prop at all: hand the view back the traits the system gave
            // it. `none` is not this — see below.
            None => base,
            // `none` is a role, and the role it names is no role. It clears
            // the mask rather than restoring it, which is what takes away the
            // `.button` a `UIButton` was carrying. The Android host makes the
            // same split, with `android.view.View` for exactly this.
            Some(Role::None) => 0,
            Some(role) => match trait_of(role) {
                Some(bit) => bit,
                None => {
                    warn_once(
                        &format!("role-without-trait:{}", role.name()),
                        &format!(
                            "`[accessibilityRole]=\"{}\"` on <{kind}>: UIKit has no trait for \
                             that, so the role is not applied. The one the system gave it stays",
                            role.name()
                        ),
                    );
                    base
                }
            },
        };

        if let Some(state) = self.state.get(&id) {
            // The two states of the contract that in UIKit are traits and not
            // something else: `disabled` and `selected`. They go in with `|`
            // and do not replace, because a disabled button is still a button.
            traits = with_bit(traits, unsafe { UIAccessibilityTraitNotEnabled }, state.disabled);
            traits = with_bit(traits, unsafe { UIAccessibilityTraitSelected }, state.selected);
        }

        view.setAccessibilityTraits(traits, mtm);
    }

    /// `checked` in the only shape UIKit knows how to read it: the value.
    ///
    /// There is no "checked" trait. What there is, is the convention
    /// `UISwitch` itself uses —value `"1"` or `"0"`, and VoiceOver says "on"
    /// or "off" in the system's language— so that one is used and not a string
    /// of ours: a label written here would come out in English on a phone set
    /// to Japanese.
    ///
    /// Only written if the template set no value. Theirs always wins.
    fn write_state_value(&self, mtm: MainThreadMarker, id: NodeId, view: &Retained<UIView>) {
        if self.explicit_value.contains(&id) {
            return;
        }
        let checked = self.state.get(&id).and_then(|s| s.checked);
        let value = match checked {
            Some(Checked::Yes) => Some("1"),
            Some(Checked::No) => Some("0"),
            // The halfway one has no shape in UIKit. It is reported in
            // `warn_unrepresentable`, and nothing is written here, which beats
            // claiming it is checked or that it is not.
            Some(Checked::Mixed) | None => None,
        };
        view.setAccessibilityValue(value.map(NSString::from_str).as_deref(), mtm);
    }

    /// What in the state UIKit cannot say. Said once.
    fn warn_unrepresentable(&self, id: NodeId, kind: &str) {
        let Some(state) = self.state.get(&id) else { return };
        if state.checked == Some(Checked::Mixed) {
            warn_once(
                "state-without-shape:checked-mixed",
                &format!(
                    "`[accessibilityState]` on <{kind}> asks for `checked: \"mixed\"`: UIKit only \
                     knows checked and unchecked, so the value is left empty instead of being \
                     rounded to one of the two"
                ),
            );
        }
        if state.expanded.is_some() {
            warn_once(
                "state-without-shape:expanded",
                &format!(
                    "`[accessibilityState]` on <{kind}> asks for `expanded`: UIKit has neither a \
                     trait nor a property for that, so it is not applied"
                ),
            );
        }
        if state.busy.is_some() {
            warn_once(
                "state-without-shape:busy",
                &format!(
                    "`[accessibilityState]` on <{kind}> asks for `busy`: UIKit has neither a \
                     trait nor a property for that, so it is not applied"
                ),
            );
        }
    }
}

/// The trait each role gets, or nothing if UIKit has none.
///
/// `Role::None` is not here: it is not a trait, it is the order to go back to
/// the ones the view carried, and `write_traits` resolves that.
fn trait_of(role: Role) -> Option<UIAccessibilityTraits> {
    unsafe {
        Some(match role {
            Role::Button => UIAccessibilityTraitButton,
            Role::Link => UIAccessibilityTraitLink,
            Role::Header => UIAccessibilityTraitHeader,
            Role::Image => UIAccessibilityTraitImage,
            Role::Text => UIAccessibilityTraitStaticText,
            // UIKit does not tell a checkbox from a switch. Its trait is
            // "button that turns on and off", and that describes both: what
            // separates them is the drawing, and the drawing is not read out.
            Role::Checkbox | Role::Switch => UIAccessibilityTraitToggleButton,
            // A slider is what VoiceOver calls adjustable: it goes up and down
            // with the rotor gesture, and that is the trait.
            Role::Slider => UIAccessibilityTraitAdjustable,
            Role::Search => UIAccessibilityTraitSearchField,
            Role::Summary => UIAccessibilityTraitSummaryElement,
            // UIKit **has no** radio trait. There is nothing near it either: a
            // radio button in a list is announced by its value and by its
            // position in the group, and that is not a trait but a whole
            // structure. It is said, not invented.
            Role::Radio => return None,
            Role::None => return None,
        })
    }
}

/// Sets or clears a bit according to what the template said. `None` is "said
/// nothing", and then the bit stays as it was.
fn with_bit(
    traits: UIAccessibilityTraits,
    bit: UIAccessibilityTraits,
    wanted: Option<bool>,
) -> UIAccessibilityTraits {
    match wanted {
        Some(true) => traits | bit,
        Some(false) => traits & !bit,
        None => traits,
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
    /// What has already been said. One warning per frame at 60 Hz is a log
    /// nobody reads.
    static SAID: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

fn warn_once(key: &str, message: &str) {
    SAID.with(|said| {
        if said.borrow_mut().insert(key.to_owned()) {
            eprintln!("angular-native: {message}");
        }
    });
}
