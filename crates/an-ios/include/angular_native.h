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

void an_runtime_set_viewport(AnRuntime *rt, float width, float height);

/// Un frame: eventos, turno de JS, mutaciones, layout y montaje.
/// `now_ms` es la marca de tiempo del CADisplayLink.
/// Devuelve las operaciones nativas aplicadas, o -1 si algo falló.
int32_t an_runtime_frame(AnRuntime *rt, double now_ms);

void an_runtime_free(AnRuntime *rt);

// ── Plugins ─────────────────────────────────────────────────────────────────
//
// Un plugin es Swift que trae un paquete npm. El core no sabe qué hace: le
// lleva la llamada al hilo principal y se trae la respuesta. Todo esto es
// global al proceso, no al runtime: se registra antes de `an_runtime_new`.

/// Da de alta un plugin por el nombre con el que JS lo invoca.
void an_plugin_register(const char *name);

/// Instala el despachador al que Rust le pasa cada llamada. Las cadenas solo
/// valen durante la llamada: hay que copiarlas. `NULL` lo desinstala.
typedef void (*AnPluginDispatch)(uint64_t id, const char *module,
                                 const char *method, const char *args);
void an_plugin_set_dispatch(AnPluginDispatch dispatch);

/// Contesta a una llamada. `json` es el valor de vuelta ya serializado
/// ("null" para un método que no devuelve nada). 0 si la llamada existía.
int32_t an_plugin_resolve(uint64_t id, const char *json);

/// Rechaza una llamada. 0 si la llamada existía.
int32_t an_plugin_reject(uint64_t id, const char *message);

#endif
