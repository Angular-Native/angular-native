import AppKit

/// La vista donde monta el host. Va volteada por lo mismo que las que crea el
/// propio host (ver `crates/an-macos/src/flipped.rs`): el núcleo coloca de
/// arriba abajo y AppKit, por defecto, de abajo arriba.
final class AnRootView: NSView {
    override var isFlipped: Bool { true }
}

/// Todo el shell de macOS cabe aquí: crear el runtime, darle una vista donde
/// montar, avisarle del tamaño y llamarle una vez por frame.
///
/// Es el mismo fichero que el `RootViewController` de iOS con dos diferencias,
/// y las dos son del escritorio:
///
/// - `viewDidLayout` se llama **mientras** se arrastra la esquina de la
///   ventana, no solo al girar el aparato. El viewport cambia en caliente y el
///   layout se rehace en el frame siguiente.
/// - El `CADisplayLink` se pide a la vista y no a la pantalla: en un Mac hay
///   varias pantallas y pueden ir a refrescos distintos, así que el reloj
///   correcto es el de la pantalla donde está la ventana, y eso lo sabe la
///   vista. Se añade en modo `.common` para que siga latiendo mientras se
///   arrastra la ventana o se abre un menú, que en AppKit corren en un bucle de
///   eventos aparte.
final class RootViewController: NSViewController {
    private var runtime: OpaquePointer?
    private var displayLink: CADisplayLink?
    private var devClient: DevClient?
    /// Solo existe si alguien puso `AN_SCREENSHOT` en el entorno; ver
    /// `Screenshot.swift`. Es `nil` en cualquier ejecución normal.
    private var screenshot: Screenshot.Disparador?

    override func loadView() {
        let root = AnRootView(frame: NSRect(x: 0, y: 0, width: 720, height: 820))
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor.black.cgColor
        view = root
    }

    override func viewDidLoad() {
        super.viewDidLoad()

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

        let link = view.displayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link

        connectDevServer()
        screenshot = Screenshot.destino.map(Screenshot.Disparador.init(path:))
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        // Se llama en cada paso del arrastre. El lado de Rust descarta los
        // tamaños repetidos, así que llamar de más no cuesta nada.
        an_runtime_set_viewport(runtime, Float(view.bounds.width), Float(view.bounds.height))
    }

    /// El bundle de la app trae el JS, igual que el `main.jsbundle` de React
    /// Native.
    private func loadBundleScript() {
        guard let path = Bundle.main.path(forResource: "main", ofType: "js"),
              let source = try? String(contentsOfFile: path, encoding: .utf8)
        else {
            NSLog("angular-native: no hay main.js en el bundle")
            return
        }
        if an_runtime_eval(runtime, "main.js", source) != 0 {
            NSLog("angular-native: main.js lanzó al evaluarse")
        }
    }

    /// Solo existe si el `.app` lo armó `an dev`. El cliente es el mismo que
    /// usan el teléfono y el reloj: vive en `shells/shared` y es Foundation
    /// pelado, sin nada de UIKit ni de AppKit.
    private func connectDevServer() {
        devClient = DevClient(bundle: .main) { [weak self] source in
            guard let self, let runtime = self.runtime else { return }
            NSLog("angular-native: recargando")
            if an_runtime_reload(runtime, "main.js", source) != 0 {
                NSLog("angular-native: el bundle recargado lanzó al evaluarse")
            }
            self.screenshot?.recargado()
        }
        devClient?.connect()
    }

    @objc private func tick(_ link: CADisplayLink) {
        // El reloj de la app es el del vsync: los temporizadores de JS avanzan
        // con los frames, no con un hilo aparte.
        let applied = an_runtime_frame(runtime, link.timestamp * 1000.0)
        if applied < 0 {
            NSLog("angular-native: el frame falló")
        }
        screenshot?.frame(applied: applied, view: view)
    }

    deinit {
        displayLink?.invalidate()
        an_runtime_free(runtime)
    }
}
