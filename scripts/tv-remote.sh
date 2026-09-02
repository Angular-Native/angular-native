#!/usr/bin/env bash
# El mando del Apple TV, desde la línea de órdenes.
#
#   ./scripts/tv-remote.sh down down select
#
# `xcrun simctl` no tiene ningún verbo para el mando: tiene `io ... screenshot`,
# `io ... recordVideo`, `ui`, `spawn` y `push`, y ninguno manda una pulsación.
# Lo que sí hay es la entrada de teclado del propio Simulator, que el simulador
# de tvOS traduce a movimientos del foco y al botón central. Así que esto es
# `osascript` mandando códigos de tecla, y no pretende ser otra cosa.
#
# Dos condiciones que no se pueden esquivar, y por eso se comprueban antes de
# mandar nada: si fallan, la pulsación se iría a la aplicación que estuviera
# delante y aquí no se enteraría nadie.
#
#   1. La pantalla del Mac no puede estar bloqueada. Con la sesión bloqueada
#      ninguna aplicación se puede traer al frente.
#   2. Simulator tiene que acabar en primer plano. Mientras esto corre, el
#      teclado es suyo.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "uso: $0 <tecla>... " >&2
  echo "     teclas: up down left right select menu" >&2
  exit 2
fi

# Códigos de tecla de macOS. Los de dirección son los que el simulador de tvOS
# convierte en movimientos del foco; el retorno es el botón central y el escape
# es el botón de menú, que es el «atrás» del mando.
tecla() {
  case "$1" in
    up) echo 126 ;;
    down) echo 125 ;;
    left) echo 123 ;;
    right) echo 124 ;;
    select | ok | enter) echo 36 ;;
    menu | back) echo 53 ;;
    *)
      echo "no conozco la tecla \"$1\"; hay up, down, left, right, select y menu" >&2
      exit 2
      ;;
  esac
}

# Se validan todas antes de mandar ninguna: media secuencia mandada y luego un
# error deja el foco a mitad de camino y la captura siguiente miente.
for nombre in "$@"; do
  tecla "$nombre" >/dev/null
done

if ioreg -n Root -d1 -a 2>/dev/null | grep -q "CGSSessionScreenIsLocked"; then
  echo "la pantalla del Mac está bloqueada: ninguna tecla llegaría al simulador." >&2
  echo "Desbloquéala y vuelve a intentarlo." >&2
  exit 1
fi

if ! xcrun simctl list devices booted 2>/dev/null | grep -q .; then
  echo "no hay ningún simulador arrancado." >&2
  exit 1
fi

osascript -e 'tell application "Simulator" to activate' >/dev/null
# Que haya llegado de verdad al frente, y no solo que se lo hayamos pedido.
frente=""
for _ in 1 2 3 4 5 6 7 8 9 10; do
  frente="$(osascript -e 'tell application "System Events" to return name of first application process whose frontmost is true')"
  [ "$frente" = "Simulator" ] && break
  sleep 0.3
done
if [ "$frente" != "Simulator" ]; then
  echo "Simulator no llegó a primer plano (delante está \"$frente\")." >&2
  echo "Sin eso las teclas irían a esa otra aplicación, así que no se manda ninguna." >&2
  exit 1
fi

for nombre in "$@"; do
  codigo="$(tecla "$nombre")"
  osascript -e "tell application \"System Events\" to key code $codigo" >/dev/null
  # El motor de foco anima el salto; encadenar sin esperar se come pulsaciones.
  sleep 0.6
done
