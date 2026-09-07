//! Plugins: native modules written outside the repo.
//!
//! A module of the [`crate::modules`] sort is implemented in Rust and compiled
//! into the core. A *plugin* is not: somebody outside writes it, in Swift or in
//! Java, and all the core has to do is carry the call over and bring the answer
//! back.
//!
//! That is why there is no implementation per plugin here but a single one,
//! [`HostPlugin`], acting as postman. What differs from one plugin to the next
//! is on the other side of the border, not here.
//!
//! The way the threads are split is the reason this is a queue and not a direct
//! call. `NativeModule::call` runs on the JS engine's thread; `UIPasteboard` and
//! `ClipboardManager` want the UI one. So the call gets queued, the UI thread
//! picks it up in its frame —it already comes through there once per vsync— and
//! answers when it can: on the spot, or three seconds later if what is behind it
//! is a camera. The engine waits for nobody.
//!
//! ```text
//!   engine thread                          UI thread
//!   ──────────────────                     ─────────────────────
//!   HostPlugin::call
//!        │ queues it with its Responder
//!        ▼
//!   PluginBridge ──────── take_calls() ──▶ AnPluginRegistry (Swift/Java)
//!        ▲                                        │
//!        └────────── resolve(id, json) ◀──────────┘
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::modules::{NativeModule, Responder};

/// A call the platform has taken and not answered.
///
/// The `Responder` is what keeps the promise on the JS side alive. The rest is
/// so that a call which never comes back can be *named*: without the module,
/// the method and the moment it went out, an unanswered promise is invisible —
/// no error, no log, a screen that simply never moves on.
struct Waiting {
    responder: Responder,
    module: String,
    method: String,
    since: Instant,
    /// Said once. A frame is sixty a second and the same call would otherwise
    /// fill the log with the same line until the app is closed.
    warned: bool,
}

/// How long a call may go unanswered before it is mentioned.
///
/// Deliberately generous. A camera is open for as long as the person using it
/// takes, and a download is as slow as the network: a number small enough to
/// catch a hang quickly is a number that cries wolf over calls that were going
/// to work. A minute is past anything that is merely slow and well short of
/// forever.
///
/// It warns and does not reject. Cutting the call short would mean deciding,
/// from here, that a plugin waiting on a human has failed — and nothing here
/// can tell that apart from a plugin that lost its call object. The decision
/// belongs to the plugin, which is the only one that knows which of its
/// methods wait on people; until it can say so, the honest thing is to make
/// the wait visible rather than to guess at it.
const SLOW_CALL: Duration = Duration::from_secs(60);

/// A call waiting for the platform to get to it.
pub struct PluginCall {
    /// The identifier the platform will answer with.
    pub id: u64,
    /// The module's name, exactly as JS wrote it.
    pub module: String,
    pub method: String,
    /// The arguments already serialised: on the other side there is Swift or
    /// Java, not serde.
    pub args: String,
}

/// The mailbox shared between the engine thread and the UI one.
///
/// It is created once per process and lives behind an `Arc`: the [`HostPlugin`]s
/// keep one copy and the platform's shell keeps another.
#[derive(Default)]
pub struct PluginBridge {
    next: AtomicU64,
    /// Calls the UI thread has not picked up yet.
    pending: Mutex<Vec<PluginCall>>,
    /// Calls picked up and still unanswered. The `Responder` inside is what
    /// keeps the promise on the JS side alive; if the mailbox is destroyed, its
    /// `Drop` rejects them instead of leaving them hanging for ever.
    waiting: Mutex<HashMap<u64, Waiting>>,
    /// One emitter per registered plugin, by the name JS calls it by.
    ///
    /// The shell emits by name — it has a string from Swift or Java and nothing
    /// else — and this is what turns that string back into the right module's
    /// emitter. A name nobody registered is an error and not a silent drop: it
    /// is a typo in a plugin, and typos that vanish are the expensive kind.
    emitters: Mutex<HashMap<String, crate::modules::Emitter>>,
}

