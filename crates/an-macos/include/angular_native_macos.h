// The C surface of the macOS host. Written by hand, just like the iOS one.
//
// It is the same as the phone's, plugins included: the four `an_plugin_*`
// functions at the bottom are the same quartet `angular_native.h` declares, so
// the Mac's `AnPluginRegistry` is the phone's with `NSViewController` where it
// said `UIViewController`.
#ifndef ANGULAR_NATIVE_MACOS_H
#define ANGULAR_NATIVE_MACOS_H

#include <stdint.h>

typedef struct AnRuntime AnRuntime;

/// `container` is the NSView the root of the tree hangs off.
AnRuntime *an_runtime_new(void *container, float width, float height);

/// Evaluates a script. 0 if it went well, -1 if JS threw.
int32_t an_runtime_eval(AnRuntime *rt, const char *name, const char *code);

/// Hot reload. If the new bundle fits what is mounted, the views are not
/// touched and the state survives; if it does not, everything is stood up
/// again.
int32_t an_runtime_reload(AnRuntime *rt, const char *name, const char *code);

/// The window changed size. On the desktop this is called very often —while a
/// corner is being dragged— so the Rust side drops the repeats and the shell
/// can call it on every `layout()` without thinking about it.
void an_runtime_set_viewport(AnRuntime *rt, float width, float height);

/// One frame: events, JS's turn, mutations, layout and mounting.
/// `now_ms` is the CADisplayLink's timestamp.
/// Returns the native operations applied, or -1 if something failed.
int32_t an_runtime_frame(AnRuntime *rt, double now_ms);

void an_runtime_free(AnRuntime *rt);

// --- Plugins ---------------------------------------------------------------
//
// Registration happens **before** an_runtime_new: the core builds one module
// per registered name when the engine starts, and a name arriving afterwards
// would never get in.

/// Registers a plugin under the name JS calls it by.
void an_plugin_register(const char *name);

/// Installs the callback the core hands the queued calls to, once per frame
/// and on the main thread. NULL takes it away.
void an_plugin_set_dispatch(void (*dispatch)(uint64_t id, const char *module,
                                             const char *method,
                                             const char *args));

/// Answers a call with its return value, already serialised; "null" for a
/// method that returns nothing. 0 if the call was waiting, -1 if it was not.
int32_t an_plugin_resolve(uint64_t id, const char *json);

/// Rejects a call. 0 if the call was waiting, -1 if it was not.
int32_t an_plugin_reject(uint64_t id, const char *message);
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
