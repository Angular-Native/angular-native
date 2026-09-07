// angular-native's C surface. Written by hand; once `an-cli` exists it will be
// generated in the build.
#ifndef ANGULAR_NATIVE_H
#define ANGULAR_NATIVE_H

#include <stdint.h>

typedef struct AnRuntime AnRuntime;

/// `container` is the UIView the root of the tree hangs off.
AnRuntime *an_runtime_new(void *container, float width, float height);

/// Evaluates a script. 0 if it went well, -1 if JS threw.
int32_t an_runtime_eval(AnRuntime *rt, const char *name, const char *code);

/// Hot reload: throws views, engine and tree away, and evaluates new code.
int32_t an_runtime_reload(AnRuntime *rt, const char *name, const char *code);

void an_runtime_set_viewport(AnRuntime *rt, float width, float height);

/// One frame: events, JS's turn, mutations, layout and mounting.
/// `now_ms` is the CADisplayLink's timestamp.
/// Returns the native operations applied, or -1 if something failed.
int32_t an_runtime_frame(AnRuntime *rt, double now_ms);

void an_runtime_free(AnRuntime *rt);

// ── Plugins ─────────────────────────────────────────────────────────────────
//
// A plugin is Swift that an npm package brings. The core does not know what
// it does: it carries the call to the main thread and brings the answer back.
// All of this is global to the process, not to the runtime: it is registered
// before `an_runtime_new`.

/// Registers a plugin under the name JS invokes it by.
void an_plugin_register(const char *name);

/// Installs the dispatcher Rust hands every call to. The strings only hold
/// for the duration of the call: they have to be copied. `NULL` uninstalls
/// it.
typedef void (*AnPluginDispatch)(uint64_t id, const char *module,
                                 const char *method, const char *args);
void an_plugin_set_dispatch(AnPluginDispatch dispatch);

/// Answers a call. `json` is the return value, already serialised ("null" for
/// a method that returns nothing). 0 if the call existed.
int32_t an_plugin_resolve(uint64_t id, const char *json);

/// Rejects a call. 0 if the call existed.
int32_t an_plugin_reject(uint64_t id, const char *message);

/// Emits an event from a plugin, under the module's own name. Unlike an answer
/// this belongs to no call: it may arrive at any time, or never, and nothing on
/// the JS side is waiting for it. Returns 0 if the module is registered.
int32_t an_plugin_emit(const char *module, const char *event, const char *json);

/// Installs the factory that turns the name in `<an-custom [view]>` into a view.
///
/// The shell hands back a `+1` reference — `Unmanaged.passRetained` — and the
/// core takes ownership of it. Returning NULL means no plugin registers that
/// name, and the core says so once rather than mounting an empty box.
void an_plugin_view_set_factory(void *(*factory)(const char *name));

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
