#!/usr/bin/env bash
# Todo lo verificable sin dispositivo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== listas duplicadas"
"$ROOT/scripts/check-styles.sh"
"$ROOT/scripts/check-kinds.sh"

echo
"$ROOT/scripts/check-signals.sh"

echo
echo "== props que llegan a los dos hosts"
"$ROOT/scripts/check-wrapper.sh"

echo
echo "== núcleo Rust"
cargo test --quiet 2>&1 | tail -1

"$ROOT/scripts/check-angular.sh"
"$ROOT/scripts/check-list.sh"
"$ROOT/scripts/check-router.sh"
"$ROOT/scripts/check-controls.sh"
"$ROOT/scripts/check-gestures.sh"
"$ROOT/scripts/check-pickers.sh"
"$ROOT/scripts/check-web.sh"
"$ROOT/scripts/check-media.sh"
"$ROOT/scripts/check-hot.sh"
"$ROOT/scripts/check-watchos.sh"
"$ROOT/scripts/check-tvos.sh"
"$ROOT/scripts/check-visionos.sh"

echo
"$ROOT/scripts/check-macos.sh"
"$ROOT/scripts/check-external.sh"

echo
"$ROOT/scripts/check-plugins.sh"

echo
"$ROOT/scripts/check-permissions.sh"

echo
"$ROOT/scripts/check-secrets.sh"

echo
"$ROOT/scripts/check-android-java.sh"

echo
"$ROOT/scripts/check-wearos.sh"

echo
echo "== compilación cruzada"
for target in aarch64-apple-ios-sim aarch64-linux-android; do
  case "$target" in
    *ios*) crate=an-ios ;;
    *) crate=an-android ;;
  esac
  if cargo build --quiet -p "$crate" --target "$target" 2>/dev/null; then
    echo "  ok   $crate para $target"
  else
    echo "  FALLO $crate para $target"
    exit 1
  fi
done
