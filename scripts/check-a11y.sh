#!/usr/bin/env bash
# Accessibility: that the contract and the Android host say the same thing.
#
# This is the light half. The heavy one is `check-a11y-device.sh`, which builds
# the APK, installs it on a running emulator and asks the system for the
# accessibility tree — the only way to prove a label arrived instead of
# claiming it. That one needs a device and two minutes, so it stays out of
# `check-all.sh`; this one is text and runs in a blink.
#
# What it protects is the failure this whole area is made of: an accessibility
# prop that reaches nobody does not crash, does not log, and looks exactly like
# one that works. It can only be seen with a screen reader on, or with a dump.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== accessibility"

python3 "$ROOT/scripts/check-a11y.py" "$ROOT"
