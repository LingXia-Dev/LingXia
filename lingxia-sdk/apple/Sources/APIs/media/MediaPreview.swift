#if os(iOS)
import UIKit
import AVFoundation
import AVKit
import QuartzCore
import Photos
import PhotosUI
import UniformTypeIdentifiers
import Vision
import CLingXiaSwiftAPI
import CLingXiaRustAPI

extension LxAppMedia {
    struct PreviewMediaPayload: Codable {
        let path: String
        let media_type: Int32
        let cover_path: String?
    }

    nonisolated static func previewMedia(items_json: RustStr) -> Bool {
        let itemsJson = items_json.toString()
        guard let jsonData = itemsJson.data(using: .utf8) else {
            NSLog("[LingXia] Failed to convert items JSON to data")
            return false
        }

        let items: [PreviewMediaPayload]
        do {
            items = try JSONDecoder().decode([PreviewMediaPayload].self, from: jsonData)
        } catch {
            NSLog("[LingXia] Failed to decode items JSON: %@", error.localizedDescription)
            return false
        }
        guard !items.isEmpty else {
            NSLog("[LingXia] previewMedia called with empty items")
            return false
        }

        // Dispatch to main actor for UI operations
        DispatchQueue.main.async {
            guard let presenter = topViewController() else {
                NSLog("[LingXia] Unable to find top view controller for media preview")
                return
            }

            let previewItems = items.map { PreviewMediaItem(payload: $0) }
            let previewController = MediaPreviewViewController(items: previewItems)
            previewController.modalPresentationStyle = .fullScreen
            previewController.modalTransitionStyle = .crossDissolve

            presenter.present(previewController, animated: true)
        }
        return true
    }
}

private struct PreviewMediaItem {
    enum MediaType {
        case image
        case video
        case unknown

        init(rawValue: Int32) {
            switch rawValue {
            case 1:
                self = .video
            case 0:
                self = .image
            default:
                self = .unknown
            }
        }
    }

    let url: URL
    let type: MediaType
    let coverURL: URL?