impl PluginBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(PluginBridge::default())
    }

    /// Picks up whatever came in since last time. The UI thread calls it once
    /// per frame.
    pub fn take_calls(&self) -> Vec<PluginCall> {
        // Once per frame is also exactly the heartbeat the slow-call check
        // wants, and it is already here on every host: no clock to wire, no
        // thread to start, and nothing at all to pay on a frame with no calls
        // outstanding.
        self.mention_slow_calls(SLOW_CALL);
        std::mem::take(&mut *self.pending.lock().expect("the plugin mailbox is poisoned"))
    }

    /// Says, once per call, that something went out and has not come back.
    ///
    /// This is the last way left to leave a promise unsettled: the plugin keeps
    /// its `AnPluginCall` and calls neither `resolve` nor `reject`. Every other
    /// road now ends in one of the two — an unknown module, an unknown method,
    /// an answer that is not JSON, an error escaping the plugin, the mailbox
    /// being destroyed.
    ///
    /// It stays a warning. See `SLOW_CALL`.
    /// The threshold is a parameter and not read straight from `SLOW_CALL` for
    /// one reason: a test cannot wait a minute. Passing `Duration::ZERO` is how
    /// the warning path is exercised at all, and a warning nobody has ever seen
    /// fire is a warning that may not.
    fn mention_slow_calls(&self, after: Duration) -> usize {
        let mut mentioned = 0;
        let mut waiting = self.waiting.lock().expect("the plugin mailbox is poisoned");
        for call in waiting.values_mut() {
            if call.warned {
                continue;
            }
            let waited = call.since.elapsed();
            if waited < after {
                continue;
            }
            call.warned = true;
            mentioned += 1;
            eprintln!(
                "angular-native: {}.{} was called {}s ago and has not answered. If that is \
                 normal for this method — a camera waits for a person — there is nothing wrong \
                 here. If it is not, the plugin is holding its call object and has called \
                 neither resolve nor reject, and the promise on the JS side will never settle.",
                call.module,
                call.method,
                waited.as_secs()
            );
        }
        mentioned
    }

    /// How long the oldest unanswered call has been waiting. For the tests.
    pub fn oldest_wait(&self) -> Option<Duration> {
        self.waiting
            .lock()
            .expect("the plugin mailbox is poisoned")
            .values()
            .map(|call| call.since.elapsed())
            .max()
    }

    /// Answers a call. `json` is the return value, already serialised; `"null"`
    /// for a method that returns nothing.
    ///
    /// The `Err` is not the plugin's failure but the shell's: either it answered
    /// the same call twice, or it answered one that does not exist. It is
    /// returned so the platform can log it; swallowing it would leave a broken
    /// plugin looking like a slow one.
    pub fn resolve(&self, id: u64, json: &str) -> Result<(), String> {
        let responder = self.take_waiting(id)?;
        match serde_json::from_str::<Value>(json) {
            Ok(value) => responder.resolve(value),
            // The plugin did answer; what makes no sense is its answer. The
            // promise is rejected with the exact reason rather than resolved
            // with an `undefined` nobody could trace back to anything.
            Err(error) => responder
                .reject(format!("the plugin answered with something that is not valid JSON: {error}")),
        }
        Ok(())
    }

    pub fn reject(&self, id: u64, message: &str) -> Result<(), String> {
        self.take_waiting(id)?.reject(message.to_owned());
        Ok(())
    }

    /// Remembers how to emit under a plugin's name. Called as it registers.
    pub fn attach_emitter(&self, module: &str, emitter: crate::modules::Emitter) {
        self.emitters
            .lock()
            .expect("the plugin mailbox is poisoned")
            .insert(module.to_owned(), emitter);
    }

    /// An event from a plugin, on its way to whoever subscribed in JS.
    ///
    /// `json` is the payload already serialised, because on the other side
    /// there is Swift or Java and not serde. Unlike an answer, this belongs to
    /// no call: it may arrive at any time, including never, and nothing on the
    /// JS side is waiting for it.
    pub fn emit(&self, module: &str, event: &str, json: &str) -> Result<(), String> {
        let payload = serde_json::from_str::<Value>(json).map_err(|error| {
            format!("{module}.{event} was emitted with something that is not valid JSON: {error}")
        })?;
        let emitters = self.emitters.lock().expect("the plugin mailbox is poisoned");
        let emitter = emitters
            .get(module)
            .ok_or_else(|| format!("no plugin called {module:?} is registered, so {event:?} has nowhere to go"))?;
        emitter.emit(event.to_owned(), payload);
        Ok(())
    }

    /// How many calls are still in flight. For diagnostics and for the tests.
    pub fn in_flight(&self) -> usize {
        self.waiting.lock().expect("the plugin mailbox is poisoned").len()
    }

    fn take_waiting(&self, id: u64) -> Result<Responder, String> {
        self.waiting
            .lock()
            .expect("the plugin mailbox is poisoned")
            .remove(&id)
            .map(|call| call.responder)
            .ok_or_else(|| format!("there is no plugin call with id {id} waiting for an answer"))
    }

    fn enqueue(&self, module: &str, method: &str, args: Value, respond: Responder) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.waiting.lock().expect("the plugin mailbox is poisoned").insert(
            id,
            Waiting {
                responder: respond,
                module: module.to_owned(),
                method: method.to_owned(),
                since: Instant::now(),
                warned: false,
            },
        );
        self.pending.lock().expect("the plugin mailbox is poisoned").push(PluginCall {
            id,
            module: module.to_owned(),
            method: method.to_owned(),
            args: args.to_string(),
        });
    }
}

/// The native module that stands in for a plugin inside the registry.
///
/// One per declared name. It does not know which methods the plugin has, and it
/// is not its job to: the one that turns down a method that does not exist is
/// the implementation, the only one that knows its own list.
pub struct HostPlugin {
    name: &'static str,
    bridge: Arc<PluginBridge>,
}

impl HostPlugin {
    /// The name is leaked on purpose. `NativeModule::name` returns a
    /// `&'static str` because the modules compiled in are literals, and here the
    /// name arrives at runtime. It is a few dozen bytes per plugin, once in the
    /// life of the process.
    pub fn new(name: &str, bridge: Arc<PluginBridge>) -> Self {
        HostPlugin { name: Box::leak(name.to_owned().into_boxed_str()), bridge }
    }
}

