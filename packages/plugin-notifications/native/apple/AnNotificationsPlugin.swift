import Foundation
import UserNotifications

/// Notifications on every Apple platform, out of one file.
///
/// `UNUserNotificationCenter` is the same class on iOS, macOS and watchOS, and
/// none of the three needs a view controller here, so one source serves all of
/// them rather than three copies that would drift.
///
/// The delegate is what makes a tap reach the app, and it is set on the shared
/// centre as this plugin is created — before the app has finished starting.
/// That matters: a notification tapped to *launch* the app is delivered by the
/// system almost immediately, and a delegate installed later would miss it. It
/// is held until something in JS subscribes.
final class AnNotificationsPlugin: NSObject, AnPlugin, UNUserNotificationCenterDelegate {

    private let centre = UNUserNotificationCenter.current()

    /// The tap that launched the app, waiting for a subscriber.
    ///
    /// Nothing here can ask JS whether anybody is listening yet, so the rule is
    /// a time one rather than a question: taps are held until JS drains for the
    /// first time, and emitted live from then on. That is the difference between
    /// an app that opens the right screen when a notification launched it and
    /// one that opens its home page.
    private var held: [[String: Any]] = []
    private var drained = false

    override init() {
        super.init()
        centre.delegate = self
    }

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "permission":
            centre.getNotificationSettings { settings in
                respond.resolve(Self.name(for: settings.authorizationStatus))
            }

        case "request":
            centre.getNotificationSettings { [weak self] settings in
                guard settings.authorizationStatus == .notDetermined else {
                    respond.resolve(Self.name(for: settings.authorizationStatus))
                    return
                }
                self?.centre.requestAuthorization(options: [.alert, .sound, .badge]) { granted, _ in
                    respond.resolve(granted ? "granted" : "denied")
                }
            }

        case "schedule":
            schedule(args, respond)

        case "cancel":
            guard let id = args["id"] as? String, !id.isEmpty else {
                respond.reject("notifications.cancel needs an id in 'id'")
                return
            }
            // Cancelling something that is not there is not an error: the caller
            // wanted it not to fire, and it will not.
            centre.removePendingNotificationRequests(withIdentifiers: [id])
            respond.resolve()

        case "pending":
            centre.getPendingNotificationRequests { requests in
                respond.resolve(requests.map(\.identifier))
            }

        case "clearDelivered":
            centre.removeAllDeliveredNotifications()
            respond.resolve()

        case "remoteToken":
            // The token arrives on the app delegate, which belongs to the shell
            // and not to a plugin. Saying so is more use than an empty string
            // somebody would send to a server that could never reach this device.
            respond.reject(
                "notifications.remoteToken needs the app to register with APNs and hand the "
                    + "token to this plugin, which the shell does not do yet. Local "
                    + "notifications work; remote ones need a certificate and a server.")

        case "drain":
            // Called by `on()` the first time somebody subscribes, so a
            // notification that launched the app is not lost between the system
            // delivering it and the app being ready to hear about it.
            let waiting = held
            held = []
            drained = true
            for payload in waiting {
                AnEvents.emit("notifications", "notification", payload)
            }
            respond.resolve()

        default:
            respond.reject("the notifications plugin has no method \(method)")
        }
    }

    private func schedule(_ args: [String: Any], _ respond: AnPluginCall) {
        guard let id = args["id"] as? String, !id.isEmpty else {
            respond.reject("notifications.schedule needs an id in 'id'")
            return
        }
        guard let title = args["title"] as? String else {
            respond.reject("notifications.schedule needs a title in 'title'")
            return
        }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = (args["body"] as? String) ?? ""
        content.sound = .default
        if let data = args["data"] as? [String: Any] {
            content.userInfo = ["an": data]
        }

        var trigger: UNNotificationTrigger?
        if let at = args["at"] as? Double {
            let seconds = at / 1000 - Date().timeIntervalSince1970
            // A trigger in the past never fires. Delivering it now is what the
            // caller meant, and it is what "at: a moment that has gone" should do.
            if seconds > 0 {
                trigger = UNTimeIntervalNotificationTrigger(timeInterval: seconds, repeats: false)
            }
        }

        // Scheduling with an id that is already pending replaces it: that is
        // `UNUserNotificationCenter`'s own behaviour and it is the useful one.
        centre.add(UNNotificationRequest(identifier: id, content: content, trigger: trigger)) {
            error in
            if let error {
                respond.reject("notifications.schedule failed: \(error.localizedDescription)")
            } else {
                respond.resolve()
            }
        }
    }

    // ------------------------------------------------------------- delegate

    /// The app is in front and a notification arrived. Without this the system
    /// suppresses it, which is nearly never what an app wants.
    func userNotificationCenter(
        _ centre: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completion: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        AnEvents.emit("notifications", "notification", Self.describe(notification, tapped: false))
        if #available(iOS 14.0, macOS 11.0, watchOS 7.0, *) {
            completion([.banner, .sound])
        } else {
            completion([.alert, .sound])
        }
    }

    /// The person tapped one.
    func userNotificationCenter(
        _ centre: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completion: @escaping () -> Void
    ) {
        let payload = Self.describe(response.notification, tapped: true)
        if drained {
            AnEvents.emit("notifications", "notification", payload)
        } else {
            // This is the tap that launched the app: JS is not up yet, and it is
            // the one an app most wants not to lose.
            held.append(payload)
        }
        completion()
    }

    private static func describe(_ notification: UNNotification, tapped: Bool) -> [String: Any] {
        let content = notification.request.content
        return [
            "id": notification.request.identifier,
            "title": content.title,
            "body": content.body,
            "data": (content.userInfo["an"] as? [String: Any]) ?? [:],
            "tapped": tapped
        ]
    }

    private static func name(for status: UNAuthorizationStatus) -> String {
        switch status {
        case .notDetermined: return "prompt"
        case .denied: return "denied"
        default: return "granted"
        }
    }
}
