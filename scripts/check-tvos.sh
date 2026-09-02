#!/usr/bin/env bash
# tvOS: que la app de la tele se monte con las medidas de una tele, que lo que
# el SDK no trae siga estando dicho en los tres sitios donde se dice, y que el
# core cruza-compile.
#
# La compilación cruzada va aparte, y al final, porque necesita nightly:
# `aarch64-apple-tvos-sim` es un target de nivel 3 y no trae `std`
# precompilada, así que hay que construirla en el momento con `-Z build-std`.
# Si no está nightly, esto avisa y no falla: el resto de las comprobaciones no
# tienen por qué caerse porque a alguien le falte un toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== tvOS"

check() {
  if [ "$1" = "ok" ]; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

# 1. La app de la tele, montada con el viewport de un Apple TV: 1920x1080
#    puntos, no los 393 de un iPhone. Sin esto un cambio en las primitivas
#    podría dejarla sin pintar y nadie se enteraría hasta abrir el simulador.
cargo an build examples/hello-tv >/dev/null
OUTPUT="$(AN_VIEWPORT=1920x1080 cargo run -q -p an-bridge --example headless -- \
  build/bundle/hello-tv/main.js 6 2>&1)"

grep_check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    check ok "$2"
  else
    check no "$2"
  fi
}

grep_check 'View#1 \[0,0 1920x1080\]' 'el viewport de la tele es 1920x1080'
# Overscan: los bordes de un televisor se recortan, y Apple pide 90 puntos a
# los lados y 60 arriba. Eso lo pone la plantilla, no el framework, así que si
# alguien lo quita aquí no falla nada: se sale de la pantalla en casa de otro.
grep_check 'Text#3 \[90,60 ' 'los márgenes de overscan son 90 x 60'
grep_check 'Button#7 \[90,286 760x88\]' 'el primer botón del sistema está montado'
grep_check 'Button#8 \[90,402 760x88\]' 'el segundo botón del sistema está montado'
grep_check 'View#9 \[90,518 760x88\]' 'el an-view enfocable está montado y no es un control'
grep_check 'title=arriba — pulsado 1 veces' 'la pulsación llegó a JS y la señal se recomputó'

# 2. Lo que el SDK de tvOS no trae, dicho en los tres sitios.
#
#    `family.rs` es la lista de verdad —es la anotación del SDK traída al
#    Rust—, y de ahí salen dos cosas que se pueden separar sin que nada falle:
#    el aviso que se escribe en el log y la fila de la tabla de docs-site/src/content/docs/platforms/tvos.md.
#    Que se separen es justo lo que deja a alguien buscando un control que
#    nunca va a aparecer.
KINDS="$(sed -n '/#\[cfg(target_os = "tvos")\]/,/#\[cfg(not(target_os = "tvos"))\]/p' \
  crates/an-ios/src/family.rs | grep -oE 'NodeKind::[A-Za-z]+' | sed 's/NodeKind:://' | sort -u)"
if [ -z "$KINDS" ]; then
  check no 'family.rs declara qué primitivas no existen en tvOS'
else
  check ok "family.rs declara $(wc -l <<<"$KINDS" | tr -d ' ') primitivas que tvOS no trae"
  for kind in $KINDS; do
    # La regla del proyecto, de PascalCase a la etiqueta: WebView es
    # an-web-view. Es la misma que va del núcleo a la plantilla, así que no
    # hay tabla de traducción que se pueda quedar atrás.
    tag="an-$(sed -E 's/([a-z0-9])([A-Z])/\1-\2/g' <<<"$kind" | tr '[:upper:]' '[:lower:]')"
    if grep -q -- "$tag" docs-site/src/content/docs/platforms/tvos.md; then
      check ok "docs-site/src/content/docs/platforms/tvos.md dice qué pasa con $tag"
    else
      check no "docs-site/src/content/docs/platforms/tvos.md no menciona $tag, que family.rs da por no disponible"
    fi
  done
fi

