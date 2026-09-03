---
title: Signing and distribution
description: Putting a build on a real device and into a store — where the settings live, what you have to get from Apple and Google yourself, and which of these paths have never been run.
sidebar:
  order: 5
---

Everything else in this documentation runs on a simulator or an emulator, signed
with a key that means nothing. This page is about the other half: a build that
installs on a phone somebody hands you, and a file a store will take.

That half cannot be done for you. A certificate is issued to a person, a
provisioning profile lists devices by their serial numbers, and an upload key is
a secret you generate and then must never lose. What `an` can do is know where
those things are, check them before it spends two minutes compiling, and say
which one is missing in a sentence you can act on. What it cannot do is have
them.

## What exists

| Command | What comes out | Needs |
|---|---|---|
| `an ios --physical` | The `.app`, signed, installed on a connected iPhone or iPad | A development certificate and a profile listing that device |
| `an ios --archive` | `<Name>.xcarchive` and `<Name>.ipa` | A distribution certificate and an App Store or ad-hoc profile |
| `an android --sign` | `<Name>-release.apk` | A keystore you generate |
| `an android --aab` | `<Name>-release.aab` for Google Play | The same keystore, plus `bundletool` |
| `an wearos --sign`, `an wearos --aab` | The same, for a Wear OS device | The same keystore |
| `an macos --sign` | The `.app`, Developer ID signed, hardened runtime | A Developer ID certificate |
| `an macos --notarize` | The same, notarised and stapled | That, plus notarytool credentials |
| `an macos --dmg` | `<Name>.dmg` | Nothing, but see below |

And what does not exist: **tvOS, visionOS, watchOS and Wear OS have no device or
store path of their own.** They build for their simulators and stop there. There
is no upload command anywhere either — no `an` talks to App Store Connect or to
the Play Console, and nothing here submits an app for review.

## Where the settings live

In `angular-native.json`, next to `app` and `platforms`, in a `signing` object
keyed by platform:

```json
{
  "app": {
    "name": "MyApp",
    "bundleId": "com.example.myapp",
    "entry": "src/main.native.ts"
  },
  "platforms": ["ios", "android"],
  "signing": {
    "ios": {
      "team": "ABCDE12345",
      "identity": "Apple Development",
      "profile": "ios/profiles/development.mobileprovision"
    },
    "macos": {
      "identity": "Developer ID Application",
      "notaryProfile": "an-notary"
    },
    "android": {
      "keystore": "../secrets/release.keystore",
      "keyAlias": "upload",
      "storePasswordEnv": "AN_ANDROID_KEYSTORE_PASSWORD",
      "keyPasswordEnv": "AN_ANDROID_KEY_PASSWORD"
    }
  }
}
```

| Key | What it is |
|---|---|
| `ios.team` | The ten-character Team ID. Optional: with none, the profile's own is used, and it is the authority anyway. |
| `ios.identity` | A prefix of the certificate's name, as `security find-identity -v -p codesigning` prints it. Defaults to `Apple Development` for `--physical` and `Apple Distribution` for `--archive`. |
| `ios.profile` | Path to the `.mobileprovision`, relative to the project. |
| `macos.identity` | Same, and it is nearly always `Developer ID Application`. |
| `macos.notaryProfile` | The **name** of a `notarytool` keychain profile. Not a credential. |
| `android.keystore` | Path to the keystore, relative to the project. |
| `android.keyAlias` | Which key inside it. |
| `android.storePasswordEnv` | The **name of an environment variable**. Defaults to `AN_ANDROID_KEYSTORE_PASSWORD`. |
| `android.keyPasswordEnv` | Same, defaulting to `AN_ANDROID_KEY_PASSWORD`, and to the store password when that variable is unset. |

There is no `macos.team`, on purpose. Nothing on that platform needs it: the
certificate carries the team and `notarytool` gets it from the keychain profile.
A key in the manifest that changes nothing is a key somebody spends an afternoon
getting right.

### The environment wins

