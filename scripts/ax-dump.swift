// The accessibility tree of a running app, as the system publishes it.
//
// This is the outside observer. It is a separate process that never links
// against the app, never sees an `NSView`, and cannot read a single one of the
// setters the host called. All it has is a pid and `AXUIElementCopyAttribute-
// Value`, which is the same door VoiceOver, Accessibility Inspector and
// XCTest's UI tests go through: the app is asked, over the accessibility
// server, what it publishes about itself.
//
// That distinction is the whole point. A check that reads back the property it
// just wrote proves that a setter works; it proves nothing about whether a
// screen reader would ever see the value. This one walks the tree the reader
// walks, and only what survives that walk counts as having arrived.
//
// It needs the Accessibility permission —`AXIsProcessTrusted()`— which is
// granted per app in System Settings and cannot be granted from a script. When
// it is missing this exits with 2 and says so, so the check that calls it can
// skip rather than fail: a machine without the grant has not broken anything.
//
// Usage:  ax-dump <pid>
// Output: one line per element, indented by depth, as
//         role[subrole] label="…" value="…" help="…" enabled=… selected=…

import ApplicationServices
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 2, let pid = pid_t(arguments[1]) else {
    FileHandle.standardError.write(Data("usage: ax-dump <pid>\n".utf8))
    exit(64)
}

guard AXIsProcessTrusted() else {
    FileHandle.standardError.write(
        Data(
            """
            ax-dump: this process does not have the Accessibility permission, so it cannot \
            read another app's accessibility tree.
            Grant it in System Settings > Privacy & Security > Accessibility, to the terminal \
            (or the app) running this.
            """.utf8
        )
    )
    exit(2)
}

/// One attribute, as a string, or nil when the element does not publish it.
///
/// Every attribute is asked for by name and not read off any object: from here
/// there is no object, there is a reference into another process.
func attribute(_ element: AXUIElement, _ name: String) -> String? {
    var raw: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &raw) == .success,
          let raw
    else { return nil }
    if let text = raw as? String { return text }
    if let number = raw as? NSNumber { return number.stringValue }
    return String(describing: raw)
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    var raw: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &raw) == .success,
          let list = raw as? [AXUIElement]
    else { return [] }
    return list
}

/// Escapes only what would make a line ambiguous to grep: quotes and newlines.
func quoted(_ text: String) -> String {
    let flat = text.replacingOccurrences(of: "\n", with: "\\n")
        .replacingOccurrences(of: "\"", with: "\\\"")
    return "\"\(flat)\""
}

func describe(_ element: AXUIElement, depth: Int) -> String {
    var parts: [String] = []
    let role = attribute(element, kAXRoleAttribute) ?? "?"
    if let subrole = attribute(element, kAXSubroleAttribute) {
        parts.append("\(role)[\(subrole)]")
    } else {
        parts.append(role)
    }
    // AppKit's `setAccessibilityLabel:` surfaces here as AXDescription, and a
    // control that never got one publishes its AXTitle instead. Both are
    // printed: which of the two carries the name is exactly the difference
    // between a label of ours and the system's own, and a check has to be able
    // to tell them apart.
    for (name, key) in [
        ("label", kAXDescriptionAttribute),
        ("title", kAXTitleAttribute),
        ("value", kAXValueAttribute),
        ("help", kAXHelpAttribute)
    ] where attribute(element, key) != nil {
        parts.append("\(name)=\(quoted(attribute(element, key)!))")
    }
    for (name, key) in [("enabled", kAXEnabledAttribute), ("selected", kAXSelectedAttribute)] {
        if let raw = attribute(element, key) {
            parts.append("\(name)=\(raw == "1" ? "true" : "false")")
        }
    }
    return String(repeating: "  ", count: depth) + parts.joined(separator: " ")
}

func walk(_ element: AXUIElement, depth: Int) {
    print(describe(element, depth: depth))
    // A deep tree is a bug elsewhere, not here, but a runaway recursion in a
    // check that is supposed to report is worse than a truncated tree.
    guard depth < 40 else { return }
    for child in children(element) {
        walk(child, depth: depth + 1)
    }
}

let app = AXUIElementCreateApplication(pid)
var role: CFTypeRef?
guard AXUIElementCopyAttributeValue(app, kAXRoleAttribute as CFString, &role) == .success else {
    FileHandle.standardError.write(
        Data("ax-dump: pid \(pid) publishes no accessibility tree\n".utf8)
    )
    exit(1)
}
walk(app, depth: 0)
