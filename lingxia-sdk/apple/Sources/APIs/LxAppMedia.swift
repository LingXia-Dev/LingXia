import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
import AVFoundation
import AVKit
import QuartzCore
import Photos
import PhotosUI
import UniformTypeIdentifiers
import AudioToolbox
import Vision

private extension UIImage {
    private final class BundleToken {}

    static func lx_control(named name: String) -> UIImage? {
        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: BundleToken.self)
        #endif

        if let image = UIImage(named: name, in: bundle, compatibleWith: nil) {
            return image
        }

        if let pdfURL = bundle.url(forResource: name, withExtension: "pdf") {
            return UIImage.renderingPDF(at: pdfURL)
        }

        return nil
    }

    private static func renderingPDF(at url: URL) -> UIImage? {
        guard
            let dataProvider = CGDataProvider(url: url as CFURL),
            let document = CGPDFDocument(dataProvider),
            let page = document.page(at: 1)
        else {
            return nil
        }

        let pageRect = page.getBoxRect(.mediaBox)
        let renderer = UIGraphicsImageRenderer(size: pageRect.size)
        return renderer.image { context in
            let cgContext = context.cgContext
            cgContext.saveGState()
            cgContext.translateBy(x: 0, y: pageRect.height)
            cgContext.scaleBy(x: 1, y: -1)
            cgContext.drawPDFPage(page)
            cgContext.restoreGState()
        }.withRenderingMode(.alwaysOriginal)
    }
}
#endif

#if os(iOS)
private enum CaptureFeedback {
    private static func play(soundID: SystemSoundID) {
        AudioServicesPlaySystemSound(soundID)
        AudioServicesPlaySystemSound(kSystemSoundID_Vibrate)
        if #available(iOS 13.0, *) {
            let generator = UIImpactFeedbackGenerator(style: .medium)
            generator.prepare()
            generator.impactOccurred()
        } else {
            AudioServicesPlaySystemSound(kSystemSoundID_Vibrate)
        }
    }

    static func playShutter() {
        play(soundID: 1108)
    }

    static func playRecordStart() {
        play(soundID: 1117)
    }

    static func playRecordStop() {
        play(soundID: 1118)
    }
}
#endif

@MainActor
enum LxAppMedia {
    nonisolated(unsafe) private static let log = OSLog(subsystem: "LingXia", category: "Media")

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

        #if os(iOS)
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
        #else
        NSLog("[LingXia] previewMedia not supported on this platform")
        return false
        #endif
    }

    #if os(iOS)
    private static func topViewController(base: UIViewController? = UIApplication.shared.connectedScenes
        .compactMap { $0 as? UIWindowScene }
        .flatMap { $0.windows }
        .first(where: { $0.isKeyWindow })?.rootViewController) -> UIViewController? {
        if let nav = base as? UINavigationController {
            return topViewController(base: nav.visibleViewController)
        }
        if let tab = base as? UITabBarController {
            if let selected = tab.selectedViewController {
                return topViewController(base: selected)
            }
        }
        if let presented = base?.presentedViewController {
            return topViewController(base: presented)
        }
        return base
    }
    #endif
}

#if os(iOS)
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
private final class ScanCodeViewController: UIViewController {
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
        button.setImage(.lx_control(named: "icon_close"), for: .normal)
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
                  let match = observations.first(where: { $0.payloadStringValue != nil }),
                  let value = match.payloadStringValue else {
                DispatchQueue.main.async {
                    self.presentAlert(message: "未识别到码")
                    self.startSession()
                }
                return
            }
            let symbology = match.symbology.rawValue.uppercased()
            DispatchQueue.main.async {
                self.reportSuccess(result: value, type: symbology)
            }
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

        if let backImage = UIImage.lx_control(named: "icon_close")?.withRenderingMode(.alwaysOriginal) {
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
#if os(iOS)
    @MainActor private static var albumPickerDelegate: AlbumDelegate?
#endif

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

    nonisolated static func chooseMedia(
        max_count: UInt32,
        mode: RustStr,
        source_types_json: RustStr,
        camera_facing: RustStr,
        max_duration: RustStr,
        callback_id: UInt64
    ) -> Bool {
        let modeStr = mode.toString()
        let sourceTypesJson = source_types_json.toString()
        let cameraFacingStr = camera_facing.toString()
        let maxDurationStr = max_duration.toString()


        #if os(iOS)
        DispatchQueue.main.async {
            guard let presenter = topViewController() else {
                let _ = onCallback(callback_id, false, "Unable to find top view controller")
                return
            }

            // Parse source types
            guard let sourceTypesData = sourceTypesJson.data(using: .utf8),
                  let sourceTypes = try? JSONDecoder().decode([String].self, from: sourceTypesData) else {
                let _ = onCallback(callback_id, false, "Failed to parse source types")
                return
            }

            let allowAlbum = sourceTypes.contains("album")
            let allowCamera = sourceTypes.contains("camera")

            if allowCamera {
                openCamera(
                    presenter: presenter,
                    mode: modeStr,
                    cameraFacing: cameraFacingStr,
                    maxDuration: maxDurationStr,
                    callbackId: callback_id
                )
            } else if allowAlbum {
                openAlbum(presenter: presenter, mode: modeStr, maxCount: max_count, callbackId: callback_id)
            } else {
                let _ = onCallback(callback_id, false, "No supported source types")
            }
        }
        return true
        #else
        return false
        #endif
    }

    private static func openCamera(
        presenter: UIViewController,
        mode: String,
        cameraFacing: String,
        maxDuration: String,
        callbackId: UInt64
    ) {

        guard UIImagePickerController.isSourceTypeAvailable(.camera) else {
            let _ = onCallback(callbackId, false, "Camera is not available on this device")
            return
        }

        checkCameraPermission { granted in
            guard granted else {
                let _ = onCallback(callbackId, false, "Camera access is required to capture media. Please enable access in Settings > Privacy & Security > Camera.")
                return
            }

            let modeLowercased = mode.lowercased()
            let desiredFacingFront = cameraFacing.lowercased() == "front"

            if modeLowercased == "video" {
                checkMicrophonePermission { micGranted in
                    guard micGranted else {
                        let _ = onCallback(callbackId, false, "Microphone access is required to record video. Please enable access in Settings > Privacy & Security > Microphone.")
                        return
                    }

                    let maxDurationValue = Double(maxDuration).flatMap { $0 > 0 ? $0 : nil }
                    let initialPosition: AVCaptureDevice.Position = desiredFacingFront ? .front : .back

                    let videoController = VideoCaptureViewController(
                        initialCameraPosition: initialPosition,
                        maxDuration: maxDurationValue
                    ) { result in
                        switch result {
                        case .cancelled:
                            let _ = onCallback(callbackId, true, "{\"cancel\":true}")
                        case .failure(let message):
                            let _ = onCallback(callbackId, false, message)
                        case .success(let fileURL):
                            let copiedURL = copyMediaFileToTemp(
                                from: fileURL,
                                prefix: "camera_video",
                                fallbackExtension: "mov",
                                requiresSecurityScope: false
                            )
                            let finalURL = copiedURL ?? fileURL
                            if finalURL != fileURL {
                                try? FileManager.default.removeItem(at: fileURL)
                            }
                            let jsonItem: [String: Any] = [
                                "uri": finalURL.absoluteString,
                                "fileType": "video",
                                "isOriginal": true
                            ]
                            if let data = try? JSONSerialization.data(withJSONObject: [jsonItem], options: []),
                               let jsonString = String(data: data, encoding: .utf8) {
                                let _ = onCallback(callbackId, true, jsonString)
                            } else {
                                let _ = onCallback(callbackId, false, "Failed to serialize camera capture result")
                            }
                        }
                    }

                    presenter.present(videoController, animated: true)
                }
                return
            }

            let initialPosition: AVCaptureDevice.Position = desiredFacingFront ? .front : .back
            let photoController = PhotoCaptureViewController(initialCameraPosition: initialPosition) { result in
                switch result {
                case .cancelled:
                    let _ = onCallback(callbackId, true, "{\"cancel\":true}")
                case .failure(let message):
                    let _ = onCallback(callbackId, false, message)
                case .success(let fileURL):
                    let copiedURL = copyMediaFileToTemp(
                        from: fileURL,
                        prefix: "camera_image",
                        fallbackExtension: "jpg",
                        requiresSecurityScope: false
                    )
                    let finalURL = copiedURL ?? fileURL
                    if finalURL != fileURL {
                        try? FileManager.default.removeItem(at: fileURL)
                    }
                    let jsonItem: [String: Any] = [
                        "uri": finalURL.absoluteString,
                        "fileType": "image",
                        "isOriginal": true
                    ]

                    if let data = try? JSONSerialization.data(withJSONObject: [jsonItem], options: []),
                       let jsonString = String(data: data, encoding: .utf8) {
                        let _ = onCallback(callbackId, true, jsonString)
                    } else {
                        let _ = onCallback(callbackId, false, "Failed to serialize camera capture result")
                    }
                }
            }
            presenter.present(photoController, animated: true)
        }
    }

    private static func checkCameraPermission(completion: @escaping (Bool) -> Void) {
        let status = AVCaptureDevice.authorizationStatus(for: .video)
        switch status {
        case .authorized:
            completion(true)
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                DispatchQueue.main.async {
                    completion(granted)
                }
            }
        case .denied, .restricted:
            completion(false)
        @unknown default:
            completion(false)
        }
    }

