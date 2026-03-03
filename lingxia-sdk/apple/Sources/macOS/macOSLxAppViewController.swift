#if os(macOS)
import Foundation
import WebKit
import os.log
import AppKit
import SwiftUI
import CLingXiaRustAPI

private let lxAppViewControllerLog = OSLog(subsystem: "LingXia", category: "LxAppView")

@MainActor
public class macOSLxAppViewController: NSViewController, WKNavigationDelegate {
    nonisolated private static let log = lxAppViewControllerLog

    private var currentTopMargin: CGFloat = 0

    private func getTopMargin() -> CGFloat {
        return currentTopMargin
    }

    internal func updateTopMargin(_ newMargin: CGFloat) {
        currentTopMargin = newMargin
        refreshWebViewLayout()
    }

    private func refreshWebViewLayout() {
        guard let webViewContainer = webViewContainer else { return }

        view.removeConstraints(view.constraints.filter { constraint in
            constraint.firstItem === webViewContainer && constraint.firstAttribute == .top
        })

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
        ])

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    // Properties
    public var appId: String
    internal var currentPath: String
    private var sessionId: UInt64
    private var webViewContainer: NSView!
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false
    private var pullToRefreshHelper: MacPullToRefreshHelper?

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?

    public init(appId: String, path: String, sessionId: UInt64) {
        self.appId = appId
        self.currentPath = path
        self.sessionId = sessionId
        super.init(nibName: nil, bundle: nil)

        // Initialize top margin based on current page
        self.currentTopMargin = calculateInitialTopMargin()
    }

    private func calculateInitialTopMargin() -> CGFloat {
        // Tab style: 0pt - tab bar handles layout
        return 0
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
    }

    public override func loadView() {
        view = NSView()
        view.wantsLayer = true
        view.layer?.backgroundColor = AppKit.NSColor.windowBackgroundColor.cgColor
    }

    public override func viewDidLoad() {
        super.viewDidLoad()

        setupLayout()
        setupNotificationObservers()
        loadWebViewContent()
    }

    // UI Setup
    private func setupLayout() {
        view.wantsLayer = true
        view.layer?.backgroundColor = AppKit.NSColor.windowBackgroundColor.cgColor

        setupWebViewContainer()

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
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

    private func loadWebViewContent() {
        if let webView = WebViewManager.findWebView(appId: appId, path: currentPath, sessionId: sessionId) {
            showWebViewToUser(webView, path: currentPath)
        }
    }

    private func showWebViewToUser(_ webView: WKWebView, path: String) {
        let oldWebView = LxAppCore.getCurrentWebView()
        if let old = oldWebView, old !== webView {
            old.pauseWebView()
        }
        oldWebView?.removeFromSuperview()

        WebViewManager.attachWebViewToContainer(webView, container: webViewContainer)
        MacNativeBridge.attachIfNeeded(to: webView, in: webViewContainer)
        webView.resumeWebView()
        setupPullToRefresh(for: webView)
    }

    private func setupPullToRefresh(for webView: WKWebView) {
        if pullToRefreshHelper == nil || pullToRefreshHelper?.webView !== webView {
            pullToRefreshHelper = MacPullToRefreshHelper(webView: webView)
        }
    }

    internal func startPullDownRefreshProgrammatically() {
        if let webView = WebViewManager.findWebView(appId: appId, path: currentPath, sessionId: sessionId) {
            setupPullToRefresh(for: webView)
        }
        pullToRefreshHelper?.startRefreshing()
        let _ = onUiEvent(appId, LxAppUIEvent.pullDownRefresh, currentPath)
    }

    internal func stopPullDownRefreshProgrammatically() {
        pullToRefreshHelper?.endRefreshing()
    }

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

    @MainActor
    public func navigate(appId: String, to path: String, with animationType: AnimationType) {
        guard !appId.isEmpty else { return }

        self.currentPath = path

        updateNavigationBar(appId: appId, path: path)

        if let webView = WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId) {
            showWebViewToUser(webView, path: path)
        }

        LxAppCore.setCurrentPath(path)
    }

    public func setSelectedTabIndex(_ index: Int) {
        selectedTabIndex = index
    }

    internal func updateSessionId(_ value: UInt64) {
        if value > 0 {
            sessionId = value
        }
    }

    @MainActor
    public func updateNavigationBar(appId: String, path: String) {
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
    }

    @MainActor
    func pauseNativeComponents() {
        if let webView = WebViewManager.findWebView(appId: appId, path: currentPath, sessionId: sessionId) {
            MacNativeBridge.notifyPageInactive(for: webView)
        }
    }

    @MainActor
    func resumeNativeComponents() {
        if let webView = WebViewManager.findWebView(appId: appId, path: currentPath, sessionId: sessionId) {
            MacNativeBridge.notifyPageActive(for: webView)
        }
    }

    @MainActor
    func destroyNativeComponents() {
        if let webView = WebViewManager.findWebView(appId: appId, path: currentPath, sessionId: sessionId) {
            MacNativeBridge.notifyPageDestroyed(for: webView)
        }
    }
}


#endif
