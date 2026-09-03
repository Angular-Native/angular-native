import Foundation

#if os(iOS) || os(visionOS)
    import UIKit
    import UniformTypeIdentifiers
#elseif os(macOS)
    import AppKit
    import UniformTypeIdentifiers
#endif

/// A path, or why there is not one.
///
/// It is not a `Result`: Swift's wants its failure to be an `Error`, and what is
/// carried here is the sentence the promise will be rejected with. Wrapping it
/// in an error type would be a type existing so that another type is happy.
enum AnFilePath {
    case at(URL)
    case refused(String)

    /// The reason, for a message that has to explain a failure it did not cause.
    var reason: String? {
        if case .refused(let reason) = self { return reason }
        return nil
    }

    var url: URL? {
        if case .at(let url) = self { return url }
        return nil
    }
}

/// The `files` module on Apple's platforms.
///
/// Two things, and they are not the same thing:
///
/// - **The app's own storage.** Foundation's `FileManager` over two directories
///   the app owns: one that lasts and one the system may empty when it needs
///   room. Nothing here asks anybody's permission, because the app is inside its
///   own container.
/// - **What the person chose.** The system picker —`UIDocumentPickerViewController`
///   on the phone and the headset, `NSOpenPanel` on the Mac— which is the only
///   way to reach a file outside the container. It is the platform's own picker
///   and not a list of our own that looks like one.
///
/// Between the two there is a rule the module enforces rather than documents:
/// **it writes only inside the app's directories, and outside them it reads only
/// what the person picked, and only while the app is running.** A path that is
/// neither is turned down saying which of the two it failed. Without that rule a
/// path arriving from JS is a path to anywhere on the disk.
///
/// A relative path is resolved against the durable directory, so
/// `read({ path: 'notes.txt' })` works without ever asking where that is.
final class AnBuiltinFiles: AnBuiltinModule {
    /// What the person picked, by the path handed back to JS.
    ///
    /// On the Mac and on iOS a picked file lives outside the container and is
    /// reachable through a security-scoped URL: the entry is what makes the
    /// later `read` legal, and `startAccessingSecurityScopedResource` around the
    /// read is what makes it work. It is emptied when the app dies, which is
    /// exactly how long the permission lasts.
    private var granted: [String: URL] = [:]

    /// Kept alive by hand: `UIDocumentPickerViewController` holds its delegate
    /// weakly, and a delegate that has been deallocated is a picker that
    /// dismisses without ever answering.
    private var pending: AnyObject?

