#!/usr/bin/env python3
"""The four things that only exist for the watch, and that live in four
different places.

It is kept apart from the `.sh` because these are text checks and not process
ones, and because the shell is no place to read XML.
"""

import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
failures: list[str] = []


def read(path: str) -> str:
    file = root / path
    if not file.is_file():
        failures.append(f'  FAIL {path} is missing')
        return ''
    return file.read_text()


watch_manifest = read('shells/android/AndroidManifest.wear.xml')
phone_manifest = read('shells/android/AndroidManifest.xml')
styles = read('shells/android/res/values/styles.xml')
host = read('shells/android/java/dev/angularnative/AnHost.java')
scroll = read('shells/android/java/dev/angularnative/AnScrollView.java')
cli = read('crates/an-cli/src/android.rs')
doc = read('docs-site/src/content/docs/platforms/wearos.md')
primitives = read('packages/primitives/src/primitives.ts')

# 1. The watch manifest declares the device's form factor. Without this the APK
#    is the phone's under another name: it installs the same and starts the
#    same, and that is why nobody catches it.
for required, reason in [
    ('android.hardware.type.watch', 'the feature that makes it a watch app'),
    ('com.google.android.wearable.standalone', 'that it needs no paired phone'),
]:
    if required not in watch_manifest:
        failures.append(f'  FAIL the watch manifest does not declare {required}: {reason}')
if 'android.hardware.type.watch' in phone_manifest:
    failures.append('  FAIL the phone manifest claims to be a watch one')

# 2. Each manifest points at a theme, and the theme has to exist. A `@style/`
#    that does not resolve is caught by aapt2; one that resolves to the other
#    device's theme is not.
declared = set(re.findall(r'<style name="([^"]+)"', styles))
for path, text in [
    ('AndroidManifest.xml', phone_manifest),
    ('AndroidManifest.wear.xml', watch_manifest),
]:
    used = re.search(r'android:theme="@style/([^"]+)"', text)
    if not used:
        failures.append(f'  FAIL {path} sets no android:theme')
    elif used.group(1) not in declared:
        failures.append(
            f'  FAIL {path} uses @style/{used.group(1)}, which is not in res/values/styles.xml'
        )

# The watch's "back". There is no hardware button: if the theme does not bring
# it, the app has no way out and gives no error at all, nothing simply happens
# on swiping.
if 'android:windowSwipeToDismiss' not in styles:
    failures.append('  FAIL the watch theme does not turn windowSwipeToDismiss on')

# 3. The list of primitives that are not mounted.
#
#    It lives in two Java methods —the reason and the tag— and is documented in
#    `docs-site/src/content/docs/platforms/wearos.md`. Three places that can drift apart: a primitive with a
#    reason and no tag comes out as "kind 7", and one in Java and not in the
#    document is only discovered when somebody uses it.
def kinds_of(method: str) -> set[str]:
    body = re.search(
        r'private static String ' + method + r'\(int kind\) \{(.*?)\n    \}',
        host,
        re.S,
    )
    if not body:
        failures.append(f'  FAIL AnHost.{method} not found')
        return set()
    return set(re.findall(r'case (KIND_\w+):', body.group(1)))


with_reason = kinds_of('notOnTheWatch')
with_tag = kinds_of('kindName')
if with_reason != with_tag:
    reason_only = ', '.join(sorted(with_reason - with_tag))
    tag_only = ', '.join(sorted(with_tag - with_reason))
    if reason_only:
        failures.append(f'  FAIL no tag in kindName: {reason_only}')
    if tag_only:
        failures.append(f'  FAIL kindName names things that do get mounted: {tag_only}')

real_tags = set(re.findall(r"@Directive\(\{ selector: '(an-[^']+)' \}\)", primitives))
if not real_tags:
    failures.append('  FAIL not one tag could be read from packages/primitives')

tag_body = re.search(
    r'private static String kindName\(int kind\) \{(.*?)\n    \}', host, re.S
)
declared_tags = (
    set(re.findall(r'return "(an-[^"]+)";', tag_body.group(1)))
    if tag_body
    else set()
)
invented = sorted(declared_tags - real_tags)
if invented:
    failures.append(
        '  FAIL the watch rejects tags that do not exist: ' + ', '.join(invented)
    )
undocumented = sorted(t for t in declared_tags if f'`{t}`' not in doc)
if undocumented:
    failures.append(
        '  FAIL these do not go on the watch and the Wear OS page does not name them: '
        + ', '.join(undocumented)
    )

# 4. The crown. It is a generic event and not a touch: if somebody moved it to
#    `onTouchEvent` it would stop arriving, and on the phone —where there is no
#    crown— no other check would fail.
for required, reason in [
    ('SOURCE_ROTARY_ENCODER', "the crown's source"),
    ('onGenericMotionEvent', 'the path its events arrive by'),
    ('AXIS_SCROLL', 'the axis that carries the detents'),
    ('setFocusableInTouchMode', 'without focus, the crown does not reach the list'),
]:
    if required not in scroll:
        failures.append(f'  FAIL AnScrollView does not mention {required}: {reason}')

