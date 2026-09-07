import CoreLocation
import Foundation

/// The device's location, on iOS and on the Mac.
///
/// One file for both because `CLLocationManager` is one class on both, and the
/// two places they genuinely differ are handled with a `#if` rather than with a
/// second copy that would drift: the name of the "allowed" case in
/// `CLAuthorizationStatus`, and the fact that a Mac has no "when in use".
///
/// This is the first plugin here that cannot answer on the spot. A fix arrives
/// through a delegate callback some seconds later, so the `AnPluginCall` is held
/// and answered from that callback — which is exactly the case the responder was
/// built for. Everything that can end the wait ends it exactly once:
///
///   · a fix arrives,
///   · CoreLocation reports a failure,
///   · the timeout fires.
///
/// A promise nobody settles is worse than a rejection, so the timer is armed
/// before the request goes out and is cancelled by whichever answer wins.
final class AnGeolocationPlugin: NSObject, AnPlugin, CLLocationManagerDelegate {

    private let manager = CLLocationManager()

    /// The call waiting for a fix, if any. Only ever touched on the main thread:
    /// CoreLocation's callbacks arrive there and so do the plugin's calls.
    private var pending: AnPluginCall?
    private var timeout: DispatchWorkItem?

    /// The call waiting for the person to answer the permission dialog.
    private var asking: AnPluginCall?

    /// Whether a `watch()` is running. A stream and a one-shot share one
    /// manager, so the delegate has to know which of the two a fix belongs to:
    /// a `current()` settles and stops, a watch keeps going.
    private var watching = false

    override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyBest
    }

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "permission":
            respond.resolve(Self.name(for: Self.status(of: manager)))

        case "request":
            let status = Self.status(of: manager)
            // The system shows the dialog once and once only. Asking again when
            // it has already been answered returns the standing answer rather
            // than doing nothing and leaving the caller waiting.
            guard status == .notDetermined else {
                respond.resolve(Self.name(for: status))
                return
            }
            if let waiting = asking {
                waiting.reject("geolocation.request was already waiting for an answer")
            }
            asking = respond
            #if os(macOS)
                manager.requestAlwaysAuthorization()
            #else
                manager.requestWhenInUseAuthorization()
            #endif

        case "current":
            current(args, respond)

        case "watch":
            let status = Self.status(of: manager)
            guard status != .notDetermined, status != .denied, status != .restricted else {
                respond.reject(
                    "geolocation.watch has no permission: location for this app is off in Settings")
                return
            }
            // A distance filter and not a timer: the platform already knows the
            // device has not moved, and asking it on an interval would either
            // miss a movement or keep the radio awake for nothing.
            let metres = (args["minMetres"] as? Double) ?? 0
            manager.distanceFilter = metres > 0 ? metres : kCLDistanceFilterNone
            watching = true
            manager.startUpdatingLocation()
            respond.resolve()

        case "unwatch":
            watching = false
            manager.stopUpdatingLocation()
            respond.resolve()

        default:
            respond.reject("the geolocation plugin has no method \(method)")
        }
    }

    private func current(_ args: [String: Any], _ respond: AnPluginCall) {
        // Checked before asking CoreLocation for anything: without this, an app
        // whose permission was refused sits watching a spinner until the
        // timeout, and then cannot say why.
        let status = Self.status(of: manager)
        switch status {
        case .notDetermined:
            respond.reject(
                "geolocation.current has no permission yet: call request() first")
            return
        case .denied, .restricted:
            respond.reject(
                "geolocation.current was refused: location for this app is off in Settings")
            return
        default:
            break
        }
        guard CLLocationManager.locationServicesEnabled() else {
            respond.reject("geolocation.current cannot run: location is switched off on this device")
            return
        }
        if let waiting = pending {
            waiting.reject("geolocation.current was superseded by another call")
        }

        pending = respond
        let milliseconds = (args["timeoutMs"] as? Double) ?? 10_000
        let work = DispatchWorkItem { [weak self] in
            guard let self, let waiting = self.pending else { return }
            self.pending = nil
            self.timeout = nil
            if !self.watching {
                self.manager.stopUpdatingLocation()
            }
            waiting.reject(
                "geolocation.current had no fix after \(Int(milliseconds)) ms")
        }
        timeout = work
        DispatchQueue.main.asyncAfter(deadline: .now() + milliseconds / 1000, execute: work)

        // `requestLocation` delivers one fix and stops by itself, which is what
        // a one-shot call wants: `startUpdatingLocation` would keep the radio on
        // until something remembered to stop it.
        manager.requestLocation()
    }

    // ------------------------------------------------------------- delegate

    func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard let fix = locations.last else { return }
        // A watch emits; it does not answer. The event belongs to no call, which
        // is the whole reason the event channel exists.
        if watching {
            AnEvents.emit("geolocation", "position", Self.describe(fix))
        }
        // And a `current()` waiting at the same time still gets its one answer:
        // the two share a manager, so one fix can serve both.
        if let waiting = settle() {
            waiting.resolve(Self.describe(fix))
        }
    }

    func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        if watching {
            // A stream cannot reject: nobody is holding a promise for it. The
            // failure is an event of its own, so an app can tell "no fix yet"
            // from "this is not going to work".
            AnEvents.emit("geolocation", "error", ["message": error.localizedDescription])
        }
        guard let waiting = settle() else { return }
        waiting.reject("geolocation.current failed: \(error.localizedDescription)")
    }

    func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        guard let waiting = asking else { return }
        let status = Self.status(of: manager)
        // The delegate also fires once as the manager starts up, before the
        // person has touched anything. Answering then would resolve `request()`
        // with "prompt", which is not an answer.
        guard status != .notDetermined else { return }
        asking = nil
        waiting.resolve(Self.name(for: status))
    }

    /// Takes the waiting call and disarms the timeout, or nil if something got
    /// there first. Every path out goes through here, which is what makes
    /// "answered exactly once" true rather than intended.
    private func settle() -> AnPluginCall? {
        timeout?.cancel()
        timeout = nil
        guard let waiting = pending else { return nil }
        pending = nil
        return waiting
    }

    // -------------------------------------------------------------- mapping

    private static func status(of manager: CLLocationManager) -> CLAuthorizationStatus {
        if #available(iOS 14.0, macOS 11.0, *) {
            return manager.authorizationStatus
        }
        return CLLocationManager.authorizationStatus()
    }

    private static func name(for status: CLAuthorizationStatus) -> String {
        switch status {
        case .notDetermined:
            return "prompt"
        case .restricted:
            // Not the same as denied: the person did not refuse, something above
            // them did — a profile, parental controls — and no dialog will help.
            return "restricted"
        case .denied:
            return "denied"
        default:
            return "granted"
        }
    }

    private static func describe(_ fix: CLLocation) -> [String: Any] {
        var out: [String: Any] = [
            "latitude": fix.coordinate.latitude,
            "longitude": fix.coordinate.longitude,
            "accuracy": fix.horizontalAccuracy,
            // The fix's own time, not the clock now: a cached fix is minutes
            // old and an app that has to decide whether to trust it needs to
            // know that.
            "timestamp": fix.timestamp.timeIntervalSince1970 * 1000
        ]
        // A negative accuracy is CoreLocation's way of saying the value is not
        // valid. It comes back as null rather than as a plausible number.
        out["altitude"] = fix.verticalAccuracy >= 0 ? fix.altitude : nil
        out["speed"] = fix.speed >= 0 ? fix.speed : nil
        out["heading"] = fix.course >= 0 ? fix.course : nil
        return out
    }
}
