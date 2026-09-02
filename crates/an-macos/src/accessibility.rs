//! Las seis props de accesibilidad del contrato, sobre AppKit.
//!
//! **Los roles de AppKit son otro juego.** No es el de UIKit con otros
//! nombres: en UIKit un rol es un bit de una máscara y varias cosas conviven
//! —«botón» y «seleccionado» a la vez—; aquí un rol es *una* cadena, la vista
//! tiene exactamente uno, y lo que en UIKit son traits de estado aquí son
//! propiedades aparte del protocolo `NSAccessibility`:
//! `setAccessibilityEnabled:`, `setAccessibilitySelected:`,
//! `setAccessibilityExpanded:`. Por eso este fichero no recompone ninguna
//! máscara y el de iOS sí.
//!
//! Lo que sí comparte con iOS es la regla de no pisar lo que ya está bien. Un
//! `NSButton` viene con rol `AXButton` y con su título de etiqueta puestos por
//! AppKit; un `NSSwitch`, con `AXCheckBox` y subrol `AXSwitch`. Escribir
//! encima sin mirar empeoraría lo que había. Así que:
//!
//! - una etiqueta vacía **quita** la nuestra en vez de escribir una vacía, y
//!   entonces vuelve la del sistema;
//! - el rol del sistema se lee y se guarda la primera vez que hace falta
//!   pisarlo, y vuelve cuando la plantilla pone `none` o retira la prop.
//!
//! **Y lo que AppKit no tiene se dice.** `summary` no tiene rol —es una idea
//! de VoiceOver en iOS, la del elemento que resume una pantalla— y `busy` no
//! tiene propiedad: el protocolo `NSAccessibility` no la lleva. Ninguna de las
//! dos se redondea a la de al lado.

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

/// Las seis props del contrato.
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
    /// El rol que AppKit le puso a la vista, guardado la primera vez que se le
    /// va a escribir uno encima. Es lo que vuelve con `none`.
    base: HashMap<NodeId, Option<Retained<NSAccessibilityRole>>>,
    /// Nodos cuyo valor lo escribió la plantilla: `checked` no lo pisa.
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
            // La pista de iOS es la ayuda de macOS: las dos son lo que se lee
            // *después* del nombre y solo cuando el nombre no basta. AppKit no
            // tiene ninguna otra: `AXHelp` es la que enseña el globo de ayuda
            // y la que VoiceOver anuncia al final.
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
                                    "`[accessibilityRole]=\"{raw}\"` en <{kind}> no es ninguno de \
                                     los roles del contrato; el rol se queda como estaba"
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
                            "`[accessibilityState]` en <{kind}> trae `{}: {}`, que no está en el \
                             contrato; esa clave no se aplicó",
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
                    // Quitar el elemento no esconde a los suyos: AppKit sigue
                    // publicando los hijos, y una vista decorativa con tres
                    // rótulos dentro seguirían siendo tres paradas. Vaciar la
                    // lista de hijos es lo que corta la rama entera, que es lo
                    // que pide el contrato.
                    view.setAccessibilityElement(false);
                    unsafe { view.setAccessibilityChildren(Some(&NSArray::new())) };
                }
                None => unsafe { view.setAccessibilityChildren(None) },
            },
            _ => {}
        }
    }

    /// Escribe el rol, guardando antes el que tenía la vista.
    fn write_role(
        &mut self,
        id: NodeId,
        view: &Retained<NSView>,
        kind: &str,
        role: Option<Role>,
    ) {
        let base = self
            .base
            .entry(id)
            .or_insert_with(|| view.accessibilityRole())
            .clone();

        // `none` y «no dijo nada» son lo mismo: devolver a la vista el rol que
        // le dio AppKit.
        let Some(role) = role.filter(|r| *r != Role::None) else {
            view.setAccessibilityRole(base.as_deref());
            view.setAccessibilitySubrole(None);
            return;
        };

        let Some((role_name, subrole)) = role_of(role) else {
            warn_once(
                &format!("role-sin-rol:{}", role.name()),
                &format!(
                    "`[accessibilityRole]=\"{}\"` en <{kind}>: AppKit no tiene ningún rol para \
                     eso, así que no se aplica. Se queda el que le dio el sistema",
                    role.name()
                ),
            );
            return;
        };

        view.setAccessibilityRole(Some(role_name));
        view.setAccessibilitySubrole(subrole);
    }

    /// Los cuatro estados que AppKit sabe decir, cada uno con su propiedad.
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
            // AppKit no lo lleva. El protocolo `NSAccessibility` no tiene
            // ninguna propiedad de «ocupado»: lo más cerca es el rol
            // `AXBusyIndicator`, que es *una rueda que gira*, no un estado de
            // otra vista. Poner ese rol convertiría un botón en una rueda.
            warn_once(
                "state-sin-forma:busy",
                &format!(
                    "`[accessibilityState]` en <{kind}> pide `busy`: el protocolo NSAccessibility \
                     no tiene ninguna propiedad para eso, así que no se aplica"
                ),
            );
        }
    }

    /// `checked` en la forma de AppKit: el valor, como número.
    ///
    /// Es lo que hace una casilla de macOS —`NSButton` de tipo `switch`
    /// publica `AXValue` 0, 1 o 2— y por eso el intermedio sí cabe aquí y en
    /// UIKit no. Solo se escribe si la plantilla no puso valor.
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

/// El rol —y el subrol, cuando hace falta— que le toca a cada uno.
///
/// El subrol no es un adorno: en AppKit un campo de búsqueda *es* un
/// `AXTextField`, y lo que lo distingue de cualquier otro campo es el subrol
/// `AXSearchField`. Lo mismo el interruptor, que es un `AXCheckBox` con subrol
/// `AXSwitch`. Poner solo el rol dejaría los dos indistinguibles de lo que no
/// son.
fn role_of(role: Role) -> Option<(&'static NSAccessibilityRole, Option<&'static NSAccessibilitySubrole>)> {
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
            Role::Search => (NSAccessibilityTextFieldRole, Some(NSAccessibilitySearchFieldSubrole)),
            // AppKit **no tiene** nada para esto. `summary` es una idea de
            // VoiceOver en iOS: el elemento que se lee solo al entrar en una
            // pantalla, para resumirla. En un Mac no hay ese momento —no se
            // «entra» en una ventana— y no hay ni rol ni subrol que se le
            // parezca. Se dice y no se inventa.
            Role::Summary => return None,
            Role::None => return None,
        })
    }
}

/// `accessible` puede llegar como booleano o como la cadena de un `[attr.]`.
fn flag(value: &PropValue) -> Option<bool> {
    match value {
        PropValue::Bool(b) => Some(*b),
        PropValue::Str(s) if s == "true" => Some(true),
        PropValue::Str(s) if s == "false" => Some(false),
        _ => None,
    }
}

thread_local! {
    static DICHO: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

fn warn_once(key: &str, message: &str) {
    DICHO.with(|dicho| {
        if dicho.borrow_mut().insert(key.to_owned()) {
            eprintln!("angular-native: {message}");
        }
    });
}