    init(payload: LxAppMedia.PreviewMediaPayload) {
        let pathString = payload.path
        self.url = URL(fileURLWithPath: pathString)

        let coverString = payload.cover_path ?? ""
        if coverString.isEmpty {
            self.coverURL = nil
        } else if let cover = URL(string: coverString), cover.scheme != nil {
            self.coverURL = cover
        } else {
            self.coverURL = URL(fileURLWithPath: coverString)
        }
        self.type = MediaType(rawValue: payload.media_type)
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

    private lazy var albumButton: UIButton = {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.setTitle("从相册选择", for: .normal)
        button.setTitleColor(.white, for: .normal)
        button.titleLabel?.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        button.backgroundColor = UIColor.black.withAlphaComponent(0.45)
        button.layer.cornerRadius = 20
        button.contentEdgeInsets = UIEdgeInsets(top: 10, left: 16, bottom: 10, right: 16)
        button.addTarget(self, action: #selector(openAlbum), for: .touchUpInside)
        return button
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
                albumButton.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -48)
            ])
        }

        scanLine.translatesAutoresizingMaskIntoConstraints = true
        scanLine.backgroundColor = UIColor(red: 0.2, green: 0.6, blue: 1.0, alpha: 0.4)
        overlayContainer.addSubview(scanLine)
    }

    private func ensureCameraPermission() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureSession()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self else { return }
                    if granted {
                        self.configureSession()
                    } else {
                        self.reportFailure("Camera permission denied")
                    }
                }
            }
        default:
            reportFailure("Camera permission denied")
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

    private func reportFailure(_ message: String) {
        guard !hasReported else { return }
        hasReported = true
        stopScanLineAnimation()
        stopSession()
        let _ = onCallback(callbackId, false, message)
        dismiss(animated: true)
    }

    @objc private func closeTapped() {
        reportCancelled()
    }

    @objc private func openAlbum() {
        if #available(iOS 14.0, *) {
            var configuration = PHPickerConfiguration()
            configuration.filter = .images
            configuration.selectionLimit = 1
            let picker = PHPickerViewController(configuration: configuration)
            picker.delegate = self
            stopSession()
            present(picker, animated: true)
        } else {
            reportFailure("Photo picker requires iOS 14.0 or later")
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

    @available(iOS 14.0, *)
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

@available(iOS 14.0, *)
extension ScanCodeViewController: PHPickerViewControllerDelegate {
    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard let provider = results.first?.itemProvider else {
            if !onlyFromCamera {
                startSession()
            }
            return
        }

        if provider.canLoadObject(ofClass: UIImage.self) {
            provider.loadObject(ofClass: UIImage.self) { [weak self] object, error in
                guard let self else { return }
                if let error {
                    DispatchQueue.main.async {
                        self.presentAlert(message: "Failed to load image: \(error.localizedDescription)")
                        self.startSession()
                    }
                    return
                }
                guard let image = object as? UIImage else {
                    DispatchQueue.main.async {
                        self.presentAlert(message: "Unsupported image format")
                        self.startSession()
                    }
                    return
                }
                self.detectBarcode(in: image)
            }
        } else {
            presentAlert(message: "Unsupported item provider")
            startSession()
        }
    }

    private func detectBarcode(in image: UIImage) {
        guard let cgImage = image.cgImage else {
            DispatchQueue.main.async {
                self.presentAlert(message: "Unable to read image")
                self.startSession()
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
                    self.presentAlert(message: "Scan failed: \(error.localizedDescription)")
                    self.startSession()
                }
                return
            }
            guard let observations = request.results as? [VNBarcodeObservation],
                  let match = observations.first(where: {
                      (allowedSet.isEmpty || allowedSet.contains($0.symbology)) && $0.payloadStringValue != nil
                  }),
                  let value = match.payloadStringValue else {
                DispatchQueue.main.async {
                    self.presentAlert(message: "未识别到码")
                    self.startSession()
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
                    self.presentAlert(message: "Scan failed: \(error.localizedDescription)")
                    self.startSession()
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

    func presentAlert(message: String) {
        let alert = UIAlertController(title: "提示", message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "确定", style: .default))
        present(alert, animated: true)
    }
}

private final class MediaPreviewViewController: UIViewController {
    private let items: [PreviewMediaItem]
    private var currentIndex: Int
    private lazy var closeButton: UIButton = {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.backgroundColor = .clear
        button.tintColor = .white
        button.contentEdgeInsets = .zero
        return button
    }()

    private lazy var pageViewController: UIPageViewController = {
        let controller = UIPageViewController(transitionStyle: .scroll, navigationOrientation: .horizontal)
        controller.dataSource = self
        controller.delegate = self
        controller.view.backgroundColor = .black
        return controller
    }()

    private let indicatorLabel: UILabel = {
        let label = UILabel()
        label.translatesAutoresizingMaskIntoConstraints = false
        label.textColor = .white
        label.font = UIFont.boldSystemFont(ofSize: 17)
        label.textAlignment = .center
        label.shadowColor = UIColor.black.withAlphaComponent(0.5)
        label.shadowOffset = CGSize(width: 0, height: 1)
        return label
    }()

    init(items: [PreviewMediaItem], startIndex: Int = 0) {
        self.items = items
        self.currentIndex = max(0, min(startIndex, items.count - 1))
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var preferredStatusBarStyle: UIStatusBarStyle {
        .lightContent
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        modalPresentationCapturesStatusBarAppearance = true

        addChild(pageViewController)
        pageViewController.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(pageViewController.view)
        pageViewController.didMove(toParent: self)

        NSLayoutConstraint.activate([
            pageViewController.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            pageViewController.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            pageViewController.view.topAnchor.constraint(equalTo: view.topAnchor),
            pageViewController.view.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        view.addSubview(indicatorLabel)
        NSLayoutConstraint.activate([
            indicatorLabel.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            indicatorLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor)
        ])

            if let backImage = LxAppMedia.controlImage(named: "icon_close")?.withRenderingMode(.alwaysOriginal) {
            closeButton.setImage(backImage, for: .normal)
            closeButton.tintColor = .clear
        } else {
            closeButton.setTitle("Back", for: .normal)
            closeButton.setTitleColor(.white, for: .normal)
        }
        closeButton.addTarget(self, action: #selector(closeTapped), for: .touchUpInside)
        view.addSubview(closeButton)
        NSLayoutConstraint.activate([
            closeButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            closeButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            closeButton.widthAnchor.constraint(equalToConstant: 44),
            closeButton.heightAnchor.constraint(equalToConstant: 44)
        ])

        if let initial = viewController(for: currentIndex) {
            pageViewController.setViewControllers([initial], direction: .forward, animated: false)
        }
        setPagerInteraction(enabled: true)
        updateIndicator()
    }

    private func viewController(for index: Int) -> UIViewController? {
        guard items.indices.contains(index) else { return nil }
        let item = items[index]
        switch item.type {
        case .video:
            return MediaPreviewVideoController(item: item, index: index)
        case .image, .unknown:
            return MediaPreviewImageController(item: item, index: index, zoomStateChanged: { [weak self] zoomed in
                self?.setPagerInteraction(enabled: !zoomed)
            }, dismissHandler: { [weak self] in self?.dismiss(animated: true) })
        }
    }

    private func setPagerInteraction(enabled: Bool) {
        pageViewController.dataSource = enabled ? self : nil
        pageViewController.delegate = enabled ? self : nil
        for subview in pageViewController.view.subviews {
            if let scrollView = subview as? UIScrollView {
                scrollView.isScrollEnabled = enabled
            }
        }
    }

    private func updateIndicator() {
        if items.count <= 1 {
            indicatorLabel.isHidden = true
        } else {
            indicatorLabel.isHidden = false
            indicatorLabel.text = "\(currentIndex + 1)/\(items.count)"
        }
    }

    @objc private func closeTapped() {
        dismiss(animated: true)
    }
}

extension MediaPreviewViewController: UIPageViewControllerDataSource, UIPageViewControllerDelegate {
    func pageViewController(_ pageViewController: UIPageViewController, viewControllerBefore viewController: UIViewController) -> UIViewController? {
        guard let current = viewController as? IndexedPreviewController else { return nil }
        let previous = current.index - 1
        return self.viewController(for: previous)
    }

    func pageViewController(_ pageViewController: UIPageViewController, viewControllerAfter viewController: UIViewController) -> UIViewController? {
        guard let current = viewController as? IndexedPreviewController else { return nil }
        let next = current.index + 1
        return self.viewController(for: next)
    }

    func pageViewController(_ pageViewController: UIPageViewController, didFinishAnimating finished: Bool, previousViewControllers: [UIViewController], transitionCompleted completed: Bool) {
        guard completed, let current = pageViewController.viewControllers?.first as? IndexedPreviewController else { return }
        currentIndex = current.index
        updateIndicator()
    }
}

private protocol IndexedPreviewController where Self: UIViewController {
    var index: Int { get }
}

private final class MediaPreviewImageController: UIViewController, IndexedPreviewController {
    let index: Int
    private let item: PreviewMediaItem
    private let zoomStateChanged: (Bool) -> Void
    private let dismissHandler: () -> Void

    private lazy var zoomView: ZoomableImageView = {
        let view = ZoomableImageView(imageURL: item.url)
        view.translatesAutoresizingMaskIntoConstraints = false
        view.onZoomStateChanged = zoomStateChanged
        view.onDismiss = dismissHandler
        return view
    }()

    init(item: PreviewMediaItem, index: Int, zoomStateChanged: @escaping (Bool) -> Void, dismissHandler: @escaping () -> Void) {
        self.item = item
        self.zoomStateChanged = zoomStateChanged
        self.dismissHandler = dismissHandler
        self.index = index
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        view.addSubview(zoomView)
        NSLayoutConstraint.activate([
            zoomView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            zoomView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            zoomView.topAnchor.constraint(equalTo: view.topAnchor),
            zoomView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
    }
}

@MainActor
private final class MediaPreviewVideoController: UIViewController, IndexedPreviewController {
    let index: Int
    private let item: PreviewMediaItem

    private var playerVC: AVPlayerViewController?
    private var player: AVPlayer?
    private var coverOverlay: UIImageView?
    private var timeObserver: Any?
    private var hasStartedPlayback = false

    init(item: PreviewMediaItem, index: Int) {
        self.item = item
        self.index = index
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black

        // Embed native player inline
        embedPlayerInline()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        player?.pause()
        hasStartedPlayback = false
        cleanupTimeObserver()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        startPlaybackIfNeeded()
    }

    private func embedPlayerInline() {
        let playerItem = AVPlayerItem(url: item.url)
        let player = AVPlayer(playerItem: playerItem)
        player.automaticallyWaitsToMinimizeStalling = true
        self.player = player

        let vc = AVPlayerViewController()
        vc.player = player
        vc.showsPlaybackControls = true
        vc.entersFullScreenWhenPlaybackBegins = false
        vc.exitsFullScreenWhenPlaybackEnds = false
        vc.view.translatesAutoresizingMaskIntoConstraints = false
        addChild(vc)
        view.addSubview(vc.view)
        NSLayoutConstraint.activate([
            vc.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            vc.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            vc.view.topAnchor.constraint(equalTo: view.topAnchor),
            vc.view.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        vc.didMove(toParent: self)
        playerVC = vc

        // If a local cover is provided, overlay it until playback starts
        if let coverURL = item.coverURL, coverURL.isFileURL,
           let image = UIImage(contentsOfFile: coverURL.path) {
            let overlay = UIImageView(image: image)
            overlay.translatesAutoresizingMaskIntoConstraints = false
            overlay.contentMode = .scaleAspectFit
            overlay.backgroundColor = .black
            (vc.contentOverlayView ?? vc.view).addSubview(overlay)
            NSLayoutConstraint.activate([
                overlay.leadingAnchor.constraint(equalTo: vc.view.leadingAnchor),
                overlay.trailingAnchor.constraint(equalTo: vc.view.trailingAnchor),
                overlay.topAnchor.constraint(equalTo: vc.view.topAnchor),
                overlay.bottomAnchor.constraint(equalTo: vc.view.bottomAnchor)
            ])
            coverOverlay = overlay

            // Hide overlay when playback actually starts
            timeObserver = player.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.1, preferredTimescale: 600), queue: .main) { [weak self] _ in
                guard let self, let player = self.player else { return }
                if player.rate > 0 {
                    self.hideCoverOverlay()
                }
            }
        }
    }

    private func hideCoverOverlay() {
        cleanupTimeObserver()
    }

    private func cleanupTimeObserver() {
        if let overlay = coverOverlay {
            overlay.removeFromSuperview()
            coverOverlay = nil
        }
        if let observer = timeObserver, let player {
            player.removeTimeObserver(observer)
            timeObserver = nil
        }
    }

    private func startPlaybackIfNeeded() {
        guard !hasStartedPlayback else { return }
        guard let player else { return }
        hasStartedPlayback = true
        player.play()
    }

}

private final class ZoomableImageView: UIView, UIScrollViewDelegate {
    let imageURL: URL
    var onZoomStateChanged: ((Bool) -> Void)?
    var onDismiss: (() -> Void)?

    private let scrollView = UIScrollView()
    private let imageView = UIImageView()
    private let activityIndicator = UIActivityIndicatorView(style: .large)

    init(imageURL: URL) {
        self.imageURL = imageURL
        super.init(frame: .zero)
        configure()
        loadImage()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func configure() {
        backgroundColor = .black

        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.delegate = self
        scrollView.minimumZoomScale = 1
        scrollView.maximumZoomScale = 6
        scrollView.showsHorizontalScrollIndicator = false
        scrollView.showsVerticalScrollIndicator = false
        scrollView.bouncesZoom = true
        scrollView.decelerationRate = .fast
        scrollView.alwaysBounceVertical = false
        scrollView.alwaysBounceHorizontal = false
        addSubview(scrollView)

        imageView.translatesAutoresizingMaskIntoConstraints = false
        imageView.contentMode = .scaleAspectFit
        scrollView.addSubview(imageView)

        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.hidesWhenStopped = true
        addSubview(activityIndicator)

        NSLayoutConstraint.activate([
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: topAnchor),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),

            imageView.centerXAnchor.constraint(equalTo: scrollView.centerXAnchor),
            imageView.centerYAnchor.constraint(equalTo: scrollView.centerYAnchor),
            imageView.widthAnchor.constraint(equalTo: scrollView.widthAnchor),
            imageView.heightAnchor.constraint(equalTo: scrollView.heightAnchor),

            activityIndicator.centerXAnchor.constraint(equalTo: centerXAnchor),
            activityIndicator.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])

        let singleTap = UITapGestureRecognizer(target: self, action: #selector(handleSingleTap))
        addGestureRecognizer(singleTap)

        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(handleDoubleTap(_:)))
        doubleTap.numberOfTapsRequired = 2
        addGestureRecognizer(doubleTap)

        singleTap.require(toFail: doubleTap)
    }

    private func loadImage() {
        activityIndicator.startAnimating()
        Task {
            // Local-only preview: load from filesystem
            let image = UIImage(contentsOfFile: imageURL.path)

            await MainActor.run {
                activityIndicator.stopAnimating()
                if let image {
                    imageView.image = image
                    resetZoom()
                } else {
                    imageView.image = UIImage(systemName: "exclamationmark.triangle")
                    imageView.tintColor = .white
                }
            }
        }
    }

    private func resetZoom() {
        scrollView.setZoomScale(1, animated: false)
        notifyScaleState()
    }

    @objc private func handleSingleTap() {
        onDismiss?()
    }

    @objc private func handleDoubleTap(_ gesture: UITapGestureRecognizer) {
        let currentScale = scrollView.zoomScale
        let targetScale: CGFloat

        if currentScale > 1.2 {
            targetScale = 1.0
        } else if currentScale < 2.0 {
            targetScale = 3.0
        } else {
            targetScale = scrollView.maximumZoomScale
        }

        let point = gesture.location(in: imageView)
        zoom(to: targetScale, center: point)
    }

    private func zoom(to scale: CGFloat, center: CGPoint) {
        UIView.animate(withDuration: 0.3, delay: 0, options: [.curveEaseInOut, .allowUserInteraction]) {
            let zoomRect = self.zoomRectForScale(scale, center: center)
            self.scrollView.zoom(to: zoomRect, animated: false)
        }
    }

    private func zoomRectForScale(_ scale: CGFloat, center: CGPoint) -> CGRect {
        let width = scrollView.bounds.size.width / scale
        let height = scrollView.bounds.size.height / scale
        let origin = CGPoint(x: center.x - (width / 2), y: center.y - (height / 2))
        return CGRect(origin: origin, size: CGSize(width: width, height: height))
    }

    func viewForZooming(in scrollView: UIScrollView) -> UIView? {
        imageView
    }

    func scrollViewDidZoom(_ scrollView: UIScrollView) {
        notifyScaleState()
        centerImageView()
    }

    func scrollViewDidEndZooming(_ scrollView: UIScrollView, with view: UIView?, atScale scale: CGFloat) {
        // 添加弹性效果
        if scale < scrollView.minimumZoomScale {
            UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut]) {
                scrollView.setZoomScale(scrollView.minimumZoomScale, animated: false)
            }
        }
    }

    private func centerImageView() {
        let offsetX = max((scrollView.bounds.width - scrollView.contentSize.width) * 0.5, 0)
        let offsetY = max((scrollView.bounds.height - scrollView.contentSize.height) * 0.5, 0)
        imageView.center = CGPoint(x: scrollView.contentSize.width * 0.5 + offsetX,
                                   y: scrollView.contentSize.height * 0.5 + offsetY)
    }

    private func notifyScaleState() {
        onZoomStateChanged?(scrollView.zoomScale > 1.05)
    }
}

// MARK: - Choose Media

extension LxAppMedia {
    @MainActor static var albumPickerDelegate: AlbumDelegate?
}
#endif
