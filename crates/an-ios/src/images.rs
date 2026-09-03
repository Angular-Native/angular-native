//! Image loading.
//!
//! A path with no scheme is a bundle resource; one with `http` or `https` is
//! fetched over the network. Either way the image's real size comes back as a
//! `load` event, because the layout cannot place something whose size it does
//! not know and only the image knows how big it is.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::{MainThreadMarker, Message};
use objc2_foundation::{
    NSData, NSError, NSOperationQueue, NSString, NSURL, NSURLResponse, NSURLSession,
};
use objc2_ui_kit::{UIImage, UIImageView};

/// Starts loading `source` into the view. It returns immediately: if there is
/// a network in the way, the image shows up when it arrives.
pub fn load(
    _mtm: MainThreadMarker,
    view: &UIImageView,
    node: NodeId,
    source: &str,
    queue: EventQueue,
) {
    if source.is_empty() {
        view.setImage(None);
        return;
    }
    if !source.starts_with("http://") && !source.starts_with("https://") {
        // A bundle resource: it is synchronous and UIKit already caches it.
        let name = NSString::from_str(source);
        if let Some(image) = UIImage::imageNamed(&name) {
            apply(view, &image, node, &queue);
        }
        return;
    }

    let url = unsafe { NSURL::URLWithString(&NSString::from_str(source)) };
    let Some(url) = url else { return };
    let view = view.retain();
    let queue = queue.clone();

    // The block runs on a networking thread. Building the UIImage there is
    // fine —`imageWithData:` is thread-safe— but hanging it on the view is
    // not: that hops to the main queue.
    let completion = RcBlock::new(
        move |data: *mut NSData, _response: *mut NSURLResponse, _error: *mut NSError| {
            let Some(data) = (unsafe { data.as_ref() }) else { return };
            let Some(image) = UIImage::imageWithData(data) else { return };
            let view = view.clone();
            let queue = queue.clone();
            let on_main = RcBlock::new(move || {
                // SAFETY: the main queue runs on the main thread.
                let mtm = unsafe { MainThreadMarker::new_unchecked() };
                let _ = mtm;
                apply(&view, &image, node, &queue);
            });
            unsafe { NSOperationQueue::mainQueue().addOperationWithBlock(&on_main) };
        },
    );
    let task = unsafe {
        NSURLSession::sharedSession().dataTaskWithURL_completionHandler(&url, &completion)
    };
    unsafe { task.resume() };
}

/// Hangs the image up and announces its real size.
fn apply(view: &UIImageView, image: &UIImage, node: NodeId, queue: &EventQueue) {
    view.setImage(Some(image));
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

/// Translates `resizeMode` into UIKit's content mode.
pub fn content_mode(mode: &str) -> objc2_ui_kit::UIViewContentMode {
    use objc2_ui_kit::UIViewContentMode;
    match mode {
        "cover" => UIViewContentMode::ScaleAspectFill,
        "stretch" => UIViewContentMode::ScaleToFill,
        "center" => UIViewContentMode::Center,
        _ => UIViewContentMode::ScaleAspectFit,
    }
}
