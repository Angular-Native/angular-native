#if os(iOS)
    import AVFoundation
    import UIKit

    /// A live camera preview that reads barcodes, inside your own layout.
    ///
    /// This is the thing a plugin could not do until the core grew
    /// `NodeKind::Custom`. Everything else a plugin contributes is a method; this
    /// is a view, and a template mounts it like any other box:
    ///
    /// ```html
    /// <an-custom [view]="'barcode-preview'" [style.height]="'320'" [borderRadius]="16" />
    /// ```
    ///
    /// What it reads goes out on the module's event channel as `barcode.read`,
    /// not as the answer to a call: a preview produces none or a hundred, and a
    /// promise carries one.
    ///
    /// It starts the session when it is put on screen and stops it when it is
    /// taken off. That is not tidiness — a camera left running is a light on the
    /// front of the phone and a hole in the battery, and the view is the only
    /// thing that knows when it stopped being visible.
    final class AnBarcodePreview: UIView {

        override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }

        private var preview: AVCaptureVideoPreviewLayer {
            // Safe by construction: `layerClass` above decides what this is.
            layer as! AVCaptureVideoPreviewLayer
        }

        private let session = AVCaptureSession()
        private let reader = Reader()

        override init(frame: CGRect) {
            super.init(frame: frame)
            backgroundColor = .black
            preview.videoGravity = .resizeAspectFill
            preview.session = session
            configure()
        }

        required init?(coder: NSCoder) {
            fatalError("AnBarcodePreview is built in code, never from a nib")
        }

        private func configure() {
            guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else {
                // Asking here would put a system dialog on screen the moment a
                // layout happened to include this view, which is not the app's
                // decision to make silently. `barcode.available()` and the
                // camera plugin's `request()` are where that belongs.
                AnEvents.emit(
                    "barcode", "error",
                    ["message": "the barcode preview has no camera permission yet"])
                return
            }
            guard let device = AVCaptureDevice.default(for: .video),
                let input = try? AVCaptureDeviceInput(device: device),
                session.canAddInput(input)
            else {
                AnEvents.emit("barcode", "error", ["message": "the barcode preview found no camera"])
                return
            }
            session.addInput(input)

            let output = AVCaptureMetadataOutput()
            guard session.canAddOutput(output) else {
                AnEvents.emit(
                    "barcode", "error",
                    ["message": "the barcode preview could not attach a metadata reader"])
                return
            }
            session.addOutput(output)
            output.setMetadataObjectsDelegate(reader, queue: .main)
            // After adding the output, never before: until then the session does
            // not know what the camera can produce and setting a type throws.
            output.metadataObjectTypes = output.availableMetadataObjectTypes
        }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            if window == nil {
                session.stopRunning()
            } else if !session.isRunning {
                DispatchQueue.global(qos: .userInitiated).async { [session] in
                    session.startRunning()
                }
            }
        }

        /// Kept apart from the view so the delegate is not the view itself:
        /// `AVCaptureMetadataOutput` holds its delegate weakly, and a view that
        /// is its own delegate is one ownership question nobody needs.
        private final class Reader: NSObject, AVCaptureMetadataOutputObjectsDelegate {
            /// The last thing read, so a barcode held in front of the camera does
            /// not emit thirty times a second.
            private var last: String?

            func metadataOutput(
                _ output: AVCaptureMetadataOutput,
                didOutput objects: [AVMetadataObject],
                from connection: AVCaptureConnection
            ) {
                guard
                    let found = objects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject })
                        .first,
                    let value = found.stringValue,
                    value != last
                else { return }
                last = value
                AnEvents.emit("barcode", "read", ["value": value, "format": found.type.rawValue])
            }
        }
    }
#endif
