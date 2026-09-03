//! The C surface the SwiftUI shell consumes.
//!
//! It is almost the same as the iOS one —create, evaluate, a frame, free— with
//! two differences that follow from there being no views here:
//!
//! - `an_watch_runtime_new` takes no container. On iOS it is handed the
//!   `UIView` to hang things off; here there is nothing to hang off, because
//!   the tree lives in Rust and SwiftUI reads it.
//! - `an_watch_runtime_snapshot` shows up, which is how the shell finds out
//!   what to paint. On iOS it is not needed: by the time `an_runtime_frame`
//!   returns, the host has already touched the views.
//! - The device's data comes in from the shell instead of being read here. On
//!   iOS `UIDevice` and `UIScreen` are one `objc2` call away; the watch's are
//!   `WKInterfaceDevice`, which is WatchKit, has no Rust binding, and would be
//!   four trips through Objective-C for what Swift settles in one line. It
//!   arrives the same way the control sizes do, and for the same reason. See
//!   `crate::modules`.
//!
//! The JS engine is still on a thread of its own, and for the same reason as
//! on iOS: QuickJS needs some 4 MB of stack for Angular's router to navigate,
//! and the main thread does not have them.

use std::ffi::{c_char, CStr, CString};
use std::time::Duration;

use an_bridge::{QuickJsRuntime, Request, RuntimeWorker};
use an_core::PropValue;
use an_host::{drain_events, new_event_queue, EventQueue, MountSide, ShadowSide};

use crate::host::WatchHost;
use crate::measure::{ControlSizes, WatchMeasurer};

/// 8 MB, as on iOS. Measured there: Angular's router needs a little over 3 MB
/// to complete a navigation.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

/// How long the UI thread waits for the engine inside the frame.
///
/// On a watch the frame is not 16.6 ms: watchOS draws at 30 Hz while the app
/// is in the foreground, so there are 33 and one can afford to be a little
/// more patient. 8 ms are left over for SwiftUI to recompose afterwards.
const FRAME_BUDGET: Duration = Duration::from_millis(25);

/// Gets panics into the system log before they are lost.
///
/// It is the same hook `an-ios` and `an-macos` install, and the watch was the
/// one Apple host without it. A panic inside an `extern "C"` cannot unwind, so
/// Rust aborts with "panic in a function that cannot unwind" and the real
/// message —the one that says what happened— never gets out before the process
/// dies. Here it is written and flushed by hand, which is the difference
/// between debugging a crash and guessing at it.
fn report_panics() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let where_at = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "an unknown place".to_owned());
            use std::io::Write;
            let mut out = std::io::stderr().lock();
            let _ = writeln!(out, "angular-native: panic at {where_at}: {info}");
            let _ = out.flush();
            previous(info);
        }));
    });
}

pub struct AnWatchRuntime {
    worker: RuntimeWorker,
    mount: MountSide<WatchHost>,
    events: EventQueue,
    /// The last snapshot handed out. It is kept so that the `CString` stays
    /// alive while Swift reads it: returning a pointer into a temporary would
    /// be a dangling pointer the moment the function returned.
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

    /// A -1 is contagious: if anything in the frame failed, the frame failed.
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

    /// Drains whatever is still in flight. Before a control operation the
    /// channel has to be left clean, or the reply picked up will belong to
    /// something else.
    fn settle(&mut self) {
        while let Some(reply) = self.worker.wait_reply() {
            if let Some(error) = reply.error {
                eprintln!("angular-native: {error}");
            }
            self.mount.apply(&reply.frame);
        }
    }
}

