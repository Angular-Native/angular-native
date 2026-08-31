// Superficie C de angular-native. Generada a mano; cuando exista `an-cli`
// pasará a generarse en el build.
#ifndef ANGULAR_NATIVE_H
#define ANGULAR_NATIVE_H

#include <stdint.h>

typedef struct AnRuntime AnRuntime;

/// `container` es el UIView del que cuelga la raíz del árbol.
AnRuntime *an_runtime_new(void *container, float width, float height);

/// Evalúa un script. 0 si fue bien, -1 si JS lanzó.
int32_t an_runtime_eval(AnRuntime *rt, const char *name, const char *code);

/// Recarga en caliente: descarta vistas, motor y árbol, y evalúa código nuevo.
int32_t an_runtime_reload(AnRuntime *rt, const char *name, const char *code);

/// Árbol de demostración construido desde Rust, para aislar fallos del puente.
void an_runtime_load_demo(AnRuntime *rt);

void an_runtime_set_viewport(AnRuntime *rt, float width, float height);

/// Un frame: eventos, turno de JS, mutaciones, layout y montaje.
/// `now_ms` es la marca de tiempo del CADisplayLink.
/// Devuelve las operaciones nativas aplicadas, o -1 si algo falló.
int32_t an_runtime_frame(AnRuntime *rt, double now_ms);

void an_runtime_free(AnRuntime *rt);

#endif
