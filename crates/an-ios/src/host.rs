//! `HostRenderer` sobre UIKit. Una vista nativa por nodo montable, colocada
//! con `frame` directo: el layout ya lo resolvió taffy, Auto Layout aquí solo
//! añadiría un segundo motor de layout compitiendo con el primero.

use std::collections::HashMap;

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{EventQueue, HostRenderer};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use block2::RcBlock;
use objc2_quartz_core::CAShapeLayer;
use objc2_ui_kit::{
    UIViewAnimationOptions,
    NSLineBreakMode, NSTextAlignment, UIAccessibilityIdentification, UIActivityIndicatorView,
    UIBezierPath, UIButton, UIControlState, UIFont, UIImageView, UILabel, UIProgressView,
    UIScrollView, UISlider, UISwitch, UITabBar, UITextField, UITextInputTraits, UIView,
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
    Tabs(Retained<UITabBar>),
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
            HostView::Tabs(v) => v,
            HostView::Toggle(v) => v,
            HostView::Slide(v) => v,
            HostView::Spinner(v) => v,
            HostView::Progress(v) => v,
            HostView::Button(v) => v,
            HostView::Overlay(v) => v,
            HostView::Dialog(v) => v,
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
            HostView::Tabs(_) => NodeKind::TabBar,
            HostView::Toggle(_) => NodeKind::Switch,
            HostView::Slide(_) => NodeKind::Slider,
            HostView::Spinner(_) => NodeKind::ActivityIndicator,
            HostView::Progress(_) => NodeKind::ProgressBar,
            HostView::Button(_) => NodeKind::Button,
            HostView::Overlay(_) => NodeKind::Modal,
            HostView::Dialog(_) => NodeKind::Alert,
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

    fn apply_font(&mut self, id: NodeId) {
        let Some(spec) = self.fonts.get(&id) else { return };
        let Some(label) = self.views.get(&id).and_then(HostView::as_label) else { return };
        let size = spec.size as f64;
        let font = if let Some(family) = &spec.family {
            let name = NSString::from_str(family);
            UIFont::fontWithName_size(&name, size)
                .unwrap_or_else(|| UIFont::systemFontOfSize(size))
        } else if spec.italic {
            UIFont::italicSystemFontOfSize(size)
        } else {
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
        };
        // Estos setters de UIKit están marcados unsafe por no ser thread-safe;
        // el `MainThreadMarker` del host garantiza que vamos por el hilo bueno.
        unsafe { label.setFont(Some(&font)) };
        label.setNumberOfLines(spec.max_lines.unwrap_or(0) as isize);
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
            NodeKind::ScrollView => HostView::Scroll(UIScrollView::new(mtm)),
            NodeKind::TabBar => HostView::Tabs(UITabBar::new(mtm)),
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
            _ => HostView::View(UIView::new(mtm)),
        };
        // Sin esto UIKit intenta resolver el layout por su cuenta y pisa
        // los `frame` que escribe el core.
        view.as_view().setTranslatesAutoresizingMaskIntoConstraints(false);
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
            "title" | "message" | "buttons" | "visible" if self.alerts.contains_key(&id) => {
                let Some(state) = self.alerts.get_mut(&id) else { return };
                match key {
                    "title" => state.title = text.clone().unwrap_or_default(),
                    "message" => state.message = text.clone().unwrap_or_default(),
                    "buttons" => {
                        state.buttons = parse_string_list(text.as_deref().unwrap_or("[]"))
                    }
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
                    HostView::Toggle(toggle) => toggle.setOnTintColor(Some(&color)),
                    HostView::Slide(slider) => slider.setMinimumTrackTintColor(Some(&color)),
                    HostView::Spinner(spinner) => unsafe { spinner.setColor(Some(&color)) },
                    HostView::Progress(bar) => bar.setProgressTintColor(Some(&color)),
                    HostView::Button(button) => unsafe {
                        button.setTitleColor_forState(Some(&color), UIControlState::Normal)
                    },
                    HostView::Tabs(bar) => unsafe { bar.setTintColor(Some(&color)) },
                    _ => {}
                }
            }
            "textAlign" | "text-align" => {
                if let (Some(label), Some(t)) = (view.as_label(), &text) {
                    label.setTextAlignment(match t.as_str() {
                        "center" => NSTextAlignment::Center,
                        "right" => NSTextAlignment::Right,
                        "justify" => NSTextAlignment::Justified,
                        _ => NSTextAlignment::Left,
                    });
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
                if let HostView::Field(field) = view {
                    let placeholder = text.as_deref().map(NSString::from_str);
                    field.setPlaceholder(placeholder.as_deref());
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
                if let HostView::Button(button) = view {
                    let title = text.as_deref().map(NSString::from_str);
                    unsafe { button.setTitle_forState(title.as_deref(), UIControlState::Normal) };
                }
            }
            "items" => {
                if let HostView::Tabs(bar) = view {
                    // Los títulos llegan como JSON: el protocolo no lleva
                    // listas, y una lista de pestañas no justifica añadirlas.
                    let titles = parse_string_list(text.as_deref().unwrap_or("[]"));
                    let items = crate::controls::tab_bar_items(self.mtm, &titles);
                    bar.setItems(Some(&items));
                }
            }
            "selectedIndex" => {
                if let (HostView::Tabs(bar), Some(index)) = (view, number) {
                    if let Some(items) = bar.items() {
                        let items = items.to_vec();
                        if let Some(item) = items.get(index.max(0.0) as usize) {
                            bar.setSelectedItem(Some(item));
                        }
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
