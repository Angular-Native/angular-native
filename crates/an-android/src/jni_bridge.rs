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
use jni::objects::{JClass, JFloatArray, JObject, JString};
use jni::sys::{jfloat, jint, jlong};
use jni::errors::LogErrorAndDefault;
use jni::EnvUnowned;

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
    mut env: EnvUnowned,
    _class: JClass,
    host: JObject,
    width: jfloat,
    height: jfloat,
) -> jlong {
    env.with_env(|env| -> Result<jlong, jni::errors::Error> {
        // Cinco referencias globales al mismo objeto Java: el host se queda en
        // el hilo de UI y las otras cuatro viajan al del motor. Una referencia
        // local moriría al volver de esta función, y desde jni 0.22 una global
        // no se puede duplicar sin el entorno, así que salen todas de aquí.
        let (Ok(vm_host), Ok(vm_measure), Ok(vm_device), Ok(vm_module), Ok(vm_log)) = (
            env.get_java_vm(),
            env.get_java_vm(),
            env.get_java_vm(),
            env.get_java_vm(),
            env.get_java_vm(),
        ) else {
            return Ok(0);
        };
        let (Ok(host_ref), Ok(measure_ref), Ok(device_ref), Ok(module_ref), Ok(log_ref)) = (
            env.new_global_ref(&host),
            env.new_global_ref(&host),
            env.new_global_ref(&host),
            env.new_global_ref(&host),
            env.new_global_ref(&host),
        ) else {
            return Ok(0);
        };

        // El redirigido de stderr es del proceso entero: se monta aquí una vez.
        crate::logging::redirect_stderr(crate::logging::AndroidLog::new(vm_log, log_ref));

        let events = new_event_queue();
        let mount = MountSide::new(JniHost::new(vm_host, host_ref));

        let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
            let sink = crate::logging::AndroidLog::new(vm_device, device_ref);
            let mut js = QuickJsRuntime::with_log(sink.clone())?;
            js.register_module(Box::new(crate::modules::DeviceModule::new(
                vm_module,
                module_ref,
            )));
            Ok((js, ShadowSide::new(JniMeasurer::new(vm_measure, measure_ref), (width, height))))
        });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                eprintln!("angular-native: no arrancó el motor JS: {error}");
                return Ok(0);
            }
        };

        Ok(Box::into_raw(Box::new(AndroidRuntime { worker, mount, events })) as jlong)
    })
    .resolve::<LogErrorAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeEval(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    name: JString,
    code: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(-1) };
        let Some((name, code)) = read_pair(env, name, code) else { return Ok(-1) };
        runtime.settle();
        Ok(report(runtime.worker.request(Request::Eval { name, code }).error))
    })
    .resolve::<LogErrorAndDefault>()
}

