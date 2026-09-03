//! Presenting a `<Modal>` as a real controller.
//!
//! It used to be a hidden view shown over everything else. It looked the same,
//! but it was not: it did not appear in UIKit's presentation stack, so the
//! system did not know there was anything modal in front. VoiceOver went on
//! reading what was behind, the keyboard did not adjust, and another presented
//! controller —a `UIAlertController`, say— came out above or below depending
//! on the order the views happened to have been created in.
//!
//! With a `UIViewController` in the way, UIKit settles all of that.

use an_core::NodeId;
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_ui_kit::{UIModalPresentationStyle, UIModalTransitionStyle, UIView, UIViewController};
// The sheet with detents is iOS's: `UISheetPresentationController` is marked
// `API_UNAVAILABLE(tvos)`. A television has no half a screen to drag.
#[cfg(not(target_os = "tvos"))]
use objc2_ui_kit::UISheetPresentationControllerDetent;

#[derive(Default)]
pub struct ModalState {
    pub visible: bool,
    /// How it comes in: covering everything, or as a sheet from the bottom.
    pub sheet: bool,
    presented: Option<Retained<UIViewController>>,
}

impl ModalState {
    /// Presents or withdraws the modal according to its state. Idempotent.
    pub fn sync(
        &mut self,
        mtm: MainThreadMarker,
        container: &UIView,
        content: &UIView,
        node: NodeId,
        queue: &EventQueue,
    ) {
        if !self.visible {
            if let Some(controller) = self.presented.take() {
                let queue = queue.clone();
                unsafe { controller.dismissViewControllerAnimated_completion(true, None) };
                // On withdrawal the view goes back to its own: the core
                // keeps sending it frames and props, and were it left hanging
                // off the controller on its way out, it would stop being seen
                // when the modal was reopened.
                push_event(
                    &queue,
                    HostEvent { target: node, name: "dismiss".to_owned(), payload: Vec::new() },
                );
            }
            return;
        }
        if self.presented.is_some() {
            return;
        }
        let Some(root) = container.window().and_then(|window| window.rootViewController()) else {
            return;
        };

        let controller = UIViewController::new(mtm);
        controller.setView(Some(content));
        // On tvOS there is no sheet: the modal presentation covers the
        // screen and that is that. It is said once, because a `[sheet]`
        // ignored in silence is a template that looks different with nobody
        // knowing why.
        #[cfg(target_os = "tvos")]
        let sheet = if self.sheet {
            crate::family::report(
                "<an-modal [sheet]>",
                "UISheetPresentationController is not in the SDK: the modal covers the whole \
                 screen, which is how it is presented on a television",
            );
            false
        } else {
            false
        };
        #[cfg(not(target_os = "tvos"))]
        let sheet = self.sheet;

        unsafe {
            if sheet {
                controller.setModalPresentationStyle(UIModalPresentationStyle::PageSheet);
                // The detents are the system's: half screen and full, with
                // its grabber and its pull-down-to-close gesture.
                #[cfg(not(target_os = "tvos"))]
                if let Some(sheet) = controller.sheetPresentationController() {
                    let detents = objc2_foundation::NSArray::from_retained_slice(&[
                        UISheetPresentationControllerDetent::mediumDetent(mtm),
                        UISheetPresentationControllerDetent::largeDetent(mtm),
                    ]);
                    sheet.setDetents(&detents);
                    sheet.setPrefersGrabberVisible(true);
                }
            } else {
                // The core has already laid the content out full screen:
                // with `OverFullScreen` the controller measures exactly that
                // and there is nothing to reposition.
                controller.setModalPresentationStyle(UIModalPresentationStyle::OverFullScreen);
                controller.setModalTransitionStyle(UIModalTransitionStyle::CoverVertical);
            }
            root.presentViewController_animated_completion(&controller, true, None);
        }
        self.presented = Some(controller);
    }

    /// Whether the modal is on screen right now.
    ///
    /// It is not the same question as `visible`: that is what the template
    /// asked for, this is what UIKit is actually showing, and between the two
    /// there is a presentation animation and a user who can pull the sheet
    /// down.
    pub fn presented(&self) -> bool {
        self.presented.is_some()
    }
}
