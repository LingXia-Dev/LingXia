#if os(iOS)
import UIKit
import AVFoundation

typealias CaptureFeedback = LxAppMedia.CaptureFeedback

final class PhotoCaptureViewController: UIViewController {
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
        do {
            return try MediaStorage.write(data: data, prefix: "photo", fileExtension: "jpg")
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

    static func makeBackButton(target: Any?, action: Selector) -> UIButton {
        let button = UIButton(type: .custom)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.setImage(LxAppMedia.controlImage(named: "icon_back_curved"), for: .normal)
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
        //  back button top-left, confirm button bottom-right
        let confirmButton = ReviewButtonFactory.makeConfirmButton(
            title: "lx_common_done".localized,
            target: self,
            action: #selector(confirmTapped)
        )

        let backButton = ReviewButtonFactory.makeBackButton(
            target: self,
            action: #selector(retakeTapped)
        )

        view.addSubview(confirmButton)
        view.addSubview(backButton)

        NSLayoutConstraint.activate([
            // Confirm button at bottom-right
            confirmButton.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
            confirmButton.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -20),
            confirmButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 80),
            confirmButton.heightAnchor.constraint(equalToConstant: 36),

            // Back button at top-left
            backButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            backButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
            backButton.widthAnchor.constraint(equalToConstant: 28),
            backButton.heightAnchor.constraint(equalToConstant: 28)
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
    private let cancelButton = UIButton(type: .custom)
    private let switchCameraButton = UIButton(type: .custom)
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
        switchCameraButton.accessibilityLabel = isFront ? "lx_camera_switch_to_back".localized : "lx_camera_switch_to_front".localized
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

        // Cancel button at bottom-left, aligned with capture button
        cancelButton.translatesAutoresizingMaskIntoConstraints = false
        cancelButton.setImage(LxAppMedia.controlImage(named: "icon_chevron_down"), for: .normal)
        cancelButton.tintColor = .white
        cancelButton.imageView?.contentMode = .scaleAspectFit
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        addSubview(cancelButton)

        // Switch camera button at top-right
        switchCameraButton.translatesAutoresizingMaskIntoConstraints = false
        switchCameraButton.setImage(LxAppMedia.controlImage(named: "icon_camera_switch"), for: .normal)
        switchCameraButton.tintColor = .white
        switchCameraButton.imageView?.contentMode = .scaleAspectFit
        switchCameraButton.addTarget(self, action: #selector(switchCameraTapped), for: .touchUpInside)
        addSubview(switchCameraButton)

        // Flash button at bottom, between cancel and capture buttons
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

            // Cancel button at bottom-left, vertically centered with capture button
            cancelButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            cancelButton.trailingAnchor.constraint(equalTo: captureButton.leadingAnchor, constant: -32),
            cancelButton.widthAnchor.constraint(equalToConstant: 44),
            cancelButton.heightAnchor.constraint(equalToConstant: 44),

            // Flash button between cancel and capture
            flashButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            flashButton.leadingAnchor.constraint(equalTo: captureButton.trailingAnchor, constant: 32),
            flashButton.widthAnchor.constraint(equalToConstant: 44),
            flashButton.heightAnchor.constraint(equalToConstant: 44),

            // Switch camera button at top-right
            switchCameraButton.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 24),
            switchCameraButton.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -24),
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
        let name = flashEnabled ? "icon_camera_flash_on" : "icon_camera_flash_off"
        flashButton.setImage(LxAppMedia.controlImage(named: name), for: .normal)
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

enum VideoCaptureResult {
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

final class VideoCaptureViewController: UIViewController {
    private let resultHandler: (VideoCaptureResult) -> Void
    private let maxDuration: TimeInterval
    private var currentPosition: AVCaptureDevice.Position

    private let session = AVCaptureSession()
    private let sessionQueue = DispatchQueue(label: "com.lingxia.camera.session")
    private let writerQueue = DispatchQueue(label: "com.lingxia.camera.writer")
    private let videoOutput = AVCaptureVideoDataOutput()
    private let audioOutput = AVCaptureAudioDataOutput()
    private lazy var sampleBufferDelegate = SampleBufferDelegate(owner: self)
    private var videoInput: AVCaptureDeviceInput?
    private var audioInput: AVCaptureDeviceInput?
    private var previewLayer: AVCaptureVideoPreviewLayer?
    private let minimumRecordingDuration: TimeInterval = 1.0
    private var assetWriter: AVAssetWriter?
    private var assetWriterVideoInput: AVAssetWriterInput?
    private var assetWriterAudioInput: AVAssetWriterInput?
    private var videoDimensions: CGSize = .zero
    private var currentRecordingURL: URL?
    private var isWriterSessionStarted = false
    private var isWriterFinishing = false

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
        overlayView.onRecordStarted = { [weak self] in
            self?.beginRecording()
        }
        overlayView.onRecordStopped = { [weak self] in
            self?.requestStopRecording()
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
                self.finish(with: .failure("lx_camera_audio_init_failed".localized))
                return
            }

            self.session.beginConfiguration()
            self.session.sessionPreset = .high

            guard let videoDevice = self.bestVideoDevice(position: self.currentPosition) else {
                self.finish(with: .failure("lx_camera_access_denied".localized))
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
                    self.finish(with: .failure("lx_camera_video_input_failed".localized))
                return
            }
            } catch {
                self.session.commitConfiguration()
                self.finish(with: .failure("lx_camera_init_failed".localized))
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

            self.videoOutput.alwaysDiscardsLateVideoFrames = false
            self.videoOutput.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            ]

            if self.session.canAddOutput(self.videoOutput) {
                self.session.addOutput(self.videoOutput)
                self.videoOutput.setSampleBufferDelegate(self.sampleBufferDelegate, queue: self.writerQueue)
            } else {
                self.session.commitConfiguration()
                self.finish(with: .failure("lx_camera_video_output_failed".localized))
                return
            }

            if self.session.canAddOutput(self.audioOutput) {
                self.session.addOutput(self.audioOutput)
                self.audioOutput.setSampleBufferDelegate(self.sampleBufferDelegate, queue: self.writerQueue)
            }

            self.session.commitConfiguration()
            self.configureVideoOutputConnection(orientation: self.currentVideoOrientation())
            self.updateTorchAvailability(for: videoDevice)
            self.startSessionIfNeeded()
        }
    }

