#!/usr/bin/env bash
# Signing and distribution: the plumbing, the refusals, and the one artefact
# that can be produced without anybody's account.
#
# **This check has to pass on a machine with no Apple Developer account, no
# certificate in its keychain and no release keystore**, because that is most
# machines and certainly every CI runner. So what is checked here is never "the
# signature is valid" — it is that the settings are read from where they are
# supposed to be, that every missing credential produces a sentence naming what
# is missing and what to do about it, and that the artefacts that *can* be built
# come out with the right shape inside.
#
# Three groups, and it is worth knowing which is which:
#
#   1. **Run end to end.** The Android release path, all of it: a keystore
#      generated here, an APK signed with it, a bundle assembled and signed, and
#      both of them opened afterwards to see whose certificate is on them. None
#      of that needs an account — an upload key is a key you make yourself.
#   2. **Run as far as the credential.** Everything Apple. The provisioning
#      profiles are real CMS envelopes, made here with `openssl` and a
#      throwaway certificate, so the whole reading-and-checking half runs for
#      real: expired, wrong app, wrong team, wildcard. What cannot run is the
#      `codesign` that comes after, and what is checked is that it stops there
#      saying which identity it wanted.
#   3. **Not run at all, anywhere.** `devicectl` on a real iPhone, `notarytool`
#      against Apple, and the App Store upload. Those are written from the
#      documentation and they are what the person with the account has to try
#      first. They are named in the docs page as unexercised, and they are not
#      pretended to be tested here.
#
# Every credential this makes lands in `build/`, which the .gitignore covers,
# and every one of them is a throwaway generated on the spot. Nothing in this
# repository is a key.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
AN="$TARGET/debug/an"
WORK="$ROOT/build/check-signing"
APP="$WORK/app"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }
contains() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; echo "$1" | sed 's/^/       /' | head -6; fi
}
exists() {
  if [ -e "$1" ]; then ok "$2"; else ko "$2"; fi
}
# Runs something that has to fail, and returns its output so it can be examined.
must_fail() {
  local output
  if output="$("$@" 2>&1)"; then
    echo "  FAIL expected a failure and it succeeded: $*"
    echo "$output" | sed 's/^/       /' | head -10
    fail=1
    return 0
  fi
  printf '%s' "$output"
}

echo "== signing and distribution"

cargo build -q -p an-cli

# ---------------------------------------------------------------------------
# 0. The settings module's own tests
# ---------------------------------------------------------------------------
#
# Reading a profile, deciding whether it expired, deciding whether it is about
# this app, refusing a manifest that carries a password. They are Rust tests and
# not shell, because they are about a function and not about a command.
if cargo test -q -p an-cli 2>&1 | grep -q '^test result: ok'; then
  ok 'the settings and profile tests pass'
else
  ko 'the settings and profile tests do not pass'
  cargo test -p an-cli 2>&1 | tail -20
fi

# ---------------------------------------------------------------------------
# A project, with nothing in it but a manifest
# ---------------------------------------------------------------------------
#
# There is no `an init` here on purpose. Everything below has to fail *before*
# anything is compiled — that is the whole design of the signing module — so a
# directory with an `angular-native.json` in it is enough to prove it, and it
# costs a second instead of a minute. `check-external.sh` is the one that does
# the real thing.
rm -rf "$WORK"
mkdir -p "$APP/src"

manifest() { # $1 the signing object; {} when nothing is passed
  cat >"$APP/angular-native.json" <<JSON
{
  "app": {
    "name": "CheckApp",
    "bundleId": "dev.angularnative.checkapp",
    "entry": "src/main.native.ts"
  },
  "platforms": ["ios", "android"],
  "signing": ${1:-{\}}
}
JSON
}
manifest
cat >"$APP/package.json" <<'JSON'
{ "name": "check-app", "version": "0.0.0", "private": true,
  "dependencies": { "@angular/core": "^22.1.0" } }
JSON

in_app() { (cd "$APP" && "$@"); }

