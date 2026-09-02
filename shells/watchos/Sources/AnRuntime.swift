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
    /// Lo que el dedo mueve y la app todavía no ha confirmado. Vive aquí y no
    /// en una vista porque tiene que sobrevivir a que el árbol se rehaga
    /// entero en cada foto, que es lo que pasa treinta veces por segundo.
    let controls = AnControls()

    @ObservationIgnored private var runtime: OpaquePointer?
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var dev: DevClient?
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
    ///
    /// El `Picker` es el más alto de todos porque en el reloj es una rueda, no
    /// un desplegable: necesita ver la opción de arriba y la de abajo o no se
    /// entiende que gira.
    private static let controlSizes = """
    {"Button":[80,44],"Switch":[52,40],"Slider":[120,36],\
    "ProgressBar":[120,6],"ActivityIndicator":[24,24],\
    "Stepper":[120,44],"Picker":[120,88],"DatePicker":[120,44],"Icon":[24,24]}
    """

    func start(width: Double, height: Double) {
        guard runtime == nil else { return }
        runtime = an_watch_runtime_new(Float(width), Float(height), Self.controlSizes)
        guard let runtime else {
            NSLog("angular-native: an_watch_runtime_new devolvió nil")
            return
        }
        controls.dispatch = { [weak self] target, name, payload in
            self?.dispatch(target, name, payload)
        }
        loadBundleScript(into: runtime)
        // Solo existe si el `.app` lo armó `an dev`. En una compilación normal
        // el inicializador devuelve `nil` y aquí no queda nada encendido.
        dev = DevClient(bundle: .main) { [weak self] source in
            self?.reload(source)
        }
        dev?.connect()

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
        if tree.sync(runtime: runtime) {
            controls.reconcile(root: tree.root, overlays: tree.overlays)
        }
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

    /// Código nuevo encima del que ya corre. Lo llama el cliente de desarrollo
    /// desde el hilo principal, que es el único que puede tocar el runtime.
    ///
    /// El estado de los controles no se toca aquí a propósito. En una recarga
    /// en caliente el árbol sigue en pie con los mismos ids, así que un
    /// deslizador a medio camino se queda donde estaba; en una recarga en frío
    /// el host vacía el árbol, y `reconcile` se lleva por delante lo que ya no
    /// existe sin que haya que acordarse de vaciarlo a mano.
    private func reload(_ source: String) {
        guard let runtime else { return }
        if an_watch_runtime_reload(runtime, "main.js", source) != 0 {
            NSLog("angular-native: la recarga falló")
        }
        if tree.sync(runtime: runtime) {
            controls.reconcile(root: tree.root, overlays: tree.overlays)
        }
    }

    /// Un evento de SwiftUI hacia JS. Se encola aquí y JS lo ve en el tick
    /// siguiente, que es como llegan los eventos en iOS y en Android.
    ///
    /// La carga va como JSON porque cada evento lleva claves distintas: un
    /// `pan` seis números, un `dismiss` ninguno. Se serializa aquí, en el hilo
    /// principal y sobre un objeto plano, así que no puede fallar por nada que
    /// no sea un error de programación — y si falla, se dice.
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
            NSLog("angular-native: la carga de \(name) no se puede serializar: \(payload)")
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
