//! Controles del sistema: interruptor, deslizador, indicadores, botón y barra
//! de pestañas.
//!
//! Cuánto miden no lo decide el framework. Un `UISwitch` no mide lo mismo en
//! iOS 17 que en iOS 26, y con texto grande de accesibilidad tampoco. Se le
//! pregunta a cada control una vez, al arrancar y en el hilo principal, porque
//! crearlos fuera de él no está permitido.

use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_core_foundation::CGSize;
use objc2_ui_kit::{UIActivityIndicatorView, UIButton, UIProgressView, UITabBar, UIView};
// tvOS no las tiene, y un `use` de algo que no se va a usar es un aviso.
#[cfg(not(target_os = "tvos"))]
use objc2_ui_kit::{UISlider, UISwitch};

/// Tamaños naturales, en puntos, indexados por el nombre del control.
pub type ControlSizes = HashMap<String, (f32, f32)>;

/// Le pregunta a cada control cuánto ocupa. Se llama una vez, en el arranque.
pub fn measure_controls(mtm: MainThreadMarker) -> ControlSizes {
    // Ancho infinito para que cada uno diga su tamaño natural sin que se lo
    // recorte nada.
    let unbounded = CGSize { width: f64::MAX / 2.0, height: f64::MAX / 2.0 };
    let mut sizes = ControlSizes::new();

    let mut record = |name: &str, view: &UIView| {
        let fitted = view.sizeThatFits(unbounded);
        sizes.insert(name.to_owned(), (fitted.width as f32, fitted.height as f32));
    };

    // Los tres que tvOS no tiene se saltan enteros: preguntar por el tamaño
    // de una clase que no existe no es una medida que salga mal, es un
    // `objc_getClass` que devuelve nulo y un proceso que se cierra. Sin
    // entrada en la tabla, `measure_control` devuelve (0, 0) y el hueco se ve
    // en la pantalla, que es lo que corresponde a un control que no está.
    #[cfg(not(target_os = "tvos"))]
    {
        record("Switch", &UISwitch::new(mtm));
        record("Slider", &UISlider::new(mtm));
        record("Stepper", &objc2_ui_kit::UIStepper::new(mtm));
        record("DatePicker", &objc2_ui_kit::UIDatePicker::new(mtm));
    }
    record("ActivityIndicator", &UIActivityIndicatorView::new(mtm));
    record("ProgressBar", &UIProgressView::new(mtm));
    record("Button", &UIButton::new(mtm));
    record("TabBar", &UITabBar::new(mtm));
    record("SegmentedControl", &objc2_ui_kit::UISegmentedControl::new(mtm));
    record("SearchBar", &objc2_ui_kit::UISearchBar::new(mtm));
    // El desplegable es un botón con menú: mide lo que mide un botón.
    record("Picker", &UIButton::new(mtm));

    // `sizeThatFits` de algunos devuelve cero porque no tienen contenido
    // todavía; para esos manda el tamaño natural conocido.
    for (name, fallback) in [
        #[cfg(not(target_os = "tvos"))]
        ("Slider", (200.0, 32.0)),
        ("ProgressBar", (200.0, 4.0)),
        ("Button", (80.0, 44.0)),
        // Un control segmentado sin segmentos y una barra de búsqueda sin
        // texto no miden nada útil: hasta que tengan contenido manda su
        // tamaño conocido.
        ("SegmentedControl", (320.0, 32.0)),
        ("SearchBar", (320.0, 56.0)),
        #[cfg(not(target_os = "tvos"))]
        ("Stepper", (94.0, 32.0)),
        ("Picker", (140.0, 44.0)),
        #[cfg(not(target_os = "tvos"))]
        ("DatePicker", (200.0, 44.0)),
    ] {
        let entry = sizes.entry(name.to_owned()).or_insert(fallback);
        if entry.0 <= 0.0 {
            entry.0 = fallback.0;
        }
        if entry.1 <= 0.0 {
            entry.1 = fallback.1;
        }
    }
    sizes
}

/// Un `UITabBar` sin ítems no mide nada útil, y con ellos hay que construirlos.
pub fn tab_bar_items(
    mtm: MainThreadMarker,
    titles: &[String],
    icons: &[String],
) -> Retained<objc2_foundation::NSArray<objc2_ui_kit::UITabBarItem>> {
    use objc2_foundation::{NSArray, NSString};
    use objc2_ui_kit::UITabBarItem;

    let items: Vec<Retained<UITabBarItem>> = titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            // El icono no lleva tamaño: en una barra de pestañas lo elige
            // UIKit, y forzarlo aquí sería pelearse con la barra.
            let image = icons.get(index).and_then(|name| crate::icons::symbol(name, 0.0, 400));
            unsafe {
                UITabBarItem::initWithTitle_image_tag(
                    mtm.alloc::<UITabBarItem>(),
                    Some(&NSString::from_str(title)),
                    image.as_deref(),
                    index as isize,
                )
            }
        })
        .collect();
    NSArray::from_retained_slice(&items)
}
