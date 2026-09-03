//! visionOS's highlight.
//!
//! visionOS does have touches —a pinch of the hand counts as an indirect touch
//! and arrives through the same recognisers as on iOS— so no focus engine is
//! needed here. What does change is how the user knows what they are about to
//! press: they aim with their gaze, and without a highlight there is no way to
//! see where they are aiming.
//!
//! The system draws that highlight, outside the app's process and without
//! going through our frame loop, but only if the view asks for it with
//! `hoverStyle`. A `UIView` with a tap recogniser does not ask: the factory
//! value is `nil`, "this view has no highlight". `UIButton` and company do
//! bring it, for the same reason they bring focus on tvOS.
//!
//! `automaticStyle` is the system's highlight with the shape UIKit itself
//! infers from the view. Nothing is drawn by hand: if visionOS changes how the
//! highlight looks in some release, this changes with it.

use objc2_ui_kit::{UIHoverStyle, UIView};

/// Marks a view as pressable in the user's eyes.
pub fn mark_pressable(mtm: objc2::MainThreadMarker, view: &UIView) {
    if view.hoverStyle().is_some() {
        return;
    }
    view.setHoverStyle(Some(&UIHoverStyle::automaticStyle(mtm)));
}
