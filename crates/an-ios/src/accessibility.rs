//! Las seis props de accesibilidad del contrato, sobre UIKit.
//!
//! Vale para las tres familias que montan este host: el teléfono, la tele y el
//! visor. No hay nada que separar por familia —`UIAccessibility` es la misma
//! categoría sobre `NSObject` en las tres— así que aquí no hay ningún `cfg`.
//!
//! **El rol es una máscara de bits, no un valor.** Eso es lo que hace este
//! fichero distinto del de AppKit. `accessibilityTraits` es un `u64` donde
//! cada trait es un bit, y ahí dentro conviven cosas de tres clases: lo que la
//! vista *es* —botón, encabezado, imagen—, lo que la vista *está* —apagada,
//! seleccionada— y lo que la vista *hace* —suena, pasa página—. El contrato
//! los separa en `accessibilityRole` y `accessibilityState`, así que aquí hay
//! que volver a juntarlos, y hay que hacerlo entero cada vez: escribir un bit
//! suelto exige leer los otros sesenta y tres.
//!
//! Por eso el rol y el estado se guardan y la máscara se recompone:
//!
//! ```text
//!   traits = base_o_rol  |  bits del estado
//! ```
//!
//! **Y qué es `base`.** Un `UIButton` ya viene con `.button` puesto de fábrica,
//! y un `UISwitch` con lo suyo. Si la plantilla no dice nada de rol, ese es el
//! que manda: la regla de la casa es que solo se pisa lo que la plantilla haya
//! puesto de verdad. Así que la primera vez que se toca una vista se guarda lo
//! que traía, y eso es lo que vuelve cuando la plantilla pone `none` o retira
//! la prop. Cuando sí hay rol, el rol manda y sustituye: una plantilla que
//! escribe `accessibilityRole="link"` sobre un `an-button` está diciendo que
//! eso se lee como un enlace, no como «enlace, botón».
//!
//! **Lo que UIKit no tiene se dice.** El vocabulario del contrato es más largo
//! que el juego de traits en tres sitios —`radio`, `expanded` y `busy`— y en
//! ninguno de los tres se redondea al trait de al lado: sale por el registro
//! la primera vez, con el nombre de lo que se pidió.

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

/// Las seis props del contrato. El host pregunta antes de entrar aquí para no
/// repartir el nombre de cada una por dos ficheros.
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

/// Lo que hay que recordar de cada vista para poder recomponer su máscara.
#[derive(Default)]
pub struct Accessibility {
    /// Los traits que la vista traía del sistema, leídos la primera vez que se
    /// le toca el rol y no antes: leerlos al crear la vista sería leerlos
    /// antes de que UIKit haya terminado de configurarla.
    base: HashMap<NodeId, UIAccessibilityTraits>,
    role: HashMap<NodeId, Role>,
    state: HashMap<NodeId, State>,
    /// Nodos cuyo `accessibilityValue` lo escribió la plantilla.
    ///
    /// Hace falta porque `checked` también acaba en el valor —es la convención
    /// de UIKit, la misma que usa un `UISwitch`— y sin esto no habría forma de
    /// saber si el valor que hay es el que pidió la plantilla o uno que
    /// pusimos nosotros por el estado. Con esto, lo que escribió la plantilla
    /// nunca se pisa.
    explicit_value: HashSet<NodeId>,
}

impl Accessibility {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un nodo que se destruyó. Sin esto, un id reciclado heredaría el rol del
    /// anterior.
    pub fn forget(&mut self, id: NodeId) {
        self.base.remove(&id);
        self.role.remove(&id);
        self.state.remove(&id);
        self.explicit_value.remove(&id);
    }

