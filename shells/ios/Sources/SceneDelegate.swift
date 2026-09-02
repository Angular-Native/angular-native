#if os(visionOS)

    import UIKit

    /// La ventana de visionOS.
    ///
    /// El nombre en Objective-C tiene que ser exactamente éste porque es el que
    /// va escrito en `UISceneDelegateClassName` del `Info.plist`. Sin `@objc`,
    /// Swift lo mangla con el módulo delante y UIKit no encuentra la clase: la
    /// escena se conecta, nadie crea la ventana y la app arranca en negro sin
    /// un solo error.
    @objc(SceneDelegate)
    final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
        var window: UIWindow?

        func scene(
            _ scene: UIScene,
            willConnectTo session: UISceneSession,
            options connectionOptions: UIScene.ConnectionOptions
        ) {
            guard let windowScene = scene as? UIWindowScene else { return }

            // Qué tamaño quiere la ventana al abrirse.
            //
            // No hay pantalla de la que deducirlo, así que si no se pide nada
            // el sistema elige por su cuenta y la app se encuentra un tamaño
            // que no eligió. Es una *preferencia*: visionOS puede darla o no,
            // y el usuario puede cambiarla a mano tirando de la esquina. Por
            // eso el viewport de verdad no sale de aquí, sale de
            // `viewDidLayoutSubviews`, que es quien ve el tamaño que acabó
            // teniendo.
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
