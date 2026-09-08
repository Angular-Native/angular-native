import SwiftUI
import UIKit

/// An image from the bundle or from the network, and its natural size back.
///
/// The size is not decoration: the layout cannot place something whose size it
/// does not know, and only the image knows it. That is why the `an-image`
/// directive always registers the `load` listener, even when the template does
/// not listen for it, and why this host sends it as soon as it has the image.
/// Without that notice an image with no measurements stays at zero, which is what
/// used to happen on the watch.
///
/// `AsyncImage` is not used: it only knows about URLs, and here half the cases
/// are bundle resources. And the `UIImage` is needed anyway, because the size in
/// points comes out of it.
struct AnImageView: View {
    let node: AnNode
    let dispatch: (UInt32, String, [String: Any]) -> Void

    @State private var image: UIImage?

    var body: some View {
        content
            // `task(id:)` is redone when the path changes and cancels itself when
            // the view goes away: a download nobody cares about any more is not
            // left running.
            .task(id: node.source) {
                image = await AnImageStore.shared.load(node.source)
                guard let image else { return }
                let size = image.size
                dispatch(node.id, "load", ["width": size.width, "height": size.height])
            }
    }

    @ViewBuilder
    private var content: some View {
        if let image {
            Image(uiImage: image)
                .resizable()
                .aspectRatio(contentMode: mode)
                .frame(width: node.width, height: node.height, alignment: .center)
                .clipped()
        } else {
            // While there is no image, nothing is painted. A grey placeholder
            // would be an image the template did not ask for.
            Color.clear
        }
    }

    /// `contain` by default, as on iOS. `stretch` and `center` are not aspect
    /// ratio modes and are resolved below, in `body`, with the frame.
    private var mode: ContentMode {
        node.resizeMode == "cover" ? .fill : .fit
    }
}

/// The images already loaded, so they are not read or downloaded again on every
/// snapshot. The tree is rebuilt thirty times a second; the images are not.
actor AnImageStore {
    static let shared = AnImageStore()

    private var cache: [String: UIImage] = [:]
    /// The names that came up empty, so each one is said once.
    ///
    /// A miss puts nothing in `cache` —there is no image to put there— so
    /// without this the file is looked for again every time the view's `task`
    /// restarts and the log fills with the same line. It is not `AnWarnings`
    /// because that one is `@MainActor` and this runs on the store's own actor:
    /// reaching it would mean hopping to the main thread to write a log line.
    private var missing: Set<String> = []

    func load(_ source: String?) async -> UIImage? {
        guard let source, !source.isEmpty else { return nil }
        if let hit = cache[source] {
            return hit
        }
        let image: UIImage?
        if source.hasPrefix("http://") || source.hasPrefix("https://") {
            image = await download(source)
        } else {
            // With no scheme it is a resource in the app bundle, as on iOS.
            image = bundled(source)
            if image == nil, missing.insert(source).inserted {
                // The same sentence `an-ios`'s `images.rs` says, because it is
                // the same mistake and the fix is in the same place. A missing
                // file otherwise draws `Color.clear`, which on screen is an
                // image still loading, a colour that matches the background and
                // a frame of zero height all at once.
                NSLog("""
                    angular-native: \(source) is not in the app. A [source] with no scheme is a \
                    file that travelled with the app; put it in the project's resources/ \
                    directory, which `an` copies into the .app under the name it has there.
                    """)
            }
        }
        if let image {
            cache[source] = image
        }
        return image
    }

    /// The image a schemeless `source` names, from inside the `.app`.
    ///
    /// Two lookups, and the second is the one that does the work here.
    /// `UIImage(named:)` resolves an asset catalogue's names, and a bundle
    /// `an` assembles has no catalogue: what it has is the files copied out of
    /// the app's `resources/`, flat in the `.app` next to `main.js`. Those are
    /// read by path — which also covers the names with a slash in them, since
    /// `resources/icons/logo.png` is asked for as `icons/logo.png` and that is
    /// not a name `imageNamed:` knows how to resolve on any platform.
    private func bundled(_ source: String) -> UIImage? {
        if let image = UIImage(named: source) {
            return image
        }
        guard let resources = Bundle.main.resourcePath else { return nil }
        return UIImage(contentsOfFile: "\(resources)/\(source)")
    }

    private func download(_ source: String) async -> UIImage? {
        guard let url = URL(string: source) else {
            NSLog("angular-native: \(source) is not a URL")
            return nil
        }
        do {
            let (data, _) = try await URLSession.shared.data(from: url)
            guard let image = UIImage(data: data) else {
                NSLog("angular-native: what arrived from \(source) is not an image")
                return nil
            }
            return image
        } catch {
            NSLog("angular-native: \(source) could not be downloaded: \(error.localizedDescription)")
            return nil
        }
    }
}
