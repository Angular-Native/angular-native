// Superficie C de angular-native. Generada a mano; cuando exista `an-cli`
// pasará a generarse en el build.
#ifndef ANGULAR_NATIVE_H
#define ANGULAR_NATIVE_H

#include <stdint.h>

typedef struct AnRuntime AnRuntime;

/// `container` es el UIView del que cuelga la raíz del árbol.
AnRuntime *an_runtime_new(void *container, float width, float height);

/// Árbol de demostración, hasta que exista el puente JS.
void an_runtime_load_demo(AnRuntime *rt);

void an_runtime_set_viewport(AnRuntime *rt, float width, float height);

/// Operaciones aplicadas en este frame, o -1 si el commit falló.
int32_t an_runtime_frame(AnRuntime *rt);

void an_runtime_free(AnRuntime *rt);

#endif