    private static func checkMicrophonePermission(completion: @escaping (Bool) -> Void) {
        let audioSession = AVAudioSession.sharedInstance()
        switch audioSession.recordPermission {
        case .granted:
            completion(true)
        case .denied:
            completion(false)
        case .undetermined:
            audioSession.requestRecordPermission { granted in
                DispatchQueue.main.async {
                    completion(granted)
                }
            }
        @unknown default:
            completion(false)
        }
    }

    private static func openAlbum(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {

        // Check if PHPickerViewController is available (iOS 14+)
        if #available(iOS 14.0, *) {
            // PHPickerViewController doesn't require explicit permission, but we should check photo library access
            checkPhotoLibraryPermission { hasPermission in
                if hasPermission {
                    presentPhotoPicker(presenter: presenter, mode: mode, maxCount: maxCount, callbackId: callbackId)
                } else {
                    // Send error callback for permission denied
                    let _ = onCallback(callbackId, false, "Photo library access is required to select photos. Please enable access in Settings > Privacy & Security > Photos.")
                }
            }
        } else {
            // For iOS 13 and below, we would need to use UIImagePickerController with permission checks
            let _ = onCallback(callbackId, false, "Photo picker requires iOS 14.0 or later")
        }
    }

    private static func checkPhotoLibraryPermission(completion: @escaping (Bool) -> Void) {
        let deliver: (Bool) -> Void = { granted in
            if Thread.isMainThread {
                completion(granted)
            } else {
                DispatchQueue.main.async {
                    completion(granted)
                }
            }
        }

        if #available(iOS 14.0, *) {
            let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)

            switch status {
            case .authorized, .limited:
                deliver(true)
            case .notDetermined:
                PHPhotoLibrary.requestAuthorization(for: .readWrite) { newStatus in
                    let granted = newStatus == .authorized || newStatus == .limited
                    deliver(granted)
                }
            case .denied:
                deliver(false)
            case .restricted:
                deliver(false)
            @unknown default:
                deliver(false)
            }
        } else {
            deliver(false)
        }
    }

    private static func presentPhotoPicker(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {

        var configuration = PHPickerConfiguration()
        configuration.selectionLimit = Int(maxCount)

        // Set media type based on mode
        switch mode.lowercased() {
        case "video":
            configuration.filter = .videos
        case "image":
            configuration.filter = .images
        default: // mix
            configuration.filter = .any(of: [.images, .videos])
        }

        let picker = PHPickerViewController(configuration: configuration)
        let delegate = AlbumDelegate(callbackId: callbackId) {
            LxAppMedia.albumPickerDelegate = nil
        }
        albumPickerDelegate = delegate
        picker.delegate = delegate
        presenter.present(picker, animated: true)
    }
}

// MARK: - Album Delegate
private class AlbumDelegate: NSObject, PHPickerViewControllerDelegate {
    private let callbackId: UInt64
    private let cleanup: () -> Void

    init(callbackId: UInt64, cleanup: @escaping () -> Void) {
        self.callbackId = callbackId
        self.cleanup = cleanup
        super.init()
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {

        picker.dismiss(animated: true)

        guard !results.isEmpty else {
            sendCancel()
            cleanup()
            return
        }

        var jsonArray: [[String: Any]] = []
        let group = DispatchGroup()
        
        for result in results {
            group.enter()
            handleResult(result) { item in
                if let item {
                    jsonArray.append(item)
                }
                group.leave()
            }
        }
        
        group.notify(queue: .main) {
            self.sendCallback(jsonArray: jsonArray)
            self.cleanup()
        }
    }

    private func handleResult(_ result: PHPickerResult, completion: @escaping ([String: Any]?) -> Void) {
        guard #available(iOS 14.0, *) else {
            DispatchQueue.main.async {
                    completion(nil)
            }
                    return
                }
                
        let provider = result.itemProvider

        if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
            provider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { url, error in
                if error != nil {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }

                guard let url else {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }

                let tempURL = copyMediaFileToTemp(
                    from: url,
                    prefix: "album_image",
                    fallbackExtension: "jpg",
                    requiresSecurityScope: true
                )
            DispatchQueue.main.async {
                    if let tempURL {
                        let jsonItem: [String: Any] = [
                            "uri": tempURL.absoluteString,
                            "fileType": "image",
                            "isOriginal": true
                        ]
                        completion(jsonItem)
                    } else {
                    completion(nil)
                    }
                }
            }
        } else if provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
            provider.loadFileRepresentation(forTypeIdentifier: UTType.movie.identifier) { url, error in
                if error != nil {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }
                
                guard let url else {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }

                let tempURL = copyMediaFileToTemp(
                    from: url,
                    prefix: "album_video",
                    fallbackExtension: "mov",
                    requiresSecurityScope: true
                )
                DispatchQueue.main.async {
                    if let tempURL {
                        let jsonItem: [String: Any] = [
                            "uri": tempURL.absoluteString,
                            "fileType": "video",
                            "isOriginal": true
                        ]
                        completion(jsonItem)
                    } else {
                        completion(nil)
                    }
                }
            }
        } else {
            DispatchQueue.main.async {
                completion(nil)
            }
        }
    }

    private func sendCallback(jsonArray: [[String: Any]]) {
        do {
            let jsonData = try JSONSerialization.data(withJSONObject: jsonArray, options: [])
            let jsonString = String(data: jsonData, encoding: .utf8) ?? "[]"

            let _ = onCallback(callbackId, true, jsonString)
        } catch {
            let _ = onCallback(callbackId, false, "Failed to serialize album data")
        }
    }

    private func sendCancel() {
        let _ = onCallback(callbackId, true, "{\"cancel\":true}")
    }
}
#endif

#if os(iOS)
private enum PhotoCaptureResult {
    case success(URL)
    case cancelled
    case failure(String)
}

private enum PhotoCaptureHint {
    static let preparing = "准备相机..."
    static let ready = "点击拍照"
    static let switching = "切换摄像头..."
}

private final class PhotoCaptureViewController: UIViewController {
    private let resultHandler: (PhotoCaptureResult) -> Void
    private var currentPosition: AVCaptureDevice.Position

    private let session = AVCaptureSession()
    private let sessionQueue = DispatchQueue(label: "com.lingxia.camera.photo.session")
    private let photoOutput = AVCapturePhotoOutput()
    private var videoInput: AVCaptureDeviceInput?
    private var previewLayer: AVCaptureVideoPreviewLayer?

    private let overlayView = PhotoCaptureOverlayView()
    private lazy var photoDelegate = PhotoCaptureDelegate(controller: self)

    private var isSessionConfigured = false
    private var isCapturingPhoto = false
    private var flashEnabled = false
    private var pendingPhotoURL: URL?

