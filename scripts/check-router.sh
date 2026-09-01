#!/usr/bin/env bash
# Router de Angular sobre una pila de navegación en memoria.
#
# El toque simulado en la primera tarjeta tiene que llevar a la ficha, con el
# parámetro de ruta ya enlazado al `input()` del componente.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/router >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/router/main.js 6 200 2>&1)"

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
check 'StackView#[0-9]+' 'la pila nativa se montó'
check '\-\- atrás simulado' 'el gesto de volver atrás tiene quien lo escuche'
check '"Barcos"' 'tras volver atrás se ve otra vez la lista'
# Los ids son los del primer montaje: si la pantalla se hubiera rehecho, el
# core habría repartido ids nuevos y más altos.
check 'Text#8 .*"Barcos"' 'la pantalla anterior se reatachó, no se rehizo'
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
