#!/usr/bin/env bash
# Compila el core en Rust, enlaza el shell Swift y lanza la app en el
# simulador. Sin .xcodeproj: todo el build cabe en un script, que es lo que
# `an-cli` acabará haciendo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="AngularNative"
BUNDLE_ID="dev.angularnative.playground"
TARGET="aarch64-apple-ios-sim"
PROFILE="${PROFILE:-debug}"
DEVICE="${DEVICE:-iPhone 17 Pro}"
DEPLOYMENT="17.0"

SDK_PATH="$(xcrun --sdk iphonesimulator --show-sdk-path)"
BUILD_DIR="$ROOT/build/ios"
APP_DIR="$BUILD_DIR/$APP_NAME.app"

echo "==> core Rust ($PROFILE)"
CARGO_FLAGS=(--target "$TARGET" -p an-ios)
[ "$PROFILE" = "release" ] && CARGO_FLAGS+=(--release)
(cd "$ROOT" && cargo build "${CARGO_FLAGS[@]}")

echo "==> shell Swift"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR"
xcrun swiftc \
  -sdk "$SDK_PATH" \
  -target "arm64-apple-ios${DEPLOYMENT}-simulator" \
  -import-objc-header "$ROOT/shells/ios/Sources/Bridging-Header.h" \
  -I "$ROOT/crates/an-ios/include" \
  -L "$ROOT/target/$TARGET/$PROFILE" \
  -lan_ios \
  -Xclang-linker -isysroot -Xclang-linker "$SDK_PATH" \
  -o "$APP_DIR/$APP_NAME" \
  "$ROOT"/shells/ios/Sources/*.swift
cp "$ROOT/shells/ios/Resources/Info.plist" "$APP_DIR/Info.plist"

echo "==> simulador: $DEVICE"
UDID="$(xcrun simctl list devices available -j \
  | python3 -c "import json,sys;d=json.load(sys.stdin)['devices'];print(next(x['udid'] for v in d.values() for x in v if x['name']=='$DEVICE'))")"
xcrun simctl boot "$UDID" 2>/dev/null || true
open -a Simulator --args -CurrentDeviceUDID "$UDID" 2>/dev/null || true
xcrun simctl install "$UDID" "$APP_DIR"
xcrun simctl launch "$UDID" "$BUNDLE_ID"

echo "==> corriendo. Log: xcrun simctl spawn $UDID log stream --predicate 'process == \"AngularNative\"'"
