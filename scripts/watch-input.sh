#!/usr/bin/env bash
# Toca la pantalla y gira la corona del simulador de watchOS desde fuera.
#
# `xcrun simctl` no tiene ningún verbo para esto: están `io … screenshot`,
# `io … recordVideo`, `ui`, `spawn`, `push`… y ninguno manda un toque ni un
# giro. Lo mismo que pasaba con el mando del Apple TV, y la salida es la misma
# que en `tv-remote.sh`: mover el ratón y el teclado del propio Simulator.
#
#   ./scripts/watch-input.sh tap 104 200        toque en (104,200), en puntos
#   ./scripts/watch-input.sh drag 104 220 104 60  arrastre: desplaza una lista
#   ./scripts/watch-input.sh turn 12            doce pasos de corona hacia abajo
#   ./scripts/watch-input.sh turn -12           y hacia arriba
#   ./scripts/watch-input.sh shot /tmp/a.png    captura
#
# Las coordenadas son **puntos del reloj**, no píxeles de la pantalla del Mac:
# la ventana se busca por su nombre y el rectángulo de la pantalla se lee del
# árbol de accesibilidad, así que da igual dónde esté la ventana y da igual que
# haya más simuladores abiertos.
#
# La corona es lo que más costó encontrar y por eso se apunta: **la rueda del
# ratón solo gira la corona si el puntero está encima del botón «Crown» de la
# ventana**, no encima de la pantalla. Sobre la pantalla no pasa absolutamente
# nada —ni un aviso— y es fácil concluir que la corona no se puede mover desde
# fuera, que fue lo que pasó aquí.
#
# Tiene las mismas dos condiciones que el mando de la tele, y por lo mismo:
#
# - **La pantalla del Mac no puede estar bloqueada.** Con la sesión bloqueada
#   ninguna aplicación se puede traer al frente y los eventos no llegan.
# - **Simulator se queda en primer plano** mientras esto corre: el ratón es
#   suyo. Con otro simulador delante, los eventos se los lleva ese.
#
# `AN_WATCH_WINDOW` cambia con qué se busca la ventana; por defecto, cualquiera
# que diga «Watch» o «Series», que es como las nombra watchOS 26.
set -euo pipefail

VENTANA="${AN_WATCH_WINDOW:-}"
UDID="${AN_WATCH_UDID:-booted}"

