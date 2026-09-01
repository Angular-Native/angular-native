#!/usr/bin/env bash
# Los controles de elegir: segmentos, desplegable, pasos, búsqueda y fecha.
#
# Lo que se comprueba aquí es que cada uno se monta como su propia clase de
# nodo y mide lo suyo, no que se vea bien: eso solo se ve en el dispositivo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/pickers >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/pickers/main.js 4 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

check 'SearchBar#[0-9]+ \[' 'la barra de búsqueda se monta'
check 'SegmentedControl#[0-9]+ \[' 'el control segmentado se monta'
check 'Picker#[0-9]+ \[[0-9]+,[0-9]+ 150x40\]' 'el desplegable se monta con su tamaño'
check 'Stepper#[0-9]+ \[[0-9]+,[0-9]+ 140x40\]' 'el de pasos se monta con su tamaño'
check 'DatePicker#[0-9]+ \[[0-9]+,[0-9]+ 180x40\]' 'el de fecha se monta con su tamaño'
# El área segura ya no mete una vista de por medio, así que lo que se le pone
# para ordenar a los hijos les llega: 20 de arriba, 33 de alto, 18 de hueco.
check 'SearchBar#[0-9]+ \[20,71' 'el hueco del área segura separa a sus hijos'
check 'último botón: texto' 'el toque llega al botón de variante texto'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
