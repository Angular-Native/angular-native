#!/usr/bin/env python3
"""Lo del host de macOS que se puede leer sin compilar nada.

Va aparte del `.sh` por lo mismo que en el reloj: aquí se comparan listas que
viven en ficheros distintos, y el shell no es sitio para eso. El `.sh` se queda
con lo que sí es un proceso —compilar, armar el `.app`, arrancarlo—.

Las tres cosas que se miran son las tres formas que tiene este host de fallar en
silencio:

1. Una primitiva que el núcleo monta y que el inventario no nombra. El `match`
   de `create` no compilaría, pero el informe de qué pinta macOS y qué no sí
   seguiría compilando, y saldría con un hueco.
2. Un `_ => {}` al final de un `match` de props o de estilos: la prop llega, no
   se aplica y no lo dice nadie.
3. Un shell que se copia el cliente del servidor de desarrollo en vez de usar el
   de `shells/shared`: dos copias que se separan la primera vez que alguien
   toca una.
"""

import pathlib
import re
import sys

raiz = pathlib.Path(sys.argv[1])
fallos: list[str] = []
oks: list[str] = []


def leer(ruta: str) -> str:
    fichero = raiz / ruta
    if not fichero.is_file():
        fallos.append(f'  FALLO falta {ruta}')
        return ''
    return fichero.read_text()


props = leer('crates/an-core/src/props.rs')
soporte = leer('crates/an-macos/src/support.rs')
host = leer('crates/an-macos/src/host.rs')
cli = leer('crates/an-cli/src/macos.rs')
raiz_swift = leer('shells/macos/Sources/RootViewController.swift')

# 1. La tabla del inventario contra el enum del núcleo.
#
# El enum se lee del núcleo y no se copia aquí: es la misma regla que en
# `check-kinds.sh`, donde una lista escrita a mano es justo lo que se quiere
# quitar de en medio.
cuerpo = props[props.index('pub enum NodeKind {'):]
cuerpo = cuerpo[:cuerpo.index('\n}')]
# `RawText` es interno: nunca llega a montarse como vista, así que no tiene por
# qué estar en un inventario de lo que se pinta.
del_nucleo = {n for n in re.findall(r'^    ([A-Z]\w+),$', cuerpo, re.M)} - {'RawText'}

del_inventario = set(re.findall(r'NodeKind::(\w+),\s*Support::', soporte))
del_inventario |= set(re.findall(r'\(\s*NodeKind::(\w+),\s*$', soporte, re.M))

faltan = sorted(del_nucleo - del_inventario)
sobran = sorted(del_inventario - del_nucleo)
if faltan:
    fallos.append(
        '  FALLO el inventario de macOS no dice con qué pinta: ' + ', '.join(faltan)
    )
if sobran:
    fallos.append('  FALLO el inventario nombra primitivas que no existen: ' + ', '.join(sobran))
if not faltan and not sobran and del_nucleo:
    nativas = len(re.findall(r'Support::Native\(', soporte))
    armadas = len(re.findall(r'Support::Assembled\(', soporte))
    ausentes = len(re.findall(r'Support::Missing\(', soporte))
    oks.append(
        f'  ok   las {len(del_nucleo)} primitivas montables están en el inventario '
        f'({nativas} con control del sistema, {armadas} armadas, {ausentes} que macOS no trae)'
    )

# 2. El reparto de props acaba avisando, no callando.
#
# Los `_ => {}` que hay en `set_prop` son otra cosa y están bien: reparten por
# *tipo de vista* —poner `textAlign` en un interruptor no hace nada y no tiene
# por qué avisar—. Lo que no puede haber es un comodín en el reparto por
# *nombre de prop*, que es donde se pierde lo que alguien escribió en una
# plantilla. Ese `match` tiene que terminar en dos brazos: el de lo que se
# ignora a sabiendas y el de lo que no conoce nadie.
cuerpo_prop = host[host.index('fn set_prop('):] if 'fn set_prop(' in host else ''
if '_ if ignored_reason(key).is_some()' not in cuerpo_prop:
    fallos.append('  FALLO set_prop no consulta IGNORED: lo descartado no se distingue del olvido')
