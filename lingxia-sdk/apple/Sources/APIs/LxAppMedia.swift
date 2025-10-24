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
            let captureMode: CameraDelegate.CaptureMode = modeLowercased == "video" ? .video : .photo
            let picker = UIImagePickerController()
            picker.sourceType = .camera
            picker.allowsEditing = false
            picker.modalPresentationStyle = .fullScreen

            if #available(iOS 14.0, *) {
                switch captureMode {
                case .video:
                    picker.mediaTypes = [UTType.movie.identifier]
                case .photo:
                    picker.mediaTypes = [UTType.image.identifier]
                }
            } else {
                switch captureMode {
                case .video:
                    picker.mediaTypes = ["public.movie"]
                case .photo:
                    picker.mediaTypes = ["public.image"]
                }
            }

            switch captureMode {
            case .video:
                picker.cameraCaptureMode = .video
                if let durationValue = Double(maxDuration), durationValue > 0 {
                    picker.videoMaximumDuration = TimeInterval(durationValue)
                }
                picker.videoQuality = .typeHigh
            case .photo:
                picker.cameraCaptureMode = .photo
            }

            let desiredFacing = cameraFacing.lowercased() == "front"
                ? UIImagePickerController.CameraDevice.front
                : UIImagePickerController.CameraDevice.rear

            if UIImagePickerController.isCameraDeviceAvailable(desiredFacing) {
                picker.cameraDevice = desiredFacing
            } else if UIImagePickerController.isCameraDeviceAvailable(.rear) {
                picker.cameraDevice = .rear
            }

            let delegate = CameraDelegate(callbackId: callbackId, captureMode: captureMode) {
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
                        "uri": tempURL.path,
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
                        "uri": tempURL.path,
                        "fileType": "image",
                        "isOriginal": true
                    ]
                }
            } else if let image = info[.originalImage] as? UIImage {
                if let tempURL = saveCapturedImageToTemp(image) {
                    jsonItem = [
                        "uri": tempURL.path,
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
                            "uri": tempURL.path,
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
                            "uri": tempURL.path,
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