/// Starts the runtime.
///
/// `control_json` holds the controls' natural sizes, which on a watch SwiftUI
/// measures and which cannot be asked about from here. Format:
/// `{"Button":[80,44]}`. It may be null: then the controls measure zero and
/// the layout collapses them, which is visible and therefore debuggable.
///
/// `device_json` is what `Device.info()` answers, minus the platform, which is
/// this crate's business: `{"systemVersion":…,"model":…,"scale":…,"locale":…}`.
/// It may be null too, and then the call is rejected saying the shell handed
/// nothing over — which beats an object with holes in it that reads as real.
///
/// # Safety
/// `control_json` and `device_json`, if not null, must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_new(
    width: f32,
    height: f32,
    control_json: *const c_char,
    device_json: *const c_char,
) -> *mut AnWatchRuntime {
    report_panics();
    let controls = unsafe { read_controls(control_json) };
    // Read before the worker starts: from there on this pointer belongs to
    // whoever called, and the module travels to the other thread already built.
    let device = crate::modules::DeviceModule::from_shell(unsafe { read(device_json) }.as_deref());
    let events = new_event_queue();
    let host = WatchHost::new(events.clone());
    // The plugins the shell registered before getting here. One per name; what
    // they do lives in Swift, so these only carry and fetch.
    let plugins = crate::plugins::host_plugins();
    // Read before the worker starts, because it names the plugins that are
    // there and the registry is closed the moment the engine is built.
    let absent = crate::plugins::absent_note();
    // The modules the framework brings. These are not plugins: nobody declares
    // them, and the SwiftUI shell serves them on the main actor. See
    // `an_bridge::builtins`.
    let builtins = an_bridge::builtins::builtin_modules();

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let mut js = QuickJsRuntime::new()?;
        js.register_module(Box::new(device));
        for plugin in plugins {
            js.register_module(Box::new(plugin));
        }
        // A call to a module that is not here has to be told why and not only
        // that the name is unknown. See `plugins::absent_note`.
        js.explain_absent_modules(absent);
        for builtin in builtins {
            js.register_module(Box::new(builtin));
        }
        Ok((js, ShadowSide::new(WatchMeasurer::new(controls), (width, height))))
    });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("angular-native: the JS engine did not start: {error}");
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

/// Evaluates a script: the app's bundle.
///
/// # Safety
/// `rt` must come from `an_watch_runtime_new`; `name` and `code`, C strings.
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

/// Puts new code into the app that is already running. It is what `an dev`
/// uses.
///
/// The same as on iOS: if the new bundle fits what is mounted, only the
/// components' definitions change and the state is kept; if it does not,
/// everything is stood up again.
///
/// # Safety
/// `rt` must come from `an_watch_runtime_new`; `name` and `code`, C strings.
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
    // The model is only emptied if there was a restart: on a hot reload the
    // tree is still standing, and throwing it away would leave a blank screen
    // waiting for creations the core has no reason to send again.
    if !reply.hot {
        rt.mount.clear();
    }
    report(reply.error)
}

/// # Safety
/// `rt` must come from `an_watch_runtime_new` and still be alive.
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

/// A complete frame: events on to JS, JS's turn, layout, and the `MountOp`s
/// applied to the model. It paints nothing: SwiftUI takes care of that when it
/// reads the snapshot.
///
/// Returns the operations applied, or -1 if something failed.
///
/// # Safety
/// `rt` must come from `an_watch_runtime_new` and still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_frame(rt: *mut AnWatchRuntime, now_ms: f64) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };

    // The plugin calls the engine left behind are served here, which the shell
    // runs on the main actor: it is the only place where a plugin may touch
    // WatchKit. It goes before JS's turn so that an answer arriving on the spot
    // makes it into this very frame.
    crate::plugins::pump();
    // The built-in calls the engine left behind are served here, on the main
    // actor: it is where WatchKit lives. Before JS's turn, so that an answer
    // arriving on the spot makes it into this very frame.
    an_bridge::builtins::pump_c();

    let mut applied = rt.pump();
    // If the worker is still busy it is not queued another turn: the queue
    // would grow without end and every mounted frame would be older than the
    // one before it.
    if !rt.worker.busy() {
        let events = drain_events(&rt.events);
        rt.worker.post(Request::Tick { now_ms, events });
        if let Some(reply) = rt.worker.wait_reply_until(FRAME_BUDGET) {
            applied = rt.mount_reply(reply, applied);
        }
    }
    applied
}

/// The model's revision number. It only rises when a frame brought changes.
///
/// The shell compares it with the one it already has and only asks for the
/// snapshot when they differ: a frame in which nothing moved serialises
/// nothing and does not wake SwiftUI.
///
/// # Safety
/// `rt` must come from `an_watch_runtime_new` and still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_revision(rt: *mut AnWatchRuntime) -> u64 {
    let Some(rt) = (unsafe { rt.as_ref() }) else { return 0 };
    rt.mount.host().revision()
}

