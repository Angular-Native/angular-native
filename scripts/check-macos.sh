#!/usr/bin/env bash
# El host de macOS: lo que hay que ejecutar para comprobarlo.
#
# Las comprobaciones de texto —el inventario contra el enum, la lista de props
# ignoradas, la recarga en caliente— están en `check-macos.py`. Aquí está lo que
# es un proceso: compilar, armar el `.app`, arrancarlo y mirar lo que pintó.
#
# **Y arrancarlo se puede.** Es lo que separa a esta plataforma de las otras
# cuatro: no hay simulador que levantar ni aparato que buscar, la app corre en
# la misma máquina que la compiló y termina sola. Así que aquí la comprobación
# no se queda en «cruza-compila»: la app se abre, monta el árbol y se hace una
# captura a sí misma, y lo que se mira es esa imagen.
#
# La captura la hace la app y no `screencapture` a propósito. Pedirle la
# pantalla al sistema exige el permiso de grabación, que se concede a mano y por
# aplicación: una comprobación que depende de eso falla en la máquina de
# cualquiera que no lo haya concedido, y falla por un motivo que no tiene nada
# que ver con lo que se estaba comprobando. Una vista, en cambio, sabe dibujarse
# en un mapa de bits sin pedir permiso a nadie. Ver
# `shells/macos/Sources/Screenshot.swift`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== macOS"

# Lo de texto va primero: es instantáneo y no compila nada, así que si el
# inventario está mal se sabe antes de esperar a un enlazado.
if ! python3 "$ROOT/scripts/check-macos.py" "$ROOT"; then
  fail=1
fi

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   el resto se omite: el host de macOS solo se compila en un Mac"
  exit "$fail"
fi

# 1. El inventario, por dentro. No abre ninguna ventana: `support.rs` está fuera
#    de `cfg(target_os = "macos")` para poder mirarlo desde aquí.
if cargo test --quiet -p an-macos >/dev/null 2>&1; then
  echo "  ok   el inventario de primitivas se sostiene"
else
  echo "  FALLO los tests de an-macos no pasan"
  cargo test -p an-macos 2>&1 | tail -20
  fail=1
fi

# 2. El `.app` entero: núcleo para aarch64-apple-darwin, shell de AppKit
#    enlazado con swiftc y firma ad-hoc. Sin la firma, macOS mata la app al
#    primer trozo de código que genera QuickJS.
BUILD_LOG="$(mktemp)"
if cargo an macos --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   el .app se arma y se firma"
else
  echo "  FALLO el .app de macOS no se arma"
  tail -30 "$BUILD_LOG"
  rm -f "$BUILD_LOG"
  exit 1
fi
rm -f "$BUILD_LOG"

APP="$ROOT/build/macos/AngularNativeMac.app"

# 3. La forma del bundle. Un `.app` de macOS no es plano como el de iOS, y uno
#    mal armado lo abre el Finder y lo rechaza `open` sin decir por qué.
faltan=()
for pieza in Contents/Info.plist Contents/MacOS/AngularNativeMac Contents/Resources/main.js; do
  [ -e "$APP/$pieza" ] || faltan+=("$pieza")
done
if [ "${#faltan[@]}" -eq 0 ]; then
  echo "  ok   el bundle tiene su Info.plist, su ejecutable y su main.js"
else
  echo "  FALLO al bundle le faltan: ${faltan[*]}"
  fail=1
fi

if codesign --verify --deep "$APP" >/dev/null 2>&1; then
  echo "  ok   la firma ad-hoc vale"
else
  echo "  FALLO el .app no está firmado: QuickJS moriría al generar código"
  fail=1
fi

# 4. Y ahora la de verdad: arrancarla.
SHOT="$ROOT/build/macos/captura.png"
RUN_LOG="$(mktemp)"
rm -f "$SHOT"
if AN_SCREENSHOT="$SHOT" "$APP/Contents/MacOS/AngularNativeMac" >"$RUN_LOG" 2>&1; then
  echo "  ok   la app arranca, monta el árbol y se cierra sola"
else
  echo "  FALLO la app no llegó a montar nada (código $?)"
  tail -30 "$RUN_LOG"
  fail=1
fi

