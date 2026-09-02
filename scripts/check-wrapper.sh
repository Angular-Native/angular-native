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
macos = (raiz / 'crates/an-macos/src/host.rs').read_text()

# Props que no son para ningún host: las consume el núcleo y ahí se acaban.
SOLO_NUCLEO = {
    'intrinsicWidth': 'la mide el layout para reservar el hueco de una imagen',
    'intrinsicHeight': 'la mide el layout para reservar el hueco de una imagen',
}

# Props que solo significan algo donde hay puntero.
#
# No son un hueco de iOS ni de Android: es que un dedo no tiene forma. Pedirle
# a esos dos hosts que miren `cursor` sería pedirles que miren algo que no
# pueden hacer, y meterla en PENDIENTES sería decir que algún día llegarán.
# Quien sí tiene que mirarlas es el host de escritorio, y eso se comprueba
# igual de fuerte que lo demás.
SOLO_PUNTERO = {
    'cursor': 'la forma del puntero; un dedo no tiene forma',
}

# Props que sí deberían llegar y todavía no llegan. Cada una con su motivo en
# docs/wrapper-nativo.md. La lista solo puede encoger.
PENDIENTES: dict[str, str] = {}

# Las props comunes son las claves de los `push({...})` de cada directiva, más
# las que alguna manda a mano —el tamaño de una imagen lo escribe su oyente de
# carga, no una entrada—.
comunes = set(re.findall(r"this\.set\('([^']+)'", directivas))
for cuerpo in re.findall(r"this\.push\(\{(.*?)\n    \}\)", directivas, re.S):
    comunes.update(re.findall(r"^      (\w+):", cuerpo, re.M))
comunes = sorted(comunes)
fallos = []
pendientes_vistas = set()
for prop in comunes:
    if prop in SOLO_NUCLEO:
        continue
    if prop in SOLO_PUNTERO:
        if f'"{prop}"' not in macos:
            fallos.append(f'  FALLO "{prop}" no la mira el host de macOS, que es el del puntero')
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

# El objeto de plataforma tiene que pasar por `pushPlatform()`, que es quien
# avisa de una clave que nadie va a mirar. Mandarlo con `set()` a pelo
# funcionaría —y por eso hay que impedirlo—: la clave viajaría y se perdería en
# silencio. Se cuentan: una entrada `[ios]` o `[android]` por empujón.
objetos = len(re.findall(r"^  readonly (?:ios|android) = input<", directivas, re.M))
empujones = directivas.count('this.pushPlatform(')
if objetos != empujones:
    fallos.append(
        f'  FALLO hay {objetos} entradas de plataforma y {empujones} pushPlatform(): '
        'alguna clave desconocida se perdería sin avisar'
    )

for linea in fallos:
    print(linea)
if fallos:
    print('  (el inventario y los motivos están en docs/wrapper-nativo.md)')
    sys.exit(1)

print(f'  ok   las {len(comunes) - len(SOLO_NUCLEO) - len(SOLO_PUNTERO) - len(pendientes_vistas)} '
      'props comunes llegan a los dos hosts')
print('  ok   las props de puntero las mira el host de escritorio: '
      + ', '.join(sorted(SOLO_PUNTERO)))
print(f'  ok   las {len(declaradas["ios"])} props de [ios] las mira solo iOS')
print(f'  ok   las {len(declaradas["android"])} props de [android] las mira solo Android')
print('  ok   todos los objetos de plataforma pasan por platform(), que avisa de lo que no reconoce')
if pendientes_vistas:
    print(f'  ok   {len(pendientes_vistas)} pendientes conocidas: {", ".join(sorted(pendientes_vistas))}')
PY
