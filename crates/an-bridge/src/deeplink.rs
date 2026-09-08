//! Deep links: the URL that opened the app, on its way to Angular's router.
//!
//! A link can arrive at two very different moments, and the difference is the
//! whole reason this is a mailbox and not a callback:
//!
//! - **Cold start.** The system launches the process *because* of the URL. It is
//!   handed to the shell before the engine exists, let alone Angular. There is
//!   nobody to give it to yet.
//! - **Already running.** `application(_:open:options:)` or `onNewIntent` fires
//!   while the app is on screen and somebody is listening.
//!
//! So the URL is written down first and read later. Until JS says it is
//! listening —which it does by taking what is waiting— every link is queued;
//! from that moment on they are emitted as `deeplink.url` events. The handover
//! happens under one lock, so a link that lands exactly between the two cannot
//! be delivered twice or lost.
//!
//! ```text
//!   UI thread                         engine thread
//!   ─────────────────                 ──────────────────────────────
//!   an_deeplink_open(url) ─┐
//!   Java …_nativeOpenUrl ──┴─▶ DeepLinks
//!                                 │  queued while nobody is listening
//!                                 │  __an.deepLinks()  ──▶ the initial URL
//!                                 └─ Emitter ────────────▶ deeplink.url
//! ```
//!
//! This lives in the bridge and not in a host crate on purpose: nothing about it
//! is platform code. Every host that builds a [`crate::QuickJsRuntime`] gets the
//! JS side for free, and the only thing a shell has to do is hand the URL over.

use std::ffi::{c_char, CStr};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::json;

use crate::modules::Emitter;

/// The name JS subscribes under. Its twin is `DEEP_LINK_MODULE` in
/// `packages/platform-native/src/deep-links.ts`; `scripts/check-deep-links.sh`
/// reads both, because a name that drifts here goes silent rather than failing.
pub const DEEP_LINK_MODULE: &str = "deeplink";

/// The event a link arrives as once the app is running.
pub const DEEP_LINK_EVENT: &str = "url";

#[derive(Default)]
struct Inner {
    /// Links that arrived before anyone was listening, oldest first.
    waiting: Vec<String>,
    emitter: Option<Emitter>,
    /// `true` once JS has taken the queue. Before that an event would be
    /// emitted into a frame no subscriber has seen yet.
    listening: bool,
}

/// The mailbox. One per process: a link belongs to the app, not to a runtime,
/// and it can arrive before the runtime exists.
#[derive(Default)]
pub struct DeepLinks {
    inner: Mutex<Inner>,
}

impl DeepLinks {
    pub fn new() -> Arc<Self> {
        Arc::new(DeepLinks::default())
    }

    /// A URL from outside. Called from whatever thread the platform delivers on.
    ///
    /// An empty URL is dropped rather than queued: some launch paths hand over
    /// an empty string when there was no link at all, and turning that into a
    /// navigation to `/` would undo whatever the app had restored.
    pub fn open(&self, url: &str) {
        let url = url.trim();
        if url.is_empty() {
            return;
        }
        let mut inner = self.inner.lock().expect("the deep-link mailbox is poisoned");
        match (inner.listening, inner.emitter.as_ref()) {
            (true, Some(emitter)) => emitter.emit(DEEP_LINK_EVENT, json!({ "url": url })),
            // Nobody has asked for the queue yet: the app is still starting, or
            // it is being restarted after a failed hot reload.
            _ => inner.waiting.push(url.to_owned()),
        }
    }

    /// Everything that arrived before now, and from now on events instead.
    ///
    /// This is what makes the cold start race-free: JS subscribes and *then*
    /// calls this, so the queue and the event stream meet exactly once.
    pub fn take(&self) -> Vec<String> {
        let mut inner = self.inner.lock().expect("the deep-link mailbox is poisoned");
        inner.listening = true;
        std::mem::take(&mut inner.waiting)
    }

    /// Where the events go. The engine attaches one as it builds its module
    /// registry.
    ///
    /// It also puts the mailbox back to queueing. A new emitter means a new
    /// engine —a restart, or a hot reload that could not be stitched— and the
    /// app inside it has not subscribed to anything yet. Whatever is still
    /// waiting stays waiting: a link that arrived while the engine was being
    /// rebuilt is not one to throw away.
    pub fn attach(&self, emitter: Emitter) {
        let mut inner = self.inner.lock().expect("the deep-link mailbox is poisoned");
        inner.emitter = Some(emitter);
        inner.listening = false;
    }

