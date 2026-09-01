//! `HostRenderer` sobre UIKit. Una vista nativa por nodo montable, colocada
//! con `frame` directo: el layout ya lo resolvió taffy, Auto Layout aquí solo
//! añadiría un segundo motor de layout compitiendo con el primero.

use std::collections::HashMap;

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::{EventQueue, HostRenderer};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use objc2_quartz_core::CAShapeLayer;
use objc2_ui_kit::{
    NSLineBreakMode, NSTextAlignment, UIAccessibilityIdentification, UIBezierPath, UIFont,
    UIImageView, UILabel, UIScrollView, UITextField, UITextInputTraits, UIView,
};

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
    Label(Retained<UILabel>),
    Image(Retained<UIImageView>),
    Scroll(Retained<UIScrollView>),
    Field(Retained<UITextField>),
}

impl HostView {
    fn as_view(&self) -> &UIView {
        match self {
            HostView::View(v) => v,
            HostView::Label(v) => v,
            HostView::Image(v) => v,
            HostView::Scroll(v) => v,
            HostView::Field(v) => v,
        }
    }

    fn kind(&self) -> NodeKind {
        match self {
            HostView::View(_) => NodeKind::View,
            HostView::Label(_) => NodeKind::Text,
            HostView::Image(_) => NodeKind::Image,
            HostView::Scroll(_) => NodeKind::ScrollView,
            HostView::Field(_) => NodeKind::TextInput,
        }
    }

    fn as_label(&self) -> Option<&UILabel> {
        match self {
            HostView::Label(v) => Some(v),
            _ => None,
        }
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

    fn font_mut(&mut self, id: NodeId) -> &mut an_layout::FontSpec {
        self.fonts.entry(id).or_default()
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
                HostView::Label(label)
            }
            NodeKind::Image => HostView::Image(UIImageView::new(mtm)),
            NodeKind::ScrollView => HostView::Scroll(UIScrollView::new(mtm)),
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
            view.as_view().removeFromSuperview();
        }
        self.fonts.remove(&id);
        self.corners.remove(&id);
        self.listeners.retain(|(node, _), _| *node != id);
    }

    fn insert(&mut self, parent: NodeId, child: NodeId, index: u32) {
        let (Some(parent_view), Some(child_view)) = (self.views.get(&parent), self.views.get(&child))
        else {
            return;
        };
        parent_view
            .as_view()
            .insertSubview_atIndex(child_view.as_view(), index as isize);
    }

    fn remove(&mut self, _parent: NodeId, child: NodeId) {
        if let Some(view) = self.views.get(&child) {
            view.as_view().removeFromSuperview();
        }
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
                    native.setAlpha(v as f64);
                }
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
            "testID" | "accessibilityIdentifier" => {
                if let Some(t) = &text {
                    native.setAccessibilityIdentifier(Some(&NSString::from_str(t)));
                }
            }
            "color" => {
                if let (Some(label), Some(color)) =
                    (view.as_label(), text.as_deref().and_then(crate::color::to_uicolor))
                {
                    unsafe { label.setTextColor(Some(&color)) };
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
            // --- scroll
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
        view.as_view().setFrame(CGRect {
            origin: CGPoint { x: frame.x as f64, y: frame.y as f64 },
            size: CGSize { width: frame.width as f64, height: frame.height as f64 },
        });
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

    fn set_root(&mut self, id: NodeId) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        if native.superview().is_none() {
            self.container.addSubview(native);
        }
    }
}
