import Foundation
import Observation

/// The bridge to Rust and the app's clock.
///
/// On iOS the frame is driven by a `CADisplayLink`, which does not exist on
/// watchOS. What there is is a `Timer`, at 30 Hz, which is what the watch draws
/// at while the app is in the foreground. Asking for 60 would only spend battery
/// for nothing.
///
/// The tick is not fired from any view's `body`. If it were, every frame would
/// mutate observable state in the middle of a recomposition and SwiftUI would
/// feed back into itself. The timer lives outside the view hierarchy and the
/// views only look at the tree.
@MainActor
@Observable
final class AnRuntime {
    let tree = AnTree()
    /// What the finger moves and the app has not confirmed yet. It lives here
    /// and not in a view because it has to survive the tree being rebuilt whole
    /// on every snapshot, which is what happens thirty times a second.
    let controls = AnControls()

    @ObservationIgnored private var runtime: OpaquePointer?
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var dev: DevClient?
    /// The app's clock starts counting at startup. The JS timers advance with
    /// these milliseconds, not with the system time.
    @ObservationIgnored private var epoch = Date()

    /// The natural sizes of the controls.
    ///
    /// On iOS a real `UISwitch` is asked. That cannot be done here: the controls
    /// are drawn by SwiftUI and there is no object to ask `sizeThatFits` before a
    /// view exists. They are fixed, with the measurements watchOS uses, and the
    /// width of the ones that stretch is decided later by the layout.
    ///
    /// The `Picker` is the tallest of them all because on the watch it is a
    /// wheel and not a dropdown: it needs to show the option above and the one
    /// below or it is not obvious that it turns.
    private static let controlSizes = """
    {"Button":[80,44],"Switch":[52,40],"Slider":[120,36],\
    "ProgressBar":[120,6],"ActivityIndicator":[24,24],\
    "Stepper":[120,44],"Picker":[120,88],"DatePicker":[120,44],"Icon":[24,24]}
    """

    func start(width: Double, height: Double) {
        guard runtime == nil else { return }
        runtime = an_watch_runtime_new(Float(width), Float(height), Self.controlSizes)
        guard let runtime else {
            NSLog("angular-native: an_watch_runtime_new returned nil")
            return
        }
        controls.dispatch = { [weak self] target, name, payload in
            self?.dispatch(target, name, payload)
        }
        loadBundleScript(into: runtime)
        // Only exists if the `.app` was built by `an dev`. In an ordinary build
        // the initialiser returns `nil` and nothing is left running here.
        dev = DevClient(bundle: .main) { [weak self] source in
            self?.reload(source)
        }
        dev?.connect()

        epoch = Date()
        let timer = Timer(timeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            // The timer is not isolated to the actor; the work does have to be,
            // because it touches the model SwiftUI observes.
            Task { @MainActor in self?.tick() }
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func setViewport(width: Double, height: Double) {
        guard let runtime else { return }
        an_watch_runtime_set_viewport(runtime, Float(width), Float(height))
    }

    private func tick() {
        guard let runtime else { return }
        let now = Date().timeIntervalSince(epoch) * 1000.0
        if an_watch_runtime_frame(runtime, now) < 0 {
            NSLog("angular-native: the frame failed")
        }
        // It only dumps if the revision changed: a settled frame does not wake
        // SwiftUI.
        if tree.sync(runtime: runtime) {
            controls.reconcile(root: tree.root, overlays: tree.overlays)
        }
    }

    /// The app bundle carries the JS, just like React Native's
    /// `main.jsbundle`.
    private func loadBundleScript(into runtime: OpaquePointer) {
        guard let path = Bundle.main.path(forResource: "main", ofType: "js"),
              let source = try? String(contentsOfFile: path, encoding: .utf8)
        else {
            NSLog("angular-native: there is no main.js in the bundle")
            return
        }
        if an_watch_runtime_eval(runtime, "main.js", source) != 0 {
            NSLog("angular-native: main.js threw while being evaluated")
        }
    }

    /// New code on top of the one already running. It is called by the
    /// development client from the main thread, which is the only one that may
    /// touch the runtime.
    ///
    /// The controls' state is deliberately not touched here. In a hot reload the
    /// tree is still standing with the same ids, so a slider halfway along stays
    /// where it was; in a cold reload the host empties the tree, and `reconcile`
    /// clears out what no longer exists without anybody having to remember to
    /// empty it by hand.
    private func reload(_ source: String) {
        guard let runtime else { return }
        if an_watch_runtime_reload(runtime, "main.js", source) != 0 {
            NSLog("angular-native: the reload failed")
        }
        if tree.sync(runtime: runtime) {
            controls.reconcile(root: tree.root, overlays: tree.overlays)
        }
    }

    /// An event from SwiftUI towards JS. It is queued here and JS sees it on the
    /// next tick, which is how events arrive on iOS and on Android.
    ///
    /// The payload travels as JSON because every event carries different keys: a
    /// `pan` six numbers, a `dismiss` none. It is serialised here, on the main
    /// thread and over a flat object, so it cannot fail for anything but a
    /// programming error —and if it does fail, it is said.
    func dispatch(_ target: UInt32, _ name: String, _ payload: [String: Any]) {
        guard let runtime else { return }
        guard !payload.isEmpty else {
            an_watch_runtime_event(runtime, target, name, nil)
            return
        }
        guard JSONSerialization.isValidJSONObject(payload),
              let data = try? JSONSerialization.data(withJSONObject: payload),
              let json = String(data: data, encoding: .utf8)
        else {
            NSLog("angular-native: the payload of \(name) cannot be serialised: \(payload)")
            return
        }
        an_watch_runtime_event(runtime, target, name, json)
    }

    deinit {
        timer?.invalidate()
        if let runtime {
            an_watch_runtime_free(runtime)
        }
    }
}