# ---------------------------------------------------------------------------
# 1. With no settings at all, each command says which section it wants
# ---------------------------------------------------------------------------
unset AN_IOS_PROFILE AN_IOS_TEAM AN_IOS_IDENTITY
output="$(must_fail in_app "$AN" ios --physical)"
contains "$output" 'signing\.ios' 'an ios --physical with no settings names signing.ios'
contains "$output" '"profile": "ios/profiles/development\.mobileprovision"' \
  'and shows the block to paste, not just the name of it'
contains "$output" 'AN_IOS_PROFILE' 'and the environment variable, for CI'
contains "$output" 'angular-native.github.io/guide/signing-and-distribution' \
  'and points at the page that says what to ask Apple for'

output="$(must_fail in_app "$AN" android --sign)"
contains "$output" 'signing\.android' 'an android --sign with no settings names signing.android'
contains "$output" 'storePasswordEnv' 'and shows a password as the name of a variable, never a value'

# `an macos` passes its own default example along, and outside the monorepo an
# app path that is not the project is an error — so the project is named. It is
# the asymmetry `reference/cli.md` warns about, and it applies here too.
output="$(must_fail in_app "$AN" macos . --sign)"
contains "$output" 'signing\.macos' 'an macos --sign with no settings names signing.macos'

# ---------------------------------------------------------------------------
# 2. A password written into the manifest stops everything
# ---------------------------------------------------------------------------
#
# Not only the signing commands: the manifest is read by every command that runs
# inside a project, so `an build` refuses too. A secret that is only noticed by
# the command that needs it is a secret that sits in a repository for months.
manifest '{ "android": { "keystore": "release.keystore", "storePassword": "hunter2" } }'
output="$(must_fail in_app "$AN" build)"
contains "$output" 'signing\.android\.storePassword' \
  'a password in the manifest is refused by name, by any command'
contains "$output" 'storePasswordEnv' 'and it says what to write instead'
contains "$output" 'has already been pushed' 'and that a pushed password has to be changed'
manifest

# ---------------------------------------------------------------------------
# 3. A credential git can see
# ---------------------------------------------------------------------------
#
# A keystore in a repository is the app's whole identity on Play: once it is
# pushed the only fix is to make another one and ask Google to change the upload
# key. So it is refused at the moment the path is first read, which is the last
# moment where it is still cheap.
git -C "$APP" init -q
git -C "$APP" config user.email check@example.com
git -C "$APP" config user.name check
keytool -genkeypair -keystore "$APP/release.keystore" \
  -storepass throwaway -keypass throwaway -alias upload \
  -keyalg RSA -keysize 2048 -validity 2 \
  -dname "CN=throwaway, O=angular-native check, C=ES" >/dev/null 2>&1
manifest '{ "android": { "keystore": "release.keystore", "keyAlias": "upload" } }'

export AN_ANDROID_KEYSTORE_PASSWORD=throwaway
output="$(must_fail in_app "$AN" android --sign)"
contains "$output" 'git is not ignoring it' 'a keystore git can see is refused before anything compiles'
contains "$output" "echo 'release.keystore' >> .gitignore" 'and it says the line to add'

git -C "$APP" add -f release.keystore >/dev/null 2>&1
output="$(must_fail in_app "$AN" android --sign)"
contains "$output" 'is committed to git' 'and a committed one is a harder no'
contains "$output" 'has to be replaced' 'that says the credential itself is now spent'
git -C "$APP" rm --cached -q release.keystore
echo 'release.keystore' >"$APP/.gitignore"

# ---------------------------------------------------------------------------
# 4. The keystore itself, opened before anything is built
# ---------------------------------------------------------------------------
export AN_ANDROID_KEYSTORE_PASSWORD=wrong
output="$(must_fail in_app "$AN" android --sign)"
export AN_ANDROID_KEYSTORE_PASSWORD=throwaway
contains "$output" 'password for .* is wrong' 'a wrong keystore password is caught before the build'
contains "$output" 'AN_ANDROID_KEYSTORE_PASSWORD' 'and it names the variable it came from'

manifest '{ "android": { "keystore": "release.keystore", "keyAlias": "not-there" } }'
output="$(must_fail in_app "$AN" android --sign)"
contains "$output" 'no key called "not-there"' 'an alias that is not in the keystore is caught too'
contains "$output" 'keytool -list' 'and it says how to list the ones that are'

