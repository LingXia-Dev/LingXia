#if os(macOS)
import AppKit
import SwiftUI
import WebKit
import os.log

/// Unified window controller for SwiftUI/macOS - supports both capsule and tab modes with SwiftUI integration
public class LxAppWindowController: NSWindowController, NSWindowDelegate {

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppWindowController")
    private static var windowWidth: CGFloat = 1200
    private static var windowHeight: CGFloat = 800

    internal struct Layout {
        static let dragBarHeight: CGFloat = 20
        static let navBarHeight: CGFloat = 32
        static let capsuleContainerWidth: CGFloat = 88
        static let capsuleContainerHeight: CGFloat = 26
        static let capsuleTrailingMargin: CGFloat = 12
        static let capsuleTopOffset: CGFloat = 8
    }

    var appId: String?
    var path: String?
    private var navigationBar: NSView?
    private var floatingCapsuleContainer: NSView?
    private var dragBar: NSView?

    // Cache the current page config to avoid repeated Rust calls
    private var cachedPageConfig: NavigationBarState?

    private let tabManager = LxAppTabManager.shared
    private var tabView: LxAppTabView?
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]

    public static func setWindowSize(width: CGFloat, height: CGFloat) {
        windowWidth = width
        windowHeight = height
    }

    public static func setWindowStyle(_ style: LxAppWindowStyle) {
        LxAppWindowManager.shared.setWindowStyle(style)
    }

    public static func getWindowStyle() -> LxAppWindowStyle {
        LxAppWindowManager.shared.windowStyle
    }

    /// Get or refresh the cached page config
    private func getPageConfig(forceRefresh: Bool = false) -> NavigationBarState? {
        guard let appId = appId, let path = path else { return nil }

        if cachedPageConfig == nil || forceRefresh {
            cachedPageConfig = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
        }
        return cachedPageConfig
    }

    /// Clear cached config when path changes
    private func clearPageConfigCache() {
        cachedPageConfig = nil
    }

    public func getTopMarginForCurrentPage() -> CGFloat {
        guard let _ = appId, let _ = path else { return Layout.navBarHeight }

        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            let pageConfig = getPageConfig()
            return pageConfig?.show_navbar == false ? 0 : Layout.dragBarHeight + Layout.navBarHeight
        }
        return Layout.navBarHeight
    }

    private var switchPageObserver: NSObjectProtocol?

    /// Initialize for single LxApp mode
    init(appId: String, path: String) {
        self.appId = appId
        self.path = path

        let window = Self.createWindow()
        super.init(window: window)

        setupSingleAppMode()
    }

    /// Initialize for tab mode
    init() {
        super.init(window: Self.createWindow(width: 1200, height: 800, style: LxAppWindowManager.shared.windowStyle))
        setupTabMode()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private static func createWindow(width: CGFloat? = nil, height: CGFloat? = nil, style: LxAppWindowStyle? = nil) -> LxAppWindow {
        let finalWidth = width ?? windowWidth
        let finalHeight = height ?? windowHeight
        let finalStyle = style ?? LxAppWindowManager.shared.windowStyle

        let styleMask: NSWindow.StyleMask
        switch finalStyle {
        case .capsuleStyle:
            styleMask = [.titled, .closable, .miniaturizable]
        case .tabStyle:
            styleMask = [.titled, .closable, .miniaturizable, .resizable]
        }

        let window = LxAppWindow(
            contentRect: NSRect(x: 0, y: 0, width: finalWidth, height: finalHeight),
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )

        window.configureForStyle(finalStyle)
        window.center()
        window.isReleasedWhenClosed = false

        return window
    }

    private func setupSingleAppMode() {
        guard let appId = appId, let path = path else { return }

        self.window?.delegate = self

        let viewController = macOSLxAppViewController(appId: appId, path: path)
        currentViewController = viewController

        // Save and restore window frame
        let savedFrame = self.window?.frame ?? .zero
        self.window?.contentViewController = viewController

        if let window = self.window, window.frame != savedFrame {
            window.setFrame(savedFrame, display: true)
        }

        setupNotificationObservers()

        // Setup UI components
        DispatchQueue.main.async { [weak self] in
            self?.ensureCorrectViewFrame()
            if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
                self?.setupTitleBar()
            }
        }
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

        // Setup title bar for capsule style
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            DispatchQueue.main.async { [weak self] in
                self?.setupTitleBar()
            }
        }
    }

    public func windowWillClose(_ notification: Notification) {
        if let appId = appId {
            // Single app mode cleanup
            macOSLxApp.handleAppClosing(appId: appId)
            removeNotificationObservers()
            macOSLxApp.removeWindowController(self)
        } else {
            // Tab mode cleanup
            for tab in tabManager.tabs {
                let _ = onLxappClosed(tab.appId)
            }
            macOSLxApp.removeTabWindowController(self)
        }
    }

    private func setupNotificationObservers() {
        guard let appId = appId else { return }

        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let notificationAppId = notification.userInfo?["appId"] as? String,
                  let path = notification.userInfo?["path"] as? String,
                  notificationAppId == appId else { return }

            Task { @MainActor in
                self.updateWindowTitle(for: path)
            }
        }
    }

    private func removeNotificationObservers() {
        if let observer = switchPageObserver {
            NotificationCenter.default.removeObserver(observer)
            switchPageObserver = nil
        }
    }

    public func updateWindowTitle(for path: String) {
        guard let appId = appId, let navigationBar = self.navigationBar else { return }

        // Clear cache when path changes
        if self.path != path {
            clearPageConfigCache()
        }
        self.path = path

        let pageConfig = getPageConfig()

        // Update navigation bar based on page configuration from Rust
        if let config = pageConfig {
            updateNavigationBarWithConfig(config)
        }

        // Update WebView layout and view controller
        updateWebViewLayoutForNavigationStyle(pageConfig)

        if let viewController = currentViewController {
            viewController.updateLayoutForNavigationStyle(currentPath: path)
        }
    }

    private func setupTitleBar() {
        guard let window = self.window, let contentView = window.contentView else {
            os_log("❌ setupTitleBar: window or contentView is nil", log: Self.log, type: .error)
            return
        }

        // Configure window for custom title bar
        configureWindowForCustomTitleBar(window)

        // Create drag bar and navigation bar
        let dragBar = createDragBar(in: window)
        let navBar = createNavigationBar(in: window)

        // Configure colors based on window style
        configureBarColors(dragBar: dragBar, navBar: navBar)

        // Setup drag behavior and add to content view
        setupDragBarBehavior(dragBar)
        contentView.addSubview(dragBar)
        contentView.addSubview(navBar)

        // Setup Auto Layout constraints for drag bar and navbar
        NSLayoutConstraint.activate([
            // Drag bar at the top of content view
            dragBar.topAnchor.constraint(equalTo: contentView.topAnchor),
            dragBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            dragBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            dragBar.heightAnchor.constraint(equalToConstant: Layout.dragBarHeight),

            // Navbar directly below drag bar
            navBar.topAnchor.constraint(equalTo: dragBar.bottomAnchor),
            navBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            navBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            navBar.heightAnchor.constraint(equalToConstant: Layout.navBarHeight)
        ])

        // Store references
        self.dragBar = dragBar
        self.navigationBar = navBar

        // Setup capsule buttons for capsule style
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            setupFloatingCapsuleButtons(in: contentView)
        }
    }

    private func configureWindowForCustomTitleBar(_ window: NSWindow) {
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.styleMask.insert(.fullSizeContentView)

        // Hide standard window buttons
        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
    }

    private func createDragBar(in window: NSWindow) -> NSView {
        guard let contentView = window.contentView else {
            fatalError("Window must have a content view")
        }

        // Create drag bar positioned at the top of the content view
        let dragBar = NSView(frame: NSRect(
            x: 0,
            y: contentView.frame.height - Layout.dragBarHeight,
            width: contentView.frame.width,
            height: Layout.dragBarHeight
        ))
        dragBar.wantsLayer = true
        dragBar.translatesAutoresizingMaskIntoConstraints = false
        return dragBar
    }

    private func createNavigationBar(in window: NSWindow) -> NSView {
        guard let contentView = window.contentView else {
            fatalError("Window must have a content view")
        }

        // Create navbar positioned at the top of the content view (below drag bar)
        let navBar = NSView(frame: NSRect(
            x: 0,
            y: contentView.frame.height - Layout.navBarHeight, // Position at top of content view
            width: contentView.frame.width,
            height: Layout.navBarHeight
        ))
        navBar.wantsLayer = true
        navBar.translatesAutoresizingMaskIntoConstraints = false
        return navBar
    }

    private func configureBarColors(dragBar: NSView, navBar: NSView) {
        let backgroundColor: NSColor = LxAppWindowManager.shared.windowStyle == .capsuleStyle
            ? .clear
            : .controlBackgroundColor

        dragBar.layer?.backgroundColor = backgroundColor.cgColor
        navBar.layer?.backgroundColor = backgroundColor.cgColor

        // Apply initial navigation configuration
        applyInitialNavigationConfiguration()
    }

    private func applyInitialNavigationConfiguration() {
        guard let appId = appId, let path = path, let navigationBar = navigationBar else { return }

        let pageConfig = getPageConfig()

        if let config = pageConfig {
            if config.show_navbar {
                updateNavigationBarWithConfig(config)
                navigationBar.isHidden = false
            } else {
                navigationBar.isHidden = true
            }
        } else {
            // Fallback: show navbar with default styling if no config provided
            navigationBar.isHidden = false
            navigationBar.layer?.backgroundColor = NSColor.systemBlue.cgColor
        }

        updateWindowTitle(for: path)
    }

    /// Clean data-driven navigation bar update
    public func updateNavigationBarWithState(_ state: NavigationBarState?) {
        guard let navigationBar = self.navigationBar else { return }

        if let state = state {
            updateNavigationBarWithConfig(state)
            navigationBar.isHidden = !state.show_navbar
        } else {
            navigationBar.isHidden = true
        }
    }

    private func setupFloatingCapsuleButtons(in contentView: NSView) {
        let capsuleContainer = createCapsuleContainer()
        contentView.addSubview(capsuleContainer)

        // Create buttons
        let buttons = createCapsuleButtons()
        let separators = createSeparators()

        // Add buttons and separators to container
        buttons.forEach { capsuleContainer.addSubview($0) }
        separators.forEach { capsuleContainer.addSubview($0) }

        // Position container
        positionCapsuleContainer(capsuleContainer, in: contentView)

        // Layout buttons and separators
        layoutCapsuleButtons(buttons, separators: separators, in: capsuleContainer)

        // Store reference
        self.floatingCapsuleContainer = capsuleContainer
    }

    private func createCapsuleContainer() -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.92).cgColor
        container.layer?.cornerRadius = Layout.capsuleContainerHeight / 2
        container.translatesAutoresizingMaskIntoConstraints = false

        // Add shadow
        container.shadow = NSShadow()
        container.layer?.shadowColor = NSColor.black.cgColor
        container.layer?.shadowOpacity = 0.12
        container.layer?.shadowOffset = CGSize(width: 0, height: 1)
        container.layer?.shadowRadius = 4

        return container
    }

    private func createCapsuleButtons() -> [NSButton] {
        // Capsule buttons are fixed floating controls, not related to navbar config
        return [
            createFloatingCapsuleButton(
                image: LxAppCapsuleButtons.createThreeDotsImage(),
                target: self,
                action: #selector(moreButtonClicked)
            ),
            createFloatingCapsuleButton(
                image: LxAppCapsuleButtons.createMinimizeButtonImage(),
                target: self,
                action: #selector(minimizeButtonClicked)
            ),
            createFloatingCapsuleButton(
                image: LxAppCapsuleButtons.createCloseButtonImage(),
                target: self,
                action: #selector(closeButtonClicked)
            )
        ]
    }

    private func createSeparators() -> [NSView] {
        return [createSeparatorLine(), createSeparatorLine()]
    }

    private func positionCapsuleContainer(_ container: NSView, in contentView: NSView) {
        let navBarCenterOffset = Layout.dragBarHeight + (Layout.navBarHeight - Layout.capsuleContainerHeight) / 2

        NSLayoutConstraint.activate([
            container.topAnchor.constraint(equalTo: contentView.topAnchor, constant: navBarCenterOffset),
            container.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -Layout.capsuleTrailingMargin),
            container.widthAnchor.constraint(equalToConstant: Layout.capsuleContainerWidth),
            container.heightAnchor.constraint(equalToConstant: Layout.capsuleContainerHeight)
        ])
    }

    private func layoutCapsuleButtons(_ buttons: [NSButton], separators: [NSView], in container: NSView) {
        let buttonWidth: CGFloat = 20
        let buttonHeight: CGFloat = 24
        let edgeSpacing: CGFloat = 8  // 增加边距
        let buttonSpacing: CGFloat = 8
        let separatorWidth: CGFloat = 1

        // Position buttons with proper spacing
        let button1X = edgeSpacing
        let button2X = edgeSpacing + buttonWidth + buttonSpacing
        let button3X = edgeSpacing + buttonWidth * 2 + buttonSpacing * 2

        // Vertically center the button
        let buttonY = (Layout.capsuleContainerHeight - buttonHeight) / 2

        if buttons.count >= 3 {
            buttons[0].frame = NSRect(x: button1X, y: buttonY, width: buttonWidth, height: buttonHeight)
            buttons[1].frame = NSRect(x: button2X, y: buttonY, width: buttonWidth, height: buttonHeight)
            buttons[2].frame = NSRect(x: button3X, y: buttonY, width: buttonWidth, height: buttonHeight)
        }

        // Position separators between buttons
        let separatorHeight: CGFloat = 18
        let separatorY = (Layout.capsuleContainerHeight - separatorHeight) / 2
        let separatorX1 = button1X + buttonWidth + (buttonSpacing - separatorWidth) / 2
        let separatorX2 = button2X + buttonWidth + (buttonSpacing - separatorWidth) / 2

        if separators.count >= 2 {
            separators[0].frame = NSRect(x: separatorX1, y: separatorY, width: separatorWidth, height: separatorHeight)
            separators[1].frame = NSRect(x: separatorX2, y: separatorY, width: separatorWidth, height: separatorHeight)
        }
    }

    private func createFloatingCapsuleButton(image: NSImage?, target: AnyObject, action: Selector) -> NSButton {
        let button = NSButton()
        button.image = image
        button.imageScaling = .scaleProportionallyDown
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.target = target
        button.action = action
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor

        // Add subtle hover effect
        button.layer?.cornerRadius = 6

        return button
    }

    private func createSeparatorLine() -> NSView {
        let separator = NSView()
        separator.wantsLayer = true
        separator.layer?.backgroundColor = NSColor(red: 0.867, green: 0.867, blue: 0.867, alpha: 1.0).cgColor
        return separator
    }

    private func setupDragBarBehavior(_ dragBar: NSView) {
        // Create a custom view that handles mouse events for dragging
        let dragView = DraggableView()
        dragView.windowController = self
        dragView.frame = dragBar.bounds
        dragView.autoresizingMask = [.width, .height]
        dragBar.addSubview(dragView)
    }

    private func setupNavigationBarTitle(in navBar: NSView) {
        // Remove existing title first
        removeTitleLabel(from: navBar)

        // Create title label
        let titleLabel = NSTextField(labelWithString: "LingXia")
        titleLabel.font = NSFont.systemFont(ofSize: 16, weight: .medium)
        titleLabel.textColor = NSColor.labelColor
        titleLabel.alignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        navBar.addSubview(titleLabel)

        // Center the title in the navbar
        NSLayoutConstraint.activate([
            titleLabel.centerXAnchor.constraint(equalTo: navBar.centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: navBar.centerYAnchor)
        ])
    }

    private func removeTitleLabel(from navBar: NSView) {
        // Find and remove existing title label
        for subview in navBar.subviews {
            if let textField = subview as? NSTextField {
                textField.removeFromSuperview()
            }
        }
    }

    private func repositionCapsuleButtons(on navigationBar: NSView) {
        let buttons = navigationBar.subviews.filter { $0 is NSButton }
        guard buttons.count == 3 else { return }

        // Use current navigation bar width instead of static window width
        let navBarWidth = navigationBar.frame.size.width
        let buttonWidth: CGFloat = 29
        let buttonHeight: CGFloat = 28
        let buttonY: CGFloat = 2
        let rightMargin: CGFloat = 7

        let startX = navBarWidth - (buttonWidth * 3) - rightMargin

        for (index, button) in buttons.enumerated() {
            let newX = startX + (buttonWidth * CGFloat(index))
            button.frame = NSRect(x: newX, y: buttonY, width: buttonWidth, height: buttonHeight)
        }
    }

    private func updateWebViewLayoutForNavigationStyle(_ pageConfig: NavigationBarState?) {
        guard let window = self.window,
              let contentView = window.contentView else {
            os_log("⚠️ updateWebViewLayoutForNavigationStyle: window or contentView is nil", log: Self.log, type: .info)
            return
        }

        guard let viewController = currentViewController else {
            os_log("⚠️ updateWebViewLayoutForNavigationStyle: currentViewController is nil", log: Self.log, type: .info)
            return
        }

        // Calculate new top offset based on navigation style
        let dragBarHeight: CGFloat = Layout.dragBarHeight
        let navBarHeight: CGFloat = Layout.navBarHeight
        let newTopOffset: CGFloat

        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            // Use the passed pageConfig parameter (already cached)
            if pageConfig?.show_navbar == false {
                // Hidden navbar: WebView covers entire area for full transparency effect
                newTopOffset = 0
            } else {
                // Default navigation style: WebView below both drag bar and navigation bar
                newTopOffset = dragBarHeight + navBarHeight
            }
        } else {
            // Tab style: drag bar + tab bar
            newTopOffset = dragBarHeight + navBarHeight
        }

        // Update the view controller's getTopMargin method instead of trying to modify constraints
        if let macOSViewController = viewController as? macOSLxAppViewController {
            macOSViewController.updateTopMargin(newTopOffset)
        } else {
            os_log("⚠️ View controller is not macOSLxAppViewController", log: Self.log, type: .info)
        }

        // Force layout update
        contentView.needsLayout = true
        contentView.layoutSubtreeIfNeeded()
    }


    @objc private func backButtonClicked() {
        print("NavigationBar: Back button clicked")
    }

    @objc private func homeButtonClicked() {
        print("NavigationBar: Home button clicked")
    }

    @objc private func moreButtonClicked() {
        // More button action
    }

    @objc private func minimizeButtonClicked() {
        window?.miniaturize(nil)
    }

    @objc private func closeButtonClicked() {
        window?.close()
    }

    private func updateNavigationBarButtons(_ config: NavigationBarState) {
        guard let navigationBar = self.navigationBar else {
            print("NavigationBar is nil, cannot add buttons")
            return
        }

        // Remove existing navbar buttons
        navigationBar.subviews.filter { $0.tag == 1001 || $0.tag == 1002 }.forEach { $0.removeFromSuperview() }

        let buttonSize: CGFloat = 22
        let buttonY = (navigationBar.frame.height - buttonSize) / 2 // Center vertically in navbar
        let buttonX: CGFloat = 16 // Start from left edge with proper margin

        // Priority logic: show back button first, only show home button if no back button
        if config.show_back_button {
            let backButton = NSButton(frame: NSRect(x: buttonX, y: buttonY, width: buttonSize, height: buttonSize))

            // Create a smaller symbol configuration for the back arrow
            let symbolConfig = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
            backButton.image = NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Back")?.withSymbolConfiguration(symbolConfig)

            backButton.imageScaling = .scaleProportionallyDown
            backButton.isBordered = false
            backButton.bezelStyle = .regularSquare
            backButton.imagePosition = .imageOnly
            backButton.tag = 1001 // Tag for identification
            backButton.target = self
            backButton.action = #selector(backButtonClicked)

            // Style the button to match navbar - use appropriate color based on navbar visibility
            if config.show_navbar {
                // Visible navbar: use white for colored backgrounds
                backButton.contentTintColor = NSColor.white
            } else {
                // Transparent navbar: use black for better visibility on light backgrounds
                backButton.contentTintColor = NSColor.black
            }

            navigationBar.addSubview(backButton)
        } else if config.show_home_button {
            // Only show home button if back button is not shown
            let homeButton = NSButton(frame: NSRect(x: buttonX, y: buttonY, width: buttonSize, height: buttonSize))

            // Create a smaller symbol configuration for the home icon
            let symbolConfig = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
            homeButton.image = NSImage(systemSymbolName: "house", accessibilityDescription: "Home")?.withSymbolConfiguration(symbolConfig)

            homeButton.imageScaling = .scaleProportionallyDown
            homeButton.isBordered = false
            homeButton.bezelStyle = .regularSquare
            homeButton.imagePosition = .imageOnly
            homeButton.tag = 1002 // Tag for identification
            homeButton.target = self
            homeButton.action = #selector(homeButtonClicked)

            // Style the button to match navbar - use appropriate color based on navbar visibility
            if config.show_navbar {
                // Visible navbar: use white for colored backgrounds
                homeButton.contentTintColor = NSColor.white
            } else {
                // Transparent navbar: use black for better visibility on light backgrounds
                homeButton.contentTintColor = NSColor.black
            }

            navigationBar.addSubview(homeButton)
        }
    }

    private func ensureCorrectViewFrame() {
        guard let window = self.window,
              let contentView = window.contentView,
              let viewController = window.contentViewController as? macOSLxAppViewController else { return }

        // Force window to correct size - this ensures proper Auto Layout constraint calculation
        let currentWindowFrame = window.frame
        let newWindowFrame = NSRect(
            x: currentWindowFrame.origin.x,
            y: currentWindowFrame.origin.y,
            width: Self.windowWidth,
            height: Self.windowHeight + 28 // Add title bar height
        )

        window.setFrame(newWindowFrame, display: true)

        // Force layout update to ensure Auto Layout constraints are recalculated
        contentView.needsLayout = true
        contentView.layoutSubtreeIfNeeded()
        viewController.view.needsLayout = true
        viewController.view.layoutSubtreeIfNeeded()
    }

    private func setupTabInterface() {
        guard let window = self.window, let contentView = window.contentView else { return }

        tabView = LxAppTabView(tabManager: tabManager)
        guard let tabBar = tabView else { return }

        tabBar.translatesAutoresizingMaskIntoConstraints = false
        tabBar.onTabSelected = { [weak self] appId in
            os_log("🔄 TabBar onTabSelected triggered: appId=%@", log: Self.log, type: .info, appId)
            self?.switchToTab(appId)
        }
        tabBar.onTabClosed = { [weak self] appId in
            self?.closeTab(appId)
        }

        contentView.addSubview(tabBar)

        NSLayoutConstraint.activate([
            tabBar.topAnchor.constraint(equalTo: contentView.topAnchor),
            tabBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            tabBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            tabBar.heightAnchor.constraint(equalToConstant: 32)
        ])
    }

    private func setupInitialTab() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else { return }

        // Get initial route from app info
        let lxappInfo = getLxAppInfo(homeLxAppId)
        let initialRoute = lxappInfo.initial_route.toString()
        LxAppCore.setLastActivePath(initialRoute, for: homeLxAppId)
        tabManager.addTab(appId: homeLxAppId)
    }

    public func openLxApp(appId: String, path: String) {
        LxAppCore.setLastActivePath(path, for: appId)
        tabManager.addTab(appId: appId)
    }

    private func switchToTab(_ appId: String) {

        let viewController = viewControllers[appId] ?? {
            let currentPath = LxAppCore.getLastActivePath(for: appId, defaultPath: "/")
            let vc = macOSLxAppViewController(appId: appId, path: currentPath)
            viewControllers[appId] = vc
            let _ = onLxappOpened(appId, currentPath)
            return vc
        }()

        updateContentView(with: viewController)
    }

    private func updateContentView(with viewController: macOSLxAppViewController) {
        currentViewController?.view.removeFromSuperview()
        currentViewController = viewController

        guard let window = self.window, let contentView = window.contentView else {
            os_log("❌ updateContentView: window or contentView is nil", log: Self.log, type: .error)
            return
        }

        viewController.view.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(viewController.view)

        // Calculate top offset based on window style and navigation style
        let topOffset: CGFloat
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            // Use cached page config
            let pageConfig = getPageConfig()
            if pageConfig?.show_navbar == false {
                // Hidden navigation bar: WebView covers navigation bar area
                topOffset = 0
            } else {
                // Default navigation style: WebView below navigation bar
                topOffset = 32  // Only navigation bar space (title bar is separate)
            }
        } else {
            // Tab style: 32pt for tab bar
            topOffset = 32
        }

        NSLayoutConstraint.activate([
            viewController.view.topAnchor.constraint(equalTo: contentView.topAnchor, constant: topOffset),
            viewController.view.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            viewController.view.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            viewController.view.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])
    }

    private func closeTab(_ appId: String) {
        if let viewController = viewControllers[appId] {
            viewController.view.removeFromSuperview()
            viewControllers.removeValue(forKey: appId)
        }

        tabManager.closeTab(appId: appId)
        let _ = onLxappClosed(appId)

        if !tabManager.hasTabs {
            window?.close()
        }
    }

    private func updateNavigationBarWithConfig(_ config: NavigationBarState) {
        guard let navigationBar = self.navigationBar else { return }

        // Ensure navigation bar has a layer
        if navigationBar.layer == nil {
            navigationBar.wantsLayer = true
        }

        // For hidden navigation bar, make navbar transparent
        if !config.show_navbar {
            navigationBar.layer?.backgroundColor = NSColor.clear.cgColor
            // Update drag bar to be transparent when navbar is transparent
            updateDragBarColor(NSColor.clear)
            // Remove title for transparent navbar
            removeTitleLabel(from: navigationBar)
        } else {
            // Apply background color for default navigation style
            let backgroundColor = PlatformColor(argb: config.background_color)

            // Ensure navigation bar has a layer and is properly configured
            navigationBar.wantsLayer = true

            // Set background color on the layer
            navigationBar.layer?.backgroundColor = backgroundColor.cgColor

            // Force immediate display update
            navigationBar.needsDisplay = true
            navigationBar.needsLayout = true

            updateDragBarColor(backgroundColor)
            setupNavigationBarTitle(in: navigationBar)
            if let titleLabel = findTitleLabel(in: navigationBar) {
                titleLabel.stringValue = config.title_text.toString()
                titleLabel.textColor = config.text_style.toString() == "white" ? NSColor.white : NSColor.black
            }

        }

        // Always update buttons regardless of navbar visibility
        updateNavigationBarButtons(config)
    }

    private func updateDragBarColor(_ color: NSColor) {
        guard let dragBar = self.dragBar else { return }

        // Ensure drag bar has a layer
        if dragBar.layer == nil {
            dragBar.wantsLayer = true
        }

        dragBar.layer?.backgroundColor = color.cgColor
    }

    private func findTitleLabel(in view: NSView) -> NSTextField? {
        // Search for NSTextField in the navigation bar view hierarchy
        for subview in view.subviews {
            if let textField = subview as? NSTextField {
                return textField
            }
            // Recursively search in subviews
            if let found = findTitleLabel(in: subview) {
                return found
            }
        }
        return nil
    }
}

/// Custom view that handles window dragging
private class DraggableView: NSView {
    weak var windowController: LxAppWindowController?

    override func mouseDown(with event: NSEvent) {
        guard let window = windowController?.window else { return }
        window.performDrag(with: event)
    }

    override func mouseDragged(with event: NSEvent) {
        // Window dragging is handled by performDrag
    }

    override func mouseUp(with event: NSEvent) {
        // End of drag
    }
}

#endif