    /// How many links are waiting. For diagnostics and for the tests.
    pub fn waiting(&self) -> usize {
        self.inner.lock().expect("the deep-link mailbox is poisoned").waiting.len()
    }
}

/// The process-wide mailbox, the one the C and JNI entry points write into.
pub fn deep_links() -> &'static Arc<DeepLinks> {
    static LINKS: OnceLock<Arc<DeepLinks>> = OnceLock::new();
    LINKS.get_or_init(DeepLinks::new)
}

/// Hands the app a URL it was opened with, or one that arrived while it ran.
///
/// Shared by the Apple hosts the way `an_builtin_resolve` is: they are separate
/// static libraries that never meet in one binary. Android does not use it —
/// everything there crosses JNI.
///
/// Returns 0 if the URL was taken and -1 if the string could not be read.
///
/// # Safety
/// `url` has to be null or a valid, nul-terminated UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn an_deeplink_open(url: *const c_char) -> i32 {
    if url.is_null() {
        return -1;
    }
    match unsafe { CStr::from_ptr(url) }.to_str() {
        Ok(text) => {
            deep_links().open(text);
            0
        }
        Err(_) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModuleRegistry;

    /// The cold start: the URL is there before anything can listen, and it is
    /// still there when something finally does.
    #[test]
    fn a_link_that_arrives_before_anyone_listens_is_kept() {
        let links = DeepLinks::new();
        links.open("myapp://ship/2");
        assert_eq!(links.waiting(), 1);
        assert_eq!(links.take(), vec!["myapp://ship/2".to_owned()]);
        // Taken once. A second reader gets nothing rather than the same link
        // again, or the app would navigate twice on every reload.
        assert!(links.take().is_empty());
    }

    /// And once it is listening, a link is an event instead of a queue entry.
    #[test]
    fn a_link_that_arrives_while_the_app_runs_is_emitted() {
        let mut registry = ModuleRegistry::new();
        let links = DeepLinks::new();
        links.attach(registry.emitter(DEEP_LINK_MODULE));

        // Before the handover it still queues: the emitter exists but nobody
        // has subscribed on the other side.
        links.open("myapp://ship/1");
        assert!(registry.drain_events().is_empty(), "it must not be emitted yet");
        assert_eq!(links.take(), vec!["myapp://ship/1".to_owned()]);

        links.open("myapp://ship/3");
        let events = registry.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, DEEP_LINK_MODULE);
        assert_eq!(events[0].1, DEEP_LINK_EVENT);
        assert_eq!(events[0].2, json!({ "url": "myapp://ship/3" }));
        // And nothing was left in the queue for the next `take` to replay.
        assert_eq!(links.waiting(), 0);
    }

    /// A new engine —a restart, or a hot reload that could not be stitched—
    /// starts over: the app inside it has subscribed to nothing.
    #[test]
    fn a_new_engine_goes_back_to_queueing_and_keeps_what_was_waiting() {
        let mut first = ModuleRegistry::new();
        let links = DeepLinks::new();
        links.attach(first.emitter(DEEP_LINK_MODULE));
        links.take();

        // Arrives with the old engine listening; the engine is then thrown away
        // before the frame that would have delivered it.
        links.open("myapp://ship/4");
        assert_eq!(first.drain_events().len(), 1);

        let mut second = ModuleRegistry::new();
        links.attach(second.emitter(DEEP_LINK_MODULE));
        links.open("myapp://ship/5");
        assert!(second.drain_events().is_empty(), "the new app has not subscribed yet");
        assert_eq!(links.take(), vec!["myapp://ship/5".to_owned()]);
    }

    /// Some launch paths hand over an empty string when there was no link.
    /// Navigating to `/` because of one would undo whatever was restored.
    #[test]
    fn an_empty_url_is_not_a_link() {
        let links = DeepLinks::new();
        links.open("");
        links.open("   ");
        assert_eq!(links.waiting(), 0);
    }
}
