import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
import AVKit
import AVFoundation
import Photos
import PhotosUI
import UniformTypeIdentifiers
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

private final class MediaPreviewViewController: UIViewController {
    private let items: [PreviewMediaItem]
    private var currentIndex: Int

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
            return MediaPreviewVideoController(item: item, index: index, dismissHandler: { [weak self] in self?.dismiss(animated: true) })
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
    private let dismissHandler: () -> Void

    private var playerVC: AVPlayerViewController?
    private var player: AVPlayer?
    private let closeButton = UIButton(type: .system)
    private var coverOverlay: UIImageView?
    private var timeObserver: Any?
    private var hasStartedPlayback = false

    init(item: PreviewMediaItem, index: Int, dismissHandler: @escaping () -> Void) {
        self.item = item
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

        // Close button setup
        closeButton.translatesAutoresizingMaskIntoConstraints = false
        if let closeImage = UIImage(systemName: "xmark.circle.fill") {
            closeButton.setImage(closeImage, for: .normal)
            closeButton.tintColor = .white
        } else {
            closeButton.setTitle("Close", for: .normal)
            closeButton.setTitleColor(.white, for: .normal)
        }
        closeButton.addTarget(self, action: #selector(closeTapped), for: .touchUpInside)
        view.addSubview(closeButton)

        NSLayoutConstraint.activate([
            closeButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
            closeButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 12)
        ])

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

    @objc private func closeTapped() {
        player?.pause()
        hasStartedPlayback = false
        cleanupTimeObserver()
        dismissHandler()
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
@MainActor
    private static var albumPickerDelegate: AlbumDelegate?
    @MainActor
    private static var cameraPickerDelegate: CameraDelegate?
#endif

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

            if allowCamera && !allowAlbum {
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

            let picker = StatusBarHiddenImagePickerController()
            picker.sourceType = .camera
            picker.allowsEditing = false
            picker.modalPresentationStyle = .fullScreen
            picker.modalPresentationCapturesStatusBarAppearance = true

            if #available(iOS 14.0, *) {
                picker.mediaTypes = [UTType.image.identifier]
            } else {
                picker.mediaTypes = ["public.image"]
            }

            picker.cameraCaptureMode = .photo

            let desiredFacing = desiredFacingFront
                ? UIImagePickerController.CameraDevice.front
                : UIImagePickerController.CameraDevice.rear

            if UIImagePickerController.isCameraDeviceAvailable(desiredFacing) {
                picker.cameraDevice = desiredFacing
            } else if UIImagePickerController.isCameraDeviceAvailable(.rear) {
                picker.cameraDevice = .rear
            }

            let delegate = CameraDelegate(callbackId: callbackId, captureMode: .photo) {
                LxAppMedia.cameraPickerDelegate = nil
            }
            LxAppMedia.cameraPickerDelegate = delegate
            picker.delegate = delegate

            presenter.present(picker, animated: true)
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

// MARK: - Camera Delegate
private final class CameraDelegate: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
    enum CaptureMode {
        case photo
        case video
    }

    private let callbackId: UInt64
    private let captureMode: CaptureMode
    private let cleanup: () -> Void

    init(callbackId: UInt64, captureMode: CaptureMode, cleanup: @escaping () -> Void) {
        self.callbackId = callbackId
        self.captureMode = captureMode
        self.cleanup = cleanup
        super.init()
    }

    func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
        picker.dismiss(animated: true) {
            let cancelPayload = "{\"cancel\":true}"
            let _ = onCallback(self.callbackId, true, cancelPayload)
            self.cleanup()
        }
    }

    func imagePickerController(
        _ picker: UIImagePickerController,
        didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
    ) {
        let mediaType = (info[.mediaType] as? String) ?? ""
        let movieIdentifier: String
        if #available(iOS 14.0, *) {
            movieIdentifier = UTType.movie.identifier
        } else {
            movieIdentifier = "public.movie"
        }

        let isVideoCapture = captureMode == .video
            || mediaType == movieIdentifier
            || mediaType == "public.movie"
            || mediaType.contains("movie")

        var jsonItem: [String: Any]?

        if isVideoCapture {
            if let mediaURL = info[.mediaURL] as? URL {
                if let tempURL = copyMediaFileToTemp(
                    from: mediaURL,
                    prefix: "camera_video",
                    fallbackExtension: "mov",
                    requiresSecurityScope: false
                ) {
                    jsonItem = [
                        "uri": tempURL.absoluteString,
                        "fileType": "video",
                        "isOriginal": true
                    ]
                }
            }
        } else {
            if let imageURL = info[.imageURL] as? URL {
                let ext = imageURL.pathExtension.isEmpty ? "jpg" : imageURL.pathExtension
                if let tempURL = copyMediaFileToTemp(
                    from: imageURL,
                    prefix: "camera_image",
                    fallbackExtension: ext,
                    requiresSecurityScope: false
                ) {
                    jsonItem = [
                        "uri": tempURL.absoluteString,
                        "fileType": "image",
                        "isOriginal": true
                    ]
                }
            } else if let image = info[.originalImage] as? UIImage {
                if let tempURL = saveCapturedImageToTemp(image) {
                    jsonItem = [
                        "uri": tempURL.absoluteString,
                        "fileType": "image",
                        "isOriginal": true
                    ]
                }
            }
        }

        picker.dismiss(animated: true) {
            if let item = jsonItem {
                self.sendSuccess(with: item)
            } else {
                let _ = onCallback(self.callbackId, false, "Failed to capture media")
                self.cleanup()
            }
        }
    }

