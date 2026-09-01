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
APK="$(cargo an android examples/controls --no-launch 2>/dev/null | tail -1)"
if [ ! -f "$APK" ]; then
  echo "  FALLO el APK no llegó a armarse"
  cargo an android examples/controls --no-launch 2>&1 | tail -30
  exit 1
fi
echo "  ok   javac compila AnHost y compañía contra Material 3"
