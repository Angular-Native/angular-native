import AVFoundation
import PhotosUI
import UIKit

/// The camera and the photo library on iOS.
///
/// Both are presented rather than embedded, which is why this plugin needs the
/// `attach` half of the protocol: a picker has to be presented from a view
/// controller, and a plugin has nowhere else to get one.
///
/// Two pickers and not one, because they are two different things. The camera is
/// `UIImagePickerController`, which needs `NSCameraUsageDescription` and a
/// permission the person grants. The library is `PHPickerViewController`, which
/// since iOS 14 runs **outside the app**: it hands back what was chosen and
/// nothing else, so there is no library permission to ask for, and asking would
/// be asking for something the system has deliberately made irrelevant.
final class AnCameraPlugin: NSObject, AnPlugin, UIImagePickerControllerDelegate,
    UINavigationControllerDelegate, PHPickerViewControllerDelegate
{
    private weak var host: UIViewController?

    /// The call waiting for a picker to close. One at a time: a second picker
    /// cannot be presented over the first anyway.
    private var pending: AnPluginCall?
    private var options = PhotoOptions()

    private struct PhotoOptions {
        var maxSize: CGFloat = 0
        var quality: CGFloat = 0.85
    }

    func attach(_ host: UIViewController) {
        self.host = host
    }

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
                // The callback is not on the main thread, and the answer is read
                // from a class that expects to be asked there.
                DispatchQueue.main.async {
                    respond.resolve(Self.name(for: AVCaptureDevice.authorizationStatus(for: .video)))
                }
            }

        case "takePhoto":
            guard UIImagePickerController.isSourceTypeAvailable(.camera) else {
                // The simulator is the usual reason, and saying so beats a
                // picker that opens black.
                respond.reject("camera.takePhoto found no camera on this device")
                return
            }
            guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else {
                respond.reject("camera.takePhoto has no permission: the camera is off for this app in Settings")
                return
            }
            guard let host, take(respond, args) else { return }
            let picker = UIImagePickerController()
            picker.sourceType = .camera
            picker.delegate = self
            host.present(picker, animated: true)

        case "pickPhoto":
            guard let host, take(respond, args) else { return }
            var configuration = PHPickerConfiguration()
            configuration.filter = .images
            configuration.selectionLimit = 1
            let picker = PHPickerViewController(configuration: configuration)
            picker.delegate = self
            host.present(picker, animated: true)

        default:
            respond.reject("the camera plugin has no method \(method)")
        }
    }

    /// Takes the call, or rejects it because another picker is already up.
    /// Returns whether the caller should carry on.
    private func take(_ respond: AnPluginCall, _ args: [String: Any]) -> Bool {
        guard host != nil else {
            respond.reject("the camera plugin has no view controller to present from")
            return false
        }
        if pending != nil {
            respond.reject("camera: another picker is already open")
            return false
        }
        pending = respond
        options = PhotoOptions(
            maxSize: CGFloat((args["maxSize"] as? Double) ?? 0),
            quality: CGFloat((args["quality"] as? Double) ?? 0.85))
        return true
    }

    // ------------------------------------------------------------ the camera

    func imagePickerController(
        _ picker: UIImagePickerController,
        didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
    ) {
        picker.dismiss(animated: true)
        guard let waiting = settle() else { return }
        guard let image = info[.originalImage] as? UIImage else {
            waiting.reject("camera.takePhoto got no image back from the picker")
            return
        }
        answer(waiting, with: image)
    }

    func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
        picker.dismiss(animated: true)
        // Cancelling says so in the message. An app that only catches can still
        // tell "they changed their mind" from "the camera is broken".
        settle()?.reject("camera.takePhoto was cancelled")
    }

    // ----------------------------------------------------------- the library

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard let waiting = settle() else { return }
        guard let provider = results.first?.itemProvider,
            provider.canLoadObject(ofClass: UIImage.self)
        else {
            waiting.reject("camera.pickPhoto was cancelled")
            return
        }
        provider.loadObject(ofClass: UIImage.self) { [weak self] object, error in
            DispatchQueue.main.async {
                guard let image = object as? UIImage else {
                    waiting.reject(
                        "camera.pickPhoto could not read the image: \(error?.localizedDescription ?? "no reason given")")
                    return
                }
                self?.answer(waiting, with: image)
            }
        }
    }

    // ------------------------------------------------------------- the file

    /// Writes the image and answers with where it went.
    ///
    /// Nothing crosses the bridge as bytes: a twelve-megapixel photograph as
    /// base64 is sixteen megabytes of string through a JSON boundary, encoded
    /// once and parsed once. A path costs a few dozen.
    private func answer(_ waiting: AnPluginCall, with image: UIImage) {
        let scaled = Self.scale(image, longestSide: options.maxSize)
        guard let data = scaled.jpegData(compressionQuality: options.quality) else {
            waiting.reject("camera: the photograph could not be encoded as JPEG")
            return
        }
        // The cache directory and not Documents: a photograph an app has not
        // decided to keep should not survive being backed up, and the system may
        // reclaim it. An app that wants to keep it copies it with `Files`.
        let name = "an-camera-\(UUID().uuidString).jpg"
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        do {
            try data.write(to: url, options: .atomic)
        } catch {
            waiting.reject("camera: the photograph could not be written: \(error.localizedDescription)")
            return
        }
        waiting.resolve([
            "path": url.path,
            "width": Double(scaled.size.width * scaled.scale),
            "height": Double(scaled.size.height * scaled.scale)
        ])
    }

    private static func scale(_ image: UIImage, longestSide: CGFloat) -> UIImage {
        let longest = max(image.size.width, image.size.height)
        guard longestSide > 0, longest > longestSide else { return image }
        let ratio = longestSide / longest
        let size = CGSize(width: image.size.width * ratio, height: image.size.height * ratio)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { _ in image.draw(in: CGRect(origin: .zero, size: size)) }
    }

    private func settle() -> AnPluginCall? {
        let waiting = pending
        pending = nil
        return waiting
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
