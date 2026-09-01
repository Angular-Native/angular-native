//! Diálogos del sistema.
//!
//! Un `UIAlertController` de verdad, presentado sobre el controlador raíz: el
//! aspecto, la animación y el comportamiento con el teclado y con VoiceOver
//! son los del sistema, no una imitación.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::NSString;
use objc2_ui_kit::{
    UIDevice,UIAlertAction, UIAlertActionStyle, UIAlertController, UIAlertControllerStyle, UIView};

/// Estado de un diálogo declarado en la plantilla.
#[derive(Default)]
pub struct AlertState {
    /// `true` para hoja de acciones en vez de diálogo centrado.
    pub sheet: bool,
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub visible: bool,
    /// El que está en pantalla, para poder quitarlo si el estado cambia.
    presented: Option<Retained<UIAlertController>>,
}

impl AlertState {
    /// Presenta o retira el diálogo según su estado. Idempotente: llamarla dos
    /// veces con el mismo estado no hace nada.
    pub fn sync(&mut self, mtm: MainThreadMarker, container: &UIView, node: NodeId, queue: &EventQueue) {
        if !self.visible {
            if let Some(controller) = self.presented.take() {
                unsafe { controller.dismissViewControllerAnimated_completion(true, None) };
            }
            return;
        }
        if self.presented.is_some() {
            return;
        }
        let Some(root) = container.window().and_then(|window| window.rootViewController()) else {
            return;
        };

        let controller = unsafe {
            UIAlertController::alertControllerWithTitle_message_preferredStyle(
                Some(&NSString::from_str(&self.title)),
                Some(&NSString::from_str(&self.message)),
                if self.sheet {
                    UIAlertControllerStyle::ActionSheet
                } else {
                    UIAlertControllerStyle::Alert
                },
                mtm,
            )
        };
        let buttons = if self.buttons.is_empty() {
            vec!["OK".to_owned()]
        } else {
            self.buttons.clone()
        };
        for (index, title) in buttons.iter().enumerate() {
            let queue = queue.clone();
            let handler = RcBlock::new(move |_action: core::ptr::NonNull<UIAlertAction>| {
                push_event(
                    &queue,
                    HostEvent {
                        target: node,
                        name: "select".to_owned(),
                        payload: vec![("index".to_owned(), PropValue::Number(index as f64))],
                    },
                );
            });
            let action = unsafe {
                UIAlertAction::actionWithTitle_style_handler(
                    Some(&NSString::from_str(title)),
                    UIAlertActionStyle::Default,
                    Some(&handler),
                    mtm,
                )
            };
            unsafe { controller.addAction(&action) };
        }

        unsafe {
            // Una hoja de acciones en iPad sale de un sitio concreto, y si no
            // se dice de cuál, UIKit no avisa: revienta la app.
            //
            // Solo en iPad. En iPhone la hoja sube desde abajo y ocupa el
            // ancho; anclarla ahí la convierte en un globo con pico, que no es
            // lo que hace ninguna app de iPhone.
            let es_ipad = UIDevice::currentDevice(mtm).userInterfaceIdiom()
                == objc2_ui_kit::UIUserInterfaceIdiom::Pad;
            if let Some(popover) = controller.popoverPresentationController().filter(|_| es_ipad) {
                popover.setSourceView(Some(container));
                let bounds = container.bounds();
                popover.setSourceRect(objc2_core_foundation::CGRect {
                    origin: objc2_core_foundation::CGPoint {
                        x: bounds.size.width / 2.0,
                        y: bounds.size.height,
                    },
                    size: objc2_core_foundation::CGSize { width: 0.0, height: 0.0 },
                });
            }
            root.presentViewController_animated_completion(&controller, true, None);
        }
        self.presented = Some(controller);
    }
}
