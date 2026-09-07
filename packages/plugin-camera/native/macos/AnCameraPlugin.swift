import AppKit
import AVFoundation

/// The camera and the photo library on the Mac — or rather, what a Mac has
/// instead of them.
///
/// This is a separate file from the phone's and not a `#if`, because the two
/// answers are genuinely different rather than the same thing spelled twice:
///
///   · **There is no photo library picker.** macOS has no `PHPickerViewController`
///     and no Photos app you can present a sheet from. What it has is an open
///     panel, so `pickPhoto` is `NSOpenPanel` filtered to images — which is what
///     a Mac user expects when an app asks for a picture anyway.
///
///   · **There is no camera picker at all.** `UIImagePickerController` has no
///     AppKit equivalent: a Mac app that wants a photograph builds a capture
///     session and its own preview, which is a view, and a plugin contributes
///     methods rather than views. So `takePhoto` refuses and says why, rather
///     than opening something that is not a camera.
final class AnCameraPlugin: AnPlugin {

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "permission":
            respond.resolve(Self.name(for: AVCaptureDevice.authorizationStatus(for: .video)))

        case "request":
            let status = AVCaptureDevice.authorizationStatus(for: .video)
            guard status == .notDetermined else {
                respond.resolve(Self.name(for: status))
                return
            }
            AVCaptureDevice.requestAccess(for: .video) { _ in
                DispatchQueue.main.async {
                    respond.resolve(Self.name(for: AVCaptureDevice.authorizationStatus(for: .video)))
                }
            }

        case "takePhoto":
            respond.reject(
                "camera.takePhoto is not available on macOS: AppKit has no picker to present. "
                    + "Taking a photograph here means an AVCaptureSession and a preview of your "
                    + "own, and a preview is a view, which a plugin cannot contribute. "
                    + "camera.pickPhoto opens an image chooser and does work.")

        case "pickPhoto":
            let panel = NSOpenPanel()
            panel.allowsMultipleSelection = false
            panel.canChooseDirectories = false
            panel.allowedContentTypes = [.image]
            panel.prompt = "Choose"
            // `begin` and not `runModal`: a modal loop here would stop the frame
            // the plugin was called inside, and the engine would stall behind it.
            panel.begin { outcome in
                guard outcome == .OK, let url = panel.url else {
                    respond.reject("camera.pickPhoto was cancelled")
                    return
                }
                guard let image = NSImage(contentsOf: url) else {
                    respond.reject("camera.pickPhoto could not read \(url.lastPathComponent)")
                    return
                }
                respond.resolve([
                    "path": url.path,
                    "width": Double(image.size.width),
                    "height": Double(image.size.height)
                ])
            }

        default:
            respond.reject("the camera plugin has no method \(method)")
        }
    }

    private static func name(for status: AVAuthorizationStatus) -> String {
        switch status {
        case .notDetermined: return "prompt"
        case .restricted: return "restricted"
        case .denied: return "denied"
        default: return "granted"
        }
    }
}
