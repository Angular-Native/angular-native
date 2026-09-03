// The C surface of the watchOS host.
//
// It looks like the iOS one, but it mounts no views: the SwiftUI shell asks
// for the tree and paints it itself. See `crates/an-watch/src/ffi.rs`.
#ifndef ANGULAR_NATIVE_WATCH_H
#define ANGULAR_NATIVE_WATCH_H

#include <stdint.h>

typedef struct AnWatchRuntime AnWatchRuntime;

/// Starts the engine. `control_json` holds the controls' natural sizes as
/// measured by SwiftUI, like {"Button":[80,44]}; it may be NULL.
AnWatchRuntime *an_watch_runtime_new(float width, float height,
                                     const char *control_json);

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

#endif
