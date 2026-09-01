#!/usr/bin/env python3
"""Baja Material Components y todo lo que arrastra.

Android no trae en la plataforma ni la barra de navegación inferior ni los
botones de Material 3: viven en `com.google.android.material`, que es una
librería aparte. Gradle la resolvería sola; aquí no hay Gradle, así que se
resuelve a mano leyendo los POM y se deja todo en `vendor/android/`.

No es un gestor de dependencias: no resuelve conflictos de versión con
ninguna estrategia sofisticada —se queda con la primera que ve, que es la más
cercana a la raíz, igual que hace Gradle— y no toca los `scope` de prueba.
"""

import pathlib
import re
import shutil
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

REPOS = [
    "https://dl.google.com/dl/android/maven2",
    "https://repo1.maven.org/maven2",
]

RAIZ = pathlib.Path(__file__).resolve().parent.parent
DESTINO = RAIZ / "vendor" / "android"
EXTRAIDO = DESTINO / "extracted"
BUILD = DESTINO / "build"
NS = {"m": "http://maven.apache.org/POM/4.0.0"}

# Lo que se pide. El resto sale de aquí.
SEMILLA = [("com.google.android.material", "material", "1.13.0")]

# Lo que no hace falta: anotaciones que solo existen en tiempo de compilación
# y cosas de Kotlin que no usamos.
IGNORAR = {
    "com.google.errorprone:error_prone_annotations",
    "org.jetbrains.kotlin:kotlin-bom",
    "com.google.guava:listenablefuture",
    # Desde Kotlin 1.8 `kotlin-stdlib` trae dentro lo que antes estaba en
    # estos tres. Siguen publicándose para no romper a quien los pida, pero
    # traer los dos juegos deja las mismas clases dos veces y `d8` se planta.
    "org.jetbrains.kotlin:kotlin-stdlib-jdk7",
    "org.jetbrains.kotlin:kotlin-stdlib-jdk8",
    "org.jetbrains.kotlin:kotlin-stdlib-common",
}


def descargar(url: str) -> bytes | None:
    try:
        with urllib.request.urlopen(url, timeout=30) as respuesta:
            return respuesta.read()
    except Exception:
        return None


def buscar(grupo: str, artefacto: str, version: str, extension: str) -> bytes | None:
    ruta = f"{grupo.replace('.', '/')}/{artefacto}/{version}/{artefacto}-{version}.{extension}"
    for repo in REPOS:
        datos = descargar(f"{repo}/{ruta}")
        if datos:
            return datos
    return None


def texto(nodo, etiqueta: str) -> str | None:
    hijo = nodo.find(f"m:{etiqueta}", NS)
    return hijo.text.strip() if hijo is not None and hijo.text else None


def gestion_de(raiz, profundidad: int = 0) -> dict[str, str]:
    """Las versiones que fija `dependencyManagement`, incluidos los BOM.

    Una sección de gestión puede no listar versiones sino *importar* otro POM
    que las lista —eso es un BOM—, así que hay que seguirlos. Sin esto, media
    androidx se queda sin versión y no se descarga.
    """
    gestionadas: dict[str, str] = {}
    if profundidad > 4:
        return gestionadas
    for dep in raiz.iterfind("m:dependencyManagement/m:dependencies/m:dependency", NS):
        g, a, v = texto(dep, "groupId"), texto(dep, "artifactId"), texto(dep, "version")
        if not (g and a and v):
            continue
        if (texto(dep, "scope") or "") == "import":
            bom = buscar(g, a, v, "pom")
            if bom:
                gestionadas.update(gestion_de(ET.fromstring(bom), profundidad + 1))
            continue
        gestionadas[f"{g}:{a}"] = v
    return gestionadas


def concreta(version: str | None) -> str | None:
    """Una versión concreta a partir de lo que declare el POM.

    Maven admite rangos: `[1.7.0]` es "exactamente esa" y `[1.7.0,2.0)` es
    "de esa en adelante". Los dos se resuelven a la cota de abajo, que es lo
    que quiere quien los escribe. Antes se descartaba todo lo que llevara
    corchetes, y con ello media androidx: appcompat pide sus recursos como
    `[1.7.0]` y se quedaban fuera sin que nada fallara hasta que la app
    arrancaba y no encontraba una clase.
    """
    if not version:
        return None
    version = version.strip()
    if "$" in version:
        return None
    if not version.startswith(("[", "(")):
        return version
    dentro = version.strip("[]()")
    cota = dentro.split(",")[0].strip()
    return cota or None