    func call(_ method: String, _ args: [String: Any], _ respond: AnBuiltinCall) {
        switch method {
        case "documentsDirectory":
            switch Self.documents() {
            case .at(let url): respond.resolve(url.path)
            case .refused(let reason): respond.reject(reason)
            }

        case "cacheDirectory":
            switch Self.caches() {
            case .at(let url): respond.resolve(url.path)
            case .refused(let reason): respond.reject(reason)
            }

        case "exists":
            resolvePath(args, respond) { url in
                respond.resolve(FileManager.default.fileExists(atPath: url.path))
            }

        case "read":
            resolvePath(args, respond) { url in
                self.reading(url) {
                    do {
                        respond.resolve(try String(contentsOf: url, encoding: .utf8))
                    } catch {
                        respond.reject("files.read could not read \(url.path): \(error)")
                    }
                }
            }

        case "write":
            guard let text = args["text"] as? String else {
                respond.reject("files.write needs the contents in 'text'")
                return
            }
            writablePath(args, respond) { url in
                do {
                    try FileManager.default.createDirectory(
                        at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                    try text.write(to: url, atomically: true, encoding: .utf8)
                    respond.resolve()
                } catch {
                    respond.reject("files.write could not write \(url.path): \(error)")
                }
            }

        case "remove":
            writablePath(args, respond) { url in
                do {
                    try FileManager.default.removeItem(at: url)
                    respond.resolve()
                } catch {
                    respond.reject("files.remove could not delete \(url.path): \(error)")
                }
            }

        case "makeDirectory":
            writablePath(args, respond) { url in
                do {
                    try FileManager.default.createDirectory(
                        at: url, withIntermediateDirectories: true)
                    respond.resolve()
                } catch {
                    respond.reject("files.makeDirectory could not create \(url.path): \(error)")
                }
            }

        case "list":
            resolvePath(args, respond) { url in
                self.reading(url) {
                    do {
                        let names = try FileManager.default.contentsOfDirectory(atPath: url.path)
                        respond.resolve(
                            names.sorted().map { Self.entry(at: url.appendingPathComponent($0)) })
                    } catch {
                        respond.reject("files.list could not read \(url.path): \(error)")
                    }
                }
            }

        case "pick":
            pick(args, respond)

        default:
            respond.reject("the files module has no method \(method)")
        }
    }

    // ── The app's two directories ──────────────────────────────────────────

    /// The one that lasts.
    ///
    /// On the Mac it is `~/Library/Application Support/<bundle id>` and not
    /// `~/Documents`: the second is the person's own folder, reaching it needs
    /// their consent through TCC, and an app writing its scratch files there
    /// would be an app misbehaving. On the phone, the headset and the watch it
    /// is the container's `Documents`, which is the same idea with the
    /// platform's name for it.
    private static func documents() -> AnFilePath {
        #if os(tvOS)
            // A television has nowhere to keep anything. tvOS gives an app no
            // persistent local storage: `Documents` is not writable, and what
            // there is —`Library/Caches`— the system empties whenever it needs
            // the room. Answering a path here would be answering a path that
            // loses its contents without warning.
            return .refused(
                "tvOS gives an app no storage that lasts: the system may empty the container "
                    + "at any time and there is no Documents directory to write to. Use "
                    + "files.cacheDirectory() and treat what is in it as something that can "
                    + "disappear, or keep the data on your server.")
        #elseif os(macOS)
            let support = FileManager.default.urls(
                for: .applicationSupportDirectory, in: .userDomainMask
            ).first
            guard let support else {
                return .refused("this Mac has no Application Support directory")
            }
            let bundle = Bundle.main.bundleIdentifier ?? "dev.angularnative.app"
            let directory = support.appendingPathComponent(bundle, isDirectory: true)
            do {
                try FileManager.default.createDirectory(
                    at: directory, withIntermediateDirectories: true)
            } catch {
                return .refused("the app's directory could not be created: \(error)")
            }
            return .at(directory.resolvingSymlinksInPath())
        #else
            guard
                let directory = FileManager.default.urls(
                    for: .documentDirectory, in: .userDomainMask
                ).first
            else {
                return .refused("this app has no Documents directory")
            }
            return .at(directory.resolvingSymlinksInPath())
        #endif
    }

    /// The one the system may empty.
    private static func caches() -> AnFilePath {
        guard
            let directory = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)
                .first
        else {
            return .refused("this app has no Caches directory")
        }
        #if os(macOS)
            let bundle = Bundle.main.bundleIdentifier ?? "dev.angularnative.app"
            let scoped = directory.appendingPathComponent(bundle, isDirectory: true)
            try? FileManager.default.createDirectory(at: scoped, withIntermediateDirectories: true)
            return .at(scoped.resolvingSymlinksInPath())
        #else
            return .at(directory.resolvingSymlinksInPath())
        #endif
    }

    // ── The rule ───────────────────────────────────────────────────────────

    /// A path that may be read: inside the app's directories, or something the
    /// person picked in this session.
    private func resolvePath(
        _ args: [String: Any], _ respond: AnBuiltinCall, _ body: (URL) -> Void
    ) {
        guard let asked = args["path"] as? String, !asked.isEmpty else {
            respond.reject("this files method needs a path in 'path'")
            return
        }
        if let picked = granted[asked] {
            body(picked)
            return
        }
        switch Self.resolve(asked) {
        case .at(let url): body(url)
        case .refused(let reason): respond.reject(reason)
        }
    }

    /// A path that may be written: inside the app's directories and nowhere
    /// else. A picked file is not one of them — the picker grants a read, and
    /// writing back to somebody's document without their having asked for it is
    /// not something the framework will do behind a `write`.
    private func writablePath(
        _ args: [String: Any], _ respond: AnBuiltinCall, _ body: (URL) -> Void
    ) {
        guard let asked = args["path"] as? String, !asked.isEmpty else {
            respond.reject("this files method needs a path in 'path'")
            return
        }
        if granted[asked] != nil {
            respond.reject(
                "files cannot write to \(asked): the system picker grants a read of what the "
                    + "person chose, not permission to change it. Write inside "
                    + "files.documentsDirectory() instead.")
            return
        }
        switch Self.resolve(asked) {
        case .at(let url): body(url)
        case .refused(let reason): respond.reject(reason)
        }
    }