manifest '{ "android": { "keystore": "release.keystore", "keyAlias": "upload" } }'
unset AN_ANDROID_KEYSTORE_PASSWORD
output="$(must_fail in_app "$AN" android --sign)"
contains "$output" 'AN_ANDROID_KEYSTORE_PASSWORD is not set' 'and a password nobody exported is named'
contains "$output" 'that file is committed' 'saying why it is not in the manifest'

# ---------------------------------------------------------------------------
# 5. Provisioning profiles, read for real
# ---------------------------------------------------------------------------
#
# A `.mobileprovision` is a plist inside a CMS envelope, and Apple is not needed
# to make one: `openssl` signs it with a certificate generated here. `security
# cms -D` unwraps it exactly as it unwraps Apple's, so everything the CLI does
# with a profile — expiry, app id, team — runs against a real file rather than
# against a fixture the code was written to match.
PROFILES="$WORK/profiles"
mkdir -p "$PROFILES"
openssl req -x509 -newkey rsa:2048 -keyout "$PROFILES/key.pem" -out "$PROFILES/cert.pem" \
  -nodes -days 2 -subj "/CN=angular-native check" >/dev/null 2>&1

profile() { # $1 name  $2 application-identifier  $3 expiry  $4 team
  cat >"$PROFILES/$1.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>Name</key><string>$1</string>
	<key>TeamIdentifier</key><array><string>$4</string></array>
	<key>Entitlements</key><dict>
		<key>application-identifier</key><string>$2</string>
		<key>com.apple.developer.team-identifier</key><string>$4</string>
		<key>get-task-allow</key><true/>
		<key>keychain-access-groups</key><array><string>$2</string></array>
	</dict>
	<key>ExpirationDate</key><date>$3</date>
</dict>
</plist>
PLIST
  openssl smime -sign -nodetach -binary -in "$PROFILES/$1.plist" \
    -out "$PROFILES/$1.mobileprovision" -signer "$PROFILES/cert.pem" \
    -inkey "$PROFILES/key.pem" -outform DER >/dev/null 2>&1
}

profile good     ABCDE12345.dev.angularnative.checkapp 2999-01-01T00:00:00Z ABCDE12345
profile expired  ABCDE12345.dev.angularnative.checkapp 2020-03-04T10:00:00Z ABCDE12345
profile other    ABCDE12345.com.example.other          2999-01-01T00:00:00Z ABCDE12345
profile wildcard 'ABCDE12345.*'                        2999-01-01T00:00:00Z ABCDE12345

manifest
export AN_IOS_IDENTITY='Not A Real Certificate'

export AN_IOS_PROFILE="$PROFILES/missing.mobileprovision"
output="$(must_fail in_app "$AN" ios --physical)"
contains "$output" 'missing\.mobileprovision is missing' 'a profile that is not there is named'
contains "$output" 'developer\.apple\.com/account/resources/profiles' 'with the page it is downloaded from'

export AN_IOS_PROFILE="$PROFILES/good.plist"
output="$(must_fail in_app "$AN" ios --physical)"
contains "$output" 'security cms -D. refused it' 'a file that is not a profile says so'

export AN_IOS_PROFILE="$PROFILES/expired.mobileprovision"
output="$(must_fail in_app "$AN" ios --physical)"
contains "$output" 'expired on 2020-03-04' 'an expired profile is refused, saying when'
contains "$output" 'Renew it' 'and how to renew it'

export AN_IOS_PROFILE="$PROFILES/other.mobileprovision"
output="$(must_fail in_app "$AN" ios --physical)"
contains "$output" 'is for com\.example\.other, and this app is dev\.angularnative\.checkapp' \
  'a profile for another app names both identifiers'
contains "$output" 'installs and is killed on' 'and says what would happen if it were used anyway'

export AN_IOS_TEAM=ZZZZZZZZZZ
export AN_IOS_PROFILE="$PROFILES/good.mobileprovision"
output="$(must_fail in_app "$AN" ios --physical)"
unset AN_IOS_TEAM
contains "$output" 'belongs to ABCDE12345' 'a team that disagrees with the profile is refused'

