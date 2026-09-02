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
eventos = leer('crates/an-macos/src/events.rs')
volteada = leer('crates/an-macos/src/flipped.rs')
plist = leer('shells/macos/Resources/Info.plist')
directivas = leer('packages/primitives/src/primitives.ts')
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
    en_otro_sitio = len(re.findall(r'Support::Elsewhere\(', soporte))
    oks.append(
        f'  ok   las {len(del_nucleo)} primitivas montables están en el inventario '
        f'({nativas} con control del sistema, {armadas} armadas, '
        f'{en_otro_sitio} que macOS pone fuera del árbol, {ausentes} que no trae)'
    )
    # Una primitiva declarada ausente tiene que avisar al montarse. Hoy no hay
    # ninguna, y por eso el camino que avisaba se quitó del host: si alguien
    # vuelve a declarar una, hay que volver a escribirlo o el nodo se montaría
    # como una caja vacía y en silencio, que es lo que este fichero persigue.
    if ausentes and 'HostView::Unsupported' not in host:
        fallos.append(
            '  FALLO el inventario declara una primitiva ausente y el host ya no tiene el '
            'camino que lo dice al montarla'
        )
    elif not ausentes:
        oks.append('  ok   no queda ninguna primitiva sin pintar en este host')

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

# 7. Los frameworks que usa el host los nombra quien enlaza.
#
# Es el fallo silencioso más caro de este host y ya se cobró una pieza: un
# `staticlib` de Rust no arrastra sus dependencias nativas, así que el
# `#[link(kind = "framework")]` del crate no llega al enlazador. Sin el
# `-framework` en el `swiftc` del CLI, el `.app` se arma, se firma, arranca y
# revienta al montar la primera vista de esa clase.
declarados = set()
for fichero in sorted((raiz / 'crates/an-macos/src').glob('*.rs')):
    declarados.update(
        re.findall(r'#\[link\(name = "(\w+)", kind = "framework"\)\]', fichero.read_text())
    )
if not declarados:
    fallos.append('  FALLO no se pudo leer ningún #[link] de framework en an-macos')
else:
    sin_enlazar = sorted(f for f in declarados if f'"{f}"' not in cli)
    if sin_enlazar:
        fallos.append(
            '  FALLO el host usa estos frameworks y el enlazado del .app no los nombra: '
            + ', '.join(sin_enlazar)
        )
    else:
        oks.append(
            f'  ok   los {len(declarados)} frameworks del host los nombra el enlazado del .app'
        )

# 8. La cabecera va a la barra de título de la ventana, no a una vista.
#
# Es la decisión de este host sobre `<an-navigation-bar>`, y tiene dos mitades
# que se pueden romper por separado: que el `[title]` acabe en la ventana, y
# que el nodo no deje hueco donde no hay nada. Sin la segunda, la pantalla
# saldría con una franja vacía arriba y nadie sabría de dónde sale.
controles = leer('crates/an-macos/src/controls.rs')
if 'Support::Elsewhere' not in soporte or 'NavigationBar' not in soporte:
    fallos.append('  FALLO el inventario no dice dónde acaba la cabecera de navegación')
elif 'window.setTitle' not in host:
    fallos.append('  FALLO el [title] de <an-navigation-bar> no llega a la barra de título')
elif not re.search(r'"NavigationBar"\.to_owned\(\), \(0\.0, 0\.0\)', controles):
    fallos.append(
        '  FALLO la cabecera no mide cero en macOS: dejaría una franja vacía bajo la barra '
        'de título'
    )
else:
    oks.append('  ok   el [title] de la cabecera acaba en la barra de título y el nodo no ocupa')

# 9. El deslizamiento es el del sistema, no un `pan` con un umbral inventado.
#
# AppKit no tiene reconocedor de deslizamiento, y la salida fácil habría sido
# medir un arrastre y decidir por nuestra cuenta cuándo cuenta. El gesto de
# verdad existe —`swipeWithEvent:`, con el umbral y el número de dedos que
# decide el sistema— y es el que hay que atender.
if 'swipeWithEvent' not in volteada:
    fallos.append('  FALLO nadie atiende swipeWithEvent:, así que (swipeLeft) no llega nunca')
elif 'msg_send![super(self), swipeWithEvent: event]' not in volteada:
    fallos.append(
        '  FALLO una vista que no escucha el deslizamiento se lo traga en vez de pasarlo a '
        'la cadena de responder'
    )
else:
    oks.append('  ok   el deslizamiento es el evento del sistema y el que no escucha lo pasa')

# 10. El puntero: hover y cursor, y ningún cursor dibujado a mano.
if '(hover)' in directivas and 'hover' not in soporte:
    fallos.append('  FALLO la primitiva declara (hover) y el host de macOS no lo conoce')
elif 'NSTrackingArea' not in eventos:
    fallos.append('  FALLO (hover) no se monta sobre un NSTrackingArea')
else:
    cursores = re.findall(r'"([a-z-]+)" => NSCursor::(\w+)\(\)', eventos)
    # El vocabulario se lee del tipo `NativeCursor`, recortado antes de mirarlo:
    # una expresión suelta sobre el fichero entero cogería cualquier otra unión
    # de cadenas y exigiría un `NSCursor` para valores que no son cursores.
    union = directivas[directivas.index('export type NativeCursor ='):]
    union = union[:union.index('\n\n')]
    vocabulario = set(re.findall(r"'([a-z-]+)'", union))
    faltan_cursores = sorted(vocabulario - {nombre for nombre, _ in cursores})
    if faltan_cursores:
        fallos.append(
            '  FALLO estos cursores los acepta la primitiva y macOS no los pone: '
            + ', '.join(faltan_cursores)
        )
    else:
        oks.append(
            f'  ok   los {len(cursores)} punteros son NSCursor del sistema, ninguno dibujado'
        )

# 11. Enseñar dónde estás pide permiso, y el permiso se declara o no se pide.
#
# `setShowsUserLocation:` sin la clave en el plist no falla: el sistema deniega
# el permiso él solo y el punto no sale nunca, sin error y sin nada que mirar.
if 'setShowsUserLocation' in host and 'NSLocationUsageDescription' not in plist:
    fallos.append(
        '  FALLO el mapa pide la ubicación y el Info.plist no declara para qué: el permiso se '
        'deniega solo y el punto no sale'
    )
elif 'setShowsUserLocation' in host:
    oks.append('  ok   el mapa declara para qué pide la ubicación antes de pedirla')

for linea in oks:
    print(linea)
for linea in fallos:
    print(linea)
sys.exit(1 if fallos else 0)
