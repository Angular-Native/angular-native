//! Superficie C que consume el shell de Xcode.
//!
//! El hilo principal se queda con lo único que no puede salir de él: las
//! vistas. El motor JS, el árbol y el layout viven en un hilo aparte con pila
//! grande, porque el principal de iOS tiene 1 MB y QuickJS necesita cuatro
//! veces eso para que Angular navegue.
//!
//! Cada frame el hilo de UI pide y espera. Bloquearse no es peor que antes
//! —todo esto corría aquí mismo— y a cambio deja de haber un techo de pila.

use std::ffi::{c_char, c_void, CStr};

use an_bridge::{QuickJsRuntime, Request, RuntimeWorker};
use an_host::{drain_events, new_event_queue, EventQueue, MountSide, ShadowSide};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_ui_kit::UIView;

use crate::host::UikitHost;
use crate::measure::UikitMeasurer;

/// 8 MB. Medido: el router de Angular necesita algo más de 3 MB para completar
/// una navegación, y con 2 MB la transición avanza siete eventos y se para.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

pub struct AnRuntime {
    worker: RuntimeWorker,
    mount: MountSide<UikitHost>,
    events: EventQueue,
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

    let events = new_event_queue();
    let host = UikitHost::new(mtm, container, events.clone());
    // Los datos del dispositivo se leen aquí, en el hilo principal, y viajan
    // ya resueltos: UIDevice y UIScreen no se pueden tocar desde el worker.
    let device = crate::modules::DeviceModule::capture(mtm);

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let mut js = QuickJsRuntime::new()?;
        js.register_module(Box::new(device));
        Ok((js, ShadowSide::new(UikitMeasurer::new(), (width, height))))
    });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(AnRuntime { worker, mount: MountSide::new(host), events }))
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
    let Some((name, code)) = (unsafe { read_pair(name, code) }) else { return -1 };
    report(rt.worker.request(Request::Eval { name, code }).error)
}

/// Tira la app y la levanta otra vez con código nuevo: vistas nativas fuera,
/// motor JS nuevo, árbol vacío. Es lo que usa `an dev` al detectar un cambio.
///
/// No conserva estado: un `signal` vuelve a su valor inicial.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new`. `name` y `code`, cadenas C válidas.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_reload(
    rt: *mut AnRuntime,
    name: *const c_char,
    code: *const c_char,
) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };
    let Some((name, code)) = (unsafe { read_pair(name, code) }) else { return -1 };
    // Las vistas se desmontan aquí, donde se puede tocar UIKit; el árbol lo
    // tira el worker.
    rt.mount.clear();
    drain_events(&rt.events);
    report(rt.worker.request(Request::Reload { name, code }).error)
}

/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_set_viewport(rt: *mut AnRuntime, width: f32, height: f32) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    rt.worker.request(Request::SetViewport(width, height));
}

/// Un frame completo: eventos nativos hacia JS, turno de JS, layout, y montaje
/// aquí. `now_ms` es la marca de tiempo del `CADisplayLink`, que es el único
/// reloj que ve la app.
///
/// Devuelve el número de operaciones nativas aplicadas, o -1 si algo falló.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_frame(rt: *mut AnRuntime, now_ms: f64) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };

    let events = drain_events(&rt.events);
    let reply = rt.worker.request(Request::Tick { now_ms, events });
    let failed = reply.error.is_some();
    if let Some(error) = reply.error {
        eprintln!("angular-native: {error}");
    }
    let applied = rt.mount.apply(&reply.frame);
    if failed {
        -1
    } else {
        applied as i32
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

/// # Safety
/// Ambos punteros tienen que ser cadenas C válidas o nulos.
unsafe fn read_pair(name: *const c_char, code: *const c_char) -> Option<(String, String)> {
    if name.is_null() || code.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(name) }.to_str().ok()?;
    let code = unsafe { CStr::from_ptr(code) }.to_str().ok()?;
    Some((name.to_owned(), code.to_owned()))
}

fn report(error: Option<String>) -> i32 {
    match error {
        None => 0,
        Some(message) => {
            eprintln!("angular-native: {message}");
            -1
        }
    }
}