uso() {
  sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

[ $# -ge 1 ] || uso
ORDEN="$1"
shift

if [ "$ORDEN" = "shot" ]; then
  [ $# -eq 1 ] || uso
  xcrun simctl io "$UDID" screenshot "$1" >/dev/null 2>&1
  echo "captura en $1"
  exit 0
fi

# ---------------------------------------------------------------- la ventana

if [ -n "$VENTANA" ]; then
  FILTRO="name of w contains \"$VENTANA\""
else
  FILTRO='name of w contains "Watch" or name of w contains "Series" or name of w contains "Ultra"'
fi

osascript -e 'tell application "Simulator" to activate' >/dev/null
# Traer al frente no es instantáneo, y un evento que llega antes se va a otra
# ventana sin decir nada.
sleep 0.6

RECT="$(osascript <<AS
tell application "System Events"
  if not (exists process "Simulator") then error "el Simulator no está abierto"
  tell process "Simulator"
    repeat with w in windows
      if $FILTRO then
        -- La pantalla del reloj y el botón de la corona, en coordenadas de
        -- pantalla del Mac. Los dos salen del árbol de accesibilidad en vez de
        -- suponerse: la ventana se puede mover y se puede escalar.
        set g to group 1 of group 1 of w
        set {px, py} to position of g
        set {sw, sh} to size of g
        set {cx, cy} to position of button "Crown" of w
        set {cw, ch} to size of button "Crown" of w
        -- Enteros a propósito: en un Mac con la coma decimal, "500 / 2" se
        -- escribe "250,0" y lo que lo lea al otro lado ve dos campos.
        return (px as text) & " " & (py as text) & " " & (sw as text) & " " & (sh as text) & " " & ((cx + (cw div 2)) as text) & " " & ((cy + (ch div 2)) as text)
      end if
    end repeat
    error "no hay ninguna ventana de reloj en el Simulator"
  end tell
end tell
AS
)"
read -r OX OY OW OH CX CY <<<"$RECT"

# El tamaño del reloj en puntos. `simctl io … enumerate` lo da en pixeles y
# todos los relojes son de 2x, asi que se divide. Hace falta porque la ventana
# del Simulator se puede escalar: sin esto, un toque en (104,200) con la ventana
# al 75 % se iria treinta puntos mas abajo de lo que dice la plantilla.
PIXELES="$(xcrun simctl io "$UDID" enumerate 2>/dev/null | awk '/Default width:/{w=$3} /Default height:/{h=$3} END{print w+0, h+0}')"
read -r PX_W PX_H <<<"$PIXELES"

punto() { # x y en puntos del reloj -> x y en la pantalla del Mac
  python3 "$AYUDA_PUNTO" "$1" "$2" "$OX" "$OY" "$OW" "$OH" "$PX_W" "$PX_H"
}

AYUDA_PUNTO="$(mktemp -t watch-punto).py"
cat > "$AYUDA_PUNTO" <<'PY'
import sys

x, y, ox, oy, ow, oh, pxw, pxh = (float(v) for v in sys.argv[1:9])
# Sin medidas del dispositivo se supone 1:1, que es lo que hace el Simulator
# con la ventana a su tamano natural.
ancho = pxw / 2 if pxw else ow
alto = pxh / 2 if pxh else oh
print(round(ox + x * ow / ancho), round(oy + y * oh / alto))
PY

SW_FILE="$(mktemp -t watch-input).swift"
trap 'rm -f "$SW_FILE" "$AYUDA_PUNTO"' EXIT
cat > "$SW_FILE" <<'SWIFT'
// Ratón por CoreGraphics. No se usa `cliclick` para no pedir un homebrew más:
// arrastrar y girar la rueda son cuatro llamadas de CGEvent.
import CoreGraphics
import Foundation

let a = CommandLine.arguments

func mueve(_ p: CGPoint) {
    CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?
        .post(tap: .cghidEventTap)
}

func boton(_ tipo: CGEventType, _ p: CGPoint) {
    CGEvent(mouseEventSource: nil, mouseType: tipo, mouseCursorPosition: p, mouseButton: .left)?
        .post(tap: .cghidEventTap)
}

switch a[1] {
case "tap":
    let p = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    mueve(p); usleep(120_000)
    boton(.leftMouseDown, p); usleep(80_000)
    boton(.leftMouseUp, p)
case "drag":
    let desde = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    let hasta = CGPoint(x: Double(a[4])!, y: Double(a[5])!)
    mueve(desde); usleep(150_000)
    boton(.leftMouseDown, desde); usleep(80_000)
    // Por pasos: un salto de un solo evento lo lee el simulador como un
    // teletransporte y no lo convierte en arrastre.
    for i in 1...12 {
        let t = Double(i) / 12.0
        let p = CGPoint(
            x: desde.x + (hasta.x - desde.x) * t,
            y: desde.y + (hasta.y - desde.y) * t
        )
        boton(.leftMouseDragged, p)
        usleep(20_000)
    }
    boton(.leftMouseUp, hasta)
case "turn":
    // Sobre el botón de la corona, no sobre la pantalla: sobre la pantalla la
    // rueda no hace nada.
    let p = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    let pasos = Int(a[4])!
    mueve(p); usleep(200_000)
    let signo: Int32 = pasos < 0 ? -1 : 1
    for _ in 0..<abs(pasos) {
        guard let e = CGEvent(
            scrollWheelEvent2Source: nil, units: .line, wheelCount: 1,
            wheel1: signo, wheel2: 0, wheel3: 0
        ) else { continue }
        e.location = p
        e.post(tap: .cghidEventTap)
        usleep(45_000)
    }
default:
    FileHandle.standardError.write("orden desconocida\n".data(using: .utf8)!)
    exit(2)
}
SWIFT

case "$ORDEN" in
  tap)
    [ $# -eq 2 ] || uso
    read -r X Y <<<"$(punto "$1" "$2")"
    swift "$SW_FILE" tap "$X" "$Y"
    echo "toque en ($1,$2) -> pantalla ($X,$Y)"
    ;;
  drag)
    [ $# -eq 4 ] || uso
    read -r X1 Y1 <<<"$(punto "$1" "$2")"
    read -r X2 Y2 <<<"$(punto "$3" "$4")"
    swift "$SW_FILE" drag "$X1" "$Y1" "$X2" "$Y2"
    echo "arrastre ($1,$2) -> ($3,$4)"
    ;;
  turn)
    [ $# -eq 1 ] || uso
    swift "$SW_FILE" turn "$CX" "$CY" "$1"
    echo "corona: $1 pasos"
    ;;
  *)
    uso
    ;;
esac