/// Igual que en iOS: en caliente solo cambian las definiciones; si no encaja,
/// vistas fuera, motor nuevo y árbol vacío.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeReload(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    name: JString,
    code: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(-1) };
        let Some((name, code)) = read_pair(env, name, code) else { return Ok(-1) };
        runtime.settle();
        drain_events(&runtime.events);
        let reply = runtime.worker.request(Request::Reload { name, code });
        // Solo se desmonta si hubo reinicio: en caliente el árbol sigue en pie
        // y tirar las vistas dejaría la pantalla vacía.
        if !reply.hot {
            runtime.mount.clear();
        }
        Ok(report(reply.error))
    })
    .resolve::<LogErrorAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeSetViewport(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    width: jfloat,
    height: jfloat,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Some(runtime) = unsafe { runtime(handle) } {
            runtime.settle();
            runtime.worker.request(Request::SetViewport(width, height));
        }
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Un frame: eventos hacia JS, turno de JS, layout, y montaje aquí.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeFrame(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    now_ms: f64,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(-1) };

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
        Ok(applied)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Kotlin encola aquí lo que produce un `OnClickListener` o un scroll. El
/// evento no se despacha ahora: espera al frame siguiente, igual que en iOS.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeDispatchEvent(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    target: jint,
    name: JString,
    x: jfloat,
    y: jfloat,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(()) };
        let Ok(name) = env.get_string(&name) else { return Ok(()) };
        let name: String = name.into();
        // `load` lleva un tamaño, no una posición: mismas dos cifras, otros
        // nombres, y tienen que ser los mismos que manda iOS.
        let (first, second) = if name == "load" { ("width", "height") } else { ("x", "y") };
        an_host::push_event(
            &runtime.events,
            HostEvent {
                target: target as u32,
                name,
                payload: vec![
                    (first.to_owned(), PropValue::Number(x as f64)),
                    (second.to_owned(), PropValue::Number(y as f64)),
                ],
            },
        );
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Gestos: arrastrar, pellizcar, girar, mantener pulsado, deslizar.
///
/// Los nombres de los campos vienen del lado de Java en vez de estar fijados
/// aquí por posición. Cuesta separar una cadena por comas, pero si un día uno
/// de los dos lados añade un campo, el otro no empieza a leer números
/// corridos de sitio.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeDispatchGesture(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    target: jint,
    name: JString,
    state: JString,
    keys: JString,
    values: JFloatArray,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(()) };
        let (Ok(name), Ok(state), Ok(keys)) = (
            env.get_string(&name),
            env.get_string(&state),
            env.get_string(&keys),
        ) else {
            return Ok(());
        };
        let name: String = name.into();
        let state: String = state.into();
        let keys: String = keys.into();

        let Ok(length) = env.get_array_length(&values) else { return Ok(()) };
        let mut numbers = vec![0f32; length as usize];
        if env.get_float_array_region(&values, 0, &mut numbers).is_err() {
            return Ok(());
        }

        let mut payload: Vec<(String, PropValue)> = keys
            .split(',')
            .zip(numbers.iter())
            .map(|(key, value)| (key.to_owned(), PropValue::Number(*value as f64)))
            .collect();
        if !state.is_empty() {
            payload.push(("state".to_owned(), PropValue::Str(state)));
        }

        an_host::push_event(&runtime.events, HostEvent { target: target as u32, name, payload });
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Eventos que llevan un índice: la pestaña elegida, por ejemplo.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeDispatchIndexEvent(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    target: jint,
    name: JString,
    index: jint,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(()) };
        let Ok(name) = env.get_string(&name) else { return Ok(()) };
        let name: String = name.into();
        an_host::push_event(
            &runtime.events,
            HostEvent {
                target: target as u32,
                name,
                payload: vec![("index".to_owned(), PropValue::Number(index as f64))],
            },
        );
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// Eventos que llevan texto en vez de coordenadas: escribir en un campo,
/// entrar y salir de él.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeDispatchValueEvent(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    target: jint,
    name: JString,
    value: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(()) };
        let Some((name, value)) = read_pair(env, name, value) else { return Ok(()) };
        // El área segura son cuatro cifras y viaja como JSON: se desempaqueta aquí
        // para que el evento llegue igual que el de iOS.
        let payload = if name == "safeArea" {
            parse_insets(&value)
        } else {
            vec![("value".to_owned(), PropValue::Str(value))]
        };
        an_host::push_event(
            &runtime.events,
            HostEvent { target: target as u32, name, payload },
        );
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeFree(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        if handle != 0 {
            drop(unsafe { Box::from_raw(handle as *mut AndroidRuntime) });
        }
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}

/// `{"top":1,"right":2,...}` a pares. Sin analizador de JSON: son cuatro
/// números con nombres conocidos.
fn parse_insets(raw: &str) -> Vec<(String, PropValue)> {
    ["top", "right", "bottom", "left"]
        .into_iter()
        .map(|key| {
            let needle = format!("\"{key}\":");
            let value = raw
                .find(&needle)
                .map(|at| &raw[at + needle.len()..])
                .and_then(|rest| {
                    let end = rest.find(['}', ',']).unwrap_or(rest.len());
                    rest[..end].trim().parse::<f64>().ok()
                })
                .unwrap_or(0.0);
            (key.to_owned(), PropValue::Number(value))
        })
        .collect()
}

fn read_pair(env: &mut jni::Env, first: JString, second: JString) -> Option<(String, String)> {
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
