import AppKit

/// The view the host mounts into. It is flipped for the same reason as the ones
/// the host itself creates (see `crates/an-macos/src/flipped.rs`): the core lays
/// out top to bottom and AppKit, by default, bottom to top.
final class AnRootView: NSView {
    override var isFlipped: Bool { true }
}

/// The whole macOS shell fits in here: create the runtime, give it a view to
/// mount into, tell it the size and call it once per frame.
///
/// It is the same file as the iOS `RootViewController` with two differences, and
/// both of them are desktop ones:
///
/// - `viewDidLayout` is called **while** the corner of the window is being
///   dragged, not only when the device is rotated. The viewport changes live and
///   the layout is redone on the next frame.
/// - The `CADisplayLink` is asked of the view and not of the screen: on a Mac
///   there are several screens and they can run at different refresh rates, so
///   the right clock is the one of the screen the window is on, and the view is
///   what knows that. It is added in `.common` mode so it keeps beating while
///   the window is dragged or a menu is open, which in AppKit run in a separate
///   event loop.
final class RootViewController: NSViewController {
    private var runtime: OpaquePointer?
    private var displayLink: CADisplayLink?
    private var devClient: DevClient?
    /// Only exists if somebody put `AN_SCREENSHOT` in the environment; see
    /// `Screenshot.swift`. It is `nil` in any normal run.
    private var screenshot: Screenshot.Trigger?

    override func loadView() {
        let root = AnRootView(frame: NSRect(x: 0, y: 0, width: 720, height: 820))
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor.black.cgColor
        view = root
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        // Before the runtime and not after: the core builds one module per
        // registered name the moment the engine starts, so anything registered
        // later would exist in this registry and nowhere else. That holds for
        // both kinds — the plugins an app depends on, and the built-ins, which
        // are not plugins and ship whether an app asks for them or not.
        AnPluginRegistry.install(host: self)
        AnBuiltinHost.view = view
        AnBuiltinModules.install()

        let bounds = view.bounds
        runtime = an_runtime_new(
            Unmanaged.passUnretained(view).toOpaque(),
            Float(bounds.width),
            Float(bounds.height)
        )
        guard runtime != nil else {
            assertionFailure("an_runtime_new returned nil: off the main thread?")
            return
        }
        loadBundleScript()

        let link = view.displayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link

        connectDevServer()
        screenshot = Screenshot.destination.map(Screenshot.Trigger.init(path:))
        // `AN_DUMP_TEXT` hangs off the screenshot's shutter: with no
        // destination there is no moment for it to fire. A variable that
        // silently does nothing is worse than one nobody reads, so it is said.
        if TextDump.wanted && screenshot == nil {
            NSLog("angular-native: AN_DUMP_TEXT needs AN_SCREENSHOT too; nothing will be dumped")
        }
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        // Called on every step of the drag. The Rust side discards repeated
        // sizes, so calling too often costs nothing.
        an_runtime_set_viewport(runtime, Float(view.bounds.width), Float(view.bounds.height))
    }

    /// The app bundle carries the JS, just like React Native's
    /// `main.jsbundle` — unless an update has been installed over it and has
    /// earned the right to run. `AnBundles` lives in `shells/shared`, so the
    /// Mac and the phone answer that question the same way.
    private func loadBundleScript() {
        let (source, version) = AnBundles.source()
        guard !source.isEmpty else { return }
        if version != "packaged" {
            NSLog("angular-native: running update \(version)")
        }
        if an_runtime_eval(runtime, "main.js", source) != 0 {
            NSLog("angular-native: main.js threw while being evaluated")
        }
    }

    /// Only exists if the `.app` was built by `an dev`. The client is the same
    /// one the phone and the watch use: it lives in `shells/shared` and is plain
    /// Foundation, with nothing from UIKit or AppKit.
    private func connectDevServer() {
        devClient = DevClient(bundle: .main) { [weak self] source in
            guard let self, let runtime = self.runtime else { return }
            NSLog("angular-native: reloading")
            if an_runtime_reload(runtime, "main.js", source) != 0 {
                NSLog("angular-native: the reloaded bundle threw while being evaluated")
            }
            self.screenshot?.reloaded()
        }
        devClient?.connect()
    }

    @objc private func tick(_ link: CADisplayLink) {
        // The app's clock is the vsync one: the JS timers advance with the
        // frames, not on a thread of their own.
        let applied = an_runtime_frame(runtime, link.timestamp * 1000.0)
        if applied < 0 {
            NSLog("angular-native: the frame failed")
        }
        screenshot?.frame(applied: applied, view: view)
    }

    deinit {
        displayLink?.invalidate()
        an_runtime_free(runtime)
    }
}
