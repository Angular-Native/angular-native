#if os(visionOS)

    import UIKit

    /// The visionOS window.
    ///
    /// The Objective-C name has to be exactly this one because it is what is
    /// written in the `Info.plist`'s `UISceneDelegateClassName`. Without
    /// `@objc`, Swift mangles it with the module in front and UIKit cannot find
    /// the class: the scene connects, nobody creates the window and the app
    /// starts up black without a single error.
    @objc(SceneDelegate)
    final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
        var window: UIWindow?

        func scene(
            _ scene: UIScene,
            willConnectTo session: UISceneSession,
            options connectionOptions: UIScene.ConnectionOptions
        ) {
            guard let windowScene = scene as? UIWindowScene else { return }

            // What size the window wants when it opens.
            //
            // There is no screen to deduce it from, so if nothing is asked for
            // the system picks on its own and the app finds itself with a size
            // it did not choose. It is a *preference*: visionOS may grant it or
            // not, and the user can change it by hand by pulling the corner.
            // That is why the real viewport does not come from here, it comes
            // from `viewDidLayoutSubviews`, which is what sees the size it ended
            // up with.
            windowScene.requestGeometryUpdate(
                .Vision(size: CGSize(width: 1280, height: 720))
            )

            let window = UIWindow(windowScene: windowScene)
            window.rootViewController = RootViewController()
            window.makeKeyAndVisible()
            self.window = window
        }
    }

#endif