# 5. Que pintó. Un PNG del tamaño correcto y enteramente negro pesa lo suyo y
#    pasaría cualquier prueba que mire el tamaño del fichero, así que lo que se
#    mira es cuántos colores distintos hay: la app los cuenta al guardar.
colores="$(sed -n 's/.*, \([0-9]*\) colores).*/\1/p' "$RUN_LOG" | tail -1)"
if [ -s "$SHOT" ] && [ -n "$colores" ] && [ "$colores" -gt 16 ]; then
  echo "  ok   la ventana pintó los controles ($colores colores en la captura)"
else
  echo "  FALLO la captura salió en blanco o no se escribió"
  fail=1
fi

# 6. El camino ruidoso, que es el que sostiene la regla de la casa. El ejemplo
#    de los controles usa props que AppKit no puede honrar, y tienen que salir
#    por pantalla la primera vez que llegan.
if grep -qE "(no se aplica en macOS|does not apply on macOS)" "$RUN_LOG"; then
  echo "  ok   lo que AppKit no cubre se dice al llegar, no se traga"
else
  echo "  FALLO ninguna prop descartada salió por pantalla: el aviso no funciona"
  fail=1
fi

# 7. Y el reverso: nada que nadie haya declarado. Una «prop desconocida» es una
#    prop que este host no mira y que tampoco está en IGNORED, o sea, un olvido.
if grep -qE "(prop desconocida|unknown prop)" "$RUN_LOG"; then
  echo "  FALLO hay props que el host no mira y que no están declaradas:"
  grep -E "(prop desconocida|unknown prop)" "$RUN_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   ninguna prop del ejemplo se queda sin dueño"
fi

# 8. El escritorio de verdad: el puntero y el deslizamiento.
#
# Las dos cosas que esta plataforma tiene y las otras cuatro no, y las dos se
# pueden comprobar aquí *corriendo la app*, que es lo que separa a macOS del
# resto: no hay simulador que levantar ni aparato que buscar.
#
# El puntero se mueve de verdad. `CGWarpMouseCursorPosition` no pide ningún
# permiso —no es `CGEventPost`, que sí exige accesibilidad—, así que lo que
# entra en el `NSTrackingArea` es el ratón, y lo que sale en la imagen es el
# área del sistema haciendo su trabajo.
#
# El deslizamiento no se puede provocar: el gesto lo reconoce el sistema a
# partir de dos dedos en el trackpad y no hay forma de pedírselo. Lo que sí se
# puede es entrar por su misma puerta —`swipeWithEvent:` sobre la vista que hay
# bajo el punto— con los deltas que manda él, y comprobar todo lo que viene
# después. Ver `Screenshot.swift`.
BUILD_LOG="$(mktemp)"
if cargo an macos examples/desktop --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   el ejemplo del escritorio se arma"
else
  echo "  FALLO el ejemplo del escritorio no se arma"
  tail -20 "$BUILD_LOG"
  fail=1
fi
rm -f "$BUILD_LOG"

BIN="$APP/Contents/MacOS/AngularNativeMac"
SHOT_QUIETO="$ROOT/build/macos/escritorio.png"
SHOT_ENCIMA="$ROOT/build/macos/escritorio-encima.png"
DESK_LOG="$(mktemp)"
HOVER_LOG="$(mktemp)"

AN_SCREENSHOT="$SHOT_QUIETO" AN_SCREENSHOT_FRAMES=150 \
  AN_SCREENSHOT_SWIPE=360,600,-1,0 "$BIN" >"$DESK_LOG" 2>&1 || true

# El signo lo dice `NSEvent.h`: «-1 for swipe right». Si esta línea deja de
# cuadrar es que alguien cambió la correspondencia, no que el gesto no llegue.
if grep -q "\[swipe\] derecha" "$DESK_LOG"; then
  echo "  ok   un deslizamiento con deltaX -1 llega a la plantilla como «derecha»"
else
  echo "  FALLO el deslizamiento no llegó a la plantilla"
  fail=1
fi

# El puntero encima de la primera tarjeta. Las coordenadas son las de la
# ventana de 720x820 que abre el shell; si el ejemplo cambia de sitio, aquí
# hay que moverlas.
AN_SCREENSHOT="$SHOT_ENCIMA" AN_SCREENSHOT_FRAMES=150 \
  AN_SCREENSHOT_HOVER=97,200 "$BIN" >"$HOVER_LOG" 2>&1 || true

