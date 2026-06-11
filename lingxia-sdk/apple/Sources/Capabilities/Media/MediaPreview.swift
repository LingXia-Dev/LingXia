#if os(iOS)
import UIKit
import CLingXiaSwiftAPI
import CLingXiaRustAPI
import os.log

extension LxAppMedia {
    nonisolated private static let previewLog = OSLog(subsystem: "LingXia", category: "MediaPreview")

    // Strong reference to keep preview window alive
    @MainActor fileprivate static var previewWindow: UIWindow?
    @MainActor fileprivate static var activePreviewController: MediaPreviewViewController?

    struct PreviewMediaPayload: Decodable {
        let path: String
        let media_type: Int32
        let rotate: Int?
        let object_fit: String?
        let durationMs: UInt64?
    }

    struct PreviewMediaRequestPayload: Decodable {
        let sources: [PreviewMediaPayload]
        let startIndex: Int
        let advance: String
        let showIndexIndicator: Bool
    }

    nonisolated static func previewMedia(items_json: RustStr, callback_id: UInt64, presented_callback_id: UInt64, change_callback_id: UInt64) -> Bool {
        let itemsJson = items_json.toString()
        guard let jsonData = itemsJson.data(using: .utf8) else {
            os_log(.error, log: previewLog, "Failed to convert items JSON to data")
            return false
        }

        let request: PreviewMediaRequestPayload
        do {
            request = try JSONDecoder().decode(PreviewMediaRequestPayload.self, from: jsonData)
        } catch {
            os_log(.error, log: previewLog, "Failed to decode items JSON: %{public}@", error.localizedDescription)
            return false
        }
        guard !request.sources.isEmpty else {
            os_log(.error, log: previewLog, "previewMedia called with empty items")
            return false
        }

        if Thread.isMainThread {
            return MainActor.assumeIsolated {
                previewMediaOnMain(request: request, callbackId: callback_id, presentedCallbackId: presented_callback_id, changeCallbackId: change_callback_id)
            }
        }
        var started = false
        DispatchQueue.main.sync {
            started = previewMediaOnMain(request: request, callbackId: callback_id, presentedCallbackId: presented_callback_id, changeCallbackId: change_callback_id)
        }
        return started
    }

    @MainActor
    private static func previewMediaOnMain(request: PreviewMediaRequestPayload, callbackId: UInt64, presentedCallbackId: UInt64, changeCallbackId: UInt64) -> Bool {
        guard let windowScene = UIApplication.shared.connectedScenes
            .compactMap({ $0 as? UIWindowScene })
            .first(where: { $0.activationState == .foregroundActive })
            ?? UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first
        else {
            os_log(.error, log: previewLog, "No active window scene for previewMedia")
            return false
        }

        let previewItems = request.sources.map { PreviewMediaItem(payload: $0) }
        let previewController = MediaPreviewViewController(
            items: previewItems,
            startIndex: request.startIndex,
            callbackId: callbackId,
            presentedCallbackId: presentedCallbackId,
            changeCallbackId: changeCallbackId,
            advance: PreviewMediaAdvance(rawValue: request.advance),
            showIndexIndicator: request.showIndexIndicator
        )

        activePreviewController?.finishPreview(reason: .interrupted)

        let window = UIWindow(windowScene: windowScene)
        window.frame = windowScene.coordinateSpace.bounds
        window.windowLevel = .statusBar + 1
        window.backgroundColor = .black
        window.alpha = 0
        window.rootViewController = previewController
        previewWindow = window
        activePreviewController = previewController
        previewController.onInitialContentReady = { [weak previewController, weak window] in
            guard let previewController, activePreviewController === previewController else { return }
            previewController.beginPreviewWindowPresentation(window)
        }
        previewController.loadViewIfNeeded()
        return true
    }

    nonisolated static func cancelPreview(callback_id: UInt64) -> Bool {
        if Thread.isMainThread {
            return MainActor.assumeIsolated {
                cancelPreviewOnMain(callbackId: callback_id)
            }
        }
        var cancelled = false
        DispatchQueue.main.sync {
            cancelled = cancelPreviewOnMain(callbackId: callback_id)
        }
        return cancelled
    }

    @MainActor
    private static func cancelPreviewOnMain(callbackId: UInt64) -> Bool {
        guard let controller = activePreviewController, controller.callbackId == callbackId else {
            return false
        }
        controller.finishPreview(reason: .interrupted)
        return true
    }

