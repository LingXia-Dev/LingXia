import Foundation
import WebKit
import os.log

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// Base class for LxApp view controllers
@MainActor
public class LxAppViewController: PlatformViewController {
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "LxAppViewController")

    internal var appId: String
    internal var initialPath: String
    internal var isDisplayingHomeLxApp: Bool = false

    // UI Components
    internal var rootContainer: PlatformView!
    internal var statusBarBackground: PlatformView!
    internal var webViewContainer: PlatformView!
    internal var currentWebView: WKWebView?

    // Navigation and Tab Bar
    internal var navigationBar: NavigationBar?
    internal var tabBar: TabBarProtocol?

    // Observers
    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

    public init(appId: String, path: String) {
        self.appId = appId
        self.initialPath = path
        self.isDisplayingHomeLxApp = LxAppCore.isHomeLxApp(appId)

        super.init(nibName: nil, bundle: nil)

        setupNotificationObservers()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
        switchPageObserver.map(NotificationCenter.default.removeObserver)
    }

    //  Lifecycle
    public override func viewDidLoad() {
        super.viewDidLoad()

        // Get the last active path for state restoration, using initialPath as default
        let targetPath = LxAppCore.getLastActivePath(for: appId, defaultPath: initialPath)
        switchToPage(targetPath)
    }

    private func setupNotificationObservers() {
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_CLOSE_LXAPP),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            let appId = notification.userInfo?["appId"] as? String
            Task { @MainActor in
                guard let self = self,
                      let targetAppId = appId,
                      targetAppId == self.appId else {
                    return
                }
                self.closeApp()
            }
        }

        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            let appId = notification.userInfo?["appId"] as? String
            let path = notification.userInfo?["path"] as? String
            Task { @MainActor in
                guard let self = self,
                      let targetAppId = appId,
                      targetAppId == self.appId,
                      let targetPath = path else {
                    return
                }
                self.switchToPage(targetPath)
            }
        }
    }

    private func removeNotificationObservers() {
        if let observer = closeAppObserver {
            NotificationCenter.default.removeObserver(observer)
        }
        if let observer = switchPageObserver {
            NotificationCenter.default.removeObserver(observer)
        }
    }

    internal func switchToPage(_ path: String) {
        guard let webView = WebViewManager.findWebView(appId: appId, path: path) else { return }
        attachWebView(webView, path: path)
        LxAppCore.setLastActivePath(path, for: appId)
    }

    private func attachWebView(_ webView: WKWebView, path: String) {
        currentWebView?.removeFromSuperview()
        currentWebView = webView
        webViewContainer.addSubview(webView)
        webView.isRegistered = true
    }

    internal func closeApp() {
        // Default implementation - subclasses should override if needed
    }
}
