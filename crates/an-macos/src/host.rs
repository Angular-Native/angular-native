//! `HostRenderer` sobre AppKit. Una vista nativa por nodo montable, colocada
//! con `frame` directo: el layout ya lo resolvió taffy, y meter Auto Layout
//! aquí solo pondría un segundo motor de layout a competir con el primero.
//!
//! Es el mismo planteamiento que el host de iOS. Lo que cambia está donde se
//! nota, y está comentado ahí: el origen de coordenadas (`flipped.rs`), los
//! gestos de ratón (`events.rs`), y que aquí no hay un `_ => {}` al final del
//! `match` de props.
//!
//! **Por qué no hay `_ => {}`.** Una prop que un host no mira no da error, no
//! deja traza y no cambia nada: es exactamente el fallo que este proyecto
//! persigue. El host de iOS y el de Android se cubren con
//! `scripts/check-wrapper.sh`, que comprueba que el nombre aparezca en el
//! fichero. Aquí, además, lo que no se aplica se dice por la salida de error la
//! primera vez que llega, con el motivo: `IGNORED` es la lista de props que
//! macOS no puede honrar, y cualquier cosa que no esté ni implementada ni en
//! esa lista sale por pantalla como «prop desconocida».

use std::collections::{HashMap, HashSet};

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{EventQueue, HostRenderer};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, Message};
use objc2_app_kit::{
    NSAnimationContext, NSBezelStyle, NSButton, NSControl, NSControlStateValueOff,
    NSControlStateValueOn, NSDatePicker, NSDatePickerMode, NSFont, NSForegroundColorAttributeName,
    NSImageView, NSKernAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSPopUpButton, NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSScrollerStyle,
    NSSearchField, NSSegmentedControl, NSSlider, NSStepper, NSStrikethroughStyleAttributeName,
    NSSwitch, NSTextAlignment, NSTextField, NSTextView, NSUnderlineStyleAttributeName,
    NSUserInterfaceItemIdentification, NSView,
};
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_foundation::{
    NSAttributedString, NSMutableAttributedString, NSNumber, NSRange, NSString,
};

use crate::flipped::FlippedView;
use crate::support::{is_known_event, support, unsupported_event, Support};

/// Lista de cadenas en JSON, sin traerse un analizador entero para esto.
/// Idéntica a la del host de iOS, y por lo mismo: solo tiene que entender lo
/// que genera el lado JS.
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

/// Props que llegan a este host y que macOS no puede honrar, con el motivo.
///
/// Estar en esta lista es una decisión, no un olvido: la prop se descarta a
/// sabiendas y sin ruido. Lo que no esté ni aquí ni implementado sale por la
/// salida de error, que es lo que hace que la lista no se pueda quedar atrás.
const IGNORED: &[(&str, &str)] = &[
    // La escribe Angular en el elemento raíz de la app; no sale de ninguna
    // plantilla ni de ninguna primitiva.
    ("ng-version", "la escribe Angular en su raíz, no es una prop de ninguna primitiva"),
    // El núcleo las consume para reservar el hueco de una imagen; no llegan a
    // ser props de vista en ningún host.
    ("intrinsicWidth", "la consume el layout, no el host"),
    ("intrinsicHeight", "la consume el layout, no el host"),
    // No hay teclado en pantalla en un Mac, así que no hay nada que
    // configurar: el teclado es de hardware y no lo elige la vista.
    ("keyboardType", "un Mac no tiene teclado en pantalla"),
    ("returnKeyType", "un Mac no tiene teclado en pantalla"),
    ("autoCapitalize", "un Mac no tiene teclado en pantalla"),
    ("autoCorrect", "la corrección en macOS es un ajuste del sistema, no de la vista"),
    // `NSSecureTextField` es otra clase, y una vista no puede cambiar de clase
    // una vez creada. La prop llega después de crear el campo.
    (
        "secureTextEntry",
        "en AppKit el campo de contraseña es otra clase (NSSecureTextField) y no se puede \
         cambiar en marcha",
    ),
    // `NSSlider` tiñe el tramo recorrido y nada más.
    ("thumbColor", "NSSlider no expone el color del pulgar"),
    ("maximumTrackColor", "NSSlider solo tiñe el tramo recorrido"),
    // Tirar para recargar es un gesto de dedo. En escritorio se recarga con un
    // botón o con ⌘R, que son cosa de la app.
    ("refreshing", "no hay «tirar para recargar» en escritorio"),
    ("bounces", "NSScrollView no rebota al final como el de iOS"),
    // La pila de pantallas se monta y se desmonta, pero no se anima: las
    // transiciones de `an-native-stack` están sin portar a este host.
    ("transition", "las transiciones de pila todavía no están portadas a este host"),
    // La cabecera de macOS es la barra de título de la ventana.
    ("title", "en un <an-navigation-bar>: la cabecera de macOS es la barra de título"),
    ("backTitle", "la cabecera de macOS es la barra de título de la ventana"),
    ("showsBack", "la cabecera de macOS es la barra de título de la ventana"),
    // Primitivas sin portar (ver `support.rs`).
    ("latitude", "MapView no está portado a este host"),
    ("longitude", "MapView no está portado a este host"),
    ("zoom", "MapView no está portado a este host"),
    ("showsUser", "MapView no está portado a este host"),
    ("playing", "VideoView no está portado a este host"),
    ("muted", "VideoView no está portado a este host"),
    // El modal de este host es una capa por encima del contenido, no una hoja
    // ni un panel: no hay dos presentaciones entre las que elegir.
    ("presentation", "el modal de este host es una capa, no hay hoja que elegir"),
    // `NSSegmentedControl` tiñe todo el control por igual.
    ("unselectedColor", "NSSegmentedControl no tiñe los segmentos apagados por separado"),
];

fn ignored_reason(key: &str) -> Option<&'static str> {
    IGNORED.iter().find(|(k, _)| *k == key).map(|(_, reason)| *reason)
}

/// Vista nativa de un nodo, con su tipo concreto: las props de un rótulo no se
/// aplican igual que las de una caja.
enum HostView {
    View(Retained<FlippedView>),
    Stack(Retained<FlippedView>),
    Label(Retained<NSTextField>),
    Field(Retained<NSTextField>),
    Area(Retained<NSTextView>),
    Image(Retained<NSImageView>),
    Icon(Retained<NSImageView>),
    Scroll(Retained<NSScrollView>),
    Button(Retained<NSButton>),
    Toggle(Retained<NSSwitch>),
    Slide(Retained<NSSlider>),
    Spinner(Retained<NSProgressIndicator>),
    Progress(Retained<NSProgressIndicator>),
    Segments(Retained<NSSegmentedControl>),
    /// La barra de pestañas de macOS es un segmentado (ver `support.rs`).
    Tabs(Retained<NSSegmentedControl>),
    Step(Retained<NSStepper>),
    Search(Retained<NSSearchField>),
    Menu(Retained<NSPopUpButton>),
    Date(Retained<NSDatePicker>),
    Web(Retained<crate::web::WKWebView>),
    /// Capa por encima de la raíz.
    Overlay(Retained<FlippedView>),
    /// Un diálogo no tiene vista: lo presenta el sistema. Se monta una vacía
    /// para que el árbol tenga algo donde colgar el nodo.
    Dialog(Retained<FlippedView>),
    /// Primitiva que este host no cubre. La vista existe para que el árbol no
    /// se descuadre, pero no dibuja nada y se avisó al crearla.
    Unsupported(Retained<FlippedView>, NodeKind),
}

