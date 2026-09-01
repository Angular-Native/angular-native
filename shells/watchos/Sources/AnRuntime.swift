import Foundation
import Observation

/// El puente con Rust y el reloj de la app.
///
/// En iOS el frame lo marca un `CADisplayLink`, que en watchOS no existe. Lo
/// que hay es un `Timer`, y a 30 Hz, que es a lo que dibuja el reloj mientras
/// la app está en primer plano. Pedir 60 solo gastaría batería para nada.
///
/// El turno no se dispara desde el `body` de ninguna vista. Si se hiciera, cada
/// frame mutaría estado observable en mitad de una recomposición y SwiftUI se
/// realimentaría consigo mismo. El temporizador vive fuera de la jerarquía de
/// vistas y ellas solo miran el árbol.
@MainActor
@Observable
final class AnRuntime {
    let tree = AnTree()

    @ObservationIgnored private var runtime: OpaquePointer?
    @ObservationIgnored private var timer: Timer?
    /// El reloj de la app empieza a contar al arrancar. Los temporizadores de
    /// JS avanzan con estos milisegundos, no con la hora del sistema.
    @ObservationIgnored private var epoch = Date()

    /// Tamaños naturales de los controles.
    ///
    /// En iOS se le pregunta a un `UISwitch` de verdad. Aquí no se puede: los
    /// controles los dibuja SwiftUI y no hay un objeto al que preguntarle
    /// `sizeThatFits` antes de que exista una vista. Van fijados, con las
    /// medidas que usa watchOS, y el ancho de los que se estiran lo decide
    /// luego el layout.
    private static let controlSizes = """
    {"Button":[80,44],"Switch":[52,32],"Slider":[120,32],\
    "ProgressBar":[120,4],"ActivityIndicator":[24,24]}
    """

    func start(width: Double, height: Double) {
        guard runtime == nil else { return }
        runtime = an_watch_runtime_new(Float(width), Float(height), Self.controlSizes)
        guard let runtime else {
            NSLog("angular-native: an_watch_runtime_new devolvió nil")
            return
        }
        loadBundleScript(into: runtime)

        epoch = Date()
        let timer = Timer(timeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            // El temporizador no está aislado al actor; el trabajo sí tiene que
            // estarlo, porque toca el modelo que SwiftUI observa.
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
            NSLog("angular-native: el frame falló")
        }
        // Solo vuelca si la revisión cambió: un frame quieto no despierta a
        // SwiftUI.
        tree.sync(runtime: runtime)
    }

    /// El bundle de la app trae el JS, igual que el `main.jsbundle` de React
    /// Native.
    private func loadBundleScript(into runtime: OpaquePointer) {
        guard let path = Bundle.main.path(forResource: "main", ofType: "js"),
              let source = try? String(contentsOfFile: path, encoding: .utf8)
        else {
            NSLog("angular-native: no hay main.js en el bundle")
            return
        }
        if an_watch_runtime_eval(runtime, "main.js", source) != 0 {
            NSLog("angular-native: main.js lanzó al evaluarse")
        }
    }

    /// Un toque de SwiftUI hacia JS. Se encola aquí y JS lo ve en el tick
    /// siguiente, que es como llegan los eventos en iOS y en Android.
    func dispatch(_ target: UInt32, _ name: String) {
        guard let runtime else { return }
        an_watch_runtime_event(runtime, target, name)
    }

    deinit {
        timer?.invalidate()
        if let runtime {
            an_watch_runtime_free(runtime)
        }
    }
}
