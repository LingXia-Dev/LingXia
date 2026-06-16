import Foundation
import OSLog

#if os(macOS)
import AppKit
import SwiftUI
import WebKit
import Quartz
import CLingXiaRustAPI

private let lxShellTerminalOSLog = OSLog(subsystem: "LingXia", category: "TerminalShell")

private func lxShellStdoutLog(_ message: String, level: Int32 = 2) {
    let type: OSLogType = level >= 4 ? .error : .info
    let debugEnabled = ProcessInfo.processInfo.environment["LX_TERMINAL_DEBUG_LOGS"] == "1"
    if debugEnabled || type == .error || type == .fault {
        os_log("%{public}@", log: lxShellTerminalOSLog, type: type, message)
    }
    guard ProcessInfo.processInfo.environment["LX_TERMINAL_STDOUT_LOGS"] == "1" else {
        return
    }
    let line = "[LingXia][Shell] \(message)\n"
    FileHandle.standardOutput.write(Data(line.utf8))
    NSLog("%@", line.trimmingCharacters(in: .newlines))
}

private func lxShellFormatRect(_ rect: NSRect) -> String {
    String(
        format: "%.0f,%.0f %.0fx%.0f",
        rect.minX,
        rect.minY,
        rect.width,
        rect.height
    )
}

@MainActor
enum LxAppShellStartupBehavior {
    case automaticHome
    case managedByAppUI
}

@MainActor
final class MacTitlebarActionStrip: NSView {
    private let stackView = NSStackView()
    private var buttons: [NSButton] = []
    private var widthConstraint: NSLayoutConstraint?

    var onAction: ((String) -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupViews() {
        stackView.translatesAutoresizingMaskIntoConstraints = false
        stackView.orientation = .horizontal
        stackView.alignment = .centerY
        stackView.spacing = 2
        addSubview(stackView)

        NSLayoutConstraint.activate([
            stackView.centerYAnchor.constraint(equalTo: centerYAnchor),
            stackView.trailingAnchor.constraint(equalTo: trailingAnchor),
            stackView.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor),
            heightAnchor.constraint(equalToConstant: 22),
        ])
        let width = widthAnchor.constraint(equalToConstant: 0)
        width.isActive = true
        widthConstraint = width
    }

    func updateActions(_ items: [LxAppUIActionItem]) {
        buttons.forEach { button in
            stackView.removeArrangedSubview(button)
            button.removeFromSuperview()
        }
        buttons.removeAll()
        widthConstraint?.constant = CGFloat(items.count * 20 + max(0, items.count - 1) * 2)

        for item in items.reversed() {
            let button = NSButton()
            button.translatesAutoresizingMaskIntoConstraints = false
            button.isBordered = false
            button.bezelStyle = .regularSquare
            button.imagePosition = .imageOnly
            button.imageScaling = .scaleProportionallyDown
            button.toolTip = item.label
            button.identifier = NSUserInterfaceItemIdentifier(item.id)
            button.target = self
            button.action = #selector(actionClicked(_:))

            if let iconURL = item.iconURL,
               let image = NSImage(contentsOf: iconURL) {
                image.size = NSSize(width: 14, height: 14)
                image.isTemplate = true
                button.image = image
                button.contentTintColor = NSColor.secondaryLabelColor
            } else {
                let fallback = NSImage(systemSymbolName: "square.grid.2x2", accessibilityDescription: item.label)
                fallback?.size = NSSize(width: 14, height: 14)
                button.image = fallback
                button.contentTintColor = NSColor.secondaryLabelColor
            }

            NSLayoutConstraint.activate([
                button.widthAnchor.constraint(equalToConstant: 20),
                button.heightAnchor.constraint(equalToConstant: 20),
            ])

            stackView.addArrangedSubview(button)
            buttons.append(button)
        }
    }

    @objc private func actionClicked(_ sender: NSButton) {
        guard let actionID = sender.identifier?.rawValue else { return }
        onAction?(actionID)
    }
}

/// The main integration point for macOS apps that want the default LingXia
/// chrome (sidebar + toolbar + floating content panel).
///
/// For fully custom hosts, use `LxAppController` + `LxAppHostView` directly.
///
/// ```swift
/// let runtime = try LxAppRuntime.shared.initialize()
/// let controller = LxAppController()
///
/// var config = LxAppShellConfiguration()
/// config.sidebar = .declarative(mySidebarTree)
///
/// let shell = LxAppShell(controller: controller, configuration: config)
/// shell.show()
/// ```
@MainActor
public final class LxAppShell: NSWindowController, NSWindowDelegate {

    // MARK: - Layout Constants

    struct Layout {
        static let sidebarWidth: CGFloat = 180
        static let sidebarHiddenThreshold: CGFloat = 1
        static let toolbarCenterY: CGFloat = 19
        static let trafficLightClearanceFallback: CGFloat = 80
        static let contentPanelPadding: CGFloat = 6
        static let contentPanelCornerRadius: CGFloat = 10
        static let sidebarRevealButtonSize = CGSize(width: 20, height: 28)
        static let sidebarRevealButtonLeadingInset: CGFloat = 0
        static let sidebarRevealButtonBottomInset: CGFloat = 4
    }

    // MARK: - Properties

    public let controller: LxAppController
    public private(set) var configuration: LxAppShellConfiguration
    public let hostView: LxAppHostView

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppShell")