Every setting has an environment variable that overrides the file:

| Variable | Overrides |
|---|---|
| `AN_IOS_TEAM` | `signing.ios.team` |
| `AN_IOS_IDENTITY` | `signing.ios.identity` |
| `AN_IOS_PROFILE` | `signing.ios.profile` |
| `AN_MACOS_IDENTITY` | `signing.macos.identity` |
| `AN_MACOS_NOTARY_PROFILE` | `signing.macos.notaryProfile` |
| `AN_ANDROID_KEYSTORE` | `signing.android.keystore` |
| `AN_ANDROID_KEY_ALIAS` | `signing.android.keyAlias` |
| `AN_BUNDLETOOL` | Where `bundletool.jar` is |

That is what CI uses, where the keystore is decoded into a temporary directory
and the path is different on every run. It is also the only source there is when
`an` runs inside this repository, which has no project manifest at all.

## Never commit a credential

**No password goes in `angular-native.json`.** The file is committed; a password
in it is a password in your history. So the manifest holds the *name* of an
environment variable, and a literal one is refused by name:

```text
angular-native.json: signing.android.storePassword is a secret, and this file is committed.
Name an environment variable instead of holding the value:
    "storePasswordEnv": "AN_SOMETHING_PASSWORD"
and export the value where the build runs. If this password has already been
pushed, it has to be changed.
```

That check runs when the manifest is read, which every command in a project
does — not only the signing ones. A secret noticed only by the command that
needs it is a secret that sits in a repository for months.

The files are checked too. If the keystore or the profile is inside the project
and git is not ignoring it, the build stops before compiling anything:

```text
release.keystore is the release keystore, and git is not ignoring it: the next
`git add .` commits it.
Add it to .gitignore first:
    echo 'release.keystore' >> .gitignore
```

and if it is already committed, the message says the credential itself is spent —
because it is. Anybody who has ever cloned the repository has it.

`an init` puts `*.keystore`, `*.jks`, `*.p12` and `*.mobileprovision` into the
project's `.gitignore` for exactly this reason. The best place for a keystore is
still outside the project altogether.

## What you have to get: Apple

None of this can be automated, and all of it is done once.

<!-- Written as prose and not as a script because every one of these steps is a
     web page that changes its layout twice a year. What does not change is what
     you end up holding. -->

1. **An Apple ID, and for most of it a paid membership.** A free Apple ID can
   sign a build onto your own device — Xcode issues a seven-day certificate for
   it. Everything else needs the Apple Developer Program, which is $99 a year:
   TestFlight, the App Store, and a Developer ID certificate for distributing a
   Mac app outside the store. There is no way around that and no free tier of it.

2. **A certificate, in this Mac's keychain.** For `--physical`, an *Apple
   Development* certificate — Xcode ▸ Settings ▸ Accounts ▸ Manage Certificates
   ▸ **+** ▸ Apple Development creates one and installs it in a single step. For
   `--archive`, an *Apple Distribution* one, from
   [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates/list).
   For `an macos --sign`, a *Developer ID Application* one, from the same page.

   A `.cer` downloaded from that page is only half of it: it carries the public
   certificate, and it is useless without the private key that was generated when
   you made the request. If you are moving to a new Mac, export a `.p12` from the
   old one's keychain — that is the file that carries both.

