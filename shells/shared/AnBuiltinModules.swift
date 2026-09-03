import Foundation

#if canImport(UIKit) && !os(watchOS)
    import UIKit
#elseif os(macOS)
    import AppKit
#endif

/// The answer to a built-in module call.
///
/// It is `AnPluginCall` under another name and over another mailbox, and it is
/// separate for the same reason the mailboxes are: a host with no plugin
/// registry —macOS, the watch— still has the built-ins, and neither shell has an
/// `AnPluginCall` to borrow.
///
/// It can be answered on the spot or kept and answered later: that is what
/// separates asking the network monitor from putting a share sheet on the screen
/// and waiting for somebody to pick something. What cannot be done is not
/// answering, so every error path has to end in `reject`.
final class AnBuiltinCall {
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

    func resolve(_ object: [String: Any]) {
        send(json: Self.encode(object))
    }

    func resolve(_ array: [Any]) {
        send(json: Self.encode(array))
    }

    /// For a method that answers "there was nothing", which is not the same as
    /// failing: a picker somebody dismissed, a share sheet somebody cancelled.
    func resolveNull() {
        send(json: "null")
    }

    func reject(_ message: String) {
        lock.lock()
        let first = !answered
        answered = true
        lock.unlock()
        guard first else { return }
        _ = an_builtin_reject(id, message)
    }

    private func send(json: String) {
        lock.lock()
        let first = !answered
        answered = true
        lock.unlock()
        guard first else {
            NSLog("angular-native: a built-in module answered the same call twice")
            return
        }
        _ = an_builtin_resolve(id, json)
    }

    private static func encode(_ value: Any) -> String {
        guard
            let data = try? JSONSerialization.data(
                withJSONObject: value, options: [.fragmentsAllowed]),
            let text = String(data: data, encoding: .utf8)
        else {
            NSLog("angular-native: a built-in module returned something that is not JSON")
            return "null"
        }
        return text
    }
}

/// What a built-in module implements. The name it answers to is in
/// `AnBuiltinModules.install`, and in `BUILTIN_MODULES` on the Rust side.
protocol AnBuiltinModule: AnyObject {
    /// Called once, at install time, before any call can arrive.
    func start()

    /// `args` is what JS sent, already decoded. If it sent something that is
    /// not an object, this arrives empty. A method that does not exist has to
    /// be rejected, not ignored.
    func call(_ method: String, _ args: [String: Any], _ respond: AnBuiltinCall)
}

extension AnBuiltinModule {
    func start() {}
}

/// Where a module puts something on the screen.
///
/// A share sheet and a file picker need somewhere to be presented from, and
/// that somewhere is not the same object on every Apple platform: UIKit
/// presents from a view controller, AppKit hangs a picker off a view, and
/// watchOS presents neither of the two because it has neither. It is kept here,
/// weakly, so a module does not have to be handed it and does not keep the
/// screen alive.
enum AnBuiltinHost {
    #if canImport(UIKit) && !os(watchOS)
        static weak var viewController: UIViewController?
    #elseif os(macOS)
        static weak var view: NSView?
    #endif
}

/// The built-in modules of an Apple shell.
///
/// It is the twin of `AnPluginRegistry` with one difference that matters: who
/// is inside is not read from anybody's `package.json`. These four ship with the
/// framework and every host has them, so the list is written here and in
/// `BUILTIN_MODULES` on the Rust side, and `scripts/check-builtins.sh` reads
/// both.
enum AnBuiltinModules {
    private static var modules: [String: AnBuiltinModule] = [:]

    /// Called once, before the runtime is created: the core builds one native
    /// module per name when the engine starts, and anything arriving later
    /// would no longer get in.
    static func install() {
        an_builtin_set_dispatch { id, module, method, args in
            AnBuiltinModules.dispatch(
                id: id,
                module: AnBuiltinModules.text(module),
                method: AnBuiltinModules.text(method),
                args: AnBuiltinModules.text(args))
        }
        register("files", AnBuiltinFiles())
        register("share", AnBuiltinShare())
    }

    private static func register(_ name: String, _ module: AnBuiltinModule) {
        guard modules[name] == nil else {
            NSLog("angular-native: the built-in module \(name) was installed twice")
            return
        }
        modules[name] = module
        module.start()
    }

    /// Rust calls in here from the main thread, inside the frame.
    private static func dispatch(id: UInt64, module: String, method: String, args: String) {
        let respond = AnBuiltinCall(id: id)
        guard let implementation = modules[module] else {
            // This should not be able to happen: the names come from
            // `BUILTIN_MODULES` and they are the same ones registered above. If
            // it does happen, it is said rather than leaving the promise
            // hanging.
            respond.reject(
                "the built-in module \(module) is not installed in this shell; "
                    + "it is in BUILTIN_MODULES and nobody registered it")
            return
        }
        let decoded = try? JSONSerialization.jsonObject(
            with: Data(args.utf8), options: [.fragmentsAllowed])
        implementation.call(method, decoded as? [String: Any] ?? [:], respond)
    }

    private static func text(_ pointer: UnsafePointer<CChar>?) -> String {
        guard let pointer else { return "" }
        return String(cString: pointer)
    }
}
