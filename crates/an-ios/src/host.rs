//! `HostRenderer` sobre UIKit. Una vista nativa por nodo montable, colocada
//! con `frame` directo: el layout ya lo resolvió taffy, Auto Layout aquí solo
//! añadiría un segundo motor de layout compitiendo con el primero.

use std::collections::HashMap;

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{EventQueue, HostRenderer};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use core::ptr::NonNull;
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use block2::RcBlock;
use objc2_quartz_core::CAShapeLayer;
use objc2_ui_kit::{
    UIViewAnimationOptions,
    NSLineBreakMode, NSTextAlignment, UIAccessibilityIdentification, UIActivityIndicatorView,
    UIBezierPath, UIButton, UIControlState, UIFont, UIImageView, UILabel, UIProgressView,
    UIScrollView, UISlider, UISwitch, UITextField, UITextInputTraits, UIView,
};

/// Lista de cadenas en JSON, sin traerse un analizador entero para esto.
///
/// Solo tiene que entender lo que genera el lado JS: `["uno","dos"]`, con
/// comillas escapadas si hiciera falta.
fn parse_string_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut item = String::new();
        while let Some(inner) = chars.next() {
            match inner {
                '"' => break,
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        item.push(escaped);
                    }
                }
                other => item.push(other),
            }
        }
        out.push(item);
    }
    out
}

/// Las manías del teclado de un campo o de un editor.
///
/// El mensaje se manda resolviendo antes su implementación por el runtime, y no
/// con el setter que genera objc2. No es una manía:
///
/// objc2 comprueba que el método exista antes de mandarlo —comprobación que en
/// este proyecto ha cazado firmas mal declaradas más de una vez— mirando la
/// tabla de métodos de la clase. En iOS 26, `-[UITextField setKeyboardType:]`
/// **no está en esa tabla**: UIKit lo resuelve la primera vez que alguien lo
/// pide. `respondsToSelector:` dice que sí y `class_getInstanceMethod` dice que
/// no, y objc2 hace caso al segundo y aborta el proceso. Resultado: cualquier
/// app con un campo de texto se cerraba nada más arrancar.
///
/// `class_getMethodImplementation` es la función del runtime que **provoca**
/// esa resolución, así que devuelve la implementación de verdad. Antes se
/// pregunta si el objeto responde: si algún día dejara de responder, esto lo
/// dice en vez de mandar un mensaje a ciegas.
fn apply_text_traits(traits: &objc2_foundation::NSObject, key: &str, text: Option<&str>, value: &PropValue) {
    use objc2::runtime::{NSObjectProtocol, Sel};

    let (selector, setting) = match key {
        "keyboardType" => (
            objc2::sel!(setKeyboardType:),
            match text {
                Some("numeric") => objc2_ui_kit::UIKeyboardType::NumberPad,
                Some("decimal") => objc2_ui_kit::UIKeyboardType::DecimalPad,
                Some("email") => objc2_ui_kit::UIKeyboardType::EmailAddress,
                Some("phone") => objc2_ui_kit::UIKeyboardType::PhonePad,
                Some("url") => objc2_ui_kit::UIKeyboardType::URL,
                _ => objc2_ui_kit::UIKeyboardType::Default,
            }
            .0,
        ),
        "returnKeyType" => (
            objc2::sel!(setReturnKeyType:),
            match text {
                Some("done") => objc2_ui_kit::UIReturnKeyType::Done,
                Some("go") => objc2_ui_kit::UIReturnKeyType::Go,
                Some("next") => objc2_ui_kit::UIReturnKeyType::Next,
                Some("search") => objc2_ui_kit::UIReturnKeyType::Search,
                Some("send") => objc2_ui_kit::UIReturnKeyType::Send,
                _ => objc2_ui_kit::UIReturnKeyType::Default,
            }
            .0,
        ),
        "autoCapitalize" => (
            objc2::sel!(setAutocapitalizationType:),
            match text {
                Some("none") => objc2_ui_kit::UITextAutocapitalizationType::None,
                Some("words") => objc2_ui_kit::UITextAutocapitalizationType::Words,
                Some("characters") => objc2_ui_kit::UITextAutocapitalizationType::AllCharacters,
                _ => objc2_ui_kit::UITextAutocapitalizationType::Sentences,
            }
            .0,
        ),
        _ => (
            objc2::sel!(setAutocorrectionType:),
            if matches!(value, PropValue::Bool(false)) {
                objc2_ui_kit::UITextAutocorrectionType::No
            } else {
                objc2_ui_kit::UITextAutocorrectionType::Yes
            }
            .0,
        ),
    };

    if !traits.respondsToSelector(selector) {
        eprintln!(
            "angular-native: {} no atiende {selector:?}; la prop `{key}` no se aplicó",
            traits.class().name().to_string_lossy()
        );
        return;
    }

    // `objc2` declara este símbolo sin tipos, para que cada quien le ponga los
    // suyos: es la función del runtime que resuelve el método —provocando la
    // resolución perezosa— y devuelve su implementación.
    unsafe extern "C" {
        fn class_getMethodImplementation(
            class: *const objc2::runtime::AnyClass,
            selector: Sel,
        ) -> Option<unsafe extern "C" fn()>;
    }

    // Las cuatro propiedades toman un solo `NSInteger`, así que la firma es la
    // misma para todas.
    type Setter = unsafe extern "C" fn(&objc2_foundation::NSObject, Sel, isize);
    let Some(implementation) = (unsafe { class_getMethodImplementation(traits.class(), selector) })
    else {
        return;
    };
    let setter: Setter = unsafe { std::mem::transmute(implementation) };
    unsafe { setter(traits, selector, setting) };
}

/// La vista que hay justo debajo de otra dentro de un contenedor.
fn previous_sibling(parent: &UIView, view: &UIView) -> Option<Retained<UIView>> {
    let subviews = parent.subviews().to_vec();
    let index = subviews.iter().position(|sibling| &**sibling == view)?;
    if index == 0 {
        return None;
    }
    subviews.get(index - 1).cloned()
}

/// Contorno de un rectángulo con un radio distinto por esquina.
///
/// Las esquinas van en el orden arriba-izq, arriba-der, abajo-der, abajo-izq,
/// el mismo que usa CSS y el mismo que espera Android.
fn rounded_path(width: f64, height: f64, radii: [f64; 4]) -> Retained<UIBezierPath> {
    use std::f64::consts::{FRAC_PI_2, PI};

    let limit = (width.min(height)) / 2.0;
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0.0, limit));
    let path = UIBezierPath::new();
    let point = |x: f64, y: f64| CGPoint { x, y };

    path.moveToPoint(point(tl, 0.0));
    path.addLineToPoint(point(width - tr, 0.0));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(width - tr, tr),
        tr,
        -FRAC_PI_2,
        0.0,
        true,
    );
    path.addLineToPoint(point(width, height - br));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(width - br, height - br),
        br,
        0.0,
        FRAC_PI_2,
        true,
    );
    path.addLineToPoint(point(bl, height));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(bl, height - bl),
        bl,
        FRAC_PI_2,
        PI,
        true,
    );
    path.addLineToPoint(point(0.0, tl));
    path.addArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(tl, tl),
        tl,
        PI,
        PI + FRAC_PI_2,
        true,
    );
    path.closePath();
    path
}

/// Vista nativa de un nodo. Se guarda con su tipo concreto porque las props
/// de un `<Text>` no se aplican igual que las de un `<View>`.
enum HostView {
    View(Retained<UIView>),
    Stack(Retained<UIView>),
    Label(Retained<UILabel>),
    Image(Retained<UIImageView>),
    Scroll(Retained<UIScrollView>),
    Field(Retained<UITextField>),
    /// La vista de un `UITabBarController`, que es quien dibuja la barra.
    TabsHost(Retained<UIView>),
    Toggle(Retained<UISwitch>),
    Slide(Retained<UISlider>),
    Spinner(Retained<UIActivityIndicatorView>),
    Progress(Retained<UIProgressView>),
    Button(Retained<UIButton>),
    /// Una capa por encima de todo. En iOS lo suyo sería presentar un
    /// controlador, pero aquí no hay uno por pantalla: es una vista que se
    /// monta sobre la raíz y se anima al aparecer.
    Overlay(Retained<UIView>),
    /// Un diálogo no tiene vista propia: lo presenta el sistema. Se monta una
    /// vista vacía para que el árbol tenga algo donde colgar el nodo.
    Dialog(Retained<UIView>),
    Segments(Retained<objc2_ui_kit::UISegmentedControl>),
    Step(Retained<objc2_ui_kit::UIStepper>),
    Search(Retained<objc2_ui_kit::UISearchBar>),
    /// Un desplegable: un botón que abre un menú del sistema.
    Menu(Retained<UIButton>),
    Date(Retained<objc2_ui_kit::UIDatePicker>),
    Area(Retained<objc2_ui_kit::UITextView>),
    Nav(Retained<objc2_ui_kit::UINavigationBar>),
    /// tvOS no la tiene: WebKit no forma parte de su SDK, así que ni el
    /// enlazado la encontraría. Ver `family.rs`.
    #[cfg(not(target_os = "tvos"))]
    Web(Retained<crate::web::WKWebView>),
    Map(Retained<crate::map::MKMapView>),
    Video(Retained<UIView>),
}

impl HostView {
    fn as_view(&self) -> &UIView {
        match self {
            HostView::View(v) => v,
            HostView::Stack(v) => v,
            HostView::Label(v) => v,
            HostView::Image(v) => v,
            HostView::Scroll(v) => v,
            HostView::Field(v) => v,
            HostView::TabsHost(v) => v,
            HostView::Toggle(v) => v,
            HostView::Slide(v) => v,
            HostView::Spinner(v) => v,
            HostView::Progress(v) => v,
            HostView::Button(v) => v,
            HostView::Overlay(v) => v,
            HostView::Dialog(v) => v,
            HostView::Segments(v) => v,
            HostView::Step(v) => v,
            HostView::Search(v) => v,
            HostView::Menu(v) => v,
            HostView::Date(v) => v,
            HostView::Area(v) => v,
            HostView::Nav(v) => v,
            #[cfg(not(target_os = "tvos"))]
            HostView::Web(v) => v,
            HostView::Map(v) => v,
            HostView::Video(v) => v,
        }
    }

    fn kind(&self) -> NodeKind {
        match self {
            HostView::View(_) => NodeKind::View,
            HostView::Stack(_) => NodeKind::StackView,
            HostView::Label(_) => NodeKind::Text,
            HostView::Image(_) => NodeKind::Image,
            HostView::Scroll(_) => NodeKind::ScrollView,
            HostView::Field(_) => NodeKind::TextInput,
            HostView::TabsHost(_) => NodeKind::TabBar,
            HostView::Toggle(_) => NodeKind::Switch,
            HostView::Slide(_) => NodeKind::Slider,
            HostView::Spinner(_) => NodeKind::ActivityIndicator,
            HostView::Progress(_) => NodeKind::ProgressBar,
            HostView::Button(_) => NodeKind::Button,
            HostView::Overlay(_) => NodeKind::Modal,
            HostView::Dialog(_) => NodeKind::Alert,
            HostView::Segments(_) => NodeKind::SegmentedControl,
            HostView::Step(_) => NodeKind::Stepper,
            HostView::Search(_) => NodeKind::SearchBar,
            HostView::Menu(_) => NodeKind::Picker,
            HostView::Date(_) => NodeKind::DatePicker,
            HostView::Area(_) => NodeKind::TextEditor,
            HostView::Nav(_) => NodeKind::NavigationBar,
            #[cfg(not(target_os = "tvos"))]
            HostView::Web(_) => NodeKind::WebView,
            HostView::Map(_) => NodeKind::MapView,
            HostView::Video(_) => NodeKind::VideoView,
        }
    }