    @MainActor
    fileprivate static func dismissPreviewWindow(for controller: MediaPreviewViewController) {
        if activePreviewController === controller {
            activePreviewController = nil
        }
        if previewWindow?.rootViewController === controller {
            let window = previewWindow
            UIView.animate(
                withDuration: 0.16,
                delay: 0,
                options: [.curveEaseOut, .beginFromCurrentState]
            ) {
                window?.alpha = 0
            } completion: { _ in
                controller.cleanupPreviewResources()
                if previewWindow === window {
                    makeMainWindowKeyIfNeeded(excluding: window)
                }
                window?.isHidden = true
                window?.rootViewController = nil
                if previewWindow === window {
                    previewWindow = nil
                }
            }
        }
    }

    @MainActor
    private static func makeMainWindowKeyIfNeeded(excluding excludedWindow: UIWindow?) {
        guard let windowScene = excludedWindow?.windowScene
            ?? UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else {
            return
        }
        if let mainWindow = windowScene.windows.first(where: { $0 !== excludedWindow }) {
            mainWindow.makeKeyAndVisible()
        }
    }
}

private enum PreviewMediaAdvance {
    case manual
    case next
    case loop

    init(rawValue: String?) {
        switch rawValue?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "next":
            self = .next
        case "loop":
            self = .loop
        default:
            self = .manual
        }
    }
}

private enum PreviewMediaCloseReason: String {
    case manual
    case completed
    case interrupted
    case error
}

private enum PreviewSwipeDirection {
    case previous
    case next
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
    let rotate: Int?
    let objectFit: LxMediaObjectFit?
    let durationMs: UInt64?
    var requiresFirstFrameBeforeDisplay: Bool {
        type == .video && url.isFileURL
    }

    init(payload: LxAppMedia.PreviewMediaPayload) {
        self.url = Self.resolveURL(payload.path)

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
        self.durationMs = payload.durationMs
    }

    // Not @MainActor: only invokes pure FFI helpers (`getCurrentLxApp` /
    // `resolveLxUri`) and string transforms. Keeping it actor-free lets the
    // synchronous `init(payload:)` (also actor-free) construct items from any
    // context — the previous @MainActor annotation produced an
    // ActorIsolatedCall error on every call site.
    private static func resolveURL(_ pathString: String) -> URL {
        let raw = pathString.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.hasPrefix("http://") || raw.hasPrefix("https://") {
            return URL(string: raw) ?? URL(fileURLWithPath: raw)
        }

        let current = getCurrentLxApp()
        let appId = current.appid.toString()
        let resolved = resolveLxUri(appId, raw)?.toString() ?? raw

        if let filePath = resolved.strippingFileScheme(), filePath.hasPrefix("/") {
            return URL(fileURLWithPath: filePath)
        }
        if resolved.hasPrefix("/") {
            return URL(fileURLWithPath: resolved)
        }
        if let parsed = URL(string: resolved), parsed.scheme != nil {
            return parsed
        }
        return URL(fileURLWithPath: resolved)
    }
}

private extension String {
    /// If self starts with "file://", returns the path remainder; otherwise nil.
    /// Duplicated from MediaSwiperComponent.swift (which scopes the same helper
    /// private to its own file). Worth folding into a shared util if a third
    /// caller shows up.
    func strippingFileScheme() -> String? {
        guard self.hasPrefix("file://") else { return nil }
        return String(self.dropFirst("file://".count))
    }
}

@MainActor
private final class MediaPreviewViewController: UIViewController, UIGestureRecognizerDelegate {
    private let items: [PreviewMediaItem]
    fileprivate let callbackId: UInt64
    /// JS-side `presented` Promise callback id; fired exactly once when the
    /// first item becomes visually ready. Zero once fired, to keep the call
    /// idempotent across multiple visual-ready paths.
    private var presentedCallbackId: UInt64
    /// JS-side change-stream callback id; fired with `{"index": N}` whenever
    /// the displayed item changes (including the initial item). Zero disables.
    private let changeCallbackId: UInt64
    private var lastNotifiedIndex: Int = -1
    private let advance: PreviewMediaAdvance
    private var currentIndex: Int
    private var didCleanup = false
    private var didFinish = false
    private var imageTimer: Timer?
    private var suppressVideoEndedUntil: CFTimeInterval = 0
    private var currentController: (UIViewController & IndexedPreviewController)?
    private var isCurrentImageZoomed = false
    private var isTransitioning = false
    private var previewTransitionGeneration: UInt64 = 0
    private let showIndexIndicator: Bool
    private var initialVisualReady = false
    private var previewWindowPresentationStarted = false
    private var previewWindowVisible = false
    private weak var previewWindowForPresentation: UIWindow?
    fileprivate var onInitialContentReady: (() -> Void)?

