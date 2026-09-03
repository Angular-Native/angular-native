import AppKit
import LocalAuthentication

/// The Mac's biometrics: `LAContext`, the same class the phone uses.
///
/// The dialog is presented by the system on top of the app, so there is no view
/// to mount here and no `NSViewController` to hang anything off: that is why this
/// plugin does not implement `attach`.
///
/// `evaluatePolicy` answers on a queue of its own, not on the main one. It does
/// not matter: `AnPluginCall` talks to a mailbox with a lock on the other side,
/// not to a variable belonging to the UI thread, so the answer can be given from
/// wherever it arrives.
///
/// What is different from the phone, and none of it is cosmetic:
///
///   · **There is no `NSFaceIDUsageDescription`.** No Mac has Face ID, and Touch
///     ID needs no usage string. The iOS half declares that key in its
///     `package.json` because iOS kills the process without it; the macOS
///     section declares none, because there is none to declare. A key copied
///     over "to be safe" would be a key nobody can explain.
///
///   · **A Mac can have no sensor at all and still authenticate**, through a
///     paired Apple Watch. That is `deviceOwnerAuthenticationWithCompanion`,
///     macOS 15 and later. It is not offered by default: it answers a different
///     question —"is the owner nearby and unlocked"— and an app that asked for
///     biometrics should not silently get that instead. It is reached with
///     `allowCompanion`, which the phone has no equivalent of and which is
///     documented as macOS-only in the TypeScript.
///
///   · **`biometryLockout` can mean the lid is shut.** On a Mac with the sensor
///     in the keyboard, closing the lid or using an external keyboard takes
///     Touch ID away mid-session. It arrives as the same `lockedOut` the phone
///     has and the `detail` is what tells them apart.
final class AnBiometricsPlugin: AnPlugin {

    /// The context of the evaluation under way.
    ///
    /// It has to be kept: `evaluatePolicy` is asynchronous and if the `LAContext`
    /// is released before it answers, the dialog closes by itself and the call
    /// never comes back. A new one per evaluation, on top of that, because a
    /// context remembers that it already authenticated and reusing it would
    /// return `success` without showing anything.
    private var pending: LAContext?

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "availability":
            respond.resolve(Self.availability())

        case "authenticate":
            guard let reason = args["reason"] as? String, !reason.isEmpty else {
                respond.reject("biometrics.authenticate needs a non-empty 'reason': it is what "
                    + "the system shows inside the dialog")
                return
            }
            authenticate(
                reason: reason,
                cancelTitle: args["cancelTitle"] as? String,
                allowDeviceCredential: args["allowDeviceCredential"] as? Bool ?? false,
                allowCompanion: args["allowCompanion"] as? Bool ?? false,
                respond: respond)