    fn as_label(&self) -> Option<&UILabel> {
        match self {
            HostView::Label(v) => Some(v),
            _ => None,
        }
    }
}

/// Las partes de una transformación, sin componer.
///
/// La escala arranca en 1 y no en 0: una vista sin `scale` tiene que verse
/// igual que antes de que existiera la prop, no desaparecer.
#[derive(Clone, Copy)]
struct Transform {
    translate_x: f64,
    translate_y: f64,
    scale_x: f64,
    scale_y: f64,
    rotate: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self { translate_x: 0.0, translate_y: 0.0, scale_x: 1.0, scale_y: 1.0, rotate: 0.0 }
    }
}

impl Transform {
    /// El orden es escalar, girar y luego desplazar.
    ///
    /// Al revés no sale lo mismo: si el desplazamiento entra antes que el
    /// giro, girar también gira el desplazamiento, y arrastrar algo inclinado
    /// se va en diagonal en vez de seguir al dedo.
    fn matrix(&self) -> CGAffineTransform {
        let scale = CGAffineTransform {
            a: self.scale_x,
            b: 0.0,
            c: 0.0,
            d: self.scale_y,
            tx: 0.0,
            ty: 0.0,
        };
        let (sin, cos) = self.rotate.sin_cos();
        let rotate = CGAffineTransform { a: cos, b: sin, c: -sin, d: cos, tx: 0.0, ty: 0.0 };
        let translate = CGAffineTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: self.translate_x,
            ty: self.translate_y,
        };
        concat(concat(scale, rotate), translate)
    }
}

/// Primero `a`, luego `b`. `CGAffineTransformConcat` no está en los enlaces, y
/// multiplicar dos matrices afines de 3x2 son seis productos: sale más barato
/// hacerlo aquí que enlazar con Core Graphics por esto.
fn concat(a: CGAffineTransform, b: CGAffineTransform) -> CGAffineTransform {
    CGAffineTransform {
        a: a.a * b.a + a.b * b.c,
        b: a.a * b.b + a.b * b.d,
        c: a.c * b.a + a.d * b.c,
        d: a.c * b.b + a.d * b.d,
        tx: a.tx * b.a + a.ty * b.c + b.tx,
        ty: a.tx * b.b + a.ty * b.d + b.ty,
    }
}

/// Cómo anima una vista sus cambios.
#[derive(Clone, Copy)]
struct Animation {
    /// Segundos. Cero apaga la animación sin borrar el resto de ajustes.
    duration: f64,
    delay: f64,
    curve: UIViewAnimationOptions,
}

impl Default for Animation {
    fn default() -> Self {
        // Sale rápido y frena al llegar. Es como se mueven las cosas, y es la
        // curva por defecto de casi todo en iOS.
        Animation { duration: 0.0, delay: 0.0, curve: UIViewAnimationOptions::CurveEaseOut }
    }
}

pub struct UikitHost {
    mtm: MainThreadMarker,
    /// Vista que da el shell de Xcode. La raíz del árbol cuelga de aquí.
    container: Retained<UIView>,
    views: HashMap<NodeId, HostView>,
    /// Fuente pendiente por nodo: `fontSize` y `fontWeight` llegan en props
    /// separadas y hay que reconstruir la `UIFont` con las dos.
    fonts: HashMap<NodeId, an_layout::FontSpec>,
    /// Radios por esquina: arriba-izq, arriba-der, abajo-der, abajo-izq.
    /// UIKit solo sabe de un radio único, así que cuando difieren hay que
    /// dibujar la forma a mano y usarla como máscara.
    corners: HashMap<NodeId, [f64; 4]>,
    /// Valor pedido a cada deslizador. Se guarda porque `value`, `minimumValue`
    /// y `maximumValue` llegan en props sueltas y en cualquier orden: fijar el
    /// valor antes que el máximo lo recorta contra el rango viejo.
    slider_values: HashMap<NodeId, f32>,
    /// Transformación de cada vista. Igual que con el deslizador, las partes
    /// llegan en props sueltas: hay que guardarlas para poder recomponer la
    /// matriz entera cada vez que cambia una.
    transforms: HashMap<NodeId, Transform>,
    /// Nodos que animan sus cambios, y cómo. Mientras hay una entrada aquí,
    /// mover, escalar, cambiar la opacidad o recolocar esa vista no salta al
    /// valor nuevo: va hasta él.
    animations: HashMap<NodeId, Animation>,
    /// Nombre, tamaño y peso del icono de cada nodo. Igual que con la fuente,
    /// las tres partes llegan sueltas y hay que rehacer el símbolo entero cada
    /// vez que cambia una.
    icons: HashMap<NodeId, (String, f32, u16)>,
    /// Títulos e iconos de cada barra de pestañas, que llegan por separado.
    tabs: HashMap<NodeId, (Vec<String>, Vec<String>)>,
    /// El controlador de pestañas de cada barra.
    tab_controllers: HashMap<NodeId, Retained<objc2_ui_kit::UITabBarController>>,
    /// El delegado de cada barra. Un delegado no se retiene, así que si no se
    /// guarda aquí muere y las pestañas dejan de avisar.
    tab_delegates: HashMap<NodeId, Retained<crate::events::TabDelegate>>,
    /// Subrayado o tachado de cada rótulo. Va aparte del `FontSpec` porque el
    /// núcleo no lo necesita: no cambia lo que mide el texto.
    decorations: HashMap<NodeId, String>,
    /// Texto y color del hueco de ayuda de cada campo. Van juntos porque
    /// `UITextField` no tiene un color de placeholder: hay que dárselo
    /// atribuido, y para eso hace falta también el texto.
    placeholders: HashMap<NodeId, String>,
    placeholder_colors: HashMap<NodeId, String>,
    /// Opciones de cada desplegable, para poder poner el título del elegido.
    menus: HashMap<NodeId, Vec<String>>,
    /// Título y color de cada botón. Cambiar la variante rehace la
    /// configuración de UIKit, que se lleva por delante los dos.
    button_titles: HashMap<NodeId, String>,
    button_colors: HashMap<NodeId, String>,
    button_variants: HashMap<NodeId, String>,
    /// Segunda línea del botón, que solo existe en iOS.
    button_subtitles: HashMap<NodeId, String>,
    /// Icono de cada botón y de qué lado va, que también llegan sueltos.
    button_icons: HashMap<NodeId, (String, String)>,
    /// Título, rótulo del atrás y si se enseña, de cada cabecera.
    navs: HashMap<NodeId, (String, String, bool)>,
    /// El botón de atrás vivo de cada cabecera, para poder engancharle el
    /// evento: se rehace cada vez que cambia el título.
    nav_backs: HashMap<NodeId, Retained<objc2_ui_kit::UIBarButtonItem>>,
    nav_targets: HashMap<NodeId, Retained<crate::events::ControlTarget>>,
    /// Centro y zoom de cada mapa, que llegan en props sueltas.
    maps: HashMap<NodeId, (f64, f64, f64)>,
    /// Reproductor y capa de cada vídeo. La capa hay que redimensionarla a
    /// mano: una capa no se estira con su vista.
    videos: HashMap<
        NodeId,
        (Retained<crate::video::AVPlayer>, Retained<crate::video::AVPlayerViewController>),
    >,
    /// Los vídeos que deberían estar sonando.
    video_playing: std::collections::HashSet<NodeId>,
    /// Valor pedido a cada `Stepper`, por lo mismo que en el deslizador: el
    /// rango y el valor llegan sueltos y en cualquier orden.
    stepper_values: HashMap<NodeId, f64>,
    /// Sentido de la próxima transición de cada pila: `push`, `pop` o nada.
    /// Lo decide Angular, que es quien sabe si se avanza o se retrocede.
    transitions: HashMap<NodeId, String>,
    /// Pantallas que acaban de entrar en una pila y todavía no se han animado.
    /// La animación no puede lanzarse al insertar porque el marco aún no está
    /// calculado: se hace en `flush`, cuando el layout ya pasó.
    entering: Vec<(NodeId, NodeId)>,
    /// Pantallas que salen. Se quedan en la jerarquía hasta que la animación
    /// termina, así que hay que retenerlas aunque el árbol ya las olvidara.
    leaving: Vec<(NodeId, Retained<UIView>)>,
    /// Nodos cuya vista está animándose fuera: `destroy` no debe tocarlos.
    animating_out: std::collections::HashSet<NodeId>,
    /// Nodos suscritos al área segura, con los últimos márgenes que se les
    /// contó. Solo se avisa cuando cambian de verdad.
    safe_area: HashMap<NodeId, [f32; 4]>,
    /// Diálogos declarados. Se presentan al cerrar el frame, cuando todas sus
    /// props ya llegaron: presentar en cuanto cambia `visible` mostraría un
    /// diálogo sin título.
    alerts: HashMap<NodeId, crate::alert::AlertState>,
    /// Diálogos cuyo estado cambió en este frame.
    dirty_alerts: Vec<NodeId>,
    /// Estado de presentación de cada `<Modal>`.
    modals: HashMap<NodeId, crate::modal::ModalState>,
    dirty_modals: Vec<NodeId>,
    /// Suscripciones vivas, indexadas por nodo y evento. Se guardan porque hay
    /// que poder quitarlas: un `@if` que desmonta su rama destruye la vista,
    /// pero un `(press)` que deja de estar bindeado no.
    listeners: HashMap<(NodeId, String), crate::events::AttachedListener>,
    events: EventQueue,
}

impl UikitHost {
    /// # Safety
    /// `container` tiene que ser un `UIView` vivo y hay que llamar desde el
    /// hilo principal.
    pub fn new(mtm: MainThreadMarker, container: Retained<UIView>, events: EventQueue) -> Self {
        UikitHost {
            mtm,
            container,
            views: HashMap::new(),
            fonts: HashMap::new(),
            corners: HashMap::new(),
            slider_values: HashMap::new(),
            transforms: HashMap::new(),
            animations: HashMap::new(),
            icons: HashMap::new(),
            tabs: HashMap::new(),
            tab_controllers: HashMap::new(),
            tab_delegates: HashMap::new(),
            menus: HashMap::new(),
            decorations: HashMap::new(),
            placeholders: HashMap::new(),
            placeholder_colors: HashMap::new(),
            button_titles: HashMap::new(),
            button_colors: HashMap::new(),
            button_variants: HashMap::new(),
            button_subtitles: HashMap::new(),
            button_icons: HashMap::new(),
            navs: HashMap::new(),
            nav_backs: HashMap::new(),
            nav_targets: HashMap::new(),
            maps: HashMap::new(),
            videos: HashMap::new(),
            video_playing: std::collections::HashSet::new(),
            stepper_values: HashMap::new(),
            transitions: HashMap::new(),
            entering: Vec::new(),
            leaving: Vec::new(),
            animating_out: std::collections::HashSet::new(),
            safe_area: HashMap::new(),
            alerts: HashMap::new(),
            dirty_alerts: Vec::new(),
            modals: HashMap::new(),
            dirty_modals: Vec::new(),
            listeners: HashMap::new(),
            events,
        }
    }

