import SwiftUI

/// Todo el shell del reloj cabe aquí: una escena, una vista raíz, y el runtime
/// arrancado con el tamaño de la pantalla.
@main
struct AngularNativeWatchApp: App {
    var body: some Scene {
        WindowGroup {
            RootView()
        }
    }
}

struct RootView: View {
    @State private var runtime = AnRuntime()
    /// Quién tiene la corona.
    ///
    /// El foco de watchOS es uno solo y la corona va con él, así que se lleva
    /// en la raíz y se pasa hacia abajo: si cada nodo tuviera el suyo, cada uno
    /// creería tenerla y ninguno la tendría.
    @FocusState private var crownTarget: UInt32?

    var body: some View {
        // `GeometryReader` da el tamaño real de la pantalla del modelo que sea
        // —de 41 a 49 mm hay bastante diferencia— y ese es el viewport que se
        // le pasa a taffy. Fijar un tamaño aquí sería fijar el modelo de reloj.
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let root = runtime.tree.root {
                    AnNodeView(
                        node: root,
                        controls: runtime.controls,
                        dispatch: runtime.dispatch,
                        crownFocus: $crownTarget
                    )
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
            .anOverlays(
                runtime.tree.overlays,
                controls: runtime.controls,
                dispatch: runtime.dispatch,
                crownFocus: $crownTarget
            )
            .onAppear {
                runtime.start(width: geometry.size.width, height: geometry.size.height)
            }
            .onChange(of: geometry.size) { _, size in
                runtime.setViewport(width: size.width, height: size.height)
            }
            // La corona se le da al primer nodo que la pide, en orden de
            // pintado. Se recalcula cuando cambia el árbol y no en cada `body`:
            // recorrerlo dentro de una recomposición sería recorrerlo varias
            // veces por frame.
            .onChange(of: runtime.tree.root) { _, root in
                if let objetivo = anPrimerNodoConCorona(root), crownTarget == nil {
                    crownTarget = objetivo
                }
            }
        }
        // El reloj no tiene barras que respetar como el iPhone: la app ocupa la
        // pantalla entera y el layout de taffy ya cuenta con ello.
        .ignoresSafeArea()
        .background(.black)
    }
}
