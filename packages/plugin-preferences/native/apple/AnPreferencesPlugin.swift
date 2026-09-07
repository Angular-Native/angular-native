import Foundation

/// Preferences on every Apple platform, out of one file.
///
/// iOS, macOS and watchOS get the same source, which is unusual here — the
/// clipboard needs one file each because `UIPasteboard` and `NSPasteboard`
/// behave differently. `UserDefaults` is Foundation, identical on all three, so
/// three copies would only be three places for the same code to drift.
///
/// The one line of the plugin protocol that differs between the platforms is
/// `attach`, which takes a `UIViewController` on the phone and an
/// `NSViewController` on the Mac. This plugin has nothing to present, so it
/// implements neither and takes the protocol's default.
final class AnPreferencesPlugin: AnPlugin {

    /// A suite of its own rather than `UserDefaults.standard`.
    ///
    /// Standard defaults are shared with everything else the app and the system
    /// put there — the scroll positions UIKit remembers, whatever a framework
    /// decided to cache. Writing into that would mean `keys()` returning things
    /// this app never wrote, and `clear()` deleting them. A suite is a separate
    /// plist inside the app's own container: no entitlement, no app group, and
    /// nothing in it that did not come through here.
    private let store = UserDefaults(suiteName: "dev.angularnative.preferences")

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        guard let store else {
            // `UserDefaults(suiteName:)` returns nil for a name that collides
            // with the app's own bundle identifier. It cannot happen with this
            // one, and it is still answered rather than left hanging.
            respond.reject("the preferences plugin could not open its store")
            return
        }

        switch method {
        case "get":
            guard let key = args["key"] as? String, !key.isEmpty else {
                respond.reject("preferences.get needs a key in 'key'")
                return
            }
            // `object(forKey:)` and not `string(forKey:)`: the string form
            // coerces numbers and booleans into text, which would quietly hand
            // back "1" for something this plugin never wrote as a string.
            if let value = store.object(forKey: key) as? String {
                respond.resolve(value)
            } else {
                respond.resolve()
            }

        case "set":
            guard let key = args["key"] as? String, !key.isEmpty else {
                respond.reject("preferences.set needs a key in 'key'")
                return
            }
            guard let value = args["value"] as? String else {
                respond.reject("preferences.set needs a string in 'value'")
                return
            }
            store.set(value, forKey: key)
            respond.resolve()

        case "remove":
            guard let key = args["key"] as? String, !key.isEmpty else {
                respond.reject("preferences.remove needs a key in 'key'")
                return
            }
            // Removing something that was never there is not an error: the
            // caller wanted it gone, and it is gone.
            store.removeObject(forKey: key)
            respond.resolve()

        case "keys":
            respond.resolve(Array(store.dictionaryRepresentation().keys))

        case "clear":
            for key in store.dictionaryRepresentation().keys {
                store.removeObject(forKey: key)
            }
            respond.resolve()

        default:
            respond.reject("the preferences plugin has no method \(method)")
        }
    }
}
