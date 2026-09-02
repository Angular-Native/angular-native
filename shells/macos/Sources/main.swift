// Punto de entrada. Sin xib y sin storyboard: la app es una ventana con un
// único NSViewController del que cuelga el árbol que dibuja Rust.
//
// `NSApplicationMain` no vale aquí porque necesita un `NSPrincipalClass` y un
// nib en el bundle. Montar la aplicación a mano son cinco líneas y deja el
// arranque a la vista, que es lo mismo que hace el shell de iOS con su
// `UIApplicationMain`.
import AppKit

let app = NSApplication.shared
// `.regular` es una app con icono en el Dock y con menú. Sin esto, un
// ejecutable lanzado fuera de un `.app` sale sin menú y sin poder hacerse
// frontal, y el fallo se ve como «la ventana no coge el foco».
app.setActivationPolicy(.regular)

let delegate = AppDelegate()
app.delegate = delegate
app.mainMenu = AnMenu.build()
app.run()
