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
// The sheet is iOS's: `UISheetPresentationController` is marked
// `API_UNAVAILABLE(tvos)`. A television has no half a screen to drag, so
// everything a detent needs leaves the binary there.
#[cfg(not(target_os = "tvos"))]
use block2::RcBlock;
#[cfg(not(target_os = "tvos"))]
use core::ptr::NonNull;
#[cfg(not(target_os = "tvos"))]
use objc2::runtime::ProtocolObject;
#[cfg(not(target_os = "tvos"))]
use objc2_core_foundation::CGFloat;
#[cfg(not(target_os = "tvos"))]
use objc2_foundation::{NSArray, NSString};
#[cfg(not(target_os = "tvos"))]
use objc2_ui_kit::{
    UISheetPresentationController, UISheetPresentationControllerDetent,
    UISheetPresentationControllerDetentResolutionContext,
};

/// One of the heights a sheet is allowed to rest at.
///
/// The three UIKit ships, no more: `.medium()`, `.large()` and
/// `.custom(resolver:)`, which is a height in points. There is no fourth kind
/// to invent — a detent is not a percentage or a fraction of the content, it is
/// a resolver UIKit calls, and everything else would be arithmetic of ours
/// dressed up as the system's.
#[cfg(not(target_os = "tvos"))]
#[derive(Clone, Copy, PartialEq)]
pub enum SheetDetent {
    Medium,
    Large,
    /// `.custom(resolver:)`, iOS 16 and up. Points from the bottom.
    Custom(f64),
}

#[cfg(not(target_os = "tvos"))]
impl SheetDetent {
    /// The UIKit object. `index` only names the custom ones.
    fn to_uikit(
        self,
        mtm: MainThreadMarker,
        index: usize,
    ) -> Retained<UISheetPresentationControllerDetent> {
        match self {
            SheetDetent::Medium => UISheetPresentationControllerDetent::mediumDetent(mtm),
            SheetDetent::Large => UISheetPresentationControllerDetent::largeDetent(mtm),
            SheetDetent::Custom(points) => {
                // The identifier is the position in the list and not the
                // height, because UIKit wants them unique and answers a
                // repeated one by aborting. `[240, 240]` is a silly thing to
                // write and it is not worth a crash.
                let identifier = NSString::from_str(&format!("an-custom-{index}"));
                let resolver = RcBlock::new(
                    move |context: NonNull<
                        ProtocolObject<dyn UISheetPresentationControllerDetentResolutionContext>,
                    >| {
                        // Asking for more than the sheet can be is not worth
                        // dropping the detent for. The resolver is asked again
                        // on every rotation, and what "600 points" means on a
                        // phone lying down is "all the way up".
                        let context = unsafe { context.as_ref() };
                        (points as CGFloat).min(context.maximumDetentValue())
                    },
                );
                UISheetPresentationControllerDetent::customDetentWithIdentifier_resolver(
                    Some(&identifier),
                    &resolver,
                    mtm,
                )
            }
        }
    }
}

#[derive(Default)]
pub struct ModalState {
    pub visible: bool,
    /// How it comes in: covering everything, or as a sheet from the bottom.
    pub sheet: bool,
    /// Where the sheet may rest. Empty is not "nowhere": it is the two the
    /// system suggests, which is what a template that says nothing gets.
    #[cfg(not(target_os = "tvos"))]
    detents: Vec<SheetDetent>,
    presented: Option<Retained<UIViewController>>,
}

impl ModalState {
    /// What `[ios].detents` asked for, as the JSON the wire carries:
    /// `["medium", 320]`. `None` un-sets it.
    ///
    /// An entry that is neither of the two names nor a height above zero is
    /// dropped with a word. The template's type already refuses those, so
    /// getting here means an object assembled at run time — exactly the case
    /// the compiler cannot see and the one the third rule of the wrapper is
    /// about. Dropping the whole list would be worse: an empty array is what
    /// UIKit answers by throwing.
    pub fn set_detents(&mut self, spec: Option<&str>) {
        #[cfg(target_os = "tvos")]
        {
            if spec.is_some() {
                crate::family::report(
                    "<an-modal [ios].detents>",
                    "UISheetPresentationController is not in the SDK: there is no sheet, so \
                     there is nothing for it to rest at",
                );
            }
        }
        #[cfg(not(target_os = "tvos"))]
        {
            self.detents = spec.map(parse_detents).unwrap_or_default();
        }
    }

    /// Hands the sheet its list of resting heights.
    #[cfg(not(target_os = "tvos"))]
    fn apply_detents(&self, mtm: MainThreadMarker, sheet: &UISheetPresentationController) {
        let detents: Vec<_> = if self.detents.is_empty() {
            // What the system suggests when nobody says otherwise: half the
            // screen and all of it, with the grabber and the pull-down.
            vec![
                UISheetPresentationControllerDetent::mediumDetent(mtm),
                UISheetPresentationControllerDetent::largeDetent(mtm),
            ]
        } else {
            self.detents.iter().enumerate().map(|(i, d)| d.to_uikit(mtm, i)).collect()
        };
        sheet.setDetents(&NSArray::from_retained_slice(&detents));
    }

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
            // The list can change while the sheet is up —it is a signal like
            // any other— and UIKit takes a new one on a live controller. Not
            // re-applying it here would make `[ios].detents` a prop that only
            // works before the sheet opens, which is the kind of half-working
            // nobody reports as a bug: they just stop using it.
            #[cfg(not(target_os = "tvos"))]
            if self.sheet {
                if let Some(sheet) =
                    self.presented.as_ref().and_then(|c| c.sheetPresentationController())
                {
                    self.apply_detents(mtm, &sheet);
                }
            }
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
                // Where it may rest is the template's call —`[ios].detents`—
                // and the grabber is not: with more than one resting height
                // there has to be something saying the sheet can be dragged,
                // and with only one it is what says it can be dismissed.
                #[cfg(not(target_os = "tvos"))]
                if let Some(sheet) = controller.sheetPresentationController() {
                    self.apply_detents(mtm, &sheet);
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

/// `["medium", 320]` into detents, saying what it had to leave out.
///
/// The order is the template's and is kept: UIKit reads the array as it is
/// given, and re-sorting it would be deciding for a template that may well have
/// meant it.
#[cfg(not(target_os = "tvos"))]
fn parse_detents(spec: &str) -> Vec<SheetDetent> {
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(spec) else {
        crate::accessibility::warn_once(
            &format!("modal detents {spec}"),
            &format!(
                "<an-modal> [ios].detents is not a list: {spec}. \
                 The sheet keeps the two the system suggests"
            ),
        );
        return Vec::new();
    };
    let mut detents = Vec::with_capacity(entries.len());
    for entry in entries {
        match &entry {
            serde_json::Value::String(n) if n == "medium" => detents.push(SheetDetent::Medium),
            serde_json::Value::String(n) if n == "large" => detents.push(SheetDetent::Large),
            // Zero and below are not a detent UIKit can resolve, and a sheet
            // resting at no height is a sheet that is not there.
            serde_json::Value::Number(points) if points.as_f64().is_some_and(|p| p > 0.0) => {
                detents.push(SheetDetent::Custom(points.as_f64().unwrap_or_default()))
            }
            other => crate::accessibility::warn_once(
                &format!("modal detent {other}"),
                &format!(
                    "<an-modal> has no detent {other} in [ios].detents, so it is left out. \
                     It takes \"medium\", \"large\" and a height in points"
                ),
            ),
        }
    }
    detents
}
