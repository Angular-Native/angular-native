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

    var body: some View {
        // `GeometryReader` da el tamaño real de la pantalla del modelo que sea
        // —de 41 a 49 mm hay bastante diferencia— y ese es el viewport que se
        // le pasa a taffy. Fijar un tamaño aquí sería fijar el modelo de reloj.
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let root = runtime.tree.root {
                    AnNodeView(node: root) { target, name in
                        runtime.dispatch(target, name)
                    }
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
            .onAppear {
                runtime.start(width: geometry.size.width, height: geometry.size.height)
            }
            .onChange(of: geometry.size) { _, size in
                runtime.setViewport(width: size.width, height: size.height)
            }
        }
        // El reloj no tiene barras que respetar como el iPhone: la app ocupa la
        // pantalla entera y el layout de taffy ya cuenta con ello.
        .ignoresSafeArea()
        .background(.black)
    }
}
