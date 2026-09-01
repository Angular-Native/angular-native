#!/usr/bin/env bash
# Plugins: que se descubran, que se enlacen y que contesten.
#
# Cubre las tres mitades del sistema sin simulador ni emulador:
#
#   1. Descubrimiento — `an plugins` encuentra el plugin por las dependencias
#      del package.json de la app, y dice qué plataformas cubre.
#   2. La plataforma que falta da la cara — un plugin que solo trae iOS para el
#      build de Android, con un mensaje que dice cuál y por qué.
#   3. La llamada va y vuelve — el ejemplo llama al módulo, la promesa resuelve
#      y lo que contestó acaba en pantalla; sin plugin, se rechaza y se ve.
#
# Y luego lo que sí cuesta medio minuto pero compila de verdad: armar el .app y
# el APK del ejemplo. Ahí es donde se comprueba que las fuentes Swift y Java del
# plugin entran en la misma invocación de `swiftc` y de `javac` que el shell, y
# que el registro generado compila.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FALLO $1"; fail=1; }

# Comprueba que una salida contenga un patrón. El `-e` no sobra: hay patrones
# que empiezan por guion y sin él grep los toma por opciones suyas.
contiene() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

echo "== plugins"

# ── 1. Descubrimiento ───────────────────────────────────────────────────────
LISTA="$(cargo an plugins examples/clipboard 2>&1)"
contiene "$LISTA" '^clipboard  \(@angular-native/plugin-clipboard\)' \
  'el plugin se descubre por la dependencia del package.json'
contiene "$LISTA" 'ios \+ android' 'y dice que cubre las dos plataformas'
contiene "$LISTA" 'packages/plugin-clipboard' 'y de dónde salió el paquete'

SIN="$(cargo an plugins examples/kitchen 2>&1)"
contiene "$SIN" 'no depende de ningún plugin' 'una app sin plugins lo dice y no inventa ninguno'

for platform in ios android; do
  if cargo an plugins examples/clipboard --platform "$platform" >/dev/null 2>&1; then
    ok "el ejemplo pasa la comprobación de $platform"
  else
    ko "el ejemplo debería pasar la comprobación de $platform"
  fi
done

# ── 2. La plataforma que falta da la cara ───────────────────────────────────
#
# Con un plugin de mentira que solo declara iOS. Se monta en `build/` para no
# meter un paquete de pega en los workspaces del repo: lo que se prueba es la
# resolución de dependencias, y `node_modules` dentro de la app es justo por
# donde Node —y `an`— buscan primero.
FIXTURE="$ROOT/build/plugins-fixture"
rm -rf "$FIXTURE"
mkdir -p "$FIXTURE/app/node_modules/@fixture/solo-ios/native/ios"
cat >"$FIXTURE/app/package.json" <<'JSON'
{
  "name": "@fixture/app-solo-ios",
  "private": true,
  "dependencies": { "@fixture/solo-ios": "0.0.1" }
}
JSON
# `an` solo pide que exista para reconocer el directorio como app.
echo '{}' >"$FIXTURE/app/tsconfig.json"
cat >"$FIXTURE/app/node_modules/@fixture/solo-ios/package.json" <<'JSON'
{
  "name": "@fixture/solo-ios",
  "version": "0.0.1",
  "angularNative": {
    "module": "soloios",
    "ios": { "sources": "native/ios", "register": "SoloIosPlugin" }
  }
}
JSON
echo '// solo para que el directorio tenga una fuente' \
  >"$FIXTURE/app/node_modules/@fixture/solo-ios/native/ios/SoloIosPlugin.swift"

if cargo an plugins build/plugins-fixture/app --platform ios >/dev/null 2>&1; then
  ok 'un plugin que solo trae iOS pasa la comprobación de iOS'
else
  ko 'un plugin que solo trae iOS debería pasar la comprobación de iOS'
fi

if NEGATIVO="$(cargo an plugins build/plugins-fixture/app --platform android 2>&1)"; then
  ko 'compilar para Android con un plugin que solo trae iOS tendría que fallar'
