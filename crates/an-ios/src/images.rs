//! Carga de imágenes.
//!
//! Una ruta sin esquema es un recurso del bundle; con `http` o `https` se baja
//! por red. En los dos casos el tamaño real de la imagen se devuelve como
//! evento `load`, porque el layout no puede colocar algo cuyo tamaño no conoce
//! y solo la imagen sabe cuánto mide.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::{MainThreadMarker, Message};
use objc2_foundation::{
    NSData, NSError, NSOperationQueue, NSString, NSURL, NSURLResponse, NSURLSession,
};
use objc2_ui_kit::{UIImage, UIImageView};

/// Empieza a cargar `source` en la vista. Vuelve en el acto: si hay red de por
/// medio, la imagen aparece cuando llegue.
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
        // Recurso del bundle: es síncrono y UIKit ya lo cachea.
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

    // El bloque corre en un hilo de red. Construir la UIImage ahí está bien
    // —`imageWithData:` es thread-safe—, pero colgarla de la vista no: eso se
    // salta a la cola principal.
    let completion = RcBlock::new(
        move |data: *mut NSData, _response: *mut NSURLResponse, _error: *mut NSError| {
            let Some(data) = (unsafe { data.as_ref() }) else { return };
            let Some(image) = UIImage::imageWithData(data) else { return };
            let view = view.clone();
            let queue = queue.clone();
            let on_main = RcBlock::new(move || {
                // SAFETY: la cola principal ejecuta en el hilo principal.
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

/// Cuelga la imagen y avisa de su tamaño real.
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

/// Traduce `resizeMode` al modo de contenido de UIKit.
pub fn content_mode(mode: &str) -> objc2_ui_kit::UIViewContentMode {
    use objc2_ui_kit::UIViewContentMode;
    match mode {
        "cover" => UIViewContentMode::ScaleAspectFill,
        "stretch" => UIViewContentMode::ScaleToFill,
        "center" => UIViewContentMode::Center,
        _ => UIViewContentMode::ScaleAspectFit,
    }
}
