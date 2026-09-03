import UIKit

/// The iOS clipboard.
///
/// It is compiled into the `.app` in the same `swiftc` invocation as the shell,
/// so it sees `AnPlugin` and `AnPluginCall` without importing anything. What
/// registers it is the file `an` generates out of the `package.json`.
///
/// `UIPasteboard` belongs to the main thread, and we are already on it here: the
/// core delivers the calls inside the frame. That is why the answer can be given
/// on the spot instead of holding on to the `AnPluginCall` for later.
final class AnClipboardPlugin: AnPlugin {
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "write":
            guard let text = args["text"] as? String else {
                respond.reject("clipboard.write needs a piece of text in 'text'")
                return
            }
            UIPasteboard.general.string = text
            respond.resolve()

        case "read":
            // An empty string and not null: whoever asks for the clipboard wants
            // to paint something, and `undefined` would force a check at every
            // single use.
            respond.resolve(UIPasteboard.general.string ?? "")

        case "hasText":
            // `hasStrings` does not read the contents, so it does not trigger the
            // paste notice iOS 16 shows when reading what another app copied.
            respond.resolve(UIPasteboard.general.hasStrings)

        default:
            // Never in silence: a method that does not exist rejects the promise
            // saying which one was asked for.
            respond.reject("the clipboard plugin has no method \(method)")
        }
    }
}
