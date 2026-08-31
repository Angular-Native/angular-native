#!/usr/bin/env bash
# Prueba de integración de la cadena entera sin simulador:
# TypeScript -> ngc -> esbuild -> QuickJS -> shadow tree -> taffy.
#
# Comprueba la forma del árbol resuelto, no píxeles: lo que se verifica es que
# Angular arrancó, que las señales mueven la pantalla, y que `@for` y `@if`
# producen los nodos nativos correctos.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-examples/hello-angular}"
NAME="$(basename "$APP")"

"$ROOT/scripts/build-js.sh" "$APP" >/dev/null
OUTPUT="$(cd "$ROOT" && cargo run -q -p an-bridge --example headless -- \
  "build/bundle/$NAME/main.js" 6 2>&1)"

fail=0
check() {
  if grep -qE "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

echo "== $NAME"
check 'Angular is running in development mode' 'Angular arrancó'
check 'Text#[0-9]+ .*"angular-native"' 'el título llegó a un nodo Text nativo'
check 'View#5 \[16,115 361x88\]' 'la fila quedó donde toca'
check 'View#10 \[0,0 116x88\]' '@for: primera tarjeta con flexGrow 1'
check 'View#11 \[128,0 233x88\]' '@for: segunda tarjeta con flexGrow 2'
check 'segundos en marcha: 5' 'la señal avanzó cinco frames'
check 'El @if entró a los 3 segundos' '@if montó su rama al pasar el umbral'
if grep -qE -- '-- frame 4 \(t=4000ms\): 1 operaciones' <<<"$OUTPUT"; then
  echo "  ok   un frame estable cuesta una sola operación"
else
  echo "  FALLO un frame estable debería costar una sola operación"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
