#!/usr/bin/env bash
# Permisos de los plugins: que se fundan, que un choque pare el build, y que
# lo fundido acabe dentro del `.app` y del APK.
#
# Cubre lo que se puede comprobar sin ningún aparato:
#
#   1. Un plugin declara una clave del `Info.plist` y acaba en el plist del
#      `.app`. Es la mitad que evita que Face ID mate la app al primer intento.
#   2. Dos plugins que piden la misma clave con el **mismo** valor no chocan:
#      dicen lo mismo y se escribe una vez. Es lo que pasa de verdad entre
#      biometrics y keychain.
#   3. Dos plugins que la piden con valores **distintos** paran el build, y el
#      mensaje dice los dos paquetes y los dos valores.
#   4. Lo mismo en Android con `uses-feature`, donde el choque es el
#      `android:required`. Los permisos no pueden chocar y también se comprueba.
#   5. Un valor que no se sabe fundir —un diccionario anidado— se rechaza en
#      vez de colarse a medias.
#   6. Los derechos acaban dentro del binario, en `__TEXT,__entitlements`, que
#      es lo que le permite al llavero guardar algo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FALLO $1"; fail=1; }

contiene() {
  if grep -qF -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

echo "== permisos de los plugins"

FIXTURE="$ROOT/build/permisos-fixture"
rm -rf "$FIXTURE"

# Monta una app de mentira con dos plugins de mentira. Se hace en `build/` y no
# en los workspaces del repo: lo que se prueba es la resolución de
# dependencias, y `node_modules` dentro de la app es por donde `an` mira
# primero, igual que Node.
#
#   $1 directorio de la app   $2 JSON de angularNative del primero
#   $3 JSON de angularNative del segundo (vacío: solo hay uno)
monta() {
  local app="$FIXTURE/$1"
  mkdir -p "$app/node_modules/@fixture/uno/native/ios" \
           "$app/node_modules/@fixture/uno/native/android" \
           "$app/node_modules/@fixture/dos/native/ios" \
           "$app/node_modules/@fixture/dos/native/android"
  echo '// una fuente, para que el directorio no esté vacío' \
    | tee "$app/node_modules/@fixture/uno/native/ios/Uno.swift" \
          "$app/node_modules/@fixture/uno/native/android/Uno.java" \
          "$app/node_modules/@fixture/dos/native/ios/Dos.swift" \
          "$app/node_modules/@fixture/dos/native/android/Dos.java" >/dev/null
  echo '{}' >"$app/tsconfig.json"
  printf '%s' "$2" >"$app/node_modules/@fixture/uno/package.json"
  if [ -n "${3:-}" ]; then
    printf '%s' "$3" >"$app/node_modules/@fixture/dos/package.json"
    cat >"$app/package.json" <<'JSON'
{ "name": "@fixture/app", "private": true,
  "dependencies": { "@fixture/uno": "0.0.1", "@fixture/dos": "0.0.1" } }
JSON
  else
    rm -rf "$app/node_modules/@fixture/dos"
    cat >"$app/package.json" <<'JSON'
{ "name": "@fixture/app", "private": true,
  "dependencies": { "@fixture/uno": "0.0.1" } }
JSON
  fi
}

manifiesto() { # $1 nombre de módulo  $2 texto del plist  $3 required del uses-feature
  cat <<JSON
{ "name": "@fixture/$1", "version": "0.0.1",
  "angularNative": {
    "module": "$1",
    "ios": { "sources": "native/ios", "register": "P$1",
             "plist": { "NSFaceIDUsageDescription": "$2" } },
    "android": { "sources": "native/android", "register": "dev.fixture.P$1",
                 "manifest": {
                   "uses-permission": ["android.permission.USE_BIOMETRIC"],
                   "uses-feature": { "android.hardware.fingerprint": $3 } } } } }
JSON
}

# ── 1 y 2. Misma clave, mismo valor: no es un choque ────────────────────────
monta acuerdo "$(manifiesto uno 'Para entrar.' false)" "$(manifiesto dos 'Para entrar.' false)"
if ACUERDO="$(cargo an plugins build/permisos-fixture/acuerdo --platform ios 2>&1)"; then
  ok 'dos plugins que piden la misma clave con el mismo valor no chocan'
else
  ko 'dos plugins que dicen lo mismo no deberían parar el build'
  echo "$ACUERDO" | tail -5
fi
if cargo an plugins build/permisos-fixture/acuerdo --platform android >/dev/null 2>&1; then
  ok 'y lo mismo con el manifiesto de Android'
else
  ko 'el manifiesto de Android tampoco debería chocar'
fi

# ── 3. Misma clave, valores distintos: se para ──────────────────────────────
monta choque "$(manifiesto uno 'Para entrar.' false)" "$(manifiesto dos 'Otra cosa.' false)"
if CHOQUE="$(cargo an plugins build/permisos-fixture/choque --platform ios 2>&1)"; then
  ko 'dos plugins que piden la misma clave con valores distintos tendrían que parar el build'
else
  ok 'dos plugins que piden la misma clave con valores distintos paran el build'
  contiene "$CHOQUE" 'NSFaceIDUsageDescription' 'y el mensaje dice qué clave es'
  contiene "$CHOQUE" '@fixture/uno' 'y cuál es el primer paquete'
  contiene "$CHOQUE" '@fixture/dos' 'y cuál es el segundo'
  contiene "$CHOQUE" 'Para entrar.' 'y qué pedía cada uno'
  contiene "$CHOQUE" 'Otra cosa.' 'y qué pedía el otro'
fi

# ── 4. El choque de Android es el `android:required` ────────────────────────
monta feature "$(manifiesto uno 'Igual.' true)" "$(manifiesto dos 'Igual.' false)"
if FEATURE="$(cargo an plugins build/permisos-fixture/feature --platform android 2>&1)"; then
  ko 'una característica pedida como obligatoria y como opcional tendría que parar el build'
else
  ok 'una característica pedida como obligatoria y como opcional para el build'
  contiene "$FEATURE" 'android.hardware.fingerprint' 'y el mensaje dice cuál es'
  contiene "$FEATURE" 'android:required' 'y en qué no se ponen de acuerdo'
fi
# El mismo par no choca en iOS: ahí piden lo mismo. Que un choque de Android no
# se cuele en el build de iOS es la otra mitad de la comprobación.
if cargo an plugins build/permisos-fixture/feature --platform ios >/dev/null 2>&1; then
  ok 'y ese mismo par no molesta al build de iOS, donde piden lo mismo'
else
  ko 'un choque de Android no tiene por qué parar el build de iOS'
fi

# ── 5. Un valor que no se sabe fundir ───────────────────────────────────────
monta anidado '{ "name": "@fixture/uno", "version": "0.0.1",
  "angularNative": { "module": "uno",
    "ios": { "sources": "native/ios", "register": "Puno",
             "plist": { "NSAppTransportSecurity": { "NSAllowsLocalNetworking": true } } } } }'
