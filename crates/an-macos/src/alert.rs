//! Diálogos del sistema: `NSAlert` de verdad.
//!
//! Se presenta como hoja de la ventana (`beginSheetModalForWindow:`) y no como
//! modal de aplicación. La diferencia importa en escritorio: un modal de
//! aplicación bloquea el bucle de eventos, y el bucle de eventos es el que
//! llama a `an_runtime_frame`. Con un `runModal` la app se congelaría entera
//! —temporizadores incluidos— hasta que alguien contestara. Una hoja no
//! bloquea: el diálogo es modal respecto a su ventana y el resto de la app
//! sigue viva, que es justo lo que se necesita.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSModalResponse, NSView};
use objc2_foundation::NSString;

/// La primera respuesta que devuelve una hoja: el primer botón es 1000, el
/// segundo 1001, y así. Restándola sale el índice que espera la plantilla.
const FIRST_BUTTON: NSModalResponse = 1000;

/// Estado de un diálogo declarado en la plantilla.
#[derive(Default)]
pub struct AlertState {
    /// En iOS `sheet` significa hoja de acciones. macOS no tiene ese control
    /// —lo más parecido es un menú contextual, que es otra cosa— así que aquí
    /// solo cambia el estilo del diálogo: informativo en vez de de aviso.
    pub sheet: bool,
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub visible: bool,
    /// El que está en pantalla, para no presentarlo dos veces.
    presented: bool,
}

impl AlertState {
    /// Presenta o retira el diálogo según su estado. Idempotente: llamarla dos
    /// veces con el mismo estado no hace nada.
    pub fn sync(
        &mut self,
        mtm: MainThreadMarker,
        container: &NSView,
        node: NodeId,
        queue: &EventQueue,
    ) {
        if !self.visible {
            self.presented = false;
            return;
        }
        if self.presented {
            return;
        }
        let Some(window) = container.window() else { return };

        let alert = NSAlert::new(mtm);
        unsafe {
            alert.setMessageText(&NSString::from_str(&self.title));
            alert.setInformativeText(&NSString::from_str(&self.message));
            alert.setAlertStyle(if self.sheet {
                NSAlertStyle::Informational
            } else {
                NSAlertStyle::Warning
            });
        }
        let buttons =
            if self.buttons.is_empty() { vec!["OK".to_owned()] } else { self.buttons.clone() };
        for title in &buttons {
            unsafe { alert.addButtonWithTitle(&NSString::from_str(title)) };
        }

        let queue = queue.clone();
        let handler = RcBlock::new(move |response: NSModalResponse| {
            let index = (response - FIRST_BUTTON).max(0) as f64;
            push_event(
                &queue,
                HostEvent {
                    target: node,
                    name: "select".to_owned(),
                    payload: vec![("index".to_owned(), PropValue::Number(index))],
                },
            );
        });
        unsafe { alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler)) };
        self.presented = true;
    }
}
