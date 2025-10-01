import Foundation
import os.log
import CLingXiaSwiftAPI

#if os(iOS)
import UIKit
import AVKit
import AVFoundation
#endif

@MainActor
enum LxAppMedia {
    nonisolated(unsafe) private static let log = OSLog(subsystem: "LingXia", category: "Media")

    struct PreviewMediaPayload: Codable {
        let path: String
        let media_type: Int32
        let cover_url: String?
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

        let coverString = payload.cover_url ?? ""
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
#endif