    private func configureVideoOutputConnection(orientation: AVCaptureVideoOrientation) {
        guard let connection = videoOutput.connection(with: .video) else { return }
        if connection.isVideoOrientationSupported {
            connection.videoOrientation = orientation
        }
        if connection.isVideoMirroringSupported {
            connection.automaticallyAdjustsVideoMirroring = false
            connection.isVideoMirrored = currentPosition == .front
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

    private func handleCancelTapped() {
        requestCancelRecording()
    }

    private func beginRecording() {
        guard recordingState == .idle else { return }
        recordingState = .preparing(pending: .none)

        sessionQueue.async {
            if self.assetWriter != nil && self.isWriterSessionStarted {
                DispatchQueue.main.async {
                    self.recordingState = .recording
                }
                return
            }

            let orientation = self.currentVideoOrientation()
            self.configureVideoOutputConnection(orientation: orientation)

            let outputURL: URL
            do {
                outputURL = try self.makeOutputFileURL()
            } catch {
                DispatchQueue.main.async {
                    self.handleRecordingSetupFailure(error)
                }
                return
            }
            do {
                try self.prepareAssetWriter(at: outputURL, orientation: orientation)
            } catch {
                DispatchQueue.main.async {
                    self.handleRecordingSetupFailure(error)
                }
            }
        }
    }

    private func requestStopRecording() {
        switch recordingState {
        case .recording:
            CaptureFeedback.playRecordStop()
            recordingState = .finishing
            stopWriter(cancelled: false)
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
            stopWriter(cancelled: true)
        case .preparing:
            CaptureFeedback.playRecordStop()
            recordingState = .preparing(pending: .cancel)
        case .idle:
            finish(with: .cancelled)
        case .cancelling:
            break
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
                    self.overlayView.showHint("lx_camera_switch_failed".localized)
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
                        self.overlayView.showHint("lx_camera_switch_failed".localized)
                    }
                    return
                }
            } catch {
                self.session.commitConfiguration()
                DispatchQueue.main.async {
                    self.overlayView.showHint("lx_camera_switch_failed".localized)
                }
                return
            }

            self.session.commitConfiguration()
            self.configureVideoOutputConnection(orientation: self.currentVideoOrientation())
            self.updateTorchAvailability(for: newDevice)

            DispatchQueue.main.async {
                self.overlayView.updateCameraPosition(isFront: self.currentPosition == .front)
            }
        }
    }

    private func makeOutputFileURL() throws -> URL {
        guard let url = MediaStorage.makeFileURL(prefix: "video", preferredExtension: "mp4") else {
            throw NSError(domain: "LingXia.Camera", code: -3004, userInfo: [NSLocalizedDescriptionKey: "缓存目录不可用"])
        }
        return url
    }

    private func prepareAssetWriter(at url: URL, orientation: AVCaptureVideoOrientation) throws {
        if FileManager.default.fileExists(atPath: url.path) {
            try FileManager.default.removeItem(at: url)
        }

        let writer = try AVAssetWriter(url: url, fileType: .mp4)
        writer.shouldOptimizeForNetworkUse = true

        guard let videoSettings = videoOutput.recommendedVideoSettingsForAssetWriter(writingTo: .mp4) as? [String: Any] else {
            throw NSError(domain: "LingXia.Camera", code: -3001, userInfo: [NSLocalizedDescriptionKey: "无法创建视频编码配置"])
        }

        let width = (videoSettings[AVVideoWidthKey] as? NSNumber)?.doubleValue ?? 0
        let height = (videoSettings[AVVideoHeightKey] as? NSNumber)?.doubleValue ?? 0
        if width > 0, height > 0 {
            videoDimensions = CGSize(width: width, height: height)
        } else {
            videoDimensions = CGSize(width: 1280, height: 720)
        }

        let videoWriterInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoWriterInput.expectsMediaDataInRealTime = true
        // Let AVCaptureConnection handle buffer orientation; keep track transform identity
        guard writer.canAdd(videoWriterInput) else {
            throw NSError(domain: "LingXia.Camera", code: -3002, userInfo: [NSLocalizedDescriptionKey: "lx_camera_video_input_failed".localized])
        }
        writer.add(videoWriterInput)

        var audioWriterInput: AVAssetWriterInput?
        if let rawSettings = audioOutput.recommendedAudioSettingsForAssetWriter(writingTo: .mp4) {
            let audioSettings = rawSettings as? [String: Any]
            if let audioSettings {
                let input = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
                input.expectsMediaDataInRealTime = true
                if writer.canAdd(input) {
                    writer.add(input)
                    audioWriterInput = input
                }
            }
        }

        if audioWriterInput == nil {
            // Fallback AAC settings in case recommended settings are unavailable.
            let fallbackAudioSettings: [String: Any] = [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVNumberOfChannelsKey: 1,
                AVSampleRateKey: 44_100,
                AVEncoderBitRateKey: 64_000
            ]
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: fallbackAudioSettings)
            input.expectsMediaDataInRealTime = true
            if writer.canAdd(input) {
                writer.add(input)
                audioWriterInput = input
            }
        }

        assetWriter = writer
        assetWriterVideoInput = videoWriterInput
        assetWriterAudioInput = audioWriterInput
        currentRecordingURL = url
        isWriterSessionStarted = false
        isWriterFinishing = false
    }

    @MainActor
    private func handleRecordingSetupFailure(_ error: Error) {
        overlayView.showHint("录制失败: \(error.localizedDescription)")
        recordingState = .idle
        finish(with: .failure("录制失败"))
    }

    // With AVCaptureVideoDataOutput, we rely on AVCaptureConnection.videoOrientation
    // to deliver buffers in the desired orientation, so we keep track transform identity.

    private func handleWriterDidStart() {
        if recordingStartDate == nil {
            recordingStartDate = Date()
            startUpdateTimer()
        }

        let pendingAction: PendingRecordingAction
        if case .preparing(let action) = recordingState {
            pendingAction = action
        } else {
            pendingAction = .none
        }

        switch pendingAction {
        case .none:
            recordingState = .recording
        case .stop:
            recordingState = .finishing
            stopWriter(cancelled: false)
        case .cancel:
            recordingState = .cancelling
            stopWriter(cancelled: true)
        }
    }

    private func stopWriter(cancelled: Bool) {
        writerQueue.async { [weak self] in
            self?.finishRecording(cancelled: cancelled)
        }
    }

    private func finishRecording(cancelled: Bool) {
        guard let writer = assetWriter else {
            let url = currentRecordingURL
            resetWriterState()
            DispatchQueue.main.async {
                self.handleWriterCompletion(url: url, error: nil, cancelled: true)
            }
            return
        }

        if isWriterFinishing {
            return
        }
        isWriterFinishing = true

        let outputURL = currentRecordingURL
        let markInputs: () -> Void = { [videoInput = assetWriterVideoInput, audioInput = assetWriterAudioInput] in
            videoInput?.markAsFinished()
            audioInput?.markAsFinished()
        }

        let complete: (_ cancelled: Bool) -> Void = { [weak self, outputURL] completedCancelled in
            guard let self else { return }
            self.resetWriterState()
            DispatchQueue.main.async {
                self.handleWriterCompletion(url: outputURL, error: writer.error, cancelled: completedCancelled)
            }
        }

        if isWriterSessionStarted {
            markInputs()
            writer.finishWriting {
                self.writerQueue.async {
                    complete(cancelled)
                }
            }
        } else {
            writer.cancelWriting()
            self.writerQueue.async {
                complete(true)
            }
        }
    }

    private func resetWriterState() {
        assetWriter = nil
        assetWriterVideoInput = nil
        assetWriterAudioInput = nil
        isWriterSessionStarted = false
        isWriterFinishing = false
        videoDimensions = .zero
    }

    @MainActor
    private func handleWriterCompletion(url: URL?, error: Error?, cancelled: Bool) {
        let recordedDuration = recordingStartDate.map { Date().timeIntervalSince($0) } ?? 0
        recordingStartDate = nil
        stopUpdateTimer()
        let outputURL = url
        currentRecordingURL = nil

        if let error = error {
            if let outputURL, FileManager.default.fileExists(atPath: outputURL.path) {
                try? FileManager.default.removeItem(at: outputURL)
            }
            overlayView.showHint("录制失败: \(error.localizedDescription)")
            recordingState = .idle
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                self.overlayView.updateToIdle()
                self.finish(with: .failure("录制失败"))
            }
            return
        }

        guard let outputURL else {
            recordingState = .idle
            finish(with: cancelled ? .cancelled : .failure("录制失败"))
            return
        }

        if cancelled {
            if FileManager.default.fileExists(atPath: outputURL.path) {
                try? FileManager.default.removeItem(at: outputURL)
            }
            recordingState = .idle
            finish(with: .cancelled)
            return
        }

        recordingState = .idle

        if recordedDuration < minimumRecordingDuration {
            if FileManager.default.fileExists(atPath: outputURL.path) {
                try? FileManager.default.removeItem(at: outputURL)
            }
            overlayView.showHint("拍摄时间过短")
            overlayView.updateToIdle(after: 1.0)
            return
        }

        presentReview(for: outputURL)
    }

    nonisolated(unsafe) fileprivate func handleSampleBuffer(_ sampleBuffer: CMSampleBuffer, from output: AVCaptureOutput) {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return }
        guard !isWriterFinishing else { return }
        guard let writer = assetWriter else { return }

        if writer.status == .failed {
            DispatchQueue.main.async { [weak self] in
                self?.handleWriterFailure(error: writer.error)
            }
            return
        }

        let isVideo = output === videoOutput
        let isAudio = output === audioOutput

        guard isVideo || isAudio else { return }

        if isVideo && !isWriterSessionStarted {
            if writer.status == .unknown && !writer.startWriting() {
                DispatchQueue.main.async { [weak self] in
                    self?.handleWriterFailure(error: writer.error)
                }
                return
            }
            let startTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
            writer.startSession(atSourceTime: startTime)
            isWriterSessionStarted = true
            DispatchQueue.main.async { [weak self] in
                self?.handleWriterDidStart()
            }
        }

        guard isWriterSessionStarted else { return }

        let input = isVideo ? assetWriterVideoInput : assetWriterAudioInput
        guard let writerInput = input else { return }
        guard writerInput.isReadyForMoreMediaData else { return }

        if !writerInput.append(sampleBuffer) {
            DispatchQueue.main.async { [weak self] in
                self?.handleWriterFailure(error: writer.error)
            }
        }
    }

    @MainActor
    private func handleWriterFailure(error: Error?) {
        guard !isWriterFinishing else { return }
        isWriterFinishing = true
        let outputURL = currentRecordingURL
        assetWriter?.cancelWriting()
        resetWriterState()
        DispatchQueue.main.async {
            self.handleWriterCompletion(url: outputURL, error: error ?? NSError(domain: "LingXia.Camera", code: -3003, userInfo: [NSLocalizedDescriptionKey: "录制失败"]), cancelled: false)
        }
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
                if self.recordingState == .recording {
                    DispatchQueue.main.async {
                        self.overlayView.showMaxDurationReached()
                    }
                }
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

private final class SampleBufferDelegate: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCaptureAudioDataOutputSampleBufferDelegate {
    private weak var owner: VideoCaptureViewController?

    init(owner: VideoCaptureViewController) {
        self.owner = owner
        super.init()
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        owner?.handleSampleBuffer(sampleBuffer, from: output)
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
        // back button top-left, confirm button bottom-right
        let confirmButton = ReviewButtonFactory.makeConfirmButton(
            title: "lx_common_done".localized,
            target: self,
            action: #selector(confirmTapped)
        )

        let backButton = ReviewButtonFactory.makeBackButton(
            target: self,
            action: #selector(retakeTapped)
        )

        view.addSubview(confirmButton)
        view.addSubview(backButton)

        NSLayoutConstraint.activate([
            // Confirm button at bottom-right
            confirmButton.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
            confirmButton.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -20),
            confirmButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 80),
            confirmButton.heightAnchor.constraint(equalToConstant: 36),

            // Back button at top-left
            backButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            backButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
            backButton.widthAnchor.constraint(equalToConstant: 28),
            backButton.heightAnchor.constraint(equalToConstant: 28)
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
    var onRecordStarted: (() -> Void)?
    var onRecordStopped: (() -> Void)?
    var onCancelTapped: (() -> Void)?
    var onSwitchCameraTapped: (() -> Void)?
    var onFlashToggle: ((Bool) -> Void)?

    private var maxDuration: TimeInterval = 15
    private var recordingActive = false
    private var cameraSwitchAvailable = false
    private var flashEnabled = false
    private var flashAvailable = false
    private var longPressTimer: Timer?
    private let longPressDelay: TimeInterval = 0.28

    private let captureButton = UIButton(type: .custom)
    private let progressRingLayer = CAShapeLayer()
    private let progressLayer = CAShapeLayer()
    private let innerCircleLayer = CAShapeLayer()
    private let timerLabel = UILabel()
    private let hintLabel = UILabel()
    private let cancelButton = UIButton(type: .custom)
    private let switchCameraButton = UIButton(type: .custom)
    private let flashButton = UIButton(type: .system)
    private let flashButtonSize: CGFloat = 44
    private let progressGreenColor = UIColor(red: 7/255, green: 193/255, blue: 96/255, alpha: 1.0)
    private let redDot = "●"

    override init(frame: CGRect) {
        super.init(frame: frame)
        configureView()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setInitialTimer(maxDuration: TimeInterval) {
        self.maxDuration = maxDuration
        setTimerText(Self.format(time: maxDuration, rounding: .up))
        timerLabel.isHidden = true
        applyCameraButtonState()
    }

    func updateToIdle() {
        recordingActive = false
        hintLabel.text = "lx_camera_long_press_to_record".localized
        timerLabel.isHidden = true
        setProgress(0)
        applyCameraButtonState()
        UIView.animate(withDuration: 0.2) {
            self.cancelButton.alpha = 1.0
        }
        animateCaptureShape(isRecording: false)
    }

    func updateToIdle(after delay: TimeInterval) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            self?.updateToIdle()
        }
    }

    func updateToRecording() {
        recordingActive = true
        hintLabel.text = "lx_camera_release_to_stop".localized
        timerLabel.isHidden = false
        applyCameraButtonState()
        animateCaptureShape(isRecording: true)
    }

    func updateToCancelling() {
        hintLabel.text = "lx_camera_cancelling".localized
        timerLabel.isHidden = true
        applyCameraButtonState()
        cancelButton.alpha = 1.0
    }

    func showHint(_ text: String) {
        hintLabel.text = text
    }

    private func setTimerText(_ text: String) {
        let full = "\(redDot) \(text)"
        let attr = NSMutableAttributedString(string: full)
        attr.addAttribute(.foregroundColor, value: UIColor.red, range: NSRange(location: 0, length: 1))
        if full.count > 2 {
            attr.addAttribute(.foregroundColor, value: UIColor.white, range: NSRange(location: 2, length: full.count - 2))
        }
        timerLabel.attributedText = attr
        timerLabel.accessibilityLabel = full
    }

    func showMaxDurationReached() {
        hintLabel.text = "lx_camera_max_duration_reached".localized
        timerLabel.isHidden = false
    }

    func updateRecordingProgress(elapsed: TimeInterval, maxDuration: TimeInterval) {
        self.maxDuration = maxDuration
        let remaining = max(maxDuration - elapsed, 0)
        setTimerText(Self.format(time: remaining, rounding: .up))
        timerLabel.isHidden = !recordingActive
        let progress = min(elapsed / maxDuration, 1.0)
        setProgress(progress)
    }

    func setProgress(_ progress: Double) {
        progressLayer.strokeEnd = CGFloat(progress)
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
        switchCameraButton.accessibilityLabel = isFront ? "lx_camera_switch_to_back".localized : "lx_camera_switch_to_front".localized
    }

    private func configureView() {
        backgroundColor = .clear

        // Capture button with progress ring
        captureButton.translatesAutoresizingMaskIntoConstraints = false
        captureButton.backgroundColor = .clear
        captureButton.adjustsImageWhenHighlighted = false
        captureButton.addTarget(self, action: #selector(recordTouchDown), for: .touchDown)
        captureButton.addTarget(self, action: #selector(recordTouchUp), for: [.touchUpInside, .touchUpOutside, .touchDragExit, .touchCancel])
        captureButton.accessibilityLabel = "lx_camera_record_video".localized
        captureButton.accessibilityTraits.insert(.button)
        addSubview(captureButton)

        // Setup progress ring layers
        setupProgressRingLayers()

        timerLabel.translatesAutoresizingMaskIntoConstraints = false
        timerLabel.font = UIFont.monospacedDigitSystemFont(ofSize: 16, weight: .medium)
        timerLabel.textAlignment = .center
        timerLabel.textColor = .white
        setTimerText("00:00")
        timerLabel.isHidden = true
        timerLabel.backgroundColor = UIColor.black.withAlphaComponent(0.5)
        timerLabel.layer.cornerRadius = 16
        timerLabel.layer.masksToBounds = true
        addSubview(timerLabel)

        hintLabel.translatesAutoresizingMaskIntoConstraints = false
        hintLabel.textAlignment = .center
        hintLabel.textColor = UIColor.white.withAlphaComponent(0.8)
        hintLabel.font = UIFont.systemFont(ofSize: 14, weight: .medium)
        addSubview(hintLabel)

        // Cancel button at bottom-left
        cancelButton.translatesAutoresizingMaskIntoConstraints = false
        cancelButton.setImage(LxAppMedia.controlImage(named: "icon_chevron_down"), for: .normal)
        cancelButton.tintColor = .white
        cancelButton.accessibilityLabel = "lx_common_cancel".localized
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        addSubview(cancelButton)

        // Flash button between cancel and capture
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

        // Switch camera button at top-right
        switchCameraButton.translatesAutoresizingMaskIntoConstraints = false
        switchCameraButton.setImage(LxAppMedia.controlImage(named: "icon_camera_switch"), for: .normal)
        switchCameraButton.tintColor = .white
        switchCameraButton.alpha = 0.0
        switchCameraButton.addTarget(self, action: #selector(switchCameraTapped), for: .touchUpInside)
        addSubview(switchCameraButton)

        NSLayoutConstraint.activate([
            captureButton.centerXAnchor.constraint(equalTo: centerXAnchor),
            captureButton.bottomAnchor.constraint(equalTo: safeAreaLayoutGuide.bottomAnchor, constant: -20),
            captureButton.widthAnchor.constraint(equalToConstant: 88),
            captureButton.heightAnchor.constraint(equalToConstant: 88),

            hintLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            hintLabel.bottomAnchor.constraint(equalTo: captureButton.topAnchor, constant: -45),

            timerLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            timerLabel.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 38),
            timerLabel.widthAnchor.constraint(greaterThanOrEqualToConstant: 80),
            timerLabel.heightAnchor.constraint(equalToConstant: 32),

            // Cancel button at bottom-left, vertically centered with capture button
            cancelButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            cancelButton.trailingAnchor.constraint(equalTo: captureButton.leadingAnchor, constant: -32),
            cancelButton.widthAnchor.constraint(equalToConstant: 44),
            cancelButton.heightAnchor.constraint(equalToConstant: 44),

            // Flash button between cancel and capture
            flashButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            flashButton.leadingAnchor.constraint(equalTo: captureButton.trailingAnchor, constant: 32),
            flashButton.widthAnchor.constraint(equalToConstant: 44),
            flashButton.heightAnchor.constraint(equalToConstant: 44),

            // Switch camera button at top-right
            switchCameraButton.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 24),
            switchCameraButton.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -24),
            switchCameraButton.widthAnchor.constraint(equalToConstant: 44),
            switchCameraButton.heightAnchor.constraint(equalToConstant: 44)
        ])

        updateToIdle()
    }

    private func setupProgressRingLayers() {
        progressRingLayer.fillColor = UIColor.clear.cgColor
        progressRingLayer.strokeColor = UIColor(white: 0.23, alpha: 0.33).cgColor
        progressRingLayer.lineCap = .round
        captureButton.layer.addSublayer(progressRingLayer)

        progressLayer.fillColor = UIColor.clear.cgColor
        progressLayer.strokeColor = progressGreenColor.cgColor
        progressLayer.lineWidth = 2
        progressLayer.lineCap = .round
        progressLayer.strokeStart = 0
        progressLayer.strokeEnd = 0
        captureButton.layer.addSublayer(progressLayer)

        innerCircleLayer.fillColor = UIColor.white.cgColor
        captureButton.layer.addSublayer(innerCircleLayer)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        updateProgressRingLayers()
    }

    private struct CaptureGeometry {
        let buttonSize: CGFloat
        let ringStrokeWidth: CGFloat
        let center: CGPoint
        let radius: CGFloat
    }

    private func captureGeometry() -> CaptureGeometry? {
        let buttonSize = captureButton.bounds.width
        guard buttonSize > 0 else { return nil }

        let ringStrokeWidth: CGFloat = buttonSize * 0.22
        let center = CGPoint(x: buttonSize / 2, y: buttonSize / 2)
        let radius = (buttonSize - ringStrokeWidth) / 2
        return CaptureGeometry(buttonSize: buttonSize, ringStrokeWidth: ringStrokeWidth, center: center, radius: radius)
    }

    private func updateProgressRingLayers() {
        guard let geometry = captureGeometry() else { return }

        let ringPath = UIBezierPath(arcCenter: geometry.center, radius: geometry.radius, startAngle: 0, endAngle: .pi * 2, clockwise: true)
        progressRingLayer.path = ringPath.cgPath
        progressRingLayer.lineWidth = geometry.ringStrokeWidth

        let progressPath = UIBezierPath(arcCenter: geometry.center, radius: geometry.buttonSize / 2 - 1, startAngle: -.pi / 2, endAngle: .pi * 1.5, clockwise: true)
        progressLayer.path = progressPath.cgPath

        let innerRadius = recordingActive ? geometry.radius - geometry.ringStrokeWidth * 0.6 : geometry.radius - 2
        innerCircleLayer.path = UIBezierPath(arcCenter: geometry.center, radius: innerRadius, startAngle: 0, endAngle: .pi * 2, clockwise: true).cgPath
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
            switchCameraButton.alpha = recordingActive ? 0.0 : 1.0
        }
        flashButton.isHidden = !flashAvailable
        flashButton.isEnabled = flashAvailable && !recordingActive
        if flashAvailable {
            flashButton.alpha = flashButton.isEnabled ? 1.0 : 0.0
        } else {
            flashButton.alpha = 0.0
        }
        // Hide cancel and hint during recording
        cancelButton.isHidden = recordingActive
        hintLabel.isHidden = recordingActive
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
        let name = flashEnabled ? "icon_camera_flash_on" : "icon_camera_flash_off"
        flashButton.setImage(LxAppMedia.controlImage(named: name), for: .normal)
        flashButton.accessibilityLabel = flashEnabled ? "关闭闪光灯" : "开启闪光灯"
    }

    private func setInnerCircle(using geometry: CaptureGeometry, radius: CGFloat, fillColor: CGColor, duration: CFTimeInterval) {
        let innerPath = UIBezierPath(arcCenter: geometry.center, radius: radius, startAngle: 0, endAngle: .pi * 2, clockwise: true).cgPath
        CATransaction.begin()
        CATransaction.setAnimationDuration(duration)
        CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeInEaseOut))
        innerCircleLayer.path = innerPath
        innerCircleLayer.fillColor = fillColor
        CATransaction.commit()
    }

    private func animateCaptureShape(isRecording: Bool) {
        guard let geometry = captureGeometry() else { return }
        let innerRadius = isRecording ? geometry.radius - geometry.ringStrokeWidth * 0.6 : geometry.radius - 2
        let fillColor = isRecording ? UIColor(white: 0.88, alpha: 1.0).cgColor : UIColor.white.cgColor
        setInnerCircle(using: geometry, radius: innerRadius, fillColor: fillColor, duration: 0.18)
    }

    @objc private func recordTouchDown() {
        // Visual feedback immediately
        animatePressVisual(pressed: true)
        // Start long-press timer
        longPressTimer?.invalidate()
        longPressTimer = Timer.scheduledTimer(withTimeInterval: longPressDelay, repeats: false) { [weak self] _ in
            guard let self else { return }
            self.longPressTimer = nil
            if !self.recordingActive {
                self.onRecordStarted?()
            }
        }
    }

    @objc private func recordTouchUp() {
        longPressTimer?.invalidate()
        longPressTimer = nil
        animatePressVisual(pressed: false)
        if recordingActive {
            onRecordStopped?()
        }
    }

    private func animatePressVisual(pressed: Bool) {
        guard let geometry = captureGeometry() else { return }
        let innerRadius = pressed ? geometry.radius - geometry.ringStrokeWidth * 0.4 : geometry.radius - 2
        let fillColor = pressed ? UIColor(white: 0.88, alpha: 1.0).cgColor : UIColor.white.cgColor
        setInnerCircle(using: geometry, radius: innerRadius, fillColor: fillColor, duration: 0.12)
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

#endif