# 3. El `Info.plist` del shell contra lo que el build espera de él.
#
#    El nombre y el identificador llevan el sufijo de la familia, y el build
#    los compara con `plutil` antes de compilar nada. Si el plist y
#    `Family::suffix` dejan de decir lo mismo, la app se instala y al abrirla
#    desaparece: el sistema busca un ejecutable que no está.
PLIST="shells/tvos/Resources/Info.plist"
plist_check() {
  leido="$(plutil -extract "$1" raw -o - "$PLIST" 2>/dev/null || echo '<sin clave>')"
  if [ "$leido" = "$2" ]; then
    check ok "$PLIST: $1 es $2"
  else
    check no "$PLIST: $1 es \"$leido\" y el build espera \"$2\""
  fi
}
plist_check CFBundleExecutable AngularNativeTV
plist_check CFBundleIdentifier dev.angularnative.playground.tv
# 3 es el Apple TV. Sin esta clave `simctl` instala algo que luego no sabe
# lanzar, y el error llega mucho después y en otro idioma.
if plutil -extract UIDeviceFamily.0 raw -o - "$PLIST" 2>/dev/null | grep -qx 3; then
  check ok "$PLIST: UIDeviceFamily es 3, el Apple TV"
else
  check no "$PLIST: UIDeviceFamily no es 3"
fi
if grep -q 'Family::TvOs => ("TV", ".tv")' crates/an-cli/src/ios.rs; then
  check ok 'Family::suffix pone TV y .tv, que es lo que dice el plist'
else
  check no 'Family::suffix ya no pone TV y .tv, y el plist sigue diciendo eso'
fi

# 4. WebKit no forma parte del SDK de tvOS. El módulo entero tiene que salir
#    del binario, y no solo por la clase: su `#[link(name = "WebKit")]` haría
#    que el enlazado buscase un framework que en ese SDK no está, y eso no se
#    ve hasta el `swiftc` del final.
if grep -B1 '^mod web;' crates/an-ios/src/lib.rs | grep -q 'target_os = "tvos"'; then
  check no 'el módulo web sigue compilándose para tvOS, y WebKit no está en su SDK'
else
  check ok 'el módulo web sale del binario en tvOS: WebKit no está en su SDK'
fi

# 5. El botón, que es donde estaba el fallo silencioso.
#
#    `TouchUpInside` es lo que UIKit manda cuando un dedo se levanta de la
#    pantalla, y en una tele no hay dedos: llega `PrimaryActionTriggered`. Con
#    el evento equivocado el botón toma el foco, se pone blanco, y pulsarlo no
#    hace nada. No hay error que mirar, así que se comprueba aquí.
if sed -n '/#\[cfg(target_os = "tvos")\]/,+3p' crates/an-ios/src/events.rs |
  grep -q 'NodeKind::Button, "press"'; then
  if sed -n '/#\[cfg(target_os = "tvos")\]/,+4p' crates/an-ios/src/events.rs |
    grep -q 'PrimaryActionTriggered'; then
    check ok 'en tvOS el botón escucha PrimaryActionTriggered, no TouchUpInside'
  else
    check no 'en tvOS el botón ya no escucha PrimaryActionTriggered'
  fi
else
  check no 'el botón de tvOS ya no tiene su propio evento de control'
fi

# 6. La compilación cruzada, que es la que cuesta y la que puede no estar.
if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "  --   compilación cruzada omitida: falta el toolchain nightly"
  echo "       rustup toolchain install nightly"
  echo "       rustup component add rust-src --toolchain nightly"
elif ! rustup component list --toolchain nightly 2>/dev/null | grep -q 'rust-src (installed)'; then
  echo "  --   compilación cruzada omitida: falta rust-src en nightly"
  echo "       rustup component add rust-src --toolchain nightly"
else
  if TVOS_DEPLOYMENT_TARGET=17.0 cargo +nightly build --quiet -Z build-std=std,panic_abort \
    -p an-ios --target aarch64-apple-tvos-sim 2>/dev/null; then
    check ok 'an-ios para aarch64-apple-tvos-sim'
  else
    check no 'an-ios no cruza-compila para aarch64-apple-tvos-sim'
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
