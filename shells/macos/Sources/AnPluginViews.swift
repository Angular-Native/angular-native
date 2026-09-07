import AppKit

/// Views a plugin brings.
///
/// A plugin contributes methods — that is the rule, and it is why a barcode
/// scanner could only ever be full screen. This is the one exception, and it is
/// deliberately narrow: a plugin registers a factory under a name, and a
/// template mounts it with `<an-custom [view]="that name">`.
///
/// This is the phone's file with `NSView` where it said `UIView`, and it is a
/// copy rather than a shared one because a plugin's own view is a copy too: an
/// `AnBarcodePreview` for AppKit is not the UIKit one under an `#if`, so the
/// registry it hands itself to cannot be either. What is shared is the name and
/// the C symbol, so a plugin covering both platforms writes the same line
/// twice and nothing else.
///
/// What it is **not** is a way to add a primitive. A primitive is a control the
/// framework mounts on every host, with a name every layer agrees about and a
/// check that keeps them agreeing. A plugin view is one platform's view, mounted
/// where the template asked, sized by the layout and nothing else.
///
/// ```swift
/// AnPluginViews.register("barcode-preview") { AnBarcodePreview() }
/// ```
public enum AnPluginViews {

    private static var factories: [String: () -> NSView] = [:]

    /// Called by a plugin, from `attach` or from its own initialiser.
    public static func register(_ name: String, _ make: @escaping () -> NSView) {
        if factories[name] != nil {
            NSLog("angular-native: two plugins register a view called \(name); the first one stays")
            return
        }
        factories[name] = make
    }

    /// Installed once by the shell, before the runtime is created.
    ///
    /// The closure is a C function pointer, so it may capture nothing — hence
    /// the explicit `AnPluginViews.` on the table below rather than the implicit
    /// `Self.` that reads better and does not compile.
    static func install() {
        let factory: @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutableRawPointer? = { name in
            guard let name,
                let make = AnPluginViews.factories[String(cString: name)]
            else {
                // The core says which name was missing, once. Returning null
                // rather than an empty view is what lets it.
                return nil
            }
            // `+1` on purpose: the core takes ownership of this reference. A
            // passUnretained here would hand over a view nothing holds, and it
            // would be gone before the next frame.
            return Unmanaged.passRetained(make()).toOpaque()
        }
        an_plugin_view_set_factory(factory)
    }
}
