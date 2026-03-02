#if os(iOS)
import UIKit
import CLingXiaSwiftAPI
import CLingXiaRustAPI
import os.log

extension LxAppMedia {
    nonisolated(unsafe) private static let previewLog = OSLog(subsystem: "LingXia", category: "MediaPreview")

    // Strong reference to keep preview window alive
    @MainActor fileprivate static var previewWindow: UIWindow?

    struct PreviewMediaPayload: Decodable {
        let path: String
        let media_type: Int32
        let cover_path: String?
        let rotate: Int?
        let object_fit: String?

        private enum CodingKeys: String, CodingKey {
            case path
            case media_type
            case mediaType
            case cover_path
            case coverPath
            case rotate
            case rotation
            case object_fit
            case objectFit
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            path = try container.decode(String.self, forKey: .path)
            media_type = try (try? container.decode(Int32.self, forKey: .media_type))
                ?? container.decode(Int32.self, forKey: .mediaType)
            cover_path = (try? container.decodeIfPresent(String.self, forKey: .cover_path))
                ?? (try? container.decodeIfPresent(String.self, forKey: .coverPath))
            rotate = (try? container.decodeIfPresent(Int.self, forKey: .rotate))
                ?? (try? container.decodeIfPresent(Int.self, forKey: .rotation))
            object_fit = (try? container.decodeIfPresent(String.self, forKey: .object_fit))
                ?? (try? container.decodeIfPresent(String.self, forKey: .objectFit))
        }
    }

    nonisolated static func previewMedia(items_json: RustStr) -> Bool {
        let itemsJson = items_json.toString()
        guard let jsonData = itemsJson.data(using: .utf8) else {
            os_log(.error, log: previewLog, "Failed to convert items JSON to data")
            return false
        }

        let items: [PreviewMediaPayload]
        do {
            items = try JSONDecoder().decode([PreviewMediaPayload].self, from: jsonData)
        } catch {
            os_log(.error, log: previewLog, "Failed to decode items JSON: %{public}@", error.localizedDescription)
            return false
        }
        guard !items.isEmpty else {
            os_log(.error, log: previewLog, "previewMedia called with empty items")
            return false
        }

        // Dispatch to main actor for UI operations
        DispatchQueue.main.async {
            let previewItems = items.map { PreviewMediaItem(payload: $0) }
            let previewController = MediaPreviewViewController(items: previewItems)

            // Create a dedicated window for preview to avoid affecting the main app's orientation
            if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
                let window = UIWindow(windowScene: windowScene)
                window.windowLevel = .statusBar + 1  // Above status bar, same as native component fullscreen
                window.backgroundColor = .black
                window.rootViewController = previewController

                // Keep strong reference to prevent window from being deallocated
                Task { @MainActor in
                    Self.previewWindow = window
                }

                window.makeKeyAndVisible()
            }
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
    let rotate: Int?
    let objectFit: LxMediaObjectFit?

    init(payload: LxAppMedia.PreviewMediaPayload) {
        let pathString = payload.path
        if let parsed = URL(string: pathString), parsed.scheme != nil {
            self.url = parsed
        } else {
            self.url = URL(fileURLWithPath: pathString)
        }

        let coverString = payload.cover_path ?? ""
        if coverString.isEmpty {
            self.coverURL = nil
        } else if let cover = URL(string: coverString), cover.scheme != nil {
            self.coverURL = cover
        } else {
            self.coverURL = URL(fileURLWithPath: coverString)
        }
        self.type = MediaType(rawValue: payload.media_type)
        self.rotate = {
            guard let value = payload.rotate else { return nil }
            switch value {
            case 0, 90, 180, 270:
                return value
            default:
                return nil
            }
        }()
        self.objectFit = {
            guard let raw = payload.object_fit?.lowercased() else { return nil }
            return LxMediaObjectFit(rawValue: raw)
        }()
    }
}


private final class MediaPreviewViewController: UIViewController, UIGestureRecognizerDelegate {
    private let items: [PreviewMediaItem]
    private var currentIndex: Int
    private var didCleanup = false

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

    deinit {
        cleanupPreviewResources()
    }

    override var prefersStatusBarHidden: Bool {
        return true  // Hide status bar like native component fullscreen
    }

    override var preferredStatusBarStyle: UIStatusBarStyle {
        .lightContent
    }

    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        return .portrait
    }

