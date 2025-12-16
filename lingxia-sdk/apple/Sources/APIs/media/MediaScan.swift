#if os(iOS)
import UIKit
import AVFoundation
import Photos
import QuartzCore
import Vision
import CLingXiaSwiftAPI
import CLingXiaRustAPI

extension LxAppMedia {
    nonisolated static func scanCode(
        scan_types_json: RustStr,
        only_from_camera: Bool,
        callback_id: UInt64
    ) -> Bool {
#if os(iOS)
        let typesJson = scan_types_json.toString()
        DispatchQueue.main.async {
            guard #available(iOS 13.0, *) else {
                let _ = onCallback(callback_id, false, "scanCode requires iOS 13.0 or later")
                return
            }
            guard let presenter = topViewController() else {
                let _ = onCallback(callback_id, false, "Unable to find top view controller")
                return
            }

            let codes: [Int]
            if let data = typesJson.data(using: .utf8),
               let parsed = try? JSONSerialization.jsonObject(with: data, options: []) as? [Int] {
                codes = parsed
            } else {
                codes = []
            }

            let controller = ScanCodeViewController(
                scanTypes: codes,
                onlyFromCamera: only_from_camera,
                callbackId: callback_id
            )
            controller.modalPresentationStyle = .fullScreen
            presenter.present(controller, animated: true)
        }
        return true
#else
        return false
#endif
    }
}

@available(iOS 13.0, *)
@MainActor
final class ScanCodeViewController: UIViewController {
    private enum Constant {
        static let animationDuration: CFTimeInterval = 1.6
    }

    private let scanTypes: [Int]
    private let onlyFromCamera: Bool
    private let callbackId: UInt64

    private let metadataQueue = DispatchQueue(label: "com.lingxia.scan.metadata")

    private var session: AVCaptureSession?
    private var metadataOutput: AVCaptureMetadataOutput?
    private var previewLayer: AVCaptureVideoPreviewLayer?
    private var hasReported = false

    private let previewContainer = UIView()
    private let overlayContainer = UIView()
    private let scanLine = UIView()
    private var scanLineAnimation: CABasicAnimation?

