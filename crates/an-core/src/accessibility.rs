//! The accessibility vocabulary of the contract, understood once.
//!
//! It lives in the core for the same reason `icons.rs` does: there are four
//! Apple hosts —UIKit for the phone, the TV and the headset, AppKit for the
//! desktop, SwiftUI for the watch— and if each read the JSON on its own there
//! would be four places where `checked: "mixed"` could stop meaning the same
//! thing.
//!
//! What does **not** live here is the translation to each platform. A role has
//! no common translation: in UIKit it is one bit of a mask, in AppKit a role
//! string, in SwiftUI an `AccessibilityTraits`. The three sets agree on
//! neither the names nor the count, so each host carries its own table — and
//! that is also where it says what has no equivalent.
//!
//! The state travels as JSON and not as five separate props because that is
//! how the contract declares it, `NativeAccessibilityState`, and because the
//! bridge protocol knows nothing about objects: `PropValue` is null, bool,
//! number, string or color, and nothing else.

/// What this is for someone who cannot see it. The contract's `NativeRole`,
/// word for word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Button,
    Link,
    Header,
    Image,
    Text,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Search,
    Summary,
    /// "This view claims no role." Not the same as not sending the prop: it is
    /// an order to drop whatever was set before and hand the view back the
    /// role the system gave it.
    None,
}

impl Role {
    /// The role that string names, or nothing if it is none of the twelve.
    ///
    /// It returns an `Option` and not a default on purpose: a typo in the
    /// template —`"heading"` for `"header"`— has to be sayable, and an
    /// `unwrap_or(Role::None)` would turn it into a role that gets applied.
    pub fn parse(raw: &str) -> Option<Role> {
        Some(match raw {
            "button" => Role::Button,
            "link" => Role::Link,
            "header" => Role::Header,
            "image" => Role::Image,
            "text" => Role::Text,
            "checkbox" => Role::Checkbox,
            "radio" => Role::Radio,
            "switch" => Role::Switch,
            "slider" => Role::Slider,
            "search" => Role::Search,
            "summary" => Role::Summary,
            "none" => Role::None,
            _ => return None,
        })
    }

    /// The name it travelled under, so a warning can say it.
    pub fn name(self) -> &'static str {
        match self {
            Role::Button => "button",
            Role::Link => "link",
            Role::Header => "header",
            Role::Image => "image",
            Role::Text => "text",
            Role::Checkbox => "checkbox",
            Role::Radio => "radio",
            Role::Switch => "switch",
            Role::Slider => "slider",
            Role::Search => "search",
            Role::Summary => "summary",
            Role::None => "none",
        }
    }

    /// All twelve, in the contract's order. The checks walk the whole
    /// vocabulary through this instead of copying it.
    pub const ALL: &'static [Role] = &[
        Role::Button,
        Role::Link,
        Role::Header,
        Role::Image,
        Role::Text,
        Role::Checkbox,
        Role::Radio,
        Role::Switch,
        Role::Slider,
        Role::Search,
        Role::Summary,
        Role::None,
    ];
}

/// Checked, unchecked, or halfway.
///
/// The third exists because the contract has it and because AppKit can express
/// it; UIKit cannot, and the iOS host says so rather than rounding it to one
/// of the other two.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Checked {
    Yes,
    No,
    Mixed,
}

/// How it is right now, for someone who cannot see it. The contract's
/// `NativeAccessibilityState`.
///
/// Every field is an `Option` and not a `bool`: "the template said nothing"
/// and "the template said no" are different things. The first leaves whatever
/// state the system view had; the second clears it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct State {
    pub disabled: Option<bool>,
    pub selected: Option<bool>,
    pub checked: Option<Checked>,
    pub expanded: Option<bool>,
    pub busy: Option<bool>,
}

impl State {
    /// Whether it asks for nothing. An incoming `{}` is not an error, but
    /// there is nothing to touch on the view either.
    pub fn is_empty(&self) -> bool {
        *self == State::default()
    }
}

/// Something the JSON carried that the contract does not have.
///
/// It comes back separately instead of aborting the whole parse: one stray key
/// cannot invalidate the four that were fine, but it cannot pass in silence
/// either. The caller says it once and carries on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unknown {
    pub key: String,
    /// The value exactly as it arrived, so the warning can show what was
    /// written.
    pub value: String,
}

