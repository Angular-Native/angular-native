//! Presentación de `<Modal>` como un controlador de verdad.
//!
//! Antes era una vista escondida que se enseñaba encima de todo. Se veía
//! igual, pero no lo era: no aparecía en la pila de presentación de UIKit, así
//! que el sistema no sabía que había algo modal delante. VoiceOver seguía
//! leyendo lo de detrás, el teclado no ajustaba, y otro controlador presentado
//! —un `UIAlertController`, por ejemplo— salía por encima o por debajo según
//! el orden en que se hubieran creado las vistas.
//!
//! Con un `UIViewController` de por medio todo eso lo resuelve UIKit.

use an_core::NodeId;
use an_host::{push_event, EventQueue, HostEvent};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_ui_kit::{UIModalPresentationStyle, UIModalTransitionStyle, UIView, UIViewController};
// La hoja con topes es de iOS: `UISheetPresentationController` está marcado
// `API_UNAVAILABLE(tvos)`. Una tele no tiene medio pantalla que arrastrar.
#[cfg(not(target_os = "tvos"))]
use objc2_ui_kit::UISheetPresentationControllerDetent;

#[derive(Default)]
pub struct ModalState {
    pub visible: bool,
    /// Cómo entra: cubriendo todo o como hoja desde abajo.
    pub sheet: bool,
    presented: Option<Retained<UIViewController>>,
}

impl ModalState {
    /// Presenta o retira el modal según su estado. Idempotente.
    pub fn sync(
        &mut self,
        mtm: MainThreadMarker,
        container: &UIView,
        content: &UIView,
        node: NodeId,
        queue: &EventQueue,
    ) {
        if !self.visible {
            if let Some(controller) = self.presented.take() {
                let queue = queue.clone();
                unsafe { controller.dismissViewControllerAnimated_completion(true, None) };
                // Al retirarlo, la vista vuelve con los suyos: el core sigue
                // mandándole marcos y props, y si se quedara colgando del
                // controlador que se va, dejaría de verse al reabrirlo.
                push_event(
                    &queue,
                    HostEvent { target: node, name: "dismiss".to_owned(), payload: Vec::new() },
                );
            }
            return;
        }
        if self.presented.is_some() {
            return;
        }
        let Some(root) = container.window().and_then(|window| window.rootViewController()) else {
            return;
        };

        let controller = UIViewController::new(mtm);
        controller.setView(Some(content));
        // En tvOS no hay hoja: la presentación modal cubre la pantalla y ya.
        // Se dice una vez, porque un `[sheet]` que se ignora en silencio es
        // una plantilla que se ve distinta sin que nadie sepa por qué.
        #[cfg(target_os = "tvos")]
        let sheet = if self.sheet {
            crate::family::report(
                "<an-modal [sheet]>",
                "UISheetPresentationController no está en el SDK: el modal cubre la pantalla \
                 entera, que es como se presenta en una tele",
            );
            false
        } else {
            false
        };
        #[cfg(not(target_os = "tvos"))]
        let sheet = self.sheet;

        unsafe {
            if sheet {
                controller.setModalPresentationStyle(UIModalPresentationStyle::PageSheet);
                // Los topes son los del sistema: media pantalla y entera, con
                // su tirador y su gesto de bajar para cerrar.
                #[cfg(not(target_os = "tvos"))]
                if let Some(sheet) = controller.sheetPresentationController() {
                    let detents = objc2_foundation::NSArray::from_retained_slice(&[
                        UISheetPresentationControllerDetent::mediumDetent(mtm),
                        UISheetPresentationControllerDetent::largeDetent(mtm),
                    ]);
                    sheet.setDetents(&detents);
                    sheet.setPrefersGrabberVisible(true);
                }
            } else {
                // El core ya calculó el contenido a pantalla completa: con
                // `OverFullScreen` el controlador mide exactamente eso y no
                // hay que recolocar nada.
                controller.setModalPresentationStyle(UIModalPresentationStyle::OverFullScreen);
                controller.setModalTransitionStyle(UIModalTransitionStyle::CoverVertical);
            }
            root.presentViewController_animated_completion(&controller, true, None);
        }
        self.presented = Some(controller);
    }

    /// La vista del modal cuando no está presentado: sigue siendo hija del
    /// árbol, solo que escondida.
    pub fn presented(&self) -> bool {
        self.presented.is_some()
    }
}
