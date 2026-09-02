import UIKit

@objc(AppDelegate)
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        #if os(visionOS)
            // La ventana la crea `SceneDelegate`, y no aquí.
            //
            // En visionOS no hay pantalla: `UIScreen` está marcado
            // `API_UNAVAILABLE(visionos)`, así que `UIScreen.main.bounds` ni
            // siquiera compila. Una app no ocupa una pantalla, ocupa una
            // ventana que el usuario cuelga en la habitación y redimensiona
            // cuando quiere, y de esa ventana solo se sabe a través de su
            // `UIWindowScene`. Por eso aquí la familia va por escenas y las
            // otras dos no.
            return true
        #else
            // iOS y tvOS: una ventana del tamaño de la pantalla, que existe y
            // no cambia mientras la app corre.
            let window = UIWindow(frame: UIScreen.main.bounds)
            window.rootViewController = RootViewController()
            window.makeKeyAndVisible()
            self.window = window
            return true
        #endif
    }
}
