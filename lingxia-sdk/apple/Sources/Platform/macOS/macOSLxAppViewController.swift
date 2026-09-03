#if os(macOS)
import Foundation
import WebKit
import AppKit
import CLingXiaRustAPI
import os.log

@MainActor
class macOSLxAppViewController: NSViewController, WKNavigationDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "macOSLxAppViewController")

    private static let navigationRetryDelayNs: UInt64 = 80_000_000
    private static let navigationRetryCount = 20
    /// Page-navigation slide duration; matches the iOS/Android 300ms transition.

    var appId: String
    internal var currentPath: String
    private var sessionId: UInt64
    private var webViewContainer: NSView!
    private weak var activeWebView: WKWebView?
    /// Covers a freshly swapped-in webview until its document paints:
    /// WKWebView's pre-commit frame is white, which reads as a flash.
    private var loadingPlaceholder: NSView?
    private var loadingObservation: NSKeyValueObservation?
    /// Owns when and how a page slides. Shared with the runner so the two
    /// hosts cannot drift apart again.
    private let pageTransition = LxAppPageTransition()

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?

    init(appId: String, path: String, sessionId: UInt64) {
        self.appId = appId
        self.currentPath = path
        self.sessionId = sessionId
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
    }

    override func loadView() {
        view = NSView()
        view.wantsLayer = true
        applyPageCanvasBackground()
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        setupLayout()
        setupNotificationObservers()
        loadWebViewContent()
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        let size = view.bounds.size
        guard size.width > 0, size.height > 0 else { return }
        _ = setSurfaceViewport(appId, Double(size.width), Double(size.height))
    }

    // MARK: - UI Setup

    private func setupLayout() {
        view.wantsLayer = true
        applyPageCanvasBackground()

        setupWebViewContainer()

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: view.topAnchor),
            webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    private func setupWebViewContainer() {
        webViewContainer = NSView()
        webViewContainer.wantsLayer = true
        webViewContainer.layer?.masksToBounds = true
        webViewContainer.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(webViewContainer)
    }

    // MARK: - WebView

    private func loadWebViewContent() {
        if let webView = findManagedWebView(path: currentPath) {
            showWebViewToUser(webView, path: currentPath)
        } else {
            // The first mount can race the page's WebView creation; converge
            // like navigate() does instead of silently staying blank.
            retryShowWebView(
                appId: appId,
                path: currentPath,
                sessionId: sessionId,
                animationType: .none,
                remainingAttempts: Self.navigationRetryCount
            )
        }
    }

    private func showWebViewToUser(
        _ webView: WKWebView,
        path: String,
        animation: LxAppAnimation = .none
    ) {
        // A controller-backed host can observe the same committed navigation
        // after the platform callback already attached it. Keep that repeated
        // delivery idempotent instead of detaching and constraining the same
        // WebView again.
        if animation == .none,
           activeWebView === webView,
           webView.superview === webViewContainer {
            webView.resumeWebView()
            return
        }

        // Same target webview (navigate to the already-shown page): a container
        // CATransition has no sublayer change to animate, so drive the slide from
        // a snapshot of the current page. Different webview: swap under a
        // CATransition (new page slides in over the old).
        if animation != .none, let current = activeWebView, current === webView {
            performSameWebViewTransition(webView: webView, animation: animation)
            return
        }
        // Sliding a page that has not painted yet shows its content settling —
        // React mounting, images and fonts landing — inside a frame that is
        // already moving. Hold the outgoing page instead and slide once the
        // incoming one can draw itself; nothing moves while we wait, so this
        // reads as the animation starting a moment later rather than as a stall.
        if activeWebView !== webView,
           LxAppPageTransition.needsPaintWait(webView, animation: animation) {
            let sessionId = self.sessionId
            pageTransition.whenPagePaints(
                webView,
                stillCurrent: { [weak self] in
                    guard let self else { return false }
                    return self.sessionId == sessionId && self.currentPath == path
                        && self.activeWebView !== webView
                },
                swap: { [weak self] in
                    self?.performWebViewSwap(webView, animation: animation)
                }
            )
            return
        }

        performWebViewSwap(webView, animation: animation)
    }

    private func performWebViewSwap(_ webView: WKWebView, animation: LxAppAnimation) {
        if activeWebView !== webView {
            pageTransition.install(animation, on: webViewContainer)
        }

        if let old = activeWebView, old !== webView {
            old.pauseWebView()
            old.removeFromSuperview()
        }

        for subview in webViewContainer.subviews {
            guard let existingWebView = subview as? WKWebView, existingWebView !== webView else {
                continue
            }
            existingWebView.pauseWebView()
            existingWebView.removeFromSuperview()
        }

        WebViewManager.attachLxAppWebView(webView, to: webViewContainer)
        activeWebView = webView
        coverUntilContentPaints(webView)
    }

    /// The ground behind the page. Anything that exposes it — a fade, a seam
    /// during a swap, rubber-band overscroll — should show the page's own
    /// canvas rather than the window chrome colour, which is a different grey.
    private func applyPageCanvasBackground() {
        WebViewManager.configureOpaquePagePlaceholder(view, appId: appId)
    }

    /// While the incoming webview still loads its document, show the page's
    /// background color instead of WebKit's white pre-commit frame, and fade
    /// it away once the load finishes.
    private func coverUntilContentPaints(_ webView: WKWebView) {
        loadingObservation?.invalidate()
        loadingObservation = nil
        loadingPlaceholder?.removeFromSuperview()
        loadingPlaceholder = nil

        // A fresh instance may swap in before its document load is even
        // queued (url == nil), not just mid-load.
        guard webView.isLoading || webView.url == nil else { return }

        let placeholder = NSView(frame: webViewContainer.bounds)
        placeholder.autoresizingMask = [.width, .height]
        WebViewManager.configureOpaquePagePlaceholder(placeholder, appId: appId)
        webViewContainer.addSubview(placeholder, positioned: .above, relativeTo: webView)
        loadingPlaceholder = placeholder

        let loadHasStarted = webView.isLoading
        loadingObservation = webView.observe(\.isLoading, options: [.new]) { [weak self] observed, change in
            // Reveal on the load FINISHING; the not-yet-started false state
            // never fires here because KVO only reports changes.
            guard change.newValue == false else { return }
            Task { @MainActor [weak self] in
                self?.revealLoadedContent()
            }
            _ = observed
        }
        // The load can finish between the check and the observation.
        if loadHasStarted && !webView.isLoading {
            revealLoadedContent()
        }
        // Never let the cover outlive a load that failed to start or report.
        let coveredPlaceholder = placeholder
        DispatchQueue.main.asyncAfter(deadline: .now() + 4.0) { [weak self] in
            guard let self, self.loadingPlaceholder === coveredPlaceholder else { return }
            self.revealLoadedContent()
        }
    }

    /// Drop the placeholder once the page can draw itself.
    ///
    /// Fading it while the navigation transition is still running reads as a
    /// second animation: the page slides in as a flat colour and then that
    /// colour dissolves, so one navigation looks like two. Most loads finish
    /// inside the 300ms slide, so during it the placeholder is removed outright
    /// and the slide is the only motion. A load that lands after the slide has
    /// nothing to collide with, and there the short fade still reads as content
    /// arriving rather than as a flash.
    private func revealLoadedContent() {
        loadingObservation?.invalidate()
        loadingObservation = nil
        guard let placeholder = loadingPlaceholder else { return }
        loadingPlaceholder = nil
        guard !pageTransition.isRunning else {
            placeholder.removeFromSuperview()
            return
        }
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.15
            placeholder.animator().alphaValue = 0
        }, completionHandler: {
            placeholder.removeFromSuperview()
        })
    }

    /// Navigating to the page already on screen (same WKWebView instance): the
    /// webview now shows the destination, so snapshot the outgoing page, slide it
    /// out while the webview slides in from the opposite edge. `.fade` cross-fades
    /// the snapshot out instead.
    private func performSameWebViewTransition(webView: WKWebView, animation: LxAppAnimation) {
        guard webView.superview === webViewContainer,
              webViewContainer.bounds.width > 0
        else {
            // Not laid out yet — just show it without an animation.
            webView.resumeWebView()
            activeWebView = webView
            return
        }
        let bounds = webViewContainer.bounds
        webViewContainer.layoutSubtreeIfNeeded()
        webView.takeSnapshot(with: nil) { [weak self] image, _ in
            guard let self, let image else {
                webView.resumeWebView()
                self?.activeWebView = webView
                return
            }
            MainActor.assumeIsolated {
                let snap = NSImageView(frame: bounds)
                snap.wantsLayer = true
                snap.imageScaling = .scaleAxesIndependently
                snap.image = image
                snap.autoresizingMask = [.width, .height]
                self.webViewContainer.addSubview(snap, positioned: .above, relativeTo: webView)

                let width = bounds.width
                let forward = animation != .pop
                webView.resumeWebView()
                self.activeWebView = webView

                CATransaction.begin()
                CATransaction.setCompletionBlock { snap.removeFromSuperview() }
                if animation == .fade {
                    let fade = CABasicAnimation(keyPath: "opacity")
                    fade.fromValue = 1.0
                    fade.toValue = 0.0
                    fade.duration = LxAppPageTransition.duration
                    snap.layer?.add(fade, forKey: "lxFadeOut")
                    snap.layer?.opacity = 0.0
                } else {
                    // Incoming webview starts off-screen on the leading/trailing
                    // edge and slides to rest; the snapshot slides out the other way.
                    let inFrom: CGFloat = forward ? width : -width
                    let outTo: CGFloat = forward ? -width : width
                    self.slide(layer: webView.layer, from: inFrom, to: 0)
                    self.slide(layer: snap.layer, from: 0, to: outTo)
                }
                CATransaction.commit()
            }
        }
    }

    private func slide(layer: CALayer?, from: CGFloat, to: CGFloat) {
        guard let layer else { return }
        let anim = CABasicAnimation(keyPath: "transform.translation.x")
        anim.fromValue = from
        anim.toValue = to
        anim.duration = LxAppPageTransition.duration
        anim.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        anim.fillMode = .forwards
        layer.add(anim, forKey: "lxSlide")
        layer.transform = CATransform3DMakeTranslation(to, 0, 0)
    }

    func currentWebView() -> WKWebView? {
        activeWebView
    }

    // MARK: - Notifications

    private func setupNotificationObservers() {
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_CLOSE_LXAPP), object: nil, queue: .main
        ) { [weak self] notification in
            let appId = notification.userInfo?["appId"] as? String
            Task { @MainActor in
                guard let self = self, let targetAppId = appId, targetAppId == self.appId else { return }
                self.view.window?.close()
            }
        }
    }

    // MARK: - Navigation

    @MainActor
    func navigate(appId: String, to path: String, with animationType: LxAppAnimation) {
        guard !appId.isEmpty else { return }

        // A restart can navigate before the view loads; force it so `webViewContainer`
        // (built in viewDidLoad) isn't a nil IUO. (`loadViewIfNeeded()` is macOS 14+.)
        _ = self.view

        self.currentPath = path
        updateNavigationBar(appId: appId, path: path)
        if let webView = findManagedWebView(path: path) {
            showWebViewToUser(webView, path: path, animation: animationType)
        } else {
            retryShowWebView(
                appId: appId,
                path: path,
                sessionId: sessionId,
                animationType: animationType,
                remainingAttempts: Self.navigationRetryCount
            )
        }
        LxAppCore.setCurrentPath(path)
    }

    @MainActor
    private func retryShowWebView(
        appId: String,
        path: String,
        sessionId: UInt64,
        animationType: LxAppAnimation,
        remainingAttempts: Int
    ) {
        guard remainingAttempts > 0 else { return }
        Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: Self.navigationRetryDelayNs)
            guard let self,
                  self.appId == appId,
                  self.sessionId == sessionId,
                  self.currentPath == path else { return }
            if let webView = self.findManagedWebView(path: path) {
                self.showWebViewToUser(webView, path: path, animation: animationType)
            } else {
                self.retryShowWebView(
                    appId: appId,
                    path: path,
                    sessionId: sessionId,
                    animationType: animationType,
                    remainingAttempts: remainingAttempts - 1
                )
            }
        }
    }

    internal func updateSessionId(_ value: UInt64) {
        if value > 0 {
            sessionId = value
        }
    }

    @MainActor
    func updateNavigationBar(appId: String, path: String) {
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
    }

    private func findManagedWebView(path: String) -> WKWebView? {
        if let exactMatch = WebViewManager.resolveWebView(appId: appId, path: path, sessionId: sessionId) {
            return exactMatch
        }

        let lookupPath = normalizePath(path)
        guard lookupPath != path else { return nil }
        let fallback = WebViewManager.resolveWebView(appId: appId, path: lookupPath, sessionId: sessionId)
        return fallback
    }

    private func normalizePath(_ rawPath: String) -> String {
        if rawPath.isEmpty { return "" }
        if let queryIndex = rawPath.firstIndex(of: "?") {
            return String(rawPath[..<queryIndex])
        }
        if let hashIndex = rawPath.firstIndex(of: "#") {
            return String(rawPath[..<hashIndex])
        }
        return rawPath
    }

    // MARK: - Native Components

    @MainActor
    func pauseNativeComponents() {
        if let webView = findManagedWebView(path: currentPath) {
            MacNativeBridge.notifyPageInactive(for: webView)
        }
    }

    @MainActor
    func resumeNativeComponents() {
        if let webView = findManagedWebView(path: currentPath) {
            MacNativeBridge.notifyPageActive(for: webView)
        }
    }

    @MainActor
    func destroyNativeComponents() {
        if let webView = findManagedWebView(path: currentPath) {
            MacNativeBridge.notifyPageDestroyed(for: webView)
        }
    }
}

#endif
