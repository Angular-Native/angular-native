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

correr() {
  AN_HOT="$1" cargo run -q -p an-bridge --example headless -- "$ANTES" 6 2>&1
}

OUTPUT="$(correr "$DESPUES")"

# Si el runner ni menciona el refresco es que no trae el soporte de `AN_HOT`:
# `cargo test` y `cargo run --example` no siempre coinciden en qué binario del
# ejemplo es el bueno, y el que queda puede ser de antes del cambio. Se le
# fuerza la recompilación una vez; si sigue igual, es un fallo de verdad y no
# vale disfrazarlo de refresco que no funciona.
if ! grep -q 'refresco en caliente' <<<"$OUTPUT"; then
  touch crates/an-bridge/examples/headless.rs
  cargo build -q -p an-bridge --example headless
  OUTPUT="$(correr "$DESPUES")"
fi
if ! grep -q 'refresco en caliente' <<<"$OUTPUT"; then
  echo "  FALLO el runner headless no trae el soporte de AN_HOT ni recompilándolo"
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
OTRO="$(correr "$DESPUES.otro")"
if grep -qE -- 'refresco en caliente: no, toca reiniciar' <<<"$OTRO"; then
  echo "  ok   si cambia el framework se pide reinicio en vez de mentir"
else
  echo "  FALLO cambiar el framework tendría que forzar el reinicio"
  fail=1
fi
rm -f "$DESPUES.otro"

# Y el mismo ejercicio con una plantilla que use un componente —no una
# directiva—, porque son dos cosas distintas y solo una se veía.
#
# `hello-angular` es todo primitivas, y una primitiva es una directiva sobre un
# elemento que el core monta igual: si el refresco deja la plantilla sin
# directivas, `[backgroundColor]` sigue llegando como propiedad y la pantalla
# no cambia. Lo que delata el fallo es un componente que hace algo que una
# propiedad no puede hacer —escribir estilos desde su host y proyectar hijos—,
# y eso es `an-safe-area`, que solo sale en `hello-wear`.
FUENTE_W="examples/hello-wear/src/app.component.ts"
COPIA_W="$(mktemp)"
ANTES_W="$(mktemp)"
DESPUES_W="$(mktemp)"
cp "$FUENTE_W" "$COPIA_W"
trap 'cp "$COPIA" "$FUENTE"; cp "$COPIA_W" "$FUENTE_W"; rm -f "$COPIA" "$ANTES" "$DESPUES" "$COPIA_W" "$ANTES_W" "$DESPUES_W"' EXIT

cargo an build examples/hello-wear >/dev/null
cp build/bundle/hello-wear/main.js "$ANTES_W"
sed -i '' 's/Gira la corona: {{ alto() }} pt\./RELOJ CAMBIADO EN CALIENTE./' "$FUENTE_W"
cargo an build examples/hello-wear >/dev/null
cp build/bundle/hello-wear/main.js "$DESPUES_W"
cp "$COPIA_W" "$FUENTE_W"

# En la esfera del emulador, para que las medidas del árbol sean las suyas.
RELOJ="$(AN_VIEWPORT=227x227 AN_HOT="$DESPUES_W" \
  cargo run -q -p an-bridge --example headless -- "$ANTES_W" 6 2>&1)"

checkw() {
  if grep -qE -- "$1" <<<"$RELOJ"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
    RELOJ_MAL=1
  fi
}

checkw 'refresco en caliente: sí' 'el reloj también se cose en caliente'
checkw 'RELOJ CAMBIADO EN CALIENTE' 'la plantilla nueva del reloj está en pantalla'
# El área segura es un componente: sus estilos los escribe su host, no la
# plantilla. Sin ella el desplazable no crece y se queda en 227x0, que es una
# pantalla negra sin un solo error.
checkw 'ScrollView#[0-9]+ \[0,0 227x227\]' 'los estilos del host del área segura siguen puestos'
# Y las entradas de las primitivas siguen siendo entradas de una directiva y no
# propiedades sueltas que casualmente acaban en el mismo sitio.
checkw 'ScrollView#[0-9]+ .*props refreshing=false' 'las primitivas siguen casando como directivas'

if [ "$fail" -ne 0 ]; then
  echo
  if [ -n "${RELOJ_MAL:-}" ]; then
    echo "$RELOJ"
  else
    echo "$OUTPUT"
  fi
  exit 1
fi
