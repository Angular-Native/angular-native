import Foundation
import SwiftUI

/// The whole watch shell fits in here: one scene, one root view, and the runtime
/// started with the size of the screen.
@main
struct AngularNativeWatchApp: App {
    var body: some Scene {
        WindowGroup {
            RootView()
                // The two doors a URL can come through on this platform, and
                // they are not the phone's. See `AnDeepLinks` below for what
                // does not exist here.
                .onOpenURL { url in
                    AnDeepLinks.open(url)
                }
                .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
                    guard let url = activity.webpageURL else { return }
                    AnDeepLinks.open(url)
                }
        }
    }
}

/// A URL on the way into the core, and the short list of ways one can get here.
///
/// **A watch app cannot be opened by a custom scheme, and the plist deliberately
/// declares none.** There is no public call on watchOS that opens a third-party
/// app by URL: `WKApplication.openSystemURL(_:)` takes Apple's own schemes and
/// refuses everything else. Declaring the scheme anyway does not help: with the
/// block in the plist and the app installed and running, `simctl openurl`
/// answers `LSApplicationWorkspaceErrorDomain error 115` — nothing claimed it,
/// because nothing on this platform can. So a `CFBundleURLTypes` block here
/// would be a declaration the system never reads, which is the kind of wiring
/// that looks done and is not; the watch's `Info.plist` says the same thing
/// where somebody would go looking for the block.
///
/// What does arrive:
///
///   * the app's own WidgetKit complication, through `widgetURL(_:)` or a
///     `Link` — a tap on the watch face launches the app with that URL, and it
///     is the ordinary way a watch app is deep-linked;
///   * a universal link, as an `NSUserActivity` of type
///     `NSUserActivityTypeBrowsingWeb`. It needs the associated-domains
///     entitlement and an apple-app-site-association file on the site, the same
///     half nobody can write for you that the phone needs.
///
/// Which of the two runs first, this or `RootView`'s `onAppear`, is SwiftUI's to
/// decide and it is not documented. It does not have to be: the core queues what
/// arrives before the engine exists and emits what arrives after, so the early
/// order is a cold start and the late one is a push. See
/// `crates/an-bridge/src/deeplink.rs`.
enum AnDeepLinks {
    static func open(_ url: URL) {
        // The absolute string and not the components: what the route means is
        // the app's business, and the core hands it to Angular's router whole.
        an_deeplink_open(url.absoluteString)
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
