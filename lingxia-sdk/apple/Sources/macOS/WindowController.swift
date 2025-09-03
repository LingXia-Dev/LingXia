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

    public struct Layout {
        @MainActor static var systemStatusBarHeight: CGFloat {
            return currentNotchSpec.statusBarHeight
        }
        static let navBarHeight: CGFloat = 40  // Increased from 32 to 40
        static let swiftUITitleBarHeight: CGFloat = LxAppWindowLayout.titleBarHeight
        static let capsuleContainerWidth: CGFloat = 88
        static let capsuleContainerHeight: CGFloat = 26
        static let capsuleTrailingMargin: CGFloat = 12
        static let capsuleTopOffset: CGFloat = 8

        // Navigation button positioning (must match updateNavigationBarButtons)
        static let navButtonSize: CGFloat = 24
        static let navButtonMargin: CGFloat = 16
        static let navButtonBottomOffset: CGFloat = 12  // From bottom of navbar (macOS coordinates)

        // System status bar layout
        static let statusBarSideMargin: CGFloat = 12

        // Current iPhone notch specification - dynamically set based on window size
        @MainActor static var currentNotchSpec: iPhoneNotchSpec = .iPhoneSE  // Default to iPhone SE
    }

    var appId: String?
    var path: String?
    private var navigationBar: NSView?
    var floatingCapsuleContainer: NSView?
    private var systemStatusBar: NSView?
    private var independentNavigationButton: NSView?

    // System status bar components
    private var timeLabel: NSTextField?
    private var batteryView: NSView?
    private var notchView: NSView?

    private let tabManager = LxAppTabManager.shared
    private var tabView: LxAppTabView?
    private var currentViewController: macOSLxAppViewController?
    private var viewControllers: [String: macOSLxAppViewController] = [:]

    /// Get view controller for specific appId (needed for navigation)
    public func getViewController(for appId: String) -> macOSLxAppViewController? {
        return viewControllers[appId]
    }

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

    /// Get page config directly
    private func getPageConfig() -> NavigationBarState? {
        guard let appId = appId, let path = path else { return nil }
        return LxPageNavigation.getNavigationBarState(appId: appId, path: path)
    }

    public func getTopMarginForCurrentPage() -> CGFloat {
        guard let _ = appId, let _ = path else { return Layout.navBarHeight }

        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            let pageConfig = getPageConfig()
            return pageConfig?.show_navbar == false ? 0 : Layout.systemStatusBarHeight + Layout.navBarHeight
        }
        return Layout.navBarHeight
    }

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
            macOSLxApp.removeWindowController(self)
        } else {
            // Tab mode cleanup
            for tab in tabManager.tabs {
                let _ = onLxappClosed(tab.appId)
            }
            macOSLxApp.removeTabWindowController(self)
        }
    }

    public func updateWindowTitle(for path: String) {
        guard let _ = appId, let _ = self.navigationBar else { return }

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

        // Create system status bar and navigation bar
        let systemStatusBar = createSystemStatusBar(in: window)
        let navBar = createNavigationBar(in: window)

        // Configure colors based on window style
        configureBarColors(systemStatusBar: systemStatusBar, navBar: navBar)

        // Setup drag behavior and add to content view
        setupSystemStatusBarBehavior(systemStatusBar)
        contentView.addSubview(systemStatusBar)
        contentView.addSubview(navBar)

        // Setup Auto Layout constraints for system status bar and navbar
        NSLayoutConstraint.activate([
            // System status bar at the top of content view
            systemStatusBar.topAnchor.constraint(equalTo: contentView.topAnchor),
            systemStatusBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            systemStatusBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            systemStatusBar.heightAnchor.constraint(equalToConstant: Layout.systemStatusBarHeight),

            // Navbar directly below system status bar
            navBar.topAnchor.constraint(equalTo: systemStatusBar.bottomAnchor),
            navBar.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            navBar.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            navBar.heightAnchor.constraint(equalToConstant: Layout.navBarHeight)
        ])

        // Store references
        self.systemStatusBar = systemStatusBar
        self.navigationBar = navBar

        // Apply initial navigation configuration now that navigationBar is set
        applyInitialNavigationConfiguration()

        // Setup capsule buttons for capsule style (only in single app mode)
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle && appId != nil {
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

    private func createSystemStatusBar(in window: NSWindow) -> NSView {
        guard let contentView = window.contentView else {
            fatalError("Window must have a content view")
        }

        // Create system status bar positioned at the top of the content view
        let statusBar = NSView(frame: NSRect(
            x: 0,
            y: contentView.frame.height - Layout.systemStatusBarHeight,
            width: contentView.frame.width,
            height: Layout.systemStatusBarHeight
        ))
        statusBar.wantsLayer = true
        statusBar.translatesAutoresizingMaskIntoConstraints = false

        // Create time label on the left
        let timeLabel = createTimeLabel()
        statusBar.addSubview(timeLabel)
        self.timeLabel = timeLabel

        // Create battery view on the right
        let batteryView = createBatteryView()
        statusBar.addSubview(batteryView)
        self.batteryView = batteryView

        // Create notch view in the center
        let notchView = createNotchView()
        statusBar.addSubview(notchView)
        self.notchView = notchView

        // Setup constraints for status bar components
        setupSystemStatusBarConstraints(statusBar: statusBar, timeLabel: timeLabel, batteryView: batteryView, notchView: notchView)

        return statusBar
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

    private func configureBarColors(systemStatusBar: NSView, navBar: NSView) {
        let backgroundColor: NSColor = LxAppWindowManager.shared.windowStyle == .capsuleStyle
            ? .clear
            : .controlBackgroundColor

        systemStatusBar.layer?.backgroundColor = backgroundColor.cgColor
        navBar.layer?.backgroundColor = backgroundColor.cgColor
    }

    private func createTimeLabel() -> NSTextField {
        let label = NSTextField()
        label.isEditable = false
        label.isBordered = false
        label.backgroundColor = NSColor.clear
        label.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        label.textColor = NSColor.labelColor
        label.alignment = .left
        label.translatesAutoresizingMaskIntoConstraints = false
        label.stringValue = "9:41"
        return label
    }

    private func createBatteryView() -> NSView {
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false

        let outline = NSView()
        outline.wantsLayer = true
        outline.layer?.borderWidth = 1.0
        outline.layer?.borderColor = NSColor.labelColor.cgColor
        outline.layer?.cornerRadius = 2.0
        outline.translatesAutoresizingMaskIntoConstraints = false

        let fill = NSView()
        fill.wantsLayer = true
        fill.layer?.backgroundColor = NSColor.systemGreen.cgColor
        fill.layer?.cornerRadius = 1.0
        fill.translatesAutoresizingMaskIntoConstraints = false

        let tip = NSView()
        tip.wantsLayer = true
        tip.layer?.backgroundColor = NSColor.labelColor.cgColor
        tip.layer?.cornerRadius = 1.0
        tip.translatesAutoresizingMaskIntoConstraints = false

        container.addSubview(outline)
        container.addSubview(fill)
        container.addSubview(tip)

        NSLayoutConstraint.activate([
            outline.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            outline.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            outline.widthAnchor.constraint(equalToConstant: 22),
            outline.heightAnchor.constraint(equalToConstant: 11),

            fill.leadingAnchor.constraint(equalTo: outline.leadingAnchor, constant: 1),
            fill.trailingAnchor.constraint(equalTo: outline.trailingAnchor, constant: -1),
            fill.topAnchor.constraint(equalTo: outline.topAnchor, constant: 1),
            fill.bottomAnchor.constraint(equalTo: outline.bottomAnchor, constant: -1),

            tip.leadingAnchor.constraint(equalTo: outline.trailingAnchor, constant: 1),
            tip.centerYAnchor.constraint(equalTo: outline.centerYAnchor),
            tip.widthAnchor.constraint(equalToConstant: 2),
            tip.heightAnchor.constraint(equalToConstant: 6),

            container.widthAnchor.constraint(equalToConstant: 26),
            container.heightAnchor.constraint(equalToConstant: 11)
        ])

        return container
    }

    private func createNotchView() -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.translatesAutoresizingMaskIntoConstraints = false

        let spec = Layout.currentNotchSpec
        if spec.width > 0 && spec.height > 0 {
            let notch = NSView()
            notch.wantsLayer = true
            notch.layer?.backgroundColor = NSColor.black.cgColor
            notch.layer?.cornerRadius = spec.cornerRadius
            notch.translatesAutoresizingMaskIntoConstraints = false

            container.addSubview(notch)
            NSLayoutConstraint.activate([
                notch.centerXAnchor.constraint(equalTo: container.centerXAnchor),
                notch.topAnchor.constraint(equalTo: container.topAnchor),
                notch.widthAnchor.constraint(equalToConstant: spec.width),
                notch.heightAnchor.constraint(equalToConstant: spec.height)
            ])
        }

        return container
    }

    private func setupSystemStatusBarConstraints(statusBar: NSView, timeLabel: NSTextField, batteryView: NSView, notchView: NSView) {
        NSLayoutConstraint.activate([
            timeLabel.leadingAnchor.constraint(equalTo: statusBar.leadingAnchor, constant: Layout.statusBarSideMargin),
            timeLabel.centerYAnchor.constraint(equalTo: statusBar.centerYAnchor),

            batteryView.trailingAnchor.constraint(equalTo: statusBar.trailingAnchor, constant: -Layout.statusBarSideMargin),
            batteryView.centerYAnchor.constraint(equalTo: statusBar.centerYAnchor),

            notchView.leadingAnchor.constraint(equalTo: statusBar.leadingAnchor),
            notchView.trailingAnchor.constraint(equalTo: statusBar.trailingAnchor),
            notchView.topAnchor.constraint(equalTo: statusBar.topAnchor),
            notchView.bottomAnchor.constraint(equalTo: statusBar.bottomAnchor)
        ])
    }

    private func applyInitialNavigationConfiguration() {
        guard let _ = appId, let path = path, let navigationBar = navigationBar else {
            os_log("applyInitialNavigationConfiguration: missing required parameters", log: Self.log, type: .error)
            return
        }

        let pageConfig = getPageConfig()
        if let config = pageConfig {
            if config.show_navbar {
                updateNavigationBarWithConfig(config)
                navigationBar.isHidden = false
            } else {
                navigationBar.isHidden = true
            }
            updateIndependentNavigationButton(config)
        } else {
            navigationBar.isHidden = false
            navigationBar.layer?.backgroundColor = NSColor.systemBlue.cgColor
            independentNavigationButton?.isHidden = true
        }

        updateWindowTitle(for: path)
    }

    /// Clean data-driven navigation bar update
    public func updateNavigationBarWithState(_ state: NavigationBarState?) {
        guard let navigationBar = self.navigationBar else { return }

        if let state = state {
            updateNavigationBarWithConfig(state)
            navigationBar.isHidden = !state.show_navbar
            updateIndependentNavigationButton(state)
        } else {
            navigationBar.isHidden = true
            independentNavigationButton?.isHidden = true
        }
    }

    private func setupFloatingCapsuleButtons(in contentView: NSView) {
        // Prevent duplicate creation
        if floatingCapsuleContainer != nil {
            return
        }

        let capsuleContainer = NSView()
        capsuleContainer.wantsLayer = true
        capsuleContainer.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.92).cgColor
        capsuleContainer.layer?.cornerRadius = Layout.capsuleContainerHeight / 2
        capsuleContainer.translatesAutoresizingMaskIntoConstraints = false
        capsuleContainer.shadow = NSShadow()
        capsuleContainer.layer?.shadowColor = NSColor.black.cgColor
        capsuleContainer.layer?.shadowOpacity = 0.12
        capsuleContainer.layer?.shadowOffset = CGSize(width: 0, height: 1)
        capsuleContainer.layer?.shadowRadius = 4

        contentView.addSubview(capsuleContainer)

        let buttons = [
            createFloatingCapsuleButton(image: LxAppCapsuleButtons.createThreeDotsImage(), target: self, action: #selector(moreButtonClicked)),
            createFloatingCapsuleButton(image: LxAppCapsuleButtons.createMinimizeButtonImage(), target: self, action: #selector(minimizeButtonClicked)),
            createFloatingCapsuleButton(image: LxAppCapsuleButtons.createCloseButtonImage(), target: self, action: #selector(closeButtonClicked))
        ]

        buttons.forEach { capsuleContainer.addSubview($0) }

        let navBarCenterOffset = Layout.systemStatusBarHeight + (Layout.navBarHeight - Layout.capsuleContainerHeight) / 2
        NSLayoutConstraint.activate([
            capsuleContainer.topAnchor.constraint(equalTo: contentView.topAnchor, constant: navBarCenterOffset),
            capsuleContainer.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -Layout.capsuleTrailingMargin),
            capsuleContainer.widthAnchor.constraint(equalToConstant: Layout.capsuleContainerWidth),
            capsuleContainer.heightAnchor.constraint(equalToConstant: Layout.capsuleContainerHeight)
        ])

        // Layout buttons
        let buttonWidth: CGFloat = 20
        let buttonHeight: CGFloat = 24
        let edgeSpacing: CGFloat = 8
        let buttonSpacing: CGFloat = 8
        let buttonY = (Layout.capsuleContainerHeight - buttonHeight) / 2

        for (index, button) in buttons.enumerated() {
            let buttonX = edgeSpacing + CGFloat(index) * (buttonWidth + buttonSpacing)
            button.frame = NSRect(x: buttonX, y: buttonY, width: buttonWidth, height: buttonHeight)
        }

        self.floatingCapsuleContainer = capsuleContainer
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
        return button
    }

    private func setupSystemStatusBarBehavior(_ systemStatusBar: NSView) {
        let dragView = DraggableView()
        dragView.windowController = self
        dragView.frame = systemStatusBar.bounds
        dragView.autoresizingMask = [.width, .height]
        systemStatusBar.addSubview(dragView, positioned: .below, relativeTo: nil)

        [timeLabel, batteryView, notchView].compactMap { $0 }.forEach {
            systemStatusBar.addSubview($0, positioned: .above, relativeTo: dragView)
        }
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
        let systemStatusBarHeight: CGFloat = Layout.systemStatusBarHeight
        let navBarHeight: CGFloat = Layout.navBarHeight
        let newTopOffset: CGFloat

        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            // Use the passed pageConfig parameter (already cached)
            if pageConfig?.show_navbar == false {
                // Hidden navbar: WebView covers entire area for full transparency effect
                newTopOffset = 0
            } else {
                // Default navigation style: WebView below both system status bar and navigation bar
                newTopOffset = systemStatusBarHeight + navBarHeight
            }
        } else {
            // Tab style: only system title bar (no custom system status bar)
            newTopOffset = 0  // System title bar is handled by macOS
        }

        // Update the view controller's getTopMargin method instead of trying to modify constraints
        viewController.updateTopMargin(newTopOffset)

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

    /// Update independent navigation button visibility and type
    private func updateIndependentNavigationButton(_ state: NavigationBarState) {
        let shouldShow = !state.show_navbar && (state.show_back_button || state.show_home_button)

        if shouldShow {
            if independentNavigationButton == nil {
                createIndependentNavigationButton(isBackButton: state.show_back_button)
            }
            independentNavigationButton?.isHidden = false
            updateIndependentButtonType(isBackButton: state.show_back_button)
        } else {
            independentNavigationButton?.isHidden = true
        }
    }

    /// Create independent navigation button
    private func createIndependentNavigationButton(isBackButton: Bool) {
        guard let contentView = window?.contentView else { return }

        let hostingView = NSHostingView(rootView:
            NavigationButton(isBackButton: isBackButton) { [weak self] in
                self?.handleIndependentNavigationButtonClick()
            }
        )

        hostingView.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(hostingView)

        let topPosition = Layout.systemStatusBarHeight + Layout.navBarHeight - Layout.navButtonBottomOffset - Layout.navButtonSize

        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: Layout.navButtonMargin),
            hostingView.topAnchor.constraint(equalTo: contentView.topAnchor, constant: topPosition),
            hostingView.widthAnchor.constraint(equalToConstant: Layout.navButtonSize),
            hostingView.heightAnchor.constraint(equalToConstant: Layout.navButtonSize)
        ])

        independentNavigationButton = hostingView
    }

    /// Update independent button type (back vs home)
    private func updateIndependentButtonType(isBackButton: Bool) {
        guard let hostingView = independentNavigationButton as? NSHostingView<NavigationButton> else { return }

        // Update the SwiftUI view with new button type
        hostingView.rootView = NavigationButton(isBackButton: isBackButton) { [weak self] in
            self?.handleIndependentNavigationButtonClick()
        }
    }

    /// Handle independent navigation button click
    @objc private func handleIndependentNavigationButtonClick() {
        // Get current navigation state to determine button type
        if let appId = appId, let path = path {
            let navState = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
            if navState?.show_back_button == true {
                backButtonClicked()
            } else if navState?.show_home_button == true {
                homeButtonClicked()
            }
        }
    }

    private func updateNavigationBarButtons(_ config: NavigationBarState) {
        guard let navigationBar = self.navigationBar else {
            print("NavigationBar is nil, cannot add buttons")
            return
        }

        // Remove existing navbar buttons (keep title label)
        navigationBar.subviews.filter { !($0 is NSTextField) }.forEach { $0.removeFromSuperview() }

        // Use iOS-style self-drawn buttons instead of system buttons
        let buttonSize: CGFloat = 24 // Even smaller size for better fit
        let buttonY: CGFloat = 12 // Move up more to avoid bottom overlap (AppKit coordinates)
        let buttonX: CGFloat = 16 // Start from left edge with proper margin

        // Use iOS-style self-drawn navigation buttons
        if config.show_back_button {
            let backButtonView = createiOSStyleNavigationButton(
                isBackButton: true,
                frame: NSRect(x: buttonX, y: buttonY, width: buttonSize, height: buttonSize),
                action: #selector(backButtonClicked)
            )
            navigationBar.addSubview(backButtonView)
        } else if config.show_home_button {
            let homeButtonView = createiOSStyleNavigationButton(
                isBackButton: false,
                frame: NSRect(x: buttonX, y: buttonY, width: buttonSize, height: buttonSize),
                action: #selector(homeButtonClicked)
            )
            navigationBar.addSubview(homeButtonView)
        }
    }

    /// Create iOS-style self-drawn navigation button using SwiftUI
    private func createiOSStyleNavigationButton(isBackButton: Bool, frame: NSRect, action: Selector) -> NSView {
        let hostingView = NSHostingView(rootView:
            NavigationButton(isBackButton: isBackButton) { [weak self] in
                self?.perform(action)
            }
        )
        hostingView.frame = frame
        return hostingView
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
        macOSLxApp.navigate(appId: appId, path: path, navigationType: .launch)
    }

    private func switchToTab(_ appId: String) {
        let isNewViewController = viewControllers[appId] == nil

        let viewController = viewControllers[appId] ?? {
            let currentPath = LxAppCore.getLastActivePath(for: appId, defaultPath: "/")
            let vc = macOSLxAppViewController(appId: appId, path: currentPath)
            viewControllers[appId] = vc
            return vc
        }()

        // Only call onLxappOpened for newly created view controllers
        if isNewViewController {
            let currentPath = LxAppCore.getLastActivePath(for: appId, defaultPath: "/")
            let _ = onLxappOpened(appId, currentPath)
        }

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
                topOffset = Layout.navBarHeight  // Use actual navbar height
            }
        } else {
            // Tab style: space for SwiftUI custom title bar only
            topOffset = Layout.swiftUITitleBarHeight
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
            // Update system status bar to be transparent when navbar is transparent
            updateSystemStatusBarColor(NSColor.clear)
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

            // Update system status bar to follow navbar color, but ensure time is visible
            updateSystemStatusBarColor(backgroundColor)
            setupNavigationBarTitle(in: navigationBar)
            if let titleLabel = findTitleLabel(in: navigationBar) {
                titleLabel.stringValue = config.title_text.toString()
                titleLabel.textColor = config.text_style.toString() == "white" ? NSColor.white : NSColor.black
            }

        }

        // Always update buttons regardless of navbar visibility
        updateNavigationBarButtons(config)
    }

    private func updateSystemStatusBarColor(_ color: NSColor) {
        guard let systemStatusBar = self.systemStatusBar else { return }

        // Ensure system status bar has a layer
        if systemStatusBar.layer == nil {
            systemStatusBar.wantsLayer = true
        }

        systemStatusBar.layer?.backgroundColor = color.cgColor

        let textColor = isColorDark(color) ? NSColor.white : NSColor.black

        timeLabel?.textColor = textColor

        batteryView?.subviews.forEach { subview in
            if subview.layer?.borderColor != nil {
                subview.layer?.borderColor = textColor.cgColor
            }
            if subview.layer?.backgroundColor == NSColor.labelColor.cgColor {
                subview.layer?.backgroundColor = textColor.cgColor
            }
        }

        if let notchView = self.notchView {
            let spec = Layout.currentNotchSpec
            notchView.alphaValue = spec.width > 0 && spec.height > 0 ? 1.0 : 0.0
            notchView.subviews.first?.layer?.backgroundColor = NSColor.black.cgColor
        }
    }

    private func isColorDark(_ color: NSColor) -> Bool {
        guard let rgbColor = color.usingColorSpace(.deviceRGB) else { return false }
        let luminance = 0.299 * rgbColor.redComponent + 0.587 * rgbColor.greenComponent + 0.114 * rgbColor.blueComponent
        return luminance < 0.5
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