    private lazy var closeButton: UIButton = {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.tintColor = .white
        button.setImage(LxAppMedia.controlImage(named: "icon_close"), for: .normal)
        button.addTarget(self, action: #selector(closeTapped), for: .touchUpInside)
        button.contentEdgeInsets = UIEdgeInsets(top: 6, left: 6, bottom: 6, right: 6)
        return button
    }()

    private lazy var albumButton: UIView = {
        let container = UIView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.isUserInteractionEnabled = true
        let tap = UITapGestureRecognizer(target: self, action: #selector(openAlbum))
        container.addGestureRecognizer(tap)
        
        let iconWrap = UIView()
        iconWrap.translatesAutoresizingMaskIntoConstraints = false
        iconWrap.backgroundColor = UIColor.black.withAlphaComponent(0.4)
        iconWrap.layer.cornerRadius = 36
        container.addSubview(iconWrap)
        
        let iconView = UIImageView()
        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.image = LxAppMedia.controlImage(named: "icon_album")
        iconView.tintColor = .white
        iconView.contentMode = .scaleAspectFit
        iconWrap.addSubview(iconView)
        
        let label = UILabel()
        label.translatesAutoresizingMaskIntoConstraints = false
        label.text = L10n.string("lx_album_label")
        label.textColor = .white
        label.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        label.textAlignment = .center
        container.addSubview(label)
        
        NSLayoutConstraint.activate([
            iconWrap.topAnchor.constraint(equalTo: container.topAnchor),
            iconWrap.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            iconWrap.widthAnchor.constraint(equalToConstant: 72),
            iconWrap.heightAnchor.constraint(equalToConstant: 72),
            
            iconView.centerXAnchor.constraint(equalTo: iconWrap.centerXAnchor),
            iconView.centerYAnchor.constraint(equalTo: iconWrap.centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 36),
            iconView.heightAnchor.constraint(equalToConstant: 36),
            
            label.topAnchor.constraint(equalTo: iconWrap.bottomAnchor, constant: 8),
            label.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            label.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])
        
        return container
    }()

    init(scanTypes: [Int], onlyFromCamera: Bool, callbackId: UInt64) {
        self.scanTypes = scanTypes
        self.onlyFromCamera = onlyFromCamera
        self.callbackId = callbackId
        super.init(nibName: nil, bundle: nil)
        modalPresentationCapturesStatusBarAppearance = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var prefersStatusBarHidden: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        configureUI()
        ensureCameraPermission()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = previewContainer.bounds
        restartScanLineIfNeeded()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        stopSession()
    }

    private func configureUI() {
        view.backgroundColor = .black

        previewContainer.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(previewContainer)

        overlayContainer.translatesAutoresizingMaskIntoConstraints = false
        overlayContainer.backgroundColor = .clear
        view.addSubview(overlayContainer)

        NSLayoutConstraint.activate([
            previewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            previewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            previewContainer.topAnchor.constraint(equalTo: view.topAnchor),
            previewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor),

            overlayContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlayContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlayContainer.topAnchor.constraint(equalTo: view.topAnchor),
            overlayContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        view.addSubview(closeButton)
        NSLayoutConstraint.activate([
            closeButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            closeButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            closeButton.widthAnchor.constraint(equalToConstant: 40),
            closeButton.heightAnchor.constraint(equalToConstant: 40)
        ])

        if !onlyFromCamera {
            view.addSubview(albumButton)
            NSLayoutConstraint.activate([
                albumButton.centerXAnchor.constraint(equalTo: view.centerXAnchor),
                albumButton.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -48),
                albumButton.widthAnchor.constraint(equalToConstant: 100),
                albumButton.heightAnchor.constraint(equalToConstant: 100)
            ])
        }

        scanLine.translatesAutoresizingMaskIntoConstraints = true
        scanLine.backgroundColor = UIColor(red: 0.2, green: 0.6, blue: 1.0, alpha: 0.4)
        overlayContainer.addSubview(scanLine)
    }

    private func ensureCameraPermission() {
        PermissionManager.ensureCameraAccess { [weak self] granted in
            guard let self else { return }
            if granted {
                self.configureSession()
            } else {
                self.reportFailure("Camera permission denied", code: "camera_permission_denied")
            }
        }
    }

    private func configureSession() {
        let session = AVCaptureSession()
        session.beginConfiguration()

        guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back) ??
            AVCaptureDevice.default(for: .video) else {
            reportFailure("Camera is not available on this device")
            return
        }

        do {
            let input = try AVCaptureDeviceInput(device: device)
            if session.canAddInput(input) {
                session.addInput(input)
            } else {
                reportFailure("Unable to add camera input")
                return
            }
        } catch {
            reportFailure("Failed to access camera: \(error.localizedDescription)")
            return
        }

        let metadataOutput = AVCaptureMetadataOutput()
        if session.canAddOutput(metadataOutput) {
            session.addOutput(metadataOutput)
            metadataOutput.setMetadataObjectsDelegate(self, queue: metadataQueue)
        } else {
            reportFailure("Unable to configure metadata output")
            return
        }

        session.commitConfiguration()

        self.session = session
        self.metadataOutput = metadataOutput
        attachPreviewLayer(to: session)
        applyMetadataTypes()
        session.startRunning()
    }

    private func attachPreviewLayer(to session: AVCaptureSession) {
        if let existing = previewLayer {
            existing.session = session
        } else {
            let layer = AVCaptureVideoPreviewLayer(session: session)
            layer.videoGravity = .resizeAspectFill
            layer.frame = previewContainer.bounds
            previewContainer.layer.insertSublayer(layer, at: 0)
            previewLayer = layer
        }
        restartScanLineIfNeeded()
    }

    private func applyMetadataTypes() {
        guard let metadataOutput = metadataOutput else { return }
        let requested = requestedMetadataTypes()
        let available = Set(metadataOutput.availableMetadataObjectTypes)
        let finalTypes: [AVMetadataObject.ObjectType]
        if requested.isEmpty {
            finalTypes = Array(available)
        } else {
            let intersection = available.intersection(requested)
            finalTypes = intersection.isEmpty ? Array(available) : Array(intersection)
        }
        metadataOutput.metadataObjectTypes = finalTypes
    }

    private func requestedMetadataTypes() -> Set<AVMetadataObject.ObjectType> {
        if scanTypes.isEmpty {
            return []
        }

        var types = Set<AVMetadataObject.ObjectType>()
        scanTypes.forEach { code in
            switch code {
            case 1:
                types.insert(.qr)
            case 2:
                types.formUnion(oneDimensionalTypes())
            case 3:
                types.insert(.dataMatrix)
            case 4:
                types.insert(.pdf417)
            default:
                break
            }
        }

        return types
    }

    private func oneDimensionalTypes() -> Set<AVMetadataObject.ObjectType> {
        return [
            .code128,
            .code39,
            .code39Mod43,
            .code93,
            .ean8,
            .ean13,
            .itf14,
            .upce,
            .interleaved2of5,
            .pdf417,
            .aztec
        ]
    }

    @available(iOS 14.0, *)
    private func allowedVisionSymbologies() -> [VNBarcodeSymbology] {
        let supported = Set(VNDetectBarcodesRequest.supportedSymbologies)
        if scanTypes.isEmpty {
            return Array(supported)
        }

        var allowed = Set<VNBarcodeSymbology>()
        func insert(_ sym: VNBarcodeSymbology) {
            if supported.contains(sym) {
                allowed.insert(sym)
            }
        }

        scanTypes.forEach { code in
            switch code {
            case 1:
                insert(.qr)
            case 2:
                insert(.code128)
                insert(.code39)
                insert(.code39Checksum)
                insert(.code39FullASCII)
                insert(.code39FullASCIIChecksum)
                insert(.code93)
                insert(.code93i)
                insert(.ean8)
                insert(.ean13)
                insert(.upce)
                insert(.i2of5)
                insert(.i2of5Checksum)
                insert(.itf14)
                insert(.pdf417)
                insert(.aztec)
            case 3:
                insert(.dataMatrix)
            case 4:
                insert(.pdf417)
            default:
                break
            }
        }

        if allowed.isEmpty {
            return Array(supported)
        }

        return Array(allowed)
    }

    private func restartScanLineIfNeeded() {
        guard previewContainer.bounds.height > 0 else { return }
        startScanLineAnimation()
    }

    private func startScanLineAnimation() {
        let bounds = previewContainer.bounds
        guard bounds.height > 0 else { return }

        let startY = bounds.minY + bounds.height * 0.2
        let endY = bounds.minY + bounds.height * 0.8

        scanLine.frame = CGRect(x: bounds.minX,
                                y: startY,
                                width: bounds.width,
                                height: 3)
        scanLine.layer.cornerRadius = 1.5
        scanLine.layer.masksToBounds = true

        let gradient = CAGradientLayer()
        gradient.frame = scanLine.bounds
        gradient.colors = [
            UIColor(red: 0.2, green: 0.65, blue: 1.0, alpha: 0.0).cgColor,
            UIColor(red: 0.2, green: 0.65, blue: 1.0, alpha: 0.8).cgColor,
            UIColor(red: 0.2, green: 0.65, blue: 1.0, alpha: 0.0).cgColor
        ]
        gradient.locations = [0.0, 0.5, 1.0]
        scanLine.layer.sublayers?.forEach { $0.removeFromSuperlayer() }
        scanLine.layer.addSublayer(gradient)

        scanLine.layer.removeAnimation(forKey: "scanLine")
        let animation = CABasicAnimation(keyPath: "position.y")
        animation.fromValue = startY
        animation.toValue = endY
        animation.duration = Constant.animationDuration
        animation.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        animation.autoreverses = true
        animation.repeatCount = .infinity
        scanLine.layer.add(animation, forKey: "scanLine")
        scanLineAnimation = animation
    }

    private func stopScanLineAnimation() {
        scanLine.layer.removeAllAnimations()
        scanLineAnimation = nil
    }

    private func stopSession() {
        guard let session = session, session.isRunning else { return }
        session.stopRunning()
    }

    private func reportSuccess(result: String, type: String) {
        guard !hasReported else { return }
        hasReported = true
        stopScanLineAnimation()
        stopSession()

        let payload: [String: Any] = [
            "scanResult": result,
            "scanType": type
        ]

        let json: String
        if let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
           let string = String(data: data, encoding: .utf8) {
            json = string
        } else {
            json = ""
        }
        let _ = onCallback(callbackId, true, json)
        dismiss(animated: true)
    }

    private func reportCancelled() {
        guard !hasReported else { return }
        hasReported = true
        stopScanLineAnimation()
        stopSession()
        let _ = onCallback(callbackId, true, "{\"cancel\":true}")
        dismiss(animated: true)
    }

    private func reportFailure(_ message: String, code: String = "scan_error") {
        guard !hasReported else { return }
        hasReported = true
        stopScanLineAnimation()
        stopSession()
        let envelope: [String: Any] = ["code": code, "error": message]
        let jsonData = try? JSONSerialization.data(withJSONObject: envelope, options: [])
        let jsonString = jsonData.flatMap { String(data: $0, encoding: .utf8) } ?? message
        let _ = onCallback(callbackId, false, jsonString)
        dismiss(animated: true)
    }

    @objc private func closeTapped() {
        reportCancelled()
    }

    @objc private func openAlbum() {
        stopSession()

        // Use custom MediaPickerViewController for consistent UX (especially in Limited mode)
        MediaPickerViewController.pickSingle(
            from: self,
            mode: "images",
            maxCount: 1
        ) { [weak self] uris in
            guard let self else { return }

            if let uri = uris.first {
                // Load image from phasset URI and detect barcode
                self.loadImageAndDetectBarcode(from: uri)
            } else {
                // User cancelled, restart camera
                self.startSession()
            }
        }
    }

    /// Load image from phasset URI and detect barcode
    private func loadImageAndDetectBarcode(from uri: String) {
        guard uri.hasPrefix("phasset:") else {
            reportFailure("Invalid URI format", code: "scan_invalid_uri")
            return
        }

        let localIdentifier = String(uri.dropFirst("phasset:".count))
        let fetchResult = PHAsset.fetchAssets(withLocalIdentifiers: [localIdentifier], options: nil)

        guard let asset = fetchResult.firstObject else {
            reportFailure("Asset not found", code: "scan_error")
            return
        }

        let options = PHImageRequestOptions()
        options.deliveryMode = .highQualityFormat
        options.isNetworkAccessAllowed = true
        options.isSynchronous = false

        PHImageManager.default().requestImage(
            for: asset,
            targetSize: PHImageManagerMaximumSize,
            contentMode: .aspectFit,
            options: options
        ) { [weak self] image, _ in
            guard let self, let image else {
                DispatchQueue.main.async {
                    self?.reportFailure("Failed to load image", code: "scan_error")
                }
                return
            }

            DispatchQueue.main.async {
                self.detectBarcode(in: image)
            }
        }
    }
}

@available(iOS 13.0, *)
extension ScanCodeViewController: @preconcurrency AVCaptureMetadataOutputObjectsDelegate {
    nonisolated func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let readable = metadataObjects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject }).first,
              let value = readable.stringValue, !value.isEmpty else {
            return
        }

        let rawType = readable.type
        let resultValue = value
        DispatchQueue.main.async { [weak self] in
            guard let self = self, !self.hasReported else { return }
            let typeString = self.mapMetadataTypeToString(rawType)
            self.reportSuccess(result: resultValue, type: typeString)
        }
    }

    private func mapMetadataTypeToString(_ type: AVMetadataObject.ObjectType) -> String {
        switch type {
        case .qr: return "QR_CODE"
        case .dataMatrix: return "DATA_MATRIX"
        case .pdf417: return "PDF_417"
        case .aztec: return "AZTEC"
        case .code128: return "CODE_128"
        case .code39: return "CODE_39"
        case .code39Mod43: return "CODE_39_MOD_43"
        case .code93: return "CODE_93"
        case .ean8: return "EAN_8"
        case .ean13: return "EAN_13"
        case .itf14: return "ITF"
        case .upce: return "UPC_E"
        case .interleaved2of5: return "INTERLEAVED_2_OF_5"
        default: return "UNKNOWN"
        }
    }

    private func mapSymbologyToString(_ symbology: VNBarcodeSymbology) -> String {
        switch symbology {
        case .qr:
            return "QR_CODE"
        case .dataMatrix:
            return "DATA_MATRIX"
        case .pdf417:
            return "PDF_417"
        case .aztec:
            return "AZTEC"
        case .code128:
            return "CODE_128"
        case .code39, .code39Checksum, .code39FullASCII, .code39FullASCIIChecksum:
            return "CODE_39"
        case .code93, .code93i:
            return "CODE_93"
        case .ean8:
            return "EAN_8"
        case .ean13:
            return "EAN_13"
        case .upce:
            return "UPC_E"
        case .i2of5, .i2of5Checksum:
            return "INTERLEAVED_2_OF_5"
        case .itf14:
            return "ITF"
        default:
            return "UNKNOWN"
        }
    }
}

