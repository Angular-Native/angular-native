#!/usr/bin/env bash
# Compila una app Angular a un bundle que el motor embebido puede evaluar.
#
# Dos pasos, ninguno del CLI de Angular: `ngc` compila las plantillas por AOT
# (nada de compilador en el dispositivo), y esbuild empaqueta a un IIFE plano
# porque QuickJS no carga módulos ES.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-examples/hello-angular}"
# ngc emite respetando rootDir, así que las rutas de salida reflejan la ruta
# del proyecto relativa a la raíz del repo. Normalizamos para que dé igual que
# llamen con ruta absoluta o relativa.
APP="${APP#"$ROOT/"}"
APP="${APP%/}"
NAME="$(basename "$APP")"
OUT="${OUT:-$ROOT/build/bundle/$NAME/main.js}"

echo "==> ngc (AOT) $APP"
(cd "$ROOT" && npx ngc -p "$APP/tsconfig.json")

echo "==> esbuild"
mkdir -p "$(dirname "$OUT")"
# ngc emite también los paquetes del workspace bajo build/js: se apuntan ahí
# en vez de a los .ts que `main` de cada package.json declara.
JS="build/js/$NAME"
(cd "$ROOT" && npx esbuild "$JS/$APP/src/main.js" \
  --bundle \
  "--alias:@angular-native/platform=./$JS/packages/platform-native/src/public-api.js" \
  "--alias:@angular-native/primitives=./$JS/packages/primitives/src/public-api.js" \
  --format=iife \
  --platform=neutral \
  --target=es2022 \
  --main-fields=module,main \
  --conditions=module \
  --outfile="$OUT" \
  --log-level=warning)

echo "==> $(du -h "$OUT" | cut -f1) en $OUT"
