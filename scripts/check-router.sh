#!/usr/bin/env bash
# Router de Angular sobre una pila de navegación en memoria.
#
# El toque simulado en la primera tarjeta tiene que llevar a la ficha, con el
# parámetro de ruta ya enlazado al `input()` del componente.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/router >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/router/main.js 4 200 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

echo "== router"
check '"Sirena"' 'la ficha se montó tras el toque'
check '"Puerto base: Ibiza"' 'el parámetro :id llegó al input del componente'
check '"‹ Volver"' 'la página de detalle trae su botón de volver'
if grep -qE -- 'búfer inválido|promesa rechazada|el arranque falló' <<<"$OUTPUT"; then
  echo "  FALLO hubo errores durante la navegación"
  fail=1
else
  echo "  ok   ni errores de protocolo ni promesas colgando"
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