    /// Aplica una de las seis. `kind` solo se usa para poder decir en qué
    /// primitiva estaba lo que no se pudo aplicar.
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
            // Una cadena vacía o un nulo no escriben una etiqueta vacía: la
            // quitan. Y quitarla en UIKit devuelve la que trae el control —la
            // de un `UIButton` es su título—, que es exactamente lo que hay
            // que hacer cuando la plantilla deja de decir nada.
            "accessibilityLabel" => {
                view.setAccessibilityLabel(text.map(NSString::from_str).as_deref(), mtm);
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
                        // Se quita el de la plantilla y vuelve el que saliera
                        // del estado, si hay estado.
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
                            // Una errata en la plantilla. No se aplica nada:
                            // aplicar `none` la escondería debajo de algo que
                            // parece funcionar.
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
                    None => {
                        self.role.remove(&id);
                    }
                }
                self.write_traits(mtm, id, view, kind);
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
                self.write_traits(mtm, id, view, kind);
                self.write_state_value(mtm, id, view);
                self.warn_unrepresentable(id, kind);
            }
            // `true` convierte la vista en **una** parada del lector, con lo
            // que tenga dentro leído de una vez. `false` la esconde a ella y a
            // los suyos, que es lo que hace falta para lo decorativo: sin
            // `accessibilityElementsHidden` los hijos seguirían siendo
            // paradas, y esconder solo el padre no esconde nada.
            "accessible" => match flag(value) {
                Some(true) => {
                    view.setIsAccessibilityElement(true, mtm);
                    view.setAccessibilityElementsHidden(false, mtm);
                }
                Some(false) => {
                    view.setIsAccessibilityElement(false, mtm);
                    view.setAccessibilityElementsHidden(true, mtm);
                }
                None => {
                    view.setAccessibilityElementsHidden(false, mtm);
                }
            },
            _ => {}
        }
    }

    /// Recompone la máscara entera. Es la única forma de escribir un trait sin
    /// llevarse por delante los demás.
    fn write_traits(
        &mut self,
        mtm: MainThreadMarker,
        id: NodeId,
        view: &Retained<UIView>,
        kind: &str,
    ) {
        let base = *self.base.entry(id).or_insert_with(|| view.accessibilityTraits(mtm));

        let mut traits = match self.role.get(&id).copied() {
            // `none` y «la plantilla no dijo nada» son la misma orden para la
            // máscara: devolver la vista a los traits que le dio el sistema.
            None | Some(Role::None) => base,
            Some(role) => match trait_of(role) {
                Some(bit) => bit,
                None => {
                    warn_once(
                        &format!("role-sin-trait:{}", role.name()),
                        &format!(
                            "`[accessibilityRole]=\"{}\"` en <{kind}>: UIKit no tiene ningún trait \
                             para eso, así que el rol no se aplica. Se queda el que le dio el \
                             sistema",
                            role.name()
                        ),
                    );
                    base
                }
            },
        };

        if let Some(state) = self.state.get(&id) {
            // Los dos estados del contrato que en UIKit son traits, y no otra
            // cosa: `disabled` y `selected`. Van con `|` y no sustituyen,
            // porque un botón apagado sigue siendo un botón.
            traits = with_bit(traits, unsafe { UIAccessibilityTraitNotEnabled }, state.disabled);
            traits = with_bit(traits, unsafe { UIAccessibilityTraitSelected }, state.selected);
        }

        view.setAccessibilityTraits(traits, mtm);
    }

    /// `checked` en la única forma que UIKit sabe leerlo: el valor.
    ///
    /// No hay ningún trait de «marcado». Lo que hay es la convención que usa
    /// el propio `UISwitch` —valor `"1"` o `"0"`, y VoiceOver dice «activado»
    /// o «desactivado» en el idioma del sistema—, así que se usa esa y no una
    /// cadena nuestra: una etiqueta escrita aquí saldría en castellano en un
    /// teléfono en japonés.
    ///
    /// Solo se escribe si la plantilla no puso valor. El suyo manda siempre.
    fn write_state_value(&self, mtm: MainThreadMarker, id: NodeId, view: &Retained<UIView>) {
        if self.explicit_value.contains(&id) {
            return;
        }
        let checked = self.state.get(&id).and_then(|s| s.checked);
        let value = match checked {
            Some(Checked::Yes) => Some("1"),
            Some(Checked::No) => Some("0"),
            // El intermedio no tiene forma en UIKit. Se avisa en
            // `warn_unrepresentable` y aquí no se escribe nada, que es mejor
            // que decir que está marcado o que no lo está.
            Some(Checked::Mixed) | None => None,
        };
        view.setAccessibilityValue(value.map(NSString::from_str).as_deref(), mtm);
    }

    /// Lo del estado que en UIKit no se puede decir. Se dice una vez.
    fn warn_unrepresentable(&self, id: NodeId, kind: &str) {
        let Some(state) = self.state.get(&id) else { return };
        if state.checked == Some(Checked::Mixed) {
            warn_once(
                "state-sin-forma:checked-mixed",
                &format!(
                    "`[accessibilityState]` en <{kind}> pide `checked: \"mixed\"`: UIKit solo sabe \
                     de marcado y sin marcar, así que el valor se queda vacío en vez de \
                     redondearse a uno de los dos"
                ),
            );
        }
        if state.expanded.is_some() {
            warn_once(
                "state-sin-forma:expanded",
                &format!(
                    "`[accessibilityState]` en <{kind}> pide `expanded`: UIKit no tiene trait ni \
                     propiedad para eso, así que no se aplica"
                ),
            );
        }
        if state.busy.is_some() {
            warn_once(
                "state-sin-forma:busy",
                &format!(
                    "`[accessibilityState]` en <{kind}> pide `busy`: UIKit no tiene trait ni \
                     propiedad para eso, así que no se aplica"
                ),
            );
        }
    }
}

/// El trait que le toca a cada rol, o nada si UIKit no tiene ninguno.
///
/// `Role::None` no está: no es un trait, es la orden de volver al que traía la
/// vista, y eso lo resuelve `write_traits`.
fn trait_of(role: Role) -> Option<UIAccessibilityTraits> {
    unsafe {
        Some(match role {
            Role::Button => UIAccessibilityTraitButton,
            Role::Link => UIAccessibilityTraitLink,
            Role::Header => UIAccessibilityTraitHeader,
            Role::Image => UIAccessibilityTraitImage,
            Role::Text => UIAccessibilityTraitStaticText,
            // UIKit no distingue una casilla de un interruptor. Su trait es
            // «botón que se enciende y se apaga», y esa es la descripción de
            // los dos: el que los separa es el dibujo, y el dibujo no se lee.
            Role::Checkbox | Role::Switch => UIAccessibilityTraitToggleButton,
            // Un deslizador es lo que VoiceOver llama ajustable: se sube y se
            // baja con el gesto de rueda, y eso es el trait.
            Role::Slider => UIAccessibilityTraitAdjustable,
            Role::Search => UIAccessibilityTraitSearchField,
            Role::Summary => UIAccessibilityTraitSummaryElement,
            // UIKit **no tiene** trait de radio. No hay ninguno cerca: el
            // botón de radio de una lista se anuncia por su valor y por su
            // posición en el grupo, y eso no es un trait sino una estructura
            // entera. Se dice y no se inventa.
            Role::Radio => return None,
            Role::None => return None,
        })
    }
}

/// Pone o quita un bit según lo que diga la plantilla. `None` es «no dijo
/// nada», y entonces el bit se queda como estaba.
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
    /// Lo ya dicho. Un aviso por frame a 60 Hz es un registro que no se lee.
    static DICHO: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

fn warn_once(key: &str, message: &str) {
    DICHO.with(|dicho| {
        if dicho.borrow_mut().insert(key.to_owned()) {
            eprintln!("angular-native: {message}");
        }
    });
}
