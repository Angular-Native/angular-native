// Entry point. No xib and no storyboard: the app is a window with a single
// NSViewController from which hangs the tree Rust draws.
//
// `NSApplicationMain` is no good here because it needs an `NSPrincipalClass` and
// a nib in the bundle. Setting the application up by hand is five lines and
// leaves the startup in plain sight, which is the same thing the iOS shell does
// with its `UIApplicationMain`.
import AppKit

let app = NSApplication.shared
// `.regular` is an app with an icon in the Dock and a menu. Without this, an
// executable launched outside an `.app` comes up with no menu and unable to
// bring itself to the front, and the bug looks like "the window will not take
// the focus".
app.setActivationPolicy(.regular)

let delegate = AppDelegate()
app.delegate = delegate
app.mainMenu = AnMenu.build()
app.run()
