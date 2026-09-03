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
            image = UIImage(named: source)
            if image == nil {
                NSLog("angular-native: there is no image called \(source) in the bundle")
            }
        }
        if let image {
            cache[source] = image
        }
        return image
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