impl NativeModule for HostPlugin {
    /// A plugin's emitter is handed straight to the mailbox the shell talks to,
    /// so the shell can emit with nothing but a name and a payload.
    fn connect(&mut self, emitter: crate::modules::Emitter) {
        self.bridge.attach_emitter(self.name, emitter);
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn call(&mut self, method: &str, args: Value, respond: Responder) {
        self.bridge.enqueue(self.name, method, args, respond);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModuleRegistry;

    fn registry_with_clipboard() -> (ModuleRegistry, Arc<PluginBridge>) {
        let bridge = PluginBridge::new();
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(HostPlugin::new("clipboard", bridge.clone())));
        (registry, bridge)
    }

    /// The last road to an unsettled promise, and the only one left: the
    /// plugin keeps its call object and calls neither. Nothing can be done
    /// about it from here without deciding that a plugin waiting on a person
    /// has failed — but it can be named, and what it must not do is stay
    /// invisible.
    #[test]
    fn a_call_nobody_answers_is_still_being_waited_on_and_can_be_named() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "read", Value::Null);

        // Taken by the platform and never answered.
        let calls = bridge.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(bridge.in_flight(), 1, "the promise is still alive");

        // Frames go by; it stays in flight and nothing else picks it up.
        for _ in 0..5 {
            assert!(bridge.take_calls().is_empty());
        }
        assert_eq!(bridge.in_flight(), 1);
        // And the mailbox knows how long it has been, which is what the
        // warning is built on.
        assert!(bridge.oldest_wait().is_some());

        // The warning itself, with the threshold brought down to nothing so a
        // test does not have to wait a minute for it.
        assert_eq!(bridge.mention_slow_calls(Duration::ZERO), 1, "it has to be mentioned");
        // Once per call and not once per frame: at sixty frames a second the
        // second kind fills the log until the app is closed.
        assert_eq!(bridge.mention_slow_calls(Duration::ZERO), 0, "and only once");
    }

    /// The counterpart: a call that *is* answered leaves nothing behind, so
    /// the slow-call bookkeeping cannot grow without bound on a healthy app.
    #[test]
    fn an_answered_call_stops_being_waited_on() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "read", Value::Null);
        let calls = bridge.take_calls();
        bridge.resolve(calls[0].id, "\"some text\"").unwrap();
        assert_eq!(bridge.in_flight(), 0);
        assert!(bridge.oldest_wait().is_none());
    }

    #[test]
    fn the_call_travels_out_and_the_answer_comes_back() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "read", Value::Null);

        // Nothing resolved yet: the engine does not block waiting on the
        // platform.
        assert!(registry.drain().is_empty());

        let calls = bridge.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].module, "clipboard");
        assert_eq!(calls[0].method, "read");
        assert_eq!(calls[0].args, "null");
        // And it is no longer pending: picking up is consuming.
        assert!(bridge.take_calls().is_empty());
        assert_eq!(bridge.in_flight(), 1);

        bridge.resolve(calls[0].id, "\"hello\"").expect("the call was waiting");
        assert_eq!(bridge.in_flight(), 0);
        let answers = registry.drain();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].1, Ok(Value::String("hello".to_owned())));
    }

    #[test]
    fn the_arguments_arrive_serialised() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "write", serde_json::json!({ "text": "ünïcôde" }));
        let calls = bridge.take_calls();
        assert_eq!(calls[0].args, r#"{"text":"ünïcôde"}"#);
    }

    #[test]
    fn a_rejection_arrives_with_its_reason() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "read", Value::Null);
        let calls = bridge.take_calls();
        bridge.reject(calls[0].id, "the clipboard is empty").expect("it was waiting");
        assert_eq!(registry.drain()[0].1, Err("the clipboard is empty".to_owned()));
    }

    #[test]
    fn answering_twice_gets_noticed() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "read", Value::Null);
        let id = bridge.take_calls()[0].id;
        bridge.resolve(id, "null").expect("the first one goes through");
        assert!(bridge.resolve(id, "null").is_err(), "the second one has to own up");
        // And JS got one answer, not two.
        assert_eq!(registry.drain().len(), 1);
    }

    #[test]
    fn an_unreadable_answer_rejects_the_promise() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("clipboard", "read", Value::Null);
        let id = bridge.take_calls()[0].id;
        bridge.resolve(id, "{this is not json}").expect("the call did exist");
        let answers = registry.drain();
        assert!(matches!(&answers[0].1, Err(message) if message.contains("JSON")));
    }

    #[test]
    fn a_module_that_is_not_registered_does_not_swallow_the_call() {
        let (mut registry, bridge) = registry_with_clipboard();
        registry.invoke("biometrics", "authenticate", Value::Null);
        // It never reached the platform's queue…
        assert!(bridge.take_calls().is_empty());
        // …and the promise is rejected on the spot, naming the one that was
        // missing.
        assert!(matches!(&registry.drain()[0].1, Err(message) if message.contains("biometrics")));
    }
}
