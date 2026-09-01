import UIKit

/// La respuesta de una llamada a un plugin.
///
/// Se puede contestar en el acto o guardarse y contestar más tarde: es lo que
/// separa leer el portapapeles de sacar una foto. Lo que no se puede es no
/// contestar — la promesa del lado JS se queda esperando para siempre—, así
/// que todo camino de error tiene que acabar en `reject`.
final class AnPluginCall {
    private let id: UInt64
    private let lock = NSLock()
    private var answered = false

    init(id: UInt64) {
        self.id = id
    }

    /// Para un método que no devuelve nada.
    func resolve() {
        send(json: "null")
    }

    func resolve(_ text: String) {
        send(json: Self.encode(text))
    }

    func resolve(_ value: Bool) {
        send(json: value ? "true" : "false")
    }

    func resolve(_ value: Double) {
        send(json: Self.encode(value))
    }

    /// Para devolver un objeto. El diccionario tiene que ser serializable a
    /// JSON: cadenas, números, booleanos, arrays y diccionarios.
    func resolve(_ object: [String: Any]) {
        send(json: Self.encode(object))
    }

    func reject(_ message: String) {
        lock.lock()
        let first = !answered
        answered = true
        lock.unlock()
        guard first else { return }
        _ = an_plugin_reject(id, message)
    }

    private func send(json: String) {
        lock.lock()
        let first = !answered
        answered = true
        lock.unlock()
        guard first else {
            NSLog("angular-native: un plugin contestó dos veces a la misma llamada")
            return
        }
        _ = an_plugin_resolve(id, json)
    }

    /// `fragmentsAllowed` es lo que permite serializar una cadena suelta y no
    /// solo objetos y arrays. Si algo no es serializable se manda `null`: el
    /// core lo entrega tal cual y el error se ve en la app, no aquí.
    private static func encode(_ value: Any) -> String {
        guard
            let data = try? JSONSerialization.data(
                withJSONObject: value, options: [.fragmentsAllowed]),
            let text = String(data: data, encoding: .utf8)
        else {
            NSLog("angular-native: un plugin devolvió algo que no es JSON")
            return "null"
        }
        return text
    }
}

/// Lo que implementa un plugin de iOS.
///
/// No declara su nombre: el nombre con el que JS lo invoca está en el
/// `angularNative.module` de su `package.json` y de ahí lo saca `an` al
/// generar el registro. Un solo sitio donde escribirlo es un sitio menos donde
/// puedan dejar de coincidir.
protocol AnPlugin: AnyObject {
    /// La pantalla de la que cuelga la app, antes de la primera llamada. Es de
    /// donde sale el `present` de quien tenga que enseñar algo —una cámara, un
    /// selector de ficheros—. Quien no la necesite no implementa nada.
    func attach(_ host: UIViewController)

    /// `args` es lo que mandó JS ya decodificado. Si no mandó un objeto llega
    /// vacío. Un método que no exista tiene que rechazarse, no ignorarse.
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall)
}

extension AnPlugin {
    func attach(_ host: UIViewController) {}
}

/// El registro de plugins del `.app`.
///
/// Quién está dentro lo decide `an` al armar la app, leyendo las dependencias
/// del `package.json`: lo escribe en `AnGeneratedPlugins.swift`, que es lo que
/// llama `install()`.
enum AnPluginRegistry {
    private static var plugins: [String: AnPlugin] = [:]
    private static weak var host: UIViewController?

    /// Se llama una vez, antes de `an_runtime_new`: el core construye un
    /// módulo por plugin registrado al arrancar el motor, y lo que llegue
    /// después ya no entraría.
    static func install(host: UIViewController) {
        self.host = host
        an_plugin_set_dispatch { id, module, method, args in
            AnPluginRegistry.dispatch(
                id: id,
                module: AnPluginRegistry.text(module),
                method: AnPluginRegistry.text(method),
                args: AnPluginRegistry.text(args))
        }
        AnGeneratedPlugins.install()
    }

    /// La llama el fichero generado, una vez por plugin. El nombre viene del
    /// `package.json` del plugin, que es el único sitio donde se escribe.
    static func register(_ name: String, _ plugin: AnPlugin) {
        guard plugins[name] == nil else {
            NSLog("angular-native: dos plugins dicen llamarse \(name); se queda el primero")
            return
        }
        if let host { plugin.attach(host) }
        plugins[name] = plugin
        an_plugin_register(name)
    }

    /// Rust llama aquí desde el hilo principal, dentro del frame.
    private static func dispatch(id: UInt64, module: String, method: String, args: String) {
        guard let plugin = plugins[module] else {
            // No debería poder pasar: el core solo conoce los nombres que este
            // registro le dio. Si pasa, se dice en vez de dejar la promesa
            // colgada.
            _ = an_plugin_reject(id, "el plugin \(module) no está en este .app")
            return
        }
        let decoded = try? JSONSerialization.jsonObject(
            with: Data(args.utf8), options: [.fragmentsAllowed])
        plugin.call(method, decoded as? [String: Any] ?? [:], AnPluginCall(id: id))
    }

    private static func text(_ pointer: UnsafePointer<CChar>?) -> String {
        guard let pointer else { return "" }
        return String(cString: pointer)
    }
}
