import UIKit

@objc(AppDelegate)
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Before the window, and that ordering is the whole point.
        //
        // Creating the root view controller is what evaluates the bundle, and
        // the bundle reads the pending links synchronously as it boots. A URL
        // handed over after this line still reaches the app —as an event— but
        // it reaches it one navigation late: the home screen would appear and
        // then be pushed aside. Handed over before, the router's very first
        // decision is already the right one.
        //
        // iOS calls `application(_:open:options:)` afterwards for the same URL.
        // That is not a duplicate delivery: the app is already on that route by
        // then and a link to where you already are does nothing.
        AnDeepLinks.take(fromLaunchOptions: launchOptions)

        #if os(visionOS)
            // The window is created by `SceneDelegate`, and not here.
            //
            // On visionOS there is no screen: `UIScreen` is marked
            // `API_UNAVAILABLE(visionos)`, so `UIScreen.main.bounds` does not
            // even compile. An app does not take up a screen, it takes up a
            // window the user hangs in the room and resizes whenever they like,
            // and that window can only be known about through its
            // `UIWindowScene`. That is why this family goes through scenes and
            // the other two do not.
            return true
        #else
            // iOS and tvOS: a window the size of the screen, which exists and
            // does not change while the app runs.
            let window = UIWindow(frame: UIScreen.main.bounds)
            window.rootViewController = RootViewController()
            window.makeKeyAndVisible()
            self.window = window
            return true
        #endif
    }

    /// A custom scheme, on an app that is already running — and, on a cold
    /// start, again for the URL `didFinishLaunching` already took.
    func application(
        _ application: UIApplication,
        open url: URL,
        options: [UIApplication.OpenURLOptionsKey: Any] = [:]
    ) -> Bool {
        AnDeepLinks.open(url)
        return true
    }

    /// A universal link: the person tapped an `https://` address the app owns
    /// and iOS handed it over instead of opening Safari.
    ///
    /// It only reaches here if the app declares `associated-domains` and Apple
    /// could fetch the `apple-app-site-association` file from the site. Without
    /// that the link opens in the browser and this is never called, which is
    /// the single most common reason a universal link "does not work".
    func application(
        _ application: UIApplication,
        continue userActivity: NSUserActivity,
        restorationHandler: @escaping ([UIUserActivityRestoring]?) -> Void
    ) -> Bool {
        guard userActivity.activityType == NSUserActivityTypeBrowsingWeb,
              let url = userActivity.webpageURL
        else {
            return false
        }
        AnDeepLinks.open(url)
        return true
    }
}

/// The one place a URL crosses into the core, so that the four ways iOS can
/// deliver one —launch options, `open:`, a user activity, a scene— do not each
/// grow their own idea of what a link is.
enum AnDeepLinks {
    static func open(_ url: URL) {
        // The absolute string and not the components: what the route means is
        // the app's business, and the core hands it to Angular's router whole.
        an_deeplink_open(url.absoluteString)
    }

    /// The URL the app was launched with, if it was launched with one.
    ///
    /// Both shapes arrive through here on a cold start: a custom scheme under
    /// `.url`, and a universal link as an `NSUserActivity` inside the activity
    /// dictionary. The dictionary's key is private to UIKit, so the activity is
    /// found by type rather than by name.
    static func take(fromLaunchOptions options: [UIApplication.LaunchOptionsKey: Any]?) {
        guard let options else { return }
        if let url = options[.url] as? URL {
            open(url)
        }
        if let activities = options[.userActivityDictionary] as? [AnyHashable: Any] {
            for value in activities.values {
                guard let activity = value as? NSUserActivity,
                      activity.activityType == NSUserActivityTypeBrowsingWeb,
                      let url = activity.webpageURL
                else {
                    continue
                }
                open(url)
            }
        }
    }

    /// The same two shapes, as a scene gets them. visionOS only: the other two
    /// families have no scene delegate.
    static func take(fromConnectionOptions options: UIScene.ConnectionOptions) {
        for context in options.urlContexts {
            open(context.url)
        }
        for activity in options.userActivities
        where activity.activityType == NSUserActivityTypeBrowsingWeb {
            if let url = activity.webpageURL {
                open(url)
            }
        }
    }
}
