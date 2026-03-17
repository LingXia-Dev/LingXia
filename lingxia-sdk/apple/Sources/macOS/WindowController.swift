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
    private static let browserAttachMaxRetry = 5
    private static let browserDevToolsMaxRetry = 30
    private static let browserDevToolsRetryDelay: TimeInterval = 0.05
    private static let lxappDevToolsDetached = true
    private static let lxappDevToolsMaxRetry = 30
    private static let lxappDevToolsRetryDelay: TimeInterval = 0.05

    public struct Layout {
        static let sidebarWidth: CGFloat = 180
        static let sidebarHiddenThreshold: CGFloat = 1
        static let browserToolbarHeight: CGFloat = 38
        /// Shared center-Y baseline for all toolbar elements (traffic lights, nav buttons, address bar).
        /// = toolbar band height / 2 = 38 / 2 = 19pt from the visual window top.
        static let toolbarCenterY: CGFloat = 19
        static let browserButtonSize: CGFloat = 28
        static let browserToolbarIconSize: CGFloat = 14
        static let browserAddressBarHeight: CGFloat = 26
        static let browserButtonLeading: CGFloat = 8
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

    // Browser tab IDs – ownership lives in Rust, titles cached locally from WKWebView KVO.

    private let tabManager = LxAppTabManager.shared
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

    // Browser tab state — source of truth lives in Rust; Swift only keeps UI cache.
    private var activeBrowserTabId: UUID?
    private var browserTabIds: [UUID] = []
    private var browserTabTitles: [UUID: String] = [:]
    private var browserTabFavicons: [UUID: NSImage] = [:]
    private var browserTabFaviconRequestOrigins: [UUID: String] = [:]
    private var browserLastObservedURLs: [UUID: String] = [:]
    private var browserHostView: NSView?
    private let browserToolbar = NSView()
    private let browserToolbarSeparator = NSView()
    private let browserBackButton = NSButton()
    private let browserForwardButton = NSButton()
    private let browserRefreshButton = NSButton()
    private let browserAddressBarContainer = NSView()
    private let browserAddressField = NSTextField()
    private let browserWebContainer = NSView()
    private var activeBrowserWebView: WKWebView?
    private var browserBackButtonLeadingConstraint: NSLayoutConstraint?
    private var browserToolbarCenterYConstraints: [NSLayoutConstraint] = []
    nonisolated(unsafe) private var browserTitleObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var browserUrlObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var browserCanGoBackObservation: NSKeyValueObservation?
    nonisolated(unsafe) private var browserCanGoForwardObservation: NSKeyValueObservation?
    private var browserDevToolsRequestToken: UInt64 = 0
    private var lxappDevToolsRequestToken: UInt64 = 0

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
        browserTitleObservation?.invalidate()
        browserUrlObservation?.invalidate()
        browserCanGoBackObservation?.invalidate()
        browserCanGoForwardObservation?.invalidate()
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

    public func windowWillClose(_ notification: Notification) {
        for (_, viewController) in viewControllers {
            viewController.destroyNativeComponents()
        }
        closeAllBrowserTabs(notifyRust: false)
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
            self?.addBrowserTab()
        }
        sidebar.onOpenSettings = { [weak self] in
            self?.addBrowserTabWithURL("lingxia://settings")
        }
        sidebar.onOpenDownloads = { [weak self] in
            self?.addBrowserTabWithURL("lingxia://downloads")
        }
        sidebar.onBrowserTabSelected = { [weak self] id in
            self?.selectBrowserTab(id: id)
        }
        sidebar.onBrowserTabCloseRequested = { [weak self] id in
            self?.closeBrowserTab(id: id)
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
        let centerPanel = workspaceManager.centerPanelView
        centerPanel.addSubview(toolbar)
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

            // Toolbar: spans full top of centerPanelView
            toolbar.topAnchor.constraint(equalTo: centerPanel.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: centerPanel.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: centerPanel.trailingAnchor),

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
        if activeBrowserTabId != nil {
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

    private func currentBrowserButtonLeading() -> CGFloat {
        let hidden = (sidebarWidthConstraint?.constant ?? Layout.sidebarWidth) < Layout.sidebarHiddenThreshold
        return hidden ? currentTrafficLightClearance() : Layout.browserButtonLeading
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
        let targetLeading: CGFloat = visible ? Layout.browserButtonLeading : currentTrafficLightClearance()
        let targetContentLeading: CGFloat = visible ? 0 : Layout.contentPanelPadding

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.25
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = targetWidth
                browserBackButtonLeadingConstraint?.animator().constant = targetLeading
                contentLeadingConstraint?.animator().constant = targetContentLeading
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.refreshSidebarVisibilityUI()
                }
            })
        } else {
            constraint.constant = targetWidth
            browserBackButtonLeadingConstraint?.constant = targetLeading
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
        let targetLeading: CGFloat = sidebarHidden ? currentTrafficLightClearance() : Layout.browserButtonLeading
        let targetContentLeading: CGFloat = sidebarHidden ? Layout.contentPanelPadding : 0

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.2
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = width
                browserBackButtonLeadingConstraint?.animator().constant = targetLeading
                contentLeadingConstraint?.animator().constant = targetContentLeading
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.refreshSidebarVisibilityUI()
                }
            })
        } else {
            constraint.constant = width
            browserBackButtonLeadingConstraint?.constant = targetLeading
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

        // Clear browser tab state if switching from a browser tab
        if activeBrowserTabId != nil {
            clearBrowserWebViewAttachment()
            hideBrowserHostView()
            activeBrowserTabId = nil
            navigationToolbar?.forceHide(false)
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
        let browserToolbarCenterY: CGFloat
        if let window = window as? LxAppWindow {
            let effectiveCenterY = window.effectiveTrafficLightCenterYFromTop()
            sidebarView?.buttonCenterYFromTop = effectiveCenterY
            browserToolbarCenterY = max(0, effectiveCenterY - Layout.contentPanelPadding)
        } else {
            sidebarView?.buttonCenterYFromTop = Layout.toolbarCenterY
            browserToolbarCenterY = Layout.toolbarCenterY
        }
        browserToolbarCenterYConstraints.forEach { $0.constant = browserToolbarCenterY }
    }

    public func windowDidResize(_ notification: Notification) {
        syncSidebarHeaderButtonAlignment()
    }

    public func windowDidMove(_ notification: Notification) {
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

// MARK: - Browser Tab Lifecycle

extension LxAppWindowController {
    func toggleActiveDevTools() -> Bool {
        if let activeId = activeBrowserTabId {
            browserDevToolsRequestToken &+= 1
            let token = browserDevToolsRequestToken
            return toggleBrowserDevToolsWhenReady(tabId: activeId, attempt: 0, token: token)
        }
        return toggleActiveLxAppDevTools()
    }

    func presentInternalBrowserTab(id: UUID) {
        if !browserTabIds.contains(id) {
            browserTabIds.append(id)
        }
        switchToBrowserTab(id: id)
    }

    private func browserTabIdString(_ id: UUID) -> String {
        id.uuidString.lowercased()
    }

    private func toggleActiveLxAppDevTools() -> Bool {
        guard activeBrowserTabId == nil else { return false }
        let webView = currentViewController?.currentWebView() ?? LxAppCore.getCurrentWebView()
        guard let webView else { return false }
        lxappDevToolsRequestToken &+= 1
        let token = lxappDevToolsRequestToken
        return toggleLxAppDevToolsWhenReady(webView: webView, attempt: 0, token: token)
    }

    @discardableResult
    private func toggleLxAppDevToolsWhenReady(webView: WKWebView, attempt: Int, token: UInt64) -> Bool {
        guard token == lxappDevToolsRequestToken else { return false }
        guard activeBrowserTabId == nil else { return false }

        prepareLxAppWebViewForDevTools(webView, detached: Self.lxappDevToolsDetached)

        guard isBrowserWebViewDisplayReady(webView) else {
            return scheduleLxAppDevToolsRetry(webView: webView, attempt: attempt, token: token)
        }

        let ptr = swiftWebViewPointer(webView)
        return toggleWebViewDevtoolsByPtr(ptr, Self.lxappDevToolsDetached)
    }

    private func prepareLxAppWebViewForDevTools(_ webView: WKWebView, detached: Bool) {
        webView.isHidden = false
        if let container = webView.superview {
            // Keep layout stable for attached inspector (same workaround used by browser mode).
            let constraintsToDeactivate = container.constraints.filter { constraint in
                constraint.firstItem as AnyObject === webView || constraint.secondItem as AnyObject === webView
            }
            if !constraintsToDeactivate.isEmpty {
                NSLayoutConstraint.deactivate(constraintsToDeactivate)
            }
            webView.translatesAutoresizingMaskIntoConstraints = true
            webView.autoresizingMask = [.width, .height]
            webView.frame = container.bounds
            webView.setFrameSize(container.bounds.size)
            container.needsLayout = true
            container.layoutSubtreeIfNeeded()
        }
        webView.needsLayout = true
        webView.layoutSubtreeIfNeeded()
        if detached {
            clearBrowserInspectorAttachment(webView)
        } else {
            configureBrowserInspectorAttachment(webView)
        }
        window?.contentView?.layoutSubtreeIfNeeded()
    }

    private func scheduleLxAppDevToolsRetry(webView: WKWebView, attempt: Int, token: UInt64) -> Bool {
        guard attempt < Self.lxappDevToolsMaxRetry else { return false }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.lxappDevToolsRetryDelay) { [weak self, weak webView] in
            guard let self, let webView else { return }
            _ = self.toggleLxAppDevToolsWhenReady(webView: webView, attempt: attempt + 1, token: token)
        }
        return true
    }

    @discardableResult
    private func toggleBrowserDevToolsWhenReady(tabId: UUID, attempt: Int, token: UInt64) -> Bool {
        guard token == browserDevToolsRequestToken else { return false }
        guard activeBrowserTabId == tabId else { return false }
        guard let webView = findBrowserWKWebView(for: tabId) else {
            return scheduleBrowserDevToolsRetry(tabId: tabId, attempt: attempt, token: token, reason: "webview-missing")
        }

        prepareBrowserWebViewForDevTools(webView)
        guard isBrowserWebViewDisplayReady(webView) else {
            return scheduleBrowserDevToolsRetry(tabId: tabId, attempt: attempt, token: token, reason: "display-not-ready")
        }

        let ptr = swiftWebViewPointer(webView)
        let ok = toggleWebViewDevtoolsByPtr(ptr, false)
        if ok {
            scheduleBrowserDevToolsDetachedFallback(tabId: tabId, webView: webView, token: token)
        }
        return ok
    }

    private func swiftWebViewPointer(_ webView: WKWebView) -> UInt {
        UInt(bitPattern: Unmanaged.passUnretained(webView).toOpaque())
    }

    private func scheduleBrowserDevToolsRetry(tabId: UUID, attempt: Int, token: UInt64, reason: String) -> Bool {
        guard attempt < Self.browserDevToolsMaxRetry else {
            os_log(
                "toggleBrowserDevToolsWhenReady timed out after %d attempts for tab=%{public}@ reason=%{public}@",
                log: Self.log,
                type: .error,
                attempt,
                browserTabIdString(tabId),
                reason
            )
            return false
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.browserDevToolsRetryDelay) { [weak self] in
            _ = self?.toggleBrowserDevToolsWhenReady(tabId: tabId, attempt: attempt + 1, token: token)
        }
        return true
    }

    private func prepareBrowserWebViewForDevTools(_ webView: WKWebView) {
        showBrowserHostView()
        if webView.superview !== browserWebContainer {
            attachBrowserWebViewToContainer(webView)
            activeBrowserWebView = webView
            observeActiveBrowserWebView(webView)
        }
        webView.isHidden = false
        configureBrowserInspectorAttachment(webView)
        window?.contentView?.layoutSubtreeIfNeeded()
    }

    private func attachBrowserWebViewToContainer(_ webView: WKWebView) {
        let constraintsToDeactivate = browserWebContainer.constraints.filter { constraint in
            constraint.firstItem as AnyObject === webView || constraint.secondItem as AnyObject === webView
        }
        if !constraintsToDeactivate.isEmpty {
            NSLayoutConstraint.deactivate(constraintsToDeactivate)
        }

        if webView.superview !== browserWebContainer {
            webView.removeFromSuperview()
            browserWebContainer.addSubview(webView)
        }

        // WebKit's attached inspector on macOS does not reliably handle Auto Layout-managed WKWebViews.
        webView.translatesAutoresizingMaskIntoConstraints = true
        webView.autoresizingMask = [.width, .height]
        webView.frame = browserWebContainer.bounds
        webView.setFrameSize(browserWebContainer.bounds.size)

        browserWebContainer.needsLayout = true
        browserWebContainer.layoutSubtreeIfNeeded()
        webView.needsLayout = true
        webView.layoutSubtreeIfNeeded()
        webView.resumeWebView()

        if let appId = webView.appId, let path = webView.currentPath {
            lingxia.onPageShow(appId, path)
        }
    }

    private func configureBrowserInspectorAttachment(_ webView: WKWebView) {
        let setSelector = NSSelectorFromString("_setInspectorAttachmentView:")
        guard webView.responds(to: setSelector) else { return }
        _ = webView.perform(setSelector, with: webView)
    }

    private func clearBrowserInspectorAttachment(_ webView: WKWebView) {
        let setSelector = NSSelectorFromString("_setInspectorAttachmentView:")
        guard webView.responds(to: setSelector) else { return }
        _ = webView.perform(setSelector, with: nil)
    }

    private func scheduleBrowserDevToolsDetachedFallback(tabId: UUID, webView: WKWebView, token: UInt64) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self, weak webView] in
            guard let self, let webView else { return }
            guard token == self.browserDevToolsRequestToken else { return }
            guard self.activeBrowserTabId == tabId else { return }
            guard self.isBrowserWebViewDisplayReady(webView) else { return }
            guard self.inspectorVisible(for: webView) == false else { return }
            _ = toggleWebViewDevtoolsByPtr(self.swiftWebViewPointer(webView), true)
        }
    }

    private func inspectorVisible(for webView: WKWebView) -> Bool? {
        let inspectorSelector = NSSelectorFromString("_inspector")
        guard webView.responds(to: inspectorSelector),
              let inspectorObject = webView.perform(inspectorSelector)?.takeUnretainedValue() else {
            return nil
        }
        let visibleSelector = NSSelectorFromString("isVisible")
        guard inspectorObject.responds(to: visibleSelector),
              let visibleObject = inspectorObject.perform(visibleSelector)?.takeUnretainedValue() else {
            return nil
        }
        if let number = visibleObject as? NSNumber {
            return number.boolValue
        }
        return nil
    }

    private func isBrowserWebViewDisplayReady(_ webView: WKWebView) -> Bool {
        guard webView.superview != nil else { return false }
        guard let window = webView.window, window.isVisible else { return false }
        guard window.screen != nil else { return false }
        if webView.isHidden || webView.isHiddenOrHasHiddenAncestor {
            return false
        }
        let bounds = webView.bounds.integral
        guard bounds.width > 1, bounds.height > 1 else { return false }
        return true
    }

    /// Find the WKWebView for a browser tab via Rust-owned app/path/session mapping.
    private func findBrowserWKWebView(for id: UUID) -> WKWebView? {
        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else {
            return nil
        }
        let path = browserTabPathForId(browserTabIdString(id)).toString()
        return WebViewManager.findWebView(
            appId: appId,
            path: path,
            sessionId: sessionId
        )
    }

    private func browserSidebarItems() -> [(id: UUID, title: String, favicon: NSImage?)] {
        browserTabIds.map { id in
            (id, browserTabTitles[id] ?? "New Tab", browserTabFavicons[id])
        }
    }

    private func faviconRequestOrigin(for url: URL) -> String? {
        guard let scheme = url.scheme?.lowercased(),
              (scheme == "http" || scheme == "https"),
              let host = url.host?.lowercased() else {
            return nil
        }
        let port: String
        if let rawPort = url.port, !((scheme == "http" && rawPort == 80) || (scheme == "https" && rawPort == 443)) {
            port = ":\(rawPort)"
        } else {
            port = ""
        }
        return "\(scheme)://\(host)\(port)"
    }

    private func fetchFavicon(for origin: String, tabId: UUID) {
        guard let faviconURL = URL(string: "\(origin)/favicon.ico") else { return }
        URLSession.shared.dataTask(with: faviconURL) { [weak self] data, response, _ in
            guard let data,
                  let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200,
                  let contentType = httpResponse.value(forHTTPHeaderField: "Content-Type"),
                  !contentType.hasPrefix("text/"),
                  let image = NSImage(data: data), image.isValid else { return }
            Task { @MainActor in
                guard let self,
                      self.browserTabIds.contains(tabId),
                      self.browserTabFaviconRequestOrigins[tabId] == origin else { return }
                self.browserTabFavicons[tabId] = image
                self.sidebarView?.updateBrowserItems(self.browserSidebarItems(), activeId: self.activeBrowserTabId)
            }
        }.resume()
    }

    private func browserOwnerForNewTab() -> (appId: String, sessionId: UInt64)? {
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

    private func setupBrowserHostIfNeeded() {
        guard browserHostView == nil else { return }

        let host = NSView()
        host.translatesAutoresizingMaskIntoConstraints = false
        host.wantsLayer = true

        browserToolbar.translatesAutoresizingMaskIntoConstraints = false
        browserToolbar.wantsLayer = true
        browserToolbar.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        host.addSubview(browserToolbar)

        configureBrowserButton(browserBackButton, iconName: "icon_back", action: #selector(browserBackClicked))
        browserToolbar.addSubview(browserBackButton)

        configureBrowserButton(browserForwardButton, iconName: "icon_forward", action: #selector(browserForwardClicked))
        browserForwardButton.isEnabled = false
        browserForwardButton.alphaValue = 0.4
        browserToolbar.addSubview(browserForwardButton)

        configureBrowserButton(browserRefreshButton, iconName: "icon_browser_refresh", action: #selector(browserRefreshClicked))
        browserToolbar.addSubview(browserRefreshButton)

        browserAddressBarContainer.translatesAutoresizingMaskIntoConstraints = false
        browserAddressBarContainer.wantsLayer = true
        browserAddressBarContainer.layer?.cornerRadius = 6
        browserAddressBarContainer.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        browserToolbar.addSubview(browserAddressBarContainer)

        browserAddressField.translatesAutoresizingMaskIntoConstraints = false
        browserAddressField.font = NSFont.systemFont(ofSize: 13)
        browserAddressField.placeholderString = "Enter URL"
        browserAddressField.isBordered = false
        browserAddressField.drawsBackground = false
        browserAddressField.focusRingType = .none
        browserAddressField.usesSingleLineMode = true
        browserAddressField.cell?.wraps = false
        browserAddressField.cell?.isScrollable = true
        browserAddressField.cell?.lineBreakMode = .byTruncatingTail
        browserAddressField.target = self
        browserAddressField.action = #selector(browserAddressSubmitted(_:))
        browserAddressBarContainer.addSubview(browserAddressField)

        browserToolbarSeparator.translatesAutoresizingMaskIntoConstraints = false
        browserToolbarSeparator.wantsLayer = true
        browserToolbarSeparator.layer?.backgroundColor = NSColor.separatorColor.cgColor
        host.addSubview(browserToolbarSeparator)

        browserWebContainer.translatesAutoresizingMaskIntoConstraints = false
        browserWebContainer.wantsLayer = true
        host.addSubview(browserWebContainer)

        let browserBackCenterY = browserBackButton.centerYAnchor.constraint(equalTo: browserToolbar.topAnchor, constant: Layout.toolbarCenterY)
        let browserForwardCenterY = browserForwardButton.centerYAnchor.constraint(equalTo: browserToolbar.topAnchor, constant: Layout.toolbarCenterY)
        let browserRefreshCenterY = browserRefreshButton.centerYAnchor.constraint(equalTo: browserToolbar.topAnchor, constant: Layout.toolbarCenterY)
        let browserAddressCenterY = browserAddressBarContainer.centerYAnchor.constraint(equalTo: browserToolbar.topAnchor, constant: Layout.toolbarCenterY)
        browserToolbarCenterYConstraints = [browserBackCenterY, browserForwardCenterY, browserRefreshCenterY, browserAddressCenterY]

        NSLayoutConstraint.activate([
            browserToolbar.topAnchor.constraint(equalTo: host.topAnchor),
            browserToolbar.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            browserToolbar.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            browserToolbar.heightAnchor.constraint(equalToConstant: Layout.browserToolbarHeight),

            {
                let c = browserBackButton.leadingAnchor.constraint(equalTo: browserToolbar.leadingAnchor, constant: currentBrowserButtonLeading())
                browserBackButtonLeadingConstraint = c
                return c
            }(),
            // All nav buttons and address bar share the traffic-light baseline.
            browserBackCenterY,
            browserBackButton.widthAnchor.constraint(equalToConstant: Layout.browserButtonSize),
            browserBackButton.heightAnchor.constraint(equalToConstant: Layout.browserButtonSize),

            browserForwardButton.leadingAnchor.constraint(equalTo: browserBackButton.trailingAnchor, constant: 4),
            browserForwardCenterY,
            browserForwardButton.widthAnchor.constraint(equalToConstant: Layout.browserButtonSize),
            browserForwardButton.heightAnchor.constraint(equalToConstant: Layout.browserButtonSize),

            browserRefreshButton.leadingAnchor.constraint(equalTo: browserForwardButton.trailingAnchor, constant: 4),
            browserRefreshCenterY,
            browserRefreshButton.widthAnchor.constraint(equalToConstant: Layout.browserButtonSize),
            browserRefreshButton.heightAnchor.constraint(equalToConstant: Layout.browserButtonSize),

            browserAddressBarContainer.leadingAnchor.constraint(equalTo: browserRefreshButton.trailingAnchor, constant: 8),
            browserAddressBarContainer.trailingAnchor.constraint(equalTo: browserToolbar.trailingAnchor, constant: -8),
            browserAddressCenterY,
            browserAddressBarContainer.heightAnchor.constraint(equalToConstant: Layout.browserAddressBarHeight),

            browserAddressField.leadingAnchor.constraint(equalTo: browserAddressBarContainer.leadingAnchor, constant: 8),
            browserAddressField.trailingAnchor.constraint(equalTo: browserAddressBarContainer.trailingAnchor, constant: -8),
            browserAddressField.centerYAnchor.constraint(equalTo: browserAddressBarContainer.centerYAnchor),

            browserToolbarSeparator.topAnchor.constraint(equalTo: browserToolbar.bottomAnchor),
            browserToolbarSeparator.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            browserToolbarSeparator.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            browserToolbarSeparator.heightAnchor.constraint(equalToConstant: 1),

            browserWebContainer.topAnchor.constraint(equalTo: browserToolbarSeparator.bottomAnchor),
            browserWebContainer.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            browserWebContainer.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            browserWebContainer.bottomAnchor.constraint(equalTo: host.bottomAnchor),
        ])

        browserHostView = host
        updateBrowserBackButtonState(canGoBack: false)
    }

    private func showBrowserHostView() {
        setupBrowserHostIfNeeded()
        guard let host = browserHostView else { return }
        let container = workspaceManager.contentContainer

        if host.superview !== container {
            container.addSubview(host)
            NSLayoutConstraint.activate([
                host.topAnchor.constraint(equalTo: container.topAnchor),
                host.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                host.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                host.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
        }

        window?.contentView?.layoutSubtreeIfNeeded()
    }

    private func hideBrowserHostView() {
        browserHostView?.removeFromSuperview()
    }

    private func updateBrowserBackButtonState(canGoBack: Bool) {
        browserBackButton.isEnabled = canGoBack
        browserBackButton.alphaValue = canGoBack ? 1.0 : 0.4
    }

    private func updateBrowserForwardButtonState(canGoForward: Bool) {
        browserForwardButton.isEnabled = canGoForward
        browserForwardButton.alphaValue = canGoForward ? 1.0 : 0.4
    }

    private func observeActiveBrowserWebView(_ webView: WKWebView) {
        browserTitleObservation?.invalidate()
        browserUrlObservation?.invalidate()
        browserCanGoBackObservation?.invalidate()
        browserCanGoForwardObservation?.invalidate()

        browserTitleObservation = webView.observe(\.title, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, let activeId = self.activeBrowserTabId else { return }
                let title = (webView.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !title.isEmpty {
                    self.handleBrowserTitleChanged(id: activeId, title: title)
                }
                _ = updateBrowserTabInfo(self.browserTabIdString(activeId), webView.url?.absoluteString ?? "", webView.title ?? "")
            }
        }

        browserUrlObservation = webView.observe(\.url, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                guard let self, let activeId = self.activeBrowserTabId else { return }
                let rawURL = webView.url?.absoluteString ?? ""
                if self.browserLastObservedURLs[activeId] == rawURL {
                    return
                }
                self.browserLastObservedURLs[activeId] = rawURL

                if self.browserAddressField.currentEditor() == nil {
                    self.browserAddressField.stringValue = self.displayableBrowserURL(rawURL)
                }
                if let origin = webView.url.flatMap({ self.faviconRequestOrigin(for: $0) }) {
                    if origin != self.browserTabFaviconRequestOrigins[activeId] {
                        self.browserTabFavicons.removeValue(forKey: activeId)
                        self.browserTabFaviconRequestOrigins[activeId] = origin
                        self.sidebarView?.updateBrowserItems(self.browserSidebarItems(), activeId: activeId)
                    }
                    if self.browserTabFavicons[activeId] == nil {
                        self.fetchFavicon(for: origin, tabId: activeId)
                    }
                }
                _ = updateBrowserTabInfo(self.browserTabIdString(activeId), webView.url?.absoluteString ?? "", webView.title ?? "")
            }
        }

        browserCanGoBackObservation = webView.observe(\.canGoBack, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                self?.updateBrowserBackButtonState(canGoBack: webView.canGoBack)
            }
        }

        browserCanGoForwardObservation = webView.observe(\.canGoForward, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor in
                self?.updateBrowserForwardButtonState(canGoForward: webView.canGoForward)
            }
        }
    }

    private func displayableBrowserURL(_ raw: String?) -> String {
        guard let raw else { return "" }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        return browserUrlIsHidden(trimmed) ? "" : trimmed
    }

    private func openAddressInActiveBrowserTab(_ urlString: String) -> Bool {
        guard let webView = activeBrowserWebView,
              let url = URL(string: urlString) else { return false }
        browserAddressField.stringValue = urlString
        webView.load(URLRequest(url: url))
        return true
    }

    @MainActor
    func consumeSelfTargetNavigationInActiveBrowserTab(urlString: String) -> Bool {
        guard activeBrowserTabId != nil else { return false }
        guard let webView = activeBrowserWebView else { return false }
        let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !displayableBrowserURL(trimmed).isEmpty else { return true }
        if webView.url?.absoluteString == trimmed { return true }
        return openAddressInActiveBrowserTab(trimmed)
    }

    @objc private func browserBackClicked() {
        guard let webView = activeBrowserWebView, webView.canGoBack else { return }
        webView.goBack()
    }

    @objc private func browserForwardClicked() {
        guard let webView = activeBrowserWebView, webView.canGoForward else { return }
        webView.goForward()
    }

    @objc private func browserRefreshClicked() {
        activeBrowserWebView?.reload()
    }

    @objc private func browserAddressSubmitted(_ sender: NSTextField) {
        guard let result = handleBrowserAddressSubmission(
            rawInput: sender.stringValue,
            currentURL: activeBrowserWebView?.url?.absoluteString,
            tabId: activeBrowserTabId?.uuidString
        ) else { return }
        _ = openAddressInActiveBrowserTab(result.url)
    }

    private func clearBrowserWebViewAttachment() {
        browserTitleObservation?.invalidate()
        browserUrlObservation?.invalidate()
        browserCanGoBackObservation?.invalidate()
        browserCanGoForwardObservation?.invalidate()
        browserTitleObservation = nil
        browserUrlObservation = nil
        browserCanGoBackObservation = nil
        browserCanGoForwardObservation = nil
        if let activeBrowserWebView {
            clearBrowserInspectorAttachment(activeBrowserWebView)
        }
        activeBrowserWebView?.removeFromSuperview()
        activeBrowserWebView = nil
        updateBrowserBackButtonState(canGoBack: false)
        updateBrowserForwardButtonState(canGoForward: false)
    }

    private func closeAllBrowserTabs(notifyRust: Bool = true) {
        if notifyRust {
            for id in browserTabIds {
                _ = browserTabClose(browserTabIdString(id))
            }
        }
        clearBrowserWebViewAttachment()
        browserTabIds.removeAll()
        browserTabTitles.removeAll()
        browserTabFavicons.removeAll()
        browserTabFaviconRequestOrigins.removeAll()
        browserLastObservedURLs.removeAll()
        activeBrowserTabId = nil
        hideBrowserHostView()
        sidebarView?.updateBrowserItems([], activeId: nil)
    }

    private func addBrowserTabWithURL(_ url: String) {
        guard let owner = browserOwnerForNewTab() else {
            os_log("Cannot create browser tab without active lxapp session", log: Self.log, type: .error)
            return
        }

        guard let openedTab = openBrowserTab(owner.appId, owner.sessionId, url) else {
            os_log(
                "openBrowserTab failed for %{public}@/%{public}llu url=%{public}@",
                log: Self.log,
                type: .error,
                owner.appId,
                owner.sessionId,
                url
            )
            return
        }

        let tabId = openedTab.toString().lowercased()
        guard let id = UUID(uuidString: tabId) else {
            os_log(
                "openBrowserTab returned invalid tab id=%{public}@ for %{public}@/%{public}llu",
                log: Self.log,
                type: .error,
                tabId,
                owner.appId,
                owner.sessionId,
            )
            return
        }

        presentInternalBrowserTab(id: id)
    }

    private func addBrowserTab() {
        addBrowserTabWithURL("")
    }

    private func selectBrowserTab(id: UUID) {
        switchToBrowserTab(id: id)
        sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: id)
    }

    private func switchToBrowserTab(id: UUID) {
        guard browserTabIds.contains(id) else { return }
        if activeBrowserTabId == id {
            sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: id)
            return
        }

        // Pause current lxapp VC if any
        currentViewController?.pauseNativeComponents()
        currentViewController?.view.removeFromSuperview()
        currentViewController = nil

        clearBrowserWebViewAttachment()

        activeBrowserTabId = id

        // Clear lxapp highlights, set browser highlight
        sidebarView?.clearAllHighlights()
        sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: id)

        // Hide lxapp navigation toolbar when browser tab is active.
        navigationToolbar?.forceHide(true)

        showBrowserHostView()
        browserAddressField.stringValue = ""
        updateBrowserBackButtonState(canGoBack: false)

        attachBrowserWebView(for: id, attempt: 0)
    }

    private func attachBrowserWebView(for tabId: UUID, attempt: Int) {
        guard activeBrowserTabId == tabId else { return }

        if let webView = findBrowserWKWebView(for: tabId) {
            if #available(macOS 13.3, *) {
                webView.isInspectable = true
            }
            // Enables "Inspect Element" in the contextual menu on macOS.
            webView.configuration.preferences.setValue(true, forKey: "developerExtrasEnabled")
            showBrowserHostView()
            attachBrowserWebViewToContainer(webView)
            configureBrowserInspectorAttachment(webView)
            activeBrowserWebView = webView
            observeActiveBrowserWebView(webView)
            browserAddressField.stringValue = displayableBrowserURL(webView.url?.absoluteString)
            updateBrowserBackButtonState(canGoBack: webView.canGoBack)
            return
        }

        // WebView not ready yet — retry or give up.
        guard attempt < Self.browserAttachMaxRetry else {
            os_log("Failed to attach browser webview after %d retries for tab=%{public}@",
                   log: Self.log, type: .error, attempt, browserTabIdString(tabId))
            if activeBrowserTabId == tabId {
                clearBrowserWebViewAttachment()
                hideBrowserHostView()
                activeBrowserTabId = nil
                navigationToolbar?.forceHide(false)
                sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: nil)
                if let activeTab = tabManager.activeTab {
                    switchToTab(activeTab.appId)
                }
            }
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            self?.attachBrowserWebView(for: tabId, attempt: attempt + 1)
        }
    }

    private func closeBrowserTab(id: UUID) {
        guard let index = browserTabIds.firstIndex(of: id) else { return }

        // Detach WebView from UI BEFORE Rust destroy to prevent ObjC exceptions
        // during WebViewInner::Drop (removeFromSuperview/release on attached view).
        if activeBrowserTabId == id {
            clearBrowserWebViewAttachment()
        }

        // Remove from Swift state
        browserTabTitles.removeValue(forKey: id)
        browserTabFavicons.removeValue(forKey: id)
        browserTabFaviconRequestOrigins.removeValue(forKey: id)
        browserLastObservedURLs.removeValue(forKey: id)
        browserTabIds.remove(at: index)

        // Destroy Rust state (triggers WebView Drop — safe now that UI is detached)
        _ = browserTabClose(browserTabIdString(id))

        if activeBrowserTabId == id {
            activeBrowserTabId = nil

            // Switch to another browser tab or the last lxapp tab
            if let lastBrowser = browserTabIds.last {
                switchToBrowserTab(id: lastBrowser)
                sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: lastBrowser)
            } else if let activeTab = tabManager.activeTab {
                navigationToolbar?.forceHide(false)
                hideBrowserHostView()
                switchToTab(activeTab.appId)
            } else {
                navigationToolbar?.forceHide(false)
                hideBrowserHostView()
            }
        }

        sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: activeBrowserTabId)
    }

    private func handleBrowserTitleChanged(id: UUID, title: String) {
        guard browserTabIds.contains(id) else { return }
        if browserTabTitles[id] == title {
            return
        }
        browserTabTitles[id] = title
        sidebarView?.updateBrowserItems(browserSidebarItems(), activeId: activeBrowserTabId)
    }

    private func configureBrowserButton(_ button: NSButton, iconName: String, action: Selector) {
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.imagePosition = .imageOnly
        button.imageScaling = .scaleProportionallyDown
        button.target = self
        button.action = action

        button.image = loadBrowserToolbarIcon(named: iconName, size: Layout.browserToolbarIconSize)
        button.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
    }

    private func loadBrowserToolbarIcon(named iconName: String, size: CGFloat) -> NSImage? {
        return LxIcon.image(named: iconName, size: CGSize(width: size, height: size))
    }
}

#endif
