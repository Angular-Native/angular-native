//! Superficie C que consume el shell de Xcode. Cinco funciones: crear, cargar,
//! redimensionar, pintar un frame y destruir.
//!
//! Todas asumen hilo principal. `an_runtime_frame` es lo que llama el
//! `CADisplayLink`: en la mayoría de los frames no hay nada que aplicar y
//! devuelve 0 sin tocar UIKit.

use std::ffi::{c_char, c_void, CStr};

use an_bridge::{apply, JsRuntime, QuickJsRuntime};
use an_host::{new_event_queue, Renderer};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_ui_kit::UIView;

use crate::host::UikitHost;
use crate::measure::UikitMeasurer;

pub struct AnRuntime {
    renderer: Renderer<UikitHost, UikitMeasurer>,
    js: QuickJsRuntime,
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
    // Host y renderer comparten la cola: el primero empuja desde los callbacks
    // de UIKit, el segundo la vacía al empezar cada frame.
    let events = new_event_queue();
    let host = UikitHost::new(mtm, container, events.clone());
    let renderer = Renderer::new(host, UikitMeasurer::new(), (width, height), events);
    let js = match QuickJsRuntime::new() {
        Ok(js) => js,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return std::ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(AnRuntime { renderer, js }))
}

/// Evalúa un script. El bundle de la app es quien decide qué cargar, igual
/// que el `main.jsbundle` de React Native.
///
/// Devuelve 0 si fue bien y -1 si JS lanzó; el error sale por stderr con su
/// traza.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new`. `name` y `code` deben ser cadenas C
/// válidas y terminadas en cero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_eval(
    rt: *mut AnRuntime,
    name: *const c_char,
    code: *const c_char,
) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };
    if name.is_null() || code.is_null() {
        return -1;
    }
    let (Ok(name), Ok(code)) = (
        unsafe { CStr::from_ptr(name) }.to_str(),
        unsafe { CStr::from_ptr(code) }.to_str(),
    ) else {
        eprintln!("angular-native: el script no es UTF-8 válido");
        return -1;
    };
    match rt.js.eval(name, code) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("angular-native: {error}");
            -1
        }
    }
}

/// Tira la app y la levanta otra vez con código nuevo: vistas nativas fuera,
/// motor JS nuevo, árbol vacío. Es lo que usa `an dev` al detectar un cambio.
///
/// No conserva estado: un `signal` vuelve a su valor inicial. Preservarlo es
/// otro problema, y bastante más grande.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new`. `name` y `code`, cadenas C válidas.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_reload(
    rt: *mut AnRuntime,
    name: *const c_char,
    code: *const c_char,
) -> i32 {
    let Some(runtime) = (unsafe { rt.as_mut() }) else { return -1 };
    runtime.renderer.reset();
    match QuickJsRuntime::new() {
        Ok(js) => runtime.js = js,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return -1;
        }
    }
    unsafe { an_runtime_eval(rt, name, code) }
}

/// Árbol de demostración construido desde Rust, sin pasar por JS. Sigue aquí
/// porque permite aislar un fallo: si esto se ve y el script no, el problema
/// está en el puente, no en el renderer.
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

/// Un frame completo, en este orden: eventos nativos hacia JS, turno de JS
/// (temporizadores y microtareas), mutaciones aplicadas al árbol, layout y
/// montaje. `now_ms` es la marca de tiempo del `CADisplayLink`, que es el
/// único reloj que ve la app.
///
/// Devuelve el número de operaciones nativas aplicadas, o -1 si algo falló.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_frame(rt: *mut AnRuntime, now_ms: f64) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };

    let events = rt.renderer.drain_events();
    if let Err(error) = rt.js.dispatch_events(&events) {
        eprintln!("angular-native: {error}");
        return -1;
    }
    match rt.js.tick(now_ms) {
        Ok(commands) if commands.is_empty() => {}
        Ok(commands) => {
            if let Err(error) = apply(&commands, &mut rt.renderer.tree) {
                eprintln!("angular-native: búfer de comandos inválido: {error:?}");
                return -1;
            }
        }
        Err(error) => {
            eprintln!("angular-native: {error}");
            return -1;
        }
    }
    match rt.renderer.render_frame() {
        Ok(count) => count as i32,
        Err(error) => {
            eprintln!("angular-native: el commit falló: {error:?}");
            -1
        }
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
