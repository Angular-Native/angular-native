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

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSButton, NSDatePicker, NSPopUpButton, NSProgressIndicator, NSProgressIndicatorStyle,
    NSSearchField, NSSegmentedControl, NSSlider, NSStepper, NSSwitch, NSView,
};
use objc2_foundation::NSString;

/// Tamaños naturales, en puntos, indexados por el nombre del control.
pub type ControlSizes = HashMap<String, (f32, f32)>;

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
