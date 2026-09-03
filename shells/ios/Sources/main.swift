// Entry point. No storyboard and no SceneDelegate: the app is a window with a
// single UIViewController from which hangs the tree Rust draws.
import UIKit

UIApplicationMain(
    CommandLine.argc,
    CommandLine.unsafeArgv,
    nil,
    NSStringFromClass(AppDelegate.self)
)
