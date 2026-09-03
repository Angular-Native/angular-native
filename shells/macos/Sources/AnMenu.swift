import AppKit

/// The app menu.
///
/// **It belongs to the system and is not exposed to Angular.** On a phone there
/// is no menu; on a Mac there always is one, it lives in the bar at the top and
/// not inside the window, so it does not fit in the view tree the core mounts:
/// there is no `NodeKind` it corresponds to and one cannot be invented without
/// opening `packages/primitives`, which belongs to somebody else. The shell puts
/// it there, full stop.
///
/// What is needed is that it **exists**, and with the real entries in it. An
/// empty menu is not a cosmetic detail: the macOS editing shortcuts —⌘X, ⌘C,
/// ⌘V, ⌘Z, ⌘A— are not implemented by `NSTextField`, they are dispatched by the
/// menu along the responder chain. With no Edit menu, copy and paste in an
/// `<an-text-input>` does not work, with no error and nothing to look at. That
/// is exactly the kind of silent failure this repository does not allow, and
/// that is why the minimal menu includes Edit and not only Quit.
///
/// The entries are hooked up with `nil` as their target on purpose: that is what
/// makes AppKit send them along the responder chain to whoever knows how to
/// handle them, which is the text field that has the focus.
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
            entry("About \(name)", #selector(NSApplication.orderFrontStandardAboutPanel(_:)), ""),
            .separator(),
            entry("Hide \(name)", #selector(NSApplication.hide(_:)), "h"),
            entry(
                "Hide Others",
                #selector(NSApplication.hideOtherApplications(_:)),
                "h",
                modifiers: [.command, .option]
            ),
            .separator(),
            entry("Quit \(name)", #selector(NSApplication.terminate(_:)), "q"),
        ])
    }

    private static func editMenu() -> NSMenuItem {
        submenu("Edit", [
            entry("Undo", Selector(("undo:")), "z"),
            entry("Redo", Selector(("redo:")), "z", modifiers: [.command, .shift]),
            .separator(),
            entry("Cut", #selector(NSText.cut(_:)), "x"),
            entry("Copy", #selector(NSText.copy(_:)), "c"),
            entry("Paste", #selector(NSText.paste(_:)), "v"),
            entry("Select All", #selector(NSText.selectAll(_:)), "a"),
        ])
    }

    private static func windowMenu() -> NSMenuItem {
        submenu("Window", [
            entry("Minimize", #selector(NSWindow.performMiniaturize(_:)), "m"),
            entry("Zoom", #selector(NSWindow.performZoom(_:)), ""),
        ])
    }
}
