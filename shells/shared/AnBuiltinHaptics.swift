import Foundation

#if os(iOS)
    import UIKit
#elseif os(macOS)
    import AppKit
#elseif os(watchOS)
    import WatchKit
#endif

/// The `haptics` module on Apple's platforms.
///
/// This is the module where the four platforms differ most, and none of the
/// differences is an implementation detail — each is a different piece of
/// hardware:
///
/// - **iOS and iPadOS** have the Taptic Engine, and UIKit's three generators
///   over it: an impact with five weights, three notification patterns, and the
///   tick a picker makes as it passes a value.
/// - **watchOS** has a motor on the wrist and `WKInterfaceDevice.play`, whose
///   vocabulary is not UIKit's: it is a list of *meanings* —`.success`,
///   `.failure`, `.notification`— because on a watch a tap is often the only
///   thing the person gets.
/// - **macOS** has haptics only if it has a Force Touch trackpad, and AppKit
///   offers no way to ask whether this Mac does. `NSHapticFeedbackManager` also
///   has nothing that means success, warning or error. Both are said out loud in
///   `support()` rather than a `perform` being fired into a Mac Mini and called
///   done.
/// - **tvOS and visionOS** have nothing at all, and say so by name.
final class AnBuiltinHaptics: AnBuiltinModule {
    #if os(iOS)
        // Kept rather than made per call: a generator that is created and
        // released for each tap never gets to `prepare` the engine, and the
        // first tap after that comes late enough to feel disconnected from what
        // caused it.
        private let selection = UISelectionFeedbackGenerator()
        private let notification = UINotificationFeedbackGenerator()
    #endif

    func call(_ method: String, _ args: [String: Any], _ respond: AnBuiltinCall) {
        switch method {
        case "support":
            respond.resolve(Self.support())
        case "impact":
            impact(args["style"] as? String ?? "medium", respond)
        case "notification":
            notify(args["type"] as? String ?? "success", respond)
        case "selection":
            select(respond)
        default:
            respond.reject("the haptics module has no method \(method)")
        }
    }

    private static func support() -> [String: Any] {
        #if os(iOS)
            return ["available": true, "notification": true, "caveat": ""]
        #elseif os(watchOS)
            return ["available": true, "notification": true, "caveat": ""]
        #elseif os(macOS)
            return [
                "available": true,
                // `NSHapticFeedbackManager` has three patterns —generic,
                // alignment, level change— and none of them means success,
                // warning or error. Mapping all three onto "generic" would be
                // three different meanings coming out as one feeling.
                "notification": false,
                "caveat":
                    "on a Mac the feedback goes to a Force Touch trackpad, and AppKit offers no "
                    + "way to ask whether this Mac has one: on any other Mac nothing is felt. "
                    + "There are also no notification patterns — haptics.notification() is "
                    + "refused rather than answered with the wrong feeling."
            ]
        #else
            return [
                "available": false,
                "notification": false,
                "caveat": Self.absent
            ]
        #endif
    }

    /// The one sentence the two platforms without haptics are turned down with.
    /// Written once so the three methods cannot drift into saying it three
    /// different ways.
    #if os(tvOS)
        private static let absent =
            "tvOS has no haptics: a television has nothing to vibrate, and the Siri Remote has no "
            + "haptic engine an app can drive. Use a sound or a change on the screen."
    #elseif os(visionOS)
        private static let absent =
            "visionOS has no haptics: nothing is held and nothing touches the wrist, so there is "
            + "no haptic engine at all. Use sound, which in the headset is placed in space and "
            + "does much of the same work."
    #else
        private static let absent = "this platform has no haptics"
    #endif

    private func impact(_ style: String, _ respond: AnBuiltinCall) {
        #if os(iOS)
            let weight: UIImpactFeedbackGenerator.FeedbackStyle
            switch style {
            case "light": weight = .light
            case "heavy": weight = .heavy
            case "soft": weight = .soft
            case "rigid": weight = .rigid
            case "medium": weight = .medium
            default:
                respond.reject(
                    "haptics.impact does not know the style \(style): it is one of light, medium, "
                        + "heavy, soft or rigid")
                return
            }
            let generator = UIImpactFeedbackGenerator(style: weight)
            generator.prepare()
            generator.impactOccurred()
            respond.resolve()

        #elseif os(watchOS)
            // The watch has no weights. `.click` is what it has for "something
            // happened", and answering a lighter or heavier one by playing the
            // same thing would be the app being told it got what it asked for.
            WKInterfaceDevice.current().play(.click)
            respond.resolve()

        #elseif os(macOS)
            // `.levelChange` for a light tap, `.generic` for anything heavier.
            // Both reach a Force Touch trackpad and nothing else; see
            // `support()`, which says so before an app relies on it.
            let pattern: NSHapticFeedbackManager.FeedbackPattern =
                style == "light" ? .levelChange : .generic
            NSHapticFeedbackManager.defaultPerformer.perform(pattern, performanceTime: .now)
            respond.resolve()

        #else
            respond.reject(Self.absent)
        #endif
    }

    private func notify(_ type: String, _ respond: AnBuiltinCall) {
        #if os(iOS)
            let pattern: UINotificationFeedbackGenerator.FeedbackType
            switch type {
            case "success": pattern = .success
            case "warning": pattern = .warning
            case "error": pattern = .error
            default:
                respond.reject(
                    "haptics.notification does not know the type \(type): it is one of success, "
                        + "warning or error")
                return
            }
            notification.prepare()
            notification.notificationOccurred(pattern)
            respond.resolve()

        #elseif os(watchOS)
            switch type {
            case "success": WKInterfaceDevice.current().play(.success)
            case "warning": WKInterfaceDevice.current().play(.notification)
            case "error": WKInterfaceDevice.current().play(.failure)
            default:
                respond.reject(
                    "haptics.notification does not know the type \(type): it is one of success, "
                        + "warning or error")
                return
            }
            respond.resolve()

        #elseif os(macOS)
            respond.reject(
                "macOS has no notification haptics: NSHapticFeedbackManager has three patterns "
                    + "—generic, alignment and level change— and none of them means success, "
                    + "warning or error. Playing the same one for all three would be three "
                    + "meanings coming out as one feeling. Use haptics.impact() and say the rest "
                    + "on the screen.")

        #else
            respond.reject(Self.absent)
        #endif
    }

    private func select(_ respond: AnBuiltinCall) {
        #if os(iOS)
            selection.prepare()
            selection.selectionChanged()
            respond.resolve()
        #elseif os(watchOS)
            WKInterfaceDevice.current().play(.click)
            respond.resolve()
        #elseif os(macOS)
            // `.alignment` is exactly this: the tick a Mac gives as something
            // snaps into place.
            NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .now)
            respond.resolve()
        #else
            respond.reject(Self.absent)
        #endif
    }
}
