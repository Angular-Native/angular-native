#!/usr/bin/env python3
"""Deja las dependencias de Android listas para enlazar.

Un `.aar` es un zip con las clases, los recursos y su manifiesto. Gradle sabe
desmontarlos; aquí se hace a mano: se extrae cada uno, se compilan sus
recursos con `aapt2 compile` y se apunta el nombre de su paquete, que hace
falta para generar su clase `R`.

El resultado queda en `vendor/android/build/`, y solo se rehace lo que falte:
compilar los recursos de cincuenta librerías tarda, y no cambian nunca.
"""

import os
import pathlib
import re
import shutil
import subprocess
import sys
import zipfile

RAIZ = pathlib.Path(__file__).resolve().parent.parent
VENDOR = RAIZ / "vendor" / "android"
EXTRAIDO = VENDOR / "extracted"
BUILD = VENDOR / "build"


def sdk() -> pathlib.Path:
    for candidata in [os.environ.get("ANDROID_HOME"), os.environ.get("ANDROID_SDK_ROOT"),
                      str(pathlib.Path.home() / "Library/Android/sdk")]:
        if candidata and pathlib.Path(candidata).is_dir():
            return pathlib.Path(candidata)
    raise SystemExit("no encuentro el SDK de Android")


def aapt2() -> pathlib.Path:
    herramientas = sorted((sdk() / "build-tools").iterdir(), reverse=True)
    for version in herramientas:
        binario = version / "aapt2"
        if binario.exists():
            return binario
    raise SystemExit("no encuentro aapt2")


def paquete_de(manifiesto: pathlib.Path) -> str | None:
    if not manifiesto.exists():
        return None
    encontrado = re.search(r'package="([^"]+)"', manifiesto.read_text(errors="ignore"))
    return encontrado.group(1) if encontrado else None


def main() -> int:
    BUILD.mkdir(parents=True, exist_ok=True)
    EXTRAIDO.mkdir(parents=True, exist_ok=True)
    compilador = aapt2()

    jars: list[str] = []
    flats: list[str] = []
    paquetes: list[str] = []

    for artefacto in sorted(VENDOR.glob("*.jar")) + sorted(VENDOR.glob("*.aar")):
        nombre = artefacto.stem
        if artefacto.suffix == ".jar":
            jars.append(str(artefacto))
            continue

        destino = EXTRAIDO / nombre
        if not destino.exists():
            with zipfile.ZipFile(artefacto) as zf:
                zf.extractall(destino)

        clases = destino / "classes.jar"
        if clases.exists():
            jars.append(str(clases))
        for extra in sorted((destino / "libs").glob("*.jar")) if (destino / "libs").is_dir() else []:
            jars.append(str(extra))

        paquete = paquete_de(destino / "AndroidManifest.xml")
        if paquete:
            paquetes.append(paquete)

        res = destino / "res"
        if res.is_dir() and any(res.iterdir()):
            flat = BUILD / f"{nombre}.zip"
            if not flat.exists():
                print(f"  recursos de {nombre}")
                subprocess.run(
                    [str(compilador), "compile", "--dir", str(res), "-o", str(flat)],
                    check=True,
                )
            flats.append(str(flat))

    (BUILD / "classpath.txt").write_text("\n".join(jars))
    (BUILD / "resources.txt").write_text("\n".join(flats))
    (BUILD / "packages.txt").write_text("\n".join(sorted(set(paquetes))))
    print(f"==> {len(jars)} jars, {len(flats)} paquetes de recursos, {len(set(paquetes))} paquetes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