/// Reads the `accessibilityState` out of the JSON it travels in.
///
/// It does not pull in a whole JSON parser for this, same as
/// `parse_string_list` in the hosts and for the same reason: the only thing to
/// understand is what the JS side generates out of a flat object of five keys,
/// each one `true`, `false` or `"mixed"`.
///
/// It also returns what it could not read. A `{"cheked": true}` typo lands
/// here as unknown and leaves through the host's log, which is the only way a
/// template typo does not become a prop that does nothing.
pub fn parse_state(raw: &str) -> (State, Vec<Unknown>) {
    let mut state = State::default();
    let mut unknown = Vec::new();
    for (key, value) in entries(raw) {
        let flag = match value.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        let ok = match (key.as_str(), flag) {
            ("disabled", Some(v)) => {
                state.disabled = Some(v);
                true
            }
            ("selected", Some(v)) => {
                state.selected = Some(v);
                true
            }
            ("expanded", Some(v)) => {
                state.expanded = Some(v);
                true
            }
            ("busy", Some(v)) => {
                state.busy = Some(v);
                true
            }
            ("checked", Some(v)) => {
                state.checked = Some(if v { Checked::Yes } else { Checked::No });
                true
            }
            ("checked", None) if value == "\"mixed\"" || value == "mixed" => {
                state.checked = Some(Checked::Mixed);
                true
            }
            _ => false,
        };
        if !ok {
            unknown.push(Unknown { key, value });
        }
    }
    (state, unknown)
}

/// `key: value` pairs of a flat object, with no nesting and no odd escapes.
///
/// The value comes out with its quotes still on so the warning can show
/// exactly what arrived: `"mixed"` with quotes is a legitimate value and
/// `mixed` without them is not, and whoever reads the warning has to be able
/// to tell them apart.
fn entries(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = raw.chars().peekable();
    loop {
        // The key: the next quoted string.
        let mut key = String::new();
        loop {
            match chars.next() {
                Some('"') => break,
                Some(_) => continue,
                None => return out,
            }
        }
        loop {
            match chars.next() {
                Some('"') => break,
                Some('\\') => {
                    if let Some(escaped) = chars.next() {
                        key.push(escaped);
                    }
                }
                Some(c) => key.push(c),
                None => return out,
            }
        }
        // The colon. Without it, that was not a key.
        loop {
            match chars.peek() {
                Some(':') => {
                    chars.next();
                    break;
                }
                Some(c) if c.is_whitespace() => {
                    chars.next();
                }
                _ => return out,
            }
        }
        // The value: up to the comma or the closing brace, quotes included.
        let mut value = String::new();
        let mut in_string = false;
        loop {
            match chars.peek() {
                Some('"') => {
                    in_string = !in_string;
                    value.push('"');
                    chars.next();
                }
                Some(',') | Some('}') if !in_string => {
                    chars.next();
                    break;
                }
                Some(c) => {
                    value.push(*c);
                    chars.next();
                }
                None => break,
            }
        }
        out.push((key, value.trim().to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_five_keys_of_the_contract() {
        let json =
            r#"{"disabled":true,"selected":false,"checked":"mixed","expanded":true,"busy":false}"#;
        let (state, unknown) = parse_state(json);
        assert_eq!(state.disabled, Some(true));
        assert_eq!(state.selected, Some(false));
        assert_eq!(state.checked, Some(Checked::Mixed));
        assert_eq!(state.expanded, Some(true));
        assert_eq!(state.busy, Some(false));
        assert!(unknown.is_empty());
    }

    #[test]
    fn a_boolean_checked_is_not_the_mixed_one() {
        assert_eq!(parse_state(r#"{"checked":true}"#).0.checked, Some(Checked::Yes));
        assert_eq!(parse_state(r#"{"checked":false}"#).0.checked, Some(Checked::No));
    }

    #[test]
    fn what_is_not_in_the_contract_comes_back_apart() {
        // Would catch: a typo in the template swallowed without a word.
        let (state, unknown) = parse_state(r#"{"cheked":true,"selected":true}"#);
        assert_eq!(state.selected, Some(true));
        assert_eq!(state.checked, None);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].key, "cheked");
    }

    #[test]
    fn a_value_the_contract_does_not_admit_does_not_pass_either() {
        // `expanded` exists, but `"yes"` is not one of its values.
        let (state, unknown) = parse_state(r#"{"expanded":"yes"}"#);
        assert_eq!(state.expanded, None);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].value, "\"yes\"");
    }

    #[test]
    fn the_empty_object_asks_for_nothing() {
        let (state, unknown) = parse_state("{}");
        assert!(state.is_empty());
        assert!(unknown.is_empty());
    }

    #[test]
    fn whitespace_in_the_json_changes_nothing() {
        let (state, unknown) = parse_state("{ \"disabled\" : true , \"busy\" : true }");
        assert_eq!(state.disabled, Some(true));
        assert_eq!(state.busy, Some(true));
        assert!(unknown.is_empty());
    }

    #[test]
    fn the_twelve_roles_of_the_contract_are_recognised() {
        for role in Role::ALL {
            assert_eq!(Role::parse(role.name()), Some(*role));
        }
    }

    #[test]
    fn an_invented_role_is_not_recognised() {
        // Would catch: `"heading"` for `"header"` applied as if nothing.
        assert_eq!(Role::parse("heading"), None);
        assert_eq!(Role::parse(""), None);
    }
}
