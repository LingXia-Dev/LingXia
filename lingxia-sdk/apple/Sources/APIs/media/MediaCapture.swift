#if os(iOS)
import UIKit
import AVFoundation
import Photos
import PhotosUI
import UniformTypeIdentifiers

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
        button.setImage(LxAppMedia.controlImage(named: "icon_back"), for: .normal)
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
        cancelButton.setImage(LxAppMedia.controlImage(named: "icon_close"), for: .normal)
        cancelButton.tintColor = .white
        cancelButton.contentEdgeInsets = .zero
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        addSubview(cancelButton)

        switchCameraButton.translatesAutoresizingMaskIntoConstraints = false
        switchCameraButton.backgroundColor = .clear
        switchCameraButton.layer.cornerRadius = 0
        switchCameraButton.setImage(LxAppMedia.controlImage(named: "icon_switch"), for: .normal)
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
        cancelButton.setImage(LxAppMedia.controlImage(named: "icon_close"), for: .normal)
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
        switchCameraButton.setImage(LxAppMedia.controlImage(named: "icon_switch"), for: .normal)
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
        flashButton.setImage(LxAppMedia.controlImage(named: name), for: .normal)
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

func copyMediaFileToTemp(
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
