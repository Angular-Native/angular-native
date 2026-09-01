//! El menú de un `<Select>`.
//!
//! En iOS no hay un control de desplegable: lo que hay es un botón que abre un
//! `UIMenu`. `UIPickerView` es la rueda a pantalla completa, que es otra cosa
//! y ya no es lo que usa el sistema para elegir de una lista corta.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::{UIAction, UIMenu, UIMenuElement};

/// Construye el menú con una acción por opción.
///
/// Cada acción lleva dentro el índice que le toca, así que al elegir se avisa
/// con el número y no con el texto: el texto puede repetirse y puede estar
/// traducido, el índice no.
pub fn build(
    mtm: MainThreadMarker,
    node: NodeId,
    titles: &[String],
    queue: &EventQueue,
) -> Retained<UIMenu> {
    let actions: Vec<Retained<UIMenuElement>> = titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            let queue = queue.clone();
            let handler = RcBlock::new(move |_action: core::ptr::NonNull<UIAction>| {
                push_event(
                    &queue,
                    HostEvent {
                        target: node,
                        name: "change".to_owned(),
                        payload: vec![("index".to_owned(), PropValue::Number(index as f64))],
                    },
                );
            });
            let action = unsafe {
                UIAction::actionWithTitle_image_identifier_handler(
                    &NSString::from_str(title),
                    None,
                    None,
                    // El enlace pide un puntero mutable al bloque; el
                    // bloque vive lo que viva la acción, que lo retiene.
                    RcBlock::as_ptr(&handler) as *mut _,
                    mtm,
                )
            };
            Retained::into_super(action)
        })
        .collect();
    UIMenu::menuWithChildren(&NSArray::from_retained_slice(&actions), mtm)
}