# A hover only happens if the window is actually under the pointer, and that
# needs the app to win the front. With a simulator or an emulator open, macOS
# hands the front to whoever asked last and this check would fail for a reason
# that has nothing to do with the code — it has already happened to three
# separate runs. So the precondition is checked first and reported as a skip,
# loudly and with its reason. A skip is not a pass: the run says so, and the
# same binary passes as soon as nothing else is fighting for the front.
if grep -q "\[hover\] dentro de pointer" "$HOVER_LOG"; then
  echo "  ok   el puntero entra en la vista y la plantilla se entera"
elif grep -q "frontmost=no" "$HOVER_LOG"; then
  echo "  omitida el (hover): la app no llegó al frente, así que el puntero"
  echo "           nunca estuvo encima. Ciérrale los simuladores y repite."
else
  echo "  FALLO nadie recibió el (hover) con el ratón encima"
  fail=1
fi

# Y que además se vea. Un `(hover)` que llega y no cambia nada en pantalla es
# la mitad del trabajo: lo que hay que comprobar es que el árbol se recompuso.
if [ -s "$SHOT_QUIETO" ] && [ -s "$SHOT_ENCIMA" ] \
  && ! cmp -s "$SHOT_QUIETO" "$SHOT_ENCIMA"; then
  echo "  ok   y la ventana cambia con el ratón encima"
else
  echo "  FALLO la ventana sale igual con el ratón encima que sin él"
  fail=1
fi

# 9. El mapa y el vídeo, que son los dos que faltaban.
#
# Lo que se mira del mapa es la imagen: `MKMapView` dibuja teselas y eso sube
# el recuento de colores muy por encima de lo que da una caja vacía. Del vídeo
# no se puede mirar la imagen y no se disimula: `AVPlayerView` compone sus
# fotogramas fuera del dibujado de la vista —por eso tampoco los ve
# `cacheDisplay`—, así que lo que se comprueba es que se monta como vista de
# verdad y que el reproductor no falla. Está dicho en docs/macos.md.
BUILD_LOG="$(mktemp)"
if cargo an macos examples/media --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   el ejemplo de mapa y vídeo se arma"
else
  echo "  FALLO el ejemplo de mapa y vídeo no se arma"
  tail -20 "$BUILD_LOG"
  fail=1
fi
rm -f "$BUILD_LOG"

SHOT_MEDIA="$ROOT/build/macos/media.png"
MEDIA_LOG="$(mktemp)"
AN_SCREENSHOT="$SHOT_MEDIA" AN_SCREENSHOT_FRAMES=240 \
  AN_SCREENSHOT_PRESS=360,782 "$BIN" >"$MEDIA_LOG" 2>&1 || true

if grep -q "no se pinta en macOS" "$MEDIA_LOG"; then
  echo "  FALLO el mapa o el vídeo siguen sin pintarse:"
  grep "no se pinta en macOS" "$MEDIA_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   el mapa y el vídeo se montan con su vista del sistema"
fi

if grep -q "no se puede reproducir" "$MEDIA_LOG"; then
  echo "  FALLO el reproductor falló:"
  grep "no se puede reproducir" "$MEDIA_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   el reproductor no falló al ponerse en marcha"
fi

colores_media="$(sed -n 's/.*, \([0-9]*\) colores).*/\1/p' "$MEDIA_LOG" | tail -1)"
if [ -n "$colores_media" ] && [ "$colores_media" -gt 200 ]; then
  echo "  ok   el mapa dibuja de verdad ($colores_media colores en la captura)"
else
  echo "  FALLO el mapa salió liso: ${colores_media:-ninguna captura} colores"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  cat "$RUN_LOG" "$DESK_LOG" "$HOVER_LOG" "$MEDIA_LOG"
  rm -f "$RUN_LOG" "$DESK_LOG" "$HOVER_LOG" "$MEDIA_LOG"
  exit 1
fi
rm -f "$RUN_LOG" "$DESK_LOG" "$HOVER_LOG" "$MEDIA_LOG"
