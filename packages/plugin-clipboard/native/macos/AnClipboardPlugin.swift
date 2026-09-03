import AppKit

/// The Mac's clipboard.
///
/// It is compiled into the `.app` in the same `swiftc` invocation as the shell,
/// so it sees `AnPlugin` and `AnPluginCall` without importing anything. What
/// registers it is the file `an` generates out of the `package.json`.
///
/// `NSPasteboard` belongs to the main thread, and we are already on it here: the
/// core delivers the calls inside the frame. That is why the answer can be given
/// on the spot instead of holding on to the `AnPluginCall` for later.
///
/// Three things are genuinely different from `UIPasteboard`, and they are the
/// reason this is a separate file rather than the iOS one under a `#if`:
///
///   · **Writing means clearing first.** A pasteboard holds several
///     representations of one thing, and `setString` adds one rather than
///     replacing what is there. Without `clearContents()` the old flavours
///     survive, and the next app to paste can pick a different one and get the
///     previous text back.
///
///   · **`clearContents()` is what bumps the change count**, and the change
///     count is the pasteboard's version number. Skipping it also means every
///     `NSPasteboard` observer on the machine misses the change.
///
///   · **There is no paste notification to avoid.** iOS 16 shows a banner when
///     an app reads what another one copied, which is why the iOS half is
///     careful to answer `hasText` without reading. macOS shows nothing, so
///     `hasText` could simply read — but it does not, because asking the
///     cheaper question is still the right one and it keeps the two halves
///     answering the same thing for the same reason.
final class AnClipboardPlugin: AnPlugin {
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        let pasteboard = NSPasteboard.general
        switch method {
        case "write":
            guard let text = args["text"] as? String else {
                respond.reject("clipboard.write needs a piece of text in 'text'")
                return
            }
            // See the header: clear, then write. `setString` returns false when
            // the pasteboard refused —another process owns it— and that is an
            // answer, not something to swallow.
            pasteboard.clearContents()
            if pasteboard.setString(text, forType: .string) {
                respond.resolve()
            } else {
                respond.reject("the pasteboard would not take the text; another app may own it")
            }

        case "read":
            // An empty string and not null: whoever asks for the clipboard wants
            // to paint something, and `undefined` would force a check at every
            // single use. It matches the iOS half exactly.
            respond.resolve(pasteboard.string(forType: .string) ?? "")

        case "hasText":
            // Asks what flavours are on offer without fetching any of them.
            // `canReadObject` would also say yes to a file URL that happens to
            // have a string representation, and `read` would then hand back
            // something nobody copied as text.
            respond.resolve(pasteboard.types?.contains(.string) ?? false)

        default:
            // Never in silence: a method that does not exist rejects the promise
            // saying which one was asked for.
            respond.reject("the clipboard plugin has no method \(method)")
        }
    }
}
