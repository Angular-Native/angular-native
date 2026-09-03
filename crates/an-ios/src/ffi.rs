//! The C surface the Xcode shell consumes.
//!
//! The main thread keeps the one thing that cannot leave it: the views. The JS
//! engine, the tree and the layout live on a separate thread with a large
//! stack, because iOS's main one has 1 MB and QuickJS needs four times that
//! for Angular to navigate.
//!
//! Each frame's `Tick` is waited on, but with a deadline: if JS's turn fits in
//! what is left of the frame it is mounted in that same frame, and if it
//! overruns, the UI thread carries on and mounts it when it arrives.

use std::ffi::{c_char, c_void, CStr};

use std::time::Duration;

use an_bridge::{QuickJsRuntime, Request, RuntimeWorker};
use an_host::{drain_events, new_event_queue, EventQueue, MountSide, ShadowSide};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_ui_kit::UIView;

use crate::host::UikitHost;
use crate::measure::UikitMeasurer;

/// 8 MB. Measured: Angular's router needs a little over 3 MB to complete a
/// navigation, and with 2 MB the transition gets seven events in and stops.
const RUNTIME_STACK: usize = 8 * 1024 * 1024;

/// How long the UI thread waits for the engine inside the frame. Twelve
/// milliseconds leave room over the 16.6 of a 60 Hz frame to mount the views
/// afterwards. An ordinary Angular turn takes far less.
const FRAME_BUDGET: Duration = Duration::from_millis(12);

pub struct AnRuntime {
    worker: RuntimeWorker,
    mount: MountSide<UikitHost>,
    events: EventQueue,
    /// Operations mounted in the last frame that actually brought work.
    last_applied: i32,
}

impl AnRuntime {
    /// Mounts whatever has arrived from the worker. Does not block.
    fn pump(&mut self) -> i32 {
        let mut applied = 0;
        while let Some(reply) = self.worker.try_reply() {
            applied = self.mount_reply(reply, applied);
        }
        applied
    }

    /// Applies one reply and accumulates the count. A -1 is contagious: if
    /// anything in the frame failed, the frame failed.
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

/// Gets panics into the system log before they are lost.
///
/// A panic inside an `extern "C"` cannot unwind, so Rust aborts with "panic in
/// a function that cannot unwind" and the real message —the one that says what
/// happened— is lost: whatever was written to the error output never gets out
/// before the process dies. Here it is written and flushed by hand, which is
/// the difference between debugging a crash and guessing at it.
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

/// # Safety
/// `container` must be a live `UIView`. Call from the main thread.
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
    let container: Retained<UIView> = unsafe {
        Retained::retain(container.cast::<UIView>()).expect("container cannot be nil")
    };

    let events = new_event_queue();
    let host = UikitHost::new(mtm, container, events.clone());
    // The device's data is read here, on the main thread, and travels already
    // resolved: UIDevice and UIScreen cannot be touched from the worker.
    let device = crate::modules::DeviceModule::capture(mtm);
    // The system's controls are measured here, on the main thread: creating a
    // UISwitch off it is not allowed.
    let control_sizes = crate::controls::measure_controls(mtm);
    // The plugins the shell registered before getting here. One per name;
    // what they do lives in Swift, so these only carry and fetch.
    let plugins = crate::modules::host_plugins();

    let worker = RuntimeWorker::spawn(RUNTIME_STACK, move || {
        let mut js = QuickJsRuntime::new()?;
        js.register_module(Box::new(device));
        for plugin in plugins {
            js.register_module(Box::new(plugin));
        }
        Ok((js, ShadowSide::new(UikitMeasurer::new(control_sizes), (width, height))))
    });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("angular-native: the JS engine did not start: {error}");
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

/// Evaluates a script. The app's bundle is what decides what to load, the
/// same way React Native's `main.jsbundle` does.
///
/// Returns 0 if it went well and -1 if JS threw; the error goes out on stderr
/// with its trace.
///
/// # Safety
/// `rt` must come from `an_runtime_new`. `name` and `code` must be valid,
/// nul-terminated C strings.
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

/// Puts new code into the app that is running. It is what `an dev` uses when
/// it spots a change.
///
/// If the new bundle fits what is mounted, only the components' definitions
/// change and the state is kept. If it does not, everything is stood up again:
/// native views out, a new JS engine, an empty tree, and a `signal` back to
/// its initial value.
///
/// # Safety
/// `rt` must come from `an_runtime_new`. `name` and `code`, valid C strings.
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
    // Unmounting comes after knowing how it ended: on a hot reload the tree
    // is still standing, and throwing the views away would leave a black
    // screen waiting for creations the core has no reason to send again. The
    // worker only throws the tree away when it restarts, and that is when this
    // has to be emptied —here, which is where UIKit can be touched.
    if !reply.hot {
        rt.mount.clear();
    }
    report(reply.error)
}

/// # Safety
/// `rt` must come from `an_runtime_new` and still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_set_viewport(rt: *mut AnRuntime, width: f32, height: f32) {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return };
    rt.settle();
    rt.worker.request(Request::SetViewport(width, height));
}

/// A complete frame: native events on to JS, JS's turn, layout, and mounting
/// over here. `now_ms` is the `CADisplayLink`'s timestamp, which is the only
/// clock the app sees.
///
/// Returns the number of native operations applied, or -1 if something failed.
///
/// # Safety
/// `rt` must come from `an_runtime_new` and still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_frame(rt: *mut AnRuntime, now_ms: f64) -> i32 {
    let Some(rt) = (unsafe { rt.as_mut() }) else { return -1 };

    // The plugin calls the engine left behind are served here, which is the
    // main thread: it is the only place where a plugin may touch UIKit. It
    // goes before JS's turn so that an answer arriving on the spot makes it
    // into this very frame.
    crate::modules::pump();

    // Whatever the worker finished since the previous frame is mounted.
    let mut applied = rt.pump();

    // If it is still busy it is not queued another turn: the queue would grow
    // without end and every mounted frame would be older than the one before
    // it.
    if !rt.worker.busy() {
        let events = drain_events(&rt.events);
        rt.worker.post(Request::Tick { now_ms, events });
        // And it is waited on for what is left of the frame. If it answers in
        // time, what the user just touched shows up in this very frame.
        if let Some(reply) = rt.worker.wait_reply_until(FRAME_BUDGET) {
            applied = rt.mount_reply(reply, applied);
        }
    }
    rt.last_applied = applied;
    applied
}

/// # Safety
/// `rt` must come from `an_runtime_new` and must not already have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_runtime_free(rt: *mut AnRuntime) {
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

fn report(error: Option<String>) -> i32 {
    match error {
        None => 0,
        Some(message) => {
            eprintln!("angular-native: {message}");
            -1
        }
    }
}
