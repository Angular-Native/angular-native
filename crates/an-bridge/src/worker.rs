//! The JS engine and the tree, on a thread of their own.
//!
//! Not for parallelism: for the stack. QuickJS needs some 4 MB for Angular's
//! router to complete one navigation —seventeen chained RxJS operators and a
//! deep subscription recursion— and iOS's main thread has 1 MB that cannot be
//! changed. A thread of its own will take whatever stack it is asked for.
//!
//! The `Tick` is sent and waited on, but with a deadline. If the JS turn fits in
//! what is left of the frame —the normal case— it mounts in that same frame and
//! there is no latency added. If it runs past the deadline, the UI thread
//! carries on and mounts that frame when it comes, without freezing.
//!
//! It is the middle ground between always blocking, which freezes the interface
//! whenever Angular takes its time, and never waiting, which adds a frame of
//! latency to every touch even when the turn took two milliseconds.
//!
//! Control operations —evaluating, reloading, changing the viewport— do wait:
//! they are rare and the order matters.

use std::cell::Cell;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

use an_core::{Frame, TextMeasurer};
use an_host::{HostEvent, ShadowSide};

use crate::protocol::apply;
use crate::quickjs::QuickJsRuntime;
use crate::runtime::{JsError, JsRuntime};

/// What the UI thread asks the worker for.
pub enum Request {
    Eval { name: String, code: String },
    /// Views out, new engine, empty tree, and evaluate from scratch.
    Reload { name: String, code: String },
    SetViewport(f32, f32),
    Tick { now_ms: f64, events: Vec<HostEvent> },
    Stop,
}

/// What comes back. A `Tick` brings the frame; the rest only say whether there
/// was an error.
#[derive(Default)]
pub struct Reply {
    pub frame: Frame,
    pub error: Option<String>,
    /// Only `Reload` looks at it: it says whether the app was stitched back
    /// together hot. When it is `true` the native views still hold and there is
    /// no need to unmount them.
    pub hot: bool,
}

pub struct RuntimeWorker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    handle: Option<JoinHandle<()>>,
    /// Requests sent and still unanswered. It is what keeps a new `Tick` from
    /// being queued on top of one that has not finished: if JS is running slow,
    /// the queue would grow without end and every mounted frame would be older
    /// than the last.
    in_flight: Cell<usize>,
}