    private lazy var closeButton: UIButton = {
        let button = UIButton(type: .system)
        button.translatesAutoresizingMaskIntoConstraints = false
        // Circular semi-transparent chip so the X is legible against any
        // video frame, not just dark ones.
        button.backgroundColor = UIColor.black.withAlphaComponent(0.45)
        button.tintColor = .white
        button.layer.cornerRadius = 18
        button.layer.masksToBounds = true
        button.contentEdgeInsets = .zero
        return button
    }()

    private let contentContainer: UIView = {
        let view = UIView()
        view.translatesAutoresizingMaskIntoConstraints = false
        view.backgroundColor = .black
        view.clipsToBounds = true
        return view
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

    private lazy var previewTapGesture: UITapGestureRecognizer = {
        let gesture = UITapGestureRecognizer(target: self, action: #selector(handlePreviewTap))
        gesture.delegate = self
        gesture.cancelsTouchesInView = false
        return gesture
    }()

    private lazy var previousEdgePanGesture: UIScreenEdgePanGestureRecognizer = {
        let gesture = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(handleEdgePan(_:)))
        gesture.edges = .left
        gesture.delegate = self
        gesture.cancelsTouchesInView = false
        return gesture
    }()

    private lazy var nextEdgePanGesture: UIScreenEdgePanGestureRecognizer = {
        let gesture = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(handleEdgePan(_:)))
        gesture.edges = .right
        gesture.delegate = self
        gesture.cancelsTouchesInView = false
        return gesture
    }()

    init(
        items: [PreviewMediaItem],
        startIndex: Int = 0,
        callbackId: UInt64,
        presentedCallbackId: UInt64,
        changeCallbackId: UInt64,
        advance: PreviewMediaAdvance,
        showIndexIndicator: Bool
    ) {
        self.items = items
        self.currentIndex = max(0, min(startIndex, items.count - 1))
        self.callbackId = callbackId
        self.presentedCallbackId = presentedCallbackId
        self.changeCallbackId = changeCallbackId
        self.advance = advance
        self.showIndexIndicator = showIndexIndicator
        super.init(nibName: nil, bundle: nil)
    }

    /// Fire the JS-side `presented` Promise exactly once. Safe to call
    /// multiple times — second call is a no-op.
    fileprivate func signalPresentedOnce() {
        let id = presentedCallbackId
        if id == 0 { return }
        presentedCallbackId = 0
        let _ = onCallback(id, true, "{}")
    }

    /// Notify the JS-side change stream that `currentIndex` is now on screen.
    /// Dedupes consecutive fires for the same index.
    fileprivate func notifyIndexChanged() {
        guard changeCallbackId != 0, currentIndex != lastNotifiedIndex else { return }
        lastNotifiedIndex = currentIndex
        let _ = onCallback(changeCallbackId, true, "{\"index\":\(currentIndex)}")
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        if !didFinish {
            didFinish = true
            // Settle `presented` first in case it never fired (degenerate
            // path: deinit before any item rendered). Idempotent — no-op
            // if already settled. Inlined because `signalPresentedOnce()`
            // is @MainActor-isolated and unreachable from nonisolated deinit.
            if presentedCallbackId != 0 {
                let pid = presentedCallbackId
                presentedCallbackId = 0
                let _ = onCallback(pid, true, "{}")
            }
            if callbackId > 0,
               let jsonData = try? JSONSerialization.data(
                withJSONObject: [
                    "reason": PreviewMediaCloseReason.interrupted.rawValue,
                    "lastIndex": currentIndex
                ],
                options: []
               ),
               let jsonString = String(data: jsonData, encoding: .utf8) {
                let _ = onCallback(callbackId, true, jsonString)
            }
        }
    }

    override var prefersStatusBarHidden: Bool {
        return true
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
        setNeedsStatusBarAppearanceUpdate()

        view.addSubview(contentContainer)

        NSLayoutConstraint.activate([
            contentContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            contentContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            contentContainer.topAnchor.constraint(equalTo: view.topAnchor),
            contentContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        view.addSubview(indicatorLabel)
        NSLayoutConstraint.activate([
            indicatorLabel.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            indicatorLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor)
        ])

        // SF Symbol "xmark" is the iOS-native close glyph; the rendering-mode
        // override keeps the button's `tintColor` in effect (avoids the
        // "alwaysOriginal" template path that previously zeroed tintColor).
        if #available(iOS 13.0, *) {
            let symbolConfig = UIImage.SymbolConfiguration(pointSize: 14, weight: .semibold)
            let xmark = UIImage(systemName: "xmark", withConfiguration: symbolConfig)?
                .withRenderingMode(.alwaysTemplate)
            closeButton.setImage(xmark, for: .normal)
        } else if let backImage = LxIcon.image(named: "icon_close")?.withRenderingMode(.alwaysTemplate) {
            closeButton.setImage(backImage, for: .normal)
        } else {
            closeButton.setTitle("✕", for: .normal)
            closeButton.setTitleColor(.white, for: .normal)
            closeButton.titleLabel?.font = UIFont.systemFont(ofSize: 16, weight: .semibold)
        }
        closeButton.addTarget(self, action: #selector(closeTapped), for: .touchUpInside)
        view.addSubview(closeButton)
        NSLayoutConstraint.activate([
            closeButton.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -12),
            closeButton.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 2),
            closeButton.widthAnchor.constraint(equalToConstant: 36),
            closeButton.heightAnchor.constraint(equalToConstant: 36)
        ])