@available(iOS 13.0, *)
private extension ScanCodeViewController {
    /// Detect barcode in the given image using Vision framework
    func detectBarcode(in image: UIImage) {
        guard let cgImage = image.cgImage else {
            DispatchQueue.main.async {
                self.reportFailure("Unable to read image", code: "scan_error")
            }
            return
        }

        let orientation = cgImageOrientation(for: image.imageOrientation)
        let symbologies = allowedVisionSymbologies()
        let allowedSet = Set(symbologies)
        let request = VNDetectBarcodesRequest { [weak self] request, error in
            guard let self else { return }
            if let error {
                DispatchQueue.main.async {
                    self.reportFailure("Scan failed: \(error.localizedDescription)", code: "scan_error")
                }
                return
            }
            guard let observations = request.results as? [VNBarcodeObservation],
                  let match = observations.first(where: {
                      (allowedSet.isEmpty || allowedSet.contains($0.symbology)) && $0.payloadStringValue != nil
                  }),
                  let value = match.payloadStringValue else {
                DispatchQueue.main.async {
                    self.reportFailure("No code detected", code: "scan_no_code")
                }
                return
            }
            let symbology = self.mapSymbologyToString(match.symbology)
            DispatchQueue.main.async {
                self.reportSuccess(result: value, type: symbology)
            }
        }
        if !symbologies.isEmpty {
            request.symbologies = symbologies
        }

        let handler = VNImageRequestHandler(cgImage: cgImage, orientation: orientation, options: [:])
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                try handler.perform([request])
            } catch {
                DispatchQueue.main.async {
                    self.reportFailure("Scan failed: \(error.localizedDescription)", code: "scan_error")
                }
            }
        }
    }

    private func cgImageOrientation(for orientation: UIImage.Orientation) -> CGImagePropertyOrientation {
        switch orientation {
        case .up: return .up
        case .upMirrored: return .upMirrored
        case .down: return .down
        case .downMirrored: return .downMirrored
        case .left: return .left
        case .leftMirrored: return .leftMirrored
        case .right: return .right
        case .rightMirrored: return .rightMirrored
        @unknown default: return .up
        }
    }
}

@available(iOS 13.0, *)
private extension ScanCodeViewController {
    func startSession() {
        guard let session = session, !session.isRunning else { return }
        session.startRunning()
    }

}

#endif