    pub fn container(&self) -> &UIView {
        &self.container
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    /// La `UIFont` que pide un `FontSpec`. Sin tocar ninguna vista: el mismo
    /// cálculo lo necesitan el rótulo, el campo, el editor y el botón.
    fn build_font(&self, spec: &an_layout::FontSpec) -> Retained<UIFont> {
        let size = spec.size as f64;
        if let Some(family) = &spec.family {
            let name = NSString::from_str(family);
            return UIFont::fontWithName_size(&name, size)
                .unwrap_or_else(|| UIFont::systemFontOfSize(size));
        }
        if spec.italic {
            return UIFont::italicSystemFontOfSize(size);
        }
        let weight = match spec.weight {
            0..=299 => -0.6,
            300..=399 => -0.4,
            400..=499 => 0.0,
            500..=599 => 0.23,
            600..=699 => 0.3,
            700..=799 => 0.4,
            _ => 0.6,
        };
        UIFont::systemFontOfSize_weight(size, weight)
    }

    /// Vuelve a poner el texto de ayuda con su color.
    ///
    /// Sin color se pone llano y no atribuido: el atribuido sin atributos se
    /// dibuja distinto del que pone UIKit por su cuenta.
    fn apply_placeholder(&self, id: NodeId) {
        let Some(HostView::Field(field)) = self.views.get(&id) else { return };
        let Some(placeholder) = self.placeholders.get(&id) else { return };
        let string = NSString::from_str(placeholder);
        let Some(color) =
            self.placeholder_colors.get(&id).and_then(|raw| crate::color::to_uicolor(raw))
        else {
            field.setPlaceholder(Some(&string));
            return;
        };
        let attributed = unsafe {
            objc2_foundation::NSMutableAttributedString::initWithString(
                self.mtm.alloc::<objc2_foundation::NSMutableAttributedString>(),
                &string,
            )
        };
        unsafe {
            attributed.addAttribute_value_range(
                objc2_ui_kit::NSForegroundColorAttributeName,
                &color,
                objc2_foundation::NSRange { location: 0, length: string.len_utf16() },
            );
            field.setAttributedPlaceholder(Some(&attributed));
        }
    }

    fn apply_font(&mut self, id: NodeId) {
        let Some(spec) = self.fonts.get(&id).cloned() else { return };
        let font = self.build_font(&spec);
        let Some(view) = self.views.get(&id) else { return };
        // Estos setters de UIKit están marcados unsafe por no ser thread-safe;
        // el `MainThreadMarker` del host garantiza que vamos por el hilo bueno.
        match view {
            HostView::Label(label) => unsafe {
                label.setFont(Some(&font));
                label.setNumberOfLines(spec.max_lines.unwrap_or(0) as isize);
            },
            // El campo, el editor y el botón también tienen letra, y hasta
            // ahora se quedaban con la de UIKit: `[fontSize]` en un
            // `<TextInput>` era una prop declarada que no hacía nada.
            HostView::Field(field) => unsafe { field.setFont(Some(&font)) },
            HostView::Area(area) => unsafe { area.setFont(Some(&font)) },
            HostView::Button(_) => self.refresh_button(id),
            _ => return,
        }
        self.apply_text_attributes(id);
    }

    /// Interlineado y espaciado entre letras, que `UILabel` no tiene como
    /// propiedades.
    ///
    /// El núcleo ya medía con los dos —están en el `FontSpec` con el que
    /// calcula el alto de cada línea— y el host dibujaba sin ellos, así que el
    /// layout reservaba un hueco que el texto no llenaba. La única forma de
    /// aplicarlos en UIKit es con texto atribuido: `kern` para el espaciado y
    /// un `NSParagraphStyle` para el alto de línea.
    ///
    /// Se ponen solo esos dos atributos. La fuente y el color se dejan fuera a
    /// propósito: sin ellos en los atributos, `UILabel` usa los suyos, y así
    /// `[color]` y `[fontSize]` siguen funcionando como antes.
    fn apply_text_attributes(&self, id: NodeId) {
        let Some(label) = self.views.get(&id).and_then(HostView::as_label) else { return };
        let spec = self.fonts.get(&id);
        let kern = spec.map(|s| s.letter_spacing).unwrap_or(0.0);
        let line_height = spec.and_then(|s| s.line_height);
        let decoration = self.decorations.get(&id).map(String::as_str).unwrap_or("none");
        let text = unsafe { label.text() }.map(|t| t.to_string()).unwrap_or_default();
        if text.is_empty() {
            return;
        }
        let string = NSString::from_str(&text);
        if kern == 0.0 && line_height.is_none() && decoration == "none" {
            // Sin nada que añadir se vuelve a texto llano: si no, quitar el
            // espaciado dejaría puesto el de antes.
            unsafe { label.setAttributedText(None) };
            label.setText(Some(&string));
            return;
        }
        let attributed = unsafe {
            objc2_foundation::NSMutableAttributedString::initWithString(
                self.mtm.alloc::<objc2_foundation::NSMutableAttributedString>(),
                &string,
            )
        };
        // En UTF-16, que es como cuenta `NSString`. Con la longitud en bytes,
        // cualquier texto con una tilde se sale del rango y `NSAttributedString`
        // levanta una excepción: la app se cierra al montar la primera letra
        // acentuada, y el volcado headless no lo ve porque ahí no hay UIKit.
        let range = objc2_foundation::NSRange { location: 0, length: string.len_utf16() };
        if kern != 0.0 {
            let number = objc2_foundation::NSNumber::new_f64(kern as f64);
            unsafe {
                attributed.addAttribute_value_range(
                    objc2_ui_kit::NSKernAttributeName,
                    &number,
                    range,
                )
            };
        }
        if let Some(height) = line_height {
            let style = objc2_ui_kit::NSMutableParagraphStyle::new();
            // Mínimo y máximo iguales: el alto de línea es el que pide la
            // plantilla, ni el que traiga la fuente ni uno mayor.
            style.setMinimumLineHeight(height as f64);
            style.setMaximumLineHeight(height as f64);
            // El estilo de párrafo se lleva también el corte de línea, así que
            // hay que devolverle el que tenía el rótulo o `numberOfLines`
            // dejaría de poner puntos suspensivos.
            style.setLineBreakMode(label.lineBreakMode());
            unsafe {
                attributed.addAttribute_value_range(
                    objc2_ui_kit::NSParagraphStyleAttributeName,
                    &style,
                    range,
                )
            };
        }
        if decoration != "none" {
            // El 1 es `NSUnderlineStyle.single`: una raya sencilla, que es la
            // única que se pide desde una plantilla.
            let style = objc2_foundation::NSNumber::new_isize(1);
            unsafe {
                attributed.addAttribute_value_range(
                    if decoration == "lineThrough" {
                        objc2_ui_kit::NSStrikethroughStyleAttributeName
                    } else {
                        objc2_ui_kit::NSUnderlineStyleAttributeName
                    },
                    &style,
                    range,
                )
            };
        }
        unsafe { label.setAttributedText(Some(&attributed)) };
    }

    /// Aplica un cambio visual, animado si el nodo lo pidió.
    ///
    /// No se puede animar "lo que pase dentro del bloque" y ya está: UIKit
    /// necesita que el estado de partida esté puesto antes de entrar, y ese
    /// es justo el que la vista tiene ahora. Por eso basta con meter el
    /// cambio dentro; lo de fuera es lo que había.
    fn animated(&self, id: NodeId, change: impl Fn() + 'static) {
        let Some(anim) = self.animations.get(&id).copied().filter(|a| a.duration > 0.0) else {
            change();
            return;
        };
        let block = RcBlock::new(move || change());
        unsafe {
            UIView::animateWithDuration_delay_options_animations_completion(
                anim.duration,
                anim.delay,
                anim.curve,
                &block,
                None,
                self.mtm,
            );
        }
    }

    /// Rehace el aspecto de un botón con su variante y su color.
    ///
    /// Las dos props llegan sueltas y en cualquier orden, y cambiar la
    /// variante rehace la configuración de UIKit, que se lleva por delante el
    /// título y el color: hay que ponerlo todo de nuevo cada vez.
    /// Rehace el botón entero con lo que lleve puesto.
    ///
    /// Variante, rótulo, subtítulo, icono, color y tipografía llegan en props
    /// sueltas y en cualquier orden, y todas acaban en la misma
    /// `UIButtonConfiguration`: cambiar una rehace la configuración y se lleva
    /// por delante las otras cinco. Así que se guardan y se monta de una pieza.
    fn refresh_button(&self, id: NodeId) {
        let Some(HostView::Button(button)) = self.views.get(&id) else { return };
        let variant = self.button_variants.get(&id).map(String::as_str).unwrap_or("text");
        let title = self.button_titles.get(&id).cloned().unwrap_or_default();
        let subtitle = self.button_subtitles.get(&id).cloned().unwrap_or_default();
        let icon = self.button_icons.get(&id).cloned();
        let spec = self.fonts.get(&id).cloned();
        let font = spec.as_ref().map(|spec| self.build_font(spec));
        let raw_color = self.button_colors.get(&id).and_then(|raw| crate::color::parse(raw));
        let color = raw_color.map(|(r, g, b, a)| {
            objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a)
        });
        // Con relleno, el rótulo va del color que se lea encima del fondo; sin
        // relleno, del color pedido. Se calcula aquí y no se deja a UIKit
        // porque el texto atribuido —el que lleva la tipografía— no hereda el
        // color de la configuración: se quedaba del tinte, o sea verde sobre
        // verde, o sea invisible.
        let foreground = raw_color.map(|value| {
            let (r, g, b, a) = if variant == "filled" {
                crate::color::contrast_on(value)
            } else {
                value
            };
            objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a)
        });
        // `UIButtonConfiguration` es lo que da los botones actuales de iOS:
        // relleno, tintado, con contorno o pelado, con sus fondos y sus
        // esquinas. Un subtítulo, un icono o una tipografía propia solo se
        // pueden pedir por ahí, así que en cuanto hay alguno de los tres hace
        // falta configuración aunque la variante sea la de solo rótulo.
        let needs_config =
            !subtitle.is_empty() || icon.is_some() || font.is_some() || variant != "text";
        let config = unsafe {
            match variant {
                "filled" => {
                    Some(objc2_ui_kit::UIButtonConfiguration::filledButtonConfiguration(self.mtm))
                }
                "tonal" => {
                    Some(objc2_ui_kit::UIButtonConfiguration::tintedButtonConfiguration(self.mtm))
                }
                // El contorno de UIKit: fondo transparente y una línea
                // alrededor, que es lo que hace el botón `outlined` de
                // Material.
                "outlined" => {
                    Some(objc2_ui_kit::UIButtonConfiguration::borderedButtonConfiguration(self.mtm))
                }
                _ if needs_config => {
                    Some(objc2_ui_kit::UIButtonConfiguration::plainButtonConfiguration(self.mtm))
                }
                _ => None,
            }
        };
        if let Some(config) = &config {
            unsafe {
                // Antes de nada, borrar lo que dejó escrito la vía de siempre.
                //
                // Las props llegan sueltas y `variant` llega después que
                // `title` y `color`, así que la primera pasada de cada botón
                // corre siempre como si fuera `text`: sin configuración, y por
                // tanto por `setTitle:forState:` y `setTitleColor:forState:`.
                // Ese color se queda pegado al botón, y cuando después se le
                // monta una configuración UIKit lo sigue aplicando **al
                // título** por encima del `baseForegroundColor` —al subtítulo
                // no, que sí lo respeta—. En las tres variantes en las que el
                // rótulo va del color pedido eso no se nota, porque el residuo
                // vale lo mismo; en `filled`, donde el rótulo va del color que
                // contrasta con el fondo, el residuo pintaba el texto del
                // mismo color que el relleno. De ahí el botón entero y sin
                // rótulo.
                button.setTitleColor_forState(None, UIControlState::Normal);
                button.setTitle_forState(None, UIControlState::Normal);
                // Con configuración, todo va por ella y nada por las llamadas
                // de siempre.
                config.setTitle(Some(&NSString::from_str(&title)));
                if !subtitle.is_empty() {
                    config.setSubtitle(Some(&NSString::from_str(&subtitle)));
                }
                if let Some((name, position)) = &icon {
                    // El icono del botón se pide por nombre, igual que en
                    // `<Icon>`: es el símbolo del sistema, no un dibujo.
                    //
                    // Y se pide al tamaño del rótulo. Sin decírselo viene al
                    // suyo, que es el de una imagen suelta, y un botón con una
                    // estrella el doble de alta que su texto no se parece a
                    // ningún botón de iOS.
                    let points = spec.as_ref().map(|spec| spec.size).unwrap_or(17.0);
                    let weight = spec.as_ref().map(|spec| spec.weight).unwrap_or(400);
                    config.setImage(crate::icons::symbol(name, points, weight).as_deref());
                    config.setImagePlacement(if position == "trailing" {
                        objc2_ui_kit::NSDirectionalRectEdge::Trailing
                    } else {
                        objc2_ui_kit::NSDirectionalRectEdge::Leading
                    });
                    config.setImagePadding(6.0);
                }
                if let Some(color) = &color {
                    // Con relleno el color pedido es el del fondo y el rótulo
                    // va del que se lea encima; sin relleno es el del rótulo,
                    // y con él el del icono.
                    if variant == "filled" {
                        config.setBaseBackgroundColor(Some(color));
                    }
                }
                if let Some(foreground) = &foreground {
                    config.setBaseForegroundColor(Some(foreground));
                }
                if let Some(font) = &font {
                    // La tipografía de un botón con configuración la resuelve
                    // UIKit: pedírsela al `titleLabel` es una sugerencia que
                    // pisa en cuanto vuelve a montar el título. El sitio donde
                    // manda de verdad es este transformador, que recibe los
                    // atributos que UIKit iba a usar y devuelve los que se
                    // usan. Se cambia solo la fuente: el color y lo demás
                    // salen de la configuración y hay que dejarlos pasar.
                    let font = font.clone();
                    let block = RcBlock::new(
                        move |attributes: NonNull<
                            objc2_foundation::NSDictionary<
                                objc2_foundation::NSAttributedStringKey,
                                objc2::runtime::AnyObject,
                            >,
                        >| {
                            let incoming = unsafe { attributes.as_ref() };
                            let out = objc2_foundation::NSMutableDictionary::
                                dictionaryWithDictionary(incoming);
                            out.insert(objc2_ui_kit::NSFontAttributeName, font.as_ref());
                            // El bloque devuelve el diccionario en +0, así que
                            // se suelta en el pool: quedárselo lo filtra y
                            // soltarlo aquí lo mata antes de que UIKit lo lea.
                            let out: Retained<objc2_foundation::NSDictionary<_, _>> =
                                out.into_super();
                            NonNull::new(Retained::autorelease_return(out)).unwrap()
                        },
                    );
                    config.setTitleTextAttributesTransformer(RcBlock::as_ptr(&block));
                }
            }
        }
        unsafe {
            button.setConfiguration(config.as_deref());
            // La letra se le pide al rótulo en los dos casos. Con
            // configuración quien manda es el transformador de arriba y esto
            // sobra; sin ella no hay transformador que valga y esta es la
            // única vía, así que se deja para las dos.
            if let (Some(font), Some(label)) = (&font, button.titleLabel()) {
                label.setFont(Some(font));
            }
            if config.is_none() {
                // Sin configuración manda el botón: rótulo y color por las
                // llamadas de siempre.
                button.setTitle_forState(Some(&NSString::from_str(&title)), UIControlState::Normal);
                if let Some(color) = &color {
                    button.setTitleColor_forState(Some(color), UIControlState::Normal);
                    button.setTintColor(Some(color));
                }
            }
        }
    }

    fn font_mut(&mut self, id: NodeId) -> &mut an_layout::FontSpec {
        self.fonts.entry(id).or_default()
    }

    /// Anima las pantallas que entraron o salieron en este frame.
    ///
    /// Se hace aquí y no al insertar porque hasta que el layout no pasa no hay
    /// marco que animar: una pantalla recién creada mide cero.
    fn run_stack_animations(&mut self) {
        let entering = std::mem::take(&mut self.entering);
        let leaving = std::mem::take(&mut self.leaving);
        if entering.is_empty() && leaving.is_empty() {
            return;
        }
        let mtm = self.mtm;

        for (stack, child) in entering {
            let Some(stack_view) = self.views.get(&stack).map(|v| v.as_view().retain()) else {
                continue;
            };
            let Some(view) = self.views.get(&child).map(|v| v.as_view().retain()) else { continue };
            let width = stack_view.bounds().size.width;
            let direction = self.transitions.get(&stack).map(String::as_str).unwrap_or("none");
            if direction != "push" || width <= 0.0 {
                continue;
            }

            // Entra desde la derecha; la de debajo se desplaza un tercio, que
            // es el paralaje que hace UINavigationController.
            let target = view.frame();
            let mut start = target;
            start.origin.x = width;
            view.setFrame(start);

            let below = previous_sibling(&stack_view, &view);
            let below_target = below.as_ref().map(|v| {
                let mut frame = v.frame();
                frame.origin.x = -width / 3.0;
                (v.clone(), frame)
            });

            let animations = RcBlock::new(move || {
                view.setFrame(target);
                if let Some((below, frame)) = &below_target {
                    below.setFrame(*frame);
                }
            });
            UIView::animateWithDuration_animations_completion(0.3, &animations, None, mtm);
        }

        for (stack, view) in leaving {
            let Some(stack_view) = self.views.get(&stack).map(|v| v.as_view().retain()) else {
                view.removeFromSuperview();
                continue;
            };
            let width = stack_view.bounds().size.width;
            if width <= 0.0 {
                view.removeFromSuperview();
                continue;
            }

            let below = previous_sibling(&stack_view, &view);
            let below_target = below.as_ref().map(|v| {
                let mut frame = v.frame();
                frame.origin.x = 0.0;
                (v.clone(), frame)
            });
            let mut target = view.frame();
            target.origin.x = width;

            let animated = view.clone();
            let animations = RcBlock::new(move || {
                animated.setFrame(target);
                if let Some((below, frame)) = &below_target {
                    below.setFrame(*frame);
                }
            });
            // La vista se quita al acabar: hasta entonces tiene que seguir
            // montada, y por eso el bloque la retiene.
            let completion = RcBlock::new(move |_finished: objc2::runtime::Bool| {
                view.removeFromSuperview();
            });
            UIView::animateWithDuration_animations_completion(
                0.3,
                &animations,
                Some(&completion),
                mtm,
            );
        }
        self.animating_out.clear();
    }

    /// Cuenta los márgenes del sistema si cambiaron desde la última vez.
    fn report_safe_area(&mut self, id: NodeId) {
        let Some(previous) = self.safe_area.get(&id).copied() else { return };
        let insets = self.container.safeAreaInsets();
        let current = [
            insets.top as f32,
            insets.right as f32,
            insets.bottom as f32,
            insets.left as f32,
        ];
        if current
            .iter()
            .zip(previous.iter())
            .all(|(a, b)| (a - b).abs() < f32::EPSILON)
        {
            return;
        }
        self.safe_area.insert(id, current);
        an_host::push_event(
            &self.events,
            an_host::HostEvent {
                target: id,
                name: "safeArea".to_owned(),
                payload: vec![
                    ("top".to_owned(), PropValue::Number(current[0] as f64)),
                    ("right".to_owned(), PropValue::Number(current[1] as f64)),
                    ("bottom".to_owned(), PropValue::Number(current[2] as f64)),
                    ("left".to_owned(), PropValue::Number(current[3] as f64)),
                ],
            },
        );
    }

    fn set_corner(&mut self, id: NodeId, corner: usize, radius: Option<f32>) {
        let radii = self.corners.entry(id).or_insert([0.0; 4]);
        radii[corner] = radius.unwrap_or(0.0) as f64;
        self.apply_corners(id);
    }

    /// Aplica los radios al nodo.
    ///
    /// Si los cuatro son iguales basta `cornerRadius`, que es barato y deja
    /// que UIKit recorte por su cuenta. Si difieren no hay API: hay que
    /// dibujar el contorno y ponerlo de máscara, y rehacerlo cada vez que la
    /// vista cambia de tamaño, porque una máscara no se estira sola.
    fn apply_corners(&mut self, id: NodeId) {
        let Some(radii) = self.corners.get(&id).copied() else { return };
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        let layer = native.layer();

        let uniform = radii.iter().all(|r| (*r - radii[0]).abs() < f64::EPSILON);
        if uniform {
            // Como el resto de setters de UIKit: marcado unsafe por no ser
            // thread-safe, y aquí siempre vamos por el hilo de UI.
            unsafe { layer.setMask(None) };
            layer.setCornerRadius(radii[0]);
            native.setClipsToBounds(radii[0] > 0.0);
            return;
        }

        layer.setCornerRadius(0.0);
        native.setClipsToBounds(true);
        let bounds = native.bounds();
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            // Todavía no tiene tamaño; el marco llegará y volveremos aquí.
            return;
        }
        let path = rounded_path(bounds.size.width, bounds.size.height, radii);
        let shape = CAShapeLayer::new();
        unsafe { shape.setPath(Some(&path.CGPath())) };
        unsafe { layer.setMask(Some(&shape)) };
    }
}