        view.addGestureRecognizer(previewTapGesture)
        view.addGestureRecognizer(previousEdgePanGesture)
        view.addGestureRecognizer(nextEdgePanGesture)

        displayInitialControllerWhenReady(index: currentIndex)
        updateManualNavigationGestures()
        updateIndicator()
        updateCloseButtonVisibility()
        scheduleBehaviorForCurrentItem()
        prefetchVideoFramesAroundCurrentIndex()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        currentController?.view.frame = contentContainer.bounds
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

    private func viewController(
        for index: Int,
        firstFrame: UIImage? = nil
    ) -> (UIViewController & IndexedPreviewController)? {
        guard items.indices.contains(index) else { return nil }
        let item = items[index]
        switch item.type {
        case .video:
            return MediaPreviewVideoController(
                item: item,
                index: index,
                firstFrame: firstFrame,
                loopPlayback: shouldLoopCurrentVideo(at: index),
                visualReadyHandler: { [weak self] readyIndex in
                    guard let self, self.currentIndex == readyIndex else { return }
                    self.markInitialVisualReady()
                },
                endedHandler: { [weak self] in
                    guard let self, self.currentIndex == index else { return }
                    self.handleVideoEnded()
                },
                errorHandler: { [weak self] in
                    guard let self, self.currentIndex == index else { return }
                    self.finishPreview(reason: .error)
                },
                scrubStateChanged: { [weak self] scrubbing in
                    guard let self, self.currentIndex == index else { return }
                    self.handleVideoScrubStateChanged(scrubbing)
                }
            )
        case .image, .unknown:
            return MediaPreviewImageController(item: item, index: index, zoomStateChanged: { [weak self] zoomed in
                self?.isCurrentImageZoomed = zoomed
                self?.updateManualNavigationGestures()
            }, visualReadyHandler: { [weak self] readyIndex in
                guard let self, self.currentIndex == readyIndex else { return }
                self.markInitialVisualReady()
            }, dismissHandler: { [weak self] in self?.finishPreview(reason: .manual) })
        }
    }

    private func shouldLoopCurrentVideo(at index: Int) -> Bool {
        return advance == .loop && items.count == 1 && items.indices.contains(index) && items[index].type == .video
    }

    private func displayInitialController(_ controller: UIViewController & IndexedPreviewController) {
        controller.beginAppearanceTransition(true, animated: false)
        addChild(controller)
        view.layoutIfNeeded()
        contentContainer.layoutIfNeeded()
        controller.view.frame = contentContainer.bounds
        controller.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        contentContainer.addSubview(controller.view)
        controller.view.layoutIfNeeded()
        controller.didMove(toParent: self)
        controller.endAppearanceTransition()
        currentController = controller
        isCurrentImageZoomed = false
        bootstrapControllerPresentation(controller)
    }

    /// Decide how aggressively to start the window-present animation when a
    /// controller is first attached. Three regimes:
    ///   - Image controllers: do nothing now; the image's own visualReady
    ///     callback will drive both the window fade-in and the `presented`
    ///     signal. We don't fade in over a still-loading image so the user
    ///     never sees an empty window appear before content.
    ///   - Video with embedded poster: poster is already on screen, so
    ///     fast-path the visual-ready signal — window animates and
    ///     `presented` fires right after it completes.
    ///   - Video without poster: AVPlayer needs the window in the hierarchy
    ///     (alpha-0 is fine) to reach viewDidAppear. Kick off only the
    ///     window animation; `presented` still waits for the `.playing`
    ///     event from the player.
    private func bootstrapControllerPresentation(_ controller: UIViewController) {
        guard let videoController = controller as? MediaPreviewVideoController else {
            return
        }
        if videoController.hasFirstFramePoster {
            markInitialVisualReady()
        } else {
            notifyInitialContentReady()
        }
    }

    private func notifyInitialContentReady() {
        guard let onInitialContentReady else { return }
        self.onInitialContentReady = nil
        onInitialContentReady()
    }