    init(initialCameraPosition: AVCaptureDevice.Position, resultHandler: @escaping (PhotoCaptureResult) -> Void) {
        self.currentPosition = initialCameraPosition
        self.resultHandler = resultHandler
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .fullScreen
        modalPresentationCapturesStatusBarAppearance = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var prefersStatusBarHidden: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configurePreviewLayer()
        configureOverlay()
        configureSession { [weak self] in
            guard let self else { return }
            self.overlayView.setBusy(false)
            self.overlayView.showHint(PhotoCaptureHint.ready)
            self.updateCameraSwitchAvailability()
            self.refreshFlashAvailability()
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        startSession()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        setFlashEnabled(false)
        stopSession()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    private func configurePreviewLayer() {
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.addSublayer(layer)
        previewLayer = layer
    }

    private func configureOverlay() {
        overlayView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(overlayView)
        NSLayoutConstraint.activate([
            overlayView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlayView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlayView.topAnchor.constraint(equalTo: view.topAnchor),
            overlayView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        overlayView.onCaptureTapped = { [weak self] in
            self?.capturePhoto()
        }
        overlayView.onCancelTapped = { [weak self] in
            self?.handleCancel()
        }
        overlayView.onSwitchCameraTapped = { [weak self] in
            self?.switchCamera()
        }
        overlayView.onFlashToggle = { [weak self] isOn in
            self?.setFlashEnabled(isOn)
        }
        overlayView.setFlashAvailable(false)
        overlayView.setFlashEnabled(flashEnabled)

        overlayView.showHint(PhotoCaptureHint.preparing)
        overlayView.setBusy(true)
        overlayView.updateCameraPosition(isFront: currentPosition == .front)
    }

    private func configureSession(completion: (() -> Void)? = nil) {
        sessionQueue.async {
            self.configureSessionOnQueue(completion: completion)
        }
    }

    private func configureSessionOnQueue(completion: (() -> Void)?) {
        self.isSessionConfigured = false
        self.session.beginConfiguration()
        self.session.sessionPreset = .photo

        if let input = self.videoInput {
            self.session.removeInput(input)
            self.videoInput = nil
        }

        guard let device = self.cameraDevice(position: self.currentPosition) else {
            self.session.commitConfiguration()
            DispatchQueue.main.async {
                self.finish(with: .failure("Camera device not available"))
            }
            return
        }

        do {
            let input = try AVCaptureDeviceInput(device: device)
            if self.session.canAddInput(input) {
                self.session.addInput(input)
                self.videoInput = input
            } else {
                throw NSError(domain: "LingXia.Camera", code: -1, userInfo: [NSLocalizedDescriptionKey: "Unable to add camera input"])
            }
        } catch {
            self.session.commitConfiguration()
            DispatchQueue.main.async {
                self.finish(with: .failure("Failed to configure camera input"))
            }
            return
        }

        if !self.session.outputs.contains(self.photoOutput) {
            if self.session.canAddOutput(self.photoOutput) {
                self.session.addOutput(self.photoOutput)
            } else {
                self.session.commitConfiguration()
                DispatchQueue.main.async {
                    self.finish(with: .failure("Failed to configure photo output"))
                }
                return
            }
        }
        self.photoOutput.isHighResolutionCaptureEnabled = true
        if #available(iOS 16.0, *) {
            self.photoOutput.maxPhotoQualityPrioritization = .quality
        }

        if let connection = self.photoOutput.connection(with: .video), connection.isVideoOrientationSupported {
            connection.videoOrientation = .portrait
        }

        self.session.commitConfiguration()
        self.updateFlashAvailability(for: device)
        self.isSessionConfigured = true
        self.startSession()

        if let completion {
            DispatchQueue.main.async {
                completion()
            }
        }
    }

    private func startSession() {
        sessionQueue.async {
            guard self.isSessionConfigured, !self.session.isRunning else { return }
            self.session.startRunning()
        }
    }

    private func stopSession() {
        sessionQueue.async {
            guard self.session.isRunning else { return }
            self.session.stopRunning()
        }
    }

    private func capturePhoto() {
        guard !isCapturingPhoto else { return }
        isCapturingPhoto = true
        CaptureFeedback.playShutter()

        let settings = AVCapturePhotoSettings()
        settings.isHighResolutionPhotoEnabled = true
        let desiredFlashMode: AVCaptureDevice.FlashMode = flashEnabled ? .on : .off
        if photoOutput.supportedFlashModes.contains(desiredFlashMode) {
            settings.flashMode = desiredFlashMode
        } else if photoOutput.supportedFlashModes.contains(.auto) {
            settings.flashMode = .auto
        } else if photoOutput.supportedFlashModes.contains(.off) {
            settings.flashMode = .off
        }
        if #available(iOS 11.0, *) {
            settings.isAutoStillImageStabilizationEnabled = true
            if photoOutput.isDepthDataDeliverySupported {
                settings.isDepthDataDeliveryEnabled = false
            }
        }

        sessionQueue.async { [weak self] in
            guard let self else { return }
            self.photoOutput.capturePhoto(with: settings, delegate: self.photoDelegate)
        }
    }

    private func switchCamera() {
        currentPosition = currentPosition == .front ? .back : .front
        overlayView.updateCameraPosition(isFront: currentPosition == .front)
        overlayView.showHint(PhotoCaptureHint.switching)

        configureSession { [weak self] in
            guard let self else { return }
            self.overlayView.showHint(PhotoCaptureHint.ready)
            self.updateCameraSwitchAvailability()
            self.refreshFlashAvailability()
        }
    }

    private func updateCameraSwitchAvailability() {
        let discovery = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInWideAngleCamera, .builtInDualCamera, .builtInDualWideCamera, .builtInTripleCamera],
            mediaType: .video,
            position: .unspecified
        )
        let positions = Set(discovery.devices.map { $0.position })
        overlayView.setSwitchAvailable(positions.contains(.front) && positions.contains(.back))
    }

    private func setFlashEnabled(_ enabled: Bool) {
        flashEnabled = enabled
        DispatchQueue.main.async {
            self.overlayView.setFlashEnabled(enabled)
        }
    }

    private func updateFlashAvailability(for device: AVCaptureDevice) {
        let available = device.hasFlash && device.isFlashAvailable
        DispatchQueue.main.async {
            self.overlayView.setFlashAvailable(available)
            if !available {
                self.setFlashEnabled(false)
            }
        }
    }

    private func refreshFlashAvailability() {
        if let device = videoInput?.device {
            updateFlashAvailability(for: device)
        } else {
            DispatchQueue.main.async {
                self.overlayView.setFlashAvailable(false)
                self.setFlashEnabled(false)
            }
        }
    }

    private func handleCancel() {
        finish(with: .cancelled)
    }

    private func finish(with result: PhotoCaptureResult) {
        stopSession()
        dismiss(animated: true) {
            self.resultHandler(result)
        }
    }

    private func cameraDevice(position: AVCaptureDevice.Position) -> AVCaptureDevice? {
        let discovery = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInDualCamera, .builtInWideAngleCamera, .builtInTripleCamera, .builtInDualWideCamera],
            mediaType: .video,
            position: position
        )
        return discovery.devices.first ?? AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: position)
    }

    @MainActor
    fileprivate func handleCapturedPhoto(data: Data?, error: Error?) {
        overlayView.setBusy(false)
        isCapturingPhoto = false

        if let error {
            overlayView.showHint(PhotoCaptureHint.ready)
            finish(with: .failure(error.localizedDescription))
            return
        }

        guard let data, let url = savePhotoData(data) else {
            overlayView.showHint(PhotoCaptureHint.ready)
            finish(with: .failure("Failed to save captured photo"))
            return
        }

        presentReview(for: url)
    }

    private func savePhotoData(_ data: Data) -> URL? {
        let tempDir = FileManager.default.temporaryDirectory
        let fileURL = tempDir.appendingPathComponent("camera_image_\(UUID().uuidString).jpg")
        do {
            try data.write(to: fileURL, options: .atomic)
            return fileURL
        } catch {
            return nil
        }
    }

    private func presentReview(for url: URL) {
        pendingPhotoURL = url
        overlayView.isHidden = true

        let reviewController = PhotoReviewViewController(
            imageURL: url,
            onRetake: { [weak self] in
                self?.handleRetakeFromReview()
            },
            onConfirm: { [weak self] in
                self?.handleConfirmFromReview()
            }
        )
        reviewController.modalPresentationStyle = .fullScreen
        present(reviewController, animated: true)
    }

    private func handleRetakeFromReview() {
        guard let url = pendingPhotoURL else { return }
        pendingPhotoURL = nil
        if FileManager.default.fileExists(atPath: url.path) {
            try? FileManager.default.removeItem(at: url)
        }
        overlayView.isHidden = false
        overlayView.setBusy(false)
        overlayView.showHint(PhotoCaptureHint.ready)
    }

    private func handleConfirmFromReview() {
        guard let url = pendingPhotoURL else { return }
        pendingPhotoURL = nil
        finish(with: .success(url))
    }
}

private final class PhotoCaptureDelegate: NSObject, AVCapturePhotoCaptureDelegate {
    private weak var controller: PhotoCaptureViewController?

    init(controller: PhotoCaptureViewController) {
        self.controller = controller
    }

