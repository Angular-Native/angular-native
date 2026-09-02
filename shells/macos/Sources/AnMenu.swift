import AppKit

/// El menú de la app.
///
/// **Es del sistema y no se expone a Angular.** En un teléfono no hay menú; en
/// un Mac lo hay siempre, vive en la barra de arriba y no dentro de la ventana,
/// así que no cabe en el árbol de vistas que el núcleo monta: no hay ningún
/// `NodeKind` al que corresponda y no se puede inventar uno sin abrir
/// `packages/primitives`, que es de otro. Lo pone el shell y punto.
///
/// Lo que sí hace falta es que **esté**, y con las entradas de verdad. Un menú
/// vacío no es un detalle estético: los atajos de edición de macOS —⌘X, ⌘C,
/// ⌘V, ⌘Z, ⌘A— no los implementa `NSTextField`, los reparte el menú por la
/// cadena de responder. Sin menú de Edición, copiar y pegar en un
/// `<an-text-input>` no funciona, sin ningún error y sin nada que mirar. Es
/// exactamente el tipo de fallo silencioso que este repositorio no admite, y
/// por eso el menú mínimo incluye Edición y no solo Salir.
///
/// Las entradas se enganchan con `nil` como destino a propósito: eso es lo que
/// hace que AppKit las mande por la cadena de responder hasta quien sepa
/// atenderlas, que es el campo de texto que tenga el foco.
enum AnMenu {
    static func build() -> NSMenu {
        let root = NSMenu()
        root.addItem(appMenu())
        root.addItem(editMenu())
        root.addItem(windowMenu())
        return root
    }

    private static func submenu(_ title: String, _ entries: [NSMenuItem]) -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu(title: title)
        for entry in entries {
            menu.addItem(entry)
        }
        item.submenu = menu
        return item
    }

    private static func entry(
        _ title: String,
        _ action: Selector?,
        _ key: String,
        modifiers: NSEvent.ModifierFlags = .command
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.keyEquivalentModifierMask = modifiers
        return item
    }

    private static func appMenu() -> NSMenuItem {
        let name = ProcessInfo.processInfo.processName
        return submenu(name, [
            entry("Acerca de \(name)", #selector(NSApplication.orderFrontStandardAboutPanel(_:)), ""),
            .separator(),
            entry("Ocultar \(name)", #selector(NSApplication.hide(_:)), "h"),
            entry(
                "Ocultar los demás",
                #selector(NSApplication.hideOtherApplications(_:)),
                "h",
                modifiers: [.command, .option]
            ),
            .separator(),
            entry("Salir de \(name)", #selector(NSApplication.terminate(_:)), "q"),
        ])
    }

    private static func editMenu() -> NSMenuItem {
        submenu("Edición", [
            entry("Deshacer", Selector(("undo:")), "z"),
            entry("Rehacer", Selector(("redo:")), "z", modifiers: [.command, .shift]),
            .separator(),
            entry("Cortar", #selector(NSText.cut(_:)), "x"),
            entry("Copiar", #selector(NSText.copy(_:)), "c"),
            entry("Pegar", #selector(NSText.paste(_:)), "v"),
            entry("Seleccionar todo", #selector(NSText.selectAll(_:)), "a"),
        ])
    }

    private static func windowMenu() -> NSMenuItem {
        submenu("Ventana", [
            entry("Minimizar", #selector(NSWindow.performMiniaturize(_:)), "m"),
            entry("Zoom", #selector(NSWindow.performZoom(_:)), ""),
        ])
    }
}