    fileprivate func beginPreviewWindowPresentation(_ window: UIWindow?) {
        if !previewWindowPresentationStarted {
            previewWindowPresentationStarted = true
            previewWindowForPresentation = window
            window?.makeKeyAndVisible()
            window?.layoutIfNeeded()
            view.layoutIfNeeded()
        }
        animatePreviewWindowIfReady()
    }

    fileprivate func markPreviewWindowVisible() {
        previewWindowVisible = true
        signalPresentedIfReady()
    }

    fileprivate func markInitialVisualReady() {
        if !initialVisualReady {
            initialVisualReady = true
        }
        notifyInitialContentReady()
        animatePreviewWindowIfReady()
        signalPresentedIfReady()
    }

    private func signalPresentedIfReady() {
        guard previewWindowVisible && initialVisualReady else { return }
        signalPresentedOnce()
    }

    private func animatePreviewWindowIfReady() {
        guard initialVisualReady, !previewWindowVisible else { return }
        guard let window = previewWindowForPresentation else { return }
        window.layoutIfNeeded()
        view.layoutIfNeeded()
        DispatchQueue.main.async { [weak self, weak window] in
            guard let self, let window else { return }
            guard self.initialVisualReady, !self.previewWindowVisible else { return }
            window.layoutIfNeeded()
            self.view.layoutIfNeeded()
            UIView.animate(
                withDuration: 0.12,
                delay: 0,
                options: [.curveEaseOut, .beginFromCurrentState]
            ) {
                window.alpha = 1
            } completion: { [weak self] _ in
                self?.markPreviewWindowVisible()
            }
        }
    }

    private func displayInitialControllerWhenReady(index: Int) {
        guard items.indices.contains(index) else { return }
        loadFirstFrameIfNeeded(for: index) { [weak self] image in
            guard let self else { return }
            guard self.currentController == nil, self.currentIndex == index else { return }
            guard !self.items[index].requiresFirstFrameBeforeDisplay || image != nil else {
                self.finishPreview(reason: .error)
                return
            }
            guard let initial = self.viewController(for: index, firstFrame: image) else { return }
            self.displayInitialController(initial)
            self.notifyIndexChanged()
        }
    }

    private func updateManualNavigationGestures() {
        guard !isTransitioning, items.indices.contains(currentIndex), items.count > 1 else {
            previousEdgePanGesture.isEnabled = false
            nextEdgePanGesture.isEnabled = false
            return
        }
        let item = items[currentIndex]
        let enabled = item.type == .video || !isCurrentImageZoomed
        previousEdgePanGesture.isEnabled = enabled
        nextEdgePanGesture.isEnabled = enabled
    }

    private func showItem(
        at index: Int,
        direction: PreviewSwipeDirection,
        animated: Bool
    ) {
        guard items.indices.contains(index) else { return }
        previewTransitionGeneration &+= 1
        let generation = previewTransitionGeneration
        loadFirstFrameIfNeeded(for: index) { [weak self] image in
            guard let self else { return }
            guard generation == self.previewTransitionGeneration else { return }
            guard !self.items[index].requiresFirstFrameBeforeDisplay || image != nil else {
                self.finishPreview(reason: .error)
                return
            }
            guard let controller = self.viewController(for: index, firstFrame: image) else { return }
            self.suppressVideoEndedUntil = 0
            self.isCurrentImageZoomed = false
            self.currentIndex = index
            self.notifyIndexChanged()
            self.updateIndicator()
            self.updateCloseButtonVisibility()
            self.transition(to: controller, direction: direction, animated: animated)
            self.scheduleBehaviorForCurrentItem()
            self.prefetchVideoFramesAroundCurrentIndex()
        }
    }

    private func loadFirstFrameIfNeeded(for index: Int, completion: @MainActor @escaping (UIImage?) -> Void) {
        guard items.indices.contains(index), items[index].type == .video else {
            completion(nil)
            return
        }
        if let cached = LxMediaFrameCache.shared.peek(for: items[index].url) {
            completion(cached)
            return
        }
        LxMediaFrameCache.shared.load(items[index].url, completion: completion)
    }

    /// Warm `LxMediaFrameCache` with first frames of current + adjacent video
    /// items so the first page and next swipe can synchronously show a still
    /// frame instead of the player's black background while AVPlayer initializes.
    private func prefetchVideoFramesAroundCurrentIndex() {
        guard !items.isEmpty else { return }
        let n = items.count
        var indices: Set<Int> = [currentIndex]
        let prev = currentIndex - 1
        if prev >= 0 {
            indices.insert(prev)
        } else if advance == .loop {
            indices.insert(n - 1)
        }
        let next = currentIndex + 1
        if next < n {
            indices.insert(next)
        } else if advance == .loop {
            indices.insert(0)
        }
        for idx in indices where items[idx].type == .video {
            LxMediaFrameCache.shared.prefetch(items[idx].url)
        }
    }

