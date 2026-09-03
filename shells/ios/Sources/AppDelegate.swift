import UIKit

@objc(AppDelegate)
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
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
}
