#!/usr/bin/env python3
"""Compare what the system says about the tree with what the template asked for.

`uiautomator dump` is the one thing in all of this that is not our word. It
serialises the `AccessibilityNodeInfo` tree the platform builds — the same tree
TalkBack walks — so a label that shows up here is a label that arrived, and not
a label we believe we wrote.

Two dumps are read, and the difference between them is the point:

  · the plain one includes views the system does not consider important for
    accessibility, because UiAutomation asks for them so that a UI test can
    reach anything;
  · `--compressed` is the one that does not: it is the tree a screen reader
    walks, and it is where `accessible: false` has to have taken a row and its
    contents out.

Expectations are not written here. They are read out of the example's template
and out of the role table in the host's own Java, so this file cannot say the
template asked for something it did not.
"""

import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
full_dump = pathlib.Path(sys.argv[2])
compressed_dump = pathlib.Path(sys.argv[3])

failures: list[str] = []
checked = 0

component = (root / 'examples/a11y/src/app.component.ts').read_text()
host_java = (root / 'shells/android/java/dev/angularnative/AnAccessibility.java').read_text()

# --------------------------------------------------------------- the role table
#
# Read out of the host instead of copied here. A fourth copy of the same list is
# a fourth thing that can drift, and this one would drift silently: a wrong
# expectation makes the check fail, but a wrong expectation that happens to
# match a wrong implementation makes it pass.
ROLE_CLASS: dict[str, str | None] = {}
ROLE_CHECKABLE: dict[str, bool] = {}
for match in re.finditer(
    r'case "([^"]+)":.*?return new Role\(\s*([^,]+),\s*[^,]+,\s*(true|false),\s*(true|false)\)',
    host_java,
    re.S,
):
    name, class_name, checkable, _heading = match.groups()
    ROLE_CLASS[name] = None if class_name.strip() == 'null' else class_name.strip().strip('"')
    ROLE_CHECKABLE[name] = checkable == 'true'


# ------------------------------------------------------------------- the template


def template_of(source: str) -> str:
    start = source.index('template: `') + len('template: `')
    return source[start : source.index('\n  `\n})', start)]


def strip_comments(text: str) -> str:
    return re.sub(r'<!--.*?-->', '', text, flags=re.S)


class Element:
    def __init__(self, tag: str, attrs: dict[str, str], events: set[str]):
        self.tag = tag
        self.attrs = attrs
        self.events = events
        self.inner = ''


def parse(template: str) -> list[Element]:
    """A parser small enough to read, because the input is one file we wrote.

    It only needs to know three things: the tag, the bracket bindings, and
    where the element ends so a hidden row's contents can be found.
    """
    elements: list[Element] = []
    stack: list[tuple[Element, int]] = []
    index = 0
    while True:
        opening = re.compile(r'<(an-[a-z-]+)').search(template, index)
        closing = re.compile(r'</(an-[a-z-]+)>').search(template, index)
        if opening is None and closing is None:
            break
        if closing is not None and (opening is None or closing.start() < opening.start()):
            if stack:
                element, content_start = stack.pop()
                element.inner = template[content_start : closing.start()]
            index = closing.end()
            continue
        # Walk to the `>` that closes the open tag, ignoring quoted values.
        cursor = opening.end()
        quote = ''
        while cursor < len(template):
            char = template[cursor]
            if quote:
                if char == quote:
                    quote = ''
            elif char in '"\'':
                quote = char
            elif char == '>':
                break
            cursor += 1
        raw = template[opening.end() : cursor]
        element = Element(
            opening.group(1),
            dict(re.findall(r'\[([\w.]+)\]="([^"]*)"', raw)),
            set(re.findall(r'\((\w+)\)="', raw)),
        )
        elements.append(element)
        if raw.rstrip().endswith('/'):
            index = cursor + 1
        else:
            stack.append((element, cursor + 1))
            index = cursor + 1
    return elements


def literal(expression: str) -> str | None:
    """`[accessibilityLabel]="'Volume'"` is a string; anything else is not."""
    match = re.fullmatch(r"'([^']*)'", expression.strip())
    return match.group(1) if match else None


# `[accessibilityState]="off"` points at a field of the component.
STATE_FIELDS: dict[str, dict[str, object]] = {}
for name, body in re.findall(
    r'readonly (\w+): NativeAccessibilityState = \{([^}]*)\}', component
):
    state: dict[str, object] = {}
    for key, value in re.findall(r"(\w+): (true|false|'[^']*')", body):
        state[key] = True if value == 'true' else False if value == 'false' else value.strip("'")
    STATE_FIELDS[name] = state


# ----------------------------------------------------------------- the dumps


