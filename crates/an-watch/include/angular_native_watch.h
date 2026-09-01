// Superficie C del host de watchOS.
//
// Se parece a la de iOS, pero no monta vistas: el shell de SwiftUI pregunta
// por el árbol y lo pinta él. Ver `crates/an-watch/src/ffi.rs`.
#ifndef ANGULAR_NATIVE_WATCH_H
#define ANGULAR_NATIVE_WATCH_H

#include <stdint.h>

typedef struct AnWatchRuntime AnWatchRuntime;

/// Arranca el motor. `control_json` son los tamaños naturales de los controles
/// medidos por SwiftUI, como {"Button":[80,44]}; puede ser NULL.
AnWatchRuntime *an_watch_runtime_new(float width, float height,
                                     const char *control_json);

/// Evalúa un script. 0 si fue bien, -1 si JS lanzó.
int32_t an_watch_runtime_eval(AnWatchRuntime *rt, const char *name,
                              const char *code);

void an_watch_runtime_set_viewport(AnWatchRuntime *rt, float width,
                                   float height);

/// Un frame: eventos, turno de JS, layout y mutación del modelo. Devuelve las
/// operaciones aplicadas, o -1 si algo falló.
int32_t an_watch_runtime_frame(AnWatchRuntime *rt, double now_ms);

/// Sube solo cuando un frame trajo cambios. El shell la compara con la que ya
/// tiene y solo pide la foto si difiere.
uint64_t an_watch_runtime_revision(AnWatchRuntime *rt);

/// El árbol en JSON. Válido hasta la siguiente llamada a esta misma función.
const char *an_watch_runtime_snapshot(AnWatchRuntime *rt);

/// Un evento nativo desde SwiftUI, por ejemplo "press".
void an_watch_runtime_event(AnWatchRuntime *rt, uint32_t target,
                            const char *name);

void an_watch_runtime_free(AnWatchRuntime *rt);

#endif