    private var baseLayerColor: NSColor = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            ? NSColor(red: 0.16, green: 0.16, blue: 0.18, alpha: 1)
            : NSColor(red: 0.90, green: 0.90, blue: 0.92, alpha: 1)
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
    private var controllerEventsTask: Task<Void, Never>?
    private var didRequestHomeOpen = false
    private let startupBehavior: LxAppShellStartupBehavior
    private var sidebarHostActionHandler: ((String) -> Void)?
    private var toolbarHostActionHandler: ((String) -> Void)?
    private var titlebarHostActionHandler: ((String) -> Void)?
    private var appUIRuntimeRef: AnyObject?
    private var titlebarActionStrip: MacTitlebarActionStrip?
    private var titlebarAccessoryController: NSTitlebarAccessoryViewController?
    private var usesPanelPresentation = false
    private var sidebarChromeEnabled = true

    var onManagedWindowCloseRequested: (() -> Void)?

    private(set) var contentPanelView: NSView?

    func getViewController(for appId: String) -> macOSLxAppViewController? {
        if let viewController = viewControllers[appId] {
            return viewController
        }
        if let currentViewController, currentViewController.appId == appId {
            return currentViewController
        }
        return nil
    }

    func ensureViewController(for appId: String, path: String) -> macOSLxAppViewController? {
        if let viewController = getViewController(for: appId) {
            return viewController
        }
        let resolvedSessionId: UInt64? = {
            if let sessionId = appSessions[appId], sessionId > 0 {
                return sessionId
            }
            if let sessionId = LxAppCore.sessionId(for: appId), sessionId > 0 {
                return sessionId
            }
            let sessionId = getLxAppSessionId(appId)
            return sessionId > 0 ? sessionId : nil
        }()

        guard let sessionId = resolvedSessionId else {
            return nil
        }

        appSessions[appId] = sessionId
        LxAppCore.setSessionId(sessionId, for: appId)
        let viewController = macOSLxAppViewController(
            appId: appId,
            path: path,
            sessionId: sessionId
        )
        viewControllers[appId] = viewController
        updateContentView(with: viewController)
        return viewController
    }

    // MARK: - Init

    public convenience init(
        controller: LxAppController = LxAppController(),
        configuration: LxAppShellConfiguration = LxAppShellConfiguration()
    ) {
        self.init(
            controller: controller,
            configuration: configuration,
            startupBehavior: .automaticHome
        )
    }

    internal init(
        controller: LxAppController = LxAppController(),
        configuration: LxAppShellConfiguration = LxAppShellConfiguration(),
        startupBehavior: LxAppShellStartupBehavior
    ) {
        self.controller = controller
        self.configuration = configuration
        self.hostView = LxAppHostView(controller: controller)
        self.startupBehavior = startupBehavior

        let window = Self.createWindow()
        super.init(window: window)
        LxAppActiveHost.activate(shell: self)
        browserCoordinator.host = self
        setupTabMode()
        observeControllerEvents()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        sidebarRefreshObserver.map(NotificationCenter.default.removeObserver)
        controllerEventsTask?.cancel()
        browserCoordinator.cleanup()
    }

    // MARK: - Configuration

    public func updateConfiguration(_ newConfig: LxAppShellConfiguration) {
        let oldConfig = configuration
        configuration = newConfig

        if oldConfig.sidebar != newConfig.sidebar {
            applySidebarMode(newConfig.sidebar)
        }
        if oldConfig.toolbar != newConfig.toolbar {
            applyToolbarMode(newConfig.toolbar)
        }
        if oldConfig.chrome != newConfig.chrome {
            applyChromeStyle(newConfig.chrome)
        }

        os_log("Shell configuration updated", log: Self.log, type: .info)
    }

    // MARK: - Show / Hide

    public func show() {
        showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
        guard startupBehavior == .automaticHome,
              !didRequestHomeOpen,
              !tabManager.hasTabs else { return }
        didRequestHomeOpen = true
        Task { @MainActor [controller] in
            _ = try? await controller.openHomeApp()
        }
    }

    public func hide() {
        window?.orderOut(nil)
    }

    // MARK: - Window Creation

    private static func createWindow() -> LxAppWindow {
        let window = LxAppWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.contentMinSize = CGSize(width: 720, height: 480)
        window.minSize = CGSize(width: 720, height: 480)
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
            guard let self else { return }
            self.switchToTab(tab.appId)
        }

        tabManager.onTabsChanged = { [weak self] tabs in
            guard let self else { return }
            self.sidebarView?.updateForTabs(tabs, activeTab: self.tabManager.activeTab)
        }

