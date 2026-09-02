#!/usr/bin/env python3
"""Las cuatro cosas que solo existen para el reloj, y que están en cuatro
sitios distintos.

Va aparte del `.sh` porque son comprobaciones de texto y no de proceso, y
porque el shell no es sitio para leer XML.
"""

import pathlib
import re
import sys

raiz = pathlib.Path(sys.argv[1])
fallos: list[str] = []


def leer(ruta: str) -> str:
    fichero = raiz / ruta
    if not fichero.is_file():
        fallos.append(f'  FALLO falta {ruta}')
        return ''
    return fichero.read_text()


manifiesto_reloj = leer('shells/android/AndroidManifest.wear.xml')
manifiesto_movil = leer('shells/android/AndroidManifest.xml')
estilos = leer('shells/android/res/values/styles.xml')
host = leer('shells/android/java/dev/angularnative/AnHost.java')
scroll = leer('shells/android/java/dev/angularnative/AnScrollView.java')
cli = leer('crates/an-cli/src/android.rs')
doc = leer('docs/wearos.md')
primitivas = leer('packages/primitives/src/primitives.ts')

# 1. El manifiesto del reloj declara la forma del aparato. Sin esto el APK es
#    el del teléfono con otro nombre: se instala igual y arranca igual, y por
#    eso no lo pilla nadie.
for exigido, motivo in [
    ('android.hardware.type.watch', 'la característica que lo hace una app de reloj'),
    ('com.google.android.wearable.standalone', 'que no necesita un teléfono emparejado'),
]:
    if exigido not in manifiesto_reloj:
        fallos.append(f'  FALLO el manifiesto del reloj no declara {exigido}: {motivo}')
if 'android.hardware.type.watch' in manifiesto_movil:
    fallos.append('  FALLO el manifiesto del teléfono declara ser de reloj')

# 2. Cada manifiesto apunta a un tema, y el tema tiene que existir. Un
#    `@style/` que no resuelve lo caza aapt2; uno que resuelve al tema del otro
#    aparato, no.
declarados = set(re.findall(r'<style name="([^"]+)"', estilos))
for ruta, texto in [
    ('AndroidManifest.xml', manifiesto_movil),
    ('AndroidManifest.wear.xml', manifiesto_reloj),
]:
    usado = re.search(r'android:theme="@style/([^"]+)"', texto)
    if not usado:
        fallos.append(f'  FALLO {ruta} no fija ningún android:theme')
    elif usado.group(1) not in declarados:
        fallos.append(
            f'  FALLO {ruta} usa @style/{usado.group(1)}, que no está en res/values/styles.xml'
        )

# El «atrás» del reloj. No hay botón físico: si el tema no lo trae, la app no
# tiene salida y no da ningún error, simplemente no pasa nada al deslizar.
if 'android:windowSwipeToDismiss' not in estilos:
    fallos.append('  FALLO el tema del reloj no activa windowSwipeToDismiss')

# 3. La lista de primitivas que no se montan.
#
#    Vive en dos métodos de Java —el motivo y la etiqueta— y se documenta en
#    `docs/wearos.md`. Tres sitios que se pueden separar: una primitiva con
#    motivo y sin etiqueta sale como «kind 7», y una en Java y no en el
#    documento solo se descubre cuando alguien la usa.
def kinds_de(metodo: str) -> set[str]:
    cuerpo = re.search(
        r'private static String ' + metodo + r'\(int kind\) \{(.*?)\n    \}',
        host,
        re.S,
    )
    if not cuerpo:
        fallos.append(f'  FALLO no encuentro AnHost.{metodo}')
        return set()
    return set(re.findall(r'case (KIND_\w+):', cuerpo.group(1)))


con_motivo = kinds_de('noVaEnElReloj')
con_etiqueta = kinds_de('kindName')
if con_motivo != con_etiqueta:
    solo_motivo = ', '.join(sorted(con_motivo - con_etiqueta))
    solo_etiqueta = ', '.join(sorted(con_etiqueta - con_motivo))
    if solo_motivo:
        fallos.append(f'  FALLO sin etiqueta en kindName: {solo_motivo}')
    if solo_etiqueta:
        fallos.append(f'  FALLO kindName nombra lo que sí se monta: {solo_etiqueta}')

etiquetas_reales = set(re.findall(r"@Directive\(\{ selector: '(an-[^']+)' \}\)", primitivas))
if not etiquetas_reales:
    fallos.append('  FALLO no pude leer ninguna etiqueta de packages/primitives')

cuerpo_etiquetas = re.search(
    r'private static String kindName\(int kind\) \{(.*?)\n    \}', host, re.S
)
etiquetas_declaradas = (
    set(re.findall(r'return "(an-[^"]+)";', cuerpo_etiquetas.group(1)))
    if cuerpo_etiquetas
    else set()
)
inventadas = sorted(etiquetas_declaradas - etiquetas_reales)
if inventadas:
    fallos.append(
        '  FALLO el reloj rechaza etiquetas que no existen: ' + ', '.join(inventadas)
    )
sin_documentar = sorted(t for t in etiquetas_declaradas if f'`{t}`' not in doc)
if sin_documentar:
    fallos.append(
        '  FALLO no van en el reloj y docs/wearos.md no las nombra: '
        + ', '.join(sin_documentar)
    )