impl HostView {
    fn as_view(&self) -> &NSView {
        match self {
            HostView::View(v) | HostView::Stack(v) | HostView::Overlay(v) | HostView::Dialog(v) => {
                v
            }
            HostView::Unsupported(v, _) => v,
            HostView::Label(v) | HostView::Field(v) => v,
            HostView::Area(v) => v,
            HostView::Image(v) | HostView::Icon(v) => v,
            HostView::Scroll(v) => v,
            HostView::Button(v) => v,
            HostView::Toggle(v) => v,
            HostView::Slide(v) => v,
            HostView::Spinner(v) | HostView::Progress(v) => v,
            HostView::Segments(v) | HostView::Tabs(v) => v,
            HostView::Step(v) => v,
            HostView::Search(v) => v,
            HostView::Menu(v) => v,
            HostView::Date(v) => v,
            HostView::Web(v) => v,
        }
    }

    fn kind(&self) -> NodeKind {
        match self {
            HostView::View(_) => NodeKind::View,
            HostView::Stack(_) => NodeKind::StackView,
            HostView::Label(_) => NodeKind::Text,
            HostView::Field(_) => NodeKind::TextInput,
            HostView::Area(_) => NodeKind::TextEditor,
            HostView::Image(_) => NodeKind::Image,
            HostView::Icon(_) => NodeKind::Icon,
            HostView::Scroll(_) => NodeKind::ScrollView,
            HostView::Button(_) => NodeKind::Button,
            HostView::Toggle(_) => NodeKind::Switch,
            HostView::Slide(_) => NodeKind::Slider,
            HostView::Spinner(_) => NodeKind::ActivityIndicator,
            HostView::Progress(_) => NodeKind::ProgressBar,
            HostView::Segments(_) => NodeKind::SegmentedControl,
            HostView::Tabs(_) => NodeKind::TabBar,
            HostView::Step(_) => NodeKind::Stepper,
            HostView::Search(_) => NodeKind::SearchBar,
            HostView::Menu(_) => NodeKind::Picker,
            HostView::Date(_) => NodeKind::DatePicker,
            HostView::Web(_) => NodeKind::WebView,
            HostView::Overlay(_) => NodeKind::Modal,
            HostView::Dialog(_) => NodeKind::Alert,
            HostView::Unsupported(_, kind) => *kind,
        }
    }

    /// Dónde van los hijos de esta vista. Para casi todas es ella misma; un
    /// `NSScrollView` es la excepción: sus hijos cuelgan del documento, no del
    /// marco que lo enseña.
    fn content_view(&self) -> Retained<NSView> {
        match self {
            HostView::Scroll(scroll) => unsafe { scroll.documentView() }
                .unwrap_or_else(|| self.as_view().retain()),
            _ => self.as_view().retain(),
        }
    }
}

/// Las partes de una transformación, sin componer. La escala arranca en 1 y no
/// en 0: una vista sin `scale` tiene que verse igual que antes de que la prop
/// existiera, no desaparecer.
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
        Transform { translate_x: 0.0, translate_y: 0.0, scale_x: 1.0, scale_y: 1.0, rotate: 0.0 }
    }
}

impl Transform {
    fn is_identity(&self) -> bool {
        self.scale_x == 1.0 && self.scale_y == 1.0 && self.rotate == 0.0
    }