        setupSidebarInterface()
        setupNotificationObservers()
        applySidebarMode(configuration.sidebar)
        applyToolbarMode(configuration.toolbar)
        applyChromeStyle(configuration.chrome)
        if startupBehavior == .automaticHome {
            setupInitialTab()
        }
    }

    // MARK: - NSWindowDelegate

    public func windowWillClose(_ notification: Notification) {
        for (_, viewController) in viewControllers {
            viewController.destroyNativeComponents()
        }
        browserCoordinator.closeAllTabs(notifyRust: false)
        for tab in tabManager.tabs {
            if let sessionId = appSessions[tab.appId], sessionId > 0 {
                let accepted = onLxappClosed(tab.appId, sessionId)
                if !accepted {
                    os_log("Ignoring stale close callback during cleanup for %@ (session=%{public}llu)",
                           log: Self.log, type: .info, tab.appId, sessionId)
                }
            }
            LxAppCore.removeSessionId(for: tab.appId)
        }
        LxAppActiveHost.clear(shell: self)
    }

    public func windowShouldClose(_ sender: NSWindow) -> Bool {
        guard startupBehavior == .managedByAppUI else { return true }
        onManagedWindowCloseRequested?()
        return false
    }

    public func windowDidResize(_ notification: Notification) {
        syncSidebarHeaderButtonAlignment()
        workspaceManager.relayoutPanels()
        reportSurfaceWidth()
    }

    /// Report the content width to the Adaptive Surface Layout core so it
    /// resolves the sizeClass (with hysteresis). This is what makes macOS
    /// drive the new shared-core model from real window geometry.
    private func reportSurfaceWidth() {
        guard let appId = currentViewController?.appId,
              let width = window?.contentView?.frame.width, width > 0 else { return }
        _ = setSurfaceWidth(appId, Double(width))
    }

    // MARK: - Sidebar Interface Setup

    private func setupSidebarInterface() {
        guard let window = self.window, let contentView = window.contentView else { return }

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
        sidebar.trafficLightClearanceProvider = { [weak self] in
            self?.trafficLightClearance() ?? SidebarView.Layout.railWidth
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
        sidebar.onPanelItemToggled = { [weak self] actionID in
            self?.sidebarHostActionHandler?(actionID)
        }
        sidebar.onUpdateActionRequested = { state in
            switch state {
            case .ready:
                _ = onAppEvent(AppEvent.updateRestartClick, "")
            case .available:
                _ = onAppEvent(AppEvent.updateInstallClick, "")
            }
        }
        sidebarView = sidebar
        contentView.addSubview(sidebar)

        // Base layer — solid color fills entire window, visible through padding gaps
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

        // Content container — floating panel with rounded corners, clips content
        let right = NSView()
        right.translatesAutoresizingMaskIntoConstraints = false
        right.wantsLayer = true
        right.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        right.layer?.cornerRadius = Layout.contentPanelCornerRadius
        right.layer?.masksToBounds = true
        shadowWrapper.addSubview(right)
        contentPanelView = right

        // Navigation toolbar
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
        toolbar.onHostAction = { [weak self] actionID in
            self?.toolbarHostActionHandler?(actionID)
        }
        navigationToolbar = toolbar
        let workspace = workspaceManager.workspaceView
        workspace.addSubview(toolbar)
        workspaceManager.attachBelowToolbar(toolbar)

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

        let workspaceRoot = workspaceManager.rootView
        workspaceRoot.translatesAutoresizingMaskIntoConstraints = false
        right.addSubview(workspaceRoot)

        // Layout constraints
        let sidebarWidth = sidebar.widthAnchor.constraint(equalToConstant: Layout.sidebarWidth)
        sidebarWidthConstraint = sidebarWidth

        let p = Layout.contentPanelPadding
        let contentLeading = shadowWrapper.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: Layout.sidebarWidth)
        contentLeadingConstraint = contentLeading
        let cardTrailing = shadowWrapper.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -p)
        cardTrailingConstraint = cardTrailing
        let cardBottom = shadowWrapper.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -p)
        cardBottomConstraint = cardBottom

        NSLayoutConstraint.activate([
            base.topAnchor.constraint(equalTo: contentView.topAnchor),
            base.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            base.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            base.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),

            sidebar.topAnchor.constraint(equalTo: contentView.topAnchor),
            sidebar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            sidebar.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            sidebarWidth,

            shadowWrapper.topAnchor.constraint(equalTo: contentView.topAnchor, constant: p),
            contentLeading,
            cardTrailing,
            cardBottom,

            right.topAnchor.constraint(equalTo: shadowWrapper.topAnchor),
            right.leadingAnchor.constraint(equalTo: shadowWrapper.leadingAnchor),
            right.trailingAnchor.constraint(equalTo: shadowWrapper.trailingAnchor),
            right.bottomAnchor.constraint(equalTo: shadowWrapper.bottomAnchor),

            toolbar.topAnchor.constraint(equalTo: workspace.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: workspace.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: workspace.trailingAnchor),

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
            switchToTab(appId)
        } else if tabManager.activeTab?.appId != appId {
            tabManager.selectTab(appId: appId)
        }

        if let tabItem = getTabBarItem(appId, Int32(itemIndex)) {
            let path = tabItem.page_path.toString()
            if !path.isEmpty {
                getViewController(for: appId)?.navigate(appId: appId, to: path, with: .none)
            }
        }

        sidebarView?.setActiveHighlight(appId: appId, pageIndex: itemIndex)
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

        return maxX <= 0 ? Layout.trafficLightClearanceFallback : ceil(maxX + 12)
    }

    private func hideSidebar() {
        setSidebarVisible(false, animated: true)
    }

    private func showSidebar() {
        setSidebarVisible(true, animated: true)
    }

    private func setSidebarVisible(_ visible: Bool, animated: Bool) {
        guard let constraint = sidebarWidthConstraint else { return }
        guard sidebarChromeEnabled else {
            constraint.constant = 0
            contentLeadingConstraint?.constant = 0
            refreshSidebarVisibilityUI()
            return
        }

        let isVisible = constraint.constant >= Layout.sidebarHiddenThreshold
        if isVisible == visible {
            refreshSidebarVisibilityUI()
            return
        }

        if isVisible && constraint.constant > Layout.sidebarHiddenThreshold
            && !(sidebarView?.isCompact ?? false) {
            lastExpandedSidebarWidth = constraint.constant
        }

        // Revealing always returns to the expanded layout. Reset while still
        // hidden (width 0) so the switch is invisible.
        if visible {
            sidebarView?.setCompactMode(false)
        }

        let targetWidth: CGFloat = visible ? lastExpandedSidebarWidth : 0
        let sidebarHidden = !visible
        let targetContentLeading = targetWidth

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.25
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = targetWidth
                contentLeadingConstraint?.animator().constant = targetContentLeading
                browserCoordinator.syncToolbarLeading(collapsed: sidebarHidden, animated: true)
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.refreshSidebarVisibilityUI()
                }
            })
        } else {
            constraint.constant = targetWidth
            contentLeadingConstraint?.constant = targetContentLeading
            browserCoordinator.syncToolbarLeading(collapsed: sidebarHidden, animated: false)
            refreshSidebarVisibilityUI()
        }
    }

    func updateSidebarWidth(_ width: CGFloat, animated: Bool) {
        guard let constraint = sidebarWidthConstraint else { return }

        // Remember only settled, genuinely-expanded widths — never the icon
        // rail or transient live-drag widths — so expanding always restores the
        // pre-collapse width.
        if animated && width > Layout.sidebarHiddenThreshold && !(sidebarView?.isCompact ?? false) {
            lastExpandedSidebarWidth = width
        }

        let sidebarHidden = width < Layout.sidebarHiddenThreshold
        let targetContentLeading = max(0, width)

        if animated {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.2
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                constraint.animator().constant = width
                contentLeadingConstraint?.animator().constant = targetContentLeading
                browserCoordinator.syncToolbarLeading(collapsed: sidebarHidden, animated: true)
            }, completionHandler: {
                MainActor.assumeIsolated { [weak self] in
                    self?.refreshSidebarVisibilityUI()
                }
            })
        } else {
            constraint.constant = width
            contentLeadingConstraint?.constant = targetContentLeading
            browserCoordinator.syncToolbarLeading(collapsed: sidebarHidden, animated: false)
            refreshSidebarVisibilityUI()
        }
    }

    private func refreshSidebarVisibilityUI() {
        sidebarView?.updateVisibilityState()
        let sidebarHidden = (sidebarWidthConstraint?.constant ?? 0) < Layout.sidebarHiddenThreshold
        contentLeadingConstraint?.constant = sidebarHidden ? 0 : max(0, sidebarWidthConstraint?.constant ?? Layout.sidebarWidth)
        sidebarRevealButton.isHidden = !sidebarChromeEnabled || !sidebarHidden
        browserCoordinator.syncToolbarLeading(collapsed: sidebarHidden, animated: false)
        syncSidebarHeaderButtonAlignment()
        window?.contentView?.layoutSubtreeIfNeeded()
        workspaceManager.relayoutPanels()
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

        let created = createPageInstance(homeLxAppId, "", sessionId, 0, "")
        let resolvedPath = created.resolved_path.toString()
        let createError = created.error.toString()
        guard created.ok, !resolvedPath.isEmpty else {
            os_log(
                "setupInitialTab rejected by Rust for %@ error=%{public}@",
                log: Self.log,
                type: .info,
                homeLxAppId,
                createError
            )
            return
        }
        appSessions[homeLxAppId] = sessionId
        LxAppCore.setSessionId(sessionId, for: homeLxAppId)
        LxAppCore.setCurrentApp(appId: homeLxAppId, path: resolvedPath)
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
            let created = createPageInstance(appId, currentPath, sessionId, 0, "")
            let resolvedPath = created.resolved_path.toString()
            let createError = created.error.toString()
            if !created.ok || resolvedPath.isEmpty {
                os_log(
                    "switchToTab rejected by Rust for %@ error=%{public}@",
                    log: Self.log,
                    type: .info,
                    appId,
                    createError
                )
                return
            }
        }

        updateContentView(with: viewController)
        sidebarView?.setActiveHighlight(appId: appId)
    }

    private func updateContentView(with viewController: macOSLxAppViewController) {
        currentViewController?.pauseNativeComponents()
        currentViewController?.view.removeFromSuperview()
        currentViewController = viewController
        navigationToolbar?.forceHide(false)

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
        viewController.updateNavigationBar(appId: viewController.appId, path: viewController.currentPath)
        navigationToolbar?.isHidden = false
        navigationToolbar?.forceHide(false)
        navigationToolbar?.refreshCurrentState()
        applyToolbarMode(configuration.toolbar)
        syncSidebarHeaderButtonAlignment()
        viewController.resumeNativeComponents()
        // Now that an lxapp is current and laid out, seed the Adaptive Surface
        // Layout core with the real container width (windowDidResize may have
        // fired before any lxapp was current).
        reportSurfaceWidth()
    }

    func refreshNavigationBar(for appId: String) {
        guard currentViewController?.appId == appId,
              let viewController = getViewController(for: appId) else {
            return
        }

        viewController.updateNavigationBar(appId: appId, path: viewController.currentPath)
        navigationToolbar?.refreshCurrentState()
        applyToolbarMode(configuration.toolbar)

        workspaceManager.rootView.needsLayout = true
        workspaceManager.rootView.layoutSubtreeIfNeeded()
        contentPanelView?.needsLayout = true
        contentPanelView?.layoutSubtreeIfNeeded()
        window?.contentView?.needsLayout = true
        window?.contentView?.layoutSubtreeIfNeeded()
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
        browserCoordinator.syncToolbarLeading(collapsed: isSidebarCollapsed(), animated: false)
    }

    private func configureSidebarRevealButton() {
        sidebarRevealButton.translatesAutoresizingMaskIntoConstraints = false
        sidebarRevealButton.isBordered = false
        sidebarRevealButton.bezelStyle = .regularSquare
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

    // MARK: - QLPreviewPanel

    override public func acceptsPreviewPanelControl(_ panel: QLPreviewPanel!) -> Bool {
        MainActor.assumeIsolated {
            LxAppMedia.qlController != nil || LxAppFile.qlController != nil
        }
    }

    override public func beginPreviewPanelControl(_ panel: QLPreviewPanel!) {
    }

    override public func endPreviewPanelControl(_ panel: QLPreviewPanel!) {
        MainActor.assumeIsolated {
            LxAppMedia.clearQLController()
            LxAppFile.clearQLController()
        }
    }

    // MARK: - Tab Close

    private func closeTab(_ appId: String) {
        closeSession(appId: appId, notifyRuntime: true)
    }

    private func closeSession(appId: String, notifyRuntime: Bool) {
        guard let sessionId = appSessions[appId], sessionId > 0 else {
            os_log("closeTab missing session for %@", log: Self.log, type: .error, appId)
            return
        }

        if notifyRuntime {
            let accepted = onLxappClosed(appId, sessionId)
            guard accepted else {
                os_log("Ignoring stale close callback for %@ (session=%{public}llu)", log: Self.log, type: .info, appId, sessionId)
                return
            }
            _ = controller.discardSession(appId: appId, sessionId: sessionId)
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

    private func observeControllerEvents() {
        controllerEventsTask = Task { [weak self, controller] in
            for await event in controller.events {
                guard let self else { return }
                switch event {
                case .didClose(let session):
                    closeSession(appId: session.appId, notifyRuntime: false)
                default:
                    continue
                }
            }
        }
    }

    // MARK: - Apply Configuration

    private func applySidebarMode(_ mode: LxAppSidebarMode) {
        switch mode {
        case .hidden:
            setSidebarVisible(false, animated: true)
        case .declarative:
            setSidebarVisible(true, animated: true)
        case .swiftNative(let handle):
            _ = LxAppSidebarRegistry.shared.resolve(handle)
            setSidebarVisible(true, animated: true)
        }
    }

    private func applyToolbarMode(_ mode: LxAppToolbarMode) {
        switch mode {
        case .hidden:
            navigationToolbar?.isHidden = true
        case .declarative:
            navigationToolbar?.isHidden = false
        case .swiftNative(let handle):
            _ = LxAppToolbarRegistry.shared.resolve(handle)
            navigationToolbar?.isHidden = false
        }
    }

    private func applyChromeStyle(_ style: LxAppChromeStyle) {
        contentPanelView?.layer?.cornerRadius = style.cornerRadius
        if let shadowWrapper = contentPanelView?.superview {
            shadowWrapper.layer?.shadowOpacity = style.hasShadow ? 0.15 : 0
        }
    }

    func setSidebarHostActionHandler(_ handler: @escaping (String) -> Void) {
        sidebarHostActionHandler = handler
    }

    func setToolbarHostActionHandler(_ handler: @escaping (String) -> Void) {
        toolbarHostActionHandler = handler
    }

    func setTitlebarHostActionHandler(_ handler: @escaping (String) -> Void) {
        titlebarHostActionHandler = handler
    }

    func updateSidebarHostActions(_ items: [LxAppUIActionItem]) {
        let sidebarItems = items.map { PanelIconItem(id: $0.id, iconURL: $0.iconURL, label: $0.label) }
        sidebarView?.updatePanelItems(sidebarItems)
    }

    /// Show the update callout above the bottom-left sidebar icon. `.ready` →
    /// click restarts to apply; `.available` → click re-opens the install
    /// flow. Reveals the sidebar first so the callout is visible.
    func presentUpdateReadyCallout(appName: String, state: UpdateCalloutState) {
        showSidebar()
        sidebarView?.presentUpdateReadyCallout(appName: appName, state: state)
    }

    /// Present the centered "update available" card (Stage 1). The card then
    /// drives the whole flow: Download & Install → live progress → Restart Now.
    func presentUpdateCard(appName: String, infoJSON: String, callbackId: UInt64) {
        UpdateAvailableCard.present(
            appName: appName,
            infoJSON: infoJSON,
            over: window,
            onDownload: {
                _ = onCallback(callbackId, true, "{\"confirm\":true}")
            },
            onLater: { [weak self] in
                _ = onCallback(callbackId, false, "2000")
                // Drop to the quiet sidebar reminder.
                self?.presentUpdateReadyCallout(appName: appName, state: .available)
            },
            onRestart: {
                _ = onAppEvent(AppEvent.updateRestartClick, "")
            })
    }

    func updateToolbarHostActions(_ items: [LxAppUIActionItem]) {
        navigationToolbar?.updateHostActions(items)
    }

    func updateTitlebarHostActions(_ items: [LxAppUIActionItem]) {
        updateTitlebarAccessoryActions(items)
    }

    func setManagedNavigationToolbarVisible(_ visible: Bool) {
        guard startupBehavior == .managedByAppUI else { return }
        if visible {
            navigationToolbar?.isHidden = false
            navigationToolbar?.forceHide(false)
        } else {
            navigationToolbar?.isHidden = true
            navigationToolbar?.forceHide(true)
        }
    }

    func applyManagedWindowPresentation(
        title: String?,
        size: CGSize?,
        resizable: Bool,
        kind: LxAppUIConfig.Presentation.Kind,
        showTrafficLights: Bool
    ) {
        guard let window else { return }

        if let title, !title.isEmpty {
            window.title = title
        }

        usesPanelPresentation = kind == .panel

        if let size {
            window.setContentSize(size)
            if !resizable {
                window.minSize = size
                window.maxSize = size
            }
        }

        if resizable && kind == .window {
            window.styleMask.insert(.resizable)
            let minSize = minimumManagedWindowSize(for: size)
            window.contentMinSize = minSize
            window.minSize = minSize
            window.maxSize = NSSize(
                width: CGFloat.greatestFiniteMagnitude,
                height: CGFloat.greatestFiniteMagnitude
            )
        } else {
            window.styleMask.remove(.resizable)
        }

        if let lxWindow = window as? LxAppWindow {
            lxWindow.setTrafficLightsHidden(!showTrafficLights)
        } else {
            for type: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
                window.standardWindowButton(type)?.isHidden = !showTrafficLights
            }
        }

        if kind == .panel {
            window.level = .floating
            window.isMovableByWindowBackground = true
            window.collectionBehavior.insert(.transient)
        } else {
            window.level = .normal
            window.collectionBehavior.remove(.transient)
            window.isMovableByWindowBackground = false
        }
    }

    private func minimumManagedWindowSize(for requestedSize: CGSize?) -> CGSize {
        let defaultMinimum = CGSize(width: 720, height: 480)
        guard let requestedSize else {
            return defaultMinimum
        }
        return CGSize(
            width: min(defaultMinimum.width, requestedSize.width),
            height: min(defaultMinimum.height, requestedSize.height)
        )
    }

    func retainAppUIRuntime(_ runtime: AnyObject) {
        appUIRuntimeRef = runtime
    }

    func setSidebarChromeEnabled(_ enabled: Bool) {
        sidebarChromeEnabled = enabled
        guard let constraint = sidebarWidthConstraint else {
            refreshSidebarVisibilityUI()
            return
        }
        if enabled {
            if constraint.constant < Layout.sidebarHiddenThreshold {
                constraint.constant = lastExpandedSidebarWidth
                contentLeadingConstraint?.constant = 0
            }
        } else {
            constraint.constant = 0
            contentLeadingConstraint?.constant = 0
        }
        refreshSidebarVisibilityUI()
    }

    private func updateTitlebarAccessoryActions(_ items: [LxAppUIActionItem]) {
        guard let window else { return }

        if items.isEmpty {
            if let controller = titlebarAccessoryController,
               let index = window.titlebarAccessoryViewControllers.firstIndex(of: controller) {
                window.removeTitlebarAccessoryViewController(at: index)
            }
            titlebarAccessoryController = nil
            titlebarActionStrip = nil
            return
        }

        let strip: MacTitlebarActionStrip
        let controller: NSTitlebarAccessoryViewController

        if let existingStrip = titlebarActionStrip,
           let existingController = titlebarAccessoryController {
            strip = existingStrip
            controller = existingController
        } else {
            strip = MacTitlebarActionStrip()
            strip.onAction = { [weak self] actionID in
                self?.titlebarHostActionHandler?(actionID)
            }

            let accessoryController = NSTitlebarAccessoryViewController()
            accessoryController.view = strip
            accessoryController.layoutAttribute = .right
            window.addTitlebarAccessoryViewController(accessoryController)

            titlebarActionStrip = strip
            titlebarAccessoryController = accessoryController
            controller = accessoryController
        }

        strip.updateActions(items)
        controller.isHidden = false
    }
}

// MARK: - Browser Coordinator Forwarding

extension LxAppShell {
    func toggleActiveDevTools() -> Bool {
        browserCoordinator.toggleActiveDevTools()
    }

    func presentInternalBrowserTab(id: String) {
        browserCoordinator.presentInternalBrowserTab(id: id)
    }

    @MainActor
    func prepareInternalBrowserTabForInput(id: String) -> Bool {
        browserCoordinator.prepareNativeInput(tabId: id)
    }

    @MainActor
    func consumeSelfTargetNavigationInActiveBrowserTab(urlString: String) -> Bool {
        browserCoordinator.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString)
    }
}

