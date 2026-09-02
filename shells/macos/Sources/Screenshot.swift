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
    private static var framesDeGracia: Int {
        Int(ProcessInfo.processInfo.environment["AN_SCREENSHOT_FRAMES"] ?? "") ?? 40
    }

    static var destino: String? {
        ProcessInfo.processInfo.environment["AN_SCREENSHOT"]
    }

    /// Dónde poner el puntero antes de disparar, en puntos de la vista raíz.
    ///
    /// Sin esto no hay forma de fotografiar un `(hover)`: el realce lo produce
    /// el sistema cuando el ratón entra de verdad en el área vigilada, y en una
    /// comprobación no hay nadie moviendo el ratón. `CGWarpMouseCursorPosition`
    /// lo mueve sin pedir ningún permiso —no es `CGEventPost`, que sí exige
    /// accesibilidad—, así que lo que entra en el área es el puntero real y lo
    /// que sale en la imagen es el `NSTrackingArea` de verdad haciendo su
    /// trabajo, no un estado puesto a mano.
    static var hover: CGPoint? {
        guard let bruto = ProcessInfo.processInfo.environment["AN_SCREENSHOT_HOVER"] else {
            return nil
        }
        let partes = bruto.split(separator: ",").compactMap { Double($0) }
        guard partes.count == 2 else {
            NSLog("angular-native: AN_SCREENSHOT_HOVER se escribe x,y (y son puntos de la ventana)")
            return nil
        }
        return CGPoint(x: partes[0], y: partes[1])
    }

    /// Un botón que pulsar antes de disparar, en `x,y`.
    ///
    /// Hay estados de una app que no existen al arrancar y que son justo los
    /// que hay que fotografiar: un vídeo sonando, por ejemplo, empieza cuando
    /// alguien le da a reproducir. Esto pulsa el control del sistema que haya
    /// bajo el punto, por su propio camino —`performClick:`—, no simulando el
    /// ratón.
    static var pulsacion: CGPoint? {
        guard let bruto = ProcessInfo.processInfo.environment["AN_SCREENSHOT_PRESS"] else {
            return nil
        }
        let partes = bruto.split(separator: ",").compactMap { Double($0) }
        guard partes.count == 2 else {
            NSLog("angular-native: AN_SCREENSHOT_PRESS se escribe x,y")
            return nil
        }
        return CGPoint(x: partes[0], y: partes[1])
    }

    /// Un deslizamiento, en `x,y,deltaX,deltaY`.
    ///
    /// Es lo único de este host que no se puede provocar de verdad sin un
    /// trackpad y una mano encima: el gesto lo reconoce el sistema, no la app,
    /// y no hay forma de pedirle que lo reconozca. Lo que sí se puede es entrar
    /// por la misma puerta por la que entra él —`swipeWithEvent:` sobre la
    /// vista que hay bajo el punto— con los mismos deltas que él manda, y ver
    /// si de ahí en adelante todo funciona: la vista lo recoge o lo pasa a su
    /// padre, el evento cruza al motor y la plantilla se recompone.
    ///
    /// Lo que esto **no** comprueba es que un gesto de dos dedos acabe en un
    /// `swipeWithEvent:`. Eso lo decide el sistema y hay que mirarlo con la
    /// mano. Lo que sí comprueba es todo lo demás, que es lo que se puede
    /// romper editando este repositorio.
    static var deslizamiento: (punto: CGPoint, dx: Double, dy: Double)? {
        guard let bruto = ProcessInfo.processInfo.environment["AN_SCREENSHOT_SWIPE"] else {
            return nil
        }
        let partes = bruto.split(separator: ",").compactMap { Double($0) }
        guard partes.count == 4 else {
            NSLog("angular-native: AN_SCREENSHOT_SWIPE se escribe x,y,deltaX,deltaY")
            return nil
        }
        return (CGPoint(x: partes[0], y: partes[1]), partes[2], partes[3])
    }

    /// Fotografiar la ventana entera, barra de título incluida.
    ///
    /// La captura normal es la del contenido, que es lo que monta el núcleo.
    /// Pero en macOS hay una cosa del árbol que **no** está en el contenido: el
    /// `[title]` de un `<an-navigation-bar>`, que acaba en la barra de título.
    /// Para verlo hay que dibujar la vista de arriba del todo, que es la que
    /// AppKit usa para el marco de la ventana.
    static var conMarco: Bool {
        ProcessInfo.processInfo.environment["AN_SCREENSHOT_WINDOW"] == "1"
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
    static func write(_ raiz: NSView, to path: String) -> Bool {
        // El marco de la ventana es el ancestro de la vista de contenido. No
        // hace falta pedirle la pantalla a nadie: sigue siendo una `NSView` que
        // sabe dibujarse en un mapa de bits.
        let view = conMarco ? (raiz.window?.contentView?.superview ?? raiz) : raiz
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

    /// Lleva el puntero de verdad al punto que pidió `AN_SCREENSHOT_HOVER`.
    ///
    /// Hay dos cosas que hacer y las dos hacen falta. Una es mover el puntero,
    /// que es lo que hace que entre en el `NSTrackingArea`. La otra es poner la
    /// app delante: las áreas de este host son `ActiveInActiveApp`, porque en
    /// un Mac los controles solo se iluminan al pasar por encima cuando la app
    /// está activa, y una comprobación no puede pedir que el host se comporte
    /// distinto que el resto del escritorio. Es la única captura que roba el
    /// foco, y solo la pide quien quiere fotografiar el puntero.
    ///
    /// Lo que pase después —que AppKit note el área nueva y reparta el
    /// `mouseEntered:`— lo hace el bucle de eventos normal de la app, que sigue
    /// corriendo. Por eso esto se llama a mitad de la espera y no justo antes
    /// de disparar.
    ///
    /// La app se pone delante al arrancar y no aquí: ver `AppDelegate`.
    static func ponerElPuntero(en view: NSView) {
        guard let punto = hover, let window = view.window else { return }
        // La app ya se activó al arrancar (ver `AppDelegate`); esto es por si
        // algo se la llevó delante mientras tanto.
        NSApp.activate(ignoringOtherApps: true)
        // Dónde estaba el ratón de quien lanzó la comprobación. Se le devuelve
        // en cuanto la foto está hecha: mover el puntero de alguien y dejarlo
        // donde te vino bien es lo mismo que romperle lo que estaba haciendo.
        dondeEstabaElPuntero = NSEvent.mouseLocation

        let enVentana = view.convert(punto, to: nil)
        let enPantalla = window.convertPoint(toScreen: enVentana)
        // `CGWarpMouseCursorPosition` trabaja en coordenadas de pantalla con el
        // origen arriba; AppKit las da con el origen abajo.
        let alto = NSScreen.screens.first?.frame.height ?? 0
        CGWarpMouseCursorPosition(CGPoint(x: enPantalla.x, y: alto - enPantalla.y))
        // Un warp deja un intervalo en el que el sistema no asocia el ratón
        // con el puntero. Sin cerrarlo, el movimiento no se reparte y el área
        // no se entera de que hay alguien encima.
        CGAssociateMouseAndMouseCursorPosition(1)
    }

    /// Dónde estaba el puntero antes de que esto lo moviera.
    private static var dondeEstabaElPuntero: CGPoint?

    /// Le devuelve el ratón a quien lo tenía.
    static func devolverElPuntero() {
        guard let punto = dondeEstabaElPuntero else { return }
        dondeEstabaElPuntero = nil
        let alto = NSScreen.screens.first?.frame.height ?? 0
        CGWarpMouseCursorPosition(CGPoint(x: punto.x, y: alto - punto.y))
        CGAssociateMouseAndMouseCursorPosition(1)
    }

    /// Pulsa el control del sistema que haya bajo el punto.
    ///
    /// Sube por los padres hasta encontrar un `NSControl` porque lo que hay
    /// justo bajo el punto suele ser la celda o el rótulo de dentro del botón,
    /// no el botón. Un `<an-view>` con `(press)` no entra aquí: ese va por un
    /// reconocedor de gestos, y esto solo sabe de controles.
    static func pulsar(en view: NSView) {
        guard let punto = pulsacion else { return }
        var destino = view.hitTest(view.convert(punto, to: view.superview))
        while let actual = destino, !(actual is NSControl) {
            destino = actual.superview
        }
        guard let control = destino as? NSControl else {
            NSLog("angular-native: no hay ningún control del sistema en %@", "\(punto)")
            return
        }
        control.performClick(nil)
    }

    /// Manda un deslizamiento a la vista que haya bajo el punto.
    ///
    /// El evento se arma con un `CGEvent` de rueda porque es la única forma de
    /// tener un `NSEvent` con los deltas puestos; lo que lo convierte en un
    /// deslizamiento no es el tipo, es a qué método se entrega. Y se entrega al
    /// resultado de `hitTest:`, que es lo que hace el sistema: si esa vista no
    /// lo atiende, sube a su padre por la cadena de responder.
    static func deslizar(en view: NSView) {
        guard let (punto, dx, dy) = deslizamiento else { return }
        guard let cg = CGEvent(source: nil) else {
            NSLog("angular-native: no se pudo armar el evento de deslizamiento")
            return
        }
        cg.type = .scrollWheel
        cg.setDoubleValueField(.scrollWheelEventDeltaAxis1, value: dy)
        cg.setDoubleValueField(.scrollWheelEventDeltaAxis2, value: dx)
        guard let evento = NSEvent(cgEvent: cg) else {
            NSLog("angular-native: el evento de deslizamiento no se pudo convertir")
            return
        }
        // `hitTest:` quiere el punto en las coordenadas del padre de la vista.
        let destino = view.hitTest(view.convert(punto, to: view.superview)) ?? view
        destino.swipe(with: evento)
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
            // El puntero se pone a mitad de la espera, no justo antes de
            // disparar. Entre que entra en el área vigilada y que el realce
            // está en pantalla hay tres pasos —AppKit reparte el
            // `mouseEntered:`, el evento cruza al motor, Angular recompone—, y
            // los tres pasan solos con dejar correr los frames. Forzar el
            // bucle de eventos desde aquí sería reentrar en este método.
            if frames == max(1, Screenshot.framesDeGracia / 4) {
                Screenshot.pulsar(en: view)
            }
            if frames == max(1, Screenshot.framesDeGracia / 3) {
                Screenshot.deslizar(en: view)
            }
            if frames == max(1, Screenshot.framesDeGracia / 2) {
                Screenshot.ponerElPuntero(en: view)
            }
            guard frames >= Screenshot.framesDeGracia else { return }
            let escrita = Screenshot.write(view, to: path)
            Screenshot.devolverElPuntero()
            exit(escrita ? 0 : 1)
        }
    }
}
