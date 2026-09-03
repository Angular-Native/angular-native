#!/usr/bin/env python3
"""Gets the Android dependencies ready to link.

An `.aar` is a zip with the classes, the resources and its manifest. Gradle
knows how to take them apart; here it is done by hand: each one is extracted,
its resources compiled with `aapt2 compile` and its package name noted down,
which is needed to generate its `R` class.

The result ends up in `vendor/android/build/`, and only what is missing is
redone: compiling the resources of fifty libraries takes a while, and they never
change.
"""

import os
import pathlib
import re
import shutil
import subprocess
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "android"
EXTRACTED = VENDOR / "extracted"
BUILD = VENDOR / "build"


def sdk() -> pathlib.Path:
    for candidate in [os.environ.get("ANDROID_HOME"), os.environ.get("ANDROID_SDK_ROOT"),
                      str(pathlib.Path.home() / "Library/Android/sdk")]:
        if candidate and pathlib.Path(candidate).is_dir():
            return pathlib.Path(candidate)
    raise SystemExit("cannot find the Android SDK")


def aapt2() -> pathlib.Path:
    tools = sorted((sdk() / "build-tools").iterdir(), reverse=True)
    for version in tools:
        binary = version / "aapt2"
        if binary.exists():
            return binary
    raise SystemExit("cannot find aapt2")


def package_of(manifest: pathlib.Path) -> str | None:
    if not manifest.exists():
        return None
    found = re.search(r'package="([^"]+)"', manifest.read_text(errors="ignore"))
    return found.group(1) if found else None


def main() -> int:
    BUILD.mkdir(parents=True, exist_ok=True)
    EXTRACTED.mkdir(parents=True, exist_ok=True)
    compiler = aapt2()

    jars: list[str] = []
    flats: list[str] = []
    packages: list[str] = []

    for artefact in sorted(VENDOR.glob("*.jar")) + sorted(VENDOR.glob("*.aar")):
        name = artefact.stem
        if artefact.suffix == ".jar":
            jars.append(str(artefact))
            continue

        target = EXTRACTED / name
        if not target.exists():
            with zipfile.ZipFile(artefact) as zf:
                zf.extractall(target)

        classes = target / "classes.jar"
        if classes.exists():
            jars.append(str(classes))
        for extra in sorted((target / "libs").glob("*.jar")) if (target / "libs").is_dir() else []:
            jars.append(str(extra))

        package = package_of(target / "AndroidManifest.xml")
        if package:
            packages.append(package)

        res = target / "res"
        if res.is_dir() and any(res.iterdir()):
            flat = BUILD / f"{name}.zip"
            if not flat.exists():
                print(f"  resources of {name}")
                subprocess.run(
                    [str(compiler), "compile", "--dir", str(res), "-o", str(flat)],
                    check=True,
                )
            flats.append(str(flat))

    (BUILD / "classpath.txt").write_text("\n".join(jars))
    (BUILD / "resources.txt").write_text("\n".join(flats))
    (BUILD / "packages.txt").write_text("\n".join(sorted(set(packages))))
    print(f"==> {len(jars)} jars, {len(flats)} resource packages, {len(set(packages))} packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