// MARK: - BrowserCoordinatorHost

extension LxAppShell: BrowserCoordinatorHost {
    var browserContentContainer: NSView { workspaceManager.contentContainer }
    var hostWindow: NSWindow? { window }
    var hasOpenTabs: Bool { tabManager.hasTabs }

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

    func updateSidebarBrowserItems(_ items: [(id: String, title: String, favicon: NSImage?)], activeId: String?) {
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

// MARK: - Panel Methods

extension LxAppShell {
    private static let panelAttachMaxRetry = 40
    private static let panelAttachRetryDelay: TimeInterval = 0.05

    func showPanelWithContent(id: String, position: PanelPosition, appId: String, path: String) {
        let wasRegistered = workspaceManager.isPanelRegistered(id: id)
        lxShellStdoutLog(
            "showPanelWithContent start id=\(id) position=\(position.rawValue) registered=\(wasRegistered) appId=\(appId) path=\(path) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))"
        )
        if !wasRegistered {
            let config = PanelConfig(id: id, position: position)
            workspaceManager.registerPanel(config)
        }

        preserveWindowFrameDuringPanelLayout(reason: "showPanelWithContent:\(id)") {
            workspaceManager.showPanel(id: id)
        }
        lxShellStdoutLog(
            "showPanelWithContent afterShow id=\(id) containerFrame=\(lxShellFormatRect(workspaceManager.panelContainer(id: id)?.frame ?? .zero)) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))"
        )
        attachPanelWebViewWhenReady(panelId: id, appId: appId, path: path, attempt: 0)
    }

    func showPanelWithNativeContent(
        id: String,
        position: PanelPosition,
        contentView: NSView,
        defaultSize: CGFloat = 320
    ) {
        let wasRegistered = workspaceManager.isPanelRegistered(id: id)
        lxShellStdoutLog(
            "showPanelWithNativeContent start id=\(id) position=\(position.rawValue) registered=\(wasRegistered) defaultSize=\(String(format: "%.1f", defaultSize)) contentType=\(String(describing: type(of: contentView))) contentFrame=\(lxShellFormatRect(contentView.frame)) contentBounds=\(lxShellFormatRect(contentView.bounds)) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero)) contentViewBounds=\(lxShellFormatRect(window?.contentView?.bounds ?? .zero))"
        )
        if !wasRegistered {
            let config = PanelConfig(
                id: id,
                position: position,
                defaultSize: defaultSize
            )
            workspaceManager.registerPanel(config)
        }

        preserveWindowFrameDuringPanelLayout(reason: "showPanelWithNativeContent:\(id)") {
            workspaceManager.showPanel(id: id)
        }
        guard let container = workspaceManager.panelContainer(id: id) else {
            lxShellStdoutLog("showPanelWithNativeContent missingContainer id=\(id)")
            return
        }
        lxShellStdoutLog(
            "showPanelWithNativeContent container id=\(id) containerFrame=\(lxShellFormatRect(container.frame)) containerBounds=\(lxShellFormatRect(container.bounds)) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero)) contentViewBounds=\(lxShellFormatRect(window?.contentView?.bounds ?? .zero))"
        )
        attachPanelContentView(contentView, container: container)
        DispatchQueue.main.async { [weak self, weak contentView, weak container] in
            lxShellStdoutLog(
                "showPanelWithNativeContent afterAttachAsync id=\(id) containerFrame=\(lxShellFormatRect(container?.frame ?? .zero)) containerBounds=\(lxShellFormatRect(container?.bounds ?? .zero)) contentFrame=\(lxShellFormatRect(contentView?.frame ?? .zero)) contentBounds=\(lxShellFormatRect(contentView?.bounds ?? .zero)) windowFrame=\(lxShellFormatRect(self?.window?.frame ?? .zero)) contentViewBounds=\(lxShellFormatRect(self?.window?.contentView?.bounds ?? .zero))"
            )
        }
    }

    func hidePanel(id: String) {
        lxShellStdoutLog("hidePanel start id=\(id) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))")
        preserveWindowFrameDuringPanelLayout(reason: "hidePanel:\(id)") {
            workspaceManager.hidePanel(id: id)
        }
        lxShellStdoutLog("hidePanel end id=\(id) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))")
    }

    func showPanel(id: String) {
        lxShellStdoutLog("showPanel start id=\(id) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))")
        preserveWindowFrameDuringPanelLayout(reason: "showPanel:\(id)") {
            workspaceManager.showPanel(id: id)
        }
        lxShellStdoutLog("showPanel end id=\(id) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))")
    }

    func togglePanel(id: String) {
        lxShellStdoutLog("togglePanel start id=\(id) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))")
        preserveWindowFrameDuringPanelLayout(reason: "togglePanel:\(id)") {
            workspaceManager.togglePanel(id: id)
        }
        lxShellStdoutLog("togglePanel end id=\(id) windowFrame=\(lxShellFormatRect(window?.frame ?? .zero))")
    }

    func setPanelFullscreen(id: String, enabled: Bool) {
        lxShellStdoutLog("setPanelFullscreen start id=\(id) enabled=\(enabled)")
        preserveWindowFrameDuringPanelLayout(reason: "setPanelFullscreen:\(id):\(enabled)") {
            workspaceManager.setPanelFullscreen(id: id, enabled: enabled)
        }
        lxShellStdoutLog("setPanelFullscreen end id=\(id) enabled=\(enabled)")
    }

    private func sameFrame(_ lhs: NSRect, _ rhs: NSRect) -> Bool {
        abs(lhs.minX - rhs.minX) <= 0.5
            && abs(lhs.minY - rhs.minY) <= 0.5
            && abs(lhs.width - rhs.width) <= 0.5
            && abs(lhs.height - rhs.height) <= 0.5
    }

    private func formatFrame(_ frame: NSRect) -> String {
        String(
            format: "%.0f,%.0f %.0fx%.0f",
            frame.minX,
            frame.minY,
            frame.width,
            frame.height
        )
    }

    private func preserveWindowFrameDuringPanelLayout(reason: String, _ operation: () -> Void) {
        guard let window else {
            lxShellStdoutLog("preserveFrame noWindow reason=\(reason)")
            operation()
            return
        }

        let frameBefore = window.frame
        let minSizeBefore = window.minSize
        let contentMinSizeBefore = window.contentMinSize
        let contentSizeBefore = window.contentView?.bounds.size ?? frameBefore.size
        window.minSize = NSSize(
            width: max(minSizeBefore.width, frameBefore.width),
            height: max(minSizeBefore.height, frameBefore.height)
        )
        window.contentMinSize = NSSize(
            width: max(contentMinSizeBefore.width, contentSizeBefore.width),
            height: max(contentMinSizeBefore.height, contentSizeBefore.height)
        )
        lxShellStdoutLog("preserveFrame begin reason=\(reason) frame=\(formatFrame(frameBefore))")
        operation()
        lxShellStdoutLog("preserveFrame afterOperation reason=\(reason) frame=\(formatFrame(window.frame)) changed=\(!sameFrame(frameBefore, window.frame))")
        restoreWindowFrameIfNeeded(frameBefore, reason: reason)

        // Panel animations and AppKit constraint passes may settle on the next ticks.
        DispatchQueue.main.async { [weak self, weak window] in
            guard let self, let window else { return }
            lxShellStdoutLog("preserveFrame asyncCheck reason=\(reason) frame=\(self.formatFrame(window.frame)) changed=\(!self.sameFrame(frameBefore, window.frame))")
            self.restoreWindowFrameIfNeeded(frameBefore, reason: "\(reason):async")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.28) { [weak self, weak window] in
                guard let self, let window else { return }
                lxShellStdoutLog("preserveFrame settledCheck reason=\(reason) frame=\(self.formatFrame(window.frame)) changed=\(!self.sameFrame(frameBefore, window.frame))")
                self.restoreWindowFrameIfNeeded(frameBefore, reason: "\(reason):settled")
                window.minSize = minSizeBefore
                window.contentMinSize = contentMinSizeBefore
                lxShellStdoutLog(
                    "preserveFrame restoredMinSize reason=\(reason) min=\(String(format: "%.0fx%.0f", minSizeBefore.width, minSizeBefore.height)) contentMin=\(String(format: "%.0fx%.0f", contentMinSizeBefore.width, contentMinSizeBefore.height))"
                )
            }
        }
    }

    private func restoreWindowFrameIfNeeded(_ frameBefore: NSRect, reason: String) {
        guard let window else { return }
        let current = window.frame
        guard abs(current.width - frameBefore.width) > 0.5 || abs(current.height - frameBefore.height) > 0.5 else {
            return
        }
        let message = "Panel layout changed window frame; restoring reason=\(reason) before=\(String(format: "%.1fx%.1f", frameBefore.width, frameBefore.height)) current=\(String(format: "%.1fx%.1f", current.width, current.height))"
        lxShellStdoutLog(message, level: 4)
        os_log(
            "%{public}@",
            log: Self.log,
            type: .error,
            message
        )
        window.setFrame(frameBefore, display: true)
    }

    private func attachPanelWebViewWhenReady(panelId: String, appId: String, path: String, attempt: Int) {
        guard let sessionId = appSessions[appId],
              let container = workspaceManager.panelContainer(id: panelId) else {
            return
        }

        if let webView = WebViewManager.resolveWebView(appId: appId, path: path, sessionId: sessionId) {
            WebViewManager.attachWebViewToContainer(webView, container: container)
            return
        }

        guard attempt < Self.panelAttachMaxRetry else {
            os_log("panel webview attach timed out for panel=%{public}@ appId=%{public}@ path=%{public}@",
                   type: .error, panelId, appId, path)
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + Self.panelAttachRetryDelay) { [weak self] in
            self?.attachPanelWebViewWhenReady(panelId: panelId, appId: appId, path: path, attempt: attempt + 1)
        }
    }

    private func attachPanelContentView(_ view: NSView, container: NSView) {
        if view.superview === container {
            lxShellStdoutLog(
                "attachPanelContentView alreadyAttached contentType=\(String(describing: type(of: view))) containerFrame=\(lxShellFormatRect(container.frame)) contentFrame=\(lxShellFormatRect(view.frame))"
            )
            return
        }
        lxShellStdoutLog(
            "attachPanelContentView start contentType=\(String(describing: type(of: view))) oldSuperview=\(String(describing: view.superview.map { type(of: $0) })) containerFrame=\(lxShellFormatRect(container.frame)) containerBounds=\(lxShellFormatRect(container.bounds))"
        )
        container.subviews.forEach { $0.removeFromSuperview() }
        view.setContentHuggingPriority(.defaultLow, for: .horizontal)
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        view.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(view)
        NSLayoutConstraint.activate([
            view.topAnchor.constraint(equalTo: container.topAnchor),
            view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            view.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])
        container.layoutSubtreeIfNeeded()
        lxShellStdoutLog(
            "attachPanelContentView complete contentType=\(String(describing: type(of: view))) containerFrame=\(lxShellFormatRect(container.frame)) containerBounds=\(lxShellFormatRect(container.bounds)) contentFrame=\(lxShellFormatRect(view.frame)) contentBounds=\(lxShellFormatRect(view.bounds))"
        )
    }
}

// MARK: - Equatable for sidebar/toolbar modes (needed for diff check)

extension LxAppSidebarMode: Equatable {
    public static func == (lhs: LxAppSidebarMode, rhs: LxAppSidebarMode) -> Bool {
        switch (lhs, rhs) {
        case (.hidden, .hidden):
            return true
        case (.declarative(let a), .declarative(let b)):
            return a == b
        case (.swiftNative(let a), .swiftNative(let b)):
            return a == b
        default:
            return false
        }
    }
}

extension LxAppToolbarMode: Equatable {
    public static func == (lhs: LxAppToolbarMode, rhs: LxAppToolbarMode) -> Bool {
        switch (lhs, rhs) {
        case (.hidden, .hidden):
            return true
        case (.declarative(let a), .declarative(let b)):
            return a == b
        case (.swiftNative(let a), .swiftNative(let b)):
            return a == b
        default:
            return false
        }
    }
}

#else

/// iOS placeholder — shell functionality is handled differently on iOS.
@MainActor
public final class LxAppShell {

