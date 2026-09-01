#!/usr/bin/env bash
# Refresco en caliente: guardar un fichero cambia la pantalla sin reiniciar.
#
# Se compilan dos bundles del mismo ejemplo, uno con la plantilla cambiada, y
# se evalúa el segundo encima del primero cuando ya hay estado que perder: un
# toque contado, un temporizador andando y un `@if` abierto. Lo que se verifica
# es que el cambio entra y que ese estado sigue ahí.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FUENTE="examples/hello-angular/src/app.component.ts"
COPIA="$(mktemp)"
ANTES="$(mktemp)"
DESPUES="$(mktemp)"
cp "$FUENTE" "$COPIA"
# Pase lo que pase, el ejemplo se queda como estaba.
trap 'cp "$COPIA" "$FUENTE"; rm -f "$COPIA" "$ANTES" "$DESPUES"' EXIT

cargo an build examples/hello-angular >/dev/null
cp build/bundle/hello-angular/main.js "$ANTES"

sed -i '' 's/Esto es una plantilla de Angular con señales, corriendo en QuickJS./PLANTILLA CAMBIADA EN CALIENTE./' "$FUENTE"
cargo an build examples/hello-angular >/dev/null
cp build/bundle/hello-angular/main.js "$DESPUES"
cp "$COPIA" "$FUENTE"

OUTPUT="$(AN_HOT="$DESPUES" cargo run -q -p an-bridge --example headless -- "$ANTES" 6 2>&1)"

# Si el runner no trae el soporte de `AN_HOT` no hay refresco que medir, y lo
# que sale son cuatro fallos que no dicen nada. Suele pasar por un binario que
# cargo dio por bueno sin serlo: `touch` al fuente y a compilar otra vez.
if ! grep -q 'refresco en caliente' <<<"$OUTPUT"; then
  echo "  FALLO el runner headless ignoró AN_HOT; recompila: cargo build -p an-bridge --example headless"
  exit 1
fi

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

echo "== refresco en caliente"
check 'refresco en caliente: sí' 'el bundle nuevo se cosió sobre el que ya corría'
check 'PLANTILLA CAMBIADA EN CALIENTE' 'la plantilla nueva está en pantalla'
check '"toques: 1 \(último en 40, 20\)"' 'el estado del componente sobrevivió'
check '"segundos en marcha: 5"' 'el temporizador siguió corriendo, no volvió a cero'
check 'El @if entró a los 3 segundos' 'lo que ya se había desplegado sigue desplegado'
if grep -qE -- 'Esto es una plantilla de Angular' <<<"$OUTPUT"; then
  echo "  FALLO la pantalla vieja se quedó montada debajo de la nueva"
  fail=1
else
  echo "  ok   la pantalla vieja se desmontó"
fi

# La mitad de arriba del bundle —Angular y el framework— no puede cambiarse en
# caliente: hay una sola copia en el intérprete. Cuando cambia, lo honesto es
# pedir el reinicio, y eso es lo que se comprueba aquí falseando la firma.
sed 's/globalThis.__anVendor !== "/globalThis.__anVendor !== "x/' "$DESPUES" >"$DESPUES.otro"
OTRO="$(AN_HOT="$DESPUES.otro" cargo run -q -p an-bridge --example headless -- "$ANTES" 6 2>&1)"
if grep -qE -- 'refresco en caliente: no, toca reiniciar' <<<"$OTRO"; then
  echo "  ok   si cambia el framework se pide reinicio en vez de mentir"
else
  echo "  FALLO cambiar el framework tendría que forzar el reinicio"
  fail=1
fi
rm -f "$DESPUES.otro"

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
