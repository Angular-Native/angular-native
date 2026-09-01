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
use objc2_ui_kit::{UIAlertAction, UIAlertActionStyle, UIAlertController, UIAlertControllerStyle, UIView};

/// Estado de un diálogo declarado en la plantilla.
#[derive(Default)]
pub struct AlertState {
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
                UIAlertControllerStyle::Alert,
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
            root.presentViewController_animated_completion(&controller, true, None);
        }
        self.presented = Some(controller);
    }
}
