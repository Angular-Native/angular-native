import Foundation
import LocalAuthentication
import Security

/// The Mac's keychain: Keychain Services, the data-protection one when the app
/// is allowed to reach it.
///
/// Each secret is a `kSecClassGenericPassword` with the bundle identifier as its
/// service and the app's key as its account, exactly as on iOS.
///
/// **A Mac has two keychains, and that is the whole reason this file is not the
/// iOS one.** The old file keychain —`login.keychain-db`, the one Keychain
/// Access shows— is per user and shared by every app that asks nicely. The
/// data-protection keychain is the iPhone's, per app, and it is the only one
/// that understands `kSecAttrAccessible`, `…ThisDeviceOnly` and an access
/// control bound to Touch ID. iOS has only the second, so `an-ios`'s half never
/// has to choose.
///
/// Reaching the data-protection keychain needs the `keychain-access-groups`
/// entitlement. This plugin declares it in its `package.json` and `an` merges it
/// into the entitlements the `.app` is signed with, so an ordinary build has it.
/// What cannot be promised is that the *system* will honour it: an ad-hoc
/// signature carries no team, and `SecItemAdd` answers `errSecMissingEntitlement`
/// (−34018) when it will not play along. That is not a state to guess at, so
/// [`store`] probes it once, for real, and everything afterwards knows which
/// keychain it is talking to — and `backing()` says which, out loud, rather than
/// letting an app believe it has hardware protection it does not have.
final class AnKeychainPlugin: AnPlugin {

    /// Which keychain this `.app` actually got.
    private enum Store {
        /// The iPhone's keychain: per app, `kSecAttrAccessible` honoured, an
        /// access control can demand Touch ID.
        case dataProtection
        /// The old file keychain. It stores and it reads; what it cannot do is
        /// bind an item to the biometrics, and `kSecAttrAccessible` is ignored.
        case file(reason: String)
    }

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

