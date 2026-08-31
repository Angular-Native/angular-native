//! `HostRenderer` sobre UIKit. Una vista nativa por nodo montable, colocada
//! con `frame` directo: el layout ya lo resolvió taffy, Auto Layout aquí solo
//! añadiría un segundo motor de layout compitiendo con el primero.

use std::collections::HashMap;

use an_core::{NodeId, NodeKind, PropValue, Rect};
use an_host::HostRenderer;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use objc2_ui_kit::{
    NSLineBreakMode, NSTextAlignment, UIAccessibilityIdentification, UIFont, UIImageView,
    UILabel, UIScrollView, UITextField, UIView,
};

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
    /// Eventos suscritos. Todavía no se enganchan gestos: llega en la fase de
    /// eventos, cuando exista el puente de vuelta hacia JS.
    listeners: HashMap<NodeId, Vec<String>>,
}

impl UikitHost {
    /// # Safety
    /// `container` tiene que ser un `UIView` vivo y hay que llamar desde el
    /// hilo principal.
    pub fn new(mtm: MainThreadMarker, container: Retained<UIView>) -> Self {
        UikitHost {
            mtm,
            container,
            views: HashMap::new(),
            fonts: HashMap::new(),
            listeners: HashMap::new(),
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
        self.listeners.remove(&id);
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
                    let layer = native.layer();
                    layer.setCornerRadius(v as f64);
                    native.setClipsToBounds(v > 0.0);
                }
            }
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
        let entry = self.listeners.entry(id).or_default();
        match enabled {
            true if !entry.iter().any(|e| e == event) => entry.push(event.to_owned()),
            false => entry.retain(|e| e != event),
            _ => {}
        }
        // TODO(fase 3): enganchar UIGestureRecognizer y encolar `HostEvent`.
    }

    fn set_layout(&mut self, id: NodeId, frame: Rect) {
        let Some(view) = self.views.get(&id) else { return };
        view.as_view().setFrame(CGRect {
            origin: CGPoint { x: frame.x as f64, y: frame.y as f64 },
            size: CGSize { width: frame.width as f64, height: frame.height as f64 },
        });
    }

    fn set_root(&mut self, id: NodeId) {
        let Some(view) = self.views.get(&id) else { return };
        let native = view.as_view();
        if native.superview().is_none() {
            self.container.addSubview(native);
        }
    }
}