/// The whole tree as JSON. The pointer is valid until the next call to this
/// same function or until `an_watch_runtime_free`: Swift copies it into a
/// `String` on the spot and does not keep it.
///
/// # Safety
/// `rt` must come from `an_watch_runtime_new` and still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_snapshot(rt: *mut AnWatchRuntime) -> *const c_char {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return std::ptr::null() };
    rt.mount.host_mut().take_dirty();
    let snapshot = crate::snapshot::snapshot(rt.mount.host());
    let json = match serde_json::to_string(&snapshot) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("angular-native: the tree could not be serialised: {error}");
            return std::ptr::null();
        }
    };
    // A `\0` inside the JSON would make it impossible to pass as a C string.
    // None should ever arrive, but if one does, painting nothing beats
    // corrupting something.
    let Ok(cstring) = CString::new(json) else {
        eprintln!("angular-native: the tree carried a zero byte inside it");
        return std::ptr::null();
    };
    let pointer = cstring.as_ptr();
    rt.last_json = Some(cstring);
    pointer
}

/// A native event from SwiftUI. It is only queued if the template registered
/// that listener on that node.
///
/// `payload_json` is a flat object —`{"value":0.4}`, `{"x":12,"y":30}`— or
/// null for the events that carry nothing. JSON is taken rather than a handful
/// of parameters because each event carries different keys: a `pan` carries
/// six numbers and a `dismiss` none, and a signature that suited both would be
/// a signature that says nothing.
///
/// Only flat values are accepted. An object or a list inside is rejected with
/// a warning, because the other side —`PropValue`— has no way to represent
/// them, and swallowing them would turn them into nulls nobody could explain.
///
/// # Safety
/// `rt` must come from `an_watch_runtime_new`; `name`, a valid C string, and
/// `payload_json`, a valid C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_event(
    rt: *mut AnWatchRuntime,
    target: u32,
    name: *const c_char,
    payload_json: *const c_char,
) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    if name.is_null() {
        return;
    }
    let Ok(name) = (unsafe { CStr::from_ptr(name) }).to_str() else { return };
    let payload = unsafe { read_payload(payload_json) };
    rt.mount.host().dispatch(target, name, payload);
}

/// Takes an event's JSON object apart into the pairs the bridge understands.
unsafe fn read_payload(raw: *const c_char) -> Vec<(String, PropValue)> {
    if raw.is_null() {
        return Vec::new();
    }
    let Ok(text) = (unsafe { CStr::from_ptr(raw) }).to_str() else {
        eprintln!("angular-native: an event's payload was not UTF-8");
        return Vec::new();
    };
    let parsed: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(text) {
        Ok(map) => map,
        Err(error) => {
            eprintln!("angular-native: an event's payload makes no sense: {error}");
            return Vec::new();
        }
    };
    let mut payload = Vec::with_capacity(parsed.len());
    for (key, value) in parsed {
        let converted = match value {
            serde_json::Value::Bool(b) => PropValue::Bool(b),
            serde_json::Value::String(s) => PropValue::Str(s),
            serde_json::Value::Null => PropValue::Null,
            serde_json::Value::Number(n) => match n.as_f64() {
                Some(number) => PropValue::Number(number),
                None => {
                    eprintln!("angular-native: {key} carried a number that does not fit in an f64");
                    continue;
                }
            },
            other => {
                eprintln!("angular-native: {key} carried {other}, which the bridge cannot carry");
                continue;
            }
        };
        payload.push((key, converted));
    }
    payload
}

/// # Safety
/// `rt` must come from `an_watch_runtime_new` and must not already have been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_watch_runtime_free(rt: *mut AnWatchRuntime) {
    if !rt.is_null() {
        drop(unsafe { Box::from_raw(rt) });
    }
}

/// # Safety
/// Both pointers have to be valid C strings or null.
unsafe fn read_pair(name: *const c_char, code: *const c_char) -> Option<(String, String)> {
    if name.is_null() || code.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(name) }.to_str().ok()?;
    let code = unsafe { CStr::from_ptr(code) }.to_str().ok()?;
    Some((name.to_owned(), code.to_owned()))
}

/// # Safety
/// `text`, if not null, has to be a valid C string.
unsafe fn read(text: *const c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(text) }.to_str().ok().map(str::to_owned)
}

/// # Safety
/// `raw`, if not null, has to be a valid C string.
unsafe fn read_controls(raw: *const c_char) -> ControlSizes {
    if raw.is_null() {
        return ControlSizes::new();
    }
    let Ok(text) = (unsafe { CStr::from_ptr(raw) }).to_str() else {
        return ControlSizes::new();
    };
    serde_json::from_str::<ControlSizes>(text).unwrap_or_else(|error| {
        eprintln!("angular-native: unreadable control sizes: {error}");
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