impl HostRenderer for UikitHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        let mtm = self.mtm;

        // Lo que esta familia de UIKit no trae, antes del `match`.
        //
        // El problema de un `<an-switch>` en tvOS no es cómo configurarlo: es
        // que `UISwitch` no está en el sistema, y pedirle la clase a objc2
        // aborta el proceso ahí mismo. Se dice en el log —una vez— y se deja
        // una vista vacía marcada, para que el árbol tenga dónde colgar a los
        // hijos del nodo y el resto de la pantalla no se descoloque. El
        // medidor no conoce ese control, así que la caja mide cero: el hueco
        // se ve, que es lo que tiene que pasar.
        if let Some(porque) = crate::family::missing_kind(kind) {
            crate::family::report(&format!("{kind:?}"), porque);
            let hueco = UIView::new(mtm);
            let marca = NSString::from_str(&format!("an-unsupported:{kind:?}"));
            hueco.setAccessibilityIdentifier(Some(&marca));
            hueco.setTranslatesAutoresizingMaskIntoConstraints(true);
            self.views.insert(id, HostView::View(hueco));
            return;
        }

        let view = match kind {
            NodeKind::Text => {
                let label = UILabel::new(mtm);
                // El alto lo decide el layout, no el auto-ajuste de UIKit.
                label.setNumberOfLines(0);
                label.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
                // La fuente por defecto tiene que ser la misma con la que el
                // layout midió. Un `UILabel` recién hecho usa 17 puntos y el
                // núcleo mide con 14: la caja salía un 20% estrecha, el texto
                // saltaba de línea y el recorte del padre se comía la
                // segunda. Se veía como texto que desaparece, sin ningún
                // error por ningún lado.
                let default_size = an_layout::FontSpec::default().size as f64;
                unsafe { label.setFont(Some(&UIFont::systemFontOfSize(default_size))) };
                HostView::Label(label)
            }
            NodeKind::Image => HostView::Image(UIImageView::new(mtm)),
            NodeKind::Icon => {
                let view = UIImageView::new(mtm);
                // `AlwaysTemplate` es lo que deja teñir el símbolo con
                // `tintColor`; sin eso saldría siempre con su color propio y
                // `[color]` no haría nada.
                unsafe { view.setContentMode(objc2_ui_kit::UIViewContentMode::ScaleAspectFit) };
                HostView::Image(view)
            }
            NodeKind::ScrollView => HostView::Scroll(UIScrollView::new(mtm)),
            NodeKind::TabBar => {
                // Un `UITabBarController` de verdad, no una `UITabBar` suelta.
                //
                // Desde iOS 26 una barra suelta no se porta: su proveedor
                // visual la dibuja por su cuenta y en iPad la sube arriba
                // *además* de en el marco que le da el layout, así que salen
                // dos. Es lo que pasa cuando se usa un control que espera un
                // controlador y no se le da.
                //
                // Con el controlador, UIKit tiene lo que necesita y coloca la
                // barra donde toca en cada dispositivo: abajo en iPhone,
                // arriba en iPad. Una sola, la del sistema, en las dos.
                let controller = objc2_ui_kit::UITabBarController::new(mtm);
                unsafe {
                    // `TabBar` y no `Automatic`: en iPad el automático puede
                    // convertirla en barra lateral, y eso cambia la pantalla
                    // entera por debajo del layout.
                    controller.setMode(objc2_ui_kit::UITabBarControllerMode::TabBar);
                }
                let delegate = crate::events::TabDelegate::new(mtm, id, self.events.clone());
                unsafe {
                    controller.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(
                        &*delegate,
                    )))
                };
                self.tab_delegates.insert(id, delegate);
                let view = controller.view().expect("el controlador trae vista");
                // La vista del controlador es solo el hueco donde va la barra:
                // el contenido lo pone el árbol. Sin esto se ve su fondo
                // blanco por debajo.
                view.setBackgroundColor(None);
                self.tab_controllers.insert(id, controller);
                HostView::TabsHost(view)
            }
            NodeKind::Switch => HostView::Toggle(UISwitch::new(mtm)),
            NodeKind::Slider => HostView::Slide(UISlider::new(mtm)),
            NodeKind::ActivityIndicator => {
                let spinner = UIActivityIndicatorView::new(mtm);
                spinner.setHidesWhenStopped(true);
                HostView::Spinner(spinner)
            }
            NodeKind::ProgressBar => HostView::Progress(UIProgressView::new(mtm)),
            NodeKind::Button => HostView::Button(UIButton::new(mtm)),
            NodeKind::Alert => {
                let placeholder = UIView::new(mtm);
                placeholder.setHidden(true);
                self.alerts.insert(id, crate::alert::AlertState::default());
                HostView::Dialog(placeholder)
            }
            NodeKind::Modal => {
                let overlay = UIView::new(mtm);
                overlay.setHidden(true);
                self.modals.insert(id, crate::modal::ModalState::default());
                HostView::Overlay(overlay)
            }
            NodeKind::StackView => {
                let stack = UIView::new(mtm);
                // Las pantallas que entran y salen se salen del marco: sin
                // recortar, se verían deslizándose por encima de lo demás.
                stack.setClipsToBounds(true);
                HostView::Stack(stack)
            }
            NodeKind::TextInput => HostView::Field(UITextField::new(mtm)),
            NodeKind::TextEditor => {
                let text_view = objc2_ui_kit::UITextView::new(mtm);
                unsafe {
                    // Sin fondo ni márgenes propios: los pone la plantilla,
                    // igual que en un campo de una línea.
                    text_view.setBackgroundColor(None);
                    text_view.setTextContainerInset(objc2_ui_kit::UIEdgeInsets {
                        top: 0.0,
                        left: 0.0,
                        bottom: 0.0,
                        right: 0.0,
                    });
                    text_view.textContainer().setLineFragmentPadding(0.0);
                }
                HostView::Area(text_view)
            }
            NodeKind::NavigationBar => {
                let bar = objc2_ui_kit::UINavigationBar::new(mtm);
                HostView::Nav(bar)
            }
            #[cfg(not(target_os = "tvos"))]
            NodeKind::WebView => {
                let web = crate::web::WKWebView::new(mtm);
                HostView::Web(web)
            }
            NodeKind::MapView => HostView::Map(crate::map::MKMapView::new(mtm)),
            NodeKind::VideoView => {
                let player = UIView::new(mtm);
                HostView::Video(player)
            }
            NodeKind::SegmentedControl => {
                HostView::Segments(objc2_ui_kit::UISegmentedControl::new(mtm))
            }
            NodeKind::Stepper => HostView::Step(objc2_ui_kit::UIStepper::new(mtm)),
            NodeKind::SearchBar => HostView::Search(objc2_ui_kit::UISearchBar::new(mtm)),
            NodeKind::Picker => {
                // Un desplegable en iOS es un botón que abre un menú: no hay
                // un control aparte, y `UIPickerView` es la rueda de pantalla
                // completa, que es otra cosa.
                let button = objc2_ui_kit::UIButton::new(mtm);
                unsafe { button.setShowsMenuAsPrimaryAction(true) };
                HostView::Menu(button)
            }
            NodeKind::DatePicker => {
                let picker = objc2_ui_kit::UIDatePicker::new(mtm);
                unsafe {
                    picker.setPreferredDatePickerStyle(objc2_ui_kit::UIDatePickerStyle::Compact);
                    // Por defecto UIKit pide fecha *y* hora. El de aquí pide
                    // fecha salvo que se diga otra cosa, igual que en Android.
                    picker.setDatePickerMode(objc2_ui_kit::UIDatePickerMode::Date);
                };
                HostView::Date(picker)
            }
            // Aquí cae `View`, que es la primitiva que la gente hace
            // pulsable con `(press)`. En tvOS no puede ser una `UIView`
            // cualquiera: `canBecomeFocused` solo se cambia heredando, y sin
            // eso el mando no llega nunca a esa vista. Ver `focus.rs`.
            #[cfg(target_os = "tvos")]
            _ => HostView::View(Retained::into_super(crate::focus::FocusableView::new(
                mtm,
                id,
                self.events.clone(),
            ))),
            #[cfg(not(target_os = "tvos"))]
            _ => HostView::View(UIView::new(mtm)),
        };
        // Quién manda sobre el marco.
        //
        // `false` significa "mi marco lo deciden mis restricciones", y es lo
        // que había aquí. Mientras no hubo ningún control con restricciones
        // propias daba igual: el motor de Auto Layout no llegaba a activarse y
        // los marcos se quedaban como los escribía el core. En cuanto entró
        // uno compuesto —una `UISearchBar`, un `UIDatePicker`— el motor se
        // activó para toda la ventana y puso a cero el marco de cada vista que
        // decía esperar restricciones que no existían. Se veía como la
        // pantalla entera amontonada en la esquina.
        //
        // `true` es lo que hay que decir cuando el marco lo escribe uno: UIKit
        // lo traduce a restricciones y respeta lo que se le da.
        view.as_view().setTranslatesAutoresizingMaskIntoConstraints(true);
        self.views.insert(id, view);
    }

    fn destroy(&mut self, id: NodeId) {
        if let Some(view) = self.views.remove(&id) {
            // Una pantalla que se está yendo sigue en pantalla hasta que la
            // animación acabe: quitarla ahora daría un salto.
            if !self.animating_out.contains(&id) {
                view.as_view().removeFromSuperview();
            }
        }
        self.fonts.remove(&id);
        self.corners.remove(&id);
        self.safe_area.remove(&id);
        self.alerts.remove(&id);
        self.slider_values.remove(&id);
        self.transforms.remove(&id);
        self.animations.remove(&id);
        self.icons.remove(&id);
        self.tabs.remove(&id);
        self.tab_controllers.remove(&id);
        self.tab_delegates.remove(&id);
        self.menus.remove(&id);
        self.button_titles.remove(&id);
        self.button_colors.remove(&id);
        self.button_variants.remove(&id);
        self.button_subtitles.remove(&id);
        self.button_icons.remove(&id);
        self.placeholders.remove(&id);
        self.placeholder_colors.remove(&id);
        self.decorations.remove(&id);
        self.navs.remove(&id);
        self.nav_backs.remove(&id);
        self.nav_targets.remove(&id);
        self.maps.remove(&id);
        self.videos.remove(&id);
        self.video_playing.remove(&id);
        self.stepper_values.remove(&id);
        self.modals.remove(&id);
        self.listeners.retain(|(node, _), _| *node != id);
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        let (Some(parent_view), Some(child_view)) = (self.views.get(&parent), self.views.get(&child))
        else {
            return;
        };
        let is_stack = matches!(parent_view, HostView::Stack(_));
        // Una pantalla que entra tiene que quedar por encima de la que sale,
        // aunque el árbol la coloque antes.
        let index = if is_stack {
            parent_view.as_view().subviews().len() as isize
        } else {
            index as isize
        };
        parent_view.as_view().insertSubview_atIndex(child_view.as_view(), index);
        if is_stack {
            self.entering.push((parent, child));
        }
    }

    fn remove(&mut self, parent: NodeId, child: NodeId) {
        let Some(view) = self.views.get(&child) else { return };
        let native = view.as_view().retain();
        let popping = matches!(self.views.get(&parent), Some(HostView::Stack(_)))
            && self.transitions.get(&parent).map(String::as_str) == Some("pop");
        if popping {
            // Se queda montada hasta que termine de salir.
            self.animating_out.insert(child);
            self.leaving.push((parent, native));
            return;
        }
        native.removeFromSuperview();
    }

    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        let text = value.as_str().map(str::to_owned);
        let number = value.as_f32();

        match key {
            "backgroundColor" | "background-color" => {
                let color = text.as_deref().and_then(crate::color::to_uicolor);
                native.setBackgroundColor(color.as_deref());
            }
            "opacity" => {
                if let Some(v) = number {
                    let view = native.retain();
                    self.animated(id, move || view.setAlpha(v as f64));
                }
            }
            // Props de una sola plataforma. Viajan con su prefijo, así que
            // este host descarta de un vistazo las que son de la otra: no
            // tiene que saber qué significan, solo de quién son.
            _ if key.starts_with("android:") => {}
            "variant" | "icon" | "iconPosition" | "ios:subtitle"
                if matches!(view, HostView::Button(_)) =>
            {
                match key {
                    "variant" => {
                        self.button_variants
                            .insert(id, text.clone().unwrap_or_else(|| "text".to_owned()));
                    }
                    "ios:subtitle" => {
                        self.button_subtitles.insert(id, text.clone().unwrap_or_default());
                    }
                    "icon" => {
                        let entry = self.button_icons.entry(id).or_default();
                        entry.0 = text.clone().unwrap_or_default();
                    }
                    _ => {
                        let entry = self.button_icons.entry(id).or_default();
                        entry.1 = text.clone().unwrap_or_else(|| "leading".to_owned());
                    }
                }
                // Un botón sin nombre de icono no lleva icono, aunque le
                // quede el lado puesto de antes.
                if self.button_icons.get(&id).is_some_and(|(name, _)| name.is_empty()) {
                    self.button_icons.remove(&id);
                }
                self.refresh_button(id);
            }
            // Apagar un control es cosa de `UIControl`, que sabe ponerse gris
            // y dejar de responder. Los que no lo son se quedan sin toque, que
            // es lo más parecido que hay.
            "enabled" => {
                let on = !matches!(value, PropValue::Bool(false));
                match view {
                    HostView::Button(v) | HostView::Menu(v) => v.setEnabled(on),
                    HostView::Toggle(v) => v.setEnabled(on),
                    HostView::Slide(v) => v.setEnabled(on),
                    HostView::Segments(v) => v.setEnabled(on),
                    HostView::Step(v) => v.setEnabled(on),
                    HostView::Date(v) => v.setEnabled(on),
                    _ => native.setUserInteractionEnabled(on),
                }
            }
            // --- mapa
            "latitude" | "longitude" | "zoom" | "showsUser"
                if matches!(view, HostView::Map(_)) =>
            {
                let HostView::Map(map) = view else { return };
                if key == "showsUser" {
                    map.setShowsUserLocation(matches!(value, PropValue::Bool(true)));
                    return;
                }
                let entry = self.maps.entry(id).or_insert((0.0, 0.0, 12.0));
                match key {
                    "latitude" => entry.0 = number.unwrap_or(0.0) as f64,
                    "longitude" => entry.1 = number.unwrap_or(0.0) as f64,
                    _ => entry.2 = number.unwrap_or(12.0) as f64,
                }
                let (lat, lon, zoom) = *entry;
                // MapKit no tiene niveles de zoom: tiene cuánto globo se ve.
                // Cada nivel es la mitad del anterior, y el 0 abarca los 360
                // grados de longitud, así que el ancho es 360 / 2^zoom.
                let span = 360.0 / 2f64.powf(zoom.max(0.0));
                map.setRegion_animated(
                    crate::map::MKCoordinateRegion {
                        center: crate::map::CLLocationCoordinate2D {
                            latitude: lat,
                            longitude: lon,
                        },
                        span: crate::map::MKCoordinateSpan {
                            latitude_delta: span,
                            longitude_delta: span,
                        },
                    },
                    false,
                );
            }
            // --- vídeo
            "url" | "playing" | "muted" if matches!(view, HostView::Video(_)) => {
                let HostView::Video(container) = view else { return };
                if key == "url" {
                    let Some(raw) = text.as_deref() else { return };
                    let Some(url) =
                        (unsafe { objc2_foundation::NSURL::URLWithString(&NSString::from_str(raw)) })
                    else {
                        return;
                    };
                    let player = crate::video::AVPlayer::with_url(&url, self.mtm);
                    let controller = crate::video::AVPlayerViewController::new(self.mtm);
                    controller.setPlayer(Some(&player));
                    controller.setShowsPlaybackControls(true);
                    // Contención de verdad: el controlador entra como hijo del
                    // que manda. Colgar solo su vista funciona hasta que algo
                    // —una rotación, el modo pantalla completa— pregunta por
                    // el controlador que la gobierna y no hay ninguno.
                    if let Some(root) =
                        self.container.window().and_then(|w| w.rootViewController())
                    {
                        unsafe { root.addChildViewController(&controller) };
                        let view = controller.view().expect("el controlador trae vista");
                // La vista del controlador es solo el hueco donde va la barra:
                // el contenido lo pone el árbol. Sin esto se ve su fondo
                // blanco por debajo.
                view.setBackgroundColor(None);
                        view.setFrame(container.bounds());
                        container.addSubview(&view);
                        unsafe { controller.didMoveToParentViewController(Some(&root)) };
                    }
                    self.videos.insert(id, (player, controller));
                    return;
                }
                let Some((player, _)) = self.videos.get(&id) else { return };
                match key {
                    "playing" => {
                        if matches!(value, PropValue::Bool(true)) {
                            self.video_playing.insert(id);
                            player.play();
                        } else {
                            self.video_playing.remove(&id);
                            player.pause();
                        }
                    }
                    _ => player.setMuted(matches!(value, PropValue::Bool(true))),
                }
            }
            // --- cabecera de navegación
            "title" | "backTitle" | "showsBack" if matches!(view, HostView::Nav(_)) => {
                let HostView::Nav(bar) = view else { return };
                let entry = self.navs.entry(id).or_default();
                match key {
                    "title" => entry.0 = text.clone().unwrap_or_default(),
                    "backTitle" => entry.1 = text.clone().unwrap_or_default(),
                    _ => entry.2 = matches!(value, PropValue::Bool(true)),
                }
                let (title, back_title, shows_back) = entry.clone();
                let item = objc2_ui_kit::UINavigationItem::new(self.mtm);
                unsafe { item.setTitle(Some(&NSString::from_str(&title))) };
                if shows_back {
                    // Fuera de un `UINavigationController` no hay botón de
                    // atrás automático: se pone uno con el mismo símbolo y el
                    // mismo sitio, y quien navega es el router.
                    // El destino vive tanto como la cabecera: el botón se
                    // rehace en cada cambio de título y el destino no.
                    let target = self
                        .nav_targets
                        .entry(id)
                        .or_insert_with(|| {
                            crate::events::ControlTarget::standalone(
                                self.mtm,
                                id,
                                self.events.clone(),
                            )
                        })
                        .clone();
                    let action = crate::events::ControlTarget::nav_back_action();
                    let back = if back_title.is_empty() {
                        unsafe {
                        objc2_ui_kit::UIBarButtonItem::initWithImage_style_target_action(
                            self.mtm.alloc::<objc2_ui_kit::UIBarButtonItem>(),
                            crate::icons::symbol("chevron.left", 0.0, 400).as_deref(),
                            objc2_ui_kit::UIBarButtonItemStyle::Plain,
                            Some(&*target),
                            Some(action),
                        )
                        }
                    } else {
                        unsafe {
                            objc2_ui_kit::UIBarButtonItem::initWithTitle_style_target_action(
                                self.mtm.alloc::<objc2_ui_kit::UIBarButtonItem>(),
                                Some(&NSString::from_str(&back_title)),
                                objc2_ui_kit::UIBarButtonItemStyle::Plain,
                                Some(&*target),
                                Some(action),
                            )
                        }
                    };
                    unsafe { item.setLeftBarButtonItem(Some(&back)) };
                    self.nav_backs.insert(id, back);
                }
                bar.setItems(Some(&objc2_foundation::NSArray::from_retained_slice(&[item])));
            }
            // --- texto de varias líneas
            "value" if matches!(view, HostView::Area(_)) => {
                let HostView::Area(area) = view else { return };
                let next = text.clone().unwrap_or_default();
                // Igual que en el campo de una línea: no se reescribe si ya
                // dice eso, o el cursor salta al final mientras se escribe.
                let current = unsafe { area.text() }.to_string();
                if current != next {
                    unsafe { area.setText(Some(&NSString::from_str(&next))) };
                }
            }
            "editable" if matches!(view, HostView::Area(_)) => {
                let HostView::Area(area) = view else { return };
                unsafe { area.setEditable(!matches!(value, PropValue::Bool(false))) };
            }
            // --- navegador embebido. No existe en tvOS: sin WebKit no hay
            // vista que cargar, y el nodo ni siquiera se creó.
            #[cfg(not(target_os = "tvos"))]
            "url" if matches!(view, HostView::Web(_)) => {
                let HostView::Web(web) = view else { return };
                let Some(raw) = text.as_deref() else { return };
                let Some(url) = (unsafe {
                    objc2_foundation::NSURL::URLWithString(&NSString::from_str(raw))
                }) else {
                    return;
                };
                let request = unsafe { objc2_foundation::NSURLRequest::requestWithURL(&url) };
                let _ = web.loadRequest(&request);
            }
            #[cfg(not(target_os = "tvos"))]
            "html" if matches!(view, HostView::Web(_)) => {
                let HostView::Web(web) = view else { return };
                let _ = web.loadHTMLString_baseURL(
                    &NSString::from_str(text.as_deref().unwrap_or("")),
                    None,
                );
            }
            // --- control segmentado, desplegable y selector de fecha
            "items" if matches!(view, HostView::Segments(_) | HostView::Menu(_)) => {
                let titles = parse_string_list(text.as_deref().unwrap_or("[]"));
                match view {
                    HostView::Segments(segments) => {
                        segments.removeAllSegments();
                        for (index, title) in titles.iter().enumerate() {
                            unsafe {
                                segments.insertSegmentWithTitle_atIndex_animated(
                                    Some(&NSString::from_str(title)),
                                    index,
                                    false,
                                );
                            }
                        }
                    }
                    HostView::Menu(button) => {
                        self.menus.insert(id, titles.clone());
                        let menu = crate::menu::build(self.mtm, id, &titles, &self.events);
                        unsafe { button.setMenu(Some(&menu)) };
                        // Sin título el botón no se ve: se pone el primero
                        // hasta que alguien elija.
                        let current = titles.first().cloned().unwrap_or_default();
                        unsafe {
                            button.setTitle_forState(
                                Some(&NSString::from_str(&current)),
                                UIControlState::Normal,
                            );
                        }
                    }
                    _ => {}
                }
            }
            "selectedIndex" if matches!(view, HostView::Segments(_) | HostView::Menu(_)) => {
                let index = number.unwrap_or(0.0).max(0.0) as usize;
                match view {
                    HostView::Segments(segments) => {
                        segments.setSelectedSegmentIndex(index as isize)
                    }
                    HostView::Menu(button) => {
                        let title = self
                            .menus
                            .get(&id)
                            .and_then(|titles| titles.get(index))
                            .cloned()
                            .unwrap_or_default();
                        unsafe {
                            button.setTitle_forState(
                                Some(&NSString::from_str(&title)),
                                UIControlState::Normal,
                            );
                        }
                    }
                    _ => {}
                }
            }
            "value" | "minimumValue" | "maximumValue" | "stepValue"
                if matches!(view, HostView::Step(_)) =>
            {
                let HostView::Step(stepper) = view else { return };
                let v = number.unwrap_or(0.0) as f64;
                unsafe {
                    match key {
                        // Igual que el deslizador: el rango antes que el
                        // valor, o el valor se recorta contra el rango viejo.
                        "minimumValue" => stepper.setMinimumValue(v),
                        "maximumValue" => stepper.setMaximumValue(v),
                        "stepValue" => stepper.setStepValue(if v > 0.0 { v } else { 1.0 }),
                        _ => {
                            self.stepper_values.insert(id, v);
                            stepper.setValue(v);
                        }
                    }
                    if key != "value" {
                        if let Some(wanted) = self.stepper_values.get(&id).copied() {
                            stepper.setValue(wanted);
                        }
                    }
                }
            }
            "value" if matches!(view, HostView::Date(_)) => {
                let HostView::Date(picker) = view else { return };
                // Llega en milisegundos desde 1970, que es lo que da `Date` en
                // JS. `NSDate` trabaja en segundos.
                let seconds = number.unwrap_or(0.0) as f64 / 1000.0;
                let date = unsafe {
                    objc2_foundation::NSDate::dateWithTimeIntervalSince1970(seconds)
                };
                unsafe { picker.setDate(&date) };
            }
            "mode" if matches!(view, HostView::Date(_)) => {
                let HostView::Date(picker) = view else { return };
                unsafe {
                    picker.setDatePickerMode(match text.as_deref() {
                        Some("time") => objc2_ui_kit::UIDatePickerMode::Time,
                        Some("dateAndTime") => objc2_ui_kit::UIDatePickerMode::DateAndTime,
                        _ => objc2_ui_kit::UIDatePickerMode::Date,
                    })
                };
            }
            "value" | "placeholder" if matches!(view, HostView::Search(_)) => {
                let HostView::Search(bar) = view else { return };
                let value = text.as_deref().map(NSString::from_str);
                unsafe {
                    if key == "value" {
                        bar.setText(value.as_deref());
                    } else {
                        bar.setPlaceholder(value.as_deref());
                    }
                }
            }
            // Iconos. El nombre y el tamaño van juntos: el tamaño de un
            // símbolo no escala el dibujo, elige el trazo, así que hay que
            // rehacerlo cuando cambia cualquiera de los dos.
            "name" | "iconSize" | "iconWeight" if matches!(view, HostView::Image(_)) => {
                let entry = self.icons.entry(id).or_default();
                match key {
                    "name" => entry.0 = text.clone().unwrap_or_default(),
                    "iconSize" => entry.1 = number.unwrap_or(24.0),
                    _ => entry.2 = number.unwrap_or(400.0) as u16,
                }
                let (name, size, weight) = entry.clone();
                if let HostView::Image(image_view) = view {
                    let symbol = crate::icons::symbol(&name, size, weight);
                    unsafe {
                        let templated = symbol.map(|image| {
                            image.imageWithRenderingMode(
                                objc2_ui_kit::UIImageRenderingMode::AlwaysTemplate,
                            )
                        });
                        image_view.setImage(templated.as_deref());
                    }
                }
            }
            // Animación. No es un valor que se vea: dice cómo se llega a los
            // que sí.
            "animate" => {
                let entry = self.animations.entry(id).or_default();
                // Los milisegundos son lo que se escribe en una plantilla;
                // UIKit trabaja en segundos.
                entry.duration = number.unwrap_or(0.0) as f64 / 1000.0;
            }
            "animateDelay" => {
                let entry = self.animations.entry(id).or_default();
                entry.delay = number.unwrap_or(0.0) as f64 / 1000.0;
            }
            "animateEasing" => {
                let entry = self.animations.entry(id).or_default();
                entry.curve = match text.as_deref() {
                    Some("linear") => UIViewAnimationOptions::CurveLinear,
                    Some("ease-in") => UIViewAnimationOptions::CurveEaseIn,
                    Some("ease-in-out") => UIViewAnimationOptions::CurveEaseInOut,
                    // `ease-out` es lo que se quiere casi siempre: sale rápido
                    // y frena al llegar, que es como se mueven las cosas.
                    _ => UIViewAnimationOptions::CurveEaseOut,
                };
            }
            // Transformaciones. No pasan por el layout a propósito: mover o
            // escalar una vista no cambia el sitio que ocupa, así que no hay
            // que recalcular nada. Es lo que permite seguir al dedo a 120 Hz.
            "translateX" | "translateY" | "scale" | "scaleX" | "scaleY" | "rotate" => {
                let entry = self.transforms.entry(id).or_default();
                let v = number.unwrap_or(0.0) as f64;
                match key {
                    "translateX" => entry.translate_x = v,
                    "translateY" => entry.translate_y = v,
                    "scale" => {
                        entry.scale_x = if number.is_some() { v } else { 1.0 };
                        entry.scale_y = entry.scale_x;
                    }
                    "scaleX" => entry.scale_x = if number.is_some() { v } else { 1.0 },
                    "scaleY" => entry.scale_y = if number.is_some() { v } else { 1.0 },
                    _ => entry.rotate = v,
                }
                let transform = entry.matrix();
                let view = native.retain();
                self.animated(id, move || view.setTransform(transform));
            }
            "borderRadius" | "border-radius" => {
                if let Some(v) = number {
                    self.corners.insert(id, [v as f64; 4]);
                    self.apply_corners(id);
                }
            }
            "borderTopLeftRadius" => self.set_corner(id, 0, number),
            "borderTopRightRadius" => self.set_corner(id, 1, number),
            "borderBottomRightRadius" => self.set_corner(id, 2, number),
            "borderBottomLeftRadius" => self.set_corner(id, 3, number),
            "borderColor" | "border-color" => {
                if let Some(color) = text.as_deref().and_then(crate::color::to_uicolor) {
                    // `CGColor` no está garantizado thread-safe; aquí siempre
                    // estamos en el hilo principal.
                    let cg = unsafe { color.CGColor() };
                    native.layer().setBorderColor(Some(&cg));
                }
            }
            "borderWidth" | "border-width" => {
                if let Some(v) = number {
                    native.layer().setBorderWidth(v as f64);
                }
            }
            // --- diálogos del sistema
            "title" | "message" | "buttons" | "visible" | "sheet"
                if self.alerts.contains_key(&id) =>
            {
                let Some(state) = self.alerts.get_mut(&id) else { return };
                match key {
                    "title" => state.title = text.clone().unwrap_or_default(),
                    "message" => state.message = text.clone().unwrap_or_default(),
                    "buttons" => {
                        state.buttons = parse_string_list(text.as_deref().unwrap_or("[]"))
                    }
                    "sheet" => state.sheet = matches!(value, PropValue::Bool(true)),
                    _ => state.visible = matches!(value, PropValue::Bool(true)),
                }
                if !self.dirty_alerts.contains(&id) {
                    self.dirty_alerts.push(id);
                }
            }
            "transition" => {
                if let Some(direction) = &text {
                    self.transitions.insert(id, direction.clone());
                }
            }
            "source" => {
                if let HostView::Image(image) = view {
                    crate::images::load(
                        self.mtm,
                        image,
                        id,
                        text.as_deref().unwrap_or_default(),
                        self.events.clone(),
                    );
                }
            }
            "resizeMode" => {
                if let HostView::Image(image) = view {
                    image.setContentMode(crate::images::content_mode(
                        text.as_deref().unwrap_or("contain"),
                    ));
                }
            }
            "testID" | "accessibilityIdentifier" => {
                if let Some(t) = &text {
                    native.setAccessibilityIdentifier(Some(&NSString::from_str(t)));
                }
            }
            "color" => {
                let Some(color) = text.as_deref().and_then(crate::color::to_uicolor) else {
                    return;
                };
                match view {
                    HostView::Label(label) => unsafe { label.setTextColor(Some(&color)) },
                    HostView::Field(field) => unsafe { field.setTextColor(Some(&color)) },
                    HostView::Area(area) => unsafe { area.setTextColor(Some(&color)) },
                    HostView::Toggle(toggle) => toggle.setOnTintColor(Some(&color)),
                    HostView::Slide(slider) => slider.setMinimumTrackTintColor(Some(&color)),
                    HostView::Spinner(spinner) => unsafe { spinner.setColor(Some(&color)) },
                    // Un símbolo se tiñe, no se recolorea: se dibuja en
                    // plantilla y el tinte manda.
                    HostView::Image(image) => unsafe { image.setTintColor(Some(&color)) },
                    HostView::Progress(bar) => bar.setProgressTintColor(Some(&color)),
                    HostView::Button(_) => {
                        if let Some(raw) = text.as_deref() {
                            self.button_colors.insert(id, raw.to_owned());
                        }
                        self.refresh_button(id);
                    }
                    HostView::TabsHost(_) => {
                        if let Some(controller) = self.tab_controllers.get(&id) {
                            unsafe { controller.tabBar().setTintColor(Some(&color)) };
                        }
                    }
                    _ => {}
                }
            }
            "textAlign" | "text-align" => {
                let Some(t) = &text else { return };
                let alignment = match t.as_str() {
                    "center" => NSTextAlignment::Center,
                    "right" => NSTextAlignment::Right,
                    "justify" => NSTextAlignment::Justified,
                    _ => NSTextAlignment::Left,
                };
                match view {
                    HostView::Label(label) => label.setTextAlignment(alignment),
                    // El campo y el editor también alinean, y hasta ahora se
                    // quedaban con lo suyo.
                    HostView::Field(field) => unsafe { field.setTextAlignment(alignment) },
                    HostView::Area(area) => unsafe { area.setTextAlignment(alignment) },
                    _ => {}
                }
            }
            "fontSize" => {
                if let Some(v) = number {
                    self.font_mut(id).size = v;
                    self.apply_font(id);
                }
            }
            "fontWeight" => {
                let weight = match value {
                    PropValue::Number(n) => *n as u16,
                    PropValue::Str(s) if s == "bold" => 700,
                    PropValue::Str(s) => s.parse().unwrap_or(400),
                    _ => 400,
                };
                self.font_mut(id).weight = weight;
                self.apply_font(id);
            }
            "fontStyle" => {
                self.font_mut(id).italic = text.as_deref() == Some("italic");
                self.apply_font(id);
            }
            "fontFamily" => {
                self.font_mut(id).family = text.clone();
                self.apply_font(id);
            }
            "textDecoration" | "text-decoration" => {
                self.decorations.insert(id, text.clone().unwrap_or_else(|| "none".to_owned()));
                self.apply_text_attributes(id);
            }
            "letterSpacing" | "letter-spacing" => {
                self.font_mut(id).letter_spacing = number.unwrap_or(0.0);
                self.apply_text_attributes(id);
            }
            "lineHeight" | "line-height" => {
                self.font_mut(id).line_height = number;
                self.apply_text_attributes(id);
            }
            // --- campos de texto
            "value" => {
                if let (HostView::Slide(slider), Some(v)) = (view, number) {
                    self.slider_values.insert(id, v);
                    // Solo si difiere: escribirlo mientras se arrastra pelearía
                    // con el dedo del usuario.
                    if (slider.value() - v).abs() > f32::EPSILON {
                        slider.setValue(v);
                    }
                }
                if let HostView::Field(field) = view {
                    // Escribir el texto mientras el usuario escribe le movería
                    // el cursor al final en cada tecla: solo se aplica si
                    // difiere de verdad.
                    let current = field.text().map(|t| t.to_string()).unwrap_or_default();
                    let next = text.clone().unwrap_or_default();
                    if current != next {
                        field.setText(Some(&NSString::from_str(&next)));
                    }
                }
            }
            "placeholder" => {
                if let HostView::Field(_) = view {
                    self.placeholders.insert(id, text.clone().unwrap_or_default());
                    self.apply_placeholder(id);
                }
            }
            // El texto de ayuda no tiene por qué ir del color del texto, y
            // `UITextField` no tiene una prop para él: hay que dárselo
            // atribuido, así que el color y el texto se guardan juntos.
            "placeholderColor" => {
                match &text {
                    Some(color) => self.placeholder_colors.insert(id, color.clone()),
                    None => self.placeholder_colors.remove(&id),
                };
                self.apply_placeholder(id);
            }
            // Cómo se comporta el teclado. Son las `UITextInputTraits`, que
            // están en el protocolo y valen igual para el campo y el editor.
            "keyboardType" | "returnKeyType" | "autoCapitalize" | "autoCorrect" => match view {
                HostView::Field(field) => apply_text_traits(&**field, key, text.as_deref(), value),
                HostView::Area(area) => apply_text_traits(&**area, key, text.as_deref(), value),
                _ => {}
            },
            "ios:clearButtonMode" => {
                if let HostView::Field(field) = view {
                    unsafe {
                        field.setClearButtonMode(match text.as_deref() {
                            Some("whileEditing") => objc2_ui_kit::UITextFieldViewMode::WhileEditing,
                            Some("always") => objc2_ui_kit::UITextFieldViewMode::Always,
                            _ => objc2_ui_kit::UITextFieldViewMode::Never,
                        })
                    };
                }
            }
            "ios:borderStyle" => {
                if let HostView::Field(field) = view {
                    unsafe {
                        field.setBorderStyle(match text.as_deref() {
                            Some("line") => objc2_ui_kit::UITextBorderStyle::Line,
                            Some("bezel") => objc2_ui_kit::UITextBorderStyle::Bezel,
                            Some("roundedRect") => objc2_ui_kit::UITextBorderStyle::RoundedRect,
                            _ => objc2_ui_kit::UITextBorderStyle::None,
                        })
                    };
                }
            }
            "secureTextEntry" => {
                if let HostView::Field(field) = view {
                    field.setSecureTextEntry(matches!(value, PropValue::Bool(true)));
                }
            }
            "editable" => {
                if let HostView::Field(field) = view {
                    field.setEnabled(!matches!(value, PropValue::Bool(false)));
                }
            }
            // --- controles del sistema
            "on" => {
                if let HostView::Toggle(toggle) = view {
                    toggle.setOn(matches!(value, PropValue::Bool(true)));
                }
            }
            "minimumValue" | "maximumValue" => {
                let (HostView::Slide(slider), Some(v)) = (view, number) else { return };
                if key == "minimumValue" {
                    slider.setMinimumValue(v);
                } else {
                    slider.setMaximumValue(v);
                }
                // El rango cambió: hay que volver a aplicar el valor, que
                // pudo llegar antes y quedarse recortado.
                if let Some(wanted) = self.slider_values.get(&id).copied() {
                    slider.setValue(wanted);
                }
            }
            // Los colores sueltos del interruptor y del deslizador. `[color]`
            // sigue siendo el principal —lo encendido, el tramo recorrido—;
            // estos son los otros.
            "thumbColor" => {
                let Some(color) = text.as_deref().and_then(crate::color::to_uicolor) else {
                    return;
                };
                match view {
                    HostView::Toggle(toggle) => toggle.setThumbTintColor(Some(&color)),
                    HostView::Slide(slider) => slider.setThumbTintColor(Some(&color)),
                    _ => {}
                }
            }
            "minimumTrackColor" | "maximumTrackColor" => {
                let (HostView::Slide(slider), Some(color)) =
                    (view, text.as_deref().and_then(crate::color::to_uicolor))
                else {
                    return;
                };
                if key == "minimumTrackColor" {
                    slider.setMinimumTrackTintColor(Some(&color));
                } else {
                    slider.setMaximumTrackTintColor(Some(&color));
                }
            }
            "ios:continuous" => {
                if let HostView::Slide(slider) = view {
                    // Apagado, el deslizador solo avisa al soltar. Sirve para
                    // lo que cuesta caro recalcular en cada punto.
                    slider.setContinuous(!matches!(value, PropValue::Bool(false)));
                }
            }
            "animating" => {
                if let HostView::Spinner(spinner) = view {
                    if matches!(value, PropValue::Bool(false)) {
                        spinner.stopAnimating();
                    } else {
                        spinner.startAnimating();
                    }
                }
            }
            "progress" => {
                if let (HostView::Progress(bar), Some(v)) = (view, number) {
                    bar.setProgress(v.clamp(0.0, 1.0));
                }
            }
            "title" => {
                if matches!(view, HostView::Button(_)) {
                    self.button_titles.insert(id, text.clone().unwrap_or_default());
                    self.refresh_button(id);
                }
            }
            "items" | "icons" if matches!(view, HostView::TabsHost(_)) => {
                // Los títulos y los iconos llegan como JSON: el protocolo no
                // lleva listas, y una barra de pestañas no justifica
                // añadirlas. Llegan en props sueltas, así que se guardan y se
                // rehacen las pestañas con las dos cada vez.
                let entry = self.tabs.entry(id).or_default();
                let list = parse_string_list(text.as_deref().unwrap_or("[]"));
                if key == "items" {
                    entry.0 = list;
                } else {
                    entry.1 = list;
                }
                let (titles, icons) = entry.clone();
                let Some(controller) = self.tab_controllers.get(&id) else { return };
                // Una pestaña de `UITabBarController` es un controlador con su
                // `tabBarItem`. Los de aquí van vacíos: el contenido lo pone
                // el árbol, no ellos; lo que se quiere del controlador es que
                // dibuje y coloque la barra como manda el sistema.
                let controllers: Vec<Retained<objc2_ui_kit::UIViewController>> = titles
                    .iter()
                    .enumerate()
                    .map(|(index, title)| {
                        let vc = objc2_ui_kit::UIViewController::new(self.mtm);
                        let image =
                            icons.get(index).and_then(|name| crate::icons::symbol(name, 0.0, 400));
                        let item = unsafe {
                            objc2_ui_kit::UITabBarItem::initWithTitle_image_tag(
                                self.mtm.alloc::<objc2_ui_kit::UITabBarItem>(),
                                Some(&NSString::from_str(title)),
                                image.as_deref(),
                                index as isize,
                            )
                        };
                        unsafe { vc.setTabBarItem(Some(&item)) };
                        vc
                    })
                    .collect();
                unsafe {
                    controller.setViewControllers(Some(
                        &objc2_foundation::NSArray::from_retained_slice(&controllers),
                    ))
                };
            }
            "selectedIndex" => {
                if let (HostView::TabsHost(_), Some(index)) = (view, number) {
                    if let Some(controller) = self.tab_controllers.get(&id) {
                        unsafe { controller.setSelectedIndex(index.max(0.0) as usize) };
                    }
                }
            }
            "presentation" if self.modals.contains_key(&id) => {
                if let Some(state) = self.modals.get_mut(&id) {
                    state.sheet = text.as_deref() == Some("sheet");
                }
                if !self.dirty_modals.contains(&id) {
                    self.dirty_modals.push(id);
                }
            }
            "visible" if self.modals.contains_key(&id) => {
                if let Some(state) = self.modals.get_mut(&id) {
                    state.visible = matches!(value, PropValue::Bool(true));
                }
                // Escondida mientras no esté presentada. Al presentarla, es
                // el controlador quien la enseña; si se dejara visible sin
                // presentar, el contenido del modal se dibujaría en línea
                // sobre la página.
                let visible = self.modals.get(&id).is_some_and(|m| m.visible);
                native.setHidden(!visible);
                if !self.dirty_modals.contains(&id) {
                    self.dirty_modals.push(id);
                }
            }
            "visible" => {
                if let HostView::Overlay(overlay) = view {
                    overlay.setHidden(matches!(value, PropValue::Bool(false)));
                }
            }
            // --- scroll
            //
            // Tirar para recargar es de iOS: `UIRefreshControl` no está en el
            // SDK de tvOS. En una tele no hay de dónde tirar, así que la prop
            // no tiene a quién hablarle. Se avisa al suscribirse, en
            // `events::attach`, y no aquí: repetirlo en cada cambio de valor
            // llenaría el log del mismo aviso.
            #[cfg(not(target_os = "tvos"))]
            "refreshing" => {
                // El control lo trajo la suscripción a `refresh`: si nadie
                // escucha, no hay nada que parar.
                if let Some(control) = self
                    .listeners
                    .get(&(id, "refresh".to_owned()))
                    .and_then(crate::events::AttachedListener::refresh_control)
                {
                    if matches!(value, PropValue::Bool(true)) {
                        control.beginRefreshing();
                    } else {
                        control.endRefreshing();
                    }
                }
            }
            "showsScrollIndicator" => {
                if let HostView::Scroll(scroll) = view {
                    let shown = !matches!(value, PropValue::Bool(false));
                    scroll.setShowsVerticalScrollIndicator(shown);
                    scroll.setShowsHorizontalScrollIndicator(shown);
                }
            }
            "bounces" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setBounces(!matches!(value, PropValue::Bool(false)));
                }
            }
            "scrollEnabled" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setScrollEnabled(!matches!(value, PropValue::Bool(false)));
                }
            }
            "ios:pagingEnabled" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setPagingEnabled(matches!(value, PropValue::Bool(true)));
                }
            }
            "ios:keyboardDismissMode" => {
                if let HostView::Scroll(scroll) = view {
                    scroll.setKeyboardDismissMode(match text.as_deref() {
                        Some("onDrag") => {
                            objc2_ui_kit::UIScrollViewKeyboardDismissMode::OnDrag
                        }
                        Some("interactive") => {
                            objc2_ui_kit::UIScrollViewKeyboardDismissMode::Interactive
                        }
                        _ => objc2_ui_kit::UIScrollViewKeyboardDismissMode::None,
                    });
                }
            }
            // --- barra de pestañas
            "unselectedColor" => {
                let (HostView::TabsHost(_), Some(controller)) =
                    (view, self.tab_controllers.get(&id))
                else {
                    return;
                };
                let color = text.as_deref().and_then(crate::color::to_uicolor);
                unsafe { controller.tabBar().setUnselectedItemTintColor(color.as_deref()) };
            }
            "ios:translucent" => {
                if let Some(controller) = self.tab_controllers.get(&id) {
                    // Con la barra opaca, lo que hay debajo deja de verse a
                    // través: sirve cuando el contenido se lee mal detrás.
                    unsafe {
                        controller.tabBar().setTranslucent(!matches!(value, PropValue::Bool(false)))
                    };
                }
            }
            "numberOfLines" => {
                self.font_mut(id).max_lines = number.filter(|v| *v >= 1.0).map(|v| v as u32);
                self.apply_font(id);
            }
            _ => {}
        }
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        if let Some(label) = self.views.get(&id).and_then(HostView::as_label) {
            label.setText(Some(&NSString::from_str(text)));
            // `setText` tira el texto atribuido, así que el espaciado y el
            // interlineado hay que volver a ponerlos con cada palabra nueva.
            self.apply_text_attributes(id);
        }
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let key = (id, event.to_owned());
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();

        // El área segura no la produce ningún gesto: la sabe el sistema, y
        // cambia al rotar o al aparecer el teclado. Se cuenta al suscribirse y
        // después en cada layout, que es cuando puede haber cambiado.
        if event == "safeArea" {
            if enabled {
                self.safe_area.insert(id, [f32::NAN; 4]);
                self.report_safe_area(id);
            } else {
                self.safe_area.remove(&id);
            }
            return;
        }

        if !enabled {
            if let Some(listener) = self.listeners.remove(&key) {
                listener.detach(native);
            }
            return;
        }
        if self.listeners.contains_key(&key) {
            return;
        }
        let kind = view.kind();
        if let Some(listener) =
            crate::events::attach(self.mtm, native, kind, id, event, self.events.clone())
        {
            self.listeners.insert(key, listener);
        }
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view().retain();
        let rect = CGRect {
            origin: CGPoint { x: frame.x as f64, y: frame.y as f64 },
            size: CGSize { width: frame.width as f64, height: frame.height as f64 },
        };
        self.animated(id, move || native.setFrame(rect));
        if self.safe_area.contains_key(&id) {
            self.report_safe_area(id);
        }
        // Una máscara de esquinas desiguales no se estira con la vista: hay
        // que redibujarla con el tamaño nuevo.
        if self
            .corners
            .get(&id)
            .is_some_and(|radii| radii.iter().any(|r| (*r - radii[0]).abs() > f64::EPSILON))
        {
            self.apply_corners(id);
        }
    }

    fn clear(&mut self) {
        for view in self.views.values() {
            view.as_view().removeFromSuperview();
        }
        self.views.clear();
        self.fonts.clear();
        self.corners.clear();
        self.listeners.clear();
    }

    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        if let Some(HostView::Scroll(scroll)) = self.views.get(&id) {
            scroll.setContentSize(CGSize { width: width as f64, height: height as f64 });
        }
    }

    fn flush(&mut self) {
        // Volver a pedir que suene lo que debería estar sonando.
        //
        // `play()` sobre un reproductor que todavía no ha cargado nada no
        // prende: el `rate` se queda en cero y ahí se queda para siempre, sin
        // error y con la capa en negro. Como la prop `playing` llega una sola
        // vez, hay que reintentarlo hasta que agarre.
        for (id, (player, _)) in &self.videos {
            if self.video_playing.contains(id) && player.rate() == 0.0 && player.status() == 1 {
                player.play();
            }
        }

        // La vista del reproductor al tamaño de la suya.
        //
        // No basta con hacerlo en `set_layout`: el reproductor se crea cuando
        // llega la dirección del vídeo, que es *después* de que el marco esté
        // puesto, así que nace con cero de ancho y nadie vuelve a tocarlo.
        for (id, (_, controller)) in &self.videos {
            let Some(view) = self.views.get(id) else { continue };
            let Some(inner) = controller.view() else { continue };
            let bounds = view.as_view().bounds();
            if inner.frame().size != bounds.size {
                inner.setFrame(bounds);
            }
        }
        self.run_stack_animations();
        for id in std::mem::take(&mut self.dirty_alerts) {
            let Some(mut state) = self.alerts.remove(&id) else { continue };
            state.sync(self.mtm, &self.container, id, &self.events);
            self.alerts.insert(id, state);
        }
        // Presentar tiene que ir después del layout: el controlador se lleva
        // la vista tal como esté, y si el marco todavía no está calculado se
        // presenta una caja vacía.
        for id in std::mem::take(&mut self.dirty_modals) {
            let Some(mut state) = self.modals.remove(&id) else { continue };
            if let Some(content) = self.views.get(&id).map(|v| v.as_view().retain()) {
                state.sync(self.mtm, &self.container, &content, id, &self.events);
            }
            self.modals.insert(id, state);
        }
    }

    fn set_root(&mut self, id: NodeId) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        if native.superview().is_none() {
            self.container.addSubview(native);
        }
    }
}