def orden(version: str) -> tuple:
    """Compara versiones por sus números; lo que no sea número va detrás."""
    partes = re.findall(r"\d+|[a-zA-Z]+", version)
    return tuple((0, int(p)) if p.isdigit() else (1, p) for p in partes)


def una_vuelta(gestion_global: dict[str, str]) -> dict[str, tuple[str, str, str, str]]:
    """Camina los POM una vez. Ante dos versiones del mismo, gana la alta."""
    elegido: dict[str, tuple[str, str, str, str]] = {}
    cola = list(SEMILLA)
    while cola:
        grupo, artefacto, version = cola.pop(0)
        clave = f"{grupo}:{artefacto}"
        if clave in IGNORAR:
            continue
        anterior = elegido.get(clave)
        if anterior and orden(anterior[2]) >= orden(version):
            continue
        pom = buscar(grupo, artefacto, version, "pom")
        if pom is None:
            print(f"  aviso: sin POM para {clave}:{version}", file=sys.stderr)
            continue
        raiz = ET.fromstring(pom)
        empaquetado = texto(raiz, "packaging") or "jar"
        elegido[clave] = (grupo, artefacto, version, empaquetado)

        # La gestión se acumula entre POM: una dependencia puede venir sin
        # versión aquí y tenerla fijada en la sección de gestión de otro
        # módulo de la misma familia, que es como androidx reparte las suyas.
        gestion_global.update(gestion_de(raiz))

        for dep in raiz.iterfind("m:dependencies/m:dependency", NS):
            alcance = texto(dep, "scope") or "compile"
            opcional = (texto(dep, "optional") or "false") == "true"
            if alcance not in ("compile", "runtime") or opcional:
                continue
            g = texto(dep, "groupId")
            a = texto(dep, "artifactId")
            if not (g and a):
                continue
            v = concreta(texto(dep, "version")) or gestion_global.get(f"{g}:{a}")
            if not v:
                print(f"  aviso: {g}:{a} sin versión utilizable, se salta", file=sys.stderr)
                continue
            cola.append((g, a, v))
    return elegido


def resolver() -> dict[str, tuple[str, str, str, str]]:
    """Resuelve el árbol.

    Se camina dos veces: en la primera vuelta se aprenden las versiones que
    fija cada sección de gestión, y en la segunda ya se pueden resolver las
    dependencias que llegaron sin versión antes de conocerlas. Sin la segunda
    vuelta se queda fuera media androidx, y eso no se ve hasta que falta una
    clase con la app ya corriendo.
    """
    gestion: dict[str, str] = {}
    una_vuelta(gestion)
    return una_vuelta(gestion)


def main() -> int:
    DESTINO.mkdir(parents=True, exist_ok=True)
    artefactos = resolver()
    print(f"==> {len(artefactos)} artefactos")
    esperados: set[str] = set()
    for clave, (grupo, artefacto, version, empaquetado) in sorted(artefactos.items()):
        extension = "aar" if empaquetado == "aar" else "jar"
        salida = DESTINO / f"{artefacto}-{version}.{extension}"
        esperados.add(salida.name)
        if salida.exists():
            continue
        datos = buscar(grupo, artefacto, version, extension)
        if datos is None and extension == "aar":
            datos, extension = buscar(grupo, artefacto, version, "jar"), "jar"
            salida = DESTINO / f"{artefacto}-{version}.jar"
        if datos is None:
            print(f"  aviso: sin binario para {clave}:{version}", file=sys.stderr)
            continue
        salida.write_bytes(datos)
        esperados.add(salida.name)
        print(f"  {salida.name} ({len(datos) // 1024} KB)")

    # Fuera lo que una resolución anterior dejó y esta ya no elige. Dos
    # versiones del mismo artefacto sueltas en la carpeta acaban las dos en el
    # dex, y `d8` se planta con "definido dos veces".
    for viejo in list(DESTINO.glob("*.jar")) + list(DESTINO.glob("*.aar")):
        if viejo.name not in esperados:
            print(f"  fuera {viejo.name}")
            viejo.unlink()
            for resto in [EXTRAIDO / viejo.stem, BUILD / f"{viejo.stem}.zip"]:
                if resto.is_dir():
                    shutil.rmtree(resto)
                elif resto.exists():
                    resto.unlink()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
