//! El vocabulario de accesibilidad del contrato, entendido una sola vez.
//!
//! Vive en el núcleo por lo mismo que `icons.rs`: hay cuatro hosts de Apple
//! —UIKit para el teléfono, la tele y el visor, AppKit para el escritorio y
//! SwiftUI para el reloj— y si cada uno leyera el JSON por su cuenta habría
//! cuatro sitios donde `checked: "mixed"` podría dejar de significar lo mismo.
//!
//! Lo que **no** vive aquí es la traducción a cada plataforma. Un rol no tiene
//! una traducción común: en UIKit es un bit de una máscara, en AppKit una
//! cadena de rol, y en SwiftUI un `AccessibilityTraits`. Los tres juegos no se
//! parecen ni en el número, así que la tabla la pone cada host y ahí se ve —y
//! ahí se dice lo que no tiene equivalente—.
//!
//! El estado viaja como JSON y no como cinco props sueltas porque así es como
//! lo declara el contrato, `NativeAccessibilityState`, y porque el protocolo
//! del puente no sabe de objetos: `PropValue` es nulo, booleano, número,
//! cadena o color, y nada más.

/// Qué es esto para quien no lo ve. Es el `NativeRole` de las primitivas,
/// palabra por palabra.
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
    /// «Esta vista no reclama ningún rol». No es lo mismo que no mandar la
    /// prop: es una orden de soltar el que se hubiera puesto antes y devolver
    /// la vista al que le dio el sistema.
    None,
}

impl Role {
    /// El rol que nombra esa cadena, o nada si no es ninguno de los doce.
    ///
    /// Devuelve `Option` y no un valor por defecto a propósito: una errata en
    /// la plantilla —`"heading"` por `"header"`— tiene que poder decirse, y
    /// con un `unwrap_or(Role::None)` se convertiría en un rol que se aplica.
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

    /// El nombre con el que viajó, para poder decirlo en un aviso.
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

    /// Los doce, en el orden del contrato. Lo usan las comprobaciones para
    /// recorrer el vocabulario entero sin copiarlo.
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

/// Marcado, sin marcar, o a medias.
///
/// El tercero existe porque lo tiene el contrato y porque AppKit lo sabe
/// expresar; UIKit no, y el host de iOS lo dice en vez de redondearlo a uno de
/// los otros dos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Checked {
    Yes,
    No,
    Mixed,
}

/// Cómo está, para quien no lo ve. Es el `NativeAccessibilityState`.
///
/// Cada campo es un `Option` y no un `bool`: «la plantilla no dijo nada» y «la
/// plantilla dijo que no» son cosas distintas. La primera deja el estado que
/// tuviera la vista del sistema; la segunda lo quita.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct State {
    pub disabled: Option<bool>,
    pub selected: Option<bool>,
    pub checked: Option<Checked>,
    pub expanded: Option<bool>,
    pub busy: Option<bool>,
}

impl State {
    /// Si no pide nada. Un `{}` que llega no es un error, pero tampoco hay que
    /// tocar la vista por él.
    pub fn is_empty(&self) -> bool {
        *self == State::default()
    }
}

/// Lo que el JSON traía y el contrato no tiene.
///
/// Sale por separado en vez de abortar el análisis entero: una clave de más no
/// puede invalidar las cuatro que sí estaban bien, pero tampoco puede pasar
/// callando. Quien llama la dice una vez y sigue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unknown {
    pub key: String,
    /// El valor tal y como venía, para que el aviso enseñe lo que se escribió.
    pub value: String,
}

/// Lee el `accessibilityState` del JSON con el que viaja.
///
/// No se trae un analizador de JSON entero para esto, igual que
/// `parse_string_list` en los hosts y por lo mismo: lo único que hay que
/// entender es lo que genera el lado JS a partir de un objeto plano de cinco
/// claves, todas con valor `true`, `false` o `"mixed"`.
///
/// Devuelve además lo que no supo leer. Un `{"cheked": true}` con errata
/// entraría aquí como desconocido y saldría por el registro del host, que es
/// la única forma de que una errata en una plantilla no se convierta en una
/// prop que no hace nada.
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

/// Parejas `clave: valor` de un objeto plano, sin anidamiento ni escapes raros.
///
/// El valor sale sin recortar comillas para que el aviso pueda enseñar
/// exactamente lo que venía: `"mixed"` con comillas es un valor legítimo y
/// `mixed` sin ellas no lo es, y quien lee el aviso tiene que poder verlo.
fn entries(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = raw.chars().peekable();
    loop {
        // La clave: la siguiente cadena entre comillas.
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
        // Los dos puntos. Si no están, esto no era una clave.
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
        // El valor: hasta la coma o el cierre, comillas incluidas.
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
    fn lee_las_cinco_claves_del_contrato() {
        let json = r#"{"disabled":true,"selected":false,"checked":"mixed","expanded":true,"busy":false}"#;
        let (state, unknown) = parse_state(json);
        assert_eq!(state.disabled, Some(true));
        assert_eq!(state.selected, Some(false));
        assert_eq!(state.checked, Some(Checked::Mixed));
        assert_eq!(state.expanded, Some(true));
        assert_eq!(state.busy, Some(false));
        assert!(unknown.is_empty());
    }

    #[test]
    fn checked_booleano_no_es_lo_mismo_que_mixto() {
        assert_eq!(parse_state(r#"{"checked":true}"#).0.checked, Some(Checked::Yes));
        assert_eq!(parse_state(r#"{"checked":false}"#).0.checked, Some(Checked::No));
    }

    #[test]
    fn lo_que_no_esta_en_el_contrato_sale_por_separado() {
        // Cazaría: que una errata en la plantilla se tragara sin decir nada.
        let (state, unknown) = parse_state(r#"{"cheked":true,"selected":true}"#);
        assert_eq!(state.selected, Some(true));
        assert_eq!(state.checked, None);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].key, "cheked");
    }

    #[test]
    fn un_valor_que_el_contrato_no_admite_tampoco_pasa() {
        // `expanded` existe, pero no vale `"sí"`.
        let (state, unknown) = parse_state(r#"{"expanded":"si"}"#);
        assert_eq!(state.expanded, None);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].value, "\"si\"");
    }

    #[test]
    fn el_objeto_vacio_no_pide_nada() {
        let (state, unknown) = parse_state("{}");
        assert!(state.is_empty());
        assert!(unknown.is_empty());
    }

    #[test]
    fn los_espacios_del_json_no_cambian_nada() {
        let (state, unknown) = parse_state("{ \"disabled\" : true , \"busy\" : true }");
        assert_eq!(state.disabled, Some(true));
        assert_eq!(state.busy, Some(true));
        assert!(unknown.is_empty());
    }

    #[test]
    fn los_doce_roles_del_contrato_se_reconocen() {
        for role in Role::ALL {
            assert_eq!(Role::parse(role.name()), Some(*role));
        }
    }

    #[test]
    fn un_rol_inventado_no_se_reconoce() {
        // Cazaría: que `"heading"` por `"header"` se aplicara como si tal cosa.
        assert_eq!(Role::parse("heading"), None);
        assert_eq!(Role::parse(""), None);
    }
}