def nodes_of(path: pathlib.Path) -> list[dict[str, str]]:
    text = path.read_text()
    return [
        dict(re.findall(r'([a-z-]+)="([^"]*)"', match.group(1)))
        for match in re.finditer(r'<node ([^>]*?)/?>', text)
    ]


full = nodes_of(full_dump)
compressed = nodes_of(compressed_dump)


def by_description(nodes: list[dict[str, str]], description: str) -> dict[str, str] | None:
    found = [node for node in nodes if node.get('content-desc') == description]
    return found[0] if len(found) == 1 else None


def expect(node: dict[str, str], attribute: str, value: str, who: str) -> None:
    global checked
    checked += 1
    actual = node.get(attribute, '')
    if actual != value:
        failures.append(
            f'  FAIL "{who}": the system says {attribute}="{actual}" and the template'
            f' asked for "{value}"'
        )


# ------------------------------------------------------------------ the compare

elements = parse(strip_comments(template_of(component)))
rows = 0

for element in elements:
    attrs = element.attrs
    label = literal(attrs.get('accessibilityLabel', ''))
    test_id = literal(attrs.get('testID', ''))
    hidden = attrs.get('accessible', '').strip() == 'false'

    if hidden:
        # A hidden row has no name to look it up by: what is checked is its
        # contents, which the system has to have taken out of the screen
        # reader's tree and left in the plain one. Both halves matter — gone
        # from both would just mean the row never got mounted.
        inner = re.search(r'>([^<>]+)</an-text>', element.inner)
        if not inner:
            failures.append('  FAIL the hidden row has no text to look for')
            continue
        text = inner.group(1).strip()
        rows += 1
        checked += 2
        if not any(node.get('text') == text for node in full):
            failures.append(
                f'  FAIL "{text}" is not in the plain dump either: the hidden row was'
                ' never mounted, so hiding it proves nothing'
            )
        if any(node.get('text') == text for node in compressed):
            failures.append(
                f'  FAIL "{text}" is still in the tree a screen reader walks, and the'
                ' template said accessible: false'
            )
        continue

    name = label if label is not None else test_id
    if name is None:
        continue

    rows += 1
    node = by_description(compressed, name)
    if node is None:
        failures.append(
            f'  FAIL no single node in the tree a screen reader walks is named'
            f' "{name}", and the template named one'
        )
        continue

    if test_id is not None and label is not None:
        # Both end up in `contentDescription`; finding the node by its label is
        # already the proof that the label won.
        checked += 1

    role = literal(attrs.get('accessibilityRole', ''))
    checkable = False
    if role is not None:
        if role not in ROLE_CLASS:
            failures.append(f'  FAIL the host has no translation for the role "{role}"')
            continue
        class_name = ROLE_CLASS[role]
        if class_name is not None:
            expect(node, 'class', class_name, name)
        checkable = ROLE_CHECKABLE[role]

    hint = literal(attrs.get('accessibilityHint', ''))
    if hint is not None:
        expect(node, 'hint', hint, name)

    if 'press' in element.events:
        expect(node, 'clickable', 'true', name)

    state = STATE_FIELDS.get(attrs.get('accessibilityState', '').strip(), {})
    if 'checked' in state:
        checkable = True
        # `mixed` is not a third boolean: on Android it is a checkable node
        # that is not checked, plus a stateDescription the dump cannot show.
        expect(node, 'checked', 'true' if state['checked'] is True else 'false', name)
    if checkable:
        expect(node, 'checkable', 'true', name)
    if 'selected' in state:
        expect(node, 'selected', 'true' if state['selected'] else 'false', name)
    if 'disabled' in state:
        expect(node, 'enabled', 'false' if state['disabled'] else 'true', name)

# The two controls the template deliberately says nothing about.
#
# This block is written out and not derived, because what it asserts is not in
# the template: it is what Material puts there by itself. It is the check that
# catches the tempting mistake — writing our role and our state onto every node
# — which would replace a switch that announces itself correctly with one that
# announces itself the way we guessed.
for class_name, label in (('android.widget.Switch', 'switch'), ('android.widget.Button', 'OK')):
    untouched = [
        node
        for node in compressed
        if node.get('class') == class_name and node.get('content-desc') == ''
    ]
    checked += 1
    if not untouched:
        failures.append(
            f'  FAIL there is no {class_name} left alone: the {label} of the template'
            ' carries no accessibility prop and the host has written on it anyway'
        )
        continue
    if class_name.endswith('Switch'):
        checked += 1
        if untouched[0].get('checked') != 'true':
            failures.append(
                '  FAIL the untouched switch does not report itself as on: its own state'
                ' has been trampled'
            )

print(f'  ok   {rows} rows of the template found in the tree the system reports')
print(f'  ok   {checked} node attributes match what the template asked for')
for line in failures:
    print(line)
sys.exit(1 if failures else 0)
