//! Superficie C que consume el shell de Swift.
//!
//! El reparto es el mismo que en iOS: el hilo principal se queda con lo único
//! que no puede salir de él —las vistas— y el motor JS, el árbol y el layout
//! viven en un hilo aparte con pila grande. En macOS el hilo principal no tiene
//! el tope de 1 MB de iOS, pero el hilo propio se mantiene igual: no es solo
//! por la pila, es que el turno de JS y el montaje de vistas son dos trabajos
//! distintos y separarlos deja que el de UI siga respondiendo cuando el otro se
//! alarga.
//!
//! **Lo que sí cambia en escritorio: el viewport.** En un teléfono cambia al
//! rotar, y eso pasa una vez cada mucho. Aquí cambia mientras alguien arrastra
//! la esquina de la ventana, sesenta veces por segundo, y cada cambio es una
//! ida y vuelta al worker que además espera a que se vacíe lo que hubiera en
//! vuelo. Por eso `an_runtime_set_viewport` recuerda el último tamaño y
//! descarta los repetidos: el shell puede llamarla en cada `layout()` sin
//! pensárselo, que es lo que hace.

use std::ffi::{c_char, c_void, CStr};
use std::time::Duration;

use an_bridge::{QuickJsRuntime, Request, RuntimeWorker};
use an_host::{drain_events, new_event_queue, EventQueue, MountSide, ShadowSide};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSView;

/// 8 MB, lo mismo que en iOS: el router de Angular necesita algo más de 3 MB
/// para completar una navegación y conviene margen.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

/// Lo que el hilo de UI espera al motor dentro del frame. Doce milisegundos
/// dejan margen sobre los 16,6 de un frame a 60 Hz para montar las vistas
/// después.
///
/// En un Mac la pantalla puede ir a 120 Hz, y entonces el frame dura 8,3 ms y
/// este plazo se pasa. No se baja a propósito: pasarse del plazo no pierde el
/// trabajo, solo lo monta en el frame siguiente, y bajarlo haría que un turno
/// normal de Angular se montara siempre un frame tarde en las pantallas de 60.
const FRAME_BUDGET: Duration = Duration::from_millis(12);

pub struct AnRuntime {
    worker: RuntimeWorker,
    mount: MountSide<crate::host::AppKitHost>,
    events: EventQueue,
    /// Último viewport que se le mandó al worker. Ver la cabecera.
    viewport: (f32, f32),
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

    /// Aplica una respuesta y acumula el recuento. Un -1 se pega: si algo falló
    /// en el frame, el frame falló.
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

/// Deja los pánicos en el log del sistema antes de que se pierdan.
///
/// Un pánico dentro de un `extern "C"` no puede desenrollar, así que Rust
/// aborta con «panic in a function that cannot unwind» y el mensaje de verdad
/// se pierde. Aquí se escribe y se vacía a mano, que es la diferencia entre
/// depurar un cierre y adivinarlo.
fn report_panics() {
    use std::sync::Once;
    static UNA_VEZ: Once = Once::new();
    UNA_VEZ.call_once(|| {
        let anterior = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let donde = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "sitio desconocido".to_owned());
            use std::io::Write;
            let mut salida = std::io::stderr().lock();
            let _ = writeln!(salida, "angular-native: pánico en {donde}: {info}");
            let _ = salida.flush();
            anterior(info);
        }));
    });
}

/// # Safety
/// `container` debe ser una `NSView` viva. Llamar desde el hilo principal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_new(
    container: *mut c_void,
    width: f32,
    height: f32,
) -> *mut AnRuntime {
    report_panics();
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    if container.is_null() {
        return std::ptr::null_mut();
    }
    let container: Retained<NSView> = unsafe {
        Retained::retain(container.cast::<NSView>()).expect("container no puede ser nil")
    };

    let events = new_event_queue();
    let host = crate::host::AppKitHost::new(mtm, container, events.clone());
    // Los controles del sistema se miden aquí, en el hilo principal: crear un
    // `NSSwitch` fuera de él no está permitido.
    let control_sizes = crate::controls::measure_controls(mtm);

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let js = QuickJsRuntime::new()?;
        Ok((
            js,
            ShadowSide::new(crate::measure::AppKitMeasurer::new(control_sizes), (width, height)),
        ))
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
        viewport: (width, height),
    }))
}

/// Evalúa un script. El bundle de la app es quien decide qué cargar.
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
/// todo otra vez.
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
    // Desmontar va después de saber en qué acabó, y solo si **no** fue en
    // caliente. En caliente el árbol sigue en pie: tirar las vistas dejaría la
    // ventana en blanco esperando unas altas que el core no tiene por qué
    // volver a mandar. El worker solo tira el árbol cuando reinicia, y es
    // entonces cuando toca vaciar esto —aquí, que es donde se puede tocar
    // AppKit—.
    if !reply.hot {
        rt.mount.clear();
    }
    report(reply.error)
}

/// La ventana cambió de tamaño. En escritorio esto pasa en caliente y muy a
/// menudo, así que se descarta lo que no cambia: ver la cabecera del módulo.
///
/// # Safety
/// `rt` debe venir de `an_runtime_new` y seguir vivo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_set_viewport(rt: *mut AnRuntime, width: f32, height: f32) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    if rt.viewport == (width, height) {
        return;
    }
    rt.viewport = (width, height);
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
        if let Some(reply) = rt.worker.wait_reply_until(FRAME_BUDGET) {
            applied = rt.mount_reply(reply, applied);
        }
    }
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
