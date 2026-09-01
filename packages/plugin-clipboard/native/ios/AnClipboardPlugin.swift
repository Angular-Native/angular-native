import UIKit

/// El portapapeles de iOS.
///
/// Se compila dentro del `.app` en la misma invocación de `swiftc` que el
/// shell, así que ve `AnPlugin` y `AnPluginCall` sin importar nada. Quien lo
/// registra es el fichero que genera `an` a partir del `package.json`.
///
/// `UIPasteboard` es del hilo principal, y aquí ya estamos en él: el core
/// entrega las llamadas dentro del frame. Por eso se puede contestar en el
/// acto en vez de guardarse el `AnPluginCall` para más tarde.
final class AnClipboardPlugin: AnPlugin {
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "write":
            guard let text = args["text"] as? String else {
                respond.reject("clipboard.write necesita un texto en 'text'")
                return
            }
            UIPasteboard.general.string = text
            respond.resolve()

        case "read":
            // Cadena vacía y no nulo: quien pide el portapapeles quiere pintar
            // algo, y `undefined` obligaría a comprobarlo en cada uso.
            respond.resolve(UIPasteboard.general.string ?? "")

        case "hasText":
            // `hasStrings` no lee el contenido, así que no dispara el aviso de
            // pegado que iOS 16 enseña al leer lo que copió otra app.
            respond.resolve(UIPasteboard.general.hasStrings)

        default:
            // Nunca en silencio: un método que no existe rechaza la promesa
            // diciendo cuál se pidió.
            respond.reject("el plugin clipboard no tiene ningún método \(method)")
        }
    }
}