3. **An App ID**, at
   [developer.apple.com/account/resources/identifiers](https://developer.apple.com/account/resources/identifiers/list),
   matching your `app.bundleId` exactly. A wildcard one (`com.example.*`) works
   for development and is not accepted for anything using push notifications or
   iCloud.

4. **The device's UDID**, at
   [developer.apple.com/account/resources/devices](https://developer.apple.com/account/resources/devices/list).
   `xcrun devicectl list devices` prints it with the phone plugged in.

5. **A provisioning profile**, at
   [developer.apple.com/account/resources/profiles](https://developer.apple.com/account/resources/profiles/list),
   tying the three together: an iOS App Development profile for `--physical`,
   listing that device; an App Store profile for `--archive`. Download it and
   put it where `signing.ios.profile` points.

6. **For `an macos --notarize`, notarytool credentials.** An app-specific
   password from [account.apple.com](https://account.apple.com) ▸ Sign-In and
   Security ▸ App-Specific Passwords, stored once in the keychain:

   ```bash
   xcrun notarytool store-credentials an-notary \
       --apple-id you@example.com \
       --team-id ABCDE12345 \
       --password xxxx-xxxx-xxxx-xxxx
   ```

   `an-notary` is then what `signing.macos.notaryProfile` names. The password
   stays in the keychain and never reaches the project.

7. **On the device: Developer Mode on.** Settings ▸ Privacy & Security ▸
   Developer Mode. The device restarts. Without it `devicectl` refuses the
   install, and the refusal does not say why.

## What you have to get: Google

Less, and none of it costs anything until you publish.

1. **A keystore, which you generate.** Nobody issues it. It is a file, it holds
   a key, and Google Play ties your app to it forever — an app already published
   cannot be updated with a different key without asking Google to reset it.

   ```bash
   keytool -genkeypair -v -keystore ~/secrets/myapp-release.keystore \
       -alias upload -keyalg RSA -keysize 2048 -validity 10000
   ```

   Keep it somewhere the project cannot reach. Back it up. Losing it is the one
   mistake on this page with no clean recovery.

2. **`bundletool`, for `--aab`.** It is not part of the Android SDK — Gradle
   pulls it in as a dependency, and there is no Gradle here.

   ```bash
   python3 scripts/fetch-android-deps.py
   ```

   leaves it in `vendor/android/tools/bundletool.jar`. Or point `AN_BUNDLETOOL`
   at a copy you already have.

3. **A Play Console account**, $25 once, at
   [play.google.com/console](https://play.google.com/console). You create the
   app there, and the first upload is where Play offers **Play App Signing**:
   accept it. Google then holds the key that signs what users install, and the
   key in your keystore becomes the *upload* key — the one that proves the
   bundle came from you. If you lose an upload key you can be issued another;
   there is no equivalent for the app signing key, which is why Play would rather
   hold it.

4. **A version code that goes up.** `android:versionCode` in
   `android/AndroidManifest.xml`. Play refuses a bundle whose version code it has
   already seen, and it refuses it on upload — after everything has been built.
   `an android --aab` checks that there is one before it compiles anything, but
   it cannot know which numbers you have already used.

## The commands

### `an ios --physical`

```bash
an ios --physical                    # the only device plugged in
an ios --physical --device "Jane's iPhone"
an ios --physical --no-launch        # build and sign, install by hand
```

It builds for `aarch64-apple-ios` rather than the simulator target, embeds the
profile in the bundle as `embedded.mobileprovision`, and signs with the
entitlements **the profile grants**. That last part is not a detail: the system
gives an app nothing its profile does not carry, so entitlements signed on top of
one produce an app that installs and is killed the moment it launches. The
plugins' requested keys are merged in and the profile wins every collision.

With one device connected it is used; with several you are asked to name one.

### `an ios --archive`

```bash
an ios --archive
```

Implies `--release`. It writes `build/ios/<Name>.xcarchive` — the app under
`Products/Applications`, the debug symbols under `dSYMs` — and
`build/ios/<Name>.ipa`, whose path it prints.

There is no `--method` flag. Whether that `.ipa` can go to TestFlight, to the
App Store or onto a handful of ad-hoc devices was decided by the certificate and
the profile you signed it with; a flag here would only be a second place to say
the same thing and a second place to get it wrong.

Uploading is not part of this. Open the `.xcarchive` in Xcode's Organizer, or use
`xcrun altool --upload-app`.

### `an android --sign` and `--aab`

```bash
export AN_ANDROID_KEYSTORE_PASSWORD='…'
an android --sign --release          # a release-signed APK
an android --aab --release           # the bundle Play takes
```

`--release` is about the compiler and `--sign` is about the key. They are
separate because they are separate things, and a build for Play wants both.
`--aab` implies `--sign`: Play takes nothing signed with a debug key.

The artefacts are named apart — `<Name>-release.apk`, `<Name>.apk` — so a
release build cannot quietly overwrite the debug APK your emulator has been
running.

A bundle is not an APK with a different extension. Its manifest and resources are
protobuf, its layout is a module zip, `bundletool` assembles it and `jarsigner`
signs it, because `apksigner` refuses one. All of that is `an`'s problem, not
yours; what is yours is the keystore and the version code.

### `an macos --sign`, `--notarize`, `--dmg`

```bash
an macos . --sign                    # Developer ID + hardened runtime
an macos . --notarize                # that, submitted, waited on, stapled
an macos . --notarize --dmg          # and packed into a signed, notarised .dmg
```

Without `--sign`, `an macos` signs ad hoc: it runs on the machine that built it
and nowhere else. `--dmg` on its own still works and says so out loud — a `.dmg`
of an ad-hoc build is a fine way to check the packaging and useless as a
download.

Signing turns on the **hardened runtime**, which notarisation requires, and which
forbids mapping writable executable memory — the first thing a JavaScript engine
does. So the build declares `com.apple.security.cs.allow-jit`. Without it the
app is killed on startup with a `Killed: 9` that mentions no entitlement
anywhere; it is the single most confusing failure on this page, and it is handled
rather than left to you.

Stapling is not optional and is easy to skip: without it the app is notarised but
the ticket lives on Apple's servers, so the first person to open it offline is
told the app cannot be opened, with nothing to distinguish that from an app that
was never notarised at all. `--notarize` staples.

The `.dmg` is signed and notarised in its own right when `--notarize` is on,
because the image is the file that gets downloaded. A `.dmg` carrying a perfectly
notarised app is still an unsigned download.

## When something is missing

Every one of these stops **before anything is compiled**, and names the thing:

- a platform with no `signing` section — and shows the block to paste and the
  environment variables that do the same job;
- a profile that is not there, or that is not a profile;
- a profile that expired, with the date;
- a profile for another app, with both bundle ids;
- a team that disagrees with the profile's;
- a certificate that is not in the keychain — listing the ones that are;
- a keystore that is not there, with the `keytool` line that creates one;
- a keystore whose password is wrong, or whose alias is not in it: the keystore
  is opened up front, so both arrive in a third of a second rather than after two
  minutes at `apksigner`;
- an environment variable nobody exported, by name;
- `bundletool` missing, naming all three places it was looked for;
- a `devicectl` that is too old, and the Xcode that has one.

And when a tool refuses anyway, what it said is repeated with the two things it
never mentions: which identity was used, and which file was being signed.

## What has actually been run

Being precise about this matters more here than anywhere else in these docs,
because a signing path that has never executed looks exactly like one that has.

**Exercised end to end by `scripts/check-signing.sh`, on a machine with no Apple
account:** the whole Android release path. A keystore generated by the check, an
APK signed with it and opened afterwards to confirm whose certificate is on it, a
bundle assembled, signed and unzipped to confirm its layout. Also the reading and
checking of provisioning profiles — the check builds real CMS-wrapped profiles
with `openssl` and a throwaway certificate, so expiry, app id and team are
validated against real files.

**Exercised as far as the credential:** everything else Apple. The check
confirms `an ios --physical` and `--archive` get through the profile and stop at
`codesign` naming the identity they wanted, and that `an macos --notarize`
reports the certificate and the notary profile together. The `codesign`
invocation itself is not run, because there is no certificate to run it with.

**Never run, by anybody, anywhere:** `devicectl` installing on a physical
iPhone, `notarytool` submitting to Apple, `stapler`, and any store upload. Those
were written from Apple's documentation. If you are the first person to hold an
Apple Developer account and point this at a real device, expect to find something
here — and the `--no-launch` variants build and sign the artefact either way, so
you can drag it into Xcode's Devices window and compare.
