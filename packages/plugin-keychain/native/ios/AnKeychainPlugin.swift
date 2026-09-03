import Foundation
import LocalAuthentication
import Security

/// The iOS keychain: Keychain Services.
///
/// Each secret is a `kSecClassGenericPassword` with the bundle identifier as its
/// service and the app's key as its account. The service is not decorative: it
/// is what separates this app's secrets from those of any other app using the
/// same keychain.
///
/// Two things that cannot be seen in the code and rule over everything else:
///
///   · **`…ThisDeviceOnly`.** Without that suffix, a keychain item travels in
///     the encrypted backup and turns up on the new phone. With it, it does not
///     leave this device. For a session token that is what you want; anyone who
///     wanted the opposite would have to make that a written decision, not the
///     default.
///
///   · **`.biometryCurrentSet`.** It binds the item to the set of faces and
///     fingerprints enrolled at the moment it is saved. If another face is added
///     tomorrow, the system throws the key away and the secret can no longer be
///     read. Without that, whoever could enrol their own fingerprint —somebody
///     who knows the device passcode— would have the secret too.
final class AnKeychainPlugin: AnPlugin {

    /// The service the items are stored under.
    ///
    /// Taken from the bundle identifier, which is unique per app. If it were
    /// ever missing, a fixed name is used: sharing a service with another
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
            let biometrics = args["requireBiometrics"] as? Bool ?? false
            if biometrics && (args["reason"] as? String ?? "").isEmpty {
                respond.reject(
                    "keychain.set with requireBiometrics needs a 'reason': it is what the "
                        + "system shows inside the dialog when reading it back")
                return
            }
            set(key: key, value: value, biometrics: biometrics, respond: respond)

        case "get":
            get(key: key, reason: args["reason"] as? String, respond: respond)

        case "has":
            respond.resolve(Self.has(key))

        case "remove":
            respond.resolve(Self.remove(key))

        default:
            respond.reject("the keychain plugin has no method \(method)")
        }
    }

    // MARK: - Storing

    /// Storing never asks anything on iOS, not even with biometrics: the dialog
    /// is shown by the read. Even so it is done off the main thread, because
    /// `SecItemAdd` with an access control can take a while.
    private func set(key: String, value: String, biometrics: Bool, respond: AnPluginCall) {
        DispatchQueue.global(qos: .userInitiated).async {
            // Replacing means deleting and adding again. `SecItemUpdate` cannot
            // change the access control of an item that already exists, so
            // storing an item with biometrics over one without them would end up
            // half done: the new value with the old protection.
            _ = Self.remove(key)

            var attributes: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: Self.service,
                kSecAttrAccount as String: key,
                kSecValueData as String: Data(value.utf8)
            ]
            if biometrics {
                var error: Unmanaged<CFError>?
                guard
                    let control = SecAccessControlCreateWithFlags(
                        nil,
                        // With a device passcode required: without a passcode
                        // there is no biometry worth anything, and an item that
                        // can be read on a phone with no lock protects nothing.
                        kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                        .biometryCurrentSet,
                        &error)
                else {
                    let detail = (error?.takeRetainedValue()).map { "\($0)" } ?? "no reason given"
                    respond.resolve([
                        "outcome": "unavailable",
                        "detail": "the access control could not be created: \(detail)"
                    ])
                    return
                }
                attributes[kSecAttrAccessControl as String] = control
            } else {
                attributes[kSecAttrAccessible as String] =
                    kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            }

            let status = SecItemAdd(attributes as CFDictionary, nil)
            if status == errSecSuccess {
                respond.resolve(["outcome": "saved", "detail": "SecItemAdd"])
                return
            }
            // errSecAuthFailed here means the device has no passcode: the access
            // control demands one.
            let outcome = status == errSecAuthFailed ? "denied" : "unavailable"
            respond.resolve(["outcome": outcome, "detail": Self.describe(status)])
        }
    }

    // MARK: - Reading

    /// Reading an item with biometrics **blocks the thread while the dialog is
    /// on screen**. If that happened on the main thread, the app would be frozen
    /// behind the system's own dialog and the core would not advance a single
    /// frame. Hence the background queue, which is also the reason this method
    /// does not answer on the spot.
    private func get(key: String, reason: String?, respond: AnPluginCall) {
        DispatchQueue.global(qos: .userInitiated).async {
            var query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: Self.service,
                kSecAttrAccount as String: key,
                kSecReturnData as String: true,
                kSecMatchLimit as String: kSecMatchLimitOne
            ]
            if let reason, !reason.isEmpty {
                query[kSecUseOperationPrompt as String] = reason
            }
            // No "Enter passcode" button: whoever wants that door should ask for
            // it themselves with the biometrics plugin. Here, if the item was
            // stored with biometrics, it is read with biometrics.
            let context = LAContext()
            context.localizedFallbackTitle = ""
            query[kSecUseAuthenticationContext as String] = context

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
                // This is also what comes out when the item was bound to a set of
                // fingerprints that no longer exists: iOS deletes it rather than
                // leaving it useless. That is why `invalidated` never arrives
                // from iOS.
                respond.resolve([
                    "outcome": "notFound", "value": NSNull(), "detail": Self.describe(status)
                ])

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

    /// Whether there is anything, without opening it.
    ///
    /// The attributes are asked for and the data is **not**. That is what keeps
    /// this from throwing up a Face ID: authentication is demanded by reading
    /// the contents, not by knowing the item exists.
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

    /// Deleting does not authenticate either: the keychain lets you throw away
    /// an item that cannot be read, and just as well, or a secret bound to a
    /// deleted fingerprint would sit there forever.
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
            "platform": "ios",
            // Every iPhone that gets as far as iOS 17 has a Secure Enclave, and
            // it is the one holding the class keys the keychain is encrypted
            // with.
            "hardwareBacked": true,
            "accessible": "kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly",
            "detail": "Keychain Services, service \(service)"
        ]
    }

    /// The `OSStatus` code with its description, for the log.
    private static func describe(_ status: OSStatus) -> String {
        let text = SecCopyErrorMessageString(status, nil) as String? ?? "no description"
        return "OSStatus \(status): \(text)"
    }
}
