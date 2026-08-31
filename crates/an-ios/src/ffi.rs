//! Superficie C que consume el shell de Xcode. Cinco funciones: crear, cargar,
//! redimensionar, pintar un frame y destruir.
//!
//! Todas asumen hilo principal. `an_runtime_frame` es lo que llama el
//! `CADisplayLink`: en la mayoría de los frames no hay nada que aplicar y
//! devuelve 0 sin tocar UIKit.

use std::ffi::c_void;

use an_host::Renderer;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_ui_kit::UIView;

use crate::host::UikitHost;
use crate::measure::UikitMeasurer;

pub struct AnRuntime {
    renderer: Renderer<UikitHost, UikitMeasurer>,
}

/// # Safety
/// `container` debe ser un `UIView` vivo. Llamar desde el hilo principal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_new(
    container: *mut c_void,
    width: f32,
    height: f32,
) -> *mut AnRuntime {
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    if container.is_null() {
        return std::ptr::null_mut();
    }
    let container: Retained<UIView> = unsafe {
        Retained::retain(container.cast::<UIView>()).expect("container no puede ser nil")
    };
    let host = UikitHost::new(mtm, container);
    let renderer = Renderer::new(host, UikitMeasurer::new(), (width, height));
    Box::into_raw(Box::new(AnRuntime { renderer }))
}

/// Carga el árbol de demostración. Desaparece cuando exista el puente JS.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_load_demo(rt: *mut AnRuntime) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    let _ = crate::build_demo(&mut rt.renderer.tree);
}

/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_set_viewport(rt: *mut AnRuntime, width: f32, height: f32) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    rt.renderer.set_viewport((width, height));
}

/// Devuelve el número de operaciones aplicadas, o -1 si el commit falló.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_frame(rt: *mut AnRuntime) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };
    match rt.renderer.render_frame() {
        Ok(count) => count as i32,
        Err(_) => -1,
    }
}

/// # Safety
/// `rt` debe venir de `an_runtime_new` y no haberse liberado ya.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_free(rt: *mut AnRuntime) {
    if !rt.is_null() {
        drop(unsafe { Box::from_raw(rt) });
    }
}
