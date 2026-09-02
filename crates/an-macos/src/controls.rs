//! Cuánto mide cada control del sistema.
//!
//! Igual que en iOS: no lo decide el framework. Un `NSSwitch` no mide lo mismo
//! en Sonoma que en Tahoe, y con el tamaño de texto grande tampoco. Se les
//! pregunta una vez, al arrancar y en el hilo principal, porque crear una
//! vista de AppKit fuera de él no está permitido.
//!
//! La diferencia con iOS es de qué se les pregunta. UIKit tiene
//! `sizeThatFits:`, que es una pregunta; AppKit tiene `fittingSize`, que es una
//! propiedad y que además obliga al control a resolver sus restricciones
//! internas. Para lo que devuelve cero —los que no tienen contenido todavía—
//! vale el mismo respaldo que allí.

use std::collections::HashMap;

use objc2::rc::Retained;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAttributedStringNSExtendedStringDrawing, NSButton, NSDatePicker, NSFont, NSFontAttributeName,
    NSPopUpButton, NSProgressIndicator, NSProgressIndicatorStyle, NSSearchField,
    NSSegmentedControl, NSSlider, NSStepper, NSStringDrawingOptions, NSSwitch, NSTextField, NSView,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSAttributedString, NSAttributedStringKey, NSDictionary, NSString};

/// Nombre con el que se guarda lo que un `NSTextField` se reserva a los lados
/// de su texto. No es un control: es una medida del sistema que viaja por el
/// mismo sitio, porque se pregunta en el mismo momento y por el mismo motivo.
pub const TEXT_INSET: &str = "__inset-de-texto";

/// Tamaños naturales, en puntos, indexados por el nombre del control.
pub type ControlSizes = HashMap<String, (f32, f32)>;

/// Cuánto más ancho que su texto es un rótulo de AppKit.
///
/// El layout mide el texto con `boundingRectWithSize:`, que mide **el texto** y
/// nada más. Un `NSTextField` dibuja ese texto dentro de su celda, y la celda
/// se guarda unos puntos a cada lado. Hoy son cuatro.
///
/// Cuatro puntos parecen nada y son justo el peor tamaño de error: una caja que
/// se queda corta no enseña el texto recortado, lo enseña **vacío**. El rótulo
/// va con `wraps`, así que lo que no cabe se lleva a la línea siguiente, y esa
/// línea está fuera del alto que el layout reservó para una. El texto
/// desaparece sin que nada dé error, que es exactamente lo que este proyecto no
/// admite. Se ve en cuanto un rótulo cae en una caja de su tamaño exacto, o sea
/// en cualquier `align-items: center`.
///
/// Se pregunta y no se escribe: es una medida del sistema, cambia con la
/// versión y con los ajustes de accesibilidad, igual que el alto de un
/// `NSSwitch`.
fn text_inset(mtm: MainThreadMarker) -> f32 {
    let font = NSFont::systemFontOfSize(an_layout::FontSpec::default().size as f64);
    let muestra = NSString::from_str("angular-native");

    let label = NSTextField::new(mtm);
    unsafe {
        label.setEditable(false);
        label.setBordered(false);
        label.setBezeled(false);
        label.setDrawsBackground(false);
        label.setFont(Some(&font));
        label.setStringValue(&muestra);
    }
    let cabe = label.fittingSize().width as f32;

    let font_ref: &objc2::runtime::AnyObject = &font;
    let attrs: Retained<NSDictionary<NSAttributedStringKey, _>> =
        NSDictionary::from_slices(&[unsafe { NSFontAttributeName }], &[font_ref]);
    // SAFETY: el diccionario lleva un `NSFont` bajo `NSFontAttributeName`, que
    // es el tipo que ese atributo espera.
    let attributed = unsafe { NSAttributedString::new_with_attributes(&muestra, &attrs) };
    let medido = attributed
        .boundingRectWithSize_options_context(
            CGSize { width: f64::MAX / 2.0, height: f64::MAX / 2.0 },
            NSStringDrawingOptions::UsesLineFragmentOrigin
                | NSStringDrawingOptions::UsesFontLeading,
            None,
        )
        .size
        .width as f32;

    // Nunca negativo: si algún día `fittingSize` midiera menos que el texto,
    // restar ancho sería peor que no hacer nada.
    (cabe - medido).max(0.0)
}

/// Le pregunta a cada control cuánto ocupa. Se llama una vez, en el arranque.
pub fn measure_controls(mtm: MainThreadMarker) -> ControlSizes {
    let mut sizes = ControlSizes::new();

    let mut record = |name: &str, view: &NSView| {
        let fitted = view.fittingSize();
        sizes.insert(name.to_owned(), (fitted.width as f32, fitted.height as f32));
    };

    record("Switch", &NSSwitch::new(mtm));
    record("Slider", &NSSlider::new(mtm));

    // El indicador y la barra son la misma clase con estilos distintos, y su
    // tamaño natural depende del estilo: hay que preguntárselo a cada uno ya
    // configurado, no a un `NSProgressIndicator` recién hecho.
    let spinner = NSProgressIndicator::new(mtm);
    spinner.setStyle(NSProgressIndicatorStyle::Spinning);
    record("ActivityIndicator", &spinner);

    let bar = NSProgressIndicator::new(mtm);
    bar.setStyle(NSProgressIndicatorStyle::Bar);
    record("ProgressBar", &bar);

    // Un botón sin rótulo mide lo que miden sus márgenes. Se le pone uno de
    // muestra para que el alto que salga sea el de un botón de verdad.
    let button = NSButton::new(mtm);
    button.setTitle(&NSString::from_str("Botón"));
    record("Button", &button);

    record("SegmentedControl", &NSSegmentedControl::new(mtm));
    record("Stepper", &NSStepper::new(mtm));
    record("SearchBar", &NSSearchField::new(mtm));
    record("Picker", &NSPopUpButton::new(mtm));
    record("DatePicker", &NSDatePicker::new(mtm));
    // La barra de pestañas de macOS es un segmentado (ver `support.rs`), así
    // que mide lo que mide él. El alto se sube un poco porque en el árbol va
    // como barra y no como control suelto.
    let tabs = NSSegmentedControl::new(mtm);
    record("TabBar", &tabs);

    sizes.insert(TEXT_INSET.to_owned(), (text_inset(mtm), 0.0));

    // La cabecera de navegación no es un control de AppKit y aquí no ocupa
    // nada: en un Mac la cabecera es la barra de título de la ventana, y el
    // `[title]` acaba ahí (ver `support.rs`). Cero por cero es la decisión, y
    // está escrita: sin esta línea saldría el mismo cero por no estar en la
    // tabla, que es otra cosa y no se distingue mirando el resultado.
    sizes.insert("NavigationBar".to_owned(), (0.0, 0.0));

    for (name, fallback) in [
        ("Slider", (200.0, 21.0)),
        ("ProgressBar", (200.0, 6.0)),
        ("Button", (80.0, 24.0)),
        ("SegmentedControl", (320.0, 24.0)),
        ("SearchBar", (320.0, 24.0)),
        ("Stepper", (13.0, 27.0)),
        ("Picker", (140.0, 25.0)),
        ("DatePicker", (200.0, 24.0)),
        ("TabBar", (320.0, 32.0)),
        ("Switch", (38.0, 22.0)),
        ("ActivityIndicator", (20.0, 20.0)),
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
