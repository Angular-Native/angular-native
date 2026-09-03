//! System dialogs: a real `NSAlert`.
//!
//! It is presented as a sheet on the window (`beginSheetModalForWindow:`) and
//! not as an application-modal dialog. The difference matters on the desktop:
//! an application-modal dialog blocks the event loop, and the event loop is
//! what calls `an_runtime_frame`. With a `runModal` the whole app would freeze
//! —timers included— until somebody answered. A sheet does not block: the
//! dialog is modal with respect to its window and the rest of the app stays
//! alive, which is exactly what is needed.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSModalResponse, NSView};
use objc2_foundation::NSString;

/// The first response a sheet returns: the first button is 1000, the second
/// 1001, and so on. Subtracting it gives the index the template expects.
const FIRST_BUTTON: NSModalResponse = 1000;

/// The state of a dialog declared in the template.
#[derive(Default)]
pub struct AlertState {
    /// On iOS `sheet` means an action sheet. macOS has no such control —the
    /// nearest thing is a context menu, which is something else— so here it
    /// only changes the dialog's style: informational instead of a warning.
    pub sheet: bool,
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub visible: bool,
    /// The one on screen, so it is not presented twice.
    presented: bool,
}

impl AlertState {
    /// Presents or withdraws the dialog according to its state. Idempotent:
    /// calling it twice with the same state does nothing.
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