    public let controller: LxAppController
    public private(set) var configuration: LxAppShellConfiguration
    public let hostView: LxAppHostView
    private var didOpenHome = false
    private var controllerEventsTask: Task<Void, Never>?

    public init(
        controller: LxAppController = LxAppController(),
        configuration: LxAppShellConfiguration = LxAppShellConfiguration()
    ) {
        self.controller = controller
        self.configuration = configuration
        self.hostView = LxAppHostView(controller: controller)
        observeControllerEvents()
    }

    deinit {
        controllerEventsTask?.cancel()
    }

    public func updateConfiguration(_ newConfig: LxAppShellConfiguration) {
        configuration = newConfig
    }

    public func show() {
        iOSLxApp.initialize(autoOpenHome: false)
        guard !didOpenHome else { return }
        didOpenHome = true
        Task { @MainActor [controller] in
            _ = try? await controller.openHomeApp()
        }
    }
    public func hide() {}

    private func observeControllerEvents() {
        controllerEventsTask = Task { [controller] in
            for await event in controller.events {
                switch event {
                case .didClose(let session):
                    iOSLxApp.closeLxApp(
                        appId: session.appId,
                        sessionId: session.id.rawValue,
                        notifyRuntime: false
                    )
                default:
                    continue
                }
            }
        }
    }
}

#endif
