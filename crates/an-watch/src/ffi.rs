//! Superficie C que consume el shell de SwiftUI.
//!
//! Es casi la misma que la de iOS —crear, evaluar, un frame, liberar— con dos
//! diferencias que vienen de que aquí no hay vistas:
//!
//! - `an_watch_runtime_new` no recibe ningún contenedor. En iOS se le pasa el
//!   `UIView` del que colgar; aquí no hay nada de lo que colgar, porque el
//!   árbol vive en Rust y SwiftUI lo lee.
//! - Aparece `an_watch_runtime_snapshot`, que es cómo el shell se entera de qué
//!   pintar. En iOS no hace falta: el host ya ha tocado las vistas cuando
//!   `an_runtime_frame` vuelve.
//!
//! El motor JS sigue en su propio hilo y por la misma razón que en iOS: QuickJS
//! necesita unos 4 MB de pila para que el router de Angular navegue, y el hilo
//! principal no los tiene.

use std::ffi::{c_char, CStr, CString};
use std::time::Duration;

use an_bridge::{QuickJsRuntime, Request, RuntimeWorker};
use an_host::{drain_events, new_event_queue, EventQueue, MountSide, ShadowSide};

use crate::host::WatchHost;
use crate::measure::{ControlSizes, WatchMeasurer};

/// 8 MB, como en iOS. Medido allí: el router de Angular necesita algo más de
/// 3 MB para completar una navegación.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

/// Lo que el hilo de UI espera al motor dentro del frame.
///
/// En el reloj el frame no son 16,6 ms: watchOS dibuja a 30 Hz mientras la app
/// está en primer plano, así que hay 33 y se puede ser algo más paciente. Se
/// dejan 8 ms de margen para que SwiftUI recomponga después.
const FRAME_BUDGET: Duration = Duration::from_millis(25);

pub struct AnWatchRuntime {
    worker: RuntimeWorker,
    mount: MountSide<WatchHost>,
    events: EventQueue,
    /// La última foto entregada. Se guarda para que el `CString` siga vivo
    /// mientras Swift lo lee: devolver un puntero a un temporal sería un
    /// puntero colgante en cuanto la función volviera.
    last_json: Option<CString>,
}

impl AnWatchRuntime {
    fn pump(&mut self) -> i32 {
        let mut applied = 0;
        while let Some(reply) = self.worker.try_reply() {
            applied = self.mount_reply(reply, applied);
        }
        applied
    }

    /// Un -1 se pega: si algo falló en el frame, el frame falló.
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
    /// dejar el canal limpio, o la respuesta que se recoja será de otra.
    fn settle(&mut self) {
        while let Some(reply) = self.worker.wait_reply() {
            if let Some(error) = reply.error {
                eprintln!("angular-native: {error}");
            }
            self.mount.apply(&reply.frame);
        }
    }
}

/// Arranca el runtime.
///
/// `control_json` son los tamaños naturales de los controles, que en el reloj
/// los mide SwiftUI y no se pueden preguntar desde aquí. Formato:
/// `{"Button":[80,44]}`. Puede ser nulo: entonces los controles miden cero y
/// el layout los colapsa, que es visible y por tanto depurable.
///
/// # Safety
/// `control_json`, si no es nulo, debe ser una cadena C válida.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_new(
    width: f32,
    height: f32,
    control_json: *const c_char,
) -> *mut AnWatchRuntime {
    let controls = unsafe { read_controls(control_json) };
    let events = new_event_queue();
    let host = WatchHost::new(events.clone());

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let js = QuickJsRuntime::new()?;
        Ok((js, ShadowSide::new(WatchMeasurer::new(controls), (width, height))))
    });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(AnWatchRuntime {
        worker,
        mount: MountSide::new(host),
        events,
        last_json: None,
    }))
}

/// Evalúa un script: el bundle de la app.
///
/// # Safety
/// `rt` debe venir de `an_watch_runtime_new`; `name` y `code`, cadenas C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_eval(
    rt: *mut AnWatchRuntime,
    name: *const c_char,
    code: *const c_char,
) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };
    let Some((name, code)) = (unsafe { read_pair(name, code) }) else { return -1 };
    rt.settle();
    report(rt.worker.request(Request::Eval { name, code }).error)
}

