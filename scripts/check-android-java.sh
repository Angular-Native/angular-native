#!/usr/bin/env bash
# Que el shell Java compile.
#
# `cargo test` y los volcados headless no tocan Java: el host de Android son
# 2.600 líneas que hasta ahora solo se compilaban al armar el APK a mano, y una
# tanda entera de props podía quedarse con un error de tipos sin que nada lo
# dijera. Armar el APK sin instalarlo cuesta medio minuto y lo compila todo:
# `aapt2` enlaza los recursos, `javac` compila el shell contra Material, y `d8`
# lo dexa.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== shell Java de Android"
# La salida va a un fichero y no a /dev/null: con `set -e` y `pipefail`, un
# fallo de compilación dentro de un `$(...)` mata el script sin imprimir nada
# y el comprobador se queda callado, que es justo lo que no puede pasar.
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT
if ! cargo an android examples/controls --no-launch >"$LOG" 2>&1; then
  echo "  FALLO el APK no llegó a armarse"
  tail -30 "$LOG"
  exit 1
fi
APK="$(tail -1 "$LOG")"
if [ ! -f "$APK" ]; then
  echo "  FALLO el APK no llegó a armarse"
  tail -30 "$LOG"
  exit 1
fi
echo "  ok   javac compila AnHost y compañía contra Material 3"
