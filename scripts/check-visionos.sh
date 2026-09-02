#!/usr/bin/env bash
# visionOS: que la app del visor se monte en una ventana que no es una
# pantalla, que no se pinte encima del cristal del sistema, y que el manifiesto
# de escenas siga diciendo lo mismo que el shell.
#
# La compilación cruzada va al final porque necesita nightly:
# `aarch64-apple-visionos-sim` es un target de nivel 3 y no trae `std`
# precompilada. Si no está nightly, esto avisa y no falla.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== visionOS"

check() {
  if [ "$1" = "ok" ]; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

# 1. La app del visor, montada con el tamaño que el shell le pide a la ventana
#    al abrirse. No es una pantalla: el usuario tira de la esquina y cambia, y
#    el viewport de verdad llega por `viewDidLayoutSubviews`. Por eso nada de
#    esta pantalla está en puntos fijos.
cargo an build examples/hello-vision >/dev/null
OUTPUT="$(AN_VIEWPORT=1280x720 cargo run -q -p an-bridge --example headless -- \
  build/bundle/hello-vision/main.js 6 2>&1)"

grep_check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    check ok "$2"
  else
    check no "$2"
  fi
}

grep_check 'View#1 \[0,0 1280x720\]' 'la ventana del visor mide lo que pidió el shell'
grep_check 'View#7 \[40,193 1200x160\]' 'la fila de tarjetas ocupa el ancho de la ventana'
grep_check 'View#13 \[404,0 ' 'las tarjetas se reparten el ancho con flexGrow, sin puntos fijos'
grep_check '"elegiste: una"' 'la pulsación llegó a JS y la señal se recomputó'

# El fondo, que en visionOS es la decisión que más se nota.
#
# La ventana ya trae uno: el cristal que dibuja el sistema, con su desenfoque y
# su sombra sobre la habitación de verdad. Un `[backgroundColor]` en el
# contenedor de arriba lo taparía entero y la app sería una losa opaca flotando
# en el salón. Aquí se comprueba que el contenedor raíz de la plantilla sigue
# sin pintar nada.
if grep -qE 'View#2 \[0,0 1280x720\]\s*$' <<<"$OUTPUT"; then
  check ok 'la raíz de la plantilla no pinta fondo: se ve el cristal del sistema'
else
  check no 'la raíz de la plantilla pinta un fondo y taparía el cristal de la ventana'
fi

# 2. El `Info.plist` del shell contra lo que el build espera de él.
PLIST="shells/visionos/Resources/Info.plist"
plist_check() {
  leido="$(plutil -extract "$1" raw -o - "$PLIST" 2>/dev/null || echo '<sin clave>')"
  if [ "$leido" = "$2" ]; then
    check ok "$PLIST: $1 es $2"
  else
    check no "$PLIST: $1 es \"$leido\" y el build espera \"$2\""
  fi
}
plist_check CFBundleExecutable AngularNativeVision
plist_check CFBundleIdentifier dev.angularnative.playground.vision
# 7 es el visor.
if plutil -extract UIDeviceFamily.0 raw -o - "$PLIST" 2>/dev/null | grep -qx 7; then
  check ok "$PLIST: UIDeviceFamily es 7, el visor"
else
  check no "$PLIST: UIDeviceFamily no es 7"
fi
if grep -q 'Family::VisionOs => ("Vision", ".vision")' crates/an-cli/src/ios.rs; then
  check ok 'Family::suffix pone Vision y .vision, que es lo que dice el plist'
else
  check no 'Family::suffix ya no pone Vision y .vision, y el plist sigue diciendo eso'
fi

# 3. El delegado de escena, que es el fallo silencioso propio de esta familia.
#
#    En visionOS no hay `UIScreen`, así que la ventana solo puede salir de un
#    `UIWindowScene`, y el sistema solo crea uno si el manifiesto dice quién lo
#    atiende. Ese nombre es el de Objective-C, el que le pone `@objc` a la
#    clase de Swift. Si dejan de coincidir, la escena se conecta, nadie crea la
#    ventana, y la app arranca en negro sin un solo error.
ESCENA="$(plutil -extract \
  UIApplicationSceneManifest.UISceneConfigurations.UIWindowSceneSessionRoleApplication.0.UISceneDelegateClassName \
  raw -o - "$PLIST" 2>/dev/null || echo '<sin clave>')"
if grep -q "@objc($ESCENA)" shells/ios/Sources/SceneDelegate.swift; then
  check ok "el manifiesto pide $ESCENA y el shell declara @objc($ESCENA)"
else
  check no "el manifiesto pide $ESCENA y el shell no declara esa clase: la app arrancaría en negro"
fi

# 4. La compilación cruzada.
if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "  --   compilación cruzada omitida: falta el toolchain nightly"
  echo "       rustup toolchain install nightly"
  echo "       rustup component add rust-src --toolchain nightly"
elif ! rustup component list --toolchain nightly 2>/dev/null | grep -q 'rust-src (installed)'; then
  echo "  --   compilación cruzada omitida: falta rust-src en nightly"
  echo "       rustup component add rust-src --toolchain nightly"
else
  if XROS_DEPLOYMENT_TARGET=1.0 cargo +nightly build --quiet -Z build-std=std,panic_abort \
    -p an-ios --target aarch64-apple-visionos-sim 2>/dev/null; then
    check ok 'an-ios para aarch64-apple-visionos-sim'
  else
    check no 'an-ios no cruza-compila para aarch64-apple-visionos-sim'
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
