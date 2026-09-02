#!/usr/bin/env bash
# Wear OS: que el APK del reloj sea el del reloj, y que lo que no va lo diga.
#
# Un reloj con Wear OS corre `android.view.View` como cualquier teléfono, así
# que casi todo lo que hay que comprobar del reloj ya lo comprueban los otros
# scripts: es el mismo core, el mismo Java y las mismas primitivas. Lo que no
# comprueba nadie es lo poco que sí cambia, y que además cambia en sitios
# distintos que se pueden desincronizar entre sí:
#
#   - el manifiesto del reloj, que es el que declara la forma del aparato;
#   - el tema, que ahora vive en un recurso y no en el manifiesto;
#   - la lista de primitivas que no se montan, que está en Java y se documenta
#     en `docs/wearos.md`: si una entra en la lista y no en el documento, el
#     hueco solo aparece cuando alguien lo pisa;
#   - que el APK que sale de `an wearos` no sea el del teléfono, que es
#     exactamente el fallo que no se ve —se instala igual y arranca igual—.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== Wear OS"

python3 "$ROOT/scripts/check-wearos.py" "$ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

# Armar un APK y devolver su ruta. Con `set -e` y `pipefail`, meter el build
# dentro de un `$(...)` y tirar su salida a /dev/null hace que un fallo de
# compilación mate el script sin imprimir nada: el propio comprobador se
# quedaría callado, que es lo contrario de lo que hace falta.
armar() {
  if ! cargo an "$@" --no-launch >"$LOG" 2>&1; then
    echo "  FALLO 'cargo an $*' no compiló"
    tail -30 "$LOG"
    exit 1
  fi
  tail -1 "$LOG"
}

# El manifiesto se lee del APK con `aapt2 dump`, que es lo que lee el sistema.
# Mirar el XML de entrada no valdría: lo que se instala es lo que salió del
# enlace, y es en el enlace donde se elige el fichero.
APK="$(armar wearos)"
if [ ! -f "$APK" ]; then
  echo "  FALLO el APK del reloj no llegó a armarse"
  exit 1
fi
echo "  ok   an wearos arma examples/hello-wear"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
AAPT2="$(ls -d "$SDK"/build-tools/*/aapt2 2>/dev/null | sort | tail -1)"
if [ -z "$AAPT2" ]; then
  echo "  aviso no encuentro aapt2; no se puede leer el manifiesto del APK"
  exit 0
fi

VOLCADO="$("$AAPT2" dump badging "$APK")"
for esperado in \
  "uses-feature: name='android.hardware.type.watch'" \
  "package: name='dev.angularnative'"
do
  if ! printf '%s' "$VOLCADO" | grep -qF "$esperado"; then
    echo "  FALLO el APK del reloj no declara: $esperado"
    exit 1
  fi
done
echo "  ok   el APK declara android.hardware.type.watch"

# Y el del teléfono no la declara: si la declarara, la Play Store dejaría de
# ofrecerlo para teléfonos y nadie se enteraría hasta publicarlo.
APK_TEL="$(armar android examples/hello-angular)"
if [ ! -f "$APK_TEL" ]; then
  echo "  FALLO el APK del teléfono no llegó a armarse"
  exit 1
fi
if "$AAPT2" dump badging "$APK_TEL" | grep -qF "name='android.hardware.type.watch'"; then
  echo "  FALLO el APK del teléfono declara ser de reloj"
  exit 1
fi
echo "  ok   el del teléfono sigue sin declararla"
