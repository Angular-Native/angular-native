//! tvOS's focus engine.
//!
//! On a television there are no touches. The remote moves an invisible cursor
//! between the views that declare themselves focusable and the centre button
//! presses whichever is focused at that moment; UIKit decides that route
//! geometrically, from the views' frames. Our frames are absolute and taffy
//! works them out, so the focus engine receives exactly the grid the template
//! describes and there is nothing to translate.
//!
//! What does have to be done is to declare oneself.
//! `-[UIView canBecomeFocused]` returns `NO` out of the box, and a view that
//! returns `NO` **cannot be pressed on a television**: the tap recogniser
//! attaches, nothing fails, and the button simply never responds. `UIButton`,
//! `UITextField`, `UISegmentedControl` and `UISearchBar` bring their `YES` as
//! standard because they are controls; a `UIView` with a `(press)` does not,
//! and that is the difference between iOS and tvOS that breaks the most code.
//!
//! `canBecomeFocused` can only be changed by inheriting, so there is a
//! subclass of `UIView` here. The host uses it for `an-view`, which is the
//! primitive people hang `(press)` on; `an-text` and `an-image` are `UILabel`
//! and `UIImageView` and still cannot take focus, so on a television they have
//! to be wrapped. It is said in `docs/tvos.md` and `events::attach` warns
//! about it in the log the moment anybody tries.
//!
//! No highlight is drawn. tvOS has no `UIFocusEffect` —it is marked
//! `API_UNAVAILABLE(tvos)`, it belongs to iOS— and the system paints nothing of
//! its own accord over an ordinary view: on tvOS the highlight is each
//! control's own business, lifting and casting a shadow because UIKit draws
//! it. Inventing a border or a scale here would be exactly the hand-made
//! imitation this project does not do. Instead the view emits `focus` and
//! `blur` towards JavaScript, and the template decides with the props it
//! already has: `[backgroundColor]`, `[scale]` and `[animate]`.

use std::cell::Cell;

use an_core::NodeId;
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, Bool};
use objc2::{define_class, msg_send, ClassType, DefinedClass};
use objc2_foundation::{NSArray, NSNumber, NSObjectProtocol};
use objc2_ui_kit::{
    UIFocusAnimationCoordinator, UIFocusUpdateContext, UIGestureRecognizer, UIPressType, UIView,
};

pub struct FocusIvars {
    node: NodeId,
    queue: EventQueue,
    /// `events::attach` switches it on when the template asks for `(focus)`
    /// or `(blur)` without asking for any press. It is a `Cell` because it
    /// arrives after the view has been built: when the node is created it is
    /// not yet known which events it carries.
    wants_focus: Cell<bool>,
}

define_class!(
    // SAFETY:
    // - UIView takes subclasses and this one does not touch its
    //   initialisation.
    // - AnFocusableView does not implement Drop.
    #[unsafe(super(UIView))]
    #[name = "AnFocusableView"]
    #[ivars = FocusIvars]
    pub struct FocusableView;

    impl FocusableView {
        /// What the focus engine asks before considering this view.
        ///
        /// The answer is worked out on the spot rather than kept in a counter:
        /// the view is focusable if it has any gesture recogniser on it, and
        /// UIKit already keeps that list. So when the core removes the last
        /// listener and `detach` takes the recogniser away, the view stops
        /// being focusable on its own. A counter of our own would have to be
        /// squared with every attach and every detach, and the day it went out
        /// of step what would be left is a box that steals the focus and does
        /// nothing.
        #[unsafe(method(canBecomeFocused))]
        fn can_become_focused(&self) -> Bool {
            if self.ivars().wants_focus.get() {
                return Bool::YES;
            }
            Bool::new(self.gestureRecognizers().is_some_and(|gestures| !gestures.is_empty()))
        }

        /// The focus came in or went out. UIKit sends this to both views
        /// involved, so which is which has to be read off the context.
        #[unsafe(method(didUpdateFocusInContext:withAnimationCoordinator:))]
        fn did_update_focus(
            &self,
            context: &UIFocusUpdateContext,
            coordinator: &UIFocusAnimationCoordinator,
        ) {
            // Super first: UIKit leans on its own implementation to keep the
            // focus environment's state, and skipping it leaves the system
            // believing things that are not so.
            unsafe {
                let _: () = msg_send![
                    super(self),
                    didUpdateFocusInContext: context,
                    withAnimationCoordinator: coordinator,
                ];
            }

            let ivars = self.ivars();
            let me: *const UIView = self.as_ref();

            let entering = unsafe { context.nextFocusedView() }
                .is_some_and(|view| Retained::as_ptr(&view) == me);
            let leaving = unsafe { context.previouslyFocusedView() }
                .is_some_and(|view| Retained::as_ptr(&view) == me);

            // Nothing to say if this view is neither of the two: the
            // notification reaches the containers along the way as well.
            if entering {
                emit(&ivars.queue, ivars.node, "focus");
            }
            if leaving {
                emit(&ivars.queue, ivars.node, "blur");
            }
        }
    }
);

fn emit(queue: &EventQueue, target: NodeId, name: &str) {
    push_event(queue, HostEvent { target, name: name.to_owned(), payload: Vec::new() });
}

impl FocusableView {
    pub fn new(mtm: objc2::MainThreadMarker, node: NodeId, queue: EventQueue) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(FocusIvars { node, queue, wants_focus: Cell::new(false) });
        unsafe { msg_send![super(this), init] }
    }

    /// For `(focus)` and `(blur)` with no press: a view that only wants to
    /// know when it is being looked at carries no recogniser at all, so
    /// `canBecomeFocused`'s calculation would not see it.
    ///
    /// `setNeedsFocusUpdate` is not called here: the engine looks at the
    /// environment again when the hierarchy changes, and forcing it from the
    /// mounting of a node would move the user's focus mid-screen.
    pub fn set_wants_focus(&self, wants: bool) {
        self.ivars().wants_focus.set(wants);
        if wants {
            self.setUserInteractionEnabled(true);
        }
    }

    /// A focusable view has to receive events. A `UIView` with interaction
    /// switched off does not enter the remote's route.
    pub fn allow_interaction(&self) {
        self.setUserInteractionEnabled(true);
    }
}

/// The same view, if it is one of ours.
///
/// The class is checked before converting, which is what separates this from
/// an `as any`: if the node was not created focusable, `None` comes out here
/// and the caller says so out loud instead of writing over a view that is not
/// the one it thinks.
pub fn focusable(view: &UIView) -> Option<&FocusableView> {
    let class: &AnyClass = FocusableView::class();
    if !view.isKindOfClass(class) {
        return None;
    }
    // SAFETY: the class was just checked.
    Some(unsafe { &*(view as *const UIView).cast::<FocusableView>() })
}

/// The remote does not send touches: it sends button presses.
///
/// `allowedPressTypes` decides which one a recogniser responds to, and the SDK
/// documents its default as "platform dependent". It is set by hand so as not
/// to depend on that. The directional ones are never asked for: they belong to
/// the focus engine, and taking them away leaves the remote unable to move
/// around the screen.
pub fn allow_press(recognizer: &UIGestureRecognizer, press: UIPressType) {
    let types = NSArray::from_retained_slice(&[NSNumber::new_isize(press.0)]);
    recognizer.setAllowedPressTypes(&types);
}

/// The centre button: the one that presses whatever is focused.
pub const SELECT: UIPressType = UIPressType::Select;

/// The menu button: the remote's "back". It is what on a phone is the drag in
/// from the left edge, which on tvOS does not exist because there is no edge
/// to drag from.
pub const MENU: UIPressType = UIPressType::Menu;
