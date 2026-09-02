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
if grep -q "no se aplica en macOS" "$RUN_LOG"; then
  echo "  ok   lo que AppKit no cubre se dice al llegar, no se traga"
else
  echo "  FALLO ninguna prop descartada salió por pantalla: el aviso no funciona"
  fail=1
fi

# 7. Y el reverso: nada que nadie haya declarado. Una «prop desconocida» es una
#    prop que este host no mira y que tampoco está en IGNORED, o sea, un olvido.
if grep -q "prop desconocida" "$RUN_LOG"; then
  echo "  FALLO hay props que el host no mira y que no están declaradas:"
  grep "prop desconocida" "$RUN_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   ninguna prop del ejemplo se queda sin dueño"
fi

if [ "$fail" -ne 0 ]; then
  echo
  cat "$RUN_LOG"
  rm -f "$RUN_LOG"
  exit 1
fi
rm -f "$RUN_LOG"