    /// Escalar y girar, en ese orden. El desplazamiento no entra aquí: se
    /// aplica sumándolo al marco en `set_layout`, que en AppKit es exacto y
    /// evita tener que compensar el punto de anclaje de la capa dos veces.
    fn matrix(&self) -> CGAffineTransform {
        let (sin, cos) = self.rotate.sin_cos();
        CGAffineTransform {
            a: self.scale_x * cos,
            b: self.scale_x * sin,
            c: -self.scale_y * sin,
            d: self.scale_y * cos,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

/// Cómo anima una vista sus cambios.
#[derive(Clone, Copy, Default)]
struct Animation {
    /// Segundos. Cero apaga la animación sin borrar el resto de ajustes.
    duration: f64,
    delay: f64,
}

pub struct AppKitHost {
    mtm: MainThreadMarker,
    /// Vista que da el shell. La raíz del árbol cuelga de aquí.
    container: Retained<NSView>,
    views: HashMap<NodeId, HostView>,
    fonts: HashMap<NodeId, an_layout::FontSpec>,
    /// Radios por esquina: arriba-izq, arriba-der, abajo-der, abajo-izq.
    corners: HashMap<NodeId, [f64; 4]>,
    /// El marco que mandó el core, sin el desplazamiento. Hace falta guardarlo
    /// porque `translateX` puede llegar después del marco y hay que recolocar.
    frames: HashMap<NodeId, Rect>,
    transforms: HashMap<NodeId, Transform>,
    animations: HashMap<NodeId, Animation>,
    /// Nombre, tamaño y peso del icono de cada nodo, que llegan sueltos.
    icons: HashMap<NodeId, (String, f32, u16)>,
    /// Títulos e iconos de cada barra de pestañas y de cada segmentado.
    segments: HashMap<NodeId, (Vec<String>, Vec<String>)>,
    /// Subrayado o tachado de cada rótulo.
    decorations: HashMap<NodeId, String>,
    placeholders: HashMap<NodeId, String>,
    placeholder_colors: HashMap<NodeId, String>,
    /// Título, color, variante e icono de cada botón: al cambiar cualquiera
    /// hay que rehacer los cuatro, igual que en iOS.
    button_titles: HashMap<NodeId, String>,
    button_colors: HashMap<NodeId, String>,
    button_variants: HashMap<NodeId, String>,
    button_icons: HashMap<NodeId, (String, String)>,
    /// Valor pedido al deslizador y al `Stepper`. Se guardan porque el valor y
    /// el rango llegan en props sueltas y en cualquier orden: fijar el valor
    /// antes que el máximo lo recorta contra el rango viejo.
    slider_values: HashMap<NodeId, f64>,
    stepper_values: HashMap<NodeId, f64>,
    alerts: HashMap<NodeId, crate::alert::AlertState>,
    dirty_alerts: Vec<NodeId>,
    listeners: HashMap<(NodeId, String), crate::events::AttachedListener>,
    /// Lo que ya se avisó, para no repetirlo sesenta veces por segundo.
    warned: HashSet<String>,
    events: EventQueue,
}

impl AppKitHost {
    /// # Safety
    /// `container` tiene que ser una `NSView` viva y hay que llamar desde el
    /// hilo principal.
    pub fn new(mtm: MainThreadMarker, container: Retained<NSView>, events: EventQueue) -> Self {
        AppKitHost {
            mtm,
            container,
            views: HashMap::new(),
            fonts: HashMap::new(),
            corners: HashMap::new(),
            frames: HashMap::new(),
            transforms: HashMap::new(),
            animations: HashMap::new(),
            icons: HashMap::new(),
            segments: HashMap::new(),
            decorations: HashMap::new(),
            placeholders: HashMap::new(),
            placeholder_colors: HashMap::new(),
            button_titles: HashMap::new(),
            button_colors: HashMap::new(),
            button_variants: HashMap::new(),
            button_icons: HashMap::new(),
            slider_values: HashMap::new(),
            stepper_values: HashMap::new(),
            alerts: HashMap::new(),
            dirty_alerts: Vec::new(),
            listeners: HashMap::new(),
            warned: HashSet::new(),
            events,
        }
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    /// Avisa una vez y calla las siguientes. El core manda la misma prop en
    /// cada cambio, así que sin esto un aviso serían miles de líneas.
    fn warn_once(&mut self, key: String, message: impl FnOnce()) {
        if self.warned.insert(key) {
            message();
        }
    }

    /// La `NSFont` que pide un `FontSpec`.
    fn build_font(&self, spec: &an_layout::FontSpec) -> Retained<NSFont> {
        crate::measure::AppKitMeasurer::nsfont(spec)
    }

    fn font_mut(&mut self, id: NodeId) -> &mut an_layout::FontSpec {
        self.fonts.entry(id).or_default()
    }

    fn apply_font(&mut self, id: NodeId) {
        let Some(spec) = self.fonts.get(&id).cloned() else { return };
        let font = self.build_font(&spec);
        let Some(view) = self.views.get(&id) else { return };
        match view {
            HostView::Label(label) | HostView::Field(label) => unsafe {
                label.setFont(Some(&font));
                // Cero significa «las que hagan falta», igual que en UIKit.
                label.setMaximumNumberOfLines(spec.max_lines.unwrap_or(0) as isize);
            },
            HostView::Area(area) => unsafe { area.setFont(Some(&font)) },
            HostView::Search(search) => unsafe { search.setFont(Some(&font)) },
            HostView::Button(_) => {
                self.refresh_button(id);
                return;
            }
            _ => return,
        }
        self.apply_text_attributes(id);
    }

    /// Interlineado, espaciado entre letras y subrayado, que `NSTextField` no
    /// tiene como propiedades. El núcleo ya mide con los dos primeros, así que
    /// sin esto el layout reservaría un hueco que el texto no llena.
    fn apply_text_attributes(&self, id: NodeId) {
        let Some(HostView::Label(label)) = self.views.get(&id) else { return };
        let spec = self.fonts.get(&id);
        let decoration = self.decorations.get(&id).map(String::as_str).unwrap_or("none");
        let spacing = spec.map(|s| s.letter_spacing).unwrap_or(0.0);
        let line_height = spec.and_then(|s| s.line_height);
        if spacing == 0.0 && line_height.is_none() && decoration == "none" {
            return;
        }

        let text = unsafe { label.stringValue() };
        let attributed = unsafe {
            NSMutableAttributedString::initWithString(
                NSMutableAttributedString::alloc(),
                &text,
            )
        };
        let range = NSRange { location: 0, length: text.len_utf16() };
        unsafe {
            if spacing != 0.0 {
                attributed.addAttribute_value_range(
                    NSKernAttributeName,
                    &NSNumber::new_f64(spacing as f64),
                    range,
                );
            }
            if let Some(height) = line_height {
                let style = NSMutableParagraphStyle::new();
                style.setMinimumLineHeight(height as f64);
                style.setMaximumLineHeight(height as f64);
                // Sin esto el texto largo deja de partir en líneas al ponerle
                // un estilo de párrafo: el modo por defecto de un estilo nuevo
                // es recortar, no ajustar.
                style.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByWordWrapping);
                style.setAlignment(label.alignment());
                attributed.addAttribute_value_range(
                    NSParagraphStyleAttributeName,
                    &style,
                    range,
                );
            }
            let underline = match decoration {
                "underline" => Some(NSUnderlineStyleAttributeName),
                "line-through" => Some(NSStrikethroughStyleAttributeName),
                _ => None,
            };
            if let Some(key) = underline {
                attributed.addAttribute_value_range(key, &NSNumber::new_i64(1), range);
            }
            // La fuente y el color no se tocan: sin ellos en los atributos,
            // `NSTextField` sigue usando los suyos y `[color]`/`[fontSize]`
            // funcionan como antes.
            label.setAttributedStringValue(&attributed);
        }
    }

    /// Vuelve a poner el texto de ayuda con su color. Sin color se pone llano:
    /// el atribuido sin atributos se dibuja distinto del que pone AppKit.
    fn apply_placeholder(&self, id: NodeId) {
        let field = match self.views.get(&id) {
            Some(HostView::Field(field)) => &**field,
            Some(HostView::Search(search)) => &***search,
            _ => return,
        };
        let Some(placeholder) = self.placeholders.get(&id) else { return };
        let string = NSString::from_str(placeholder);
        let Some(color) =
            self.placeholder_colors.get(&id).and_then(|raw| crate::color::to_nscolor(raw))
        else {
            unsafe { field.setPlaceholderString(Some(&string)) };
            return;
        };
        let attrs = objc2_foundation::NSDictionary::from_slices(
            &[unsafe { NSForegroundColorAttributeName }],
            &[&*color as &objc2::runtime::AnyObject],
        );
        // SAFETY: el diccionario lleva un `NSColor` bajo la clave de color, que
        // es el tipo que ese atributo espera.
        let attributed = unsafe { NSAttributedString::new_with_attributes(&string, &attrs) };
        unsafe { field.setPlaceholderAttributedString(Some(&attributed)) };
    }

    /// Corre un cambio dentro de una animación si el nodo la pidió.
    ///
    /// `allowsImplicitAnimation` es lo que hace que los setters normales
    /// animen: en AppKit lo habitual es escribir sobre `view.animator()`, pero
    /// eso obliga a tener el proxy del tipo concreto en cada sitio. Con el
    /// grupo abierto y las animaciones implícitas encendidas, un `setFrame`
    /// corriente ya va animado, y el mismo cierre vale para todos los
    /// controles.
    fn animated(&self, id: NodeId, change: impl Fn()) {
        let animation = self.animations.get(&id).copied().unwrap_or_default();
        if animation.duration <= 0.0 {
            change();
            return;
        }
        // El retraso no lo tiene `NSAnimationContext`: se consigue metiendo el
        // grupo en la cola principal más tarde. Como el cierre no puede cruzar
        // ahí sin ser `'static`, un retraso pedido se aplica como duración
        // total y se dice.
        let duration = animation.duration + animation.delay;
        let block = RcBlock::new(move |context: core::ptr::NonNull<NSAnimationContext>| {
            let context = unsafe { context.as_ref() };
            unsafe {
                context.setDuration(duration);
                context.setAllowsImplicitAnimation(true);
            }
            change();
        });
        unsafe { NSAnimationContext::runAnimationGroup(&block) };
    }

    /// Rehace el botón entero: título, color, variante e icono llegan sueltos y
    /// cambiar la variante se lleva por delante lo demás.
    fn refresh_button(&self, id: NodeId) {
        let Some(HostView::Button(button)) = self.views.get(&id) else { return };
        let title = self.button_titles.get(&id).cloned().unwrap_or_default();
        let variant = self.button_variants.get(&id).map(String::as_str).unwrap_or("text");
        let color = self.button_colors.get(&id).and_then(|raw| crate::color::to_nscolor(raw));

        unsafe {
            button.setTitle(&NSString::from_str(&title));
            button.setBezelStyle(NSBezelStyle::Push);
            if let Some(spec) = self.fonts.get(&id) {
                button.setFont(Some(&self.build_font(spec)));
            }
            match variant {
                // Sin marco: es el botón de solo texto de macOS, el que se usa
                // en las barras y en los enlaces de una hoja.
                "text" => {
                    button.setBordered(false);
                    button.setBezelColor(None);
                    button.setContentTintColor(color.as_deref());
                }
                // Relleno: el color va al bisel y el rótulo se pinta del color
                // que contraste, que lo calcula el núcleo.
                "filled" => {
                    button.setBordered(true);
                    button.setBezelColor(color.as_deref());
                    let contrast = self
                        .button_colors
                        .get(&id)
                        .and_then(|raw| crate::color::contrasting(raw));
                    button.setContentTintColor(contrast.as_deref());
                }
                // Tonal: el mismo color, apagado. `NSColor` sabe mezclarse con
                // el fondo, así que no hay que inventar un segundo tono.
                "tonal" => {
                    button.setBordered(true);
                    let tinted = color.as_ref().map(|c| {
                        c.colorWithAlphaComponent(0.25)
                    });
                    button.setBezelColor(tinted.as_deref());
                    button.setContentTintColor(color.as_deref());
                }
                // Contorno: marco del sistema y rótulo del color pedido.
                // AppKit no deja pintar el borde del bisel por separado, así
                // que el contorno es el estándar y el color va en el texto.
                _ => {
                    button.setBordered(true);
                    button.setBezelColor(None);
                    button.setContentTintColor(color.as_deref());
                }
            }

            match self.button_icons.get(&id) {
                Some((name, position)) if !name.is_empty() => {
                    let size = self.fonts.get(&id).map(|f| f.size).unwrap_or(0.0);
                    button.setImage(crate::icons::symbol(name, size, 400).as_deref());
                    button.setImagePosition(if position == "trailing" {
                        objc2_app_kit::NSCellImagePosition::ImageTrailing
                    } else {
                        objc2_app_kit::NSCellImagePosition::ImageLeading
                    });
                }
                _ => button.setImage(None),
            }
        }
    }

    fn set_corner(&mut self, id: NodeId, corner: usize, radius: Option<f32>) {
        let entry = self.corners.entry(id).or_insert([0.0; 4]);
        entry[corner] = radius.unwrap_or(0.0) as f64;
        self.apply_corners(id);
    }

    /// Redondea las esquinas con la capa.
    ///
    /// Una capa tiene **un** radio y una máscara de qué esquinas lo llevan, así
    /// que cuatro radios distintos no se pueden pedir. Cuando difieren se
    /// redondean con el mayor las que tengan algo y se dice: dibujar la forma a
    /// mano como hace el host de iOS costaría una `CAShapeLayer` por vista y
    /// aquí todavía no hace falta.
    fn apply_corners(&mut self, id: NodeId) {
        let Some(radii) = self.corners.get(&id).copied() else { return };
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        native.setWantsLayer(true);
        let Some(layer) = (unsafe { native.layer() }) else { return };

        let max = radii.iter().cloned().fold(0.0_f64, f64::max);
        let desiguales = radii.iter().any(|r| (*r - radii[0]).abs() > f64::EPSILON);
        layer.setCornerRadius(max);
        if desiguales {
            use objc2_quartz_core::CACornerMask;
            let mut mask = CACornerMask::empty();
            // El orden del núcleo es arriba-izq, arriba-der, abajo-der,
            // abajo-izq, y el de la capa está en coordenadas *sin* voltear: lo
            // que la capa llama «MinY» es el arriba de una vista volteada.
            if radii[0] > 0.0 {
                mask |= CACornerMask::LayerMinXMinYCorner;
            }
            if radii[1] > 0.0 {
                mask |= CACornerMask::LayerMaxXMinYCorner;
            }
            if radii[2] > 0.0 {
                mask |= CACornerMask::LayerMaxXMaxYCorner;
            }
            if radii[3] > 0.0 {
                mask |= CACornerMask::LayerMinXMaxYCorner;
            }
            layer.setMaskedCorners(mask);
            self.warn_once(format!("corners:{id}"), || {
                eprintln!(
                    "angular-native: una capa de AppKit solo tiene un radio; las esquinas del \
                     nodo {id} se redondean todas con el mayor ({max})"
                );
            });
        }
        layer.setMasksToBounds(max > 0.0);
    }

    /// El marco del core más el desplazamiento pedido, que aquí se suma en vez
    /// de pasar por la matriz: en AppKit es exacto y evita tener que compensar
    /// el punto de anclaje de la capa.
    fn place(&self, id: NodeId) {
        let (Some(view), Some(frame)) = (self.views.get(&id), self.frames.get(&id)) else {
            return;
        };
        let transform = self.transforms.get(&id).copied().unwrap_or_default();
        let native = view.as_view().retain();
        let rect = CGRect {
            origin: CGPoint {
                x: frame.x as f64 + transform.translate_x,
                y: frame.y as f64 + transform.translate_y,
            },
            size: CGSize { width: frame.width as f64, height: frame.height as f64 },
        };
        self.animated(id, || native.setFrame(rect));

        if transform.is_identity() {
            return;
        }
        // Escala y giro sí van por la capa. El anclaje se pone en el centro
        // para que gire sobre sí misma y no sobre su esquina; `setFrame` de la
        // capa deriva la posición del anclaje, así que ponerlo aquí no
        // descoloca nada.
        native.setWantsLayer(true);
        if let Some(layer) = unsafe { native.layer() } {
            layer.setAnchorPoint(CGPoint { x: 0.5, y: 0.5 });
            layer.setAffineTransform(transform.matrix());
        }
    }
}

impl HostRenderer for AppKitHost {
    fn create(&mut self, id: NodeId, kind: NodeKind) {
        let mtm = self.mtm;
        // El `match` no lleva comodín a propósito: si el núcleo añade un
        // `NodeKind`, este fichero deja de compilar y alguien tiene que decidir
        // qué hace macOS con él. Ver la cabecera de `support.rs`.
        let view = match kind {
            NodeKind::View => HostView::View(FlippedView::new(mtm)),
            NodeKind::StackView => {
                let stack = FlippedView::new(mtm);
                // Las pantallas que entran y salen se salen del marco.
                stack.clip_to_bounds();
                HostView::Stack(stack)
            }
            NodeKind::Text => {
                let label = NSTextField::new(mtm);
                unsafe {
                    // AppKit no tiene `NSLabel`: un rótulo es un campo de texto
                    // sin marco, sin fondo y sin editar. Es lo que hace
                    // `NSTextField.labelWithString:`, escrito a mano porque el
                    // texto llega después.
                    label.setEditable(false);
                    label.setSelectable(false);
                    label.setBordered(false);
                    label.setBezeled(false);
                    label.setDrawsBackground(false);
                    label.setUsesSingleLineMode(false);
                    label.setMaximumNumberOfLines(0);
                    label.cell().inspect(|cell| cell.setWraps(true));
                    // La fuente por defecto tiene que ser la misma con la que
                    // el layout midió, o la caja sale estrecha y el texto se
                    // corta sin que nada dé error.
                    let default_size = an_layout::FontSpec::default().size as f64;
                    label.setFont(Some(&NSFont::systemFontOfSize(default_size)));
                }
                HostView::Label(label)
            }
            NodeKind::TextInput => {
                let field = NSTextField::new(mtm);
                unsafe { field.setUsesSingleLineMode(true) };
                HostView::Field(field)
            }
            NodeKind::TextEditor => {
                let area = NSTextView::new(mtm);
                unsafe {
                    area.setDrawsBackground(false);
                    area.setTextContainerInset(CGSize { width: 0.0, height: 0.0 });
                }
                HostView::Area(area)
            }
            NodeKind::Image => HostView::Image(NSImageView::new(mtm)),
            NodeKind::Icon => HostView::Icon(NSImageView::new(mtm)),
            NodeKind::ScrollView => {
                let scroll = NSScrollView::new(mtm);
                // El documento es el que lleva a los hijos, y va volteado por
                // lo mismo que todo lo demás: el core coloca de arriba abajo.
                let document = FlippedView::new(mtm);
                unsafe {
                    scroll.setDocumentView(Some(&document));
                    scroll.setDrawsBackground(false);
                    scroll.setHasVerticalScroller(true);
                    // Los indicadores que se esconden solos son el
                    // comportamiento normal de macOS desde Lion; el estilo
                    // heredado ocupa sitio y descuadraría el layout que taffy
                    // ya calculó.
                    scroll.setScrollerStyle(NSScrollerStyle::Overlay);
                }
                HostView::Scroll(scroll)
            }
            NodeKind::Button => {
                let button = NSButton::new(mtm);
                unsafe { button.setBezelStyle(NSBezelStyle::Push) };
                HostView::Button(button)
            }
            NodeKind::Switch => HostView::Toggle(NSSwitch::new(mtm)),
            NodeKind::Slider => HostView::Slide(NSSlider::new(mtm)),
            NodeKind::ActivityIndicator => {
                let spinner = NSProgressIndicator::new(mtm);
                spinner.setStyle(NSProgressIndicatorStyle::Spinning);
                spinner.setDisplayedWhenStopped(false);
                HostView::Spinner(spinner)
            }
            NodeKind::ProgressBar => {
                let bar = NSProgressIndicator::new(mtm);
                bar.setStyle(NSProgressIndicatorStyle::Bar);
                bar.setIndeterminate(false);
                bar.setMinValue(0.0);
                bar.setMaxValue(1.0);
                HostView::Progress(bar)
            }
            NodeKind::SegmentedControl => {
                let segments = NSSegmentedControl::new(mtm);
                unsafe {
                    segments.setSegmentStyle(objc2_app_kit::NSSegmentStyle::Automatic);
                    segments.setTrackingMode(
                        objc2_app_kit::NSSegmentSwitchTracking::SelectOne,
                    );
                }
                HostView::Segments(segments)
            }
            NodeKind::TabBar => {
                let tabs = NSSegmentedControl::new(mtm);
                unsafe {
                    tabs.setSegmentStyle(objc2_app_kit::NSSegmentStyle::Automatic);
                    tabs.setTrackingMode(objc2_app_kit::NSSegmentSwitchTracking::SelectOne);
                }
                HostView::Tabs(tabs)
            }
            NodeKind::Stepper => HostView::Step(NSStepper::new(mtm)),
            NodeKind::SearchBar => HostView::Search(NSSearchField::new(mtm)),
            NodeKind::Picker => {
                // El desplegable de macOS sí existe como control: es un
                // `NSPopUpButton`, y no hace falta armarlo con un botón y un
                // menú como en iOS.
                let menu = NSPopUpButton::new(mtm);
                unsafe { menu.setPullsDown(false) };
                HostView::Menu(menu)
            }
            NodeKind::DatePicker => {
                let picker = NSDatePicker::new(mtm);
                unsafe {
                    picker.setDatePickerElements(
                        objc2_app_kit::NSDatePickerElementFlags::YearMonthDay,
                    );
                    picker.setDatePickerStyle(
                        objc2_app_kit::NSDatePickerStyle::TextFieldAndStepper,
                    );
                }
                HostView::Date(picker)
            }
            NodeKind::WebView => HostView::Web(crate::web::WKWebView::new(mtm)),
            NodeKind::Alert => {
                let placeholder = FlippedView::new(mtm);
                placeholder.setHidden(true);
                self.alerts.insert(id, crate::alert::AlertState::default());
                HostView::Dialog(placeholder)
            }
            NodeKind::Modal => {
                let overlay = FlippedView::new(mtm);
                overlay.setHidden(true);
                HostView::Overlay(overlay)
            }
            // Lo que macOS no trae. No se imita: se monta una vista que no
            // dibuja nada y se dice por qué, una vez.
            NodeKind::NavigationBar | NodeKind::MapView | NodeKind::VideoView => {
                let reason = match support(kind) {
                    Some(Support::Missing(reason)) => reason,
                    _ => "sin portar",
                };
                self.warn_once(format!("kind:{kind:?}"), || {
                    eprintln!(
                        "angular-native: <{kind:?}> no se pinta en macOS: {reason}. \
                         El nodo ocupa su sitio en el layout pero no dibuja nada."
                    );
                });
                HostView::Unsupported(FlippedView::new(mtm), kind)
            }
            // Nunca llega: el core no manda `Create` de un nodo de texto crudo.
            NodeKind::RawText => return,
        };
        self.views.insert(id, view);
    }

    fn destroy(&mut self, id: NodeId) {
        if let Some(view) = self.views.remove(&id) {
            view.as_view().removeFromSuperview();
        }
        self.fonts.remove(&id);
        self.corners.remove(&id);
        self.frames.remove(&id);
        self.transforms.remove(&id);
        self.animations.remove(&id);
        self.icons.remove(&id);
        self.segments.remove(&id);
        self.decorations.remove(&id);
        self.placeholders.remove(&id);
        self.placeholder_colors.remove(&id);
        self.button_titles.remove(&id);
        self.button_colors.remove(&id);
        self.button_variants.remove(&id);
        self.button_icons.remove(&id);
        self.slider_values.remove(&id);
        self.stepper_values.remove(&id);
        self.alerts.remove(&id);
        self.listeners.retain(|(node, _), _| *node != id);
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        let (Some(parent_view), Some(child_view)) =
            (self.views.get(&parent), self.views.get(&child))
        else {
            return;
        };
        let host = parent_view.content_view();
        let child_native = child_view.as_view();

        // AppKit no tiene `insertSubview:atIndex:`: se coloca por encima o por
        // debajo de un hermano. Es lo mismo dicho de otra forma, porque el
        // orden de `subviews` es el orden de dibujado.
        let siblings = host.subviews().to_vec();
        let index = index as usize;
        unsafe {
            if index == 0 {
                match siblings.first() {
                    Some(first) => host.addSubview_positioned_relativeTo(
                        child_native,
                        objc2_app_kit::NSWindowOrderingMode::Below,
                        Some(first),
                    ),
                    None => host.addSubview(child_native),
                }
            } else {
                match siblings.get(index - 1) {
                    Some(previous) => host.addSubview_positioned_relativeTo(
                        child_native,
                        objc2_app_kit::NSWindowOrderingMode::Above,
                        Some(previous),
                    ),
                    None => host.addSubview(child_native),
                }
            }
        }
    }

    fn remove(&mut self, _parent: NodeId, child: NodeId) {
        let Some(view) = self.views.get(&child) else { return };
        view.as_view().removeFromSuperview();
    }

    fn set_prop(&mut self, id: NodeId, key: &str, value: &PropValue) {
        let Some(view) = self.views.get(&id) else { return };
        let kind = view.kind();
        let native = view.as_view().retain();
        let text = value.as_str().map(str::to_owned);
        let number = value.as_f32();

        match key {
            // Props de otra plataforma. Viajan con su prefijo, así que este
            // host las descarta de un vistazo sin tener que saber qué son.
            _ if key.starts_with("ios:") || key.starts_with("android:") => {}

            "backgroundColor" | "background-color" => {
                let color = text.as_deref().and_then(crate::color::to_nscolor);
                match self.views.get(&id) {
                    // Un campo y un rótulo pintan su fondo con su propia
                    // propiedad; la capa se los dibujaría por debajo.
                    Some(HostView::Field(field)) | Some(HostView::Label(field)) => unsafe {
                        field.setDrawsBackground(color.is_some());
                        field.setBackgroundColor(color.as_deref());
                    },
                    _ => {
                        native.setWantsLayer(true);
                        if let Some(layer) = unsafe { native.layer() } {
                            layer.setBackgroundColor(
                                color.as_ref().map(|c| c.CGColor()).as_deref(),
                            );
                        }
                    }
                }
            }
            "opacity" => {
                if let Some(v) = number {
                    let view = native.clone();
                    self.animated(id, move || unsafe { view.setAlphaValue(v as f64) });
                }
            }
            "enabled" => {
                let on = !matches!(value, PropValue::Bool(false));
                let control: *const NSView = &*native;
                match kind {
                    NodeKind::Button
                    | NodeKind::Switch
                    | NodeKind::Slider
                    | NodeKind::SegmentedControl
                    | NodeKind::TabBar
                    | NodeKind::Stepper
                    | NodeKind::Picker
                    | NodeKind::DatePicker
                    | NodeKind::TextInput
                    | NodeKind::SearchBar => unsafe {
                        (*control.cast::<NSControl>()).setEnabled(on)
                    },
                    // Lo que no es un control no sabe ponerse gris, y AppKit
                    // no tiene el `userInteractionEnabled` de UIKit: una vista
                    // que no es control no se puede apagar sin imitarlo. Se
                    // dice en vez de fingir que se apagó.
                    _ => {
                        let _ = on;
                        self.warn_once(format!("enabled:{kind:?}"), || {
                            eprintln!(
                                "angular-native: `enabled` en <{kind:?}> no se aplica en macOS: \
                                 AppKit solo sabe apagar controles, no vistas cualesquiera"
                            );
                        });
                    }
                }
            }
            "testID" | "accessibilityIdentifier" => {
                if let Some(t) = &text {
                    unsafe { native.setIdentifier(Some(&NSString::from_str(t))) };
                }
            }

            // --- geometría propia de la vista
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
                if let Some(color) = text.as_deref().and_then(crate::color::to_cgcolor) {
                    native.setWantsLayer(true);
                    if let Some(layer) = unsafe { native.layer() } {
                        layer.setBorderColor(Some(&color));
                    }
                }
            }
            "borderWidth" | "border-width" => {
                if let Some(v) = number {
                    native.setWantsLayer(true);
                    if let Some(layer) = unsafe { native.layer() } {
                        layer.setBorderWidth(v as f64);
                    }
                }
            }

            // --- animación y transformaciones
            "animate" => {
                self.animations.entry(id).or_default().duration =
                    number.unwrap_or(0.0) as f64 / 1000.0;
            }
            "animateDelay" => {
                let delay = number.unwrap_or(0.0) as f64 / 1000.0;
                self.animations.entry(id).or_default().delay = delay;
                if delay > 0.0 {
                    self.warn_once("animateDelay".to_owned(), || {
                        eprintln!(
                            "angular-native: NSAnimationContext no tiene retraso; en macOS \
                             `animateDelay` se suma a la duración"
                        );
                    });
                }
            }
            "animateEasing" => {
                // `NSAnimationContext` toma una curva de temporización de Core
                // Animation, que no son las cuatro de UIKit. Mientras solo se
                // pidan esas cuatro, la del sistema es la que corresponde a
                // `ease-in-out` y las otras tres se dirían mal.
                if text.as_deref().is_some_and(|t| t != "ease-in-out") {
                    self.warn_once("animateEasing".to_owned(), || {
                        eprintln!(
                            "angular-native: en macOS la curva de animación la pone el sistema; \
                             `animateEasing` no se aplica"
                        );
                    });
                }
            }
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
                self.place(id);
            }

            // --- texto
            "color" => {
                let Some(color) = text.as_deref().and_then(crate::color::to_nscolor) else {
                    return;
                };
                match self.views.get(&id) {
                    Some(HostView::Label(label)) | Some(HostView::Field(label)) => unsafe {
                        label.setTextColor(Some(&color))
                    },
                    Some(HostView::Search(search)) => unsafe {
                        search.setTextColor(Some(&color))
                    },
                    Some(HostView::Area(area)) => unsafe { area.setTextColor(Some(&color)) },
                    // Un símbolo se tiñe, no se recolorea.
                    Some(HostView::Icon(icon)) => unsafe {
                        icon.setContentTintColor(Some(&color))
                    },
                    // `NSSwitch` y `NSProgressIndicator` no se tiñen: van del
                    // color de acento que el usuario haya elegido en Ajustes,
                    // y AppKit no expone ninguna propiedad para cambiarlo por
                    // vista. Teñirlos a mano —una capa encima, un filtro—
                    // sería dibujar un control en vez de usar el del sistema.
                    Some(HostView::Toggle(_))
                    | Some(HostView::Spinner(_))
                    | Some(HostView::Progress(_)) => {
                        self.warn_once(format!("tint:{kind:?}"), || {
                            eprintln!(
                                "angular-native: `color` en <{kind:?}> no se aplica en macOS: \
                                 estos controles van del color de acento del sistema y AppKit no \
                                 deja cambiarlo por vista"
                            );
                        });
                    }
                    Some(HostView::Slide(slider)) => unsafe {
                        slider.setTrackFillColor(Some(&color))
                    },
                    Some(HostView::Segments(segments)) | Some(HostView::Tabs(segments)) => unsafe {
                        segments.setSelectedSegmentBezelColor(Some(&color))
                    },
                    Some(HostView::Button(_)) => {
                        if let Some(raw) = text.as_deref() {
                            self.button_colors.insert(id, raw.to_owned());
                        }
                        self.refresh_button(id);
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
                match self.views.get(&id) {
                    Some(HostView::Label(label)) | Some(HostView::Field(label)) => unsafe {
                        label.setAlignment(alignment)
                    },
                    Some(HostView::Area(area)) => unsafe { area.setAlignment(alignment) },
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
            "numberOfLines" => {
                self.font_mut(id).max_lines = number.filter(|v| *v >= 1.0).map(|v| v as u32);
                self.apply_font(id);
            }
            "lineHeight" | "line-height" => {
                self.font_mut(id).line_height = number;
                self.apply_text_attributes(id);
            }
            "letterSpacing" | "letter-spacing" => {
                self.font_mut(id).letter_spacing = number.unwrap_or(0.0);
                self.apply_text_attributes(id);
            }
            "textDecoration" | "text-decoration" => {
                self.decorations.insert(id, text.clone().unwrap_or_else(|| "none".to_owned()));
                self.apply_text_attributes(id);
            }
            "placeholder" => {
                self.placeholders.insert(id, text.clone().unwrap_or_default());
                self.apply_placeholder(id);
            }
            "placeholderColor" => {
                match &text {
                    Some(color) => self.placeholder_colors.insert(id, color.clone()),
                    None => self.placeholder_colors.remove(&id),
                };
                self.apply_placeholder(id);
            }
            "editable" => {
                let on = !matches!(value, PropValue::Bool(false));
                match self.views.get(&id) {
                    Some(HostView::Field(field)) => unsafe { field.setEditable(on) },
                    Some(HostView::Area(area)) => unsafe { area.setEditable(on) },
                    _ => {}
                }
            }

            // --- valores de los controles
            "value" => match self.views.get(&id) {
                Some(HostView::Slide(slider)) => {
                    let Some(v) = number else { return };
                    self.slider_values.insert(id, v as f64);
                    // Solo si difiere: escribirlo mientras se arrastra pelearía
                    // con el ratón del usuario.
                    if (unsafe { slider.doubleValue() } - v as f64).abs() > f64::EPSILON {
                        unsafe { slider.setDoubleValue(v as f64) };
                    }
                }
                Some(HostView::Step(stepper)) => {
                    let v = number.unwrap_or(0.0) as f64;
                    self.stepper_values.insert(id, v);
                    unsafe { stepper.setDoubleValue(v) };
                }
                Some(HostView::Date(picker)) => {
                    // Llega en milisegundos desde 1970, que es lo que da `Date`
                    // en JS. `NSDate` trabaja en segundos.
                    let seconds = number.unwrap_or(0.0) as f64 / 1000.0;
                    let date =
                        unsafe { objc2_foundation::NSDate::dateWithTimeIntervalSince1970(seconds) };
                    unsafe { picker.setDateValue(&date) };
                }
                // Escribirlo mientras el usuario escribe le movería el cursor
                // al final en cada tecla: solo si difiere de verdad.
                Some(HostView::Field(field)) => {
                    let next = text.clone().unwrap_or_default();
                    if unsafe { field.stringValue() }.to_string() != next {
                        unsafe { field.setStringValue(&NSString::from_str(&next)) };
                    }
                }
                Some(HostView::Search(search)) => {
                    let next = text.clone().unwrap_or_default();
                    if unsafe { search.stringValue() }.to_string() != next {
                        unsafe { search.setStringValue(&NSString::from_str(&next)) };
                    }
                }
                Some(HostView::Area(area)) => {
                    let current = unsafe { area.string() }.to_string();
                    let next = text.clone().unwrap_or_default();
                    if current != next {
                        unsafe { area.setString(&NSString::from_str(&next)) };
                    }
                }
                _ => {}
            },
            "on" => {
                if let Some(HostView::Toggle(toggle)) = self.views.get(&id) {
                    unsafe {
                        toggle.setState(if matches!(value, PropValue::Bool(true)) {
                            NSControlStateValueOn
                        } else {
                            NSControlStateValueOff
                        })
                    };
                }
            }
            "minimumValue" | "maximumValue" | "stepValue" => {
                let v = number.unwrap_or(0.0) as f64;
                match self.views.get(&id) {
                    Some(HostView::Slide(slider)) => {
                        unsafe {
                            match key {
                                "minimumValue" => slider.setMinValue(v),
                                "maximumValue" => slider.setMaxValue(v),
                                // Un deslizador de macOS no tiene paso libre:
                                // se pide por número de marcas, y con marcas
                                // salen las rayitas. Se deja continuo.
                                _ => return,
                            }
                        }
                        // El rango cambió: hay que volver a aplicar el valor,
                        // que pudo llegar antes y quedarse recortado.
                        if let Some(wanted) = self.slider_values.get(&id).copied() {
                            unsafe { slider.setDoubleValue(wanted) };
                        }
                    }
                    Some(HostView::Step(stepper)) => {
                        unsafe {
                            match key {
                                "minimumValue" => stepper.setMinValue(v),
                                "maximumValue" => stepper.setMaxValue(v),
                                _ => stepper.setIncrement(if v > 0.0 { v } else { 1.0 }),
                            }
                        }
                        if let Some(wanted) = self.stepper_values.get(&id).copied() {
                            unsafe { stepper.setDoubleValue(wanted) };
                        }
                    }
                    _ => {}
                }
            }
            "minimumTrackColor" => {
                if let (Some(HostView::Slide(slider)), Some(color)) =
                    (self.views.get(&id), text.as_deref().and_then(crate::color::to_nscolor))
                {
                    unsafe { slider.setTrackFillColor(Some(&color)) };
                }
            }
            "animating" => {
                if let Some(HostView::Spinner(spinner)) = self.views.get(&id) {
                    if matches!(value, PropValue::Bool(false)) {
                        unsafe { spinner.stopAnimation(None) };
                    } else {
                        unsafe { spinner.startAnimation(None) };
                    }
                }
            }
            "progress" => {
                if let (Some(HostView::Progress(bar)), Some(v)) = (self.views.get(&id), number) {
                    bar.setDoubleValue(v.clamp(0.0, 1.0) as f64);
                }
            }

            // --- botón
            "title" if kind == NodeKind::Button => {
                self.button_titles.insert(id, text.clone().unwrap_or_default());
                self.refresh_button(id);
            }
            "variant" if kind == NodeKind::Button => {
                self.button_variants.insert(id, text.clone().unwrap_or_else(|| "text".to_owned()));
                self.refresh_button(id);
            }
            "icon" | "iconPosition" if kind == NodeKind::Button => {
                let entry = self.button_icons.entry(id).or_default();
                if key == "icon" {
                    entry.0 = text.clone().unwrap_or_default();
                } else {
                    entry.1 = text.clone().unwrap_or_else(|| "leading".to_owned());
                }
                self.refresh_button(id);
            }

            // --- iconos
            "name" | "iconSize" | "iconWeight" if kind == NodeKind::Icon => {
                let entry = self.icons.entry(id).or_default();
                match key {
                    "name" => entry.0 = text.clone().unwrap_or_default(),
                    "iconSize" => entry.1 = number.unwrap_or(24.0),
                    _ => entry.2 = number.unwrap_or(400.0) as u16,
                }
                let (name, size, weight) = entry.clone();
                if let Some(HostView::Icon(icon)) = self.views.get(&id) {
                    // Plantilla: así `[color]` tiñe el símbolo en vez de que
                    // salga con el color que traiga de fábrica.
                    let symbol = crate::icons::symbol(&name, size, weight);
                    if let Some(image) = &symbol {
                        unsafe { image.setTemplate(true) };
                    }
                    unsafe { icon.setImage(symbol.as_deref()) };
                }
            }

            // --- imagen
            "source" => {
                if let Some(HostView::Image(image)) = self.views.get(&id) {
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
                if let Some(HostView::Image(image)) = self.views.get(&id) {
                    unsafe {
                        image.setImageScaling(crate::images::image_scaling(
                            text.as_deref().unwrap_or("contain"),
                        ))
                    };
                }
            }

            // --- listas: pestañas, segmentos y desplegable
            "items" | "icons" => {
                let entry = self.segments.entry(id).or_default();
                let list = parse_string_list(text.as_deref().unwrap_or("[]"));
                if key == "items" {
                    entry.0 = list;
                } else {
                    entry.1 = list;
                }
                let (titles, icons) = entry.clone();
                match self.views.get(&id) {
                    Some(HostView::Segments(control)) | Some(HostView::Tabs(control)) => unsafe {
                        control.setSegmentCount(titles.len() as isize);
                        for (index, title) in titles.iter().enumerate() {
                            let index = index as isize;
                            control.setLabel_forSegment(&NSString::from_str(title), index);
                            let symbol = icons
                                .get(index as usize)
                                .and_then(|name| crate::icons::symbol(name, 0.0, 400));
                            if let Some(image) = &symbol {
                                image.setTemplate(true);
                            }
                            control.setImage_forSegment(symbol.as_deref(), index);
                        }
                    },
                    Some(HostView::Menu(menu)) => unsafe {
                        menu.removeAllItems();
                        for title in &titles {
                            menu.addItemWithTitle(&NSString::from_str(title));
                        }
                    },
                    _ => {}
                }
            }
            "selectedIndex" => {
                let index = number.unwrap_or(0.0).max(0.0) as isize;
                match self.views.get(&id) {
                    Some(HostView::Segments(control)) | Some(HostView::Tabs(control)) => unsafe {
                        if index < control.segmentCount() {
                            control.setSelectedSegment(index);
                        }
                    },
                    Some(HostView::Menu(menu)) => unsafe { menu.selectItemAtIndex(index) },
                    _ => {}
                }
            }
            "mode" if kind == NodeKind::DatePicker => {
                if let Some(HostView::Date(picker)) = self.views.get(&id) {
                    use objc2_app_kit::NSDatePickerElementFlags as Flags;
                    unsafe {
                        picker.setDatePickerElements(match text.as_deref() {
                            Some("time") => Flags::HourMinute,
                            Some("dateAndTime") => Flags::YearMonthDay | Flags::HourMinute,
                            _ => Flags::YearMonthDay,
                        });
                        // El modo de rango no se usa aquí, pero hay que fijarlo
                        // o el selector recuerda el de antes.
                        picker.setDatePickerMode(NSDatePickerMode::Single);
                    }
                }
            }

            // --- scroll
            "showsScrollIndicator" => {
                if let Some(HostView::Scroll(scroll)) = self.views.get(&id) {
                    let shown = !matches!(value, PropValue::Bool(false));
                    unsafe {
                        scroll.setHasVerticalScroller(shown);
                        scroll.setHasHorizontalScroller(shown);
                    }
                }
            }
            "scrollEnabled" => {
                if let Some(HostView::Scroll(scroll)) = self.views.get(&id) {
                    // AppKit no tiene un interruptor de scroll: quitarle los
                    // desplazadores y dejar el documento del tamaño del marco
                    // es lo mismo visto desde fuera.
                    let on = !matches!(value, PropValue::Bool(false));
                    unsafe { scroll.setScrollerStyle(if on {
                        NSScrollerStyle::Overlay
                    } else {
                        NSScrollerStyle::Legacy
                    }) };
                    unsafe { scroll.setHasVerticalScroller(on) };
                }
            }

            // --- navegador embebido
            "url" if kind == NodeKind::WebView => {
                let Some(HostView::Web(web)) = self.views.get(&id) else { return };
                let Some(raw) = text.as_deref() else { return };
                let Some(url) =
                    (unsafe { objc2_foundation::NSURL::URLWithString(&NSString::from_str(raw)) })
                else {
                    return;
                };
                let request = unsafe { objc2_foundation::NSURLRequest::requestWithURL(&url) };
                let _ = web.loadRequest(&request);
            }
            "html" => {
                if let Some(HostView::Web(web)) = self.views.get(&id) {
                    let _ = web.loadHTMLString_baseURL(
                        &NSString::from_str(text.as_deref().unwrap_or("")),
                        None,
                    );
                }
            }

            // --- diálogos del sistema
            "title" | "message" | "buttons" | "sheet" if self.alerts.contains_key(&id) => {
                let Some(state) = self.alerts.get_mut(&id) else { return };
                match key {
                    "title" => state.title = text.clone().unwrap_or_default(),
                    "message" => state.message = text.clone().unwrap_or_default(),
                    "buttons" => {
                        state.buttons = parse_string_list(text.as_deref().unwrap_or("[]"))
                    }
                    _ => state.sheet = matches!(value, PropValue::Bool(true)),
                }
                if !self.dirty_alerts.contains(&id) {
                    self.dirty_alerts.push(id);
                }
            }
            "visible" if self.alerts.contains_key(&id) => {
                if let Some(state) = self.alerts.get_mut(&id) {
                    state.visible = matches!(value, PropValue::Bool(true));
                }
                if !self.dirty_alerts.contains(&id) {
                    self.dirty_alerts.push(id);
                }
            }
            "visible" => {
                native.setHidden(matches!(value, PropValue::Bool(false)));
            }

            // Lo que macOS no puede honrar, dicho a propósito.
            _ if ignored_reason(key).is_some() => {
                let reason = ignored_reason(key).unwrap_or_default();
                self.warn_once(format!("ignored:{kind:?}:{key}"), || {
                    eprintln!(
                        "angular-native: `{key}` en <{kind:?}> no se aplica en macOS: {reason}"
                    );
                });
            }
            // Y lo que no está ni implementado ni declarado. Aquí no hay un
            // `_ => {}`: una prop que nadie mira sale por pantalla la primera
            // vez, que es la diferencia entre un hueco conocido y uno que
            // nadie ve.
            other => {
                self.warn_once(format!("unknown:{kind:?}:{other}"), || {
                    eprintln!(
                        "angular-native: prop desconocida `{other}` en <{kind:?}>; el host de \
                         macOS no la mira y no está declarada en IGNORED"
                    );
                });
            }
        }
    }

    fn set_text(&mut self, id: NodeId, text: &str) {
        let Some(HostView::Label(label)) = self.views.get(&id) else { return };
        unsafe { label.setStringValue(&NSString::from_str(text)) };
        // `setStringValue` tira el texto atribuido, así que el espaciado, el
        // interlineado y el subrayado hay que volver a ponerlos con cada
        // palabra nueva.
        self.apply_text_attributes(id);
    }

    fn set_listener(&mut self, id: NodeId, event: &str, enabled: bool) {
        let key = (id, event.to_owned());
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view().retain();

        // El área segura es el recorte de la pantalla: la muesca, la barra de
        // inicio. Una ventana de escritorio no tiene nada de eso, así que la
        // respuesta correcta es cero por los cuatro lados, y se contesta una
        // vez al suscribirse en vez de dejar a la plantilla esperando.
        if event == "safeArea" {
            if enabled {
                an_host::push_event(
                    &self.events,
                    an_host::HostEvent {
                        target: id,
                        name: "safeArea".to_owned(),
                        payload: ["top", "right", "bottom", "left"]
                            .into_iter()
                            .map(|edge| (edge.to_owned(), PropValue::Number(0.0)))
                            .collect(),
                    },
                );
            }
            return;
        }

        if !enabled {
            if let Some(listener) = self.listeners.remove(&key) {
                listener.detach(&native);
            }
            return;
        }
        if self.listeners.contains_key(&key) {
            return;
        }

        let kind = view.kind();

        // El diálogo del sistema no entrega su elección por la vista: no tiene
        // vista. La entrega `NSAlert` desde su bloque de cierre, así que
        // engancharla aquí sería engancharla dos veces.
        if kind == NodeKind::Alert && event == "select" {
            return;
        }

        // Lo que esta plataforma no sabe dar se dice al suscribirse, no cuando
        // el evento no llega.
        if let Some(reason) = unsupported_event(kind, event) {
            self.warn_once(format!("event:{kind:?}:{event}"), || {
                eprintln!(
                    "angular-native: `({event})` en <{kind:?}> no se puede entregar en macOS: \
                     {reason}"
                );
            });
            return;
        }

        if let Some(listener) =
            crate::events::attach(self.mtm, &native, kind, id, event, self.events.clone())
        {
            self.listeners.insert(key, listener);
        } else if is_known_event(event) {
            // Un nombre que el framework sí manda y que este host no cubre. Los
            // que no manda —los nombres de salida que Angular registra de
            // paso— se descartan sin ruido: ver `support::KNOWN_EVENTS`.
            self.warn_once(format!("event:{kind:?}:{event}"), || {
                eprintln!(
                    "angular-native: el host de macOS no sabe entregar `({event})` en <{kind:?}>"
                );
            });
        }
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        self.frames.insert(id, frame);
        self.place(id);
        // Una máscara de esquinas no se estira con la vista.
        if self.corners.contains_key(&id) {
            self.apply_corners(id);
        }
    }

    fn set_content_size(&mut self, id: NodeId, width: f32, height: f32) {
        let Some(HostView::Scroll(scroll)) = self.views.get(&id) else { return };
        let Some(document) = (unsafe { scroll.documentView() }) else { return };
        // En AppKit el tamaño del contenido *es* el marco del documento: no hay
        // un `contentSize` aparte como en `UIScrollView`.
        document.setFrame(CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize { width: width as f64, height: height as f64 },
        });
    }

    fn set_root(&mut self, id: NodeId) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        if unsafe { native.superview() }.is_none() {
            self.container.addSubview(native);
        }
    }

    fn flush(&mut self) {
        // Presentar va después del layout: un diálogo se presenta cuando todas
        // sus props ya llegaron, o saldría con el título a medias.
        for id in std::mem::take(&mut self.dirty_alerts) {
            let Some(mut state) = self.alerts.remove(&id) else { continue };
            state.sync(self.mtm, &self.container, id, &self.events);
            self.alerts.insert(id, state);
        }
    }

    fn clear(&mut self) {
        for view in self.views.values() {
            view.as_view().removeFromSuperview();
        }
        self.views.clear();
        self.fonts.clear();
        self.corners.clear();
        self.frames.clear();
        self.transforms.clear();
        self.listeners.clear();
        self.alerts.clear();
        self.dirty_alerts.clear();
    }
}