    func photoOutput(_ output: AVCapturePhotoOutput, didFinishProcessingPhoto photo: AVCapturePhoto, error: Error?) {
        let data = photo.fileDataRepresentation()
        guard let controller else { return }
        DispatchQueue.main.async {
            controller.handleCapturedPhoto(data: data, error: error)
        }
    }
}

@MainActor
private enum ReviewButtonFactory {
    private static let primaryColor = UIColor(red: 0x33/255.0, green: 0x70/255.0, blue: 1.0, alpha: 1.0)

    static func makeConfirmButton(title: String, target: Any?, action: Selector) -> UIButton {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.setTitle(title, for: .normal)
        button.titleLabel?.font = UIFont.systemFont(ofSize: 17, weight: .semibold)
        button.setTitleColor(.white, for: .normal)
        button.backgroundColor = primaryColor.withAlphaComponent(0.8)
        button.layer.cornerRadius = 10
        button.addTarget(target, action: action, for: .touchUpInside)
        return button
    }

    static func makeRetakeActionButton(target: Any?, action: Selector) -> UIButton {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.setTitle("重新拍摄", for: .normal)
        button.titleLabel?.font = UIFont.systemFont(ofSize: 17, weight: .semibold)
        button.setTitleColor(primaryColor, for: .normal)
        button.backgroundColor = UIColor.white.withAlphaComponent(0.8)
        button.layer.cornerRadius = 10
        button.layer.borderWidth = 1
        button.layer.borderColor = primaryColor.cgColor
        button.addTarget(target, action: action, for: .touchUpInside)
        return button
    }

    static func makeBackButton(target: Any?, action: Selector) -> UIButton {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.tintColor = .white
        button.contentEdgeInsets = UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        button.setImage(.lx_control(named: "icon_back"), for: .normal)
        button.imageView?.contentMode = .scaleAspectFit
        button.addTarget(target, action: action, for: .touchUpInside)
        return button
    }
}

private final class PhotoReviewViewController: UIViewController {
    private let imageURL: URL
    private let onRetake: () -> Void
    private let onConfirm: () -> Void

    private let imageView = UIImageView()

    init(imageURL: URL, onRetake: @escaping () -> Void, onConfirm: @escaping () -> Void) {
        self.imageURL = imageURL
        self.onRetake = onRetake
        self.onConfirm = onConfirm
        super.init(nibName: nil, bundle: nil)
        modalPresentationCapturesStatusBarAppearance = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var prefersStatusBarHidden: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configureImageView()
        configureControls()
        loadImage()
    }

    private func configureImageView() {
        imageView.translatesAutoresizingMaskIntoConstraints = false
        imageView.contentMode = .scaleAspectFill
        imageView.clipsToBounds = true
        view.addSubview(imageView)

        NSLayoutConstraint.activate([
            imageView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            imageView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            imageView.topAnchor.constraint(equalTo: view.topAnchor),
            imageView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
    }

    private func configureControls() {
        let confirmButton = ReviewButtonFactory.makeConfirmButton(
            title: "确定",
            target: self,
            action: #selector(confirmTapped)
        )

        let retakeActionButton = ReviewButtonFactory.makeRetakeActionButton(
            target: self,
            action: #selector(retakeTapped)
        )

        let backButton = ReviewButtonFactory.makeBackButton(
            target: self,
            action: #selector(retakeTapped)
        )

        let buttonStack = UIStackView(arrangedSubviews: [retakeActionButton, confirmButton])
        buttonStack.translatesAutoresizingMaskIntoConstraints = false
        buttonStack.axis = .horizontal
        buttonStack.alignment = .center
        buttonStack.spacing = 55

        view.addSubview(buttonStack)
        view.addSubview(backButton)

        NSLayoutConstraint.activate([
            confirmButton.widthAnchor.constraint(equalToConstant: 120),
            confirmButton.heightAnchor.constraint(equalToConstant: 40),

            retakeActionButton.widthAnchor.constraint(equalToConstant: 120),
            retakeActionButton.heightAnchor.constraint(equalToConstant: 40),

            buttonStack.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            buttonStack.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -48),

            backButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            backButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16)
        ])
    }

    private func loadImage() {
        if let image = UIImage(contentsOfFile: imageURL.path) {
            imageView.image = image
            return
        }

        if let data = try? Data(contentsOf: imageURL), let image = UIImage(data: data) {
            imageView.image = image
            return
        }

        // 如果加载失败，直接视为重拍
        retakeTapped()
    }

    @objc private func confirmTapped() {
        dismiss(animated: false) { [weak self] in
            self?.onConfirm()
        }
    }

    @objc private func retakeTapped() {
        dismiss(animated: true) { [weak self] in
            self?.onRetake()
        }
    }
}

@MainActor
private final class PhotoCaptureOverlayView: UIView {
    var onCaptureTapped: (() -> Void)?
    var onCancelTapped: (() -> Void)?
    var onSwitchCameraTapped: (() -> Void)?
    var onFlashToggle: ((Bool) -> Void)?

    private let captureButton = UIButton(type: .custom)
    private let captureInnerLayer = CAShapeLayer()
    private let hintLabel = UILabel()
    private let cancelButton = UIButton(type: .system)
    private let switchCameraButton = UIButton(type: .system)
    private let flashButton = UIButton(type: .system)
    private let activityIndicator = UIActivityIndicatorView(style: .large)
    private var flashEnabled = false
    private var flashAvailable = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        configureView()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setBusy(_ busy: Bool) {
        captureButton.isEnabled = !busy
        switchCameraButton.isEnabled = !busy
        flashButton.isEnabled = flashAvailable && !busy
        busy ? activityIndicator.startAnimating() : activityIndicator.stopAnimating()
        activityIndicator.isHidden = !busy
        captureButton.alpha = busy ? 0.6 : 1.0
        if flashAvailable {
            flashButton.alpha = flashButton.isEnabled ? 1.0 : 0.5
        }
    }

    func showHint(_ text: String) {
        hintLabel.text = text
    }

    func setSwitchAvailable(_ available: Bool) {
        switchCameraButton.isHidden = !available
    }

    func updateCameraPosition(isFront: Bool) {
        switchCameraButton.accessibilityLabel = isFront ? "切换到后置摄像头" : "切换到前置摄像头"
    }

    func setFlashAvailable(_ available: Bool) {
        flashAvailable = available
        flashButton.isHidden = !available
        flashButton.isEnabled = available
        flashButton.alpha = available ? 1.0 : 0.0
        if !available {
            setFlashEnabled(false)
        }
    }

    func setFlashEnabled(_ enabled: Bool) {
        flashEnabled = enabled
        updateFlashButtonImage()
    }

