//! Native modules: how JavaScript calls Rust code that is not the renderer.
//!
//! Everything that is not painting —reading the device, saving a file, asking
//! for permissions, talking over the network— comes in through here. The
//! contract is the same as the rest of the bridge: JS never blocks, and answers
//! arrive in a frame, this one or a later one.

use std::sync::{Arc, Mutex};

use serde_json::Value;

pub type ModuleResult = Result<Value, String>;

/// A call identifier. The bridge assigns it, and it travels out and back so the
/// answer can be matched to its promise on the JS side.
pub type CallId = u64;

/// The answers mailbox. It is an `Arc<Mutex<..>>` and not an `Rc<RefCell<..>>`
/// on purpose: a module can resolve from another thread —a download, a trip to
/// disk— and that is precisely why it exists.
type Outbox = Arc<Mutex<Vec<(CallId, ModuleResult)>>>;

/// The events mailbox: what a module has to say that nobody asked for.
///
/// It is deliberately a second mailbox rather than a special kind of answer.
/// An answer belongs to a call and is delivered once; an event belongs to
/// nothing, arrives whenever the platform has something, and may arrive a
/// thousand times or never. Squeezing the second through the first would mean
/// inventing a call that is never made.
type Events = Arc<Mutex<Vec<(&'static str, String, Value)>>>;

/// A module's way of speaking without being spoken to.
///
/// A module is handed one at registration and may keep it, clone it and send it
/// to another thread: a location manager delivering fixes, a socket, a
/// subscription to something the system publishes. Emitting from a background
/// thread is the normal case, not the exception, which is why this is an `Arc`
/// over a lock like the answers mailbox next to it.
///
/// Emitting into a runtime that has gone away is not an error. The app is
/// closing, the last thing anybody wants is a crash in a callback, and the
/// event has nowhere useful to go.
#[derive(Clone)]
pub struct Emitter {
    module: &'static str,
    events: Events,
}

impl Emitter {
    /// Queues an event. It reaches JS at the top of the next frame, in order,
    /// alongside the answers.
    pub fn emit(&self, event: impl Into<String>, payload: Value) {
        let Ok(mut queue) = self.events.lock() else { return };
        queue.push((self.module, event.into(), payload));
    }
}

/// What a module answers with. It can be resolved on the spot or held onto and
/// resolved later; if it is dropped without an answer, the promise on the JS
/// side is rejected instead of hanging around for ever.
pub struct Responder {
    id: CallId,
    outbox: Outbox,
    answered: bool,
}

impl Responder {
    pub fn resolve(mut self, value: Value) {
        self.answered = true;
        self.outbox.lock().expect("the mailbox is poisoned").push((self.id, Ok(value)));
    }

    pub fn reject(mut self, message: impl Into<String>) {
        self.answered = true;
        self.outbox
            .lock()
            .expect("the mailbox is poisoned")
            .push((self.id, Err(message.into())));
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        if self.answered {
            return;
        }
        // A module that forgets to answer is a bug, but a promise left hanging
        // for ever is worse: at least this one can be seen.
        self.outbox.lock().expect("the mailbox is poisoned").push((
            self.id,
            Err("the native module never answered".to_owned()),
        ));
    }
}

pub trait NativeModule {
    /// Handed the module's emitter, once, as it is registered.
    ///
    /// A module that only answers calls implements nothing. One that has
    /// something to say on its own keeps this and emits through it later,
    /// from whatever thread the platform calls it back on.
    fn connect(&mut self, _emitter: Emitter) {}

    /// The name JS calls it by.
    fn name(&self) -> &'static str;

    /// `respond` can be consumed right here or kept for later.
    fn call(&mut self, method: &str, args: Value, respond: Responder);
}

#[derive(Default)]
pub struct ModuleRegistry {
    modules: Vec<Box<dyn NativeModule>>,
    outbox: Outbox,
    events: Events,
    next_id: CallId,
    /// Why a name that is not here may still be a name somebody wrote in good
    /// faith. See `explain_absent`.
    absent: Option<String>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, mut module: Box<dyn NativeModule>) {
        // The emitter is handed over here rather than asked for later, so a
        // module cannot be in the registry without having had the chance to
        // take one, and cannot take one bound to a name that is not its own.
        let emitter = self.emitter(module.name());
        module.connect(emitter);
        self.modules.push(module);
    }

    /// Says why a module that is not registered may still be a name somebody
    /// wrote in good faith.
    ///
    /// On a host that loads plugins there is nothing to explain: a name that is
    /// not in the registry is a typo, and the message saying so is the whole
    /// answer. On a host that does not —macOS and the watch— it is not: the
    /// module may exist as an npm package, be installed, be imported and still
    /// not be here, and without this the rejection sends whoever reads it
    /// hunting for a spelling mistake that is not there.
    pub fn explain_absent(&mut self, note: impl Into<String>) {
        self.absent = Some(note.into());
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.modules.iter().map(|m| m.name()).collect()
    }

    /// Starts a call and returns its identifier. It does not block.
    pub fn invoke(&mut self, module: &str, method: &str, args: Value) -> CallId {
        self.next_id += 1;
        let id = self.next_id;
        let respond = Responder { id, outbox: self.outbox.clone(), answered: false };

        // The position first and the call afterwards: the `None` arm needs to
        // read the note, and it cannot while the search still holds the
        // registry borrowed.
        match self.modules.iter().position(|m| m.name() == module) {
            Some(index) => self.modules[index].call(method, args, respond),
            None => respond.reject(self.absent_message(module)),
        }
        id
    }

    /// The rejection for a module nobody registered.
    ///
    /// The name always comes out: it is what turns "something failed" into
    /// "this failed". What the host may add is the reason there is nothing
    /// under that name here, which is not something the message could otherwise
    /// be guessed to mean.
    fn absent_message(&self, module: &str) -> String {
        let mut message = format!("there is no native module called {module:?}");
        if let Some(note) = &self.absent {
            message.push_str(" — ");
            message.push_str(note);
        }
        message
    }

    /// Answers that came in since last time. The bridge collects them as it
    /// closes the frame.
    pub fn drain(&mut self) -> Vec<(CallId, ModuleResult)> {
        std::mem::take(&mut *self.outbox.lock().expect("the mailbox is poisoned"))
    }

    /// A handle a module keeps in order to emit events under its own name.
    ///
    /// The name is not a parameter of `emit`: it is bound here, so a module
    /// cannot emit under somebody else's name, by accident or otherwise.
    pub fn emitter(&self, module: &'static str) -> Emitter {
        Emitter { module, events: self.events.clone() }
    }

    /// Events that came in since last time, in the order they were emitted.
    pub fn drain_events(&mut self) -> Vec<(&'static str, String, Value)> {
        std::mem::take(&mut *self.events.lock().expect("the mailbox is poisoned"))
    }
}

