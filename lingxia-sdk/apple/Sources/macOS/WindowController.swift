#if os(macOS)
import AppKit
import SwiftUI
import WebKit
import Quartz
import os.log
import CLingXiaRustAPI

/// Window controller for macOS
public class LxAppWindowController: NSWindowController, NSWindowDelegate {

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppWindowController")

    public struct Layout {
        static let tabBarHeight: CGFloat = 32
    }

    private let tabManager = LxAppTabManager.shared
    private var tabView: LxAppTabView?
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]
    private var appSessions: [String: UInt64] = [:]
    internal let panelManager = PanelLayoutManager()

    /// Get view controller for specific appId (needed for navigation)
    public func getViewController(for appId: String) -> macOSLxAppViewController? {
        return viewControllers[appId]
    }

    /// Initialize for tab mode
    init() {
        let window = Self.createWindow()
        super.init(window: window)
        setupTabMode()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private static func createWindow() -> LxAppWindow {
        let window = LxAppWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        window.configureForTabStyle()
        window.center()
        window.isReleasedWhenClosed = false

        return window
    }

    private func setupTabMode() {
        self.window?.delegate = self

        if let window = self.window as? LxAppWindow {
            window.standardWindowButton(.zoomButton)?.isHidden = false
        }

        tabManager.onTabChanged = { [weak self] tab in
            self?.switchToTab(tab.appId)
        }

        setupTabInterface()
        setupInitialTab()
    }

    public func windowWillClose(_ notification: Notification) {
        for (_, viewController) in viewControllers {
            viewController.destroyNativeComponents()
        }
        // Tab mode cleanup
        for tab in tabManager.tabs {
            if let sessionId = appSessions[tab.appId], sessionId > 0 {
                let accepted = onLxappClosed(tab.appId, sessionId)
                if !accepted {
                    os_log("Ignoring stale close callback during cleanup for %@ (session=%{public}llu)", log: Self.log, type: .info, tab.appId, sessionId)
                }
            }
            LxAppCore.removeSessionId(for: tab.appId)
        }
        macOSLxApp.removeTabWindowController(self)
    }

    private func setupTabInterface() {
        guard let window = self.window, let contentView = window.contentView else { return }

        tabView = LxAppTabView(tabManager: tabManager)
        guard let tabBar = tabView else { return }

        tabBar.translatesAutoresizingMaskIntoConstraints = false
        tabBar.wantsLayer = true
        tabBar.layer?.zPosition = 10
        tabBar.onTabClosed = { [weak self] appId in
            self?.closeTab(appId)
        }

        tabBar.onNavigationAction = { [weak self] action in
            guard let appId = self?.tabManager.activeTab?.appId else { return }
            if action == "back" {
                let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionBack)
            } else if action == "home" {
                let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionHome)
            }
        }

        contentView.addSubview(tabBar)

        NSLayoutConstraint.activate([
            tabBar.topAnchor.constraint(equalTo: contentView.topAnchor),
            tabBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            tabBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            tabBar.heightAnchor.constraint(equalToConstant: Layout.tabBarHeight)
        ])

        // Panel layout manager's root view fills the area below the tab bar
        let panelRoot = panelManager.rootView
        panelRoot.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(panelRoot)

        NSLayoutConstraint.activate([
            panelRoot.topAnchor.constraint(equalTo: tabBar.bottomAnchor),
            panelRoot.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            panelRoot.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            panelRoot.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])
    }

    private func setupInitialTab() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else { return }
        let currentLxApp = getCurrentLxApp()
        let currentAppId = currentLxApp.appid.toString()
        let sessionId = currentLxApp.session_id
        guard currentAppId == homeLxAppId, sessionId > 0 else {
            os_log("setupInitialTab missing home session for %@", log: Self.log, type: .error, homeLxAppId)
            return
        }

        // Get resolved path from onLxappOpened (pass empty string to get initial route)
        let resolvedPath = onLxappOpened(homeLxAppId, "", sessionId)
        guard !resolvedPath.toString().isEmpty else {
            os_log("setupInitialTab rejected by Rust (stale session?) for %@", log: Self.log, type: .info, homeLxAppId)
            return
        }
        appSessions[homeLxAppId] = sessionId
        LxAppCore.setSessionId(sessionId, for: homeLxAppId)
        LxAppCore.setCurrentApp(appId: homeLxAppId, path: resolvedPath.toString())
        tabManager.addTab(appId: homeLxAppId)
    }

    public func openLxApp(appId: String, path: String, sessionId: UInt64) {
        appSessions[appId] = sessionId
        LxAppCore.setSessionId(sessionId, for: appId)
        LxAppCore.setCurrentApp(appId: appId, path: path)
        tabManager.addTab(appId: appId)
        macOSLxApp.navigate(appId: appId, path: path, animationType: .none)
    }

    private func switchToTab(_ appId: String) {
        guard let sessionId = appSessions[appId], sessionId > 0 else {
            os_log("switchToTab missing session for %@", log: Self.log, type: .error, appId)
            return
        }
        let isNewViewController = viewControllers[appId] == nil

        let viewController = viewControllers[appId] ?? {
            let currentPath = LxAppCore.getCurrentPath()
            let vc = macOSLxAppViewController(appId: appId, path: currentPath, sessionId: sessionId)
            viewControllers[appId] = vc
            return vc
        }()
        viewController.updateSessionId(sessionId)

        if isNewViewController {
            let currentPath = LxAppCore.getCurrentPath()
            let resolved = onLxappOpened(appId, currentPath, sessionId).toString()
            if resolved.isEmpty {
                os_log("switchToTab rejected by Rust (stale session?) for %@", log: Self.log, type: .info, appId)
                return
            }
        }

        updateContentView(with: viewController)
    }

    private func updateContentView(with viewController: macOSLxAppViewController) {
        currentViewController?.pauseNativeComponents()
        currentViewController?.view.removeFromSuperview()
        currentViewController = viewController

        let container = panelManager.contentContainer

        viewController.view.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(viewController.view)

        NSLayoutConstraint.activate([
            viewController.view.topAnchor.constraint(equalTo: container.topAnchor),
            viewController.view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            viewController.view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            viewController.view.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        viewController.resumeNativeComponents()
    }

    // MARK: - QLPreviewPanel support

    public override func acceptsPreviewPanelControl(_ panel: QLPreviewPanel!) -> Bool {
        return MainActor.assumeIsolated {
            LxAppMedia.qlController != nil
        }
    }

    public override func beginPreviewPanelControl(_ panel: QLPreviewPanel!) {
    }

    public override func endPreviewPanelControl(_ panel: QLPreviewPanel!) {
        MainActor.assumeIsolated {
            LxAppMedia.clearQLController()
        }
    }

    // MARK: - Panel Control

    /// Show a panel with WebView content. Registers the panel if not already registered.
    public func showPanelWithContent(id: String, position: PanelPosition, appId: String, path: String) {
        if !panelManager.isPanelRegistered(id: id) {
            let config = PanelConfig(id: id, position: position)
            panelManager.registerPanel(config)
        }

        if let sessionId = appSessions[appId],
           let webView = WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId),
           let container = panelManager.panelContainer(id: id) {
            WebViewManager.attachWebViewToContainer(webView, container: container)
        }

        panelManager.showPanel(id: id)
    }

    public func hidePanel(id: String) {
        panelManager.hidePanel(id: id)
    }

    public func togglePanel(id: String) {
        panelManager.togglePanel(id: id)
    }

    private func closeTab(_ appId: String) {
        guard let sessionId = appSessions[appId], sessionId > 0 else {
            os_log("closeTab missing session for %@", log: Self.log, type: .error, appId)
            return
        }
        let accepted = onLxappClosed(appId, sessionId)
        guard accepted else {
            os_log("Ignoring stale close callback for %@ (session=%{public}llu)", log: Self.log, type: .info, appId, sessionId)
            return
        }

        if let viewController = viewControllers[appId] {
            viewController.destroyNativeComponents()
            viewController.view.removeFromSuperview()
            viewControllers.removeValue(forKey: appId)
        }

        tabManager.closeTab(appId: appId)
        appSessions.removeValue(forKey: appId)
        LxAppCore.removeSessionId(for: appId)

        let currentLxApp = getCurrentLxApp()
        let appidStr = currentLxApp.appid.toString()
        let pathStr = currentLxApp.path.toString()
        let nextSessionId = currentLxApp.session_id
        if !appidStr.isEmpty && nextSessionId > 0 {
            os_log("Opening next LxApp from stack as tab: %@:%@", log: Self.log, type: .info, appidStr, pathStr)
            macOSLxApp.openLxApp(appId: appidStr, path: pathStr, sessionId: nextSessionId)
        } else if !tabManager.hasTabs {
            window?.close()
        }
    }
}

#endif
