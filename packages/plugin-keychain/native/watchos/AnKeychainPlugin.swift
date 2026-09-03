import Foundation
import Security

/// The watch's keychain: Keychain Services, the same API as the phone's.
///
/// This is the one plugin of the three that a watch can genuinely run, and it is
/// worth saying why, because the other two look just as plausible from a
/// distance. A watch has no pasteboard —`UIPasteboard` is
/// `API_UNAVAILABLE(watchos)`— and no biometric sensor. What it does have is
/// Security.framework, whole: `SecItemAdd`, `SecItemCopyMatching`,
/// `SecItemDelete`, a Secure Enclave underneath and only one keychain, the
/// data-protection one. Nothing here is a stand-in for something else.
///
/// Each secret is a `kSecClassGenericPassword` with the bundle identifier as its
/// service and the app's key as its account, exactly as on iOS. `…ThisDeviceOnly`
/// for the same reason too: a session token that travelled in the backup and
/// turned up on the next watch would be a decision nobody wrote down.
///
/// **What is missing, and it is missing out loud.** `requireBiometrics` cannot be
/// honoured here and it is refused rather than downgraded. watchOS does have
/// `kSecAccessControlUserPresence`, and on a watch that resolves to the passcode
/// or to wrist detection — a real check, and not the one the caller asked for.
/// Storing an item behind the passcode when the app asked for a fingerprint would
/// leave it believing the secret is behind biometry, which is the one mistake
/// nobody can see from the outside. See `set`.
final class AnKeychainPlugin: AnPlugin {

    /// The service the items are stored under.
    ///
    /// Taken from the bundle identifier, which is unique per app. If it were ever
    /// missing, a fixed name is used: sharing a service with another
    /// angular-native app would be worse than storing nothing, so it says so.
    private static var service: String {
        guard let identifier = Bundle.main.bundleIdentifier else {
            NSLog("angular-native: the bundle has no identifier; the keychain will use a fixed name")
            return "dev.angularnative.keychain"
        }
        return identifier
    }

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        // `backing` is the only one that does not need a key.
        if method == "backing" {
            respond.resolve(Self.backing())
            return
        }
        guard let key = args["key"] as? String, !key.isEmpty else {
            respond.reject("keychain.\(method) needs a non-empty key in 'key'")
            return
        }

        switch method {
        case "set":
            guard let value = args["value"] as? String else {
                respond.reject("keychain.set needs the secret in 'value'")
                return
            }
            set(
                key: key,
                value: value,
                biometrics: args["requireBiometrics"] as? Bool ?? false,
                respond: respond)

        case "get":
            get(key: key, respond: respond)

        case "has":
            respond.resolve(Self.has(key))

        case "remove":
            respond.resolve(Self.remove(key))

        default:
            respond.reject("the keychain plugin has no method \(method)")
        }
    }

    // MARK: - Storing

    /// Off the main thread, because `SecItemAdd` can take a while and the shell's
    /// timer is what draws the watch's frames.
    private func set(key: String, value: String, biometrics: Bool, respond: AnPluginCall) {
        // The refusal comes first, before anything is written and before the
        // thread hop, so that nothing at all happened when it is turned down.
        if biometrics {
            respond.resolve([
                "outcome": "unavailable",
                "detail": "requireBiometrics cannot be honoured on watchOS: a watch has no "
                    + "biometric sensor, and LocalAuthentication marks "
                    + "deviceOwnerAuthenticationWithBiometrics API_UNAVAILABLE(watchos). "
                    + "Nothing was stored. What the watch does protect an item with is its own "
                    + "passcode, which is what kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly "
                    + "already relies on: store it without requireBiometrics and it is behind "
                    + "the unlocked watch."
            ])
            return
        }
        DispatchQueue.global(qos: .userInitiated).async {
            // Replacing means deleting and adding again, the same as on the
            // phone: `SecItemUpdate` cannot change an item's protection, so
            // writing over one would end up half done.
            _ = Self.remove(key)

            let attributes: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: Self.service,
                kSecAttrAccount as String: key,
                kSecValueData as String: Data(value.utf8),
                // A watch is unlocked on the wrist and locks itself the moment it
                // comes off, so "after first unlock" is a much shorter window
                // here than the same constant means on a phone left on a desk.
                kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            ]
            let status = SecItemAdd(attributes as CFDictionary, nil)
            if status == errSecSuccess {
                respond.resolve(["outcome": "saved", "detail": "SecItemAdd"])
                return
            }
            let outcome = status == errSecAuthFailed ? "denied" : "unavailable"
            respond.resolve(["outcome": outcome, "detail": Self.describe(status)])
        }
    }

    // MARK: - Reading

    /// No `LAContext` and no prompt: nothing stored by this half is behind an
    /// access control, so nothing here can put a dialog on the watch's screen.
    /// The `reason` the phone half takes is accepted by the TypeScript and
    /// ignored here, which is the truth — there is no dialog for it to appear in.
    private func get(key: String, respond: AnPluginCall) {
        DispatchQueue.global(qos: .userInitiated).async {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: Self.service,
                kSecAttrAccount as String: key,
                kSecReturnData as String: true,
                kSecMatchLimit as String: kSecMatchLimitOne
            ]
            var item: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &item)
            switch status {
            case errSecSuccess:
                guard let data = item as? Data, let text = String(data: data, encoding: .utf8)
                else {
                    respond.resolve([
                        "outcome": "unavailable", "value": NSNull(),
                        "detail": "the keychain returned something that is not UTF-8 text"
                    ])
                    return
                }
                respond.resolve(["outcome": "found", "value": text, "detail": "SecItemCopyMatching"])

            case errSecItemNotFound:
                respond.resolve([
                    "outcome": "notFound", "value": NSNull(), "detail": Self.describe(status)
                ])

            // `interactionNotAllowed` is the one that really happens on a watch:
            // it is what comes back when the app is woken in the background —a
            // complication refreshing, a notification— and the watch is locked
            // on the charger. It is `denied` and not `unavailable`: trying again
            // once the watch is on the wrist works.
            case errSecUserCanceled, errSecAuthFailed, errSecInteractionNotAllowed:
                respond.resolve([
                    "outcome": "denied", "value": NSNull(), "detail": Self.describe(status)
                ])

            default:
                respond.resolve([
                    "outcome": "unavailable", "value": NSNull(), "detail": Self.describe(status)
                ])
            }
        }
    }

    // MARK: - Looking and deleting

    /// Whether there is anything, without opening it: the attributes are asked
    /// for and the data is not.
    private static func has(_ key: String) -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        return SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
    }

    private static func remove(_ key: String) -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        return SecItemDelete(query as CFDictionary) == errSecSuccess
    }

    // MARK: - What is underneath

    private static func backing() -> [String: Any] {
        [
            "platform": "watchos",
            // Every Apple Watch that runs watchOS 11 has a Secure Enclave, and
            // it holds the class keys the keychain is encrypted with. The watch
            // has no file keychain to fall back to, so unlike the Mac there is
            // nothing to probe: if Keychain Services answered, this is what
            // answered.
            "hardwareBacked": true,
            "accessible": "kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly",
            "detail": "Keychain Services, service \(service). No item here is behind biometrics: "
                + "a watch has no sensor, and set with requireBiometrics is refused rather than "
                + "stored behind the passcode instead."
        ]
    }

    /// The `OSStatus` code with its description, for the log.
    private static func describe(_ status: OSStatus) -> String {
        let text = SecCopyErrorMessageString(status, nil) as String? ?? "no description"
        return "OSStatus \(status): \(text)"
    }
}
