//! A `<Select>`'s menu.
//!
//! On iOS there is no drop-down control: what there is is a button that opens
//! a `UIMenu`. `UIPickerView` is the full-screen wheel, which is a different
//! thing and no longer what the system uses to pick from a short list.

use an_core::{NodeId, PropValue};
use an_host::{push_event, EventQueue, HostEvent};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::{UIAction, UIMenu, UIMenuElement};

/// Builds the menu with one action per option.
///
/// Each action carries its own index inside it, so choosing reports the number
/// and not the text: the text can repeat and can be translated, the index
/// cannot.
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
                    // The binding asks for a mutable pointer to the block;
                    // the block lives as long as the action does, and the
                    // action retains it.
                    RcBlock::as_ptr(&handler) as *mut _,
                    mtm,
                )
            };
            Retained::into_super(action)
        })
        .collect();
    UIMenu::menuWithChildren(&NSArray::from_retained_slice(&actions), mtm)
}