if ANIDADO="$(cargo an plugins build/permisos-fixture/anidado --platform ios 2>&1)"; then
  ko 'un diccionario anidado en el plist tendría que rechazarse'
else
  ok 'un diccionario anidado en el plist se rechaza en vez de colarse a medias'
  contiene "$ANIDADO" 'NSAppTransportSecurity' 'y el mensaje dice qué clave es'
fi

rm -rf "$FIXTURE"

# ── 6. Y que lo fundido llegue de verdad al .app ────────────────────────────
#
# Esto sí compila: arma el `.app` del ejemplo, que depende de los dos plugins
# de verdad. Los dos piden `NSFaceIDUsageDescription` con el mismo texto, que
# es justo el caso que tiene que pasar sin decir nada.
if cargo an ios examples/secrets --no-launch >/dev/null 2>&1; then
  ok 'el .app del ejemplo se arma con los dos plugins dentro'
else
  ko 'el .app del ejemplo no llegó a armarse'
  cargo an ios examples/secrets --no-launch 2>&1 | tail -20
fi

PLIST="build/ios/AngularNative.app/Info.plist"
if [ -f "$PLIST" ]; then
  if VALOR="$(plutil -extract NSFaceIDUsageDescription raw -o - "$PLIST" 2>/dev/null)"; then
    ok "la clave del plugin acabó en el Info.plist del .app"
    contiene "$VALOR" 'comprobar que eres tú' 'y con el texto que declaró el plugin'
  else
    ko 'NSFaceIDUsageDescription no llegó al Info.plist del .app'
  fi
  # Y la del shell sigue donde estaba: fundir no es reemplazar.
  if plutil -extract CFBundleExecutable raw -o - "$PLIST" >/dev/null 2>&1; then
    ok 'y lo que ya traía el plist del proyecto sigue ahí'
  else
    ko 'fundir el plist se llevó por delante lo que ya había'
  fi
else
  ko 'no se armó el .app'
fi

# Los derechos van dentro del binario, no en la firma: `codesign -d` no los ve,
# pero la sección `__TEXT,__entitlements` está y lleva el grupo del llavero.
BIN="build/ios/AngularNative.app/AngularNative"
if [ -f "$BIN" ]; then
  if otool -s __TEXT __entitlements "$BIN" 2>/dev/null | grep -q "__entitlements"; then
    ok 'el binario lleva la sección __TEXT,__entitlements'
    # El volcado de `otool` viene en palabras de cuatro bytes y del revés, así
    # que se busca el texto en el binario tal cual en vez de recomponerlo.
    for aguja in keychain-access-groups dev.angularnative.playground; do
      if python3 -c 'import sys; sys.exit(0 if sys.argv[2].encode() in open(sys.argv[1],"rb").read() else 1)' \
           "$BIN" "$aguja"; then
        ok "y el binario lleva dentro $aguja"
      else
        ko "el binario no lleva $aguja"
      fi
    done
  else
    ko 'el binario no lleva los derechos, y sin ellos el llavero contesta -34018'
  fi
else
  ko 'no hay binario que mirar'
fi

exit "$fail"
