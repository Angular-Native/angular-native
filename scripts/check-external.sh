#!/usr/bin/env bash
# Un proyecto Angular de fuera del monorepo, de principio a fin y sin simulador.
#
# Es el camino que hace alguien que tiene su app de `ng new` y quiere llevarla al
# móvil: `an init`, `an add ios`, `an build`. Aquí se comprueba todo menos el
# último paso —instalar en el simulador—, que es lo único que no se puede hacer
# en una máquina sin Xcode arrancado.
#
# El proyecto de mentira es un proyecto de verdad: `angular.json`, `package.json`
# con `@angular/core`, `src/main.ts` y su componente web. Lo único que no se hace
# es bajarse Angular otra vez de la red: se clona el `node_modules` del propio
# SDK, que trae las mismas versiones. En APFS un clon no copia bytes ni ocupa
# disco.
#
# Se comprueban las dos mitades: que lo que tiene que salir sale, y que lo que
# tiene que fallar falla diciendo por qué. Un `an init` sobre algo que no es
# Angular, uno repetido que pisara el código del usuario, o un `Info.plist`
# desincronizado del manifiesto son las tres formas que esto tiene de estropear
# el proyecto de otro, y ninguna puede pasar en silencio.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
AN="$TARGET/debug/an"
WORK="$ROOT/build/check-external"
APP="$WORK/mi-app"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FALLO $1"; fail=1; }
contiene() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}
existe() {
  if [ -e "$1" ]; then ok "$2"; else ko "$2"; fi
}
# Ejecuta algo que tiene que fallar, y devuelve su salida para mirarla.
falla() {
  local salida
  if salida="$("$@" 2>&1)"; then
    echo "ESPERABA UN FALLO Y SALIÓ BIEN: $*"
    echo "$salida"
    return 1
  fi
  printf '%s' "$salida"
}

echo "== proyecto Angular de fuera del monorepo"

cargo build -q -p an-cli
export AN_HOME="$ROOT"

# ---------------------------------------------------------------------------
# Un proyecto Angular de verdad, montado a mano
# ---------------------------------------------------------------------------
rm -rf "$WORK"
mkdir -p "$APP/src/app"

cat >"$APP/package.json" <<'JSON'
{
  "name": "mi-app",
  "version": "0.0.0",
  "private": true,
  "dependencies": {
    "@angular/common": "^22.1.0",
    "@angular/compiler": "^22.1.0",
    "@angular/core": "^22.1.0",
    "@angular/router": "^22.1.0",
    "rxjs": "~7.8.0"
  }
}
JSON

cat >"$APP/angular.json" <<'JSON'
{
  "$schema": "./node_modules/@angular/cli/lib/config/schema.json",
  "version": 1,
  "projects": {
    "mi-app": {
      "projectType": "application",
      "root": "",
      "sourceRoot": "src",
      "architect": {
        "build": {
          "builder": "@angular/build:application",
          "options": { "browser": "src/main.ts", "index": "src/index.html" }
        }
      }
    }
  }
}
JSON

cat >"$APP/src/main.ts" <<'TS'
import { bootstrapApplication } from '@angular/platform-browser'
import { App } from './app/app'

bootstrapApplication(App)
TS

cat >"$APP/src/app/app.ts" <<'TS'
import { Component } from '@angular/core'

@Component({ selector: 'app-root', template: '<h1>la app web, intacta</h1>' })
export class App {}
TS

# El `node_modules` del SDK, clonado. `cp -c` usa clonefile en APFS: instantáneo
# y sin ocupar disco. Si el sistema de ficheros no lo soporta se copia de verdad,
# y se dice, porque entonces esto tarda medio minuto y no es un misterio.
if ! cp -Rc "$ROOT/node_modules" "$APP/node_modules" 2>/dev/null; then
  echo "  (este sistema de ficheros no clona; copiando node_modules de verdad)"
  cp -R "$ROOT/node_modules" "$APP/node_modules"