impl RuntimeWorker {
    /// `init` runs inside the thread already: QuickJS's runtime is not `Send`,
    /// so it cannot be built outside and moved. What does cross over are the
    /// pieces it is built from.
    pub fn spawn<M, F>(stack_size: usize, init: F) -> Result<Self, JsError>
    where
        M: TextMeasurer + 'static,
        F: FnOnce() -> Result<(QuickJsRuntime, ShadowSide<M>), JsError> + Send + 'static,
    {
        let (request_tx, request_rx) = std::sync::mpsc::channel::<Request>();
        let (reply_tx, reply_rx) = std::sync::mpsc::channel::<Reply>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        let handle = std::thread::Builder::new()
            .name("an-runtime".to_owned())
            .stack_size(stack_size)
            .spawn(move || {
                let (mut js, mut shadow) = match init() {
                    Ok(parts) => {
                        let _ = ready_tx.send(Ok(()));
                        parts
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                // The `layout` events a frame produces are dispatched at the
                // start of the next one, without crossing back to the UI thread:
                // the worker holds both ends.
                let mut pending_layout: Vec<HostEvent> = Vec::new();

                while let Ok(request) = request_rx.recv() {
                    let reply = match request {
                        Request::Stop => break,
                        Request::Eval { name, code } => Reply {
                            error: js.eval(&name, &code).err().map(|e| e.to_string()),
                            ..Reply::default()
                        },
                        Request::Reload { name, code } => {
                            // Hot is tried first: if the new bundle fits what
                            // is already mounted, the components get their
                            // definitions swapped and the instances stay alive,
                            // state and all. The tree is left alone: whatever
                            // Angular redoes goes out through the command buffer
                            // like any other change.
                            if matches!(js.eval_hot(&name, &code), Ok(true)) {
                                Reply { hot: true, ..Reply::default() }
                            } else {
                                // Whatever state the app wants kept is asked for
                                // before the engine is thrown away and handed
                                // back to the new one before anything is
                                // evaluated: the components read it while they
                                // are being built.
                                let state = js.take_hot_state();
                                shadow.reset();
                                pending_layout.clear();
                                match QuickJsRuntime::new() {
                                    Ok(fresh) => {
                                        js = fresh;
                                        let restored = js.restore_hot_state(&state);
                                        let mut error = restored.err().map(|e| e.to_string());
                                        if error.is_none() {
                                            let evaluated = js.eval(&name, &code);
                                            error = evaluated.err().map(|e| e.to_string());
                                        }
                                        Reply { error, ..Reply::default() }
                                    }
                                    Err(error) => {
                                        Reply { error: Some(error.to_string()), ..Reply::default() }
                                    }
                                }
                            }
                        }
                        Request::SetViewport(width, height) => {
                            shadow.set_viewport((width, height));
                            Reply::default()
                        }
                        Request::Tick { now_ms, mut events } => {
                            events.append(&mut pending_layout);
                            let mut error = None;
                            if !events.is_empty() {
                                error = js.dispatch_events(&events).err().map(|e| e.to_string());
                            }
                            if error.is_none() {
                                match js.tick(now_ms) {
                                    Ok(commands) if commands.is_empty() => {}
                                    Ok(commands) => {
                                        if let Err(protocol) = apply(&commands, &mut shadow.tree) {
                                            error = Some(format!(
                                                "invalid command buffer: {protocol:?}"
                                            ));
                                        }
                                    }
                                    Err(js_error) => error = Some(js_error.to_string()),
                                }
                            }
                            match shadow.commit() {
                                Ok((frame, layout_events)) => {
                                    pending_layout = layout_events;
                                    Reply { frame, error, hot: false }
                                }
                                Err(commit) => Reply {
                                    error: Some(
                                        error.unwrap_or_else(|| format!("the commit failed: {commit:?}")),
                                    ),
                                    ..Reply::default()
                                },
                            }
                        }
                    };
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| JsError::Engine(format!("the runtime thread could not be created: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(RuntimeWorker {
                requests: request_tx,
                replies: reply_rx,
                handle: Some(handle),
                in_flight: Cell::new(0),
            }),
            Ok(Err(message)) => Err(JsError::Engine(message)),
            Err(_) => Err(JsError::Engine("the runtime thread died on startup".to_owned())),
        }
    }

    /// Sends without waiting. Returns `false` if the worker is already busy or
    /// not answering; the caller decides whether that matters.
    pub fn post(&self, request: Request) -> bool {
        if self.requests.send(request).is_err() {
            return false;
        }
        self.in_flight.set(self.in_flight.get() + 1);
        true
    }

    /// An answer if there is one. It does not block.
    pub fn try_reply(&self) -> Option<Reply> {
        match self.replies.try_recv() {
            Ok(reply) => {
                self.in_flight.set(self.in_flight.get().saturating_sub(1));
                Some(reply)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.in_flight.set(0);
                Some(Reply {
                    error: Some("the runtime thread died".to_owned()),
                    ..Reply::default()
                })
            }
        }
    }

    /// Waits for the next answer until the deadline runs out.
    ///
    /// Returns `None` if there is nothing in flight or if the deadline passed:
    /// in that case the request is still alive and its answer will be picked up
    /// later with `try_reply`.
    pub fn wait_reply_until(&self, deadline: Duration) -> Option<Reply> {
        if self.in_flight.get() == 0 {
            return None;
        }
        match self.replies.recv_timeout(deadline) {
            Ok(reply) => {
                self.in_flight.set(self.in_flight.get().saturating_sub(1));
                Some(reply)
            }
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                self.in_flight.set(0);
                Some(Reply {
                    error: Some("the runtime thread died".to_owned()),
                    ..Reply::default()
                })
            }
        }
    }

    /// Waits for the next pending answer, with no deadline.
    pub fn wait_reply(&self) -> Option<Reply> {
        if self.in_flight.get() == 0 {
            return None;
        }
        let reply = self.replies.recv().unwrap_or_else(|_| Reply {
            error: Some("the runtime thread died".to_owned()),
            ..Reply::default()
        });
        self.in_flight.set(self.in_flight.get().saturating_sub(1));
        Some(reply)
    }

    pub fn busy(&self) -> bool {
        self.in_flight.get() > 0
    }

    /// Sends and waits. Control operations only: the caller has to have drained
    /// whatever was in flight first, or it will pick up the wrong answer.
    pub fn request(&self, request: Request) -> Reply {
        if !self.post(request) {
            return Reply {
                error: Some("the runtime thread is not responding".to_owned()),
                ..Reply::default()
            };
        }
        self.wait_reply().unwrap_or_else(|| Reply {
            error: Some("the runtime thread died".to_owned()),
            ..Reply::default()
        })
    }
}

impl Drop for RuntimeWorker {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
