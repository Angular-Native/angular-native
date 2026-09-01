#!/usr/bin/env bash
# Cabecera, texto de varias líneas, navegador embebido y hoja de acciones.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/web >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/web/main.js 4 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

check 'NavigationBar#[0-9]+ \[0,0 393x44\]' 'la cabecera se pega arriba con su alto'
check 'TextEditor#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x110\]' 'el editor de varias líneas se monta'
# `flex: 1` es el atajo que casi todo el mundo escribe. Antes no existía como
# propiedad —solo como valor de `display`— y se perdía por el camino: la vista
# se quedaba con el alto de su contenido en vez de repartirse lo que sobra.
check 'WebView#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x568\]' 'el navegador se reparte el hueco con flex'
check 'Alert#[0-9]+ \[0,0 0x0\]' 'la hoja de acciones no ocupa sitio en el layout'
check 'último: atrás' 'el botón de atrás de la cabecera avisa'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