/// Mete código nuevo en la app que ya está corriendo. Es lo que usa `an dev`.
///
/// Igual que en iOS: si el bundle nuevo encaja con lo que hay montado, solo
/// cambian las definiciones de los componentes y el estado se conserva; si no,
/// se levanta todo otra vez.
///
/// # Safety
/// `rt` debe venir de `an_watch_runtime_new`; `name` y `code`, cadenas C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_reload(
    rt: *mut AnWatchRuntime,
    name: *const c_char,
    code: *const c_char,
) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };
    let Some((name, code)) = (unsafe { read_pair(name, code) }) else { return -1 };
    rt.settle();
    drain_events(&rt.events);
    let reply = rt.worker.request(Request::Reload { name, code });
    // El modelo solo se vacía si hubo reinicio: en caliente el árbol sigue en
    // pie, y tirarlo dejaría la pantalla vacía esperando unas altas que el
    // núcleo no tiene por qué volver a mandar.
    if !reply.hot {
        rt.mount.clear();
    }
    report(reply.error)
}

/// # Safety
/// `rt` debe venir de `an_watch_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_set_viewport(
    rt: *mut AnWatchRuntime,
    width: f32,
    height: f32,
) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    rt.settle();
    rt.worker.request(Request::SetViewport(width, height));
}

/// Un frame completo: eventos hacia JS, turno de JS, layout, y las `MountOp`
/// aplicadas sobre el modelo. No pinta nada: de eso se encarga SwiftUI cuando
/// lea la foto.
///
/// Devuelve las operaciones aplicadas, o -1 si algo falló.
///
/// # Safety
/// `rt` debe venir de `an_watch_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_frame(rt: *mut AnWatchRuntime, now_ms: f64) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };

    let mut applied = rt.pump();
    // Si el worker sigue ocupado no se le encola otro turno: la cola crecería
    // sin fin y cada frame montado sería más viejo que el anterior.
    if !rt.worker.busy() {
        let events = drain_events(&rt.events);
        rt.worker.post(Request::Tick { now_ms, events });
        if let Some(reply) = rt.worker.wait_reply_until(FRAME_BUDGET) {
            applied = rt.mount_reply(reply, applied);
        }
    }
    applied
}

/// Número de revisión del modelo. Sube solo cuando un frame trajo cambios.
///
/// El shell lo compara con el que ya tiene y solo pide la foto si difiere: un
/// frame quieto no serializa nada ni despierta a SwiftUI.
///
/// # Safety
/// `rt` debe venir de `an_watch_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_revision(rt: *mut AnWatchRuntime) -> u64 {
    let Some(rt) = (unsafe { rt.as_ref() }) else { return 0 };
    rt.mount.host().revision()
}

/// El árbol entero en JSON. El puntero es válido hasta la siguiente llamada a
/// esta misma función o hasta `an_watch_runtime_free`: Swift lo copia a un
/// `String` en el acto y no lo guarda.
///
/// # Safety
/// `rt` debe venir de `an_watch_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_snapshot(rt: *mut AnWatchRuntime) -> *const c_char {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return std::ptr::null() };
    rt.mount.host_mut().take_dirty();
    let snapshot = crate::snapshot::snapshot(rt.mount.host());
    let json = match serde_json::to_string(&snapshot) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("angular-native: no se pudo serializar el árbol: {error}");
            return std::ptr::null();
        }
    };
    // Un `\0` dentro del JSON lo haría imposible de pasar como cadena C. No
    // debería llegar ninguno, pero si llega es mejor no pintar que corromper.
    let Ok(cstring) = CString::new(json) else {
        eprintln!("angular-native: el árbol traía un cero dentro");
        return std::ptr::null();
    };
    let pointer = cstring.as_ptr();
    rt.last_json = Some(cstring);
    pointer
}

/// Un evento nativo desde SwiftUI. Solo se encola si la plantilla registró ese
/// oyente sobre ese nodo.
///
/// # Safety
/// `rt` debe venir de `an_watch_runtime_new`; `name`, una cadena C válida.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_event(
    rt: *mut AnWatchRuntime,
    target: u32,
    name: *const c_char,
) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    if name.is_null() {
        return;
    }
    let Ok(name) = (unsafe { CStr::from_ptr(name) }).to_str() else { return };
    // Un `press` no lleva carga: en el reloj no hay coordenadas que valga la
    // pena mandar, porque el dedo tapa el sitio donde tocó.
    rt.mount.host().dispatch(target, name, Vec::new());
}

/// # Safety
/// `rt` debe venir de `an_watch_runtime_new` y no haberse liberado ya.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_free(rt: *mut AnWatchRuntime) {
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

/// # Safety
/// `raw`, si no es nulo, tiene que ser una cadena C válida.
unsafe fn read_controls(raw: *const c_char) -> ControlSizes {
    if raw.is_null() {
        return ControlSizes::new();
    }
    let Ok(text) = (unsafe { CStr::from_ptr(raw) }).to_str() else {
        return ControlSizes::new();
    };
    serde_json::from_str::<ControlSizes>(text).unwrap_or_else(|error| {
        eprintln!("angular-native: tamaños de control ilegibles: {error}");
        ControlSizes::new()
    })
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