elif 'prop desconocida' not in cuerpo_prop:
    fallos.append('  FALLO set_prop se traga las props que no conoce sin decirlo')
elif re.search(r'\n            _ => \{\s*\}\n        \}\n    \}', cuerpo_prop):
    fallos.append('  FALLO set_prop termina en un comodín vacío')
else:
    oks.append('  ok   una prop que macOS no mira sale por la salida de error')

# 3. Lo que macOS no puede honrar, dicho y con motivo.
#
# `IGNORED` es la lista de props que este host no aplica a propósito. Sin motivo
# escrito, el aviso que sale por pantalla no sirve de nada. Se recorta la lista
# antes de leerla: en un fichero de mil seiscientas líneas hay muchas tuplas de
# dos cadenas que no son esta.
if 'const IGNORED' in host:
    lista = host[host.index('const IGNORED'):]
    lista = lista[:lista.index('\n];')]
    # Cada entrada empieza en `("nombre"`; el motivo es todo lo que va hasta la
    # siguiente. Se corta así, y no con una expresión regular sobre la cadena
    # entera, porque en Rust un literal largo se parte en varias líneas con `\`
    # y ninguna expresión razonable lo recompone.
    trozos = re.split(r'\n    \(', '\n' + lista)
    ignoradas = []
    for trozo in trozos:
        m = re.match(r'\s*"(\w+)",(.*)', trozo, re.S)
        if m:
            ignoradas.append((m.group(1), m.group(2)))
    sin_motivo = [nombre for nombre, motivo in ignoradas if len(re.findall(r'[a-zA-Z]', motivo)) < 8]
    if not ignoradas:
        fallos.append('  FALLO no se pudo leer la lista IGNORED de host.rs')
    elif sin_motivo:
        fallos.append('  FALLO estas props se ignoran sin decir por qué: ' + ', '.join(sin_motivo))
    else:
        oks.append(f'  ok   las {len(ignoradas)} props que AppKit no cubre salen con su motivo')

# 4. El shell usa el cliente de desarrollo compartido, no una copia.
if 'shells/shared' not in cli:
    fallos.append('  FALLO el build de macOS no compila shells/shared: ¿se copió el DevClient?')
elif any((raiz / 'shells/macos/Sources').glob('DevClient*.swift')):
    fallos.append('  FALLO hay un DevClient dentro de shells/macos: el bueno vive en shells/shared')
else:
    oks.append('  ok   el shell comparte el cliente de desarrollo de shells/shared')

# 5. La recarga en caliente no desmonta lo que sigue en pie.
#
# El fallo ya se cometió una vez —una recarga en caliente tiraba las vistas y
# dejaba la ventana en negro—, y la única defensa es que el `clear()` esté
# detrás de la condición.
ffi = leer('crates/an-macos/src/ffi.rs')
if not re.search(r'if !reply\.hot \{\s*\n\s*rt\.mount\.clear\(\);', ffi):
    fallos.append(
        '  FALLO an_runtime_reload desmonta sin mirar si la recarga fue en caliente'
    )
else:
    oks.append('  ok   la recarga en caliente conserva las vistas montadas')

# 6. El viewport se puede mover en caliente, que es lo que distingue una ventana
#    de una pantalla de teléfono.
if 'an_runtime_set_viewport' not in raiz_swift or 'viewDidLayout' not in raiz_swift:
    fallos.append('  FALLO el shell no le cuenta al núcleo que la ventana cambió de tamaño')
else:
    oks.append('  ok   redimensionar la ventana rehace el layout')

for linea in oks:
    print(linea)
for linea in fallos:
    print(linea)
sys.exit(1 if fallos else 0)
