//! Superficie C que consume el shell de Xcode.
//!
//! El hilo principal se queda con lo único que no puede salir de él: las
//! vistas. El motor JS, el árbol y el layout viven en un hilo aparte con pila
//! grande, porque el principal de iOS tiene 1 MB y QuickJS necesita cuatro
//! veces eso para que Angular navegue.
//!
//! El `Tick` de cada frame se espera, pero con plazo: si el turno de JS cabe
//! en lo que queda de frame se monta en el mismo frame, y si se pasa, el hilo
//! de UI sigue y lo monta cuando llegue.

use std::ffi::{c_char, c_void, CStr};

use std::time::Duration;

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

/// Lo que el hilo de UI espera al motor dentro del frame. Doce milisegundos
/// dejan margen sobre los 16,6 de un frame a 60 Hz para montar las vistas
/// después. Un turno normal de Angular tarda mucho menos.
const FRAME_BUDGET: Duration = Duration::from_millis(12);

pub struct AnRuntime {
    worker: RuntimeWorker,
    mount: MountSide<UikitHost>,
    events: EventQueue,
    /// Operaciones montadas en el último frame que sí trajo trabajo.
    last_applied: i32,
}

impl AnRuntime {
    /// Monta lo que haya llegado del worker. No bloquea.
    fn pump(&mut self) -> i32 {
        let mut applied = 0;
        while let Some(reply) = self.worker.try_reply() {
            applied = self.mount_reply(reply, applied);
        }
        applied
    }

    /// Aplica una respuesta y acumula el recuento. Un -1 se pega: si algo
    /// falló en el frame, el frame falló.
    fn mount_reply(&mut self, reply: an_bridge::Reply, applied: i32) -> i32 {
        let failed = reply.error.is_some();
        if let Some(error) = reply.error {
            eprintln!("angular-native: {error}");
        }
        let count = self.mount.apply(&reply.frame);
        if failed || applied < 0 {
            -1
        } else {
            applied + count as i32
        }
    }

    /// Vacía lo que quede en vuelo. Antes de una operación de control hay que
    /// dejar el canal limpio, o la respuesta que se recoja será de otro.
    fn settle(&mut self) {
        while let Some(reply) = self.worker.wait_reply() {
            if let Some(error) = reply.error {
                eprintln!("angular-native: {error}");
            }
            self.mount.apply(&reply.frame);
        }
    }
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
    // Los controles del sistema se miden aquí, en el hilo principal: crear un
    // UISwitch fuera de él no está permitido.
    let control_sizes = crate::controls::measure_controls(mtm);

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let mut js = QuickJsRuntime::new()?;
        js.register_module(Box::new(device));
        Ok((js, ShadowSide::new(UikitMeasurer::new(control_sizes), (width, height))))
    });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(AnRuntime {
        worker,
        mount: MountSide::new(host),
        events,
        last_applied: 0,
    }))
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
    rt.settle();
    report(rt.worker.request(Request::Eval { name, code }).error)
}

/// Mete código nuevo en la app que está corriendo. Es lo que usa `an dev` al
/// detectar un cambio.
///
/// Si el bundle nuevo encaja con lo que hay montado, solo cambian las
/// definiciones de los componentes y el estado se conserva. Si no, se levanta
/// todo otra vez: vistas nativas fuera, motor JS nuevo, árbol vacío, y un
/// `signal` vuelve a su valor inicial.
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
    rt.settle();
    drain_events(&rt.events);
    let reply = rt.worker.request(Request::Reload { name, code });
    // Desmontar va después de saber en qué acabó: en caliente el árbol sigue
    // en pie, y tirar las vistas dejaría la pantalla en negro esperando unas
    // altas que el core no tiene por qué volver a mandar. El worker solo tira
    // el árbol cuando reinicia, y es entonces cuando toca vaciar esto —aquí,
    // que es donde se puede tocar UIKit.
    if !reply.hot {
        rt.mount.clear();
    }
    report(reply.error)
}

/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_set_viewport(rt: *mut AnRuntime, width: f32, height: f32) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    rt.settle();
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

    // Se monta lo que el worker haya terminado desde el frame anterior.
    let mut applied = rt.pump();

    // Si sigue ocupado no se le encola otro turno: la cola crecería sin fin y
    // cada frame montado sería más viejo que el anterior.
    if !rt.worker.busy() {
        let events = drain_events(&rt.events);
        rt.worker.post(Request::Tick { now_ms, events });
        // Y se le espera lo que queda de frame. Si contesta a tiempo, lo que
        // el usuario acaba de tocar se ve en este mismo frame.
        if let Some(reply) = rt.worker.wait_reply_until(FRAME_BUDGET) {
            applied = rt.mount_reply(reply, applied);
        }
    }
    rt.last_applied = applied;
    applied
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
