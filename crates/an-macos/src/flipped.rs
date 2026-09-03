//! This host's container view: an `NSView` whose origin is at the top.
//!
//! It is the biggest difference between AppKit and UIKit, and it is not
//! cosmetic. In UIKit a view's origin is at the top left and `y` grows
//! downwards; in AppKit it is at the **bottom** left and `y` grows upwards.
//! The core works the layout out with taffy, which is CSS, and CSS is like
//! UIKit.
//!
//! There are two ways to fix it:
//!
//! 1. Turn every frame upside down in `set_layout`, subtracting from the
//!    parent's height. That requires the host to know each node's parent's
//!    height at the moment it places it, and that height may arrive *after*
//!    the child's: the core sends the frames in tree order, not outside in.
//!    The result would be one view placed right and another one out of place
//!    depending on the order, which is the kind of bug that is invisible until
//!    it is not.
//! 2. Tell AppKit that these views run the other way round. `isFlipped` is
//!    exactly that, and it is a property of the *parent* view: it is the
//!    container that decides how its children's frames are read.
//!
//! The second is what is done. Since every container this host mounts is of
//! this class —view, stack, scroll content, the modal's layer— any child,
//! whether a system `NSButton` or not, gets its frame in top-down coordinates
//! with nothing to convert anywhere.
//!
//! Clipping goes separately, through `clipsToBounds`: in UIKit it is a
//! property of the view, and in AppKit a layer has to be asked for and told
//! about it.
//!
//! ## And the swipe lives here
//!
//! AppKit has no `NSSwipeGestureRecognizer`, and for a long time that had
//! `(swipeLeft)` and its three siblings warning that they were never going to
//! arrive. But the gesture **does exist**: it is not a recogniser hung off a
//! view, it is an event —`NSEventTypeSwipe`— that the system sends down the
//! responder chain with `swipeWithEvent:`. The same trackpad produces it, by
//! the same criteria any Mac app uses to turn a page, so the threshold, the
//! number of fingers and whether the gesture counts at all are the system's to
//! decide, which is the rule of the house.
//!
//! What it takes to catch it is for the method to be on the class, and that
//! can only be done with a class of our own. Hence it being here and not in
//! `events.rs` with the recognisers: every other gesture attaches to any view,
//! this one only to the ones in this file. A system control does not catch it,
//! but neither does it lose it: by not handling it, the event goes up to the
//! next responder in the chain, which is its parent view. See
//! `support::catches_swipe`.

use std::cell::{Cell, RefCell};

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSView};
use objc2_foundation::NSObjectProtocol;

use crate::support::swipe_direction;

pub struct FlippedIvars {
    /// Who to tell, once somebody has asked for a swipe.
    swipe: RefCell<Option<(NodeId, EventQueue)>>,
    /// The directions subscribed to. Zero means "nobody is listening", and
    /// then the event is passed on down the responder chain as it arrived.
    swipe_mask: Cell<u8>,
}

define_class!(
    // SAFETY:
    // - `NSView` places no requirements on its subclasses beyond living on the
    //   main thread, which `MainThreadOnly` guarantees.
    // - `AnFlippedView` does not implement `Drop`.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnFlippedView"]
    #[ivars = FlippedIvars]
    pub struct FlippedView;

    unsafe impl NSObjectProtocol for FlippedView {}

    impl FlippedView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        /// The trackpad's swipe.
        ///
        /// Which direction each sign means is `support::swipe_direction`'s to
        /// say; it sits outside the platform precisely so it can be tested
        /// without a trackpad.
        #[unsafe(method(swipeWithEvent:))]
        fn swipe_with_event(&self, event: &NSEvent) {
            let ivars = self.ivars();
            let deltas = (unsafe { event.deltaX() }, unsafe { event.deltaY() });
            let Some((bit, name)) = swipe_direction(deltas.0, deltas.1) else {
                // A swipe with no direction belongs to nobody. It is passed
                // on, which is what the view would do without this method.
                return unsafe { msg_send![super(self), swipeWithEvent: event] };
            };

            if ivars.swipe_mask.get() & bit == 0 {
                // This view is not listening for that direction. Stopping it
                // here would be swallowing it: the `<an-view>` outside would
                // stop receiving it just because the one inside exists.
                return unsafe { msg_send![super(self), swipeWithEvent: event] };
            }

            let Some((node, queue)) = ivars.swipe.borrow().clone() else {
                return unsafe { msg_send![super(self), swipeWithEvent: event] };
            };
            // Where the pointer was, in this view's coordinates, just as in a
            // `press`. The window gives the point in its own.
            let point = self.convertPoint_fromView(unsafe { event.locationInWindow() }, None);
            push_event(
                &queue,
                HostEvent {
                    target: node,
                    name: name.to_owned(),
                    payload: vec![
                        ("x".to_owned(), PropValue::Number(point.x)),
                        ("y".to_owned(), PropValue::Number(point.y)),
                    ],
                },
            );
        }
    }
);

impl FlippedView {
    pub fn new(mtm: objc2::MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm)
            .set_ivars(FlippedIvars { swipe: RefCell::new(None), swipe_mask: Cell::new(0) });
        unsafe { msg_send![super(this), init] }
    }

    /// Clips whichever children spill out. In UIKit it is `clipsToBounds`;
    /// here the view has to be given a layer and the layer has to be told.
    pub fn clip_to_bounds(&self) {
        self.setWantsLayer(true);
        if let Some(layer) = unsafe { self.layer() } {
            layer.setMasksToBounds(true);
        }
    }

    /// Starts delivering one swipe direction.
    pub fn listen_swipe(&self, node: NodeId, queue: EventQueue, bit: u8) {
        let ivars = self.ivars();
        *ivars.swipe.borrow_mut() = Some((node, queue));
        ivars.swipe_mask.set(ivars.swipe_mask.get() | bit);
    }

    /// Stops delivering it. When none is left, the queue is released too: a
    /// view that is no longer listening has no business retaining anything.
    pub fn unlisten_swipe(&self, bit: u8) {
        let ivars = self.ivars();
        let mask = ivars.swipe_mask.get() & !bit;
        ivars.swipe_mask.set(mask);
        if mask == 0 {
            *ivars.swipe.borrow_mut() = None;
        }
    }
}
