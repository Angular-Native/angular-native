#!/usr/bin/env bash
# Que ninguna prop de una directiva se pierda por el camino.
#
# Una prop viaja de la directiva al núcleo y del núcleo a los dos hosts. Si un
# host no la reconoce no pasa nada: no hay error, no hay traza, y el control se
# queda como estaba. Eso es exactamente lo que hizo caros los fallos de estilos
# —cuatro en un día, todos con la misma cara de "esto no hace nada"— y aquí
# vuelve a ser posible, así que se comprueba igual que allí.
#
# No demuestra que la prop haga lo correcto: demuestra que alguien la mira. Que
# haga lo correcto lo dice `check-controls.sh` sobre el volcado headless, y el
# aspecto final solo se ve en el dispositivo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

raiz = pathlib.Path(sys.argv[1])
directivas = (raiz / 'packages/primitives/src/primitives.ts').read_text()
ios = (raiz / 'crates/an-ios/src/host.rs').read_text()
android = (raiz / 'shells/android/java/dev/angularnative/AnHost.java').read_text()

# Props que no son para ningún host: las consume el núcleo y ahí se acaban.
SOLO_NUCLEO = {
    'intrinsicWidth': 'la mide el layout para reservar el hueco de una imagen',
    'intrinsicHeight': 'la mide el layout para reservar el hueco de una imagen',
}

# Props que sí deberían llegar y todavía no llegan. Cada una con su motivo en
# docs/wrapper-nativo.md. La lista solo puede encoger.
PENDIENTES = {
    'bounces': 'Android tiene overScrollMode y nadie lo ha conectado',
    'fontFamily': 'Android no crea el Typeface',
    'fontStyle': 'Android no crea el Typeface',
    'letterSpacing': 'la mide el núcleo pero no la dibuja ningún host',
    'lineHeight': 'la mide el núcleo pero no la dibuja ningún host',
    'refreshing': 'el arco de recarga de Android no se puede parar desde fuera',
    'secureTextEntry': 'un campo de contraseña se ve en claro en Android',
    'showsScrollIndicator': 'AnScrollView no expone la barra',
}

comunes = sorted(set(re.findall(r"this\.set\('([^']+)'", directivas)))
fallos = []
pendientes_vistas = set()
for prop in comunes:
    if prop in SOLO_NUCLEO:
        continue
    faltan = [n for n, h in (('iOS', ios), ('Android', android)) if f'"{prop}"' not in h]
    if not faltan:
        if prop in PENDIENTES:
            fallos.append(f'  FALLO "{prop}" ya llega a los dos hosts: sácala de PENDIENTES')
        continue
    if prop in PENDIENTES:
        pendientes_vistas.add(prop)
        continue
    fallos.append(f'  FALLO "{prop}" no la mira {" ni ".join(faltan)}')

# Props de una sola plataforma: viajan con su prefijo, y el prefijo dice quién
# tiene que mirarlas. Que aparezcan en el host de la otra sería una prop común
# disfrazada, y entonces no debería llevar prefijo.
HOSTS = {'ios': ('iOS', ios, 'Android', android), 'android': ('Android', android, 'iOS', ios)}
declaradas = {'ios': set(), 'android': set()}
for plataforma, cuerpo in re.findall(
    r"platformKeys\(\s*'[^']+',\s*'(ios|android)',\s*\[(.*?)\]\s*\)", directivas, re.S
):
    declaradas[plataforma].update(re.findall(r"'([^']+)'", cuerpo))

for plataforma, claves in declaradas.items():
    suyo, propio, ajeno_nombre, ajeno = HOSTS[plataforma]
    for clave in sorted(claves):
        if f'"{plataforma}:{clave}"' not in propio:
            fallos.append(f'  FALLO [{plataforma}] "{clave}" no la mira el host de {suyo}')
        if f'"{plataforma}:{clave}"' in ajeno:
            fallos.append(
                f'  FALLO [{plataforma}] "{clave}" también la mira {ajeno_nombre}: '
                'entonces es común y va sin prefijo'
            )

for linea in fallos:
    print(linea)
if fallos:
    print('  (el inventario y los motivos están en docs/wrapper-nativo.md)')
    sys.exit(1)

print(f'  ok   las {len(comunes) - len(SOLO_NUCLEO) - len(pendientes_vistas)} props comunes '
      'llegan a los dos hosts')
print(f'  ok   las {len(declaradas["ios"])} props de [ios] las mira solo iOS')
print(f'  ok   las {len(declaradas["android"])} props de [android] las mira solo Android')
if pendientes_vistas:
    print(f'  ok   {len(pendientes_vistas)} pendientes conocidas: {", ".join(sorted(pendientes_vistas))}')
PY
