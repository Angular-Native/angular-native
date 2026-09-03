import LocalAuthentication
import UIKit

/// The iOS biometrics: `LAContext`.
///
/// The dialog is presented by the system on top of the app, so there is no view
/// to mount here and no `UIViewController` to hang anything off: that is why
/// this plugin does not implement `attach`.
///
/// `evaluatePolicy` answers on a queue of its own, not on the main one. It does
/// not matter: `AnPluginCall` talks to a mailbox with a lock on the other side,
/// not to a variable belonging to the UI thread, so the answer can be given from
/// wherever it arrives.
///
/// None of this works without `NSFaceIDUsageDescription` in the `Info.plist`.
/// Without that key iOS gives no warning and returns no error: it kills the
/// process the moment the policy is evaluated, and from the outside it looks as
/// though the app closed by itself. The key is declared by this plugin's
/// `package.json` and merged in by `an` when it puts the `.app` together; it does
/// not have to be added by hand.
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
                respond: respond)

        default:
            respond.reject("the biometrics plugin has no method \(method)")
        }
    }

    // MARK: - Availability

    /// What there is and whether it can be used, without showing anything.
    ///
    /// `biometryType` is worthless until after `canEvaluatePolicy`: on a
    /// freshly created `LAContext` it is always `.none`, even on a device with
    /// Face ID. That is why the question is always asked, even when only the
    /// type is of interest.
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
        // `LABiometryType` can grow along with the system, and a version of iOS
        // newer than this code would bring a type that is not here. Saying
        // `unknown` is true; saying `none` would be saying there is no sensor.
        @unknown default: return "unknown"
        }
    }

    // MARK: - Authentication

    private func authenticate(
        reason: String,
        cancelTitle: String?,
        allowDeviceCredential: Bool,
        respond: AnPluginCall
    ) {
        let context = LAContext()
        if let cancelTitle, !cancelTitle.isEmpty {
            context.localizedCancelTitle = cancelTitle
        }
        // An empty string in `localizedFallbackTitle` takes the "Enter passcode"
        // button away. Without this iOS shows it anyway, the user presses it and
        // the call comes back with `userFallback` without the app ever having
        // asked for that door.
        if !allowDeviceCredential {
            context.localizedFallbackTitle = ""
        }
        let policy: LAPolicy =
            allowDeviceCredential ? .deviceOwnerAuthentication : .deviceOwnerAuthenticationWithBiometrics

        // The type is read before evaluating, while the context is still alive
        // and has already resolved the policy: once the answer is given there is
        // nobody left to ask.
        var probe: NSError?
        let allowed = context.canEvaluatePolicy(policy, error: &probe)
        let kind = Self.describe(context.biometryType)
        if !allowed {
            // No sensor, nothing enrolled or locked out: the reason is given
            // back instead of showing a dialog the system is going to close by
            // itself.
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

    // MARK: - Errors

    /// From `LAError` to the names in the contract.
    ///
    /// Every branch is a different situation for whoever is using the app, and
    /// that is why they are not folded together: `userCancel` does not deserve
    /// so much as a warning, `biometryNotEnrolled` deserves a link to Settings
    /// and `biometryLockout` deserves offering the passcode.
    ///
    /// iOS does not tell the temporary lockout from the permanent one
    /// —`biometryLockout` is the only code there is—, so `permanentlyLockedOut`
    /// never comes out of here. It is a real difference between the two
    /// platforms and it is documented; faking the second state by counting
    /// attempts would be making it up.
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
        // Same as with `LABiometryType`: a new code from a newer version lands
        // here, and `unavailable` with the number inside it is true.
        default: return ("unavailable", detail)
        }
    }
}
