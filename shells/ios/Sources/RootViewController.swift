import UIKit

/// The whole iOS shell fits in here: create the runtime, give it a view to
/// mount into, tell it the size and call it once per frame.
final class RootViewController: UIViewController {
    private var runtime: OpaquePointer?
    private var displayLink: CADisplayLink?
    private var devClient: DevClient?

    override func viewDidLoad() {
        super.viewDidLoad()

        #if os(visionOS)
            // Transparent, not black.
            //
            // The visionOS window already brings a background: the glass the
            // system draws behind the app, with its blur and its shadow over the
            // real room. Painting black on top covers it entirely and leaves an
            // opaque slab floating in the living room. What shows as a
            // background is decided by the template with `[backgroundColor]`,
            // and whatever paints nothing lets the glass through.
            view.backgroundColor = .clear
        #else
            view.backgroundColor = .black
        #endif

        // Before creating the runtime: the core builds one native module per
        // plugin when the engine starts, and whatever is registered afterwards
        // does not get in.
        AnPluginRegistry.install(host: self)
        // And the modules the framework brings, which are not plugins: nobody
        // declares them and every host has them. They present from this
        // controller when they have something to show.
        AnBuiltinHost.viewController = self
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

        // The core only works when there is something to apply; on a settled
        // frame `an_runtime_frame` returns 0 without touching UIKit.
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link

        connectDevServer()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        an_runtime_set_viewport(runtime, Float(view.bounds.width), Float(view.bounds.height))
    }

    /// The app bundle carries the JS, just like React Native's
    /// `main.jsbundle` — unless an update has been installed over it and has
    /// earned the right to run. `AnBundles` decides which, and retires an
    /// update that failed to confirm itself on its one trial launch.
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

    /// Only exists if the `.app` was built by `an dev`.
    private func connectDevServer() {
        devClient = DevClient(bundle: .main) { [weak self] source in
            guard let self, let runtime = self.runtime else { return }
            NSLog("angular-native: reloading")
            if an_runtime_reload(runtime, "main.js", source) != 0 {
                NSLog("angular-native: the reloaded bundle threw while being evaluated")
            }
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
    }

    deinit {
        displayLink?.invalidate()
        an_runtime_free(runtime)
    }
}
