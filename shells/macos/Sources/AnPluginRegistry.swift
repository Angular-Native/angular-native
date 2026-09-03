import AppKit

/// The answer to a plugin call.
///
/// It can be answered on the spot or kept and answered later: that is what
/// separates reading the pasteboard from waiting on Touch ID. What cannot be done
/// is not answering —the promise on the JS side is left waiting for ever—, so
/// every error path has to end in `reject`.
///
/// This is the iOS `AnPluginCall` word for word, and on purpose: a plugin that
/// covers both platforms writes the same `respond.resolve(...)` in both halves,
/// and the only thing that differs between them is what they ask the system.
final class AnPluginCall {
    private let id: UInt64
    private let lock = NSLock()
    private var answered = false

    init(id: UInt64) {
        self.id = id
    }

    /// For a method that returns nothing.
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

    /// For returning an object. The dictionary has to be serialisable to JSON:
    /// strings, numbers, booleans, arrays and dictionaries.
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
            NSLog("angular-native: a plugin answered the same call twice")
            return
        }
        _ = an_plugin_resolve(id, json)
    }

    /// `fragmentsAllowed` is what allows a loose string to be serialised and
    /// not only objects and arrays. If something is not serialisable, `null` is
    /// sent: the core hands it over as it comes and the error shows in the app,
    /// not here.
    private static func encode(_ value: Any) -> String {
        guard
            let data = try? JSONSerialization.data(
                withJSONObject: value, options: [.fragmentsAllowed]),
            let text = String(data: data, encoding: .utf8)
        else {
            NSLog("angular-native: a plugin returned something that is not JSON")
            return "null"
        }
        return text
    }
}

/// What a macOS plugin implements.
///
/// It does not declare its own name: the name JS calls it by is in the
/// `angularNative.module` of its `package.json`, and that is where `an` takes it
/// from when generating the registry. One single place to write it is one fewer
/// place where two copies can stop matching.
protocol AnPlugin: AnyObject {
    /// The view controller the app hangs off, before the first call. On the Mac
    /// it is where a sheet is presented from —`beginSheet` needs the window, and
    /// the window is reached through the controller's view. Whoever does not need
    /// it implements nothing.
    ///
    /// This is the one line of the protocol that is **not** the phone's: there it
    /// is a `UIViewController`. A plugin covering both platforms writes the
    /// signature its own half needs, which is why the two halves are separate
    /// source directories in the first place.
    func attach(_ host: NSViewController)

    /// `args` is what JS sent, already decoded. If it sent something that is not
    /// an object, this arrives empty. A method that does not exist has to be
    /// rejected, not ignored.
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall)
}

extension AnPlugin {
    func attach(_ host: NSViewController) {}
}

/// The `.app`'s plugin registry.
///
/// Who is inside is decided by `an` when it builds the app, reading the
/// dependencies of the `package.json`: it writes them into
/// `AnGeneratedPlugins.swift`, which is what `install()` calls.
enum AnPluginRegistry {
    private static var plugins: [String: AnPlugin] = [:]
    private static weak var host: NSViewController?

    /// Called once, before `an_runtime_new`: the core builds one module per
    /// registered plugin when the engine starts, and anything arriving after
    /// that would no longer get in.
    static func install(host: NSViewController) {
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

    /// Called by the generated file, once per plugin. The name comes from the
    /// plugin's `package.json`, which is the only place it is written.
    static func register(_ name: String, _ plugin: AnPlugin) {
        guard plugins[name] == nil else {
            NSLog("angular-native: two plugins claim to be called \(name); the first one stays")
            return
        }
        if let host { plugin.attach(host) }
        plugins[name] = plugin
        an_plugin_register(name)
    }

    /// Rust calls in here from the main thread, inside the frame.
    private static func dispatch(id: UInt64, module: String, method: String, args: String) {
        guard let plugin = plugins[module] else {
            // This should not be able to happen: the core only knows the names
            // this registry gave it. If it does happen, it is said rather than
            // leaving the promise hanging.
            _ = an_plugin_reject(id, "the plugin \(module) is not in this .app")
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