# The wildcard covers the app, so this one gets *past* the profile and stops at
# the certificate. That order is the check: the profile is read first because
# the team comes out of it.
export AN_IOS_PROFILE="$PROFILES/wildcard.mobileprovision"
output="$(must_fail in_app "$AN" ios --physical)"
contains "$output" 'Not A Real Certificate' \
  'a wildcard profile is accepted, and the build stops at the certificate instead'
contains "$output" 'find-identity|Manage Certificates' 'saying where a certificate comes from'

# And the same profile through `--archive`, which is the other half of the iOS
# story and asks for a distribution certificate rather than a development one.
output="$(must_fail in_app "$AN" ios --archive)"
contains "$output" 'certificates/list' 'an archive asks for a distribution certificate'

unset AN_IOS_IDENTITY AN_IOS_PROFILE

# ---------------------------------------------------------------------------
# 6. macOS: the two errands are reported together
# ---------------------------------------------------------------------------
export AN_MACOS_IDENTITY='Not A Real Certificate'
output="$(must_fail in_app "$AN" macos . --notarize)"
unset AN_MACOS_IDENTITY
contains "$output" 'Not A Real Certificate' 'an macos --notarize names the certificate it wanted'
contains "$output" 'paid Apple Developer Program' 'and says a free Apple ID cannot issue one'
contains "$output" 'notarytool store-credentials' \
  'and reports the missing notary profile in the same breath, not one errand at a time'
contains "$output" 'leave .--sign. off' 'and says what to do to build without any of it'

# ---------------------------------------------------------------------------
# 7. The Android release path, end to end
# ---------------------------------------------------------------------------
#
# This is the one that is not a rehearsal: a keystore made here, a real APK
# signed with it, a real bundle assembled and signed, and both of them opened
# afterwards to see whose certificate came out. It is also the slow part — two
# Android builds — so it goes last.
KEYSTORE="$WORK/throwaway.keystore"
keytool -genkeypair -keystore "$KEYSTORE" \
  -storepass throwaway -keypass throwaway -alias upload \
  -keyalg RSA -keysize 2048 -validity 2 \
  -dname "CN=angular-native check, O=throwaway, C=ES" >/dev/null 2>&1
exists "$KEYSTORE" 'a throwaway keystore is generated for this run'
if git check-ignore -q "$KEYSTORE"; then
  ok 'and it lands where the .gitignore covers it'
else
  ko 'and it lands where the .gitignore covers it'
fi

export AN_ANDROID_KEYSTORE="$KEYSTORE"
export AN_ANDROID_KEY_ALIAS=upload
export AN_ANDROID_KEYSTORE_PASSWORD=throwaway
export AN_ANDROID_KEY_PASSWORD=throwaway

BUILD_TOOLS="$(ls -d "${ANDROID_HOME:-$HOME/Library/Android/sdk}"/build-tools/* 2>/dev/null | tail -1)"
if [ ! -d "$BUILD_TOOLS" ] || [ ! -s "$ROOT/vendor/android/build/classpath.txt" ]; then
  echo "  --   the Android release build is skipped: no SDK or no prepared dependencies"
  echo "       python3 scripts/fetch-android-deps.py && python3 scripts/prepare-android-deps.py"
