import AVFoundation
#if os(macOS)
    import AppKit
#else
    import UIKit
#endif

/// Reading a barcode with the camera.
///
/// `AVCaptureMetadataOutput` does the decoding: it is part of AVFoundation, on
/// the device, with no model to download and no service to depend on. That is
/// the whole reason this plugin exists on Apple platforms and not on Android,
/// where the equivalent lives in Play Services.
///
/// The scanner is presented full screen and torn down the moment it reads
/// something. A preview embedded in the app's own layout would be a view, and a
/// plugin contributes methods rather than views.
final class AnBarcodePlugin: NSObject, AnPlugin, AVCaptureMetadataOutputObjectsDelegate {

    #if os(macOS)
        private weak var host: NSViewController?
        func attach(_ host: NSViewController) { self.host = host }
    #else
        private weak var host: UIViewController?
        func attach(_ host: UIViewController) {
            self.host = host
            // The inline preview, registered under the name a template mounts
            // it by. It is the one thing here that is a view rather than a
            // method, and it needs the core's `Custom` node to exist at all.
            AnPluginViews.register("barcode-preview") { AnBarcodePreview() }
        }
    #endif

    private var pending: AnPluginCall?
    private var session: AVCaptureSession?
    private var presented: Any?

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        switch method {
        case "available":
            let camera = AVCaptureDevice.default(for: .video) != nil
            let allowed = AVCaptureDevice.authorizationStatus(for: .video) != .denied
            respond.resolve(camera && allowed)

        case "scan":
            scan(args, respond)

        default:
            respond.reject("the barcode plugin has no method \(method)")
        }
    }

    private func scan(_ args: [String: Any], _ respond: AnPluginCall) {
        guard pending == nil else {
            respond.reject("barcode: a scanner is already open")
            return
        }
        guard let device = AVCaptureDevice.default(for: .video) else {
            respond.reject("barcode.scan found no camera on this device")
            return
        }
        // Asked for rather than assumed: a session started without permission
        // produces a black preview and no error, which is the worst of both.
        guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else {
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                DispatchQueue.main.async {
                    guard granted else {
                        respond.reject(
                            "barcode.scan has no permission: the camera is off for this app in Settings")
                        return
                    }
                    self?.scan(args, respond)
                }
            }
            return
        }

        let session = AVCaptureSession()
        guard let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) else {
            respond.reject("barcode.scan could not open the camera")
            return
        }
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else {
            respond.reject("barcode.scan could not attach a metadata reader to the camera")
            return
        }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)

        // The types have to be set *after* the output is added: before that the
        // session does not yet know what the camera can produce, and setting an
        // available type it has not learned about throws.
        let wanted = (args["formats"] as? [String])?.compactMap {
            AVMetadataObject.ObjectType(rawValue: $0)
        }
        let supported = output.availableMetadataObjectTypes
        output.metadataObjectTypes = wanted?.filter(supported.contains) ?? supported

        self.session = session
        pending = respond
        present(session, prompt: args["prompt"] as? String)
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput objects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard let waiting = pending,
            let found = objects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject }).first,
            let value = found.stringValue
        else { return }
        pending = nil
        finish()
        waiting.resolve(["value": value, "format": found.type.rawValue])
    }

    /// The person closed it without scanning anything.
    @objc func cancel() {
        guard let waiting = pending else { return }
        pending = nil
        finish()
        waiting.reject("barcode.scan was cancelled")
    }

    private func finish() {
        session?.stopRunning()
        session = nil
        #if os(macOS)
            if let window = presented as? NSWindow {
                host?.view.window?.endSheet(window)
            }
        #else
            (presented as? UIViewController)?.dismiss(animated: true)
        #endif
        presented = nil
    }

    // ------------------------------------------------------------ presenting

    #if os(macOS)
        private func present(_ session: AVCaptureSession, prompt: String?) {
            guard let parent = host?.view.window else {
                fail("the barcode plugin has no window to present from")
                return
            }
            let window = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 640, height: 480),
                styleMask: [.titled, .closable],
                backing: .buffered,
                defer: false)
            window.title = prompt ?? "Scan a barcode"
            let layer = AVCaptureVideoPreviewLayer(session: session)
            layer.videoGravity = .resizeAspectFill
            let view = NSView(frame: window.contentLayoutRect)
            view.wantsLayer = true
            layer.frame = view.bounds
            layer.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
            view.layer?.addSublayer(layer)
            window.contentView = view
            presented = window
            parent.beginSheet(window)
            DispatchQueue.global(qos: .userInitiated).async { session.startRunning() }
        }
    #else
        private func present(_ session: AVCaptureSession, prompt: String?) {
            guard let host else {
                fail("the barcode plugin has no view controller to present from")
                return
            }
            let controller = UIViewController()
            controller.view.backgroundColor = .black
            let layer = AVCaptureVideoPreviewLayer(session: session)
            layer.videoGravity = .resizeAspectFill
            layer.frame = controller.view.bounds
            layer.needsDisplayOnBoundsChange = true
            controller.view.layer.addSublayer(layer)

            let cancelButton = UIButton(type: .system)
            cancelButton.setTitle("Cancel", for: .normal)
            cancelButton.setTitleColor(.white, for: .normal)
            cancelButton.addTarget(self, action: #selector(cancel), for: .touchUpInside)
            cancelButton.translatesAutoresizingMaskIntoConstraints = false
            controller.view.addSubview(cancelButton)

            let label = UILabel()
            label.text = prompt ?? "Point the camera at a barcode"
            label.textColor = .white
            label.textAlignment = .center
            label.numberOfLines = 0
            label.translatesAutoresizingMaskIntoConstraints = false
            controller.view.addSubview(label)

            let guides = controller.view.safeAreaLayoutGuide
            NSLayoutConstraint.activate([
                cancelButton.bottomAnchor.constraint(equalTo: guides.bottomAnchor, constant: -24),
                cancelButton.centerXAnchor.constraint(equalTo: guides.centerXAnchor),
                label.topAnchor.constraint(equalTo: guides.topAnchor, constant: 24),
                label.leadingAnchor.constraint(equalTo: guides.leadingAnchor, constant: 24),
                label.trailingAnchor.constraint(equalTo: guides.trailingAnchor, constant: -24)
            ])

            controller.modalPresentationStyle = .fullScreen
            presented = controller
            host.present(controller, animated: true) {
                // The preview layer only has a real size once the controller is
                // on screen; sized before that it is a zero rectangle and the
                // camera appears not to work.
                layer.frame = controller.view.bounds
            }
            DispatchQueue.global(qos: .userInitiated).async { session.startRunning() }
        }
    #endif

    private func fail(_ why: String) {
        let waiting = pending
        pending = nil
        session = nil
        waiting?.reject(why)
    }
}
