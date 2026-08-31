import UIKit

/// Todo el shell de iOS cabe aquí: crear el runtime, darle una vista donde
/// montar, avisarle del tamaño y llamarle una vez por frame.
final class RootViewController: UIViewController {
    private var runtime: OpaquePointer?
    private var displayLink: CADisplayLink?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black

        let bounds = view.bounds
        runtime = an_runtime_new(
            Unmanaged.passUnretained(view).toOpaque(),
            Float(bounds.width),
            Float(bounds.height)
        )
        guard runtime != nil else {
            assertionFailure("an_runtime_new devolvió nil: ¿fuera del hilo principal?")
            return
        }
        an_runtime_load_demo(runtime)

        // El core solo trabaja cuando hay algo que aplicar; en un frame
        // quieto `an_runtime_frame` devuelve 0 sin tocar UIKit.
        let link = CADisplayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        an_runtime_set_viewport(runtime, Float(view.bounds.width), Float(view.bounds.height))
    }

    @objc private func tick() {
        let applied = an_runtime_frame(runtime)
        if applied < 0 {
            NSLog("angular-native: el commit falló")
        }
    }

    deinit {
        displayLink?.invalidate()
        an_runtime_free(runtime)
    }
}
