import AppKit

@objc(AppDelegate)
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?
    private var root: RootViewController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let controller = RootViewController()
        // A desktop window can be resized, and that is the case a phone does
        // not have: the viewport changes while somebody drags the corner, not
        // only when the device is rotated. The starting size is that of an
        // ordinary Mac window; from there on the user is in charge, and the
        // layout is recomputed on every `viewDidLayout`.
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 820),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "angular-native"
        window.contentViewController = controller
        window.setFrameAutosaveName("AngularNativeMain")
        window.center()
        window.makeKeyAndOrderFront(nil)

        self.window = window
        self.root = controller
        // Stealing the focus is the right thing when somebody has just launched
        // the app by hand, and the wrong thing when a check launches it: the
        // window plants itself on top of whatever the person running it was
        // doing and eats their keystrokes. The screenshot does not need it
        // —`cacheDisplay` draws the view whether it is in front or behind—, so
        // in that mode it does not activate.
        //
        // With one exception: the pointer screenshot. This host's tracking areas
        // are `ActiveInActiveApp`, because on a Mac controls only light up on
        // hover when the app is in front, and a check cannot ask the host to
        // behave differently from the rest of the desktop. It activates **here**
        // and not when moving the pointer: activating takes time, and a mouse
        // that enters an area while the app is not yet active never enters again
        // —it does not move a second time—, so the check came out right or wrong
        // depending on how long the system had taken.
        if Screenshot.destination == nil || Screenshot.hover != nil {
            NSApp.activate(ignoringOtherApps: true)
        }
    }

    /// On a phone there is no "close the window": the app goes to the
    /// background. On the desktop there is, and a single-window app left running
    /// with nothing to show is a zombie in the Dock.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}
