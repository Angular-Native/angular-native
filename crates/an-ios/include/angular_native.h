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

#endif