    /// Turns what JS sent into a URL inside the container, or says why it is
    /// not one.
    private static func resolve(_ asked: String) -> AnFilePath {
        let roots = [documents(), caches()].compactMap(\.url)
        if roots.isEmpty {
            return .refused(
                "this platform gave the app no directory to work in: "
                    + (documents().reason ?? "no reason given"))
        }
        // A relative path belongs to the durable directory, which is what makes
        // `read({ path: 'notes.txt' })` mean something without the app having to
        // ask where that is first.
        let url: URL
        if asked.hasPrefix("/") {
            url = URL(fileURLWithPath: asked).standardizedFileURL
        } else {
            guard let base = roots.first else {
                return .refused("there is no directory to resolve \(asked) against")
            }
            url = base.appendingPathComponent(asked).standardizedFileURL
        }
        let resolved = url.resolvingSymlinksInPath().path
        for root in roots where resolved == root.path || resolved.hasPrefix(root.path + "/") {
            return .at(url)
        }
        return .refused(
            "files will not touch \(asked): it is outside the app's own directories and it is "
                + "not something the person picked. What the app owns is "
                + "files.documentsDirectory() and files.cacheDirectory(); anything else has to "
                + "come back from files.pick().")
    }

    /// Runs `body` with the security scope open if the URL needs one. A picked
    /// file is outside the container and unreadable without it.
    private func reading(_ url: URL, _ body: () -> Void) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped { url.stopAccessingSecurityScopedResource() }
        }
        body()
    }

    private static func entry(at url: URL) -> [String: Any] {
        let values = try? url.resourceValues(forKeys: [.fileSizeKey, .isDirectoryKey])
        return [
            "name": url.lastPathComponent,
            "path": url.path,
            "size": values?.fileSize ?? 0,
            "isDirectory": values?.isDirectory ?? false
        ]
    }

    // ── The system picker ──────────────────────────────────────────────────

    private func pick(_ args: [String: Any], _ respond: AnBuiltinCall) {
        #if os(iOS) || os(visionOS)
            guard let host = AnBuiltinHost.viewController else {
                respond.reject("files.pick has no screen to present the picker from")
                return
            }
            let types = (args["types"] as? [String] ?? []).compactMap { UTType($0) }
            let picker = UIDocumentPickerViewController(
                forOpeningContentTypes: types.isEmpty ? [.item] : types)
            picker.allowsMultipleSelection = args["multiple"] as? Bool ?? false
            let delegate = AnDocumentPickerDelegate { [weak self] urls in
                self?.pending = nil
                respond.resolve(urls.map { url -> [String: Any] in
                    var entry = Self.entry(at: url)
                    // The path handed to JS is what a later `read` will be
                    // matched against, so it is remembered exactly as it goes
                    // out.
                    self?.granted[url.path] = url
                    entry["path"] = url.path
                    return entry
                })
            }
            pending = delegate
            picker.delegate = delegate
            host.present(picker, animated: true)

        #elseif os(macOS)
            let panel = NSOpenPanel()
            panel.allowsMultipleSelection = args["multiple"] as? Bool ?? false
            panel.canChooseDirectories = false
            panel.canChooseFiles = true
            if let types = args["types"] as? [String] {
                let allowed = types.compactMap { UTType($0) }
                if !allowed.isEmpty { panel.allowedContentTypes = allowed }
            }
            let answer: (NSApplication.ModalResponse) -> Void = { [weak self] response in
                guard response == .OK else {
                    // Not a failure: somebody closed the panel. An empty list is
                    // the answer, and a `catch` firing here would make cancelling
                    // look like something breaking.
                    respond.resolve([Any]())
                    return
                }
                respond.resolve(panel.urls.map { url -> [String: Any] in
                    self?.granted[url.path] = url
                    return Self.entry(at: url)
                })
            }
            if let window = AnBuiltinHost.view?.window {
                panel.beginSheetModal(for: window, completionHandler: answer)
            } else {
                answer(panel.runModal())
            }

        #elseif os(tvOS)
            respond.reject(
                "tvOS has no file picker: there is no file system a person can browse on a "
                    + "television, and UIDocumentPickerViewController does not exist there. "
                    + "Read what the app itself wrote with files.read, or fetch it over the "
                    + "network.")

        #elseif os(watchOS)
            respond.reject(
                "watchOS has no file picker: the watch has no document browser and "
                    + "UIDocumentPickerViewController does not exist there. Read what the app "
                    + "itself wrote with files.read, or have the phone pick it and send it over.")

        #else
            respond.reject("this platform has no file picker")
        #endif
    }
}

#if os(iOS) || os(visionOS)
    /// The picker's delegate. It answers exactly once, whichever way the picker
    /// ends: a dismissal is an empty list and not a promise left hanging.
    private final class AnDocumentPickerDelegate: NSObject, UIDocumentPickerDelegate {
        private let answer: ([URL]) -> Void
        private var answered = false

        init(answer: @escaping ([URL]) -> Void) {
            self.answer = answer
        }

        func documentPicker(
            _ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]
        ) {
            once(urls)
        }

        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
            once([])
        }

        private func once(_ urls: [URL]) {
            guard !answered else { return }
            answered = true
            answer(urls)
        }
    }
#endif