    private func transition(
        to controller: UIViewController & IndexedPreviewController,
        direction: PreviewSwipeDirection,
        animated: Bool
    ) {
        let previousController = currentController
        currentController = controller
        updateManualNavigationGestures()

        addChild(controller)
        let bounds = contentContainer.bounds
        controller.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]

        guard animated, let previousController, previousController !== controller else {
            previousController?.beginAppearanceTransition(false, animated: false)
            controller.beginAppearanceTransition(true, animated: false)
            previousController?.willMove(toParent: nil)
            previousController?.view.removeFromSuperview()
            previousController?.removeFromParent()
            controller.view.frame = bounds
            contentContainer.addSubview(controller.view)
            controller.didMove(toParent: self)
            previousController?.endAppearanceTransition()
            controller.endAppearanceTransition()
            if let previousController {
                teardownPlayers(in: previousController)
            }
            return
        }

        isTransitioning = true
        updateManualNavigationGestures()

        let width = max(bounds.width, 1)
        let offset = direction == .next ? width : -width
        let enteringFrame = bounds.offsetBy(dx: offset, dy: 0)
        let exitingFrame = bounds.offsetBy(dx: -offset, dy: 0)

        previousController.beginAppearanceTransition(false, animated: true)
        controller.beginAppearanceTransition(true, animated: true)
        controller.view.frame = enteringFrame
        contentContainer.addSubview(controller.view)
        previousController.willMove(toParent: nil)

        UIView.animate(
            withDuration: 0.28,
            delay: 0,
            options: [.curveEaseInOut, .allowUserInteraction]
        ) {
            previousController.view.frame = exitingFrame
            controller.view.frame = bounds
        } completion: { [weak self] _ in
            guard let self else { return }
            previousController.view.removeFromSuperview()
            previousController.removeFromParent()
            controller.didMove(toParent: self)
            previousController.endAppearanceTransition()
            controller.endAppearanceTransition()
            self.teardownPlayers(in: previousController)
            self.isTransitioning = false
            self.updateManualNavigationGestures()
        }
    }

    private func resolvedSwipeTargetIndex(for direction: PreviewSwipeDirection) -> Int? {
        switch direction {
        case .next:
            let nextIndex = currentIndex + 1
            if items.indices.contains(nextIndex) {
                return nextIndex
            }
            return advance == .loop && items.count > 1 ? 0 : nil
        case .previous:
            let previousIndex = currentIndex - 1
            if items.indices.contains(previousIndex) {
                return previousIndex
            }
            return advance == .loop && items.count > 1 ? items.count - 1 : nil
        }
    }

    @objc private func handleEdgePan(_ gesture: UIScreenEdgePanGestureRecognizer) {
        guard gesture.state == .ended else { return }

        let translation = gesture.translation(in: view)
        let direction: PreviewSwipeDirection
        switch gesture.edges {
        case .left:
            guard translation.x > 40 else { return }
            direction = .previous
        case .right:
            guard translation.x < -40 else { return }
            direction = .next
        default:
            return
        }

        guard let targetIndex = resolvedSwipeTargetIndex(for: direction) else { return }
        showItem(at: targetIndex, direction: direction, animated: true)
    }

    private func handleVideoScrubStateChanged(_ scrubbing: Bool) {
        if !scrubbing {
            suppressVideoEndedUntil = CACurrentMediaTime() + 0.8
        }
    }

    private func updateIndicator() {
        if !showIndexIndicator || items.isEmpty {
            indicatorLabel.isHidden = true
        } else {
            indicatorLabel.isHidden = false
            indicatorLabel.text = "\(currentIndex + 1)/\(items.count)"
        }
    }

    private func updateCloseButtonVisibility() {
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

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if gestureRecognizer === previewTapGesture {
            // Only suppress the gesture for taps that originate inside the
            // preview's own close button — letting it fire for taps on the
            // video page (including the player's tap-catcher UIButton, which
            // shows the player's controls overlay) is what makes our custom
            // close button toggle visible together with playback controls.
            return !touchOriginatesFromCloseButton(touch.view)
        }
        if gestureRecognizer === previousEdgePanGesture || gestureRecognizer === nextEdgePanGesture {
            guard !isTransitioning, items.indices.contains(currentIndex), items.count > 1 else { return false }
            if items[currentIndex].type != .video && isCurrentImageZoomed {
                return false
            }
            if let videoController = currentController as? MediaPreviewVideoController, videoController.isScrubbing {
                return false
            }
            return true
        }
        return true
    }

    private func touchOriginatesFromCloseButton(_ view: UIView?) -> Bool {
        var current = view
        while let candidate = current {
            if candidate === closeButton {
                return true
            }
            current = candidate.superview
        }
        return false
    }

    @objc private func closeTapped() {
        finishPreview(reason: .manual)
    }

    fileprivate func cleanupPreviewResources() {
        if didCleanup {
            return
        }
        didCleanup = true
        clearTimers()

        teardownPlayers(in: self)
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

    private func clearTimers() {
        imageTimer?.invalidate()
        imageTimer = nil
    }

    private func scheduleBehaviorForCurrentItem() {
        clearTimers()
        guard advance != .manual, items.indices.contains(currentIndex) else { return }
        let item = items[currentIndex]
        guard item.type != .video, let durationMs = item.durationMs, durationMs > 0 else {
            return
        }
        imageTimer = Timer.scheduledTimer(withTimeInterval: TimeInterval(durationMs) / 1000.0, repeats: false) { [weak self] _ in
            self?.advanceFromCurrentItem()
        }
    }

    private func handleVideoEnded() {
        if CACurrentMediaTime() < suppressVideoEndedUntil {
            return
        }
        // Suppress auto-advance while the user is scrubbing the progress bar.
        if let controller = currentController as? MediaPreviewVideoController,
           controller.index == currentIndex,
           controller.isScrubbing {
            return
        }
        clearTimers()
        advanceFromCurrentItem()
    }

    private func advanceFromCurrentItem() {
        switch advance {
        case .manual:
            return
        case .next:
            let nextIndex = currentIndex + 1
            guard items.indices.contains(nextIndex) else {
                finishPreview(reason: .completed)
                return
            }
            showItem(at: nextIndex, direction: .next, animated: true)
            return
        case .loop:
            guard !items.isEmpty else {
                finishPreview(reason: .completed)
                return
            }
            if items.count == 1 {
                scheduleBehaviorForCurrentItem()
                return
            }
            let nextIndex = currentIndex < items.count - 1 ? currentIndex + 1 : 0
            showItem(at: nextIndex, direction: .next, animated: true)
        }
    }
    fileprivate func finishPreview(reason: PreviewMediaCloseReason) {
        if didFinish {
            return
        }
        didFinish = true
        signalPresentedOnce()
        if callbackId > 0,
           let jsonData = try? JSONSerialization.data(
            withJSONObject: [
                "reason": reason.rawValue,
                "lastIndex": currentIndex
            ],
            options: []
           ),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            let _ = onCallback(callbackId, true, jsonString)
        }
        LxAppMedia.dismissPreviewWindow(for: self)
    }
}

