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

    var appId: String
    internal var currentPath: String
    private var sessionId: UInt64
    private var webViewContainer: NSView!
    private var refreshStrip: NSView!
    private var refreshStripHeight: NSLayoutConstraint!
    private let refreshIndicator = MacRefreshIndicatorView()
    private let refreshStripExpandedHeight: CGFloat = 40
    private var isRefreshing = false
    private var refreshShownAt: Date?
    // Keep the indicator on screen briefly even if the page finishes refreshing immediately,
    // otherwise a fast onPullDownRefresh collapses it before the animation is perceptible.
    private let refreshMinVisibleDuration: TimeInterval = 0.8
    private weak var activeWebView: WKWebView?

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
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        setupLayout()
        setupNotificationObservers()
        loadWebViewContent()
    }

    // Deliberately NOT reporting setSurfaceWidth here: as the desktop shell's main
    // content this view's width is the residual main pane (it shrinks as asides
    // dock), which would override the shell's stable workspace report and drop the
    // sizeClass. The shell owns width on desktop; SimulatorViewController on phone.

    // MARK: - UI Setup

    private func setupLayout() {
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

        setupRefreshStrip()
        setupWebViewContainer()

        // The refresh strip sits in the layout flow between the navigation bar (owned by the
        // host shell, above this view) and the web view. Idle height is 0; an active refresh
        // expands it, pushing the web content down instead of floating over it.
        refreshStripHeight = refreshStrip.heightAnchor.constraint(equalToConstant: 0)

        NSLayoutConstraint.activate([
            refreshStrip.topAnchor.constraint(equalTo: view.topAnchor),
            refreshStrip.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            refreshStrip.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            refreshStripHeight,

            webViewContainer.topAnchor.constraint(equalTo: refreshStrip.bottomAnchor),
            webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    private func setupRefreshStrip() {
        refreshStrip = NSView()
        refreshStrip.translatesAutoresizingMaskIntoConstraints = false
        refreshStrip.wantsLayer = true
        // Clip the indicator while the strip is collapsed (height 0).
        refreshStrip.layer?.masksToBounds = true
        view.addSubview(refreshStrip)

        refreshIndicator.translatesAutoresizingMaskIntoConstraints = false
        refreshStrip.addSubview(refreshIndicator)
        NSLayoutConstraint.activate([
            refreshIndicator.centerXAnchor.constraint(equalTo: refreshStrip.centerXAnchor),
            refreshIndicator.centerYAnchor.constraint(equalTo: refreshStrip.centerYAnchor),
            refreshIndicator.widthAnchor.constraint(equalToConstant: 64),
            refreshIndicator.heightAnchor.constraint(equalToConstant: 32)
        ])
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
        }
    }

    private func showWebViewToUser(_ webView: WKWebView, path: String) {
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

        WebViewManager.attachWebViewToContainer(webView, container: webViewContainer)
        MacNativeBridge.attachIfNeeded(to: webView, in: webViewContainer)
        webView.resumeWebView()
        activeWebView = webView
    }

    func currentWebView() -> WKWebView? {
        activeWebView
    }

    internal func startPullDownRefreshProgrammatically() {
        if !isRefreshing {
            isRefreshing = true
            // Match the revealed strip to the page background so it reads as the page being
            // pulled down. underPageBackgroundColor is WebKit's own overscroll/around-content
            // color and tracks the document background. Derive a dot color that contrasts with
            // that background so the indicator stays visible — a semantic color resolves against
            // the view's dark-chrome appearance and vanishes on a light page strip.
            let pageBackground = activeWebView?.underPageBackgroundColor ?? .windowBackgroundColor
            refreshStrip.layer?.backgroundColor = pageBackground.cgColor
            refreshIndicator.setDotColor(Self.contrastingDotColor(for: pageBackground))
            refreshShownAt = Date()
            refreshIndicator.startLoading()
            view.layoutSubtreeIfNeeded()
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.22
                context.timingFunction = CAMediaTimingFunction(name: .easeOut)
                refreshStripHeight.animator().constant = refreshStripExpandedHeight
                view.layoutSubtreeIfNeeded()
            }
        }
        let _ = onLxappEvent(appId, LxAppEvent.pullDownRefresh, currentPath)
    }

    internal func stopPullDownRefreshProgrammatically() {
        guard isRefreshing else { return }
        let elapsed = refreshShownAt.map { Date().timeIntervalSince($0) } ?? refreshMinVisibleDuration
        let remaining = refreshMinVisibleDuration - elapsed
        if remaining > 0 {
            DispatchQueue.main.asyncAfter(deadline: .now() + remaining) { [weak self] in
                self?.collapseRefreshStrip()
            }
        } else {
            collapseRefreshStrip()
        }
    }

    /// Pick a dot color that contrasts with the page background: dark dots on a light page,
    /// light dots on a dark page.
    private static func contrastingDotColor(for background: NSColor) -> NSColor {
        let rgb = background.usingColorSpace(.sRGB) ?? background.usingColorSpace(.deviceRGB)
        guard let rgb else { return NSColor.black.withAlphaComponent(0.55) }
        let luminance = 0.299 * rgb.redComponent + 0.587 * rgb.greenComponent + 0.114 * rgb.blueComponent
        return luminance > 0.5
            ? NSColor.black.withAlphaComponent(0.55)
            : NSColor.white.withAlphaComponent(0.85)
    }

    private func collapseRefreshStrip() {
        guard isRefreshing else { return }
        isRefreshing = false
        refreshShownAt = nil
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            refreshStripHeight.animator().constant = 0
            view.layoutSubtreeIfNeeded()
        } completionHandler: { [weak self] in
            Task { @MainActor in self?.refreshIndicator.stopLoading() }
        }
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
            showWebViewToUser(webView, path: path)
        } else {
            retryShowWebView(
                appId: appId,
                path: path,
                sessionId: sessionId,
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
                self.showWebViewToUser(webView, path: path)
            } else {
                self.retryShowWebView(
                    appId: appId,
                    path: path,
                    sessionId: sessionId,
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
