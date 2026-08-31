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
        loadBundleScript()

        // El core solo trabaja cuando hay algo que aplicar; en un frame
        // quieto `an_runtime_frame` devuelve 0 sin tocar UIKit.
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        an_runtime_set_viewport(runtime, Float(view.bounds.width), Float(view.bounds.height))
    }

    /// El bundle de la app trae el JS, igual que el `main.jsbundle` de React
    /// Native. Si falta, se cae al árbol de demostración construido en Rust:
    /// así se distingue un fallo del puente de uno del renderer.
    private func loadBundleScript() {
        guard let path = Bundle.main.path(forResource: "main", ofType: "js"),
              let source = try? String(contentsOfFile: path, encoding: .utf8)
        else {
            NSLog("angular-native: no hay main.js en el bundle, cargando la demo de Rust")
            an_runtime_load_demo(runtime)
            return
        }
        if an_runtime_eval(runtime, "main.js", source) != 0 {
            NSLog("angular-native: main.js lanzó al evaluarse")
        }
    }

    @objc private func tick(_ link: CADisplayLink) {
        // El reloj de la app es el del vsync: los temporizadores de JS avanzan
        // con los frames, no con un hilo aparte.
        let applied = an_runtime_frame(runtime, link.timestamp * 1000.0)
        if applied < 0 {
            NSLog("angular-native: el frame falló")
        }
    }

    deinit {
        displayLink?.invalidate()
        an_runtime_free(runtime)
    }
}
