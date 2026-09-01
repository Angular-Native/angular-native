//! Puntos de entrada que llama Kotlin.
//!
//! El equivalente exacto de `an-ios/src/ffi.rs`, con el mismo reparto: el hilo
//! de UI se queda con las vistas y el motor JS vive en un hilo con pila grande.
//! La diferencia es que aquí el puntero al runtime viaja como `long`, porque es
//! lo que la JVM sabe guardar.

use std::time::Duration;

use an_bridge::{QuickJsRuntime, Request, RuntimeWorker};
use an_core::PropValue;
use an_host::{drain_events, new_event_queue, EventQueue, HostEvent, MountSide, ShadowSide};
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jfloat, jint, jlong};
use jni::JNIEnv;

use crate::host::JniHost;
use crate::measure::JniMeasurer;

/// Mismo tamaño que en iOS y por el mismo motivo: el router de Angular
/// necesita algo más de 3 MB de pila para completar una navegación.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

/// Lo que el hilo de UI espera al motor dentro del frame, igual que en iOS.
const FRAME_BUDGET: Duration = Duration::from_millis(12);

pub struct AndroidRuntime {
    worker: RuntimeWorker,
    mount: MountSide<JniHost>,
    events: EventQueue,
}

impl AndroidRuntime {
    /// Monta lo que haya llegado del worker. No bloquea.
    fn pump(&mut self) -> jint {
        let mut applied = 0;
        while let Some(reply) = self.worker.try_reply() {
            applied = self.mount_reply(reply, applied);
        }
        applied
    }

    /// Aplica una respuesta y acumula el recuento.
    fn mount_reply(&mut self, reply: an_bridge::Reply, applied: jint) -> jint {
        let failed = reply.error.is_some();
        if let Some(error) = reply.error {
            eprintln!("angular-native: {error}");
        }
        let count = self.mount.apply(&reply.frame);
        if failed || applied < 0 {
            -1
        } else {
            applied + count as jint
        }
    }

    /// Vacía lo que quede en vuelo antes de una operación de control.
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
/// El puntero tiene que venir de `nativeNew` y no haberse liberado.
unsafe fn runtime<'a>(handle: jlong) -> Option<&'a mut AndroidRuntime> {
    if handle == 0 {
        return None;
    }
    Some(unsafe { &mut *(handle as *mut AndroidRuntime) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeNew(
    env: JNIEnv,
    _class: JClass,
    host: JObject,
    width: jfloat,
    height: jfloat,
) -> jlong {
    // Cuatro referencias globales al mismo objeto Java: el host se queda en el
    // hilo de UI y las otras tres viajan al del motor. Una referencia local
    // moriría al volver de esta función.
    let (Ok(vm_host), Ok(vm_measure), Ok(vm_device), Ok(vm_log)) = (
        env.get_java_vm(),
        env.get_java_vm(),
        env.get_java_vm(),
        env.get_java_vm(),
    ) else {
        return 0;
    };
    let (Ok(host_ref), Ok(measure_ref), Ok(device_ref), Ok(log_ref)) = (
        env.new_global_ref(&host),
        env.new_global_ref(&host),
        env.new_global_ref(&host),
        env.new_global_ref(&host),
    ) else {
        return 0;
    };

    // El redirigido de stderr es del proceso entero: se monta aquí una vez.
    crate::logging::redirect_stderr(crate::logging::AndroidLog::new(vm_log, log_ref));

    let events = new_event_queue();
    let mount = MountSide::new(JniHost::new(vm_host, host_ref));

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let sink = crate::logging::AndroidLog::new(vm_device, device_ref);
        let mut js = QuickJsRuntime::with_log(sink.clone())?;
        js.register_module(Box::new(crate::modules::DeviceModule::new(
            sink.java_vm(),
            sink.host_ref(),
        )));
        Ok((js, ShadowSide::new(JniMeasurer::new(vm_measure, measure_ref), (width, height))))
    });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return 0;
        }
    };

    Box::into_raw(Box::new(AndroidRuntime { worker, mount, events })) as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeEval(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
    code: JString,
) -> jint {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return -1 };
    let Some((name, code)) = read_pair(&mut env, name, code) else { return -1 };
    runtime.settle();
    report(runtime.worker.request(Request::Eval { name, code }).error)
}

/// Igual que en iOS: vistas fuera, motor nuevo, árbol vacío.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeReload(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
    code: JString,
) -> jint {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return -1 };
    let Some((name, code)) = read_pair(&mut env, name, code) else { return -1 };
    runtime.settle();
    // Las vistas se desmontan aquí, donde se puede tocar la jerarquía; el
    // árbol lo tira el worker.
    runtime.mount.clear();
    drain_events(&runtime.events);
    report(runtime.worker.request(Request::Reload { name, code }).error)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeSetViewport(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    width: jfloat,
    height: jfloat,
) {
    if let Some(runtime) = unsafe { runtime(handle) } {
        runtime.settle();
        runtime.worker.request(Request::SetViewport(width, height));
    }
}

/// Un frame: eventos hacia JS, turno de JS, layout, y montaje aquí.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeFrame(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    now_ms: f64,
) -> jint {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return -1 };

    // Se monta lo que el worker haya terminado desde el frame anterior, se le
    // manda el turno siguiente si no sigue ocupado, y se le espera lo que
    // queda de frame: si contesta a tiempo, lo que el usuario acaba de tocar
    // se ve en este mismo frame.
    let mut applied = runtime.pump();
    if !runtime.worker.busy() {
        let events = drain_events(&runtime.events);
        runtime.worker.post(Request::Tick { now_ms, events });
        if let Some(reply) = runtime.worker.wait_reply_until(FRAME_BUDGET) {
            applied = runtime.mount_reply(reply, applied);
        }
    }
    applied
}

/// Kotlin encola aquí lo que produce un `OnClickListener` o un scroll. El
/// evento no se despacha ahora: espera al frame siguiente, igual que en iOS.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeDispatchEvent(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    target: jint,
    name: JString,
    x: jfloat,
    y: jfloat,
) {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return };
    let Ok(name) = env.get_string(&name) else { return };
    let name: String = name.into();
    an_host::push_event(
        &runtime.events,
        HostEvent {
            target: target as u32,
            name,
            payload: vec![
                ("x".to_owned(), PropValue::Number(x as f64)),
                ("y".to_owned(), PropValue::Number(y as f64)),
            ],
        },
    );
}

/// Eventos que llevan texto en vez de coordenadas: escribir en un campo,
/// entrar y salir de él.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeDispatchValueEvent(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    target: jint,
    name: JString,
    value: JString,
) {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return };
    let Some((name, value)) = read_pair(&mut env, name, value) else { return };
    an_host::push_event(
        &runtime.events,
        HostEvent {
            target: target as u32,
            name,
            payload: vec![("value".to_owned(), PropValue::Str(value))],
        },
    );
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeFree(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as *mut AndroidRuntime) });
    }
}

fn read_pair(env: &mut JNIEnv, first: JString, second: JString) -> Option<(String, String)> {
    let first = env.get_string(&first).ok()?;
    let second = env.get_string(&second).ok()?;
    Some((first.into(), second.into()))
}

fn report(error: Option<String>) -> jint {
    match error {
        None => 0,
        Some(message) => {
            eprintln!("angular-native: {message}");
            -1
        }
    }
}
