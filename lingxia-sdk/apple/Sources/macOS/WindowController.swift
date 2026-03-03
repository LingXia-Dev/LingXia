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
        static let sidebarWidth: CGFloat = 180
        static let minSidebarWidth: CGFloat = 48
    }

    private let tabManager = LxAppTabManager.shared
    private var sidebarView: SidebarView?
    private var navigationToolbar: MacNavigationToolbar?
    private var sidebarWidthConstraint: NSLayoutConstraint?
    private var lastExpandedSidebarWidth: CGFloat = Layout.sidebarWidth
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]
    private var appSessions: [String: UInt64] = [:]
    internal let panelManager = PanelLayoutManager()
    nonisolated(unsafe) private var sidebarRefreshObserver: NSObjectProtocol?

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

    deinit {
        sidebarRefreshObserver.map(NotificationCenter.default.removeObserver)
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

        tabManager.onTabsChanged = { [weak self] tabs in
            self?.sidebarView?.updateForTabs(tabs, activeTab: self?.tabManager.activeTab)
        }

        setupSidebarInterface()
        setupNotificationObservers()
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

    // MARK: - Sidebar Interface Setup

    private func setupSidebarInterface() {
        guard let window = self.window, let contentView = window.contentView else { return }

        // Create sidebar
        let sidebar = SidebarView()
        sidebar.translatesAutoresizingMaskIntoConstraints = false
        sidebar.onAppPageSelected = { [weak self] appId, itemIndex in
            self?.handleSidebarPageSelection(appId: appId, itemIndex: itemIndex)
        }
        sidebar.onAppCloseRequested = { [weak self] appId in
            self?.closeTab(appId)
        }
        sidebar.onToggleRequested = { [weak self] in
            self?.toggleSidebar()
        }
        sidebar.onWidthChanged = { [weak self] width, animated in
            self?.updateSidebarWidth(width, animated: animated)
        }
        sidebarView = sidebar
        contentView.addSubview(sidebar)

        // Create content container (right of sidebar)
        let right = NSView()
        right.translatesAutoresizingMaskIntoConstraints = false
        right.wantsLayer = true
        contentView.addSubview(right)

        // Create navigation toolbar
        let toolbar = MacNavigationToolbar()
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.onNavigationAction = { [weak self] action in
            guard let appId = self?.tabManager.activeTab?.appId else { return }
            if action == "back" {
                let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionBack)
            } else if action == "home" {
                let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionHome)
            }
        }
        navigationToolbar = toolbar
        right.addSubview(toolbar)

        // Panel layout manager's root view fills area below toolbar
        let panelRoot = panelManager.rootView
        panelRoot.translatesAutoresizingMaskIntoConstraints = false
        right.addSubview(panelRoot)

        // Layout constraints
        let sidebarWidth = sidebar.widthAnchor.constraint(equalToConstant: Layout.sidebarWidth)
        sidebarWidthConstraint = sidebarWidth

        NSLayoutConstraint.activate([
            // Sidebar: left side, full height
            sidebar.topAnchor.constraint(equalTo: contentView.topAnchor),
            sidebar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            sidebar.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            sidebarWidth,

            // Right container: fills remaining space
            right.topAnchor.constraint(equalTo: contentView.topAnchor),
            right.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor),
            right.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            right.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),

            // Navigation toolbar: top of right container
            toolbar.topAnchor.constraint(equalTo: right.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: right.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: right.trailingAnchor),

            // Panel root: below toolbar, fills rest
            panelRoot.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            panelRoot.leadingAnchor.constraint(equalTo: right.leadingAnchor),
            panelRoot.trailingAnchor.constraint(equalTo: right.trailingAnchor),
            panelRoot.bottomAnchor.constraint(equalTo: right.bottomAnchor),
        ])
    }

    private func setupNotificationObservers() {
        sidebarRefreshObserver = NotificationCenter.default.addObserver(
            forName: .sidebarNeedsRefresh,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            let appId = notification.object as? String
            Task { @MainActor in
                guard let self, let appId else { return }
                self.sidebarView?.refreshAppGroup(appId: appId)
                if let activeAppId = self.tabManager.activeTab?.appId, activeAppId == appId {
                    self.sidebarView?.setActiveHighlight(appId: appId)
                }
            }
        }
    }

    // MARK: - Sidebar Actions

    func handleSidebarPageSelection(appId: String, itemIndex: Int) {
        // Switch to the lxapp tab if not already active
        if tabManager.activeTab?.appId != appId {
            tabManager.selectTab(appId: appId)
        }
        // Always update sidebar highlight, even if Rust returns early for same index
        sidebarView?.setActiveHighlight(appId: appId, pageIndex: itemIndex)
        // Notify Rust of page navigation via tabbar click
        let _ = onUiEvent(appId, LxAppUIEvent.tabBarClick, String(itemIndex))
    }

    func toggleSidebar() {
        guard let constraint = sidebarWidthConstraint else { return }

        let isCollapsing = constraint.constant > Layout.minSidebarWidth
        if isCollapsing {
            lastExpandedSidebarWidth = constraint.constant
        }
        let targetWidth: CGFloat = isCollapsing ? Layout.minSidebarWidth : lastExpandedSidebarWidth

        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.25
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            constraint.animator().constant = targetWidth
        }, completionHandler: {
            MainActor.assumeIsolated { [weak self] in
                self?.sidebarView?.updateMinimizedState()
            }
        })
    }

    func updateSidebarWidth(_ width: CGFloat, animated: Bool) {
        guard let constraint = sidebarWidthConstraint else { return }

        if width > Layout.minSidebarWidth {
            lastExpandedSidebarWidth = width
        }

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.2
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = width
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.sidebarView?.updateMinimizedState()
                }
            })
        } else {
            constraint.constant = width
            sidebarView?.updateMinimizedState()
        }
    }

    // MARK: - Tab Lifecycle

    private func setupInitialTab() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else { return }
        let currentLxApp = getCurrentLxApp()
        let currentAppId = currentLxApp.appid.toString()
        let sessionId: UInt64 = (currentAppId == homeLxAppId)
            ? currentLxApp.session_id
            : getLxAppSessionId(homeLxAppId)
        guard sessionId > 0 else {
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

        // Update sidebar highlight
        sidebarView?.setActiveHighlight(appId: appId)
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
