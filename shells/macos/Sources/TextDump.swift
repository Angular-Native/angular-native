import AppKit

/// The strings the window is really showing, written to the log.
///
/// It exists for the one thing a screenshot cannot do: prove that a hot reload
/// kept the app's state. The picture shows that the window is not black and
/// that the new template arrived, but nobody can read a counter out of a PNG,
/// and «seconds running: 8» against «seconds running: 0» is exactly the
/// difference between a fast refresh and a restart wearing its clothes.
///
/// It walks the hierarchy the host mounted and logs the string of every view
/// that carries one. It invents nothing and asks the core nothing: what comes
/// out is what the `NSTextField` on screen would say if it were asked out loud,
/// which is the point — the check reads the window, not the model behind it.
///
/// Turned on with `AN_DUMP_TEXT=1`, and it fires at the same moment the
/// screenshot does, so it needs `AN_SCREENSHOT` as well. On its own it would
/// have nothing to hang off; `RootViewController` says so at startup rather
/// than letting the variable do nothing in silence.
enum TextDump {
    static var wanted: Bool {
        ProcessInfo.processInfo.environment["AN_DUMP_TEXT"] == "1"
    }

    /// One line per string, each prefixed so a script can pick them out of the
    /// rest of the log.
    static func write(_ root: NSView) {
        for text in strings(of: root) {
            NSLog("angular-native: [text] %@", text)
        }
    }

    private static func strings(of view: NSView) -> [String] {
        var found: [String] = []
        // `NSTextField` first: it is an `NSControl` too, and a button is asked
        // for its title, which is where its text lives.
        if let field = view as? NSTextField {
            found.append(squeezed(field.stringValue))
        } else if let editor = view as? NSTextView {
            found.append(squeezed(editor.string))
        } else if let button = view as? NSButton {
            found.append(squeezed(button.title))
        }
        for child in view.subviews {
            found.append(contentsOf: strings(of: child))
        }
        return found.filter { !$0.isEmpty }
    }

    /// The whitespace is squeezed on purpose. A template writes its text across
    /// several lines and indented, and it reaches the `NSTextField` with the
    /// newlines and the indentation still in it; a check looking for a sentence
    /// should not have to know how somebody laid the template out.
    private static func squeezed(_ text: String) -> String {
        text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }
}