    override var shouldAutorotate: Bool {
        return false
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        modalPresentationCapturesStatusBarAppearance = true
        setNeedsStatusBarAppearanceUpdate()  // Force status bar update

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

            if let backImage = LxIcon.image(named: "icon_close")?.withRenderingMode(.alwaysOriginal) {
            closeButton.setImage(backImage, for: .normal)
            closeButton.tintColor = .clear
        } else {
            closeButton.setTitle("Back", for: .normal)
            closeButton.setTitleColor(.white, for: .normal)
        }
        closeButton.addTarget(self, action: #selector(closeTapped), for: .touchUpInside)
        view.addSubview(closeButton)
        NSLayoutConstraint.activate([
            closeButton.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            closeButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            closeButton.widthAnchor.constraint(equalToConstant: 44),
            closeButton.heightAnchor.constraint(equalToConstant: 44)
        ])

        if let initial = viewController(for: currentIndex) {
            pageViewController.setViewControllers([initial], direction: .forward, animated: false)
        }
        setPagerInteraction(enabled: true)
        updateIndicator()
        updateCloseButtonVisibility()

        // Keep close button in sync with player controls via the same tap gesture.
        let tapGesture = UITapGestureRecognizer(target: self, action: #selector(handlePreviewTap))
        tapGesture.delegate = self
        tapGesture.cancelsTouchesInView = false
        view.addGestureRecognizer(tapGesture)
    }

    @objc private func handlePreviewTap() {
        guard items.indices.contains(currentIndex) else { return }
        let currentItem = items[currentIndex]
        if currentItem.type == .video {
            UIView.animate(withDuration: 0.3) {
                self.closeButton.alpha = self.closeButton.alpha > 0 ? 0 : 1
            }
        }
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
            }, dismissHandler: { [weak self] in self?.closeTapped() })
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

    private func updateCloseButtonVisibility() {
        // Hide close button for images (tap to dismiss)
        // For videos, also hide initially (will show with controls on tap)
        guard items.indices.contains(currentIndex) else { return }
        let currentItem = items[currentIndex]
        if currentItem.type == .video {
            closeButton.isHidden = false
            closeButton.alpha = 0
        } else {
            closeButton.isHidden = true
            closeButton.alpha = 0
        }
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        true
    }

    @objc private func closeTapped() {
        // Hide and clean up the dedicated window
        Task { @MainActor in
            cleanupPreviewResources()

            // Find the main app window and restore it as key window before dismissing
            if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
               let mainWindow = windowScene.windows.first(where: { $0 != LxAppMedia.previewWindow && $0.isKeyWindow == false }) {
                mainWindow.makeKeyAndVisible()
            }

            LxAppMedia.previewWindow?.isHidden = true
            LxAppMedia.previewWindow?.rootViewController = nil
            LxAppMedia.previewWindow = nil
        }
    }

    private func cleanupPreviewResources() {
        if didCleanup {
            return
        }
        didCleanup = true

        teardownPlayers(in: self)
        pageViewController.dataSource = nil
        pageViewController.delegate = nil
    }

    private func teardownPlayers(in controller: UIViewController) {
        if let videoController = controller as? MediaPreviewVideoController {
            videoController.teardownPlayer()
        }
        for child in controller.children {
            teardownPlayers(in: child)
        }
        if let presented = controller.presentedViewController {
            teardownPlayers(in: presented)
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
        updateCloseButtonVisibility()
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
        let view = ZoomableImageView(
            imageURL: item.url,
            rotateDegrees: item.rotate,
            objectFit: item.objectFit
        )
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
    private let log = OSLog(subsystem: "LingXia", category: "MediaPreview")

    private var player: LxMediaPlayer?
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

        // Embed LxMediaPlayer inline
        embedPlayerInline()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        // Let setFrame handle both view.frame and playerLayer.frame
        player?.setFrame(view.bounds)
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        // Pause when leaving (e.g., swiping to another page or dismissing)
        player?.handle(command: .pause)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Resume playback when returning to this page
        if hasStartedPlayback {
            player?.handle(command: .play)
        }
    }

    fileprivate func teardownPlayer() {
        player?.handle(command: .stop)
        player?.detach()
        player = nil
    }

    private func embedPlayerInline() {
        let config = LxMediaPlayerConfig(
            src: item.url,
            poster: item.coverURL,
            autoplay: true,
            controls: true,  // Show all controls
            showControlsOnInit: false,  // Hide controls initially, show on tap
            objectFit: item.objectFit ?? .contain,
            rotateDegrees: item.rotate
        )

        let player = LxMediaPlayer(eventHandler: { [weak self] event in
            switch event {
            case .play:
                self?.hasStartedPlayback = true
                os_log("MediaPreview player event: play", log: self?.log ?? .default, type: .info)
            case .error(let code, let message):
                os_log("Error: %{public}@ - %{public}@", log: self?.log ?? .default, type: .error, code, message)
            default:
                break
            }
        })

        player.update(config: config)
        // Don't show player's close button - using preview's custom close button
        player.setShowCloseButton(false)
        // Don't show fullscreen button - preview is already fullscreen
        player.setShowFullscreenButton(false)
        self.player = player

        // Add player view
        let playerView = player.view
        playerView.translatesAutoresizingMaskIntoConstraints = true
        view.addSubview(playerView)
    }

    private func startPlaybackIfNeeded() {
        // Autoplay is enabled in config, so playback starts automatically
        // This method kept for potential future use
        hasStartedPlayback = true
    }
}

private final class ZoomableImageView: UIView, UIScrollViewDelegate {
    let imageURL: URL
    private let rotateDegrees: Int?
    private let objectFit: LxMediaObjectFit?
    var onZoomStateChanged: ((Bool) -> Void)?
    var onDismiss: (() -> Void)?

    private let scrollView = UIScrollView()
    private let zoomContentView = UIView()
    private let imageView = UIImageView()
    private let activityIndicator = UIActivityIndicatorView(style: .large)

    init(imageURL: URL, rotateDegrees: Int?, objectFit: LxMediaObjectFit?) {
        self.imageURL = imageURL
        self.rotateDegrees = rotateDegrees
        self.objectFit = objectFit
        super.init(frame: .zero)
        configure()
        loadImage()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        applyImageRotationTransform()
        centerImageView()
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

        zoomContentView.translatesAutoresizingMaskIntoConstraints = false
        zoomContentView.backgroundColor = .clear
        scrollView.addSubview(zoomContentView)

        imageView.translatesAutoresizingMaskIntoConstraints = false
        imageView.contentMode = resolveImageContentMode()
        imageView.clipsToBounds = true
        zoomContentView.addSubview(imageView)

        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.hidesWhenStopped = true
        addSubview(activityIndicator)

        NSLayoutConstraint.activate([
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: topAnchor),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),

            zoomContentView.centerXAnchor.constraint(equalTo: scrollView.centerXAnchor),
            zoomContentView.centerYAnchor.constraint(equalTo: scrollView.centerYAnchor),
            zoomContentView.widthAnchor.constraint(equalTo: scrollView.widthAnchor),
            zoomContentView.heightAnchor.constraint(equalTo: scrollView.heightAnchor),

            imageView.leadingAnchor.constraint(equalTo: zoomContentView.leadingAnchor),
            imageView.trailingAnchor.constraint(equalTo: zoomContentView.trailingAnchor),
            imageView.topAnchor.constraint(equalTo: zoomContentView.topAnchor),
            imageView.bottomAnchor.constraint(equalTo: zoomContentView.bottomAnchor),
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
            let image: UIImage? = {
                if imageURL.isFileURL {
                    return UIImage(contentsOfFile: imageURL.path)
                }
                guard let data = try? Data(contentsOf: imageURL) else { return nil }
                return UIImage(data: data)
            }()

            await MainActor.run {
                activityIndicator.stopAnimating()
                if let image {
                    imageView.image = image
                    self.applyImageRotationTransform()
                    resetZoom()
                } else {
                    imageView.image = UIImage(systemName: "exclamationmark.triangle")
                    imageView.tintColor = .white
                    self.applyImageRotationTransform()
                }
            }
        }
    }

    private func resolveImageContentMode() -> UIView.ContentMode {
        guard let objectFit else { return .scaleAspectFit }
        switch objectFit {
        case .cover:
            return .scaleAspectFill
        case .fill:
            return .scaleToFill
        case .contain, .fit:
            return .scaleAspectFit
        @unknown default:
            return .scaleAspectFit
        }
    }

    private func normalizedRotationDegrees() -> Int? {
        guard let degrees = rotateDegrees else { return nil }
        let normalized = ((degrees % 360) + 360) % 360
        if normalized == 0 || normalized == 90 || normalized == 180 || normalized == 270 {
            return normalized
        }
        return nil
    }

    private func rotationScale(for degrees: Int) -> (x: CGFloat, y: CGFloat) {
        guard degrees == 90 || degrees == 270 else {
            return (1, 1)
        }
        let width = zoomContentView.bounds.width
        let height = zoomContentView.bounds.height
        guard width > 0, height > 0 else {
            return (1, 1)
        }

        let ratio1 = width / height
        let ratio2 = height / width
        switch objectFit ?? .contain {
        case .cover:
            let scale = max(ratio1, ratio2)
            return (scale, scale)
        case .fill:
            return (ratio1, ratio2)
        case .contain, .fit:
            let scale = min(ratio1, ratio2)
            return (scale, scale)
        @unknown default:
            let scale = min(ratio1, ratio2)
            return (scale, scale)
        }
    }

    private func applyImageRotationTransform() {
        let degrees = normalizedRotationDegrees() ?? 0
        let radians = CGFloat(degrees) * (.pi / 180)
        let scale = rotationScale(for: degrees)
        imageView.transform = CGAffineTransform(rotationAngle: radians).scaledBy(x: scale.x, y: scale.y)
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

        let pointInZoomContent = gesture.location(in: zoomContentView)
        zoom(to: targetScale, center: pointInZoomContent)
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
        zoomContentView
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
        zoomContentView.center = CGPoint(
            x: scrollView.contentSize.width * 0.5 + offsetX,
            y: scrollView.contentSize.height * 0.5 + offsetY
        )
    }

    private func notifyScaleState() {
        onZoomStateChanged?(scrollView.zoomScale > 1.05)
    }
}

#endif
