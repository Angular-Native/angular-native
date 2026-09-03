import SwiftUI

/// The whole watch shell fits in here: one scene, one root view, and the runtime
/// started with the size of the screen.
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
    /// Who has the crown.
    ///
    /// watchOS has a single focus and the crown goes with it, so it is kept at
    /// the root and passed down: if every node had its own, each would believe it
    /// had the crown and none would. What claims it is the node itself when it
    /// appears, in `AnCrown`.
    @FocusState private var crownTarget: UInt32?

    var body: some View {
        // `GeometryReader` gives the real screen size of whichever model it is
        // —from 41 to 49 mm there is quite a difference— and that is the viewport
        // handed to taffy. Fixing a size here would mean fixing the watch model.
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
        }
        // The watch has no bars to respect the way the iPhone does: the app takes
        // up the whole screen and taffy's layout already accounts for it.
        .ignoresSafeArea()
        .background(.black)
    }
}
