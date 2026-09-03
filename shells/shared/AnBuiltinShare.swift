import Foundation

#if os(iOS) || os(visionOS)
    import UIKit
#elseif os(macOS)
    import AppKit
#endif

/// The `share` module on Apple's platforms.
///
/// It is the system's own sheet and not a screen of ours that looks like one:
/// `UIActivityViewController` on the phone, the iPad and the headset,
/// `NSSharingServicePicker` on the Mac. That matters beyond the look — what
/// appears in that sheet is every app on the device that declared it can receive
/// this kind of thing, plus AirDrop, plus the shortcuts the person pinned, and
/// none of that is something a framework can reproduce.
///
/// Two platforms have no sheet at all and say so by name rather than showing
/// something else. `canShare()` answers `false` there, so an app can hide the
/// button instead of finding out by being rejected.
final class AnBuiltinShare: AnBuiltinModule {
    /// Kept alive by hand while the sheet is up: on the Mac the picker's
    /// delegate is held weakly, and a delegate that has been deallocated is a
    /// promise nobody will ever answer.
    private var pending: AnyObject?

    func call(_ method: String, _ args: [String: Any], _ respond: AnBuiltinCall) {
        switch method {
        case "canShare":
            #if os(iOS) || os(visionOS) || os(macOS)
                respond.resolve(true)
            #else
                respond.resolve(false)
            #endif

        case "share":
            share(args, respond)

        default:
            respond.reject("the share module has no method \(method)")
        }
    }

    /// What is being shared, in the order the system likes to see it: the title
    /// first if there is one, then the text, then the link, then the files.
    private static func items(_ args: [String: Any]) -> [Any] {
        var items: [Any] = []
        if let text = args["text"] as? String, !text.isEmpty {
            items.append(text)
        }
        if let link = args["url"] as? String, let url = URL(string: link) {
            items.append(url)
        }
        for path in args["files"] as? [String] ?? [] {
            items.append(URL(fileURLWithPath: path))
        }
        return items
    }

    private func share(_ args: [String: Any], _ respond: AnBuiltinCall) {
        let items = Self.items(args)
        guard !items.isEmpty else {
            respond.reject(
                "share.share was given nothing to share: it needs at least one of 'text', "
                    + "'url' or 'files'")
            return
        }

        #if os(iOS) || os(visionOS)
            guard let host = AnBuiltinHost.viewController else {
                respond.reject("share.share has no screen to present the sheet from")
                return
            }
            let sheet = UIActivityViewController(activityItems: items, applicationActivities: nil)
            if let title = args["title"] as? String {
                sheet.setValue(title, forKey: "subject")
            }
            // On an iPad and in the headset the sheet is a popover and a popover
            // has to come from somewhere. Without this it is not that it looks
            // wrong: UIKit raises, and the app goes down while trying to share.
            if let popover = sheet.popoverPresentationController {
                popover.sourceView = host.view
                popover.sourceRect = CGRect(
                    x: host.view.bounds.midX, y: host.view.bounds.midY, width: 0, height: 0)
                popover.permittedArrowDirections = []
            }
            sheet.completionWithItemsHandler = { _, completed, _, error in
                if let error {
                    respond.reject("share.share failed: \(error.localizedDescription)")
                } else {
                    // `false` is somebody dismissing the sheet, which is not a
                    // failure and must not reject: a `catch` firing on a
                    // cancelled share would make backing out look like a bug.
                    respond.resolve(completed)
                }
            }
            host.present(sheet, animated: true)

        #elseif os(macOS)
            guard let view = AnBuiltinHost.view else {
                respond.reject("share.share has no view to hang the picker off")
                return
            }
            let picker = NSSharingServicePicker(items: items)
            let delegate = AnSharingPickerDelegate { [weak self] chose in
                self?.pending = nil
                respond.resolve(chose)
            }
            pending = delegate
            picker.delegate = delegate
            // From the middle of the window, which is where a picker with no
            // button behind it belongs.
            let anchor = NSRect(x: view.bounds.midX, y: view.bounds.midY, width: 1, height: 1)
            picker.show(relativeTo: anchor, of: view, preferredEdge: .minY)

        #elseif os(tvOS)
            respond.reject(
                "tvOS has no share sheet: UIActivityViewController does not exist there, and a "
                    + "television has no AirDrop, no Messages and no apps to hand something to. "
                    + "Show a code or a link on the screen instead.")

        #elseif os(watchOS)
            respond.reject(
                "watchOS has no share sheet: the watch has no UIActivityViewController and no "
                    + "app to hand something to. Send it to the phone and share it from there.")

        #else
            respond.reject("this platform has no share sheet")
        #endif
    }
}

#if os(macOS)
    /// The picker's delegate.
    ///
    /// `NSSharingServicePicker` has no completion handler: what it has is this
    /// one call, with the service that was chosen or `nil` if the menu was
    /// dismissed. It answers exactly once either way, so cancelling ends the
    /// promise instead of leaving it open.
    private final class AnSharingPickerDelegate: NSObject, NSSharingServicePickerDelegate {
        private let answer: (Bool) -> Void
        private var answered = false

        init(answer: @escaping (Bool) -> Void) {
            self.answer = answer
        }

        func sharingServicePicker(
            _ picker: NSSharingServicePicker, didChoose service: NSSharingService?
        ) {
            guard !answered else { return }
            answered = true
            answer(service != nil)
        }
    }
#endif
