#!/usr/bin/env bash
# La app de lista: campo de texto, ScrollView y lista con ventana.
#
# Lo que verifica de verdad es que cinco mil filas no son cinco mil vistas.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/kitchen >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/kitchen/main.js 6 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

echo "== kitchen"
check 'TextInput#[0-9]+ \[16,70 361x40\]' 'el campo de texto se midió y se colocó'
check '"headless 0.0 . es-ES"' 'el módulo nativo contestó y la promesa resolvió'
check 'ScrollView#[0-9]+ \[0,0 393x666\]' 'el ScrollView llena el hueco, no crece con su contenido'
check 'contenido 393x280000' 'el contentSize sale del layout: 5000 filas de 56'
check '"fila número 7[0-9]"' 'tras desplazarse se ven las filas de esa altura'
check 'desplazarse costó 0 vistas creadas y 0 destruidas' 'desplazarse recicla: ni una vista nueva'
if grep -qE -- '"fila número (1|2|300)"' <<<"$OUTPUT"; then
  echo "  FALLO tras desplazarse no debería quedar nada del principio ni del final"
  fail=1
else
  echo "  ok   fuera de la ventana no se monta nada"
fi
mounted="$(grep -oE 'vistas nativas montadas: [0-9]+' <<<"$OUTPUT" | grep -oE '[0-9]+')"
if [ -n "$mounted" ] && [ "$mounted" -lt 120 ]; then
  echo "  ok   $mounted vistas nativas para 5000 filas"
else
  echo "  FALLO demasiadas vistas montadas: ${mounted:-?}"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
