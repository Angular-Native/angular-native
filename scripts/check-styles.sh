#!/usr/bin/env bash
# Que la lista de nombres de estilo del lado JS siga siendo la del núcleo.
#
# Están duplicadas y no se puede evitar: el núcleo la necesita para resolver el
# layout y el renderer para avisar antes de mandar algo que nadie va a mirar.
# Lo que sí se puede evitar es que se separen sin que nadie se entere.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

raiz = pathlib.Path(sys.argv[1])
fuente = (raiz / 'crates/an-layout/src/style.rs').read_text()
cuerpo = fuente[fuente.index('impl StyleKey'):fuente.index('impl Keyword')]
nucleo = set()
for linea in cuerpo.splitlines():
    m = re.match(r'\s*("(?:[^"]+)"(?:\s*\|\s*"[^"]+")*)\s*=>', linea)
    if m:
        nucleo.update(re.findall(r'"([^"]+)"', m.group(1)))

lista = (raiz / 'packages/platform-native/src/style-names.ts').read_text()
js = set(re.findall(r"^\s*'([^']+)'", lista, re.M))

faltan = sorted(nucleo - js)
sobran = sorted(js - nucleo)
if faltan or sobran:
    if faltan:
        print(f"  FALLO en la lista de JS faltan: {', '.join(faltan)}")
    if sobran:
        print(f"  FALLO en la lista de JS sobran: {', '.join(sobran)}")
    print("  (regenera packages/platform-native/src/style-names.ts)")
    sys.exit(1)
print(f"  ok   los {len(nucleo)} nombres de estilo coinciden entre el núcleo y el renderer")
PY
