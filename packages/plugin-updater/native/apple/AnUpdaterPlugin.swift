import CryptoKit
import Foundation

/// Shipping new JavaScript without a store release.
///
/// The download and the checksum live here; which bundle actually runs, and the
/// rule that retires one that never confirmed itself, live in `AnBundles` in the
/// shell — because that decision has to be made before any plugin exists, at the
/// moment the runtime is handed its source.
///
/// One file for iOS and macOS: `URLSession`, `FileManager` and `CryptoKit` are
/// the same on both, and this plugin presents nothing, so it needs neither
/// platform's `attach`.
final class AnUpdaterPlugin: AnPlugin {

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "current":
            respond.resolve(AnBundles.current())

        case "notifyReady":
            // Doing nothing when the packaged bundle is running is deliberate:
            // an app should be able to call this unconditionally at startup
            // without knowing or caring what it is running.
            AnBundles.confirm()
            respond.resolve()

        case "reset":
            AnBundles.discard()
            respond.resolve()

        case "download":
            download(args, respond)

        default:
            respond.reject("the updater plugin has no method \(method)")
        }
    }

    private func download(_ args: [String: Any], _ respond: AnPluginCall) {
        guard let address = args["url"] as? String, let url = URL(string: address) else {
            respond.reject("updater.download needs a url in 'url'")
            return
        }
        guard url.scheme == "https" else {
            // Not pedantry. This file becomes the code the app runs; over http
            // anyone on the path chooses what that code is.
            respond.reject("updater.download refuses \(url.scheme ?? "that scheme"): a bundle must come over https")
            return
        }
        guard let version = args["version"] as? String, !version.isEmpty else {
            respond.reject("updater.download needs a version in 'version'")
            return
        }
        let expected = (args["sha256"] as? String)?.lowercased()

        let task = URLSession.shared.downloadTask(with: url) { file, response, error in
            if let error {
                respond.reject("updater.download failed: \(error.localizedDescription)")
                return
            }
            guard let file else {
                respond.reject("updater.download got no file back")
                return
            }
            if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                respond.reject("updater.download got HTTP \(http.statusCode)")
                return
            }
            guard let data = try? Data(contentsOf: file) else {
                respond.reject("updater.download could not read what it downloaded")
                return
            }
            if let expected {
                let actual = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
                guard actual == expected else {
                    // Nothing is installed. A mismatch is either a corrupted
                    // download or somebody else's file, and there is no version
                    // of this where running it is the right answer.
                    respond.reject(
                        "updater.download: the bundle does not match its sha256 — expected \(expected), got \(actual)")
                    return
                }
            }
            // Written beside the destination and moved, so a bundle is never
            // half-written when the app is killed mid-download.
            let staging = AnBundles.directory.appendingPathComponent("staging-\(UUID().uuidString).js")
            do {
                try data.write(to: staging, options: .atomic)
                try AnBundles.install(from: staging, version: version)
            } catch {
                try? FileManager.default.removeItem(at: staging)
                respond.reject("updater.download could not install the bundle: \(error.localizedDescription)")
                return
            }
            respond.resolve()
        }
        task.resume()
    }
}
