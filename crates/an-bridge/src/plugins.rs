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

use serde_json::Value;

use crate::modules::{NativeModule, Responder};

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
    /// Calls picked up and still unanswered. The `Responder` kept here is what
    /// keeps the promise on the JS side alive; if the mailbox is destroyed, its
    /// `Drop` rejects them instead of leaving them hanging for ever.
    waiting: Mutex<HashMap<u64, Responder>>,
}

impl PluginBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(PluginBridge::default())
    }

    /// Picks up whatever came in since last time. The UI thread calls it once
    /// per frame.
    pub fn take_calls(&self) -> Vec<PluginCall> {
        std::mem::take(&mut *self.pending.lock().expect("the plugin mailbox is poisoned"))
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

    /// How many calls are still in flight. For diagnostics and for the tests.
    pub fn in_flight(&self) -> usize {
        self.waiting.lock().expect("the plugin mailbox is poisoned").len()
    }

    fn take_waiting(&self, id: u64) -> Result<Responder, String> {
        self.waiting.lock().expect("the plugin mailbox is poisoned").remove(&id).ok_or_else(|| {
            format!("there is no plugin call with id {id} waiting for an answer")
        })
    }

    fn enqueue(&self, module: &str, method: &str, args: Value, respond: Responder) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.waiting.lock().expect("the plugin mailbox is poisoned").insert(id, respond);
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