    private func configureView() {
        backgroundColor = .clear

        captureButton.translatesAutoresizingMaskIntoConstraints = false
        captureButton.backgroundColor = .clear
        captureButton.layer.cornerRadius = 42
        captureButton.layer.borderWidth = 4
        captureButton.layer.borderColor = UIColor.white.withAlphaComponent(0.9).cgColor
        captureButton.adjustsImageWhenHighlighted = false
        captureInnerLayer.fillColor = UIColor.white.cgColor
        captureButton.layer.addSublayer(captureInnerLayer)
        captureButton.addTarget(self, action: #selector(captureTapped), for: .touchUpInside)
        captureButton.addTarget(self, action: #selector(captureTouchDown), for: .touchDown)
        captureButton.addTarget(self, action: #selector(captureTouchUp), for: [.touchDragExit, .touchCancel, .touchUpInside, .touchUpOutside])
        addSubview(captureButton)

        hintLabel.translatesAutoresizingMaskIntoConstraints = false
        hintLabel.textAlignment = .center
        hintLabel.textColor = .white
        hintLabel.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        addSubview(hintLabel)

        cancelButton.translatesAutoresizingMaskIntoConstraints = false
        cancelButton.backgroundColor = .clear
        cancelButton.layer.cornerRadius = 0
        cancelButton.setImage(.lx_control(named: "icon_close"), for: .normal)
        cancelButton.tintColor = .white
        cancelButton.contentEdgeInsets = .zero
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        addSubview(cancelButton)

        switchCameraButton.translatesAutoresizingMaskIntoConstraints = false
        switchCameraButton.backgroundColor = .clear
        switchCameraButton.layer.cornerRadius = 0
        switchCameraButton.setImage(.lx_control(named: "icon_switch"), for: .normal)
        switchCameraButton.tintColor = .white
        switchCameraButton.contentEdgeInsets = .zero
        switchCameraButton.addTarget(self, action: #selector(switchCameraTapped), for: .touchUpInside)
        addSubview(switchCameraButton)

        flashButton.translatesAutoresizingMaskIntoConstraints = false
        flashButton.backgroundColor = .clear
        flashButton.layer.cornerRadius = 0
        flashButton.tintColor = .white
        flashButton.contentEdgeInsets = .zero
        flashButton.addTarget(self, action: #selector(flashTapped), for: .touchUpInside)
        addSubview(flashButton)
        flashButton.isHidden = true
        flashButton.isEnabled = false
        flashButton.alpha = 0.0
        updateFlashButtonImage()

        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.color = .white
        activityIndicator.hidesWhenStopped = true
        addSubview(activityIndicator)

        NSLayoutConstraint.activate([
            captureButton.centerXAnchor.constraint(equalTo: centerXAnchor),
            captureButton.bottomAnchor.constraint(equalTo: safeAreaLayoutGuide.bottomAnchor, constant: -32),
            captureButton.widthAnchor.constraint(equalToConstant: 84),
            captureButton.heightAnchor.constraint(equalToConstant: 84),

            hintLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            hintLabel.bottomAnchor.constraint(equalTo: captureButton.topAnchor, constant: -18),

            cancelButton.leadingAnchor.constraint(equalTo: safeAreaLayoutGuide.leadingAnchor, constant: 16),
            cancelButton.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 16),
            cancelButton.widthAnchor.constraint(equalToConstant: 44),
            cancelButton.heightAnchor.constraint(equalToConstant: 44),

            flashButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            flashButton.trailingAnchor.constraint(equalTo: captureButton.leadingAnchor, constant: -32),
            flashButton.widthAnchor.constraint(equalToConstant: 44),
            flashButton.heightAnchor.constraint(equalToConstant: 44),

            switchCameraButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            switchCameraButton.leadingAnchor.constraint(equalTo: captureButton.trailingAnchor, constant: 32),
            switchCameraButton.widthAnchor.constraint(equalToConstant: 44),
            switchCameraButton.heightAnchor.constraint(equalToConstant: 44),

            activityIndicator.centerXAnchor.constraint(equalTo: centerXAnchor),
            activityIndicator.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        updateCaptureLayers()
    }

    private func updateCaptureLayers() {
        let bounds = captureButton.bounds
        captureInnerLayer.frame = bounds
        captureInnerLayer.path = UIBezierPath(ovalIn: bounds.insetBy(dx: 10, dy: 10)).cgPath
    }

    private func updateFlashButtonImage() {
        let name = flashEnabled ? "icon_flash_on" : "icon_flash_off"
        flashButton.setImage(.lx_control(named: name), for: .normal)
        flashButton.accessibilityLabel = flashEnabled ? "关闭闪光灯" : "开启闪光灯"
    }

    @objc private func captureTapped() {
        animateCapturePulse()
        onCaptureTapped?()
    }

    @objc private func captureTouchDown() {
        animateInnerCircle(to: 0.9)
    }

    @objc private func captureTouchUp() {
        animateInnerCircle(to: 1.0)
    }

    private func animateCapturePulse() {
        animateInnerCircle(to: 0.85) { [weak self] in
            self?.animateInnerCircle(to: 1.0)
        }
    }

    private func animateInnerCircle(to scale: CGFloat, completion: (() -> Void)? = nil) {
        CATransaction.begin()
        CATransaction.setAnimationDuration(0.12)
        CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeInEaseOut))
        CATransaction.setCompletionBlock(completion)
        CATransaction.setDisableActions(false)
        let transform = CATransform3DMakeScale(scale, scale, 1)
        captureInnerLayer.transform = transform
        CATransaction.commit()
    }

    @objc private func cancelTapped() {
        onCancelTapped?()
    }

    @objc private func switchCameraTapped() {
        onSwitchCameraTapped?()
    }

    @objc private func flashTapped() {
        guard flashAvailable else { return }
        setFlashEnabled(!flashEnabled)
        onFlashToggle?(flashEnabled)
    }
}

private enum VideoCaptureResult {
    case success(URL)
    case cancelled
    case failure(String)
}

private enum PendingRecordingAction: Equatable {
    case none
    case stop
    case cancel
}

private enum RecordingState: Equatable {
    case idle
    case preparing(pending: PendingRecordingAction)
    case recording
    case finishing
    case cancelling
}

private final class VideoCaptureViewController: UIViewController {
    private let resultHandler: (VideoCaptureResult) -> Void
    private let maxDuration: TimeInterval
    private var currentPosition: AVCaptureDevice.Position

    private let session = AVCaptureSession()
    private let sessionQueue = DispatchQueue(label: "com.lingxia.camera.session")
    private let movieOutput = AVCaptureMovieFileOutput()
    private var videoInput: AVCaptureDeviceInput?
    private var audioInput: AVCaptureDeviceInput?
    private var previewLayer: AVCaptureVideoPreviewLayer?

    private let overlayView = VideoCaptureOverlayView()

    private var isSessionRunning = false
    private var recordingState: RecordingState = .idle {
        didSet {
            handleRecordingStateChange(from: oldValue, to: recordingState)
        }
    }
    private var recordingStartDate: Date?
    private var updateTimer: Timer?
    private var pendingReviewURL: URL?
    private var torchEnabled = false

    init(initialCameraPosition: AVCaptureDevice.Position, maxDuration: TimeInterval?, resultHandler: @escaping (VideoCaptureResult) -> Void) {
        self.currentPosition = initialCameraPosition
        let fallback: TimeInterval = 15
        self.maxDuration = max(maxDuration ?? fallback, 1)
        self.resultHandler = resultHandler
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .fullScreen
        modalPresentationCapturesStatusBarAppearance = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var prefersStatusBarHidden: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configurePreviewLayer()
        configureOverlay()
        configureSession()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        startSessionIfNeeded()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        stopSession()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    deinit {
        stopSession()
    }

    @MainActor
    private func handleRecordingStateChange(from oldValue: RecordingState, to newValue: RecordingState) {
        guard oldValue != newValue else { return }

        switch newValue {
        case .idle:
            overlayView.updateToIdle()
        case .preparing(let pending):
            switch pending {
            case .stop:
                overlayView.updateToRecording()
            case .cancel:
                overlayView.updateToCancelling()
            case .none:
                overlayView.updateToIdle()
            }
        case .recording:
            overlayView.updateToRecording()
        case .finishing:
            overlayView.updateToRecording()
        case .cancelling:
            overlayView.updateToCancelling()
        }
    }

    private func configurePreviewLayer() {
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.addSublayer(layer)
        previewLayer = layer
    }

    private func configureOverlay() {
        overlayView.translatesAutoresizingMaskIntoConstraints = false
        overlayView.setInitialTimer(maxDuration: maxDuration)
        overlayView.updateCameraPosition(isFront: currentPosition == .front)
        overlayView.onRecordTapped = { [weak self] in
            self?.handleRecordTapped()
        }
        overlayView.onCancelTapped = { [weak self] in
            self?.handleCancelTapped()
        }
        overlayView.onSwitchCameraTapped = { [weak self] in
            self?.switchCamera()
        }
        overlayView.onFlashToggle = { [weak self] isOn in
            self?.setTorchEnabled(isOn)
        }
        overlayView.setFlashAvailable(false)
        overlayView.setFlashEnabled(torchEnabled)
        view.addSubview(overlayView)
        NSLayoutConstraint.activate([
            overlayView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlayView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlayView.topAnchor.constraint(equalTo: view.topAnchor),
            overlayView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        overlayView.updateToIdle()
    }

    private func configureSession() {
        sessionQueue.async {
            do {
                try self.configureAudioSession()
            } catch {
                self.finish(with: .failure("音频会话初始化失败: \(error.localizedDescription)"))
                return
            }

            self.session.beginConfiguration()
            self.session.sessionPreset = .high

            guard let videoDevice = self.bestVideoDevice(position: self.currentPosition) else {
                self.finish(with: .failure("无法访问摄像头"))
                self.session.commitConfiguration()
                return
            }

            do {
                let videoInput = try AVCaptureDeviceInput(device: videoDevice)
                if self.session.canAddInput(videoInput) {
                    self.session.addInput(videoInput)
                    self.videoInput = videoInput
        } else {
                    self.session.commitConfiguration()
                    self.finish(with: .failure("无法添加视频输入"))
                return
            }
            } catch {
                self.session.commitConfiguration()
                self.finish(with: .failure("摄像头初始化失败: \(error.localizedDescription)"))
                return
            }

            if let audioDevice = AVCaptureDevice.default(for: .audio) {
                do {
                    let audioInput = try AVCaptureDeviceInput(device: audioDevice)
                    if self.session.canAddInput(audioInput) {
                        self.session.addInput(audioInput)
                        self.audioInput = audioInput
                    }
                } catch {
                    // ignore audio input failure; we will proceed without it
                }
            }

            if self.session.canAddOutput(self.movieOutput) {
                self.session.addOutput(self.movieOutput)
                self.movieOutput.maxRecordedDuration = CMTime(seconds: self.maxDuration, preferredTimescale: 1)
            } else {
                self.session.commitConfiguration()
                self.finish(with: .failure("无法添加视频输出"))
                return
            }

            self.session.commitConfiguration()
            self.updateTorchAvailability(for: videoDevice)
            self.startSessionIfNeeded()
        }
    }

    private func configureAudioSession() throws {
        let audioSession = AVAudioSession.sharedInstance()
        try audioSession.setCategory(.playAndRecord, mode: .videoRecording, options: [.defaultToSpeaker, .allowBluetooth])
        try audioSession.setActive(true, options: .notifyOthersOnDeactivation)
    }

    private func startSessionIfNeeded() {
        sessionQueue.async {
            guard !self.isSessionRunning else { return }
            self.session.startRunning()
            self.isSessionRunning = true
        }
    }

    private func stopSession() {
        sessionQueue.async {
            guard self.isSessionRunning else { return }
            self.session.stopRunning()
            self.isSessionRunning = false
        }
    }

    private func setTorchEnabled(_ enabled: Bool) {
        guard torchEnabled != enabled else {
            DispatchQueue.main.async {
                self.overlayView.setFlashEnabled(enabled)
            }
            return
        }
        torchEnabled = enabled
        DispatchQueue.main.async {
            self.overlayView.setFlashEnabled(enabled)
        }
        sessionQueue.async { [weak self] in
            guard let self else { return }
            guard let device = self.videoInput?.device, device.hasTorch else { return }
            do {
                try device.lockForConfiguration()
                device.torchMode = enabled ? .on : .off
                device.unlockForConfiguration()
            } catch {
                self.torchEnabled = false
                DispatchQueue.main.async {
                    self.overlayView.setFlashEnabled(false)
                }
            }
        }
    }

    private func updateTorchAvailability(for device: AVCaptureDevice) {
        let available = device.hasTorch && device.isTorchModeSupported(.on)
        if !available && torchEnabled {
            setTorchEnabled(false)
        }
        DispatchQueue.main.async {
            self.overlayView.setFlashAvailable(available)
            if available {
                self.overlayView.setFlashEnabled(self.torchEnabled)
            }
        }
    }

    private func handleRecordTapped() {
        switch recordingState {
        case .idle:
            beginRecording()
        case .recording, .finishing:
            requestStopRecording()
        case .preparing:
            updatePendingActionForPreparing(.stop)
        case .cancelling:
            break
        }
    }

    private func handleCancelTapped() {
        requestCancelRecording()
    }

    private func beginRecording() {
        guard recordingState == .idle else { return }
        recordingState = .preparing(pending: .none)

        sessionQueue.async {
            if self.movieOutput.isRecording {
                DispatchQueue.main.async {
                    self.recordingState = .recording
                }
                return
            }
            if let connection = self.movieOutput.connection(with: .video) {
                connection.videoOrientation = self.currentVideoOrientation()
            }

            let outputURL = self.makeTemporaryFileURL()
            self.movieOutput.startRecording(to: outputURL, recordingDelegate: self)
        }
    }

    private func requestStopRecording() {
        switch recordingState {
        case .recording:
            CaptureFeedback.playRecordStop()
            recordingState = .finishing
            stopMovieOutput()
        case .preparing:
            CaptureFeedback.playRecordStop()
            recordingState = .preparing(pending: .stop)
        default:
            break
        }
    }

    private func requestCancelRecording() {
        switch recordingState {
        case .recording, .finishing:
            CaptureFeedback.playRecordStop()
            recordingState = .cancelling
            stopMovieOutput()
        case .preparing:
            CaptureFeedback.playRecordStop()
            recordingState = .preparing(pending: .cancel)
        case .idle:
            finish(with: .cancelled)
        case .cancelling:
            break
        }
    }

    private func updatePendingActionForPreparing(_ action: PendingRecordingAction) {
        guard case .preparing = recordingState else { return }
        recordingState = .preparing(pending: action)
    }

    private func stopMovieOutput() {
        sessionQueue.async {
            guard self.movieOutput.isRecording else { return }
            self.movieOutput.stopRecording()
        }
    }

    private var isRecordingActive: Bool {
        switch recordingState {
        case .recording, .finishing, .cancelling:
            return true
        default:
            return false
        }
    }

    private func switchCamera() {
        guard recordingState == .idle else { return }
        sessionQueue.async {
            self.session.beginConfiguration()
            if let currentInput = self.videoInput {
                self.session.removeInput(currentInput)
            }

            self.currentPosition = self.currentPosition == .front ? .back : .front

            guard let newDevice = self.bestVideoDevice(position: self.currentPosition) else {
                self.session.commitConfiguration()
                DispatchQueue.main.async {
                    self.overlayView.showHint("无法切换摄像头")
                }
                return
            }

            do {
                let newInput = try AVCaptureDeviceInput(device: newDevice)
                if self.session.canAddInput(newInput) {
                    self.session.addInput(newInput)
                    self.videoInput = newInput
                } else {
                    self.session.commitConfiguration()
                    DispatchQueue.main.async {
                        self.overlayView.showHint("无法切换摄像头")
                    }
                    return
                }
            } catch {
                self.session.commitConfiguration()
                DispatchQueue.main.async {
                    self.overlayView.showHint("摄像头切换失败")
                }
                return
            }

            self.session.commitConfiguration()
            self.updateTorchAvailability(for: newDevice)

            DispatchQueue.main.async {
                self.overlayView.updateCameraPosition(isFront: self.currentPosition == .front)
            }
        }
    }

    private func makeTemporaryFileURL() -> URL {
        let tempDir = FileManager.default.temporaryDirectory
        return tempDir.appendingPathComponent("lx_video_\(UUID().uuidString).mov")
    }

    private func startUpdateTimer() {
        updateTimer?.invalidate()
        updateTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            guard let self = self, self.isRecordingActive, let start = self.recordingStartDate else { return }
            let elapsed = Date().timeIntervalSince(start)
            DispatchQueue.main.async {
                self.overlayView.updateRecordingProgress(elapsed: elapsed, maxDuration: self.maxDuration)
            }
            if elapsed >= self.maxDuration {
                self.requestStopRecording()
            }
        }
        if let updateTimer {
            RunLoop.main.add(updateTimer, forMode: .common)
        }
    }

    private func stopUpdateTimer() {
        updateTimer?.invalidate()
        updateTimer = nil
    }

    private func currentVideoOrientation() -> AVCaptureVideoOrientation {
        switch UIApplication.shared.windows.first?.windowScene?.interfaceOrientation {
        case .landscapeLeft:
            return .landscapeRight
        case .landscapeRight:
            return .landscapeLeft
        case .portraitUpsideDown:
            return .portraitUpsideDown
        default:
            return .portrait
        }
    }

    private func presentReview(for url: URL) {
        pendingReviewURL = url
        overlayView.isHidden = true

        let reviewController = VideoReviewViewController(
            videoURL: url,
            onRetake: { [weak self] in
                self?.handleRetakeFromReview()
            },
            onConfirm: { [weak self] in
                self?.handleConfirmFromReview()
            }
        )
        reviewController.modalPresentationStyle = .fullScreen
        present(reviewController, animated: true)
    }

    private func handleRetakeFromReview() {
        guard let url = pendingReviewURL else { return }
        pendingReviewURL = nil
        if FileManager.default.fileExists(atPath: url.path) {
            try? FileManager.default.removeItem(at: url)
        }
        overlayView.isHidden = false
        recordingState = .idle
        startSessionIfNeeded()
    }

    private func handleConfirmFromReview() {
        guard let url = pendingReviewURL else { return }
        pendingReviewURL = nil
        finish(with: .success(url))
    }

    nonisolated private func bestVideoDevice(position: AVCaptureDevice.Position) -> AVCaptureDevice? {
        if let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: position) {
            return device
        }
        if let device = AVCaptureDevice.default(.builtInDualCamera, for: .video, position: position) {
            return device
        }
        return AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back)
    }

    private func finish(with result: VideoCaptureResult) {
        stopUpdateTimer()
        setTorchEnabled(false)
        stopSession()
        pendingReviewURL = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        DispatchQueue.main.async {
            self.recordingState = .idle
            self.dismiss(animated: true) {
                self.resultHandler(result)
            }
        }
    }
}

extension VideoCaptureViewController: @preconcurrency AVCaptureFileOutputRecordingDelegate {
    @MainActor
    func fileOutput(_ output: AVCaptureFileOutput, didStartRecordingTo fileURL: URL, from connections: [AVCaptureConnection]) {
        let pendingAction: PendingRecordingAction
        if case .preparing(let action) = recordingState {
            pendingAction = action
        } else {
            pendingAction = .none
        }

        recordingStartDate = Date()
        startUpdateTimer()

        switch pendingAction {
        case .none:
            recordingState = .recording
        case .stop:
            recordingState = .finishing
            stopMovieOutput()
        case .cancel:
            recordingState = .cancelling
            stopMovieOutput()
        }
    }

    @MainActor
    func fileOutput(_ output: AVCaptureFileOutput, didFinishRecordingTo outputFileURL: URL, from connections: [AVCaptureConnection], error: Error?) {
        let wasCancelling: Bool
        switch recordingState {
        case .cancelling:
            wasCancelling = true
        case .preparing(let action):
            wasCancelling = action == .cancel
        default:
            wasCancelling = false
        }

        recordingStartDate = nil
        stopUpdateTimer()
        recordingState = .idle
        if let error {
            let nsError = error as NSError
            if nsError.domain == AVFoundationErrorDomain,
               nsError.code == AVError.Code.maximumDurationReached.rawValue {
                self.overlayView.showMaxDurationReached()
                self.overlayView.updateToRecording()
                self.finish(with: .success(outputFileURL))
                return
            }

            if FileManager.default.fileExists(atPath: outputFileURL.path) {
                try? FileManager.default.removeItem(at: outputFileURL)
            }
            self.overlayView.showHint("录制失败: \(error.localizedDescription)")
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                self.overlayView.updateToIdle()
                self.finish(with: .failure("录制失败"))
            }
            return
        }

        if wasCancelling {
            if FileManager.default.fileExists(atPath: outputFileURL.path) {
                try? FileManager.default.removeItem(at: outputFileURL)
            }
            self.finish(with: .cancelled)
            return
        }
        self.presentReview(for: outputFileURL)
    }
}

private final class VideoReviewViewController: UIViewController {
    private let videoURL: URL
    private let onRetake: () -> Void
    private let onConfirm: () -> Void
    private var player: AVPlayer?
    private var playerLayer: AVPlayerLayer?

    init(videoURL: URL, onRetake: @escaping () -> Void, onConfirm: @escaping () -> Void) {
        self.videoURL = videoURL
        self.onRetake = onRetake
        self.onConfirm = onConfirm
        super.init(nibName: nil, bundle: nil)
        modalPresentationCapturesStatusBarAppearance = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var prefersStatusBarHidden: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configurePlayer()
        configureControls()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        playerLayer?.frame = view.bounds
    }

    nonisolated deinit {
        NotificationCenter.default.removeObserver(self)
    }

    private func configurePlayer() {
        let player = AVPlayer(url: videoURL)
        self.player = player
        let layer = AVPlayerLayer(player: player)
        layer.videoGravity = .resizeAspect
        view.layer.addSublayer(layer)
        playerLayer = layer

        NotificationCenter.default.addObserver(self, selector: #selector(loopPlayback), name: .AVPlayerItemDidPlayToEndTime, object: player.currentItem)
        player.play()
    }

    private func configureControls() {
        let confirmButton = ReviewButtonFactory.makeConfirmButton(
            title: "确定",
            target: self,
            action: #selector(confirmTapped)
        )

        let retakeActionButton = ReviewButtonFactory.makeRetakeActionButton(
            target: self,
            action: #selector(retakeTapped)
        )

        let backButton = ReviewButtonFactory.makeBackButton(
            target: self,
            action: #selector(retakeTapped)
        )

        let buttonStack = UIStackView(arrangedSubviews: [retakeActionButton, confirmButton])
        buttonStack.translatesAutoresizingMaskIntoConstraints = false
        buttonStack.axis = .horizontal
        buttonStack.alignment = .center
        buttonStack.spacing = 55

        view.addSubview(buttonStack)
        view.addSubview(backButton)

        NSLayoutConstraint.activate([
            confirmButton.widthAnchor.constraint(equalToConstant: 120),
            confirmButton.heightAnchor.constraint(equalToConstant: 40),

            retakeActionButton.widthAnchor.constraint(equalToConstant: 120),
            retakeActionButton.heightAnchor.constraint(equalToConstant: 40),

            buttonStack.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            buttonStack.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -48),

            backButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            backButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16)
        ])
    }

    @objc private func loopPlayback() {
        player?.seek(to: .zero)
        player?.play()
    }

    @objc private func confirmTapped() {
        player?.pause()
        dismiss(animated: false) { [weak self] in
            self?.onConfirm()
        }
    }

    @objc private func retakeTapped() {
        player?.pause()
        dismiss(animated: true) { [weak self] in
            self?.onRetake()
        }
    }
}

private final class VideoCaptureOverlayView: UIView {
    var onRecordTapped: (() -> Void)?
    var onCancelTapped: (() -> Void)?
    var onSwitchCameraTapped: (() -> Void)?
    var onFlashToggle: ((Bool) -> Void)?

    private var maxDuration: TimeInterval = 15
    private var recordingActive = false
    private var cameraSwitchAvailable = false
    private var flashEnabled = false
    private var flashAvailable = false

    private let captureButton = UIButton(type: .custom)
    private let innerCircle = UIView()
    private let flashButton = UIButton(type: .system)
    private let hintLabel = UILabel()
    private let timerLabel = UILabel()
    private let cancelButton = UIButton(type: .system)
    private let switchCameraButton = UIButton(type: .system)
    private let cancelBaseColor = UIColor.clear
    private let flashButtonSize: CGFloat = 44

    override init(frame: CGRect) {
        super.init(frame: frame)
        configureView()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    func setInitialTimer(maxDuration: TimeInterval) {
        self.maxDuration = maxDuration
        timerLabel.text = Self.format(time: maxDuration, rounding: .up)
        timerLabel.isHidden = true
        applyCameraButtonState()
    }

    func updateToIdle() {
        recordingActive = false
        hintLabel.text = "点击录制"
        timerLabel.isHidden = true
        applyCameraButtonState()
        UIView.animate(withDuration: 0.2) {
            self.cancelButton.alpha = 1.0
        }
        animateCaptureShape(isRecording: false)
        cancelButton.backgroundColor = cancelBaseColor
    }

    func updateToIdle(after delay: TimeInterval) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            self?.updateToIdle()
        }
    }

    func updateToRecording() {
        recordingActive = true
        hintLabel.text = "点击停止"
        timerLabel.isHidden = false
        cancelButton.backgroundColor = cancelBaseColor
        applyCameraButtonState()
        animateCaptureShape(isRecording: true)
    }

    func updateToCancelling() {
        hintLabel.text = "取消中..."
        timerLabel.isHidden = true
        applyCameraButtonState()
        cancelButton.alpha = 1.0
        cancelButton.backgroundColor = cancelBaseColor
    }

    func showHint(_ text: String) {
        hintLabel.text = text
    }

    func showMaxDurationReached() {
        hintLabel.text = "已达最长录制时间"
        timerLabel.isHidden = false
    }

    func updateRecordingProgress(elapsed: TimeInterval, maxDuration: TimeInterval) {
        self.maxDuration = maxDuration
        let remaining = max(maxDuration - elapsed, 0)
        timerLabel.text = Self.format(time: remaining, rounding: .up)
        timerLabel.isHidden = !recordingActive
    }

    func updateCameraPosition(isFront: Bool) {
        let discovery = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInWideAngleCamera, .builtInDualCamera, .builtInTripleCamera, .builtInDualWideCamera],
            mediaType: .video,
            position: .unspecified
        )
        let positions = Set(discovery.devices.map { $0.position })
        cameraSwitchAvailable = positions.contains(.front) && positions.contains(.back)
        applyCameraButtonState()
        switchCameraButton.accessibilityLabel = isFront ? "切换到后置摄像头" : "切换到前置摄像头"
    }

    private func configureView() {
        backgroundColor = .clear

        captureButton.translatesAutoresizingMaskIntoConstraints = false
        captureButton.backgroundColor = .clear
        captureButton.layer.cornerRadius = 42
        captureButton.layer.borderColor = UIColor.white.withAlphaComponent(0.9).cgColor
        captureButton.layer.borderWidth = 4
        captureButton.adjustsImageWhenHighlighted = false
        captureButton.addTarget(self, action: #selector(recordTapped), for: .touchUpInside)
        captureButton.accessibilityLabel = "录制视频"
        captureButton.accessibilityTraits.insert(.button)
        addSubview(captureButton)

        innerCircle.translatesAutoresizingMaskIntoConstraints = false
        innerCircle.backgroundColor = UIColor(red: 0.95, green: 0.12, blue: 0.12, alpha: 1)
        innerCircle.layer.cornerRadius = 30
        innerCircle.layer.masksToBounds = true
        innerCircle.isUserInteractionEnabled = false
        captureButton.addSubview(innerCircle)

        timerLabel.translatesAutoresizingMaskIntoConstraints = false
        timerLabel.font = UIFont.monospacedDigitSystemFont(ofSize: 22, weight: .medium)
        timerLabel.textAlignment = .center
        timerLabel.textColor = .white
        timerLabel.text = "00:00"
        timerLabel.isHidden = true
        addSubview(timerLabel)

        hintLabel.translatesAutoresizingMaskIntoConstraints = false
        hintLabel.textAlignment = .center
        hintLabel.textColor = .white
        hintLabel.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        addSubview(hintLabel)

        cancelButton.translatesAutoresizingMaskIntoConstraints = false
        cancelButton.backgroundColor = cancelBaseColor
        cancelButton.layer.cornerRadius = 0
        cancelButton.tintColor = .white
        cancelButton.setImage(.lx_control(named: "icon_close"), for: .normal)
        cancelButton.accessibilityLabel = "关闭"
        cancelButton.contentEdgeInsets = .zero
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        addSubview(cancelButton)

        flashButton.translatesAutoresizingMaskIntoConstraints = false
        flashButton.backgroundColor = .clear
        flashButton.layer.cornerRadius = 0
        flashButton.tintColor = .white
        flashButton.contentEdgeInsets = .zero
        flashButton.addTarget(self, action: #selector(flashTapped), for: .touchUpInside)
        addSubview(flashButton)
        flashButton.isHidden = true
        flashButton.isEnabled = false
        flashButton.alpha = 0.0
        updateFlashButtonImage()

        switchCameraButton.translatesAutoresizingMaskIntoConstraints = false
        switchCameraButton.backgroundColor = .clear
        switchCameraButton.layer.cornerRadius = 0
        switchCameraButton.setImage(.lx_control(named: "icon_switch"), for: .normal)
        switchCameraButton.tintColor = .white
        switchCameraButton.contentEdgeInsets = .zero
        switchCameraButton.alpha = 0.0
        switchCameraButton.addTarget(self, action: #selector(switchCameraTapped), for: .touchUpInside)
        addSubview(switchCameraButton)

        NSLayoutConstraint.activate([
            captureButton.centerXAnchor.constraint(equalTo: centerXAnchor),
            captureButton.bottomAnchor.constraint(equalTo: safeAreaLayoutGuide.bottomAnchor, constant: -28),
            captureButton.widthAnchor.constraint(equalToConstant: 84),
            captureButton.heightAnchor.constraint(equalToConstant: 84),

            innerCircle.centerXAnchor.constraint(equalTo: captureButton.centerXAnchor),
            innerCircle.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            innerCircle.widthAnchor.constraint(equalToConstant: 60),
            innerCircle.heightAnchor.constraint(equalToConstant: 60),

            hintLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            hintLabel.bottomAnchor.constraint(equalTo: captureButton.topAnchor, constant: -18),

            timerLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            timerLabel.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 18),

            cancelButton.leadingAnchor.constraint(equalTo: safeAreaLayoutGuide.leadingAnchor, constant: 16),
            cancelButton.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 16),
            cancelButton.widthAnchor.constraint(equalToConstant: 44),
            cancelButton.heightAnchor.constraint(equalToConstant: 44),

            flashButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            flashButton.trailingAnchor.constraint(equalTo: captureButton.leadingAnchor, constant: -32),
            flashButton.widthAnchor.constraint(equalToConstant: flashButtonSize),
            flashButton.heightAnchor.constraint(equalToConstant: flashButtonSize),

            switchCameraButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            switchCameraButton.leadingAnchor.constraint(equalTo: captureButton.trailingAnchor, constant: 32),
            switchCameraButton.widthAnchor.constraint(equalToConstant: 44),
            switchCameraButton.heightAnchor.constraint(equalToConstant: 44)
        ])

        updateToIdle()
    }

    private static func format(time: TimeInterval, rounding rule: FloatingPointRoundingRule) -> String {
        let totalSeconds = max(Int(time.rounded(rule)), 0)
        let minutes = totalSeconds / 60
        let seconds = totalSeconds % 60
        return String(format: "%02d:%02d", minutes, seconds)
    }

    private func applyCameraButtonState() {
        if !cameraSwitchAvailable {
            switchCameraButton.isEnabled = false
            switchCameraButton.alpha = 0.0
        } else {
            switchCameraButton.isEnabled = !recordingActive
            switchCameraButton.alpha = recordingActive ? 0.4 : 1.0
        }
        flashButton.isHidden = !flashAvailable
        flashButton.isEnabled = flashAvailable && !recordingActive
        if flashAvailable {
            flashButton.alpha = flashButton.isEnabled ? 1.0 : 0.5
        } else {
            flashButton.alpha = 0.0
        }
    }

    func setFlashAvailable(_ available: Bool) {
        flashAvailable = available
        if !available {
            setFlashEnabled(false)
        }
        applyCameraButtonState()
    }

    func setFlashEnabled(_ enabled: Bool) {
        flashEnabled = enabled
        updateFlashButtonImage()
    }

    private func updateFlashButtonImage() {
        let name = flashEnabled ? "icon_flash_on" : "icon_flash_off"
        flashButton.setImage(.lx_control(named: name), for: .normal)
        flashButton.accessibilityLabel = flashEnabled ? "关闭闪光灯" : "开启闪光灯"
    }

    private func animateCaptureShape(isRecording: Bool) {
        let targetScale: CGFloat = isRecording ? 0.65 : 1.0
        let targetCornerRadius: CGFloat = isRecording ? 8 : 30

        let cornerAnimation = CABasicAnimation(keyPath: "cornerRadius")
        cornerAnimation.fromValue = innerCircle.layer.cornerRadius
        cornerAnimation.toValue = targetCornerRadius
        cornerAnimation.duration = 0.2
        cornerAnimation.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        innerCircle.layer.add(cornerAnimation, forKey: "cornerRadius")
        innerCircle.layer.cornerRadius = targetCornerRadius

        UIView.animate(withDuration: 0.2, delay: 0, options: [.curveEaseInOut, .allowUserInteraction]) {
            self.innerCircle.transform = CGAffineTransform(scaleX: targetScale, y: targetScale)
        }
    }

    @objc private func recordTapped() {
        if !recordingActive {
            CaptureFeedback.playRecordStart()
        }
        onRecordTapped?()
    }

    @objc private func cancelTapped() {
        onCancelTapped?()
    }

    @objc private func switchCameraTapped() {
        onSwitchCameraTapped?()
    }

    @objc private func flashTapped() {
        guard flashAvailable else { return }
        setFlashEnabled(!flashEnabled)
        onFlashToggle?(flashEnabled)
    }
}

private func copyMediaFileToTemp(
    from sourceURL: URL,
    prefix: String,
    fallbackExtension: String,
    requiresSecurityScope: Bool
) -> URL? {
    let fileManager = FileManager.default
    let tempDir = fileManager.temporaryDirectory

    let sanitizedFallback = fallbackExtension.isEmpty ? "tmp" : fallbackExtension
    let ext = sourceURL.pathExtension.isEmpty ? sanitizedFallback : sourceURL.pathExtension
    let filename = "\(prefix)_\(UUID().uuidString)" + (ext.isEmpty ? "" : ".\(ext)")
    let destinationURL = tempDir.appendingPathComponent(filename)

    let accessed = requiresSecurityScope ? sourceURL.startAccessingSecurityScopedResource() : false
    defer {
        if requiresSecurityScope && accessed {
            sourceURL.stopAccessingSecurityScopedResource()
        }
    }

    do {
        if fileManager.fileExists(atPath: destinationURL.path) {
            try fileManager.removeItem(at: destinationURL)
        }
        try fileManager.copyItem(at: sourceURL, to: destinationURL)
        return destinationURL
    } catch {
        return nil
    }
}
#endif
