//! Puntos de entrada que llama Kotlin.
//!
//! El equivalente exacto de `an-ios/src/ffi.rs`: crear, evaluar, un frame,
//! evento, y destruir. La diferencia es que aquí el puntero al runtime viaja
//! como `long` porque es lo que la JVM sabe guardar.

use an_bridge::{apply, JsRuntime, QuickJsRuntime};
use an_core::PropValue;
use an_host::{new_event_queue, HostEvent, Renderer};
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jfloat, jint, jlong, jstring};
use jni::JNIEnv;

use crate::host::JniHost;
use crate::measure::JniMeasurer;

pub struct AndroidRuntime {
    renderer: Renderer<JniHost, JniMeasurer>,
    js: QuickJsRuntime,
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
    // Tres referencias globales al mismo objeto: el host, el medidor y el
    // módulo de dispositivo lo conservan cada uno por su cuenta, y una
    // referencia local moriría al volver de esta función.
    let (Ok(vm_host), Ok(vm_measure), Ok(vm_device)) =
        (env.get_java_vm(), env.get_java_vm(), env.get_java_vm())
    else {
        return 0;
    };
    let Ok(host_ref) = env.new_global_ref(&host) else { return 0 };
    let Ok(measure_ref) = env.new_global_ref(&host) else { return 0 };
    let Ok(device_ref) = env.new_global_ref(&host) else { return 0 };

    let events = new_event_queue();
    let renderer = Renderer::new(
        JniHost::new(vm_host, host_ref),
        JniMeasurer::new(vm_measure, measure_ref),
        (width, height),
        events,
    );
    let Ok(vm_log) = env.get_java_vm() else { return 0 };
    let Ok(log_ref) = env.new_global_ref(&host) else { return 0 };
    let sink = crate::logging::AndroidLog::new(vm_log, log_ref);
    crate::logging::redirect_stderr(sink.clone());

    let js = match QuickJsRuntime::with_log(sink) {
        Ok(mut js) => {
            js.register_module(Box::new(crate::modules::DeviceModule::new(vm_device, device_ref)));
            js
        }
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return 0;
        }
    };
    Box::into_raw(Box::new(AndroidRuntime { renderer, js })) as jlong
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
    let (Ok(name), Ok(code)) = (env.get_string(&name), env.get_string(&code)) else {
        return -1;
    };
    let (name, code): (String, String) = (name.into(), code.into());
    match runtime.js.eval(&name, &code) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("angular-native: {error}");
            -1
        }
    }
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
        runtime.renderer.set_viewport((width, height));
    }
}

/// Un frame: eventos hacia JS, turno de JS, mutaciones, layout y montaje.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeFrame(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    now_ms: f64,
) -> jint {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return -1 };

    let events = runtime.renderer.drain_events();
    if let Err(error) = runtime.js.dispatch_events(&events) {
        eprintln!("angular-native: {error}");
        return -1;
    }
    match runtime.js.tick(now_ms) {
        Ok(commands) if commands.is_empty() => {}
        Ok(commands) => {
            if let Err(error) = apply(&commands, &mut runtime.renderer.tree) {
                eprintln!("angular-native: búfer de comandos inválido: {error:?}");
                return -1;
            }
        }
        Err(error) => {
            eprintln!("angular-native: {error}");
            return -1;
        }
    }
    match runtime.renderer.render_frame() {
        Ok(count) => count as jint,
        Err(error) => {
            eprintln!("angular-native: el commit falló: {error:?}");
            -1
        }
    }
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
    runtime.renderer.push_event(HostEvent {
        target: target as u32,
        name,
        payload: vec![
            ("x".to_owned(), PropValue::Number(x as f64)),
            ("y".to_owned(), PropValue::Number(y as f64)),
        ],
    });
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
    let (Ok(name), Ok(value)) = (env.get_string(&name), env.get_string(&value)) else {
        return;
    };
    let (name, value): (String, String) = (name.into(), value.into());
    runtime.renderer.push_event(HostEvent {
        target: target as u32,
        name,
        payload: vec![("value".to_owned(), PropValue::Str(value))],
    });
}

/// Igual que en iOS: vistas fuera, motor nuevo, árbol vacío.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeReload(
    env: JNIEnv,
    class: JClass,
    handle: jlong,
    name: JString,
    code: JString,
) -> jint {
    let Some(runtime) = (unsafe { runtime(handle) }) else { return -1 };
    runtime.renderer.reset();
    match QuickJsRuntime::new() {
        Ok(js) => runtime.js = js,
        Err(error) => {
            eprintln!("angular-native: no arrancó el motor JS: {error}");
            return -1;
        }
    }
    Java_dev_angularnative_AnRuntime_nativeEval(env, class, handle, name, code)
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

/// Silencia el aviso de import sin usar cuando se compila sin todos los
/// puntos de entrada.
const _: Option<jstring> = None;