fi

# ---------------------------------------------------------------------------
# Lo que tiene que fallar, antes de nada
# ---------------------------------------------------------------------------
VACIO="$(mktemp -d)"
trap 'rm -rf "$VACIO"' EXIT
salida="$(cd "$VACIO" && falla "$AN" init)"
contiene "$salida" 'angular\.json' '`an init` sobre algo que no es Angular dice qué falta'
contiene "$salida" 'npx @angular/cli new' 'y dice cómo se crea un proyecto'

# Un proyecto Angular sin inicializar, fuera del repo: `an` no puede adivinar
# nada, pero sí sabe qué le falta.
SIN_INIT="$VACIO/sin-init"
mkdir -p "$SIN_INIT"
cp "$APP/package.json" "$APP/angular.json" "$SIN_INIT/"
salida="$(cd "$SIN_INIT" && falla "$AN" build)"
contiene "$salida" 'an init' 'un proyecto Angular sin inicializar manda a `an init`'

# ---------------------------------------------------------------------------
# an init
# ---------------------------------------------------------------------------
antes="$(shasum "$APP/src/app/app.ts" "$APP/src/main.ts")"
(cd "$APP" && "$AN" init >/dev/null 2>&1)

existe "$APP/angular-native.json" 'an init: escribe el manifiesto'
existe "$APP/.angular-native/tsconfig.json" 'an init: escribe el tsconfig del build nativo'
existe "$APP/src/main.native.ts" 'an init: escribe el punto de entrada nativo'
existe "$APP/src/app/app-native.ts" 'an init: escribe el componente raíz nativo'
existe "$APP/node_modules/@angular-native/platform/dist/public-api.js" \
  'an init: instala @angular-native/platform compilado'
existe "$APP/node_modules/@angular-native/primitives/dist/public-api.js" \
  'an init: instala @angular-native/primitives compilado'
existe "$APP/node_modules/@angular-native/platform/dist/public-api.d.ts" \
  'an init: el paquete instalado trae sus tipos'

contiene "$(cat "$APP/package.json")" 'file:\.angular-native/vendor/angular-native-platform' \
  'an init: la dependencia apunta al tarball vendorizado, no a una ruta del disco'
contiene "$(ls "$APP/.angular-native/vendor")" '\.tgz' 'an init: los tarballs quedan en el proyecto'
contiene "$(cat "$APP/.gitignore")" '^/\.angular-native/build/$' \
  'an init: solo el directorio de artefactos va al .gitignore'
contiene "$(tar -tzf "$APP/.angular-native/vendor/"*platform*.tgz)" \
  'package/dist/public-api\.js' 'an init: el tarball lleva el paquete compilado, no las fuentes'

if [ "$antes" = "$(shasum "$APP/src/app/app.ts" "$APP/src/main.ts")" ]; then
  ok 'an init: no toca la app web'
else
  ko 'an init: no toca la app web'
fi

# El paquete compilado tiene que ir en modo parcial: si `ngc` compilara en
# `full`, el bundle traería código atado a la versión del compilador del SDK y
# el Angular Linker no tendría nada que resolver.
contiene "$(cat "$APP/node_modules/@angular-native/primitives/dist/public-api.js" \
  "$APP/node_modules/@angular-native/primitives/dist/"*.js)" \
  'ɵɵngDeclare' 'an init: los paquetes se publican en modo parcial'

# Repetirlo no puede estropear nada de lo que haya escrito el usuario.
echo "// el usuario editó esto" >>"$APP/src/app/app-native.ts"
huella="$(shasum "$APP/src/app/app-native.ts")"
(cd "$APP" && "$AN" init >/dev/null 2>&1)
if [ "$huella" = "$(shasum "$APP/src/app/app-native.ts")" ]; then
  ok 'an init repetido: respeta el código que ya estaba'
else
  ko 'an init repetido: respeta el código que ya estaba'
fi

