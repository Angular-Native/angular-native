import Foundation

/// The answer to a plugin call.
///
/// It can be answered on the spot or kept and answered later: that is what
/// separates asking whether a key is in the keychain from reading one that is
/// behind the passcode. What cannot be done is not answering —the promise on the
/// JS side is left waiting for ever—, so every error path has to end in `reject`.
///
/// This is the iOS `AnPluginCall` word for word except for the prefix on the
/// four C functions, and on purpose: a plugin that covers both writes the same
/// `respond.resolve(...)` in both halves.
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
        _ = an_watch_plugin_reject(id, message)
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
        _ = an_watch_plugin_resolve(id, json)
    }

    /// `fragmentsAllowed` is what allows a loose string to be serialised and not
    /// only objects and arrays. If something is not serialisable, `null` is sent:
    /// the core hands it over as it comes and the error shows in the app, not
    /// here.
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

/// What a watchOS plugin implements.
///
/// It does not declare its own name: the name JS calls it by is in the
/// `angularNative.module` of its `package.json`, and that is where `an` takes it
/// from when generating the registry. One single place to write it is one fewer
/// place where two copies can stop matching.
///
/// **There is no `attach` here**, and the absence is the honest one. On the
/// phone and on the Mac a plugin is handed the view controller it would present
/// something from; on a watch there is no view controller — the shell is a
/// SwiftUI `App` and the tree is drawn from a model. A plugin that needs to put
/// something on screen cannot be written against this protocol, and inventing an
/// `attach` that hands over nothing would only hide that until run time.
protocol AnPlugin: AnyObject {
    /// `args` is what JS sent, already decoded. If it sent something that is not
    /// an object, this arrives empty. A method that does not exist has to be
    /// rejected, not ignored.
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall)
}

/// The watch `.app`'s plugin registry.
///
/// Who is inside is decided by `an` when it builds the app, reading the
/// dependencies of the `package.json`: it writes them into
/// `AnGeneratedPlugins.swift`, which is what `install()` calls. A plugin only
/// gets that far if it declared `angularNative.watchos`, so anything in here is
/// something somebody wrote for a watch on purpose.
enum AnPluginRegistry {
    private static var plugins: [String: AnPlugin] = [:]

    /// Called once, before `an_watch_runtime_new`: the core builds one module
    /// per registered plugin when the engine starts, and anything arriving after
    /// that would no longer get in.
    static func install() {
        an_watch_plugin_set_dispatch { id, module, method, args in
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
        plugins[name] = plugin
        an_watch_plugin_register(name)
    }

    /// Rust calls in here from inside the frame, on the main actor.
    private static func dispatch(id: UInt64, module: String, method: String, args: String) {
        guard let plugin = plugins[module] else {
            // This should not be able to happen: the core only knows the names
            // this registry gave it. If it does happen, it is said rather than
            // leaving the promise hanging.
            _ = an_watch_plugin_reject(id, "the plugin \(module) is not in this watch .app")
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
