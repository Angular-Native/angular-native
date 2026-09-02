import Foundation
import LocalAuthentication
import Security

/// El llavero de iOS: Keychain Services.
///
/// Cada secreto es un `kSecClassGenericPassword` con el identificador del
/// bundle como servicio y la clave de la app como cuenta. El servicio no es
/// decorativo: es lo que separa los secretos de esta app de los de cualquier
/// otra que use el mismo llavero.
///
/// Dos cosas que no se ven en el código y mandan sobre todo lo demás:
///
///   · **`…ThisDeviceOnly`.** Sin ese sufijo, un elemento del llavero viaja en
///     la copia de seguridad cifrada y aparece en el teléfono nuevo. Con él, no
///     sale de este aparato. Para un testigo de sesión es lo que se quiere; si
///     alguien quisiera lo contrario tendría que ser una decisión escrita, no
///     el valor por defecto.
///
///   · **`.biometryCurrentSet`.** Ata el elemento al juego de caras y huellas
///     que hay registrado en el momento de guardarlo. Si mañana se añade otra
///     cara, el sistema tira la clave y el secreto ya no se puede leer. Sin
///     eso, quien pudiera añadir su propia huella —alguien con el código del
///     aparato— tendría también el secreto.
final class AnKeychainPlugin: AnPlugin {

    /// El servicio con el que se guardan los elementos.
    ///
    /// Del identificador del bundle, que es único por app. Si algún día
    /// faltara, se usa un nombre fijo: compartir servicio con otra app de
    /// angular-native sería peor que no guardar nada, así que se avisa.
    private static var service: String {
        guard let identifier = Bundle.main.bundleIdentifier else {
            NSLog("angular-native: el bundle no tiene identificador; el llavero usará un nombre fijo")
            return "dev.angularnative.keychain"
        }
        return identifier
    }

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        // `backing` es lo único que no necesita clave.
        if method == "backing" {
            respond.resolve(Self.backing())
            return
        }
        guard let key = args["key"] as? String, !key.isEmpty else {
            respond.reject("keychain.\(method) necesita una clave no vacía en 'key'")
            return
        }

        switch method {
        case "set":
            guard let value = args["value"] as? String else {
                respond.reject("keychain.set necesita el secreto en 'value'")
                return
            }
            let biometrics = args["requireBiometrics"] as? Bool ?? false
            if biometrics && (args["reason"] as? String ?? "").isEmpty {
                respond.reject(
                    "keychain.set con requireBiometrics necesita un 'reason': es lo que el "
                        + "sistema enseña dentro del diálogo al leerlo")
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
            respond.reject("el plugin keychain no tiene ningún método \(method)")
        }
    }

    // MARK: - Guardar

    /// Guardar nunca pregunta nada en iOS, ni siquiera con biometría: el
    /// diálogo lo enseña la lectura. Aun así se hace fuera del hilo principal,
    /// porque `SecItemAdd` con control de acceso puede tardar.
    private func set(key: String, value: String, biometrics: Bool, respond: AnPluginCall) {
        DispatchQueue.global(qos: .userInitiated).async {
            // Reemplazar es borrar y volver a añadir. `SecItemUpdate` no puede
            // cambiar el control de acceso de un elemento que ya existe, así
            // que guardar encima de uno sin biometría uno con biometría se
            // quedaría a medias: el valor nuevo con la protección vieja.
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
                        // Con código de aparato obligatorio: sin código no hay
                        // biometría que valga, y un elemento que se pueda leer
                        // en un teléfono sin bloqueo no protege de nada.
                        kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                        .biometryCurrentSet,
                        &error)
                else {
                    let detail = (error?.takeRetainedValue()).map { "\($0)" } ?? "sin motivo"
                    respond.resolve([
                        "outcome": "unavailable",
                        "detail": "no se pudo crear el control de acceso: \(detail)"
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
            // errSecAuthFailed aquí quiere decir que el aparato no tiene
            // código: el control de acceso lo exige.
            let outcome = status == errSecAuthFailed ? "denied" : "unavailable"
            respond.resolve(["outcome": outcome, "detail": Self.describe(status)])
        }
    }

    // MARK: - Leer

    /// Leer un elemento con biometría **bloquea el hilo mientras el diálogo
    /// está en pantalla**. Si eso pasara en el hilo principal, la app se
    /// quedaría congelada detrás del propio diálogo del sistema y el core no
    /// avanzaría ni un frame. De ahí la cola de fondo, que es también el
    /// motivo por el que este método no contesta en el acto.
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
            // Sin botón de «Introducir código»: quien quiera esa puerta que la
            // pida por su cuenta con el plugin de biometría. Aquí, si el
            // elemento se guardó con biometría, se lee con biometría.
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
                        "detail": "el llavero devolvió algo que no es texto UTF-8"
                    ])
                    return
                }
                respond.resolve(["outcome": "found", "value": text, "detail": "SecItemCopyMatching"])

            case errSecItemNotFound:
                // También es lo que sale cuando el elemento estaba atado a unas
                // huellas que ya no existen: iOS lo borra, no lo deja inservible.
                // Por eso `invalidated` no llega nunca desde iOS.
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

    // MARK: - Mirar y borrar

    /// Si hay algo, sin abrirlo.
    ///
    /// Se piden los atributos y **no** los datos. Es lo que hace que esto no
    /// saque un Face ID: la autenticación la exige leer el contenido, no saber
    /// que el elemento existe.
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

    /// Borrar tampoco autentica: el llavero deja tirar un elemento que no se
    /// puede leer, y menos mal, o un secreto atado a una huella borrada se
    /// quedaría ahí para siempre.
    private static func remove(_ key: String) -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        return SecItemDelete(query as CFDictionary) == errSecSuccess
    }

    // MARK: - Qué hay debajo

    private static func backing() -> [String: Any] {
        [
            "platform": "ios",
            // Todo iPhone que llegue a iOS 17 tiene Secure Enclave, y es él
            // quien guarda las claves de clase con las que se cifra el llavero.
            "hardwareBacked": true,
            "accessible": "kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly",
            "detail": "Keychain Services, servicio \(service)"
        ]
    }

    /// El código de `OSStatus` con su descripción, para el registro.
    private static func describe(_ status: OSStatus) -> String {
        let text = SecCopyErrorMessageString(status, nil) as String? ?? "sin descripción"
        return "OSStatus \(status): \(text)"
    }
}