# And only on the watch: on a phone, a list that asks for the focus takes it away
# from whatever text field is underneath.
if not re.search(r'if \(watch\) \{\s*\n\s*scroll\.enableRotary\(\);', host):
    failures.append('  FAIL AnHost turns the crown on outside a watch (or does not turn it on)')

# And the raw crown, which is the other half: scrolling is not the only thing
# done with it. The output lives in `packages/primitives` and both watches
# provide it; if the Android host stops delivering it, the template subscribes to
# something that never fires and nobody finds out, which is the failure this
# whole document is trying to avoid.
crown_body = re.search(r'private void setCrown\((.*?)\n    \}', host, re.S)
if not crown_body:
    failures.append('  FAIL AnHost does not deliver (crown): the output exists and never arrives here')
else:
    crown = crown_body.group(1)
    if 'if (!watch)' not in crown:
        failures.append('  FAIL AnHost delivers (crown) outside a watch, where there is no crown')
    if 'Log.e' not in crown:
        failures.append(
            '  FAIL off the watch, (crown) is discarded in silence instead of saying so'
        )
if 'crownIdle' not in host:
    failures.append(
        '  FAIL (crownIdle) is missing: the system sends detents and goes quiet, so the end '
        'has to be counted by the host'
    )
if not re.search(r'setOnGenericMotionListener\(this\)', host):
    failures.append('  FAIL the raw crown does not listen along the generic event path')

# 5. The inset of the round screen. It is geometry, not taste: the side of the
#    inscribed square is d/√2. A constant picked by eye would go unnoticed.
if '(1 - 1 / Math.sqrt(2)) / 2' not in host:
    failures.append('  FAIL ROUND_INSET is no longer the square inscribed in the circle')
if 'isScreenRound' not in host:
    failures.append('  FAIL the host guesses the screen shape instead of asking for it')

# 6. The CLI builds two form factors and not one. `Form::Watch` without its
#    manifest would be a phone APK with a watch's name.
if 'AndroidManifest.wear.xml' not in cli:
    failures.append('  FAIL the CLI does not know about the watch manifest')
if 'ro.build.characteristics' not in cli:
    failures.append('  FAIL the CLI does not tell a watch from a phone when installing')

# 7. And that this device reaches `adb`.
#
#    Asking for the text is not enough, and the checker itself learned this:
#    `ro.build.characteristics` was in the file, in a function nobody called.
#    `install_and_launch` received the form factor and the `--device` and used
#    neither. With two devices running, `adb` refuses; with a phone alone, the
#    watch APK installs on the phone, starts, paints, and nobody says a word.
install_body = re.search(r'pub fn install_and_launch\((.*?)\n\}\n', cli, re.S)
if not install_body:
    failures.append('  FAIL install_and_launch not found in the CLI')
else:
    install = install_body.group(1)
    if 'pick_device' not in install:
        failures.append(
            '  FAIL install_and_launch picks no device: it sends the APK to whichever adb likes'
        )
    for command, purpose in [
        ('install', 'installing'),
        ('start', 'launching'),
        ('force-stop', 'stopping the previous app'),
    ]:
        for arguments in re.findall(r'\[([^\[\]]*"' + command + r'"[^\[\]]*)\]', install):
            if '"-s"' not in arguments:
                failures.append(
                    f'  FAIL {purpose} without `-s`: it goes to whichever device adb picks, '
                    'not to the chosen one'
                )

# 8. The platform an app reads is the device's and not the APK's.
#
#    `deviceInfo()` is what `Device.info()` answers, and `NativePlatform`
#    declares 'wearos'. A hard-coded "android" there means an app on a watch is
#    told it is on a phone, which is the one thing that value exists to prevent.
#    And it has to come from `watch` —the field the constructor fills from
#    FEATURE_WATCH— and not from the manifest: the phone APK installs on a
#    watch, and then the manifest says phone while the system says watch.
device_info = re.search(r'public String deviceInfo\(\) \{(.*?)\n    \}', host, re.S)
if not device_info:
    failures.append('  FAIL AnHost.deviceInfo not found')
else:
    body = device_info.group(1)
    if '"wearos"' not in body:
        failures.append(
            '  FAIL deviceInfo never answers "wearos": on a watch an app is told it is on a phone'
        )
    elif not re.search(r'\bwatch\b\s*\?', body):
        failures.append(
            '  FAIL deviceInfo picks the platform without asking the FEATURE_WATCH flag'
        )

for line in failures:
    print(line)
if failures:
    sys.exit(1)

print('  ok   the watch manifest declares its feature and its theme')
print(f'  ok   the {len(declared)} themes in res/values/styles.xml cover both manifests')
print(
    f'  ok   the {len(declared_tags)} primitives that do not go on the watch '
    'agree in Java and on the Wear OS page'
)
print('  ok   the crown arrives through onGenericMotionEvent and only on the watch')
print('  ok   (crown) and (crownIdle) are delivered by the host, and off the watch they say so')
print('  ok   the APK goes to the watch-shaped device, and every adb carries its -s')
print('  ok   the inset of the round screen is the inscribed square')
print('  ok   deviceInfo answers wearos on a watch and android on a phone')
