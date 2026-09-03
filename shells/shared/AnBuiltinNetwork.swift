import Foundation
import Network

/// The `network` module on Apple's platforms.
///
/// It is `NWPathMonitor`, which is the one thing on Apple's platforms that
/// really knows: not a reachability check against some address, not a request
/// that failed, but the system's own view of the path out — updated when the
/// Wi-Fi drops, when the phone falls back to cellular, and when somebody turns
/// Low Data Mode on.
///
/// The monitor is started once, at install, and left running. That is what makes
/// `status()` answer immediately and from the truth: the call reads a value the
/// system last pushed, rather than going off to ask a question that takes time
/// and can be wrong by the time it comes back.
///
/// It is also the one module of the four that every host has in full. A watch, a
/// television and a headset all have a network, and all four platforms have this
/// class.
final class AnBuiltinNetwork: AnBuiltinModule {
    private let monitor = NWPathMonitor()
    private let queue = DispatchQueue(label: "dev.angularnative.network")
    private let lock = NSLock()
    /// What the monitor last said. `nil` only in the instant between starting it
    /// and its first update, which is answered honestly rather than guessed at.
    private var path: NWPath?

    func start() {
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            self.lock.lock()
            self.path = path
            self.lock.unlock()
        }
        monitor.start(queue: queue)
    }

    func call(_ method: String, _ args: [String: Any], _ respond: AnBuiltinCall) {
        switch method {
        case "status":
            lock.lock()
            let path = self.path ?? monitor.currentPath
            lock.unlock()
            respond.resolve(Self.describe(path))

        default:
            respond.reject("the network module has no method \(method)")
        }
    }

    private static func describe(_ path: NWPath) -> [String: Any] {
        [
            "online": path.status == .satisfied,
            "connection": connection(path),
            // A metered connection: cellular, or a personal hotspot. What an app
            // consults before downloading something large.
            "expensive": path.isExpensive,
            // Low Data Mode. The person asked for less traffic and the system is
            // passing that on.
            "constrained": path.isConstrained
        ]
    }

    /// Which interface it is going out through.
    ///
    /// `'none'` is not "some interface called none": it is what a path that is
    /// not satisfied gets, because there is no interface. Asking `usesInterfaceType`
    /// of an unsatisfied path answers `false` to everything, and calling that
    /// `'other'` would be reporting a connection where there is none.
    private static func connection(_ path: NWPath) -> String {
        guard path.status == .satisfied else { return "none" }
        if path.usesInterfaceType(.wifi) { return "wifi" }
        if path.usesInterfaceType(.cellular) { return "cellular" }
        if path.usesInterfaceType(.wiredEthernet) { return "ethernet" }
        return "other"
    }
}
