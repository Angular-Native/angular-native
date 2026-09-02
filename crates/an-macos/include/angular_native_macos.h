// Superficie C del host de macOS. Escrita a mano, igual que la de iOS.
//
// Es la misma que la del teléfono menos los plugins: este host todavía no los
// carga, y `an macos` para el build antes de compilar si la app depende de
// alguno, en vez de dejar un módulo que se traga las llamadas.
#ifndef ANGULAR_NATIVE_MACOS_H
#define ANGULAR_NATIVE_MACOS_H

#include <stdint.h>

typedef struct AnRuntime AnRuntime;

/// `container` es la NSView de la que cuelga la raíz del árbol.
AnRuntime *an_runtime_new(void *container, float width, float height);

/// Evalúa un script. 0 si fue bien, -1 si JS lanzó.
int32_t an_runtime_eval(AnRuntime *rt, const char *name, const char *code);

/// Refresco en caliente. Si el bundle nuevo encaja con lo montado, las vistas
/// no se tocan y el estado sobrevive; si no, se levanta todo otra vez.
int32_t an_runtime_reload(AnRuntime *rt, const char *name, const char *code);

/// La ventana cambió de tamaño. En escritorio se llama muy a menudo —mientras
/// se arrastra una esquina—, así que el lado de Rust descarta los repetidos y
/// el shell puede llamarla en cada `layout()` sin pensárselo.
void an_runtime_set_viewport(AnRuntime *rt, float width, float height);

/// Un frame: eventos, turno de JS, mutaciones, layout y montaje.
/// `now_ms` es la marca de tiempo del CADisplayLink.
/// Devuelve las operaciones nativas aplicadas, o -1 si algo falló.
int32_t an_runtime_frame(AnRuntime *rt, double now_ms);

void an_runtime_free(AnRuntime *rt);

#endif