else
  LOG="$WORK/android.log"
  if "$AN" android --sign --no-launch >"$LOG" 2>&1; then
    APK="$(tail -1 "$LOG")"
    ok 'an android --sign builds an APK'
    # The name is not the debug one. A release build quietly overwriting the
    # APK the emulator has been running is how the wrong artefact reaches a
    # store.
    contains "$APK" '\-release\.apk$' 'and it is named apart from the debug one'
    CERTS="$("$BUILD_TOOLS/apksigner" verify --print-certs "$APK" 2>&1)"
    contains "$CERTS" 'CN=angular-native check' \
      'and it carries the certificate of the keystore it was given, not the debug one'
    # apksigner prints one `V<n> Signer:` block per scheme it found. v1 alone —
    # a jar signature — is what a modern Android refuses to install.
    contains "$CERTS" '^V[23]\.[0-9]+ Signer' \
      'signed with a modern APK signature scheme and not only the jar one'
  else
    ko 'an android --sign builds an APK'
    tail -20 "$LOG"
  fi

  # And the bundle. bundletool is not part of the Android SDK, so this half is
  # skipped rather than failed when it is absent: see `an android --aab`, which
  # says the same thing in the same words.
  if [ ! -f "$ROOT/vendor/android/tools/bundletool.jar" ] && [ -z "${AN_BUNDLETOOL:-}" ]; then
    output="$(must_fail "$AN" android --aab)"
    contains "$output" 'fetch-android-deps\.py' \
      'with no bundletool, --aab says where to get it'
    echo "  --   the .aab is not built: bundletool is not here"
  else
    if "$AN" android --aab >"$LOG" 2>&1; then
      AAB="$(tail -1 "$LOG")"
      ok 'an android --aab builds a bundle'
      INSIDE="$(unzip -Z1 "$AAB" 2>/dev/null)"
      # The four things that make it a bundle rather than a zip of an APK. Play
      # rejects it for any one of them, and it rejects it on upload, which is
      # the slowest possible place to find out.
      contains "$INSIDE" '^BundleConfig\.pb$' 'the bundle carries its BundleConfig'
      contains "$INSIDE" '^base/manifest/AndroidManifest\.xml$' \
        'the manifest is in the module, where a bundle keeps it'
      contains "$INSIDE" '^base/dex/classes\.dex$' 'the dex is under base/dex'
      contains "$INSIDE" '^base/lib/arm64-v8a/liban_android\.so$' \
        'and the Rust core is under base/lib, with its ABI'
      contains "$INSIDE" '^META-INF/[A-Z]+\.(RSA|DSA|EC)$' \
        'and it is signed as the jar it is, by jarsigner and not apksigner'
      contains "$(unzip -p "$AAB" META-INF/*.RSA 2>/dev/null | openssl pkcs7 -inform DER -print_certs 2>/dev/null)" \
        'CN ?= ?angular-native check' \
        'with the certificate of the keystore it was given'
    else
      ko 'an android --aab builds a bundle'
      tail -20 "$LOG"
    fi
  fi
fi

# ---------------------------------------------------------------------------
# 8. The .dmg, which needs nobody's account
# ---------------------------------------------------------------------------
#
# The one Apple artefact that can be produced in full here. Signing it needs a
# Developer ID certificate and notarising it needs Apple; *packing* it needs
# neither, so the layout of the image and the warning that comes with an ad-hoc
# one are both checked for real.
#
# It runs in the monorepo, on the desktop example, because that is the app the
# macOS host is checked with everywhere else and `check-macos.sh` has already
# left its cargo build warm.
LOG="$WORK/macos.log"
if "$AN" macos --dmg >"$LOG" 2>&1; then
  DMG="$(tail -1 "$LOG")"
  ok 'an macos --dmg packs the .app into a disk image'
  contains "$(cat "$LOG")" 'another Mac will refuse to open it' \
    'and says out loud that an ad-hoc image is useless as a download'
  # Really a disk image, and compressed: `hdiutil` is the only thing that can
  # say so, and a .dmg that turns out to be a folder with the wrong name is
  # exactly the kind of thing nobody notices until somebody downloads it.
  contains "$(hdiutil imageinfo "$DMG" 2>&1)" 'Format: *UDZO' \
    'and the image is a real, compressed one'
  contains "$(codesign -dv "$ROOT/build/macos/AngularNativeMac.app" 2>&1)" 'Signature=adhoc' \
    'and the app inside it is ad-hoc signed, which is what --sign was not asked for'
else
  ko 'an macos --dmg packs the .app into a disk image'
  tail -20 "$LOG"
fi

# ---------------------------------------------------------------------------
# 9. Nothing in this repository is a credential
# ---------------------------------------------------------------------------
#
# The one check here that is about the repository rather than about the CLI. It
# costs nothing and it is the only one that would catch the mistake that cannot
# be undone.
COMMITTED="$(git -C "$ROOT" ls-files | grep -E '\.(keystore|jks|p12|pfx|mobileprovision|cer|p8)$' || true)"
if [ -z "$COMMITTED" ]; then
  ok 'no keystore, certificate or profile is committed'
else
  ko 'no keystore, certificate or profile is committed'
  echo "$COMMITTED" | sed 's/^/       /'
fi

exit "$fail"
