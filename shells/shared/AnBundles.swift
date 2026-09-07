import Foundation

/// Which JavaScript the app runs, and what to do when a new one is wrong.
///
/// The `.app` always carries a `main.js`, and that one can never fail: it was
/// there when the app was reviewed and installed. An update is a second bundle
/// written into the app's own support directory, and the whole difficulty is
/// that a bad one is only discovered *after* it has been loaded — by which
/// point, without care, the app cannot start well enough to fix itself.
///
/// So an installed update is on probation. It is loaded once marked `pending`;
/// if the JS reaches a point where it is plainly working and calls
/// `updater.notifyReady()`, it becomes `good` and is used from then on. If the
/// app starts again and finds a bundle still `pending`, that bundle crashed or
/// hung before it could confirm, and it is thrown away — the packaged one runs
/// instead.
///
/// That rule is the whole design. It costs one launch to recover from a broken
/// update and needs nothing from a server.
enum AnBundles {

    /// Where an update lives once installed. Application Support and not the
    /// caches directory: the system may empty caches whenever it likes, and an
    /// app whose JavaScript vanishes mid-flight is worse than one that is out
    /// of date.
    static var directory: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let ours = base.appendingPathComponent("angular-native/bundles", isDirectory: true)
        try? FileManager.default.createDirectory(at: ours, withIntermediateDirectories: true)
        return ours
    }

    private static var installed: URL { directory.appendingPathComponent("main.js") }
    private static var state: URL { directory.appendingPathComponent("state.json") }

    /// The source to evaluate at launch, and where it came from.
    ///
    /// Called once, before anything else: it is also what retires a bundle that
    /// did not confirm itself.
    static func source() -> (script: String, version: String) {
        let packaged = packagedSource()

        guard let details = read(), let version = details["version"] as? String else {
            return (packaged, "packaged")
        }
        switch details["status"] as? String {
        case "good":
            guard let script = try? String(contentsOf: installed, encoding: .utf8) else {
                // Marked good and no longer there: something outside the app
                // removed it. The packaged one is always a safe answer.
                discard()
                return (packaged, "packaged")
            }
            return (script, version)

        case "pending":
            // It was loaded once and never confirmed. That is what a bundle
            // that crashes on launch looks like from here, and one launch is
            // all it gets.
            NSLog("angular-native: update \(version) did not confirm itself; going back to the packaged bundle")
            discard()
            return (packaged, "packaged")

        default:
            // Freshly installed and never yet loaded: this launch is its trial.
            write(["version": version, "status": "pending"])
            guard let script = try? String(contentsOf: installed, encoding: .utf8) else {
                discard()
                return (packaged, "packaged")
            }
            return (script, version)
        }
    }

    /// The JS says it is working. Called by the updater plugin.
    static func confirm() {
        guard var details = read() else { return }
        guard (details["status"] as? String) != "good" else { return }
        details["status"] = "good"
        write(details)
    }

    /// Installs a downloaded bundle. It is not loaded until the next launch:
    /// swapping the JavaScript under a running app would leave the native views
    /// on screen belonging to a tree nothing remembers building.
    static func install(from file: URL, version: String) throws {
        try? FileManager.default.removeItem(at: installed)
        try FileManager.default.moveItem(at: file, to: installed)
        write(["version": version, "status": "fresh"])
    }

    /// Throws the update away and goes back to what the `.app` carries.
    static func discard() {
        try? FileManager.default.removeItem(at: installed)
        try? FileManager.default.removeItem(at: state)
    }

    /// What is installed right now, whatever its state.
    static func current() -> [String: Any] {
        guard let details = read() else {
            return ["version": "packaged", "status": "packaged"]
        }
        return details
    }

    // ------------------------------------------------------------- privates

    private static func packagedSource() -> String {
        guard let path = Bundle.main.path(forResource: "main", ofType: "js"),
            let source = try? String(contentsOfFile: path, encoding: .utf8)
        else {
            NSLog("angular-native: there is no main.js in the .app")
            return ""
        }
        return source
    }

    private static func read() -> [String: Any]? {
        guard let data = try? Data(contentsOf: state),
            let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return parsed
    }

    private static func write(_ details: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: details) else { return }
        try? data.write(to: state, options: .atomic)
    }
}
