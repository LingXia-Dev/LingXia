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
    private var pullToRefreshHelper: MacPullToRefreshHelper?
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

    // MARK: - UI Setup

    private func setupLayout() {
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

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
        setupPullToRefresh(for: webView)
    }

    func currentWebView() -> WKWebView? {
        activeWebView
    }

    private func setupPullToRefresh(for webView: WKWebView) {
        if pullToRefreshHelper == nil || pullToRefreshHelper?.webView !== webView {
            pullToRefreshHelper = MacPullToRefreshHelper(webView: webView)
        }
    }

    internal func startPullDownRefreshProgrammatically() {
        if let webView = findManagedWebView(path: currentPath) {
            setupPullToRefresh(for: webView)
        }
        pullToRefreshHelper?.startRefreshing()
        let _ = onLxappEvent(appId, LxAppEvent.pullDownRefresh, currentPath)
    }

    internal func stopPullDownRefreshProgrammatically() {
        pullToRefreshHelper?.endRefreshing()
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
        if let exactMatch = WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId) {
            return exactMatch
        }

        let lookupPath = normalizePath(path)
        guard lookupPath != path else { return nil }
        let fallback = WebViewManager.findWebView(appId: appId, path: lookupPath, sessionId: sessionId)
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
