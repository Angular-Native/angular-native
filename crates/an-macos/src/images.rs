//! Image loading.
//!
//! The same as on iOS: a path with no scheme is a bundle resource, and one
//! with `http`/`https` is fetched over the network. Either way the real size
//! comes back as a `load` event, because the layout cannot place something
//! whose size it does not know.
//!
//! A bundle resource that is not in the bundle is said out loud. It used to be
//! the one failure this host was silent about — `imageNamed:` returns nil and
//! the view stays empty, which looks exactly like an image that is still
//! loading, a colour that matches the background, or a frame of zero height.
//! The name is printed once, with the directory it should have come from.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, Message};
use objc2_app_kit::{NSImage, NSImageScaling, NSImageView};
use objc2_foundation::{
    NSBundle, NSData, NSError, NSOperationQueue, NSString, NSURL, NSURLResponse, NSURLSession,
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
        match bundled(source) {
            Some(image) => apply(view, &image, node, &queue),
            None => warn_once(
                source,
                &format!(
                    "{source} is not in the app. A [source] with no scheme is a file that \
                     travelled with the app; put it in the project's resources/ directory, \
                     which `an` copies into Contents/Resources under the name it has there."
                ),
            ),
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
/// Three of the four are translations: `stretch` is `ScaleAxesIndependently`,
/// `center` is `ScaleNone` with the alignment `NSImageView` already defaults
/// to, and `contain` is `ScaleProportionallyUpOrDown`.
///
/// **`cover` is not one.** It fills the box and crops what does not fit, and
/// `NSImageView` has no scaling that crops: the image is drawn inside the cell,
/// never past it. What goes in is `contain`, which is a different picture — the
/// whole image with empty space where the crop would have been — so it is said
/// once instead of passing for the mode that was asked for. Cropping properly
/// means drawing into the layer with `contentsGravity` and giving up
/// `NSImageView`'s own handling of the image.
pub fn image_scaling(mode: &str) -> NSImageScaling {
    match mode {
        "stretch" => NSImageScaling::ScaleAxesIndependently,
        "center" => NSImageScaling::ScaleNone,
        "cover" => {
            warn_once(
                "resizeMode:cover",
                "`resizeMode=\"cover\"` does not apply on macOS: NSImageView cannot crop, so \
                 the image is fitted inside the box —`contain`— instead of filling it",
            );
            NSImageScaling::ScaleProportionallyUpOrDown
        }
        _ => NSImageScaling::ScaleProportionallyUpOrDown,
    }
}

/// The image a schemeless `source` names, from inside the `.app`.
///
/// Two lookups and they are not the same one. `imageNamed:` covers the asset
/// catalogue and the system's own names, and it caches what it finds, so it
/// goes first. What it does not cover is a path: `resources/icons/logo.png`
/// arrives in the bundle as `icons/logo.png`, and a name with a slash in it is
/// not a name `imageNamed:` knows how to resolve. That one is found by asking
/// the bundle where its resources are and reading the file.
fn bundled(source: &str) -> Option<Retained<NSImage>> {
    if let Some(image) = NSImage::imageNamed(&NSString::from_str(source)) {
        return Some(image);
    }
    let resources = unsafe { NSBundle::mainBundle().resourcePath() }?;
    let path = NSString::from_str(&format!("{}/{source}", resources));
    NSImage::initWithContentsOfFile(NSImage::alloc(), &path)
}

/// What has already been said. The core resends `source` on every change of the
/// node, so a missing file without this is a line per frame. Same shape as
/// `measure.rs`'s.
fn warn_once(key: &str, message: &str) {
    thread_local! {
        static SAID: std::cell::RefCell<std::collections::HashSet<String>> =
            std::cell::RefCell::new(std::collections::HashSet::new());
    }
    SAID.with(|said| {
        if said.borrow_mut().insert(key.to_owned()) {
            eprintln!("angular-native: {message}");
        }
    });
}
