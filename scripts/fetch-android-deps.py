#!/usr/bin/env python3
"""Downloads Material Components and everything it drags along.

Android ships neither the bottom navigation bar nor the Material 3 buttons in
the platform: they live in `com.google.android.material`, which is a separate
library. Gradle would resolve it on its own; there is no Gradle here, so it is
resolved by hand by reading the POMs and everything is left in
`vendor/android/`.

It is not a dependency manager: it resolves version conflicts with no
sophisticated strategy —it keeps the first one it sees, which is the one nearest
the root, the same as Gradle does— and it does not touch the test `scope`s.
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

ROOT = pathlib.Path(__file__).resolve().parent.parent
TARGET = ROOT / "vendor" / "android"
EXTRACTED = TARGET / "extracted"
BUILD = TARGET / "build"
NS = {"m": "http://maven.apache.org/POM/4.0.0"}

# What is asked for. The rest comes out of this.
SEED = [("com.google.android.material", "material", "1.13.0")]

# What is not needed: annotations that only exist at compile time and Kotlin
# things we do not use.
IGNORE = {
    "com.google.errorprone:error_prone_annotations",
    "org.jetbrains.kotlin:kotlin-bom",
    "com.google.guava:listenablefuture",
    # Since Kotlin 1.8 `kotlin-stdlib` carries inside it what used to be in these
    # three. They are still published so as not to break whoever asks for them,
    # but bringing both sets leaves the same classes twice over and `d8` refuses.
    "org.jetbrains.kotlin:kotlin-stdlib-jdk7",
    "org.jetbrains.kotlin:kotlin-stdlib-jdk8",
    "org.jetbrains.kotlin:kotlin-stdlib-common",
}


def download(url: str) -> bytes | None:
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            return response.read()
    except Exception:
        return None


def fetch(group: str, artefact: str, version: str, extension: str) -> bytes | None:
    path = f"{group.replace('.', '/')}/{artefact}/{version}/{artefact}-{version}.{extension}"
    for repo in REPOS:
        data = download(f"{repo}/{path}")
        if data:
            return data
    return None


def text(node, tag: str) -> str | None:
    child = node.find(f"m:{tag}", NS)
    return child.text.strip() if child is not None and child.text else None


def management_of(root, depth: int = 0) -> dict[str, str]:
    """The versions `dependencyManagement` pins, BOMs included.

    A management section may list no versions at all and instead *import*
    another POM that lists them —that is a BOM—, so they have to be followed.
    Without this, half of androidx is left with no version and is not downloaded.
    """
    managed: dict[str, str] = {}
    if depth > 4:
        return managed
    for dep in root.iterfind("m:dependencyManagement/m:dependencies/m:dependency", NS):
        g, a, v = text(dep, "groupId"), text(dep, "artifactId"), text(dep, "version")
        if not (g and a and v):
            continue
        if (text(dep, "scope") or "") == "import":
            bom = fetch(g, a, v, "pom")
            if bom:
                managed.update(management_of(ET.fromstring(bom), depth + 1))
            continue
        managed[f"{g}:{a}"] = v
    return managed


def concrete(version: str | None) -> str | None:
    """A concrete version out of whatever the POM declares.

    Maven allows ranges: `[1.7.0]` is "exactly that one" and `[1.7.0,2.0)` is
    "that one and later". Both are resolved to the lower bound, which is what
    whoever writes them wants. Everything with brackets in it used to be
    discarded, and with it half of androidx: appcompat asks for its resources as
    `[1.7.0]` and they were left out without anything failing until the app
    started and could not find a class.
    """
    if not version:
        return None
    version = version.strip()
    if "$" in version:
        return None
    if not version.startswith(("[", "(")):
        return version
    inside = version.strip("[]()")
    bound = inside.split(",")[0].strip()
    return bound or None


def order(version: str) -> tuple:
    """Compares versions by their numbers; anything that is not a number sorts last."""
    parts = re.findall(r"\d+|[a-zA-Z]+", version)
    return tuple((0, int(p)) if p.isdigit() else (1, p) for p in parts)


def one_pass(global_management: dict[str, str]) -> dict[str, tuple[str, str, str, str]]:
    """Walks the POMs once. Faced with two versions of the same, the higher wins."""
    chosen: dict[str, tuple[str, str, str, str]] = {}
    queue = list(SEED)
    while queue:
        group, artefact, version = queue.pop(0)
        key = f"{group}:{artefact}"
        if key in IGNORE:
            continue
        previous = chosen.get(key)
        if previous and order(previous[2]) >= order(version):
            continue
        pom = fetch(group, artefact, version, "pom")
        if pom is None:
            print(f"  warning: no POM for {key}:{version}", file=sys.stderr)
            continue
        root = ET.fromstring(pom)
        packaging = text(root, "packaging") or "jar"
        chosen[key] = (group, artefact, version, packaging)

        # The management accumulates across POMs: a dependency may arrive with no
        # version here and have it pinned in the management section of another
        # module of the same family, which is how androidx hands out its own.
        global_management.update(management_of(root))

        for dep in root.iterfind("m:dependencies/m:dependency", NS):
            scope = text(dep, "scope") or "compile"
            optional = (text(dep, "optional") or "false") == "true"
            if scope not in ("compile", "runtime") or optional:
                continue
            g = text(dep, "groupId")
            a = text(dep, "artifactId")
            if not (g and a):
                continue
            v = concrete(text(dep, "version")) or global_management.get(f"{g}:{a}")
            if not v:
                print(f"  warning: {g}:{a} has no usable version, skipping", file=sys.stderr)
                continue
            queue.append((g, a, v))
    return chosen


def resolve() -> dict[str, tuple[str, str, str, str]]:
    """Resolves the tree.

    It is walked twice: on the first pass the versions each management section
    pins are learned, and on the second the dependencies that arrived with no
    version before those were known can be resolved. Without the second pass half
    of androidx is left out, and that does not show until a class is missing with
    the app already running.
    """
    management: dict[str, str] = {}
    one_pass(management)
    return one_pass(management)


def main() -> int:
    TARGET.mkdir(parents=True, exist_ok=True)
    artefacts = resolve()
    print(f"==> {len(artefacts)} artefacts")
    expected: set[str] = set()
    for key, (group, artefact, version, packaging) in sorted(artefacts.items()):
        extension = "aar" if packaging == "aar" else "jar"
        output = TARGET / f"{artefact}-{version}.{extension}"
        expected.add(output.name)
        if output.exists():
            continue
        data = fetch(group, artefact, version, extension)
        if data is None and extension == "aar":
            data, extension = fetch(group, artefact, version, "jar"), "jar"
            output = TARGET / f"{artefact}-{version}.jar"
        if data is None:
            print(f"  warning: no binary for {key}:{version}", file=sys.stderr)
            continue
        output.write_bytes(data)
        expected.add(output.name)
        print(f"  {output.name} ({len(data) // 1024} KB)")

    # Out with whatever an earlier resolution left behind and this one no longer
    # picks. Two versions of the same artefact loose in the folder both end up in
    # the dex, and `d8` refuses with "defined twice".
    for old in list(TARGET.glob("*.jar")) + list(TARGET.glob("*.aar")):
        if old.name not in expected:
            print(f"  dropping {old.name}")
            old.unlink()
            for leftover in [EXTRACTED / old.stem, BUILD / f"{old.stem}.zip"]:
                if leftover.is_dir():
                    shutil.rmtree(leftover)
                elif leftover.exists():
                    leftover.unlink()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