        default:
            respond.reject("the biometrics plugin has no method \(method)")
        }
    }

    // MARK: - Availability

    /// What there is and whether it can be used, without showing anything.
    ///
    /// `biometryType` is worthless until after `canEvaluatePolicy`: on a freshly
    /// created `LAContext` it is always `.none`, even on a Mac with Touch ID.
    /// That is why the question is always asked, even when only the type is of
    /// interest.
    private static func availability() -> [String: Any] {
        let context = LAContext()
        var error: NSError?
        let allowed = context.canEvaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics, error: &error)
        let kind = describe(context.biometryType)
        if allowed {
            return ["status": "available", "kind": kind, "detail": "LAContext.canEvaluatePolicy"]
        }
        guard let error else {
            // `canEvaluatePolicy` promises to fill the error in when it returns
            // false. If one day it does not, it is said so; answering `available`
            // here would mean showing a button that does not work.
            return [
                "status": "unavailable", "kind": kind,
                "detail": "canEvaluatePolicy said no and did not say why"
            ]
        }
        let (status, detail) = translate(error)
        return ["status": status, "kind": kind, "detail": detail]
    }

    private static func describe(_ type: LABiometryType) -> String {
        switch type {
        case .faceID: return "faceId"
        case .touchID: return "touchId"
        case .opticID: return "opticId"
        case .none: return "none"
        // `LABiometryType` can grow along with the system, and a version of
        // macOS newer than this code would bring a type that is not here. Saying
        // `unknown` is true; saying `none` would be saying there is no sensor.
        @unknown default: return "unknown"
        }
    }

    // MARK: - Authentication

    private func authenticate(
        reason: String,
        cancelTitle: String?,
        allowDeviceCredential: Bool,
        allowCompanion: Bool,
        respond: AnPluginCall
    ) {
        let context = LAContext()
        if let cancelTitle, !cancelTitle.isEmpty {
            context.localizedCancelTitle = cancelTitle
        }
        // An empty string in `localizedFallbackTitle` takes the "Enter password"
        // button away. Without this macOS shows it anyway, the user presses it
        // and the call comes back with `userFallback` without the app ever
        // having asked for that door.
        if !allowDeviceCredential {
            context.localizedFallbackTitle = ""
        }
        let policy = Self.policy(
            allowDeviceCredential: allowDeviceCredential, allowCompanion: allowCompanion)

        // The type is read before evaluating, while the context is still alive
        // and has already resolved the policy: once the answer is given there is
        // nobody left to ask.
        var probe: NSError?
        let allowed = context.canEvaluatePolicy(policy, error: &probe)
        let kind = Self.describe(context.biometryType)
        if !allowed {
            // No sensor, nothing enrolled, no watch nearby or locked out: the
            // reason is given back instead of showing a dialog the system is
            // going to close by itself.
            let (status, detail) = Self.translate(probe)
            respond.resolve(["outcome": status, "kind": kind, "detail": detail])
            return
        }

        pending = context
        context.evaluatePolicy(policy, localizedReason: reason) { [weak self] ok, error in
            self?.pending = nil
            if ok {
                respond.resolve(["outcome": "success", "kind": kind, "detail": "evaluatePolicy"])
                return
            }
            let (outcome, detail) = Self.translate(error as NSError?)
            respond.resolve(["outcome": outcome, "kind": kind, "detail": detail])
        }
    }

    /// Which policy the two flags add up to.
    ///
    /// `allowCompanion` only exists from macOS 15; on 14 the constant is not
    /// there to be named. Asking for it on an older system is not an error and
    /// not a silent downgrade either — the request falls back to the nearest
    /// policy that does exist, and `detail` in the answer carries the name of the
    /// one that actually ran.
    private static func policy(allowDeviceCredential: Bool, allowCompanion: Bool) -> LAPolicy {
        if allowCompanion, #available(macOS 15.0, *) {
            return allowDeviceCredential
                ? .deviceOwnerAuthentication
                : .deviceOwnerAuthenticationWithBiometricsOrCompanion
        }
        return allowDeviceCredential
            ? .deviceOwnerAuthentication
            : .deviceOwnerAuthenticationWithBiometrics
    }

    // MARK: - Errors

    /// From `LAError` to the names in the contract.
    ///
    /// Every branch is a different situation for whoever is using the app, and
    /// that is why they are not folded together: `userCancel` does not deserve so
    /// much as a warning, `biometryNotEnrolled` deserves a link to System
    /// Settings and `biometryLockout` deserves offering the password.
    ///
    /// macOS does not tell the temporary lockout from the permanent one
    /// —`biometryLockout` is the only code there is— so `permanentlyLockedOut`
    /// never comes out of here, exactly as on iOS. Faking the second state by
    /// counting attempts would be making it up.
    ///
    /// The three codes at the bottom are the Mac's own and have no phone
    /// equivalent: a Touch ID keyboard that is not paired, one that has been
    /// unplugged mid-session, and a companion watch that is not nearby. They map
    /// onto the statuses the contract already has, and the exact code stays in
    /// `detail` for whoever is reading a bug report.
    private static func translate(_ error: NSError?) -> (String, String) {
        guard let error else {
            return ("unavailable", "the system failed and did not say why")
        }
        let detail = "LAError \(error.code): \(error.localizedDescription)"
        guard error.domain == LAErrorDomain, let code = LAError.Code(rawValue: error.code) else {
            return ("unavailable", detail)
        }
        switch code {
        case .authenticationFailed: return ("failed", detail)
        case .userCancel: return ("userCancel", detail)
        case .userFallback: return ("userFallback", detail)
        case .systemCancel: return ("systemCancel", detail)
        case .appCancel: return ("systemCancel", detail)
        case .passcodeNotSet: return ("passcodeNotSet", detail)
        case .biometryNotAvailable: return ("noHardware", detail)
        case .biometryNotEnrolled: return ("notEnrolled", detail)
        case .biometryLockout: return ("lockedOut", detail)
        case .invalidContext: return ("unavailable", detail)
        case .notInteractive: return ("unavailable", detail)
        // No Touch ID keyboard has ever been paired with this Mac: there is
        // hardware in the world but none this machine can use.
        case .biometryNotPaired: return ("noHardware", detail)
        // There was one and it went away — an external keyboard unplugged, the
        // lid shut. It is not `noHardware`, because plugging it back in fixes it.
        case .biometryDisconnected: return ("unavailable", detail)
        // Same as with `LABiometryType`: a new code from a newer version lands
        // here, and `unavailable` with the number inside it is true.
        default: return ("unavailable", detail)
        }
    }
}