/// Declares a native module without writing the dispatch by hand.
///
/// Each method gets its arguments already deserialised and returns
/// `Result<T, String>` with `T: Serialize`. A method that does not exist is
/// rejected with a message naming the one that was asked for.
///
/// ```ignore
/// native_module! {
///     Device as "device" {
///         fn info(&mut self, _: ()) -> Result<DeviceInfo, String> { ... }
///         fn vibrate(&mut self, args: VibrateArgs) -> Result<(), String> { ... }
///     }
/// }
/// ```
#[macro_export]
macro_rules! native_module {
    (
        $ty:ty as $name:literal {
            $(
                fn $method:ident (&mut $self_:ident, $arg:ident : $arg_ty:ty) -> Result<$ret:ty, String> $body:block
            )*
        }
    ) => {
        impl $ty {
            $(
                fn $method(&mut $self_, $arg: $arg_ty) -> Result<$ret, String> $body
            )*
        }

        impl $crate::modules::NativeModule for $ty {
            fn name(&self) -> &'static str {
                $name
            }

            fn call(
                &mut self,
                method: &str,
                args: ::serde_json::Value,
                respond: $crate::modules::Responder,
            ) {
                match method {
                    $(
                        stringify!($method) => {
                            let parsed: $arg_ty = match ::serde_json::from_value(args) {
                                Ok(parsed) => parsed,
                                Err(error) => {
                                    respond.reject(format!(
                                        concat!($name, ".", stringify!($method), ": invalid arguments: {}"),
                                        error
                                    ));
                                    return;
                                }
                            };
                            match self.$method(parsed) {
                                Ok(value) => match ::serde_json::to_value(value) {
                                    Ok(value) => respond.resolve(value),
                                    Err(error) => respond.reject(error.to_string()),
                                },
                                Err(message) => respond.reject(message),
                            }
                        }
                    )*
                    other => respond.reject(format!(
                        concat!("the ", $name, " module has no method {:?}")
                        , other
                    )),
                }
            }
        }
    };
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use serde_json::json;

    struct Talker;

    impl NativeModule for Talker {
        fn name(&self) -> &'static str {
            "talker"
        }
        fn call(&mut self, _method: &str, _args: Value, respond: Responder) {
            respond.resolve(json!("ok"));
        }
    }

    /// An emitter is bound to its module's name, so nothing can emit under
    /// somebody else's.
    #[test]
    fn an_event_carries_the_name_of_the_module_that_emitted_it() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(Talker));
        let emitter = registry.emitter("talker");

        assert!(registry.drain_events().is_empty(), "nothing has been emitted yet");

        emitter.emit("position", json!({ "latitude": 43.36 }));
        let events = registry.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "talker");
        assert_eq!(events[0].1, "position");
        assert_eq!(events[0].2, json!({ "latitude": 43.36 }));

        // Draining empties: an event is delivered once, not once a frame for
        // ever after.
        assert!(registry.drain_events().is_empty());
    }

    /// The whole reason for a second mailbox: emitting is not answering, and a
    /// module with nothing to answer can still have something to say.
    #[test]
    fn events_and_answers_do_not_share_a_queue() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(Talker));
        let emitter = registry.emitter("talker");

        emitter.emit("tick", json!(1));
        registry.invoke("talker", "anything", json!(null));

        assert_eq!(registry.drain().len(), 1, "the call was answered");
        assert_eq!(registry.drain_events().len(), 1, "and the event is still its own");
    }

    /// A module keeps its emitter and uses it from wherever the platform calls
    /// back, which is nearly always another thread.
    #[test]
    fn an_emitter_can_be_moved_to_another_thread() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(Talker));
        let emitter = registry.emitter("talker");

        std::thread::spawn(move || emitter.emit("from-a-thread", json!(true)))
            .join()
            .expect("the thread finished");

        let events = registry.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, "from-a-thread");
    }
}
