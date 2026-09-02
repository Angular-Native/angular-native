//! Carga de imágenes.
//!
//! Lo mismo que en iOS: una ruta sin esquema es un recurso del bundle y con
//! `http`/`https` se baja por red. En los dos casos el tamaño real vuelve como
//! evento `load`, porque el layout no puede colocar algo cuyo tamaño no conoce.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::{AnyThread, MainThreadMarker, Message};
use objc2_app_kit::{NSImage, NSImageScaling, NSImageView};
use objc2_foundation::{
    NSData, NSError, NSOperationQueue, NSString, NSURL, NSURLResponse, NSURLSession,
};

/// Empieza a cargar `source` en la vista. Vuelve en el acto: si hay red de por
/// medio, la imagen aparece cuando llegue.
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
        // Recurso del bundle. `imageNamed:` ya lo cachea.
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

    // El bloque corre en un hilo de red. Construir la `NSImage` ahí vale, pero
    // colgarla de la vista no: eso se salta a la cola principal.
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

/// Cuelga la imagen y avisa de su tamaño real.
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

/// Traduce `resizeMode` al escalado de AppKit.
///
/// AppKit tiene cuatro modos y no los diez de UIKit, así que `cover` y
/// `center` caen en el más parecido y se dice aquí en vez de fingir que hay una
/// correspondencia exacta: `cover` recorta en iOS y aquí encaja dentro, porque
/// `NSImageView` no sabe recortar sin salirse.
pub fn image_scaling(mode: &str) -> NSImageScaling {
    match mode {
        "stretch" => NSImageScaling::ScaleAxesIndependently,
        "center" => NSImageScaling::ScaleNone,
        _ => NSImageScaling::ScaleProportionallyUpOrDown,
    }
}