else
  ok 'compilar para Android con un plugin que solo trae iOS falla'
  contiene "$NEGATIVO" '@fixture/solo-ios' 'y el mensaje dice qué paquete es'
  contiene "$NEGATIVO" 'no se puede compilar para Android' 'y para qué plataforma'
  contiene "$NEGATIVO" 'angularNative.android' 'y qué hay que hacer para arreglarlo'
fi
rm -rf "$FIXTURE"

# ── 3. La llamada va y vuelve ───────────────────────────────────────────────
cargo an build examples/clipboard >/dev/null
ok 'el bundle compila con el import del plugin resuelto'

# Con el plugin contestando. `AN_PLUGINS` monta un módulo de respuestas fijas
# por cada nombre: los plugins de verdad son Swift y Java, y aquí no hay
# ninguno de los dos, pero el camino —registro, llamada, promesa— es el mismo.
CON="$(AN_PLUGINS='{"clipboard":{"read":"texto de prueba","write":null,"hasText":true}}' \
  cargo run -q -p an-bridge --example headless -- build/bundle/clipboard/main.js 6 2>&1)"
contiene "$CON" '-- plugin de mentira: clipboard' 'el módulo se registra con el nombre de su package.json'
contiene "$CON" 'en el portapapeles: texto de prueba' 'lo que contestó el plugin llega a la pantalla'
contiene "$CON" '"copiado"' 'y el toque escribió: write resolvió y disparó la relectura'

# Sin plugin: la promesa se rechaza y se ve. Es la mitad que más importa —un
# método que se traga la llamada dejaría esta misma pantalla con un guion y sin
# ninguna pista de por qué.
SIN_PLUGIN="$(cargo run -q -p an-bridge --example headless -- build/bundle/clipboard/main.js 4 2>&1)"
contiene "$SIN_PLUGIN" 'el portapapeles falló: Error: no hay nin' \
  'sin plugin la promesa se rechaza diciendo que el módulo no existe'

# Un método que el plugin no declara tampoco se traga.
OTRO="$(AN_PLUGINS='{"clipboard":{"read":"algo"}}' \
  cargo run -q -p an-bridge --example headless -- build/bundle/clipboard/main.js 4 2>&1)"
contiene "$OTRO" 'el portapapeles falló: Error: el plugin' \
  'un método que el plugin no atiende rechaza la promesa'

# ── 4. Que compile de verdad ────────────────────────────────────────────────
#
# Lo anterior no toca ni Swift ni Java. Armar el .app y el APK sin instalarlos
# sí: `swiftc` compila el plugin junto al shell, `javac` lo mismo, y los dos
# tienen que ver el registro que `an` acaba de generar.
if cargo an ios examples/clipboard --no-launch >/dev/null 2>&1; then
  ok 'swiftc compila el plugin y su registro dentro del .app'
else
  ko 'el .app con el plugin no llegó a armarse'
  cargo an ios examples/clipboard --no-launch 2>&1 | tail -20
fi
GENERADO="build/ios/generated/AnGeneratedPlugins.swift"
if [ -f "$GENERADO" ]; then
  contiene "$(cat "$GENERADO")" 'AnPluginRegistry.register\("clipboard", AnClipboardPlugin\(\)\)' \
    'el registro de iOS enlaza el nombre del manifiesto con el tipo Swift'
else
  ko 'no se generó el registro de iOS'
fi

APK="$(cargo an android examples/clipboard --no-launch 2>/dev/null | tail -1)"
if [ -f "$APK" ]; then
  ok 'javac compila el plugin y su registro dentro del APK'
else
  ko 'el APK con el plugin no llegó a armarse'
  cargo an android examples/clipboard --no-launch 2>&1 | tail -20
fi
GENERADO="build/android/gen-plugins/dev/angularnative/AnGeneratedPlugins.java"
if [ -f "$GENERADO" ]; then
  contiene "$(cat "$GENERADO")" \
    'AnPluginRegistry.register\("clipboard", new dev.angularnative.plugins.ClipboardPlugin\(\)\);' \
    'el registro de Android enlaza el nombre del manifiesto con la clase Java'
else
  ko 'no se generó el registro de Android'
fi

exit "$fail"
