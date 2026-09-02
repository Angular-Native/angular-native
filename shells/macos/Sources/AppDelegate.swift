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
        if Screenshot.destino == nil {
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
