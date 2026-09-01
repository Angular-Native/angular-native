#!/usr/bin/env bash
# Controles del sistema: que existan, que midan lo suyo y que se coloquen.
#
# Los tamaños son los que devuelve el medidor aproximado, no los de la
# plataforma: lo que se comprueba aquí es que cada control se mide como control
# y no como caja vacía.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/controls >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/controls/main.js 3 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

echo "== controls"
check 'Switch#[0-9]+ \[[0-9]+,[0-9]+ 51x31\]' 'el interruptor mide lo que mide un interruptor'
check 'Slider#[0-9]+ \[[0-9]+,[0-9]+ 361x32\]' 'el deslizador estira a lo ancho y mantiene su alto'
check 'ProgressBar#[0-9]+ \[[0-9]+,[0-9]+ 361x4\]' 'la barra de progreso también estira'
check 'ActivityIndicator#[0-9]+ \[[0-9]+,[0-9]+ 20x20\]' 'la ruedecilla tiene su tamaño propio'
check 'Button#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x44\]' 'el botón tiene el alto de un botón'
check 'TabBar#[0-9]+ \[0,803 393x49\]' 'la barra de pestañas se pega abajo con su alto'
check 'Modal#[0-9]+ \[0,0 393x852\]' 'el modal cubre la pantalla'
# Que las props lleguen, no solo que el control se monte. El volcado las
# imprime porque una prop que llega y una que se pierde se ven igual en el
# árbol: ninguna de las dos cambia el marco.
check 'Switch#[0-9]+ .*props color=#6ee7b7 on=true' 'el interruptor recibe su estado y su color'
check 'ProgressBar#[0-9]+ .*props color=#6ee7b7 progress=0.35' 'la barra recibe el progreso calculado'
# Los iconos: que midan lo suyo. El 24 es el de por defecto, sin `[size]`; los
# otros vienen del tamaño pedido, que además elige el trazo del símbolo.
check 'Icon#[0-9]+ \[[0-9]+,[0-9]+ 24x24\]' 'un icono sin medidas mide 24'
check 'Icon#[0-9]+ \[[0-9]+,[0-9]+ 40x40\]' 'y con [size] mide lo que se le pide'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
