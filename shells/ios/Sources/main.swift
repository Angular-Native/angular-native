// Punto de entrada. Sin storyboard y sin SceneDelegate: la app es una ventana
// con un único UIViewController del que cuelga el árbol que dibuja Rust.
import UIKit

UIApplicationMain(
    CommandLine.argc,
    CommandLine.unsafeArgv,
    nil,
    NSStringFromClass(AppDelegate.self)
)