    private func sendSuccess(with item: [String: Any]) {
        do {
            let data = try JSONSerialization.data(withJSONObject: [item], options: [])
            let jsonString = String(data: data, encoding: .utf8) ?? "[]"
            let _ = onCallback(callbackId, true, jsonString)
        } catch {
            let _ = onCallback(callbackId, false, "Failed to serialize camera capture result")
        }
        cleanup()
    }

    private func saveCapturedImageToTemp(_ image: UIImage) -> URL? {
        guard let data = image.jpegData(compressionQuality: 0.95) else {
            return nil
        }
        let tempDir = FileManager.default.temporaryDirectory
        let fileURL = tempDir.appendingPathComponent("camera_image_\(UUID().uuidString).jpg")
        do {
            try data.write(to: fileURL)
            return fileURL
        } catch {
            return nil
        }
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
private final class StatusBarHiddenImagePickerController: UIImagePickerController {
    override var prefersStatusBarHidden: Bool { true }
    override var preferredStatusBarUpdateAnimation: UIStatusBarAnimation { .fade }
}

private enum VideoCaptureResult {
    case success(URL)
    case cancelled
    case failure(String)
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
    private var isRecording = false
    private var isCancelling = false
    private var recordingStartDate: Date?
    private var updateTimer: Timer?
    private var lastPressInside = true
    private var pendingReviewURL: URL?

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
        overlayView.onLongPressChanged = { [weak self] state in
            self?.handleLongPressState(state)
        }
        overlayView.onCancelTapped = { [weak self] in
            self?.handleCancelTapped()
        }
        overlayView.onSwitchCameraTapped = { [weak self] in
            self?.switchCamera()
        }
        view.addSubview(overlayView)
        NSLayoutConstraint.activate([
            overlayView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlayView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlayView.topAnchor.constraint(equalTo: view.topAnchor),
            overlayView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
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

    private func handleLongPressState(_ state: VideoCaptureOverlayView.LongPressState) {
        switch state {
        case .began:
            lastPressInside = true
            overlayView.updateFingerOutside(false)
            if !isRecording {
                beginRecording()
            }
        case .changed(let isInside):
            lastPressInside = isInside
            overlayView.updateFingerOutside(!isInside)
        case .ended:
            overlayView.updateFingerOutside(false)
            if isRecording {
                if lastPressInside {
                    stopRecording()
                } else {
                    cancelRecording()
                }
            } else {
                overlayView.updateToIdle()
            }
        case .cancelled:
            overlayView.updateFingerOutside(false)
            if isRecording {
                cancelRecording()
        } else {
                finish(with: .cancelled)
            }
        }
    }

    private func handleCancelTapped() {
        if isRecording {
            cancelRecording()
        } else {
            finish(with: .cancelled)
        }
    }

    private func beginRecording() {
        sessionQueue.async {
            guard !self.movieOutput.isRecording else { return }
            if let connection = self.movieOutput.connection(with: .video) {
                connection.videoOrientation = self.currentVideoOrientation()
            }

            let outputURL = self.makeTemporaryFileURL()
            self.isCancelling = false

            DispatchQueue.main.async {
                self.overlayView.prepareForRecording()
            }

            self.movieOutput.startRecording(to: outputURL, recordingDelegate: self)
        }
    }

    private func stopRecording() {
        sessionQueue.async {
            guard self.movieOutput.isRecording else { return }
            self.movieOutput.stopRecording()
        }
    }

    private func cancelRecording() {
        sessionQueue.async {
            guard self.movieOutput.isRecording else {
            DispatchQueue.main.async {
                    self.overlayView.updateToIdle()
                }
                return
            }
            self.isCancelling = true
            DispatchQueue.main.async {
                self.overlayView.updateToCancelling()
            }
            self.movieOutput.stopRecording()
        }
    }

    private func switchCamera() {
        guard !isRecording else { return }
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
            guard let self = self, self.isRecording, let start = self.recordingStartDate else { return }
            let elapsed = Date().timeIntervalSince(start)
            DispatchQueue.main.async {
                self.overlayView.updateRecordingProgress(elapsed: elapsed, maxDuration: self.maxDuration)
            }
            if elapsed >= self.maxDuration {
                self.stopRecording()
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
        overlayView.updateToIdle()
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
        stopSession()
        pendingReviewURL = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        DispatchQueue.main.async {
            self.dismiss(animated: true) {
                self.resultHandler(result)
            }
        }
    }
}

extension VideoCaptureViewController: @preconcurrency AVCaptureFileOutputRecordingDelegate {
    @MainActor
    func fileOutput(_ output: AVCaptureFileOutput, didStartRecordingTo fileURL: URL, from connections: [AVCaptureConnection]) {
        self.isRecording = true
        self.recordingStartDate = Date()
        self.overlayView.updateToRecording()
        self.startUpdateTimer()
    }

    @MainActor
    func fileOutput(_ output: AVCaptureFileOutput, didFinishRecordingTo outputFileURL: URL, from connections: [AVCaptureConnection], error: Error?) {
        self.isRecording = false
        self.recordingStartDate = nil
        self.stopUpdateTimer()
        if let error {
            let nsError = error as NSError
            if nsError.domain == AVFoundationErrorDomain,
               nsError.code == AVError.Code.maximumDurationReached.rawValue {
                self.overlayView.showMaxDurationReached()
                self.overlayView.updateToFinishing()
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

        if self.isCancelling {
            if FileManager.default.fileExists(atPath: outputFileURL.path) {
                try? FileManager.default.removeItem(at: outputFileURL)
            }
            self.isCancelling = false
            self.overlayView.updateToIdle()
            self.finish(with: .cancelled)
            return
        }
        self.isCancelling = false
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
        let confirmButton = UIButton(type: .system)
        confirmButton.translatesAutoresizingMaskIntoConstraints = false
        confirmButton.setTitle("完成", for: .normal)
        confirmButton.titleLabel?.font = UIFont.systemFont(ofSize: 17, weight: .semibold)
        confirmButton.setTitleColor(.white, for: .normal)
        confirmButton.backgroundColor = UIColor.systemBlue.withAlphaComponent(0.9)
        confirmButton.layer.cornerRadius = 20
        confirmButton.contentEdgeInsets = UIEdgeInsets(top: 10, left: 24, bottom: 10, right: 24)
        confirmButton.addTarget(self, action: #selector(confirmTapped), for: .touchUpInside)

        let retakeButton = UIButton(type: .system)
        retakeButton.translatesAutoresizingMaskIntoConstraints = false
        retakeButton.setTitle("返回", for: .normal)
        retakeButton.titleLabel?.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        retakeButton.setTitleColor(.white, for: .normal)
        retakeButton.backgroundColor = UIColor.black.withAlphaComponent(0.45)
        retakeButton.layer.cornerRadius = 18
        retakeButton.contentEdgeInsets = UIEdgeInsets(top: 8, left: 16, bottom: 8, right: 16)
        retakeButton.addTarget(self, action: #selector(retakeTapped), for: .touchUpInside)

        view.addSubview(confirmButton)
        view.addSubview(retakeButton)

            NSLayoutConstraint.activate([
            confirmButton.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -24),
            confirmButton.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -24),

            retakeButton.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            retakeButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16)
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
    enum LongPressState {
        case began
        case changed(isInside: Bool)
        case ended
        case cancelled
    }

    var onLongPressChanged: ((LongPressState) -> Void)?
    var onCancelTapped: (() -> Void)?
    var onSwitchCameraTapped: (() -> Void)?

    private var maxDuration: TimeInterval = 15
    private var recordingActive = false
    private var cameraSwitchAvailable = false

    private let captureButton = UIView()
    private let innerCircle = UIView()
    private let hintLabel = UILabel()
    private let timerLabel = UILabel()
    private let cancelButton = UIButton(type: .system)
    private let switchCameraButton = UIButton(type: .system)
    private let progressBackgroundLayer = CAShapeLayer()
    private let progressLayer = CAShapeLayer()
    private let cancelBaseColor = UIColor.clear
    private let cancelHighlightColor = UIColor.clear
    private let cancelNormalTextColor = UIColor.white
    private let cancelHighlightTextColor = UIColor.systemRed

    private lazy var longPressRecognizer: UILongPressGestureRecognizer = {
        let recognizer = UILongPressGestureRecognizer(target: self, action: #selector(handleLongPress(_:)))
        recognizer.minimumPressDuration = 0.05
        recognizer.allowableMovement = 15
        return recognizer
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        configureView()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    override func layoutSubviews() {
        super.layoutSubviews()
        updateProgressPath()
    }

    func setInitialTimer(maxDuration: TimeInterval) {
        self.maxDuration = maxDuration
        timerLabel.text = Self.format(time: maxDuration, rounding: .up)
        timerLabel.isHidden = true
        updateProgressLayer(progress: 0, hidden: true)
        applyCameraButtonState()
    }

    func updateToIdle() {
        recordingActive = false
        hintLabel.text = "长按摄像"
        updateFingerOutside(false)
        timerLabel.isHidden = true
        updateProgressLayer(progress: 0, hidden: true)
        applyCameraButtonState()
        UIView.animate(withDuration: 0.2) {
            self.innerCircle.transform = .identity
            self.cancelButton.alpha = 1.0
        }
    }

    func updateToIdle(after delay: TimeInterval) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            self?.updateToIdle()
        }
    }

    func prepareForRecording() {
        hintLabel.text = "准备录制..."
        timerLabel.text = Self.format(time: maxDuration, rounding: .up)
        timerLabel.isHidden = false
        cancelButton.alpha = 0.0
    }

    func updateToRecording() {
        recordingActive = true
        hintLabel.text = "松开停止"
        cancelButton.backgroundColor = cancelBaseColor
        cancelButton.setTitleColor(cancelNormalTextColor, for: .normal)
        timerLabel.isHidden = false
        updateProgressLayer(progress: 0, hidden: false)
        applyCameraButtonState()
        UIView.animate(withDuration: 0.2) {
            self.innerCircle.transform = CGAffineTransform(scaleX: 0.82, y: 0.82)
            self.cancelButton.alpha = 0.0
        }
    }

    func updateToFinishing() {
        hintLabel.text = "保存中..."
        timerLabel.isHidden = true
        applyCameraButtonState()
    }

    func updateToCancelling() {
        hintLabel.text = "取消中..."
        cancelButton.backgroundColor = cancelBaseColor
        cancelButton.setTitleColor(cancelNormalTextColor, for: .normal)
        timerLabel.isHidden = true
        applyCameraButtonState()
        cancelButton.alpha = 1.0
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
        updateProgressLayer(progress: CGFloat(min(max(elapsed / maxDuration, 0), 1)), hidden: !recordingActive)
        timerLabel.isHidden = !recordingActive
    }

    func updateFingerOutside(_ isOutside: Bool) {
        guard recordingActive else {
            cancelButton.backgroundColor = cancelBaseColor
            cancelButton.setTitleColor(cancelNormalTextColor, for: .normal)
            cancelButton.alpha = 1.0
            return
        }
        if isOutside {
            hintLabel.text = "松手取消"
            cancelButton.backgroundColor = cancelHighlightColor
            cancelButton.setTitleColor(cancelHighlightTextColor, for: .normal)
            if cancelButton.alpha < 1.0 {
                UIView.animate(withDuration: 0.15) {
                    self.cancelButton.alpha = 1.0
                }
            }
        } else {
            hintLabel.text = "松开停止"
            cancelButton.backgroundColor = cancelBaseColor
            cancelButton.setTitleColor(cancelNormalTextColor, for: .normal)
            if cancelButton.alpha > 0.0 {
                UIView.animate(withDuration: 0.15) {
                    self.cancelButton.alpha = 0.0
                }
            }
        }
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

    private func updateProgressLayer(progress: CGFloat, hidden: Bool) {
        progressLayer.strokeEnd = min(max(progress, 0), 1)
        progressLayer.isHidden = hidden
        progressBackgroundLayer.isHidden = hidden
    }

    private func configureView() {
        backgroundColor = .clear

        captureButton.translatesAutoresizingMaskIntoConstraints = false
        captureButton.backgroundColor = UIColor.white
        captureButton.layer.cornerRadius = 42
        captureButton.layer.borderColor = UIColor.black.withAlphaComponent(0.25).cgColor
        captureButton.layer.borderWidth = 4
        captureButton.addGestureRecognizer(longPressRecognizer)
        addSubview(captureButton)

        innerCircle.translatesAutoresizingMaskIntoConstraints = false
        innerCircle.backgroundColor = UIColor.white
        innerCircle.layer.cornerRadius = 30
        innerCircle.layer.masksToBounds = true
        captureButton.addSubview(innerCircle)

        progressBackgroundLayer.fillColor = UIColor.clear.cgColor
        progressBackgroundLayer.strokeColor = UIColor.white.withAlphaComponent(0.2).cgColor
        progressBackgroundLayer.lineWidth = 4
        progressBackgroundLayer.lineCap = .round
        progressBackgroundLayer.zPosition = 1
        captureButton.layer.addSublayer(progressBackgroundLayer)

        progressLayer.fillColor = UIColor.clear.cgColor
        progressLayer.strokeColor = UIColor.systemBlue.cgColor
        progressLayer.lineWidth = 4
        progressLayer.lineCap = .round
        progressLayer.strokeEnd = 0
        progressLayer.zPosition = 2
        captureButton.layer.addSublayer(progressLayer)

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
        cancelButton.layer.cornerRadius = 18
        cancelButton.layer.masksToBounds = false
        cancelButton.layer.borderWidth = 0
        cancelButton.setTitle("取消", for: .normal)
        cancelButton.setTitleColor(cancelNormalTextColor, for: .normal)
        cancelButton.titleLabel?.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        cancelButton.contentEdgeInsets = UIEdgeInsets(top: 6, left: 10, bottom: 6, right: 10)
        cancelButton.accessibilityLabel = "取消"
        cancelButton.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        addSubview(cancelButton)

        switchCameraButton.translatesAutoresizingMaskIntoConstraints = false
        switchCameraButton.backgroundColor = UIColor.black.withAlphaComponent(0.35)
        switchCameraButton.layer.cornerRadius = 22
        if let image = UIImage(systemName: "arrow.triangle.2.circlepath.camera") {
            switchCameraButton.setImage(image, for: .normal)
            switchCameraButton.tintColor = .white
        } else {
            switchCameraButton.setTitle("切换", for: .normal)
            switchCameraButton.setTitleColor(.white, for: .normal)
        }
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

            cancelButton.centerYAnchor.constraint(equalTo: captureButton.centerYAnchor),
            cancelButton.trailingAnchor.constraint(equalTo: captureButton.leadingAnchor, constant: -40),
            

            timerLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            timerLabel.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 18),

            switchCameraButton.widthAnchor.constraint(equalToConstant: 44),
            switchCameraButton.heightAnchor.constraint(equalToConstant: 44),
            switchCameraButton.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -16),
            switchCameraButton.topAnchor.constraint(equalTo: safeAreaLayoutGuide.topAnchor, constant: 16)
        ])

        updateToIdle()
    }

    private func updateProgressPath() {
        let radius = min(captureButton.bounds.width, captureButton.bounds.height) / 2 - 6
        let center = CGPoint(x: captureButton.bounds.midX, y: captureButton.bounds.midY)
        let path = UIBezierPath(arcCenter: center, radius: radius, startAngle: -.pi / 2, endAngle: 1.5 * .pi, clockwise: true)
        progressBackgroundLayer.path = path.cgPath
        progressLayer.path = path.cgPath
    }

    private static func format(time: TimeInterval, rounding rule: FloatingPointRoundingRule) -> String {
        let totalSeconds = max(Int(time.rounded(rule)), 0)
        let minutes = totalSeconds / 60
        let seconds = totalSeconds % 60
        return String(format: "%02d:%02d", minutes, seconds)
    }

    private func applyCameraButtonState() {
        guard cameraSwitchAvailable else {
            switchCameraButton.isEnabled = false
            switchCameraButton.alpha = 0.0
            return
        }
        switchCameraButton.isEnabled = !recordingActive
        switchCameraButton.alpha = recordingActive ? 0.5 : 1.0
    }

    @objc private func handleLongPress(_ recognizer: UILongPressGestureRecognizer) {
        let location = recognizer.location(in: captureButton)
        let isInside = captureButton.bounds.contains(location)

        switch recognizer.state {
        case .began:
            onLongPressChanged?(.began)
        case .changed:
            onLongPressChanged?(.changed(isInside: isInside))
        case .ended:
            onLongPressChanged?(.ended)
        case .cancelled, .failed:
            onLongPressChanged?(.cancelled)
        default:
            break
        }
    }
    
    @objc private func cancelTapped() {
        onCancelTapped?()
    }

    @objc private func switchCameraTapped() {
        onSwitchCameraTapped?()
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
