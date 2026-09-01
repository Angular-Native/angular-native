#!/usr/bin/env bash
# Gestos y transformaciones.
#
# Lo que se comprueba es la cadena entera sin dispositivo: el reconocedor se
# engancha, el evento llega con sus campos, la señal cambia, y la
# transformación sale hacia el host *sin* mover el marco.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/gestures >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/gestures/main.js 6 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

check -- "-- arrastre simulado" "el reconocedor de arrastre se engancha"
check "translateX=60.00" "el desplazamiento del dedo llega a la transformación"
check "translateY=25.00" "y en los dos ejes"
check "soltado a 120 pt/s" "la velocidad llega a la plantilla al soltar"
# El marco es el que le dio el layout, sin sumarle la transformación: si
# `translateX` entrara en el layout, este número sería otro.
check "View#8 \[107,90 140x140\]" "transformar no mueve el marco ni relanza el layout"
check "scale=1.00" "la escala arranca en 1, no en 0"
# `[style.fontSize]` es el camino que Angular deja escribir siempre y que
# antes se perdía por el camino: llegaba con guion, nadie lo reconocía, y el
# texto se medía con la letra por defecto. 23 de alto es 18 puntos; 17 sería
# la de por defecto.
check "Text#12 \[.* 149x23\]" "un [style.fontSize] llega a la medición"

if [[ $fail -ne 0 ]]; then
  echo "$OUTPUT"
  exit 1
fi
