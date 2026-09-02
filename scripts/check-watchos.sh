#!/usr/bin/env bash
# El host del reloj: que cruza-compile, que el modelo que ve SwiftUI sea el que
# el layout calculó, y que las tres listas que tienen que decir lo mismo lo
# digan.
#
# La compilación cruzada va aparte de lo demás porque necesita nightly:
# `aarch64-apple-watchos-sim` es un target de nivel 3 y no trae `std`
# precompilada, así que hay que construirla en el momento con `-Z build-std`.
# Si no está nightly, esto avisa y no falla: el resto de las comprobaciones no
# tienen por qué caerse porque a alguien le falte un toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== watchOS"

# El resultado se pasa ya evaluado y con `&& r=0 || r=1` delante, no con `$?` a
# secas: `set -e` mata el script en cuanto una comprobación devuelve 1, y lo que
# se quiere es que las diga todas y falle al final.
check() { # <0 si bien, 1 si mal> <qué se comprobaba>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

# 1. El modelo. Corre en el Mac y no necesita ni simulador ni nightly: el host
#    aplica MountOp sobre una estructura de datos, y eso se comprueba aquí.
if cargo test --quiet -p an-watch >/dev/null 2>&1; then
  echo "  ok   el modelo que ve SwiftUI se construye como debe"
else
  echo "  FALLO los tests de an-watch no pasan"
  cargo test -p an-watch 2>&1 | tail -20
  fail=1
fi

# 2. Las tres listas de primitivas.
#
#    El vocabulario está en `an-core`, lo que el reloj no pinta está en
#    `an-watch/src/snapshot.rs`, y lo que sí pinta está en Swift. Nada obliga a
#    las tres a decir lo mismo salvo esto: sin ello, añadir una primitiva al
#    núcleo la dejaría en el reloj como un hueco que nadie decidió.
TODAS="$(grep -oE '=> NodeKind::[A-Za-z]+' crates/an-core/src/props.rs \
  | sed 's/.*NodeKind:://' | sort -u)"
SIN_PINTAR="$(awk '/^pub fn unsupported/,/^\}/' crates/an-watch/src/snapshot.rs \
  | grep -oE 'NodeKind::[A-Za-z]+' | sed 's/NodeKind:://' | sort -u)"
# Lo que el shell dibuja: los `case` del `switch` de `AnNodeView`, más `View`,
# que es la rama por defecto, más los dos que presenta el sistema y por eso
# viven en `AnOverlays`.
PINTADAS="$( { awk '/private var content: some View/,/^    \}/' shells/watchos/Sources/AnNodeView.swift \
    | sed -n 's/.*case "\([A-Za-z]*\)".*/\1/p'
  sed -n 's/.*case "\([A-Za-z]*\)".*/\1/p' shells/watchos/Sources/AnOverlays.swift
  echo View; } | sort -u)"

SOLAPE="$(comm -12 <(echo "$SIN_PINTAR") <(echo "$PINTADAS"))"
[ -z "$SOLAPE" ] && r=0 || r=1
check $r "ninguna primitiva se pinta y se descarta a la vez"
if [ -n "$SOLAPE" ]; then echo "       en las dos listas: $(echo "$SOLAPE" | tr '\n' ' ')"; fi

SIN_DECIDIR="$(comm -23 <(echo "$TODAS") <(cat <(echo "$SIN_PINTAR") <(echo "$PINTADAS") | sort -u))"
[ -z "$SIN_DECIDIR" ] && r=0 || r=1
check $r "toda primitiva del núcleo o la pinta el reloj o dice por qué no"
if [ -n "$SIN_DECIDIR" ]; then echo "       sin decidir: $(echo "$SIN_DECIDIR" | tr '\n' ' ')"; fi

# 3. Los gestos. Lo que el shell engancha y lo que el host dice que no llega no
#    pueden solaparse: un gesto que se engancha y encima avisa de que no
#    funciona es peor que cualquiera de las dos cosas por separado.
ENGANCHADOS="$(grep -oE 'listens\(to: "[a-zA-Z]+"\)' shells/watchos/Sources/*.swift \
  | sed 's/.*"\(.*\)".*/\1/' | sort -u)"
NO_LLEGAN="$(awk '/^fn unheard/,/^\}/' crates/an-watch/src/snapshot.rs \
  | grep -oE '^\s+"[a-zA-Z]+"( \| "[a-zA-Z]+")* =>' \
  | grep -oE '"[a-zA-Z]+"' | tr -d '"' | sort -u)"
SOLAPE_GESTOS="$(comm -12 <(echo "$ENGANCHADOS") <(echo "$NO_LLEGAN"))"
[ -z "$SOLAPE_GESTOS" ] && r=0 || r=1
check $r "ningún gesto se engancha y se declara imposible a la vez"
if [ -n "$SOLAPE_GESTOS" ]; then echo "       en las dos listas: $(echo "$SOLAPE_GESTOS" | tr '\n' ' ')"; fi

