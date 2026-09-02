import LocalAuthentication
import UIKit

/// Biometría de iOS: `LAContext`.
///
/// El diálogo lo presenta el sistema por encima de la app, así que aquí no hay
/// vista que montar ni `UIViewController` del que colgar nada: por eso este
/// plugin no implementa `attach`.
///
/// `evaluatePolicy` contesta en una cola suya, no en la principal. Da igual:
/// `AnPluginCall` habla con un buzón con cerrojo al otro lado, no con una
/// variable del hilo de UI, así que se puede contestar desde donde llegue la
/// respuesta.
///
/// Nada de esto funciona sin `NSFaceIDUsageDescription` en el `Info.plist`.
/// Sin esa clave iOS no avisa ni devuelve un error: mata el proceso en cuanto
/// se evalúa la política, y desde fuera parece que la app se cerró sola. La
/// clave la declara el `package.json` de este plugin y la funde `an` al armar
/// el `.app`; no hay que ponerla a mano.
final class AnBiometricsPlugin: AnPlugin {

    /// El contexto de la evaluación en curso.
    ///
    /// Hay que guardarlo: `evaluatePolicy` es asíncrono y si el `LAContext` se
    /// libera antes de que conteste, el diálogo se cierra solo y la llamada no
    /// vuelve. Uno nuevo por evaluación, además, porque un contexto recuerda
    /// que ya autenticó y reusarlo devolvería `success` sin enseñar nada.
    private var pending: LAContext?

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "availability":
            respond.resolve(Self.availability())

        case "authenticate":
            guard let reason = args["reason"] as? String, !reason.isEmpty else {
                respond.reject("biometrics.authenticate necesita un 'reason' no vacío: es lo que "
                    + "el sistema enseña dentro del diálogo")
                return
            }
            authenticate(
                reason: reason,
                cancelTitle: args["cancelTitle"] as? String,
                allowDeviceCredential: args["allowDeviceCredential"] as? Bool ?? false,
                respond: respond)

        default:
            respond.reject("el plugin biometrics no tiene ningún método \(method)")
        }
    }

    // MARK: - Disponibilidad

    /// Qué hay y si se puede usar, sin enseñar nada.
    ///
    /// `biometryType` no vale hasta después de `canEvaluatePolicy`: en un
    /// `LAContext` recién creado siempre es `.none`, aunque el aparato tenga
    /// Face ID. Por eso se pregunta siempre, incluso cuando solo interesa el
    /// tipo.
    private static func availability() -> [String: Any] {
        let context = LAContext()
        var error: NSError?
        let puede = context.canEvaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics, error: &error)
        let kind = describe(context.biometryType)
        if puede {
            return ["status": "available", "kind": kind, "detail": "LAContext.canEvaluatePolicy"]
        }
        guard let error else {
            // `canEvaluatePolicy` promete rellenar el error cuando devuelve
            // false. Si algún día no lo hace, se dice; dar `available` aquí
            // sería enseñar un botón que no funciona.
            return [
                "status": "unavailable", "kind": kind,
                "detail": "canEvaluatePolicy dijo que no y no dijo por qué"
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
        // `LABiometryType` puede crecer con el sistema, y una versión de iOS
        // más nueva que este código traería un tipo que aquí no está. Decir
        // `unknown` es cierto; decir `none` sería decir que no hay sensor.
        @unknown default: return "unknown"
        }
    }

    // MARK: - Autenticación

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
        // Cadena vacía en `localizedFallbackTitle` quita el botón de «Introducir
        // código». Sin esto iOS lo enseña igualmente, el usuario lo pulsa y la
        // llamada vuelve con `userFallback` sin que la app haya pedido nunca
        // esa puerta.
        if !allowDeviceCredential {
            context.localizedFallbackTitle = ""
        }
        let policy: LAPolicy =
            allowDeviceCredential ? .deviceOwnerAuthentication : .deviceOwnerAuthenticationWithBiometrics

        // El tipo se lee antes de evaluar, mientras el contexto sigue vivo y
        // ya ha resuelto la política: después de contestar no hay a quién
        // preguntar.
        var probe: NSError?
        let puede = context.canEvaluatePolicy(policy, error: &probe)
        let kind = Self.describe(context.biometryType)
        if !puede {
            // Sin sensor, sin huella registrada o bloqueado: se contesta con
            // el motivo en vez de enseñar un diálogo que el sistema va a
            // cerrar solo.
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

    // MARK: - Errores

    /// De `LAError` a los nombres del contrato.
    ///
    /// Cada rama es una situación distinta para quien usa la app, y por eso no
    /// se juntan: `userCancel` no merece ni un aviso, `biometryNotEnrolled`
    /// merece un enlace a Ajustes y `biometryLockout` merece ofrecer el código.
    ///
    /// iOS no distingue el bloqueo temporal del permanente —`biometryLockout`
    /// es el único código que hay—, así que `permanentlyLockedOut` no sale
    /// nunca de aquí. Es una diferencia real entre las dos plataformas y está
    /// documentada; fingir el segundo estado a base de contar intentos sería
    /// inventárselo.
    private static func translate(_ error: NSError?) -> (String, String) {
        guard let error else {
            return ("unavailable", "el sistema falló y no dijo por qué")
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
        // Igual que con `LABiometryType`: un código nuevo de una versión más
        // nueva llega aquí, y `unavailable` con el número dentro es cierto.
        default: return ("unavailable", detail)
        }
    }
}