private protocol IndexedPreviewController where Self: UIViewController {
    var index: Int { get }
}

private final class MediaPreviewImageController: UIViewController, IndexedPreviewController {
    let index: Int
    private let item: PreviewMediaItem
    private let zoomStateChanged: (Bool) -> Void
    private let visualReadyHandler: (Int) -> Void
    private let dismissHandler: () -> Void

    private lazy var zoomView: ZoomableImageView = {
        let view = ZoomableImageView(
            imageURL: item.url,
            rotateDegrees: item.rotate,
            objectFit: item.objectFit
        )
        view.translatesAutoresizingMaskIntoConstraints = false
        view.onZoomStateChanged = zoomStateChanged
        view.onVisualReady = { [weak self] in
            guard let self else { return }
            self.visualReadyHandler(self.index)
        }
        view.onDismiss = dismissHandler
        return view
    }()

    init(
        item: PreviewMediaItem,
        index: Int,
        zoomStateChanged: @escaping (Bool) -> Void,
        visualReadyHandler: @escaping (Int) -> Void,
        dismissHandler: @escaping () -> Void
    ) {
        self.item = item
        self.zoomStateChanged = zoomStateChanged
        self.visualReadyHandler = visualReadyHandler
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
    private let firstFrame: UIImage?
    private let loopPlayback: Bool
    private let visualReadyHandler: (Int) -> Void
    private let endedHandler: () -> Void
    private let errorHandler: () -> Void
    private let scrubStateChanged: (Bool) -> Void
    private let log = OSLog(subsystem: "LingXia", category: "MediaPreview")

    private var player: LxMediaPlayer?
    private let firstFrameImageView = UIImageView()
    private var hasStartedPlayback = false
    private var didSignalVisualReady = false
    fileprivate private(set) var isScrubbing = false

    fileprivate var hasFirstFramePoster: Bool {
        firstFrame != nil
    }

    init(
        item: PreviewMediaItem,
        index: Int,
        firstFrame: UIImage?,
        loopPlayback: Bool,
        visualReadyHandler: @escaping (Int) -> Void,
        endedHandler: @escaping () -> Void,
        errorHandler: @escaping () -> Void,
        scrubStateChanged: @escaping (Bool) -> Void
    ) {
        self.item = item
        self.index = index
        self.firstFrame = firstFrame
        self.loopPlayback = loopPlayback
        self.visualReadyHandler = visualReadyHandler
        self.endedHandler = endedHandler
        self.errorHandler = errorHandler
        self.scrubStateChanged = scrubStateChanged
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configureFirstFrameImageView()
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
        player?.refreshGestureInterference()
        if player == nil {
            DispatchQueue.main.async { [weak self] in
                guard let self, self.view.window != nil, self.player == nil else { return }
                self.embedPlayerInline()
            }
            return
        }
        if hasStartedPlayback {
            player?.handle(command: .play)
        }
    }

    fileprivate func teardownPlayer() {
        player?.handle(command: .stop)
        player?.detach()
        player = nil
    }

    private func configureFirstFrameImageView() {
        firstFrameImageView.translatesAutoresizingMaskIntoConstraints = false
        firstFrameImageView.backgroundColor = .black
        firstFrameImageView.contentMode = imageContentMode(for: item.objectFit)
        firstFrameImageView.clipsToBounds = true
        firstFrameImageView.image = firstFrame
        firstFrameImageView.isHidden = firstFrame == nil
        firstFrameImageView.layer.zPosition = 3000
        view.addSubview(firstFrameImageView)
        NSLayoutConstraint.activate([
            firstFrameImageView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            firstFrameImageView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            firstFrameImageView.topAnchor.constraint(equalTo: view.topAnchor),
            firstFrameImageView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        applyFirstFrameRotation()
    }

    private func imageContentMode(for objectFit: LxMediaObjectFit?) -> UIView.ContentMode {
        switch objectFit ?? .contain {
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

    private func applyFirstFrameRotation() {
        let degrees = item.rotate ?? 0
        let normalized = ((degrees % 360) + 360) % 360
        guard normalized == 0 || normalized == 90 || normalized == 180 || normalized == 270 else {
            firstFrameImageView.transform = .identity
            return
        }
        firstFrameImageView.transform = CGAffineTransform(rotationAngle: CGFloat(normalized) * .pi / 180)
    }

    private func handOffFirstFrameToPlayerOverlay() {
        guard !firstFrameImageView.isHidden else { return }
        firstFrameImageView.isHidden = true
        firstFrameImageView.image = nil
    }

    private func markVisualReadyOnce() {
        if didSignalVisualReady { return }
        didSignalVisualReady = true
        visualReadyHandler(index)
    }

    private func embedPlayerInline() {
        view.layoutIfNeeded()
        let config = LxMediaPlayerConfig(
            src: item.url,
            poster: nil,
            autoplay: true,
            loop: loopPlayback,
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
            case .playing:
                // .playing fires when AVPlayer actually starts rendering frames
                // — that's the right moment to hand off from the still poster
                // (firstFrameImageView) to the live player surface. Hiding the
                // poster on attach (which used to happen) caused a visible
                // black gap between attach and first decoded frame, i.e. the
                // flicker the user reported.
                self?.handOffFirstFrameToPlayerOverlay()
                self?.markVisualReadyOnce()
            case .ended:
                self?.endedHandler()
            case .error(let code, let message):
                os_log("Error: %{public}@ - %{public}@", log: self?.log ?? .default, type: .error, code, message)
                self?.errorHandler()
            default:
                break
            }
        })

        player.update(config: config)
        // Don't show player's close button - using preview's custom close button
        player.setShowCloseButton(false)
        // Don't show fullscreen button - preview is already fullscreen
        player.setShowFullscreenButton(false)
        player.onScrubStateChanged = { [weak self] scrubbing in
            self?.isScrubbing = scrubbing
            self?.scrubStateChanged(scrubbing)
        }
        self.player = player

        player.view.layer.zPosition = 1000
        player.attach(to: view)
        // Don't hand off the poster yet — wait for the player's `.playing`
        // event which signals the first decoded frame has been rendered.
        // Hiding the poster eagerly here is what produced the post-load flicker.
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
    var onVisualReady: (() -> Void)?
    var onDismiss: (() -> Void)?
    private var didSignalVisualReady = false

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
                self.notifyVisualReadyOnce()
            }
        }
    }

    private func notifyVisualReadyOnce() {
        if didSignalVisualReady { return }
        didSignalVisualReady = true
        setNeedsLayout()
        layoutIfNeeded()
        onVisualReady?()
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