    /// Which keychain answers, worked out once by actually writing to it.
    ///
    /// Asking `SecTaskCopyValueForEntitlement` would tell us what the signature
    /// claims, which is not the same question: what matters is whether securityd
    /// accepts the claim. So a throwaway item is added and deleted. It costs one
    /// round trip in the life of the process and it is the only answer that
    /// cannot be wrong.
    private static let store: Store = {
        let probe: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecUseDataProtectionKeychain as String: true,
            kSecAttrService as String: service,
            kSecAttrAccount as String: "dev.angularnative.probe",
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData as String: Data("probe".utf8)
        ]
        SecItemDelete(probe as CFDictionary)
        let status = SecItemAdd(probe as CFDictionary, nil)
        SecItemDelete(probe as CFDictionary)
        switch status {
        case errSecSuccess, errSecDuplicateItem:
            return .dataProtection
        default:
            let reason = describe(status)
            NSLog("angular-native: the data-protection keychain turned this app away (\(reason)); "
                + "falling back to the file keychain, where an item cannot be bound to Touch ID")
            return .file(reason: reason)
        }
    }()

    /// The keys every query carries, whichever keychain answered.
    private static var base: [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service
        ]
        if case .dataProtection = store {
            query[kSecUseDataProtectionKeychain as String] = true
        }
        return query
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

    /// Storing never asks anything, not even with biometrics: the dialog is shown
    /// by the read. Even so it is done off the main thread, because `SecItemAdd`
    /// with an access control can take a while.
    private func set(key: String, value: String, biometrics: Bool, respond: AnPluginCall) {
        DispatchQueue.global(qos: .userInitiated).async {
            // The refusal that has to come before anything is written. On the
            // file keychain there is no way to bind an item to Touch ID, and
            // storing it unprotected instead would leave an app believing its
            // secret is behind a fingerprint when it is behind nothing at all.
            if biometrics, case .file(let reason) = Self.store {
                respond.resolve([
                    "outcome": "unavailable",
                    "detail": "requireBiometrics needs the data-protection keychain, and this "
                        + ".app was turned away from it (\(reason)). Nothing was stored: an item "
                        + "saved in the file keychain would not be behind Touch ID. Sign the app "
                        + "with an identity that carries the keychain-access-groups entitlement."
                ])
                return
            }

            // Replacing means deleting and adding again. `SecItemUpdate` cannot
            // change the access control of an item that already exists, so
            // storing an item with biometrics over one without them would end up
            // half done: the new value with the old protection.
            _ = Self.remove(key)

            var attributes = Self.base
            attributes[kSecAttrAccount as String] = key
            attributes[kSecValueData as String] = Data(value.utf8)
            if biometrics {
                var error: Unmanaged<CFError>?
                guard
                    let control = SecAccessControlCreateWithFlags(
                        nil,
                        // With a device passcode required: without a passcode
                        // there is no biometry worth anything, and an item that
                        // can be read on a Mac with no password protects nothing.
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
            } else if case .dataProtection = Self.store {
                // Only the data-protection keychain honours this. Sending it to
                // the file one is not an error, but it is not a promise either,
                // and `backing()` is where that difference is stated.
                attributes[kSecAttrAccessible as String] =
                    kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            }

            let status = SecItemAdd(attributes as CFDictionary, nil)
            if status == errSecSuccess {
                respond.resolve(["outcome": "saved", "detail": "SecItemAdd"])
                return
            }
            // errSecAuthFailed here means the Mac has no password set: the access
            // control demands one.
            let outcome = status == errSecAuthFailed ? "denied" : "unavailable"
            respond.resolve(["outcome": outcome, "detail": Self.describe(status)])
        }
    }

    // MARK: - Reading

    /// Reading an item with biometrics **blocks the thread while the dialog is on
    /// screen**. If that happened on the main thread, the app would be frozen
    /// behind the system's own dialog and the core would not advance a single
    /// frame — on a Mac that is a beachball on top of a window that is still
    /// showing. Hence the background queue, which is also the reason this method
    /// does not answer on the spot.
    private func get(key: String, reason: String?, respond: AnPluginCall) {
        DispatchQueue.global(qos: .userInitiated).async {
            var query = Self.base
            query[kSecAttrAccount as String] = key
            query[kSecReturnData as String] = true
            query[kSecMatchLimit as String] = kSecMatchLimitOne
            // No "Enter password" button: whoever wants that door should ask for
            // it themselves with the biometrics plugin. Here, if the item was
            // stored behind Touch ID, it is read with Touch ID.
            let context = LAContext()
            context.localizedFallbackTitle = ""
            if let reason, !reason.isEmpty {
                context.localizedReason = reason
            }
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
                // fingerprints that no longer exists: the system deletes it
                // rather than leaving it useless. That is why `invalidated` never
                // arrives from here either.
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
    /// this from throwing up a Touch ID prompt: authentication is demanded by
    /// reading the contents, not by knowing the item exists.
    private static func has(_ key: String) -> Bool {
        var query = base
        query[kSecAttrAccount as String] = key
        query[kSecReturnAttributes as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        return SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
    }

    /// Deleting does not authenticate either: the keychain lets you throw away an
    /// item that cannot be read, and just as well, or a secret bound to a deleted
    /// fingerprint would sit there forever.
    private static func remove(_ key: String) -> Bool {
        var query = base
        query[kSecAttrAccount as String] = key
        return SecItemDelete(query as CFDictionary) == errSecSuccess
    }

    // MARK: - What is underneath

    /// Which keychain is answering and what that is worth.
    ///
    /// This is the method that earns its keep on the Mac and barely does on the
    /// phone, where the answer is always the same. Here it is the difference
    /// between a secret in the Secure Enclave's keychain and one in a file the
    /// user's login password opens, and an app that has to decide whether to keep
    /// a session token at all has no other way of finding out.
    private static func backing() -> [String: Any] {
        switch store {
        case .dataProtection:
            return [
                "platform": "macos",
                // Every Mac this can run on is Apple silicon —the host is built
                // for aarch64-apple-darwin— and every one of those has a Secure
                // Enclave holding the class keys the data-protection keychain is
                // encrypted with.
                "hardwareBacked": true,
                "accessible": "kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly",
                "detail": "the data-protection keychain, service \(service)"
            ]
        case .file(let reason):
            return [
                "platform": "macos",
                // Not a guess and not a hedge: the file keychain is unlocked by
                // the login password and its contents are not sealed to this
                // machine's Secure Enclave.
                "hardwareBacked": false,
                "accessible": "not honoured by the file keychain",
                "detail": "the file keychain, service \(service). The data-protection one turned "
                    + "this app away (\(reason)), so kSecAttrAccessible is ignored and an item "
                    + "cannot be bound to Touch ID."
            ]
        }
    }

    /// The `OSStatus` code with its description, for the log.
    private static func describe(_ status: OSStatus) -> String {
        let text = SecCopyErrorMessageString(status, nil) as String? ?? "no description"
        return "OSStatus \(status): \(text)"
    }
}
