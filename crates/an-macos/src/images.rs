//! Image loading.
//!
//! The same as on iOS: a path with no scheme is a bundle resource, and one
//! with `http`/`https` is fetched over the network. Either way the real size
//! comes back as a `load` event, because the layout cannot place something
//! whose size it does not know.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::{AnyThread, MainThreadMarker, Message};
use objc2_app_kit::{NSImage, NSImageScaling, NSImageView};
use objc2_foundation::{
    NSData, NSError, NSOperationQueue, NSString, NSURL, NSURLResponse, NSURLSession,
};

/// Starts loading `source` into the view. It returns immediately: if there is
/// a network in the way, the image shows up when it arrives.
pub fn load(
    _mtm: MainThreadMarker,
    view: &NSImageView,
    node: NodeId,
    source: &str,
    queue: EventQueue,
) {
    if source.is_empty() {
        unsafe { view.setImage(None) };
        return;
    }
    if !source.starts_with("http://") && !source.starts_with("https://") {
        // A bundle resource. `imageNamed:` already caches it.
        let name = NSString::from_str(source);
        if let Some(image) = NSImage::imageNamed(&name) {
            apply(view, &image, node, &queue);
        }
        return;
    }

    let Some(url) = (unsafe { NSURL::URLWithString(&NSString::from_str(source)) }) else {
        return;
    };
    let view = view.retain();
    let queue = queue.clone();

    // The block runs on a networking thread. Building the `NSImage` there is
    // fine, hanging it on the view is not: that hops to the main queue.
    let completion = RcBlock::new(
        move |data: *mut NSData, _response: *mut NSURLResponse, _error: *mut NSError| {
            let Some(data) = (unsafe { data.as_ref() }) else { return };
            let Some(image) = NSImage::initWithData(NSImage::alloc(), data) else { return };
            let view = view.clone();
            let queue = queue.clone();
            let on_main = RcBlock::new(move || apply(&view, &image, node, &queue));
            unsafe { NSOperationQueue::mainQueue().addOperationWithBlock(&on_main) };
        },
    );
    let task = unsafe {
        NSURLSession::sharedSession().dataTaskWithURL_completionHandler(&url, &completion)
    };
    unsafe { task.resume() };
}

/// Hangs the image up and announces its real size.
fn apply(view: &NSImageView, image: &NSImage, node: NodeId, queue: &EventQueue) {
    unsafe { view.setImage(Some(image)) };
    let size = unsafe { image.size() };
    push_event(
        queue,
        HostEvent {
            target: node,
            name: "load".to_owned(),
            payload: vec![
                ("width".to_owned(), PropValue::Number(size.width)),
                ("height".to_owned(), PropValue::Number(size.height)),
            ],
        },
    );
}

/// Translates `resizeMode` into AppKit's scaling.
///
/// AppKit has four modes and not UIKit's ten, so `cover` and `center` fall
/// onto the nearest one — said here rather than pretending the mapping is
/// exact: `cover` crops on iOS and fits inside here, because `NSImageView`
/// cannot crop without spilling out.
pub fn image_scaling(mode: &str) -> NSImageScaling {
    match mode {
        "stretch" => NSImageScaling::ScaleAxesIndependently,
        "center" => NSImageScaling::ScaleNone,
        _ => NSImageScaling::ScaleProportionallyUpOrDown,
    }
}
