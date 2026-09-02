import AppKit

/// La captura que la app se hace a sí misma.
///
/// Existe porque en escritorio la prueba de que un host funciona es una imagen
/// de la ventana, y `screencapture` no sirve para automatizarla: pedir la
/// pantalla exige el permiso de grabación, que se concede a mano y por
/// aplicación. Una vista, en cambio, sabe dibujarse en un mapa de bits sin
/// pedirle permiso a nadie: `cacheDisplay(in:to:)` recorre la jerarquía y pinta
/// los controles de AppKit tal cual están, que es justo lo que hay que
/// comprobar.
///
/// Se activa con `AN_SCREENSHOT=<ruta>` en el entorno. Sin esa variable no hay
/// nada de esto en el camino del frame.
enum Screenshot {
    /// Frames que se dejan pasar desde el primer montaje hasta disparar.
    ///
    /// No es un adorno: cuando el núcleo manda las primeras altas, las vistas
    /// ya están puestas pero AppKit todavía no las ha dibujado, y los controles
    /// del sistema —el `NSSwitch`, el `NSProgressIndicator`— animan su estado
    /// inicial durante unas décimas. Disparar en el frame del montaje da una
    /// imagen de controles a medio pintar.
    private static let framesDeGracia = 40

    static var destino: String? {
        ProcessInfo.processInfo.environment["AN_SCREENSHOT"]
    }

    /// Cuántas recargas en caliente hay que esperar antes de disparar.
    ///
    /// Existe por un fallo concreto que ya se cometió una vez: una recarga en
    /// caliente desmontaba las vistas y dejaba la ventana en negro. Eso no se ve
    /// en ningún registro —la app sigue viva, los frames siguen pasando y nadie
    /// devuelve error—; solo se ve mirando la pantalla. Con
    /// `AN_SCREENSHOT_RELOADS=1` la captura no es la del arranque sino la de
    /// después de recargar, que es la que enseña si el árbol sigue en pie.
    static var recargasQueEsperar: Int {
        Int(ProcessInfo.processInfo.environment["AN_SCREENSHOT_RELOADS"] ?? "") ?? 0
    }

    /// Escribe la vista en PNG. Devuelve `false` y dice por qué si no puede:
    /// una captura que no se guarda y no avisa deja una comprobación en verde
    /// mirando un fichero de la ejecución anterior.
    static func write(_ view: NSView, to path: String) -> Bool {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            NSLog("angular-native: la vista no pudo dar un mapa de bits")
            return false
        }
        view.cacheDisplay(in: view.bounds, to: rep)
        guard let png = rep.representation(using: .png, properties: [:]) else {
            NSLog("angular-native: el mapa de bits no se pudo codificar en PNG")
            return false
        }
        do {
            try png.write(to: URL(fileURLWithPath: path))
        } catch {
            NSLog("angular-native: no se pudo escribir \(path): \(error)")
            return false
        }
        // El recuento de colores va en el mismo renglón porque es lo que
        // convierte la captura en una comprobación automática: un fichero PNG
        // del tamaño correcto y completamente negro pesa lo suyo y pasa
        // cualquier prueba que mire el tamaño. Si aquí sale 1, la ventana no
        // pintó nada y el script lo puede decir sin abrir la imagen.
        NSLog(
            "angular-native: captura escrita en \(path) (\(rep.pixelsWide)x\(rep.pixelsHigh), \(colores(rep)) colores)"
        )
        return true
    }

    /// Cuántos colores distintos hay en el mapa de bits.
    ///
    /// Se muestrea uno de cada cuatro píxeles por filas y columnas: son
    /// dieciséis veces menos trabajo y cualquier control del sistema ocupa
    /// bastante más que un píxel, así que ninguno se escapa por el muestreo.
    private static func colores(_ rep: NSBitmapImageRep) -> Int {
        guard let base = rep.bitmapData else { return 0 }
        let porPixel = rep.bitsPerPixel / 8
        guard porPixel >= 3 else { return 0 }
        var vistos = Set<UInt32>()
        for y in stride(from: 0, to: rep.pixelsHigh, by: 4) {
            let fila = base + y * rep.bytesPerRow
            for x in stride(from: 0, to: rep.pixelsWide, by: 4) {
                let p = fila + x * porPixel
                vistos.insert(UInt32(p[0]) << 16 | UInt32(p[1]) << 8 | UInt32(p[2]))
            }
        }
        return vistos.count
    }

    /// Lleva la cuenta de los frames desde el primer montaje.
    ///
    /// Se separa del controlador para que el camino normal del frame —el que
    /// corre sesenta veces por segundo en cualquier app— sea una comparación
    /// contra `nil` y nada más.
    final class Disparador {
        /// Frames que se esperan a que el núcleo monte algo antes de darlo por
        /// perdido. Diez segundos a 60 Hz. Sin este tope, una app que no
        /// arranca deja la ventana en negro y el script esperando para siempre,
        /// que es la forma más cara de fallar en silencio.
        private static let framesDeEspera = 600

        private let path: String
        private var montado = false
        private var frames = 0
        /// Recargas que quedan por ver. Mientras no llegue a cero, el disparador
        /// mira pasar los frames y no cuenta ninguno.
        private var recargasPendientes: Int

        init(path: String) {
            self.path = path
            self.recargasPendientes = Screenshot.recargasQueEsperar
        }

        /// El shell acaba de coser un bundle nuevo sobre el que corría.
        ///
        /// Se rearma el conteo desde cero: lo que interesa capturar es lo que
        /// hay en pantalla *después* de la recarga, y las vistas que sobreviven
        /// a una recarga en caliente no se vuelven a montar, así que la espera
        /// de aquí en adelante no puede exigir que llegue ninguna alta.
        func recargado() {
            guard recargasPendientes > 0 else { return }
            recargasPendientes -= 1
            frames = 0
            montado = recargasPendientes == 0
        }

        /// `applied` es lo que devolvió `an_runtime_frame`. Cuando le toca,
        /// termina el proceso: el código de salida distingue las tres cosas que
        /// le importan a quien lo llamó desde un script —la app montó y la
        /// captura se guardó, la captura falló, o no llegó a montar nada—.
        func frame(applied: Int32, view: NSView) {
            // Con recargas pendientes no hay nada que contar: la captura la
            // dispara `recargado()`, no el arranque.
            guard recargasPendientes == 0 else { return }
            frames += 1
            if !montado {
                guard applied > 0 else {
                    if frames >= Self.framesDeEspera {
                        NSLog("angular-native: %d frames sin montar nada; no hay nada que capturar", frames)
                        exit(2)
                    }
                    return
                }
                montado = true
                frames = 0
            }
            guard frames >= Screenshot.framesDeGracia else { return }
            exit(Screenshot.write(view, to: path) ? 0 : 1)
        }
    }
}