grep -q '"crown"' shells/watchos/Sources/AnCrown.swift && r=0 || r=1
check $r "la corona se engancha desde el shell"
grep -q 'digitalCrownRotation' shells/watchos/Sources/AnCrown.swift && r=0 || r=1
check $r "y con la API de la corona de SwiftUI, no con un gesto imitado"

# 4. La tabla de iconos, que vive en dos sitios mientras `an-ios` siga con la
#    suya. Copiada está permitido; divergida, no: `back` tiene que ser el mismo
#    dibujo en el teléfono y en el reloj.
tabla() {
  awk '/fn translate/,/^\}/' "$1" | grep -oE '"[^"]+"( \| "[^"]+")* => "[^"]+"' | sort
}
diff <(tabla crates/an-core/src/icons.rs) <(tabla crates/an-ios/src/icons.rs) >/dev/null 2>&1 && r=0 || r=1
check $r "la tabla de iconos del núcleo dice lo mismo que la de an-ios"

# 5. El shell no puede colocar nada por su cuenta. Todo el layout es de taffy, y
#    un `VStack` o un `padding` metido sin querer sería un segundo motor
#    decidiendo lo mismo; ganaría el que corriese después y nadie sabría por qué.
#    Se miran solo las fuentes que pintan el árbol.
# Sin los comentarios: este fichero explica por qué no los usa, y explicarlo no
# puede contar como usarlo.
COLOCAN="$(grep -vE '^\s*(//|\*)' shells/watchos/Sources/AnNodeView.swift shells/watchos/Sources/AnOverlays.swift \
  | grep -nE '\b(VStack|HStack|LazyVStack|LazyHStack|Spacer\(\)|\.padding\()' || true)"
[ -z "$COLOCAN" ] && r=0 || r=1
check $r "el shell no coloca nada: ni VStack, ni HStack, ni padding"
if [ -n "$COLOCAN" ]; then echo "$COLOCAN" | sed 's/^/       /'; fi

# 6. Los ejemplos, montados con el viewport de un Series 11 de 46 mm. Sin esto,
#    un cambio en las primitivas podría dejar la app del reloj sin pintar y
#    nadie se enteraría hasta abrir el simulador.
cargo an build examples/hello-watch >/dev/null
HOLA="$(cargo run -q -p an-bridge --example headless -- build/bundle/hello-watch/main.js 4 2>&1)"

en_hola() {
  grep -qE -- "$1" <<<"$HOLA" && r=0 || r=1
  check $r "$2"
}

en_hola 'ScrollView#[0-9]+ .*contenido' 'el ScrollView declara su contentSize'
en_hola 'Text#[0-9]+ .*"angular-native"' 'la cabecera se midió y se colocó'
en_hola 'Button#[0-9]+' 'el botón del sistema está montado'
en_hola '"toques: 1"' 'el toque llegó a JS y la señal se recomputó'

cargo an build examples/watch-controls >/dev/null
CONTROLES="$(cargo run -q -p an-bridge --example headless -- build/bundle/watch-controls/main.js 4 2>&1)"

en_controles() {
  grep -qE -- "$1" <<<"$CONTROLES" && r=0 || r=1
  check $r "$2"
}

# Que cada control llegue montado y con su estado. Un control que se monta sin
# props se ve igual que uno que no se monta: en las dos el layout deja un hueco.
en_controles 'Switch#[0-9]+ .*on=true' 'el interruptor baja encendido'
en_controles 'Slider#[0-9]+ .*maximumValue=100' 'el deslizador baja con su recorrido'
en_controles 'Stepper#[0-9]+ .*stepValue=1' 'el paso a paso baja con su salto'
en_controles 'ProgressBar#[0-9]+ .*progress=0\.4' 'la barra baja con el progreso que calculó la señal'
en_controles 'ActivityIndicator#[0-9]+ .*animating=true' 'la ruedecilla baja andando'
en_controles 'Icon#[0-9]+ .*name=favorite' 'el icono baja con su nombre'
en_controles 'Alert#[0-9]+ .*buttons=' 'el diálogo baja con sus botones'
en_controles 'Modal#[0-9]+ .*presentation=sheet' 'la hoja baja diciendo cómo se presenta'
en_controles 'StackView#[0-9]+ .*transition=' 'la pila baja con el sentido de la transición'

# El diálogo no ocupa sitio: lo presenta el sistema. Si algún día lo ocupara, en
# 248 puntos de alto se comería media pantalla y no se vería por qué.
en_controles 'Alert#[0-9]+ \[0,0 0x0\]' 'el diálogo no ocupa sitio en el layout'

# 7. La compilación cruzada, que es la que cuesta y la que puede no estar.
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
  echo "$CONTROLES"
  exit 1
fi
