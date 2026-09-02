#!/usr/bin/env bash
# Biometría y llavero: que los métodos contesten y que lo que contestan se vea.
#
# El runner headless no tiene ni Swift ni Java, así que los dos plugins se
# montan con `AN_PLUGINS`, que da una respuesta fija por método. Lo que se
# comprueba no es la biometría —eso hay que verlo en un aparato— sino el camino
# entero: la app llama, la promesa resuelve, y lo que contestó el plugin acaba
# en la pantalla con la frase que le corresponde.
#
# La mitad que más importa es la de abajo: **sin plugin la promesa se rechaza y
# se ve**. Un método que se tragara la llamada dejaría esta misma pantalla con
# un guion y sin ninguna pista.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FALLO $1"; fail=1; }

contiene() {
  if grep -qF -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

echo "== biometría y llavero"

cargo an build examples/secrets >/dev/null
ok 'el bundle compila con los dos plugins resueltos'

BUNDLE=build/bundle/secrets/main.js
headless() { cargo run -q -p an-bridge --example headless -- "$BUNDLE" "$1" 2>&1; }

# ── Todo contesta ───────────────────────────────────────────────────────────
#
# El toque simulado cae en el primer nodo que escucha, que es «guardar».
TODO='{
  "biometrics": {
    "availability": { "status": "available", "kind": "faceId", "detail": "de mentira" },
    "authenticate": { "outcome": "success", "kind": "faceId", "detail": "de mentira" }
  },
  "keychain": {
    "has": true,
    "set": { "outcome": "saved", "detail": "de mentira" },
    "get": { "outcome": "found", "value": "secreto de prueba", "detail": "de mentira" },
    "remove": true
  }
}'
# La línea del sensor y la de si hay algo guardado son señales aparte de la del
# último resultado, así que el toque simulado no se las lleva por delante.
ARRANQUE="$(AN_PLUGINS="$TODO" headless 6)"
contiene "$ARRANQUE" '-- plugin de mentira: biometrics' 'el módulo de biometría se registra con su nombre'
contiene "$ARRANQUE" '-- plugin de mentira: keychain' 'y el del llavero también'
contiene "$ARRANQUE" 'Face ID, listo' 'availability llega y la app dice qué sensor hay'
contiene "$ARRANQUE" 'hay un secreto guardado' 'keychain.has llega sin pedir biometría'

# Y seis, que dan para el toque simulado. Cae en el primer nodo que escucha,
# que es «guardar», así que lo que se ve después es lo que contestó `set`.
CON="$(AN_PLUGINS="$TODO" headless 6)"
contiene "$CON" 'ahora hace falta tu cara' 'y el toque guardó: set resolvió'

# ── Cada final de la biometría tiene su frase ───────────────────────────────
#
# Es la razón de que `authenticate` no devuelva un booleano. Se comprueban tres
# de los once, que son los tres que piden que la app haga cosas distintas.
sin_reconocer() { # $1 outcome  $2 trozo de la frase que tiene que salir
  local respuesta
  respuesta="$(AN_PLUGINS='{
    "biometrics": { "availability": { "status": "'"$1"'", "kind": "none", "detail": "de mentira" } },
    "keychain": { "has": false }
  }' headless 4)"
  contiene "$respuesta" "$2" "availability=$1 sale como \"$2\""
}
sin_reconocer noHardware 'este aparato no tiene sensor'
sin_reconocer notEnrolled 'no hay ninguna cara'
sin_reconocer lockedOut 'demasiados intentos'

# ── Sin plugin, la promesa se rechaza y se ve ───────────────────────────────
SIN="$(headless 4)"
contiene "$SIN" 'falló: Error: no hay nin' 'sin plugin la promesa se rechaza diciendo que el módulo no existe'

# ── Un método sin respuesta preparada tampoco se traga ──────────────────────
#
# El llavero contesta a `has` pero no a `set`, y el toque llama a `set`.
MEDIAS="$(AN_PLUGINS='{
  "biometrics": { "availability": { "status": "available", "kind": "faceId", "detail": "x" } },
  "keychain": { "has": true }
}' headless 6)"
# El volcado del runner corta los textos a cuarenta caracteres, así que lo que
# se ve en la pantalla es el principio del rechazo. El nombre del método va
# dentro del mensaje, y de eso responde el propio plugin.
contiene "$MEDIAS" 'falló: Error: el plugin keychain' 'un método que el plugin no atiende rechaza la promesa'

exit "$fail"
