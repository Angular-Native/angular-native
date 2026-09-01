#!/usr/bin/env bash
# El host del reloj: que cruza-compile y que el modelo que ve SwiftUI sea el
# que el layout calculó.
#
# La compilación cruzada va aparte de las otras dos porque necesita nightly:
# `aarch64-apple-watchos-sim` es un target de nivel 3 y no trae `std`
# precompilada, así que hay que construirla en el momento con `-Z build-std`.
# Si no está nightly, esto avisa y no falla: el resto de las comprobaciones no
# tienen por qué caerse porque a alguien le falte un toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== watchOS"

# 1. El modelo. Corre en el Mac y no necesita ni simulador ni nightly: el host
#    aplica MountOp sobre una estructura de datos, y eso se comprueba aquí.
if cargo test --quiet -p an-watch >/dev/null 2>&1; then
  echo "  ok   el modelo que ve SwiftUI se construye como debe"
else
  echo "  FALLO los tests de an-watch no pasan"
  cargo test -p an-watch 2>&1 | tail -20
  fail=1
fi

# 2. El ejemplo del reloj, montado con el viewport de un Series 11 de 46 mm.
#    Sin esto un cambio en las primitivas podría dejar la app del reloj sin
#    pintar y nadie se enteraría hasta abrir el simulador.
cargo an build examples/hello-watch >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/hello-watch/main.js 4 2>&1)"

check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

check 'ScrollView#[0-9]+ .*contenido' 'el ScrollView declara su contentSize'
check 'Text#[0-9]+ .*"angular-native"' 'la cabecera se midió y se colocó'
check 'Button#[0-9]+' 'el botón del sistema está montado'
check '"toques: 1"' 'el toque llegó a JS y la señal se recomputó'

# 3. La compilación cruzada, que es la que cuesta y la que puede no estar.
if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "  --   compilación cruzada omitida: falta el toolchain nightly"
  echo "       rustup toolchain install nightly"
  echo "       rustup component add rust-src --toolchain nightly"
elif ! rustup component list --toolchain nightly 2>/dev/null | grep -q 'rust-src (installed)'; then
  echo "  --   compilación cruzada omitida: falta rust-src en nightly"
  echo "       rustup component add rust-src --toolchain nightly"
else
  if cargo +nightly build --quiet -Z build-std=std,panic_abort \
      -p an-watch --target aarch64-apple-watchos-sim 2>/dev/null; then
    echo "  ok   an-watch para aarch64-apple-watchos-sim"
  else
    echo "  FALLO an-watch no cruza-compila para aarch64-apple-watchos-sim"
    fail=1
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
