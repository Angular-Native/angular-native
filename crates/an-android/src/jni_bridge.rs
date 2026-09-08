//! The entry points Kotlin calls.
//!
//! The exact counterpart of `an-ios/src/ffi.rs`, with the same split: the UI
//! thread keeps the views and the JS engine lives on a thread with a large
//! stack. The difference is that here the pointer to the runtime travels as a
//! `long`, because that is what the JVM knows how to store.

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

/// The same size as on iOS and for the same reason: Angular's router needs a
/// little over 3 MB of stack to complete a navigation.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

/// How long the UI thread waits for the engine inside the frame, as on iOS.
const FRAME_BUDGET: Duration = Duration::from_millis(12);

pub struct AndroidRuntime {
    worker: RuntimeWorker,
    mount: MountSide<JniHost>,
    events: EventQueue,
}

impl AndroidRuntime {
    /// Mounts whatever has arrived from the worker. Does not block.
    fn pump(&mut self) -> jint {
        let mut applied = 0;
        while let Some(reply) = self.worker.try_reply() {
            applied = self.mount_reply(reply, applied);
        }
        applied
    }

    /// Applies one reply and accumulates the count.
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

    /// Drains whatever is still in flight before a control operation.
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
/// The pointer has to come from `nativeNew` and must not have been freed.
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
        // Five global references to the same Java object: the host stays on the
        // UI thread and the other four travel to the engine's. A local reference
        // would die on returning from this function, and since jni 0.22 a global
        // one cannot be duplicated without the environment, so they all come out
        // of here.
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

        // The stderr redirection belongs to the whole process: set up once here.
        crate::logging::redirect_stderr(crate::logging::AndroidLog::new(vm_log, log_ref));

        let events = new_event_queue();
        let mount = MountSide::new(JniHost::new(vm_host, host_ref));
        // The plugins the shell registered before getting here. One per name;
        // what they do lives in Java, so these only carry and fetch.
        let plugins = crate::plugins::host_plugins();
        // The modules the framework brings. They are not plugins —nobody
        // declares them and there is no npm package— but they answer the same
        // way, because what they call lives on the UI thread too. See
        // `an_bridge::builtins`.
        let builtins = an_bridge::builtins::builtin_modules();

        let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
            let sink = crate::logging::AndroidLog::new(vm_device, device_ref);
            let mut js = QuickJsRuntime::with_log(sink.clone())?;
            js.register_module(Box::new(crate::modules::DeviceModule::new(
                vm_module,
                module_ref,
            )));
            for builtin in builtins {
                js.register_module(Box::new(builtin));
            }
            for plugin in plugins {
                js.register_module(Box::new(plugin));
            }
            Ok((js, ShadowSide::new(JniMeasurer::new(vm_measure, measure_ref), (width, height))))
        });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                eprintln!("angular-native: the JS engine did not start: {error}");
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

/// The same as on iOS: on a hot reload only the definitions change; if it does
/// not fit, the views go, the engine is new and the tree is empty.
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
        // It only unmounts if there was a restart: on a hot reload the tree is
        // still standing and throwing the views away would leave a blank screen.
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

/// One frame: events on to JS, JS's turn, layout, and mounting over here.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_AnRuntime_nativeFrame(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    now_ms: f64,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let Some(runtime) = (unsafe { runtime(handle) }) else { return Ok(-1) };

        // The plugin calls the engine left behind are served here, which is the
        // UI thread. It goes before JS's turn so that an answer arriving on the
        // spot makes it into this very frame.
        crate::plugins::pump(env);
        // And the built-ins', through their own mailbox and their own Java
        // object.
        crate::builtins::pump(env);

        // Whatever the worker finished since the previous frame is mounted, the
        // next turn is sent to it if it is not still busy, and it is waited on
        // for what is left of the frame: if it answers in time, what the user
        // just touched shows up in this very frame.
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

/// Kotlin queues here whatever an `OnClickListener` or a scroll produces. The
/// event is not dispatched now: it waits for the next frame, as it does on iOS.
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
        // `load` carries a size, not a position: the same two figures under
        // different names, and they have to be the ones iOS sends.
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

/// Gestures: pan, pinch, rotate, long press, swipe.
///
/// The field names come from the Java side instead of being pinned down here by
/// position. Splitting a string on commas costs something, but if one day
/// either side adds a field, the other does not start reading numbers that have
/// shifted along by one.
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

/// Events that carry an index: the selected tab, for instance.
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

/// Events that carry text instead of coordinates: typing in a field, entering
/// it and leaving it.
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
        // The safe area is four figures and travels as JSON: it is unpacked here
        // so the event arrives looking just like the iOS one.
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

/// `{"top":1,"right":2,...}` into pairs. No JSON parser: it is four numbers
/// under known names.
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

/// A URL the app was opened with, or one that arrived while it was running.
///
/// It takes no handle: the mailbox is global to the process, because a link can
/// be what starts the process in the first place and there is no runtime yet to
/// hang it off. The Apple hosts reach the same mailbox through
/// `an_deeplink_open`; here everything crosses JNI, so it gets its own door.
///
/// See `crates/an-bridge/src/deeplink.rs`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_angularnative_MainActivity_nativeOpenUrl(
    mut env: EnvUnowned,
    _class: JClass,
    url: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(url) = env.get_string(&url) else { return Ok(()) };
        let url: String = url.into();
        an_bridge::deep_links().open(&url);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>()
}