# ---------------------------------------------------------------------------
# an add
# ---------------------------------------------------------------------------
(cd "$APP" && "$AN" add ios >/dev/null 2>&1)
existe "$APP/ios/Info.plist" 'an add ios: crea el Info.plist del proyecto'
contiene "$(plutil -extract CFBundleExecutable raw -o - "$APP/ios/Info.plist")" '^MiApp$' \
  'an add ios: el ejecutable del plist es el nombre de la app'
contiene "$(plutil -extract CFBundleIdentifier raw -o - "$APP/ios/Info.plist")" \
  '^dev\.angularnative\.miapp$' 'an add ios: el identificador del plist sale del manifiesto'
contiene "$(cat "$APP/angular-native.json")" '"ios"' 'an add ios: queda apuntado en el manifiesto'

echo "<!-- el usuario añadió esto -->" >>"$APP/ios/Info.plist"
huella="$(shasum "$APP/ios/Info.plist")"
(cd "$APP" && "$AN" add ios >/dev/null 2>&1)
if [ "$huella" = "$(shasum "$APP/ios/Info.plist")" ]; then
  ok 'an add ios repetido: no pisa el plist del usuario'
else
  ko 'an add ios repetido: no pisa el plist del usuario'
fi
# Y quitar la línea de prueba: un comentario detrás de </plist> ya no es un plist
# válido, y lo que viene después lo lee `plutil`.
sed -i '' -e '$d' "$APP/ios/Info.plist"

(cd "$APP" && "$AN" add android >/dev/null 2>&1)
existe "$APP/android/AndroidManifest.xml" 'an add android: crea el manifiesto del proyecto'
contiene "$(cat "$APP/android/AndroidManifest.xml")" 'android:label="MiApp"' \
  'an add android: la etiqueta es el nombre de la app'
contiene "$(cat "$APP/android/AndroidManifest.xml")" 'package="dev\.angularnative"' \
  'an add android: el paquete sigue siendo el de las clases del shell'

salida="$(cd "$APP" && falla "$AN" add windows)"
contiene "$salida" 'ios y android' 'an add de una plataforma que no existe lo dice'

# ---------------------------------------------------------------------------
# an build
# ---------------------------------------------------------------------------
(cd "$APP" && "$AN" build >/dev/null 2>&1)
BUNDLE="$APP/.angular-native/build/bundle/main.js"
existe "$BUNDLE" 'an build: sale el bundle, dentro del proyecto y no del SDK'
if [ -e "$ROOT/build/bundle/mi-app" ]; then
  ko 'an build: no escribe nada en el SDK'
else
  ok 'an build: no escribe nada en el SDK'
fi

# Y que el bundle corra: el mismo `headless` que usan los demás scripts, que
# monta el pipeline entero menos la plataforma.
salida="$(cargo run -q -p an-bridge --example headless -- "$BUNDLE" 3 2>&1)"
contiene "$salida" 'Angular is running in development mode' 'el bundle arranca Angular'
contiene "$salida" 'Text#[0-9]+ .*"MiApp"' 'el título de la app llegó a un nodo Text nativo'
contiene "$salida" 'Button#[0-9]+ .*title=Van 1 toques' 'un toque llegó hasta la señal del componente'

# ---------------------------------------------------------------------------
# El plist y el manifiesto, desincronizados
# ---------------------------------------------------------------------------
# Cambiar el nombre de la app y no tocar el plist deja una app que se instala y
# no abre: iOS busca un ejecutable que no está. Tiene que pararse antes de
# compilar.
sed -i '' -e 's/"name": "MiApp"/"name": "OtroNombre"/' "$APP/angular-native.json"
salida="$(cd "$APP" && falla "$AN" ios --no-launch)"
contiene "$salida" 'CFBundleExecutable' 'un plist que no cuadra con el manifiesto para el build'
sed -i '' -e 's/"name": "OtroNombre"/"name": "MiApp"/' "$APP/angular-native.json"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
