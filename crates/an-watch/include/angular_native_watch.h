// The C surface of the watchOS host.
//
// It looks like the iOS one, but it mounts no views: the SwiftUI shell asks
// for the tree and paints it itself. See `crates/an-watch/src/ffi.rs`.
#ifndef ANGULAR_NATIVE_WATCH_H
#define ANGULAR_NATIVE_WATCH_H

#include <stdint.h>

typedef struct AnWatchRuntime AnWatchRuntime;

/// Starts the engine.
///
/// `control_json` holds the controls' natural sizes as measured by SwiftUI,
/// like {"Button":[80,44]}; it may be NULL.
///
/// `device_json` is what Device.info() answers, minus the platform, which the
/// host decides: {"systemVersion":..,"model":..,"scale":..,"locale":..}. It
/// comes from the shell because those four are WatchKit's and there is no way
/// to them from Rust. It may be NULL, and then the call is rejected saying so.
AnWatchRuntime *an_watch_runtime_new(float width, float height,
                                     const char *control_json,
                                     const char *device_json);

/// Evaluates a script. 0 if it went well, -1 if JS threw.
int32_t an_watch_runtime_eval(AnWatchRuntime *rt, const char *name,
                              const char *code);

/// Puts new code into the app already running: what `an dev` uses on save.
/// If the new bundle fits what is mounted, the state is kept; if it does not,
/// everything is stood up again. 0 if it went well, -1 if it failed.
int32_t an_watch_runtime_reload(AnWatchRuntime *rt, const char *name,
                                const char *code);

void an_watch_runtime_set_viewport(AnWatchRuntime *rt, float width,
                                   float height);

/// One frame: events, JS's turn, layout and mutation of the model. Returns the
/// operations applied, or -1 if something failed.
int32_t an_watch_runtime_frame(AnWatchRuntime *rt, double now_ms);

/// Rises only when a frame brought changes. The shell compares it with the one
/// it already has and only asks for the snapshot when they differ.
uint64_t an_watch_runtime_revision(AnWatchRuntime *rt);

/// The tree as JSON. Valid until the next call to this same function.
const char *an_watch_runtime_snapshot(AnWatchRuntime *rt);

/// A native event from SwiftUI, "press" or "crown" for instance.
///
/// `payload_json` is a flat object —{"value":0.4}— or NULL if the event
/// carries nothing. Only numbers, strings and booleans are accepted: anything
/// nested is rejected with a warning, because the bridge cannot carry it.
void an_watch_runtime_event(AnWatchRuntime *rt, uint32_t target,
                            const char *name, const char *payload_json);

void an_watch_runtime_free(AnWatchRuntime *rt);

// --- Plugins ---------------------------------------------------------------
//
// The same quartet the phone has, with this host's prefix. Registration happens
// **before** an_watch_runtime_new: the core builds one module per registered
// name when the engine starts, and a name arriving afterwards would never get
// in.
//
// What a watch can and cannot do is not decided here. A plugin only reaches
// this registry if it declared a watchOS half in its package.json, and `an
// watchos` stops the build of an app whose plugins did not.

/// Registers a plugin under the name JS calls it by.
void an_watch_plugin_register(const char *name);

/// Installs the callback the core hands the queued calls to, once per frame and
/// on the main actor. NULL takes it away.
void an_watch_plugin_set_dispatch(void (*dispatch)(uint64_t id,
                                                   const char *module,
                                                   const char *method,
                                                   const char *args));

/// Answers a call with its return value, already serialised; "null" for a
/// method that returns nothing. 0 if the call was waiting, -1 if it was not.
int32_t an_watch_plugin_resolve(uint64_t id, const char *json);

/// Rejects a call. 0 if the call was waiting, -1 if it was not.
int32_t an_watch_plugin_reject(uint64_t id, const char *message);

/// Emits an event from a plugin, under the module's own name. Unlike an answer
/// this belongs to no call: it may arrive at any time, or never, and nothing on
/// the JS side is waiting for it. Returns 0 if the module is registered.
int32_t an_watch_plugin_emit(const char *module, const char *event, const char *json);
// ── Built-in modules ────────────────────────────────────────────────────────
//
// The modules the framework brings: files, share, network status and haptics.
// They are not plugins —nobody declares them, there is no npm package and no
// host can be without them— but they travel the same way, because what they
// call belongs to the main thread. See `crates/an-bridge/src/builtins.rs`.
//
// This block is written the same in the three Apple headers, which are never
// imported together: `scripts/check-builtins.sh` is what keeps the three
// copies from drifting apart.
//
// Like the plugins', all of this is global to the process and not to the
// runtime: it is installed before the runtime is created and survives a hot
// restart.

/// Installs the dispatcher Rust hands every built-in call to. The strings only
/// hold for the duration of the call: they have to be copied. `NULL`
/// uninstalls it.
typedef void (*AnBuiltinDispatch)(uint64_t id, const char *module,
                                  const char *method, const char *args);
void an_builtin_set_dispatch(AnBuiltinDispatch dispatch);

/// Answers a call. `json` is the return value, already serialised ("null" for
/// a method that returns nothing). 0 if the call existed.
int32_t an_builtin_resolve(uint64_t id, const char *json);

/// Rejects a call. 0 if the call existed.
int32_t an_builtin_reject(uint64_t id, const char *message);

#endif