# 4. La corona. Es un evento genérico y no un toque: si alguien la moviera a
#    `onTouchEvent` dejaría de llegar, y en el teléfono —donde no hay corona—
#    no fallaría ninguna otra comprobación.
for exigido, motivo in [
    ('SOURCE_ROTARY_ENCODER', 'la fuente de la corona'),
    ('onGenericMotionEvent', 'el camino por el que llegan sus eventos'),
    ('AXIS_SCROLL', 'el eje que trae las muescas'),
    ('setFocusableInTouchMode', 'sin foco, la corona no llega a la lista'),
]:
    if exigido not in scroll:
        fallos.append(f'  FALLO AnScrollView no menciona {exigido}: {motivo}')

# Y solo en el reloj: en el teléfono, una lista que pide el foco se lo quita al
# campo de texto que hubiera debajo.
if not re.search(r'if \(watch\) \{\s*\n\s*scroll\.enableRotary\(\);', host):
    fallos.append('  FALLO AnHost enciende la corona fuera de un reloj (o no la enciende)')

# Y la corona en crudo, que es la otra mitad: desplazar no es lo único que se
# hace con ella. La salida vive en `packages/primitives` y la ponen los dos
# relojes; si el host de Android deja de entregarla, la plantilla se suscribe a
# algo que no dispara y nadie se entera, que es el fallo que este documento
# lleva entero intentando evitar.
cuerpo_corona = re.search(r'private void setCrown\((.*?)\n    \}', host, re.S)
if not cuerpo_corona:
    fallos.append('  FALLO AnHost no entrega (crown): la salida existe y aquí no llega')
else:
    corona = cuerpo_corona.group(1)
    if 'if (!watch)' not in corona:
        fallos.append('  FALLO AnHost entrega (crown) fuera de un reloj, donde no hay corona')
    if 'Log.e' not in corona:
        fallos.append(
            '  FALLO fuera del reloj, (crown) se descarta en silencio en vez de decirlo'
        )
if 'crownIdle' not in host:
    fallos.append(
        '  FALLO falta (crownIdle): el sistema manda muescas y calla, así que el final '
        'lo tiene que contar el host'
    )
if not re.search(r'setOnGenericMotionListener\(this\)', host):
    fallos.append('  FALLO la corona en crudo no escucha por el camino de los eventos genéricos')

# 5. El margen de la pantalla redonda. Es geometría, no gusto: el lado del
#    cuadrado inscrito es d/√2. Una constante a ojo pasaría desapercibida.
if '(1 - 1 / Math.sqrt(2)) / 2' not in host:
    fallos.append('  FALLO ROUND_INSET ya no es el cuadrado inscrito en la circunferencia')
if 'isScreenRound' not in host:
    fallos.append('  FALLO el host adivina la forma de la pantalla en vez de preguntarla')

# 6. El CLI arma dos formas y no una. `Form::Watch` sin su manifiesto sería un
#    APK de teléfono con nombre de reloj.
if 'AndroidManifest.wear.xml' not in cli:
    fallos.append('  FALLO el CLI no conoce el manifiesto del reloj')
if 'ro.build.characteristics' not in cli:
    fallos.append('  FALLO el CLI no distingue un reloj de un teléfono al instalar')

# 7. Y que ese aparato llegue a `adb`.
#
#    Preguntar por el texto no basta, y esto lo aprendió el propio comprobador:
#    `ro.build.characteristics` estaba en el fichero, en una función que no
#    llamaba nadie. `install_and_launch` recibía la forma y el `--device` y no
#    usaba ninguno de los dos. Con dos aparatos arrancados, `adb` se planta; con
#    un teléfono solo, el APK del reloj se instala en el teléfono, arranca y
#    pinta, y nadie dice nada.
cuerpo_instalar = re.search(r'pub fn install_and_launch\((.*?)\n\}\n', cli, re.S)
if not cuerpo_instalar:
    fallos.append('  FALLO no encuentro install_and_launch en el CLI')
else:
    instalar = cuerpo_instalar.group(1)
    if 'pick_device' not in instalar:
        fallos.append(
            '  FALLO install_and_launch no elige aparato: manda el APK al que adb quiera'
        )
    for orden, motivo in [
        ('install', 'instalar'),
        ('start', 'lanzar'),
        ('force-stop', 'parar la app anterior'),
    ]:
        for argumentos in re.findall(r'\[([^\[\]]*"' + orden + r'"[^\[\]]*)\]', instalar):
            if '"-s"' not in argumentos:
                fallos.append(
                    f'  FALLO {motivo} sin `-s`: va al aparato que elija adb, no al elegido'
                )

for linea in fallos:
    print(linea)
if fallos:
    sys.exit(1)

print(f'  ok   el manifiesto del reloj declara su característica y su tema')
print(f'  ok   los {len(declarados)} temas de res/values/styles.xml cubren los dos manifiestos')
print(
    f'  ok   las {len(etiquetas_declaradas)} primitivas que no van en el reloj '
    'coinciden en Java y en docs/wearos.md'
)
print('  ok   la corona llega por onGenericMotionEvent y solo en el reloj')
print('  ok   (crown) y (crownIdle) los entrega el host, y fuera del reloj lo dicen')
print('  ok   el APK va al aparato con forma de reloj, y cada adb lleva su -s')
print('  ok   el margen de la pantalla redonda es el cuadrado inscrito')
