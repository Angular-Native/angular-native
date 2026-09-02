import AppKit

@objc(AppDelegate)
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?
    private var root: RootViewController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let controller = RootViewController()
        // Una ventana de escritorio se puede redimensionar, y ese es el caso
        // que un teléfono no tiene: el viewport cambia mientras alguien
        // arrastra la esquina, no solo al girar el aparato. El tamaño de
        // partida es el de una ventana normal de Mac; a partir de ahí manda el
        // usuario, y el layout se recalcula en cada `viewDidLayout`.
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 820),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "angular-native"
        window.contentViewController = controller
        window.setFrameAutosaveName("AngularNativeMain")
        window.center()
        window.makeKeyAndOrderFront(nil)

        self.window = window
        self.root = controller
        // Robar el foco es lo correcto cuando alguien acaba de lanzar la app a
        // mano, y lo contrario cuando la lanza una comprobación: la ventana se
        // planta encima de lo que estuviera haciendo quien la ejecutó y le come
        // las pulsaciones. La captura no lo necesita —`cacheDisplay` dibuja la
        // vista esté delante o detrás—, así que en ese modo no se activa.
        //
        // Con una excepción: la captura del puntero. Las áreas de seguimiento
        // de este host son `ActiveInActiveApp`, porque en un Mac los controles
        // solo se iluminan al pasar por encima cuando la app está delante, y
        // una comprobación no puede pedirle al host que se comporte distinto
        // que el resto del escritorio. Se activa **aquí** y no al mover el
        // puntero: activarse tarda, y un ratón que entra en un área mientras la
        // app todavía no está activa no vuelve a entrar nunca —no se mueve otra
        // vez—, así que la comprobación salía bien o mal según lo que hubiera
        // tardado el sistema.
        if Screenshot.destino == nil || Screenshot.hover != nil {
            NSApp.activate(ignoringOtherApps: true)
        }
    }

    /// En un teléfono no hay «cerrar la ventana»: la app se va al fondo. En
    /// escritorio sí, y una app de una sola ventana que se queda corriendo sin
    /// nada que enseñar es una app zombi en el Dock.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}
