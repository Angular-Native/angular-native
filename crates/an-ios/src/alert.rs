//! System dialogs.
//!
//! A real `UIAlertController`, presented over the root controller: the look,
//! the animation and the behaviour with the keyboard and with VoiceOver are
//! the system's, not an imitation.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::NSString;
use objc2_ui_kit::{
    UIDevice,UIAlertAction, UIAlertActionStyle, UIAlertController, UIAlertControllerStyle, UIView};

/// The state of a dialog declared in the template.
#[derive(Default)]
pub struct AlertState {
    /// `true` for an action sheet instead of a centred dialog.
    pub sheet: bool,
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub visible: bool,
    /// The one on screen, so it can be taken away if the state changes.
    presented: Option<Retained<UIAlertController>>,
}

impl AlertState {
    /// Presents or withdraws the dialog according to its state. Idempotent:
    /// calling it twice with the same state does nothing.
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
            // On an iPad an action sheet comes out of a particular place,
            // and if it is not told which, UIKit does not warn: it crashes the
            // app.
            //
            // On iPad only. On an iPhone the sheet rises from the bottom and
            // takes the width; anchoring it there turns it into a popover with
            // an arrow, which is not what any iPhone app does.
            let is_ipad = UIDevice::currentDevice(mtm).userInterfaceIdiom()
                == objc2_ui_kit::UIUserInterfaceIdiom::Pad;
            if let Some(popover) = controller.popoverPresentationController().filter(|_| is_ipad) {
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
