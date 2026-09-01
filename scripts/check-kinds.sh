#!/usr/bin/env bash
# Que la etiqueta, el nombre de la primitiva y el código del protocolo sigan
# diciendo lo mismo.
#
# La etiqueta no se traduce con una tabla: se le quita `an-` y se junta en
# PascalCase. Eso quita una lista que mantener, pero deja una cadena de tres
# eslabones —selector de la directiva, `NATIVE_KINDS` del renderer, `KIND` del
# prelude— que se puede romper por cualquiera de ellos sin que nada falle a la
# vista: un nombre que no case sale del renderer como envoltorio, se monta como
# una vista de más y no da error. Mismo trato que `check-styles.sh` y por el
# mismo motivo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

raiz = pathlib.Path(sys.argv[1])

# Los dos nombres que el núcleo no puede cambiar. `Picker` y `TextEditor` se
# eligieron cuando la etiqueta no podía llamarse `Select` ni `TextArea` porque
# Angular no auto-cierra lo que se llame como un elemento de HTML; con prefijo
# la etiqueta ya recuperó su nombre, pero el enum de Rust y los tres hosts
# siguen con el viejo, y renombrarlos sería cambiar el protocolo.
NATURALES = {'Select': 'Picker', 'Textarea': 'TextEditor'}


def de_etiqueta(tag):
    """La misma regla que el renderer: quitar `an-` y juntar en PascalCase."""
    return re.sub(r'(^|-)([a-z])', lambda m: m.group(2).upper(), tag[len('an-'):])


fallos = []

# 1. Selector de cada directiva -> nombre de primitiva.
directivas = (raiz / 'packages/primitives/src/primitives.ts').read_text()
etiquetas = re.findall(r"@Directive\(\{ selector: '([^']+)' \}\)", directivas)
sin_prefijo = [t for t in etiquetas if not t.startswith('an-')]
if sin_prefijo:
    fallos.append(f"  FALLO estas etiquetas no llevan el prefijo an-: {', '.join(sin_prefijo)}")
desde_etiquetas = [de_etiqueta(t) for t in etiquetas if t.startswith('an-')]

# 2. La lista del renderer.
renderer = (raiz / 'packages/platform-native/src/native-node.ts').read_text()
cuerpo = renderer[renderer.index('const NATIVE_KINDS = ['):renderer.index('] as const')]
del_renderer = re.findall(r"'([^']+)'", cuerpo)

# 3. La del prelude, con su código.
prelude = (raiz / 'packages/runtime/runtime.js').read_text()
cuerpo = prelude[prelude.index('const KIND = {'):]
cuerpo = cuerpo[:cuerpo.index('}')]
del_prelude = {n: int(c) for n, c in re.findall(r'(\w+): (\d+)', cuerpo)}

# 4. La de Rust, con su código.
protocolo = (raiz / 'crates/an-bridge/src/protocol.rs').read_text()
cuerpo = protocolo[protocolo.index('pub fn kind_from_byte'):protocolo.index('pub fn kind_to_byte')]
del_nucleo = {int(c): n for c, n in re.findall(r'(\d+) => NodeKind::(\w+)', cuerpo)}

faltan = sorted(set(desde_etiquetas) - set(del_renderer))
sobran = sorted(set(del_renderer) - set(desde_etiquetas))
if faltan:
    fallos.append(f"  FALLO en NATIVE_KINDS faltan: {', '.join(faltan)}")
if sobran:
    fallos.append(f"  FALLO NATIVE_KINDS tiene primitivas sin directiva: {', '.join(sobran)}")

# `RawText` no se escribe en ninguna plantilla: no tiene etiqueta ni directiva.
faltan = sorted(set(del_renderer) - set(del_prelude))
sobran = sorted(set(del_prelude) - set(del_renderer) - {'RawText'})
if faltan:
    fallos.append(f"  FALLO el prelude no sabe mandar: {', '.join(faltan)}")
if sobran:
    fallos.append(f"  FALLO el prelude manda lo que el renderer no crea: {', '.join(sobran)}")

for nombre, codigo in sorted(del_prelude.items(), key=lambda par: par[1]):
    esperado = NATURALES.get(nombre, nombre)
    real = del_nucleo.get(codigo)
    if real is None:
        fallos.append(f'  FALLO "{nombre}" viaja con el código {codigo} y Rust no lo reconoce')
    elif real != esperado:
        fallos.append(
            f'  FALLO "{nombre}" viaja con el código {codigo}, que en Rust es "{real}"'
        )

for linea in fallos:
    print(linea)
if fallos:
    sys.exit(1)

print(f'  ok   las {len(etiquetas)} etiquetas an-* dan el nombre de su primitiva sin tabla de por medio')
print(f'  ok   las {len(del_prelude)} primitivas del prelude viajan con el código que Rust espera')
print(f'  ok   {len(NATURALES)} nombres naturales declarados: '
      + ', '.join(f'{js} es {rust} en el núcleo' for js, rust in sorted(NATURALES.items())))
PY
