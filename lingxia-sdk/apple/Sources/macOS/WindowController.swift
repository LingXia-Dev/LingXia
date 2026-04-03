#if os(macOS)
import AppKit
import SwiftUI
import WebKit
import Quartz
import os.log
import CLingXiaRustAPI

/// Window controller for macOS
class LxAppWindowController: NSWindowController, NSWindowDelegate {

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppWindowController")

    struct Layout {
        static let sidebarWidth: CGFloat = 180
        static let sidebarHiddenThreshold: CGFloat = 1
        /// Shared center-Y baseline for all toolbar elements (traffic lights, nav buttons, address bar).
        /// = toolbar band height / 2 = 38 / 2 = 19pt from the visual window top.
        static let toolbarCenterY: CGFloat = 19
        static let trafficLightClearanceFallback: CGFloat = 80
        /// Padding around the floating content panel (Layer 2)
        static let contentPanelPadding: CGFloat = 6
        /// Corner radius of the floating content panel
        static let contentPanelCornerRadius: CGFloat = 10
        static let sidebarRevealButtonSize = CGSize(width: 36, height: 36)
        static let sidebarRevealButtonLeadingInset: CGFloat = 4
        static let sidebarRevealButtonBottomInset: CGFloat = 4
    }

    /// Background color for the base layer (Layer 1) visible through padding gaps.
    /// Change this to customize the window chrome color.
    private var baseLayerColor: NSColor = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            ? NSColor(red: 0.16, green: 0.16, blue: 0.18, alpha: 1)   // dark mode
            : NSColor(red: 0.90, green: 0.90, blue: 0.92, alpha: 1)   // light mode
    }

    private let tabManager = LxAppTabManager.shared
    let browserCoordinator = BrowserTabCoordinator()
    internal var sidebarView: SidebarView?
    private var navigationToolbar: MacNavigationToolbar?
    private var sidebarWidthConstraint: NSLayoutConstraint?
    private var contentLeadingConstraint: NSLayoutConstraint?
    private var cardTrailingConstraint: NSLayoutConstraint?
    private var cardBottomConstraint: NSLayoutConstraint?
    private var lastExpandedSidebarWidth: CGFloat = Layout.sidebarWidth
    private let sidebarRevealButton = NSButton()
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]
    internal var appSessions: [String: UInt64] = [:]
    internal let workspaceManager = WorkspaceManager()
    nonisolated(unsafe) private var sidebarRefreshObserver: NSObjectProtocol?

    /// The content panel view (excludes sidebar). Use this as the root container for popups.
    private(set) var contentPanelView: NSView?

    /// Get view controller for specific appId (needed for navigation)
    func getViewController(for appId: String) -> macOSLxAppViewController? {
        return viewControllers[appId]
    }

    /// Initialize for tab mode
    init() {
        let window = Self.createWindow()
        super.init(window: window)
        browserCoordinator.host = self
        setupTabMode()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        sidebarRefreshObserver.map(NotificationCenter.default.removeObserver)
        browserCoordinator.cleanup()
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
            window.trafficLightCenterYFromTop = Layout.contentPanelPadding + Layout.toolbarCenterY
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

    func windowWillClose(_ notification: Notification) {
        for (_, viewController) in viewControllers {
            viewController.destroyNativeComponents()
        }
        browserCoordinator.closeAllTabs(notifyRust: false)
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
        sidebar.onHideRequested = { [weak self] in
            self?.hideSidebar()
        }
        sidebar.onWidthChanged = { [weak self] width, animated in
            self?.updateSidebarWidth(width, animated: animated)
        }
        sidebar.onAddBrowserTab = { [weak self] in
            self?.browserCoordinator.addTab()
        }
        sidebar.onOpenSettings = { [weak self] in
            self?.browserCoordinator.openSettings()
        }
        sidebar.onOpenDownloads = { [weak self] in
            self?.browserCoordinator.openDownloads()
        }
        sidebar.onBrowserTabSelected = { [weak self] id in
            self?.browserCoordinator.selectTab(id: id)
        }
        sidebar.onBrowserTabCloseRequested = { [weak self] id in
            self?.browserCoordinator.closeTab(id: id)
        }
        sidebar.onPanelItemToggled = { panelId in
            macOSLxApp.togglePanel(id: panelId)
        }
        sidebar.updatePanelItems(macOSLxApp.panelItems.map {
            PanelIconItem(id: $0.id, icon: $0.icon, label: $0.label)
        })
        sidebarView = sidebar
        contentView.addSubview(sidebar)

        // Base layer (Layer 1) — solid color fills entire window, visible through padding gaps
        let base = NSView()
        base.translatesAutoresizingMaskIntoConstraints = false
        base.wantsLayer = true
        base.layer?.backgroundColor = baseLayerColor.cgColor
        contentView.addSubview(base, positioned: .below, relativeTo: sidebar)

        // Shadow wrapper — provides elevation shadow without clipping
        let shadowWrapper = NSView()
        shadowWrapper.translatesAutoresizingMaskIntoConstraints = false
        shadowWrapper.wantsLayer = true
        shadowWrapper.layer?.shadowColor = NSColor.black.cgColor
        shadowWrapper.layer?.shadowOpacity = 0.15
        shadowWrapper.layer?.shadowRadius = 8
        shadowWrapper.layer?.shadowOffset = CGSize(width: -2, height: 0)
        contentView.addSubview(shadowWrapper)

        // Content container (Layer 2) — floating panel with rounded corners, clips content.
        let right = NSView()
        right.translatesAutoresizingMaskIntoConstraints = false
        right.wantsLayer = true
        right.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        right.layer?.cornerRadius = Layout.contentPanelCornerRadius
        right.layer?.masksToBounds = true
        shadowWrapper.addSubview(right)
        contentPanelView = right

        // Create navigation toolbar
        let toolbar = MacNavigationToolbar()
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.onNavigationAction = { [weak self] action in
            guard let appId = self?.tabManager.activeTab?.appId else { return }
            if action == "back" {
                let _ = onLxappEvent(appId, LxAppEvent.navigationClick, LxAppEvent.navigationActionBack)
            } else if action == "home" {
                let _ = onLxappEvent(appId, LxAppEvent.navigationClick, LxAppEvent.navigationActionHome)
            }
        }
        navigationToolbar = toolbar
        let workspace = workspaceManager.workspaceView
        workspace.addSubview(toolbar)
        workspaceManager.attachBelowToolbar(toolbar)

        // Wire up WorkspaceManager: panel cards float in contentView as siblings of the WebView
        // card. When a panel opens/closes, the callback fires inside the animation block so
        // the WebView card shrinks/grows in sync with the panel slide-in/out.
        workspaceManager.configure(
            overlayParent: contentView,
            sidebar: sidebar,
            padding: Layout.contentPanelPadding
        ) { [weak self] trailingInset, bottomInset in
            guard let self else { return }
            self.cardTrailingConstraint?.constant = -trailingInset
            self.cardBottomConstraint?.constant = -bottomInset
        }

        configureSidebarRevealButton()
        contentView.addSubview(sidebarRevealButton, positioned: .above, relativeTo: shadowWrapper)

        // Workspace root fills the entire WebView card (toolbar + content, no panel panes).
        let workspaceRoot = workspaceManager.rootView
        workspaceRoot.translatesAutoresizingMaskIntoConstraints = false
        right.addSubview(workspaceRoot)

        // Layout constraints
        let sidebarWidth = sidebar.widthAnchor.constraint(equalToConstant: Layout.sidebarWidth)
        sidebarWidthConstraint = sidebarWidth

        let p = Layout.contentPanelPadding
        let contentLeading = shadowWrapper.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor)
        contentLeadingConstraint = contentLeading
        let cardTrailing = shadowWrapper.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -p)
        cardTrailingConstraint = cardTrailing
        let cardBottom = shadowWrapper.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -p)
        cardBottomConstraint = cardBottom

        NSLayoutConstraint.activate([
            // Base layer: fills entire contentView
            base.topAnchor.constraint(equalTo: contentView.topAnchor),
            base.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            base.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            base.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),

            // Sidebar: left side, full height
            sidebar.topAnchor.constraint(equalTo: contentView.topAnchor),
            sidebar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            sidebar.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            sidebarWidth,

            // Shadow wrapper: floating WebView card (trailing + bottom are dynamic)
            shadowWrapper.topAnchor.constraint(equalTo: contentView.topAnchor, constant: p),
            contentLeading,
            cardTrailing,
            cardBottom,

            // Right container fills shadow wrapper
            right.topAnchor.constraint(equalTo: shadowWrapper.topAnchor),
            right.leadingAnchor.constraint(equalTo: shadowWrapper.leadingAnchor),
            right.trailingAnchor.constraint(equalTo: shadowWrapper.trailingAnchor),
            right.bottomAnchor.constraint(equalTo: shadowWrapper.bottomAnchor),

            // Toolbar: spans full top of workspaceView
            toolbar.topAnchor.constraint(equalTo: workspace.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: workspace.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: workspace.trailingAnchor),

            // Workspace root fills the entire WebView card
            workspaceRoot.topAnchor.constraint(equalTo: right.topAnchor),
            workspaceRoot.leadingAnchor.constraint(equalTo: right.leadingAnchor),
            workspaceRoot.trailingAnchor.constraint(equalTo: right.trailingAnchor),
            workspaceRoot.bottomAnchor.constraint(equalTo: right.bottomAnchor),

            sidebarRevealButton.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: Layout.sidebarRevealButtonLeadingInset),
            sidebarRevealButton.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -Layout.sidebarRevealButtonBottomInset),
            sidebarRevealButton.widthAnchor.constraint(equalToConstant: Layout.sidebarRevealButtonSize.width),
            sidebarRevealButton.heightAnchor.constraint(equalToConstant: Layout.sidebarRevealButtonSize.height),
        ])

        syncSidebarHeaderButtonAlignment()
        refreshSidebarVisibilityUI()
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
        if browserCoordinator.isActive {
            // Coming from a browser tab — force switch back to lxapp
            switchToTab(appId)
        } else if tabManager.activeTab?.appId != appId {
            tabManager.selectTab(appId: appId)
        }
        // Always update sidebar highlight, even if Rust returns early for same index
        sidebarView?.setActiveHighlight(appId: appId, pageIndex: itemIndex)
        // Notify Rust of page navigation via tabbar click
        let _ = onLxappEvent(appId, LxAppEvent.tabBarClick, String(itemIndex))
    }

    private func currentTrafficLightClearance() -> CGFloat {
        guard let window = self.window,
              let contentView = window.contentView else {
            return Layout.trafficLightClearanceFallback
        }

        var maxX: CGFloat = 0
        for type: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
            guard let button = window.standardWindowButton(type), !button.isHidden else { continue }
            let frameInContent = contentView.convert(button.bounds, from: button)
            maxX = max(maxX, frameInContent.maxX)
        }

        if maxX <= 0 {
            return Layout.trafficLightClearanceFallback
        }
        return ceil(maxX + 12)
    }

    private func hideSidebar() {
        setSidebarVisible(false, animated: true)
    }

    private func showSidebar() {
        setSidebarVisible(true, animated: true)
    }

    private func setSidebarVisible(_ visible: Bool, animated: Bool) {
        guard let constraint = sidebarWidthConstraint else { return }

        let isVisible = constraint.constant >= Layout.sidebarHiddenThreshold
        if isVisible == visible {
            refreshSidebarVisibilityUI()
            return
        }

        if isVisible && constraint.constant > Layout.sidebarHiddenThreshold {
            lastExpandedSidebarWidth = constraint.constant
        }

        let targetWidth: CGFloat = visible ? lastExpandedSidebarWidth : 0
        let targetContentLeading: CGFloat = visible ? 0 : Layout.contentPanelPadding

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.25
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = targetWidth
                contentLeadingConstraint?.animator().constant = targetContentLeading
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.refreshSidebarVisibilityUI()
                }
            })
        } else {
            constraint.constant = targetWidth
            contentLeadingConstraint?.constant = targetContentLeading
            refreshSidebarVisibilityUI()
        }
    }

    func updateSidebarWidth(_ width: CGFloat, animated: Bool) {
        guard let constraint = sidebarWidthConstraint else { return }

        if width > Layout.sidebarHiddenThreshold {
            lastExpandedSidebarWidth = width
        }

        let sidebarHidden = width < Layout.sidebarHiddenThreshold
        let targetContentLeading: CGFloat = sidebarHidden ? Layout.contentPanelPadding : 0

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.2
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = width
                contentLeadingConstraint?.animator().constant = targetContentLeading
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.refreshSidebarVisibilityUI()
                }
            })
        } else {
            constraint.constant = width
            contentLeadingConstraint?.constant = targetContentLeading
            refreshSidebarVisibilityUI()
        }
    }

    private func refreshSidebarVisibilityUI() {
        sidebarView?.updateVisibilityState()
        let sidebarHidden = (sidebarWidthConstraint?.constant ?? 0) < Layout.sidebarHiddenThreshold
        contentLeadingConstraint?.constant = sidebarHidden ? Layout.contentPanelPadding : 0
        sidebarRevealButton.isHidden = !sidebarHidden
        syncSidebarHeaderButtonAlignment()
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

    func openLxApp(appId: String, path: String, sessionId: UInt64) {
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

        // Clear browser tab state if switching from a browser tab
        browserCoordinator.deactivate()

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

        // Update sidebar highlight (also clears browser selection)
        sidebarView?.setActiveHighlight(appId: appId)
    }

    private func updateContentView(with viewController: macOSLxAppViewController) {
        currentViewController?.pauseNativeComponents()
        currentViewController?.view.removeFromSuperview()
        currentViewController = viewController

        let container = workspaceManager.contentContainer

        viewController.view.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(viewController.view)

        NSLayoutConstraint.activate([
            viewController.view.topAnchor.constraint(equalTo: container.topAnchor),
            viewController.view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            viewController.view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            viewController.view.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        container.layoutSubtreeIfNeeded()
        os_log("updateContentView appId=%{public}@ frame=(%.1f,%.1f,%.1f,%.1f)",
               log: Self.log, type: .info,
               viewController.appId,
               container.frame.origin.x, container.frame.origin.y,
               container.frame.width, container.frame.height)

        syncSidebarHeaderButtonAlignment()
        viewController.resumeNativeComponents()
    }

    private func syncSidebarHeaderButtonAlignment() {
        guard let contentView = window?.contentView else { return }
        contentView.layoutSubtreeIfNeeded()
        let toolbarCenterY: CGFloat
        if let window = window as? LxAppWindow {
            let effectiveCenterY = window.effectiveTrafficLightCenterYFromTop()
            sidebarView?.buttonCenterYFromTop = effectiveCenterY
            toolbarCenterY = max(0, effectiveCenterY - Layout.contentPanelPadding)
        } else {
            sidebarView?.buttonCenterYFromTop = Layout.toolbarCenterY
            toolbarCenterY = Layout.toolbarCenterY
        }
        browserCoordinator.syncToolbarCenterY(toolbarCenterY)
    }

    func windowDidResize(_ notification: Notification) {
        syncSidebarHeaderButtonAlignment()
    }

    func windowDidMove(_ notification: Notification) {
        syncSidebarHeaderButtonAlignment()
    }

    private func configureSidebarRevealButton() {
        sidebarRevealButton.translatesAutoresizingMaskIntoConstraints = false
        // Use bordered circular bezel — system handles dark/light mode automatically
        sidebarRevealButton.isBordered = true
        sidebarRevealButton.bezelStyle = .circular
        sidebarRevealButton.imagePosition = .imageOnly
        sidebarRevealButton.imageScaling = .scaleProportionallyDown
        sidebarRevealButton.image = NSImage(systemSymbolName: "chevron.right", accessibilityDescription: "Show sidebar")
        sidebarRevealButton.contentTintColor = NSColor.secondaryLabelColor
        sidebarRevealButton.toolTip = "Show sidebar"
        sidebarRevealButton.target = self
        sidebarRevealButton.action = #selector(sidebarRevealButtonClicked)
        sidebarRevealButton.isHidden = true
    }

    @objc private func sidebarRevealButtonClicked() {
        showSidebar()
    }

    // MARK: - QLPreviewPanel support

    override func acceptsPreviewPanelControl(_ panel: QLPreviewPanel!) -> Bool {
        return MainActor.assumeIsolated {
            LxAppMedia.qlController != nil
        }
    }

    override func beginPreviewPanelControl(_ panel: QLPreviewPanel!) {
    }

    override func endPreviewPanelControl(_ panel: QLPreviewPanel!) {
        MainActor.assumeIsolated {
            LxAppMedia.clearQLController()
        }
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

// MARK: - Browser Coordinator Forwarding

extension LxAppWindowController {
    func toggleActiveDevTools() -> Bool {
        browserCoordinator.toggleActiveDevTools()
    }

    func presentInternalBrowserTab(id: UUID) {
        browserCoordinator.presentInternalBrowserTab(id: id)
    }

    @MainActor
    func consumeSelfTargetNavigationInActiveBrowserTab(urlString: String) -> Bool {
        browserCoordinator.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString)
    }
}

// MARK: - BrowserCoordinatorHost

extension LxAppWindowController: BrowserCoordinatorHost {
    var browserContentContainer: NSView { workspaceManager.contentContainer }
    var hostWindow: NSWindow? { window }

    func browserOwnerForNewTab() -> (appId: String, sessionId: UInt64)? {
        if let appId = tabManager.activeTab?.appId {
            let sessionId = appSessions[appId] ?? getLxAppSessionId(appId)
            if sessionId > 0 {
                return (appId, sessionId)
            }
        }
        let current = getCurrentLxApp()
        let appId = current.appid.toString()
        if !appId.isEmpty && current.session_id > 0 {
            return (appId, current.session_id)
        }
        return nil
    }

    func browserWillActivateTab() {
        currentViewController?.pauseNativeComponents()
        currentViewController?.view.removeFromSuperview()
        currentViewController = nil
    }

    func switchToLxAppTab(_ appId: String) {
        switchToTab(appId)
    }

    func activeAppTabId() -> String? {
        tabManager.activeTab?.appId
    }

    func updateSidebarBrowserItems(_ items: [(id: UUID, title: String, favicon: NSImage?)], activeId: UUID?) {
        sidebarView?.updateBrowserItems(items, activeId: activeId)
    }

    func clearSidebarHighlights() {
        sidebarView?.clearAllHighlights()
    }

    func forceHideNavigationToolbar(_ hidden: Bool) {
        navigationToolbar?.forceHide(hidden)
    }

    func trafficLightClearance() -> CGFloat {
        currentTrafficLightClearance()
    }

    func isSidebarCollapsed() -> Bool {
        (sidebarWidthConstraint?.constant ?? Layout.sidebarWidth) < Layout.sidebarHiddenThreshold
    }

    func currentLxAppWebView() -> WKWebView? {
        currentViewController?.currentWebView() ?? LxAppCore.getCurrentWebView()
    }
}

#endif
