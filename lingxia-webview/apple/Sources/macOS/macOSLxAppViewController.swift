#if os(macOS)
import Foundation
import WebKit
import os.log
import Cocoa
import CLingXiaFFI

private let lxAppViewControllerLog = OSLog(subsystem: "LingXia", category: "LxAppView")

@MainActor
public class macOSLxAppViewController: NSViewController, WKNavigationDelegate {
    nonisolated private static let log = lxAppViewControllerLog

    //  - Constants
    private static let TAB_BAR_HEIGHT: CGFloat = 40
    internal static let DEFAULT_NAV_BAR_HEIGHT: CGFloat = 32 // This constant is no longer used for layout, but kept for reference if needed elsewhere

    // Helper method to get top margin based on window style
    private func getTopMargin() -> CGFloat {
        return macOSLxAppWindowController.getTopMarginForCurrentStyle()
    }

    // Properties
    internal var appId: String
    private var initialPath: String
    private var webViewContainer: NSView!
    private var tabBarView: NSView?
    private var currentWebView: WKWebView?
    public var tabBarConfig: TabBarConfig!

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

    public init(appId: String, path: String) {
        self.appId = appId
        self.initialPath = path
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
        switchPageObserver.map(NotificationCenter.default.removeObserver)
    }

    public override func loadView() {
        view = NSView()
        view.wantsLayer = true
        view.layer?.backgroundColor = AppKit.NSColor.windowBackgroundColor.cgColor
    }

    public override func viewDidLoad() {
        super.viewDidLoad()

        // Set view background color
        view.wantsLayer = true
        view.layer?.backgroundColor = AppKit.NSColor.windowBackgroundColor.cgColor

        // Let the view occupy the full contentView
        if let window = view.window, let contentView = window.contentView {
            view.frame = contentView.bounds
        }

        // Setup UI components
        setupLayout()
        setupNotificationObservers()
        setupKeyboardShortcuts()

        loadWebViewContent()

        // Force layout update
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    // UI Setup
    private func setupLayout() {
        // Set main view background
        view.wantsLayer = true
        view.layer?.backgroundColor = AppKit.NSColor.windowBackgroundColor.cgColor

        // Let the view occupy the full contentView
        if let window = view.window, let contentView = window.contentView {
            view.frame = contentView.bounds
        }

        // Create TabBar first
        setupTabBar()

        // Create WebView container
        setupWebViewContainer()

        // Add TabBar to view hierarchy and set constraints based on position and transparency
        if let tabBar = tabBarView, let tabBarConfigRust = getTabBarConfig(appId), let tabBarConfig = TabBarConfig.fromJson(tabBarConfigRust.toString()) {
            view.addSubview(tabBar)

            // Check if TabBar is transparent using platform extension
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBarConfig.backgroundColor)

            // Get TabBar height from constants
            let tabBarHeight: CGFloat = Self.TAB_BAR_HEIGHT

            // Set TabBar position based on config - support all four positions
            var tabBarConstraints: [NSLayoutConstraint] = []

            switch tabBarConfig.position {
            case .bottom:
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.heightAnchor.constraint(equalToConstant: tabBarHeight)
                ]

            case .top:
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor),
                    tabBar.heightAnchor.constraint(equalToConstant: tabBarHeight)
                ]

            case .left:
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.widthAnchor.constraint(equalToConstant: 80) // Same width as independent implementation
                ]

            case .right:
                tabBarConstraints = [
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.widthAnchor.constraint(equalToConstant: 80) // Same width as independent implementation
                ]
            }

            NSLayoutConstraint.activate(tabBarConstraints)
            os_log("[TabBar] Activated TabBar constraints for position: %@", log: Self.log, type: .info, String(describing: tabBarConfig.position))

            // Set WebView container constraints based on TabBar position and transparency
            var webViewConstraints: [NSLayoutConstraint] = []

            if !isTabBarTransparent {
                // Non-transparent TabBar: WebView avoids TabBar area
                switch tabBarConfig.position {
                case .bottom:
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: tabBar.topAnchor)
                    ]

                case .top:
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: tabBar.bottomAnchor),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]

                case .left:
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: tabBar.trailingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]

                case .right:
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: tabBar.leadingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]
                }
            } else {
                // Transparent TabBar: WebView extends full area, TabBar overlays
                webViewConstraints = [
                    webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                    webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                ]
            }

            NSLayoutConstraint.activate(webViewConstraints)
            os_log("[TabBar] WebView container constrained for position: %@ (transparent: %@)", log: Self.log, type: .info, String(describing: tabBarConfig.position), isTabBarTransparent ? "true" : "false")
        } else {
            // No TabBar, WebView container takes full height but leaves space for title bar
            NSLayoutConstraint.activate([
                webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
            ])
        }

        // Force layout update
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    private func setupWebViewContainer() {
        webViewContainer = NSView()
        webViewContainer.wantsLayer = true
        webViewContainer.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(webViewContainer)
    }

    private func setupTabBar(config: TabBarConfig? = nil) {
        guard let tabBarConfigRust = getTabBarConfig(appId) else {
            return
        }

        let tabBarConfigJson = tabBarConfigRust.toString()
        guard let tabBarConfig = TabBarConfig.fromJson(tabBarConfigJson) else {
            os_log("Failed to parse TabBar config for appId: %@", log: Self.log, type: .error, appId)
            return
        }

        // Store config as instance property
        self.tabBarConfig = tabBarConfig

        // Create macOS TabBar
        let tabBar = NSView()
        tabBar.wantsLayer = true

        // Set background color using platform extension
        let resolvedColor = tabBarConfig.resolvedBackgroundColor(isVertical: false)
        tabBar.layer?.backgroundColor = resolvedColor.cgColor

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        // Add tab buttons with orientation based on position
        let stackView = NSStackView()

        // Set orientation and spacing based on TabBar position
        switch tabBarConfig.position {
        case .left, .right:
            stackView.orientation = .vertical
            stackView.distribution = .equalSpacing  // Same as independent implementation
            stackView.spacing = 10  // Same spacing as independent implementation
        case .top, .bottom:
            stackView.orientation = .horizontal
            stackView.distribution = .fillEqually
            stackView.spacing = 8  // Standard spacing for horizontal layout
        }

        stackView.translatesAutoresizingMaskIntoConstraints = false

        for (index, item) in tabBarConfig.list.enumerated() {
            let button = NSButton()
            button.title = item.text ?? ""
            button.font = NSFont.systemFont(ofSize: 10, weight: .medium)
            button.isBordered = false
            button.wantsLayer = true
            button.layer?.backgroundColor = NSColor.clear.cgColor
            button.tag = index
            button.target = self
            button.action = #selector(tabButtonTapped(_:))
            button.translatesAutoresizingMaskIntoConstraints = false

            // Set colors from config using the same method as independent implementation
            let isSelected = item.pagePath == initialPath
            button.contentTintColor = getTabColor(selected: isSelected)

            // Set icon if available
            if !item.iconPath.isEmpty {
                setButtonIcon(button: button, iconPath: item.iconPath, isSelected: isSelected, item: item)
            }

            // Configure button layout based on TabBar position
            switch tabBarConfig.position {
            case .left, .right:
                // For vertical TabBar, use same layout as independent implementation
                button.imagePosition = .imageAbove
                button.imageScaling = .scaleProportionallyDown
                button.font = NSFont.systemFont(ofSize: 10, weight: .medium)
                // Set fixed height for vertical buttons (same as independent implementation)
                button.heightAnchor.constraint(equalToConstant: 50).isActive = true

            case .top, .bottom:
                // For horizontal TabBar, use standard layout
                button.imagePosition = .imageAbove
                button.imageScaling = .scaleProportionallyDown
                button.font = NSFont.systemFont(ofSize: 10, weight: .medium)
            }

            stackView.addArrangedSubview(button)
        }

        tabBar.addSubview(stackView)

        // Set StackView constraints based on TabBar position
        switch tabBarConfig.position {
        case .left, .right:
            // For vertical TabBar, use centerY constraint (same as independent implementation)
            NSLayoutConstraint.activate([
                stackView.leadingAnchor.constraint(equalTo: tabBar.leadingAnchor, constant: 4), // Reduced inset like independent implementation
                stackView.trailingAnchor.constraint(equalTo: tabBar.trailingAnchor, constant: -4),
                stackView.centerYAnchor.constraint(equalTo: tabBar.centerYAnchor)
            ])

        case .top, .bottom:
            // For horizontal TabBar, fill the entire area
            NSLayoutConstraint.activate([
                stackView.leadingAnchor.constraint(equalTo: tabBar.leadingAnchor, constant: 16),
                stackView.trailingAnchor.constraint(equalTo: tabBar.trailingAnchor, constant: -16),
                stackView.topAnchor.constraint(equalTo: tabBar.topAnchor),
                stackView.bottomAnchor.constraint(equalTo: tabBar.bottomAnchor)
            ])
        }

        self.tabBarView = tabBar
    }

    private func loadWebViewContent() {
        if let webView = WebViewManager.findWebView(appId: appId, path: initialPath) {
            showWebViewToUser(webView, path: initialPath)
        }

        webViewContainer.needsLayout = true
        webViewContainer.layoutSubtreeIfNeeded()
    }



    private func attachWebViewToContainer(_ webView: WKWebView) {
        currentWebView?.removeFromSuperview()
        currentWebView = webView

        webView.translatesAutoresizingMaskIntoConstraints = false
        webViewContainer.addSubview(webView)

        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: webViewContainer.topAnchor),
            webView.leadingAnchor.constraint(equalTo: webViewContainer.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: webViewContainer.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: webViewContainer.bottomAnchor)
        ])

        // Force layout update - use macOS compatible method
        #if os(macOS)
        webView.needsLayout = true
        webViewContainer.needsLayout = true
        webViewContainer.layoutSubtreeIfNeeded()
        #else
        // iOS version
        webView.setNeedsLayout()
        webView.layoutIfNeeded()
        webViewContainer.setNeedsLayout()
        webViewContainer.layoutIfNeeded()
        #endif

        // Ensure WebView is visible
        webView.isHidden = false
        #if os(iOS)
        webView.alpha = 1.0
        #endif
    }

    /// Unified method to show a WebView to the user - this is the ONLY place where onPageShow should be called
    private func showWebViewToUser(_ webView: WKWebView, path: String) {
        // Attach WebView to container (handles UI setup)
        attachWebViewToContainer(webView)

        // Hide previous WebView if different
        if let previousWebView = currentWebView, previousWebView != webView {
            previousWebView.isHidden = true
        }

        let _ = onPageShow(appId, path)
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

        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE), object: nil, queue: .main
        ) { [weak self] notification in
            let appId = notification.userInfo?["appId"] as? String
            let path = notification.userInfo?["path"] as? String
            Task { @MainActor in
                guard let self = self, let targetAppId = appId, let targetPath = path, targetAppId == self.appId else { return }

                self.switchPage(targetPath: targetPath)
            }
        }

    }

    private func setupKeyboardShortcuts() {
        // Add keyboard shortcut for back navigation (Cmd+Left Arrow or Escape)
        let backMenuItem = NSMenuItem(title: "Back", action: #selector(handleBackKeyPress), keyEquivalent: "\u{001B}") // Escape key
        backMenuItem.target = self

        // Also support Cmd+Left Arrow
        let backMenuItem2 = NSMenuItem(title: "Back", action: #selector(handleBackKeyPress), keyEquivalent: String(Character(UnicodeScalar(NSLeftArrowFunctionKey)!)))
        backMenuItem2.keyEquivalentModifierMask = .command
        backMenuItem2.target = self

        // Add to main menu if available
        if let mainMenu = NSApp.mainMenu {
            let appMenu = mainMenu.items.first
            appMenu?.submenu?.addItem(backMenuItem)
            appMenu?.submenu?.addItem(backMenuItem2)
        }
    }

    @objc private func handleBackKeyPress() {
        let result = onBackPressed(appId)
        if result {
            return
        }

        // No back navigation available, close window if not home app
        if appId != LxAppCore.getHomeLxAppId() {
            view.window?.close()
        }
    }

    //  - Page Switching
    public func switchPage(targetPath: String) {
        guard !appId.isEmpty else { return }

        if currentWebView?.currentPath == targetPath {
            return
        }

        self.initialPath = targetPath

        if let _ = tabBarView?.subviews.first as? NSStackView,
           let tabIndex = findTabIndexByPath(targetPath), tabIndex >= 0 {
            switchToTab(targetPath: targetPath, tabIndex: tabIndex)
        } else {
            navigateToPage(targetPath: targetPath)
        }

        LxAppCore.setLastActivePath(targetPath, for: appId)

        // Send notification for WindowController to update title (matching iOS/demo behavior)
        // This covers both TabBar switches and other page navigation
        NotificationCenter.default.post(
            name: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            userInfo: ["appId": appId, "path": targetPath]
        )
    }



    //  - Helper Methods
    private func findTabIndexByPath(_ targetPath: String) -> Int? {
        guard let tabBarConfig = tabBarConfig else { return nil }

        for (index, item) in tabBarConfig.list.enumerated() {
            if item.pagePath == targetPath {
                return index
            }
        }
        return nil
    }

    public func switchToTab(targetPath: String, tabIndex: Int) {
        // Find target WebView (should be created by Rust layer when needed)
        guard let targetWebView = WebViewManager.findWebView(appId: appId, path: targetPath) else {
            return
        }

        // Update TabBar UI (without triggering listener)
        updateTabBarSelection(selectedIndex: tabIndex)

        showWebViewToUser(targetWebView, path: targetPath)
    }

    private func navigateToPage(targetPath: String) {
        // Find WebView for the target page
        guard let newWebView = WebViewManager.findWebView(appId: appId, path: targetPath) else {
            return
        }

        showWebViewToUser(newWebView, path: targetPath)
    }

    private func updateTabBarSelection(selectedIndex: Int) {
        guard let stackView = tabBarView?.subviews.first as? NSStackView else { return }

        for (buttonIndex, arrangedSubview) in stackView.arrangedSubviews.enumerated() {
            if let button = arrangedSubview as? NSButton {
                let isSelected = buttonIndex == selectedIndex

                // Update button color using the same method as independent implementation
                button.contentTintColor = getTabColor(selected: isSelected)

                // Update icon if needed
                let configItem = tabBarConfig.list[buttonIndex]
                if !configItem.iconPath.isEmpty {
                    setButtonIcon(button: button, iconPath: configItem.iconPath, isSelected: isSelected, item: configItem)
                }
            }
        }
    }

    @objc private func tabButtonTapped(_ sender: NSButton) {
        let index = sender.tag
        guard index >= 0 && index < tabBarConfig.list.count else { return }

        let item = tabBarConfig.list[index]
        switchPage(targetPath: item.pagePath)
    }

    private func getResourcesPath() -> String {
        let executablePath = Bundle.main.executablePath ?? ""
        let executableDir = (executablePath as NSString).deletingLastPathComponent
        return "\(executableDir)/Resources"
    }

    private func getTabColor(selected: Bool) -> NSColor {
        if selected {
            if let selectedColor = tabBarConfig.parseColor(tabBarConfig.selectedColor) {
                return selectedColor
            }
            return NSColor(hexString: TabBarConfig.DEFAULT_SELECTED_COLOR) ?? NSColor.controlAccentColor
        } else {
            if let color = tabBarConfig.parseColor(tabBarConfig.color) {
                return color
            }
            return NSColor(hexString: TabBarConfig.DEFAULT_UNSELECTED_COLOR) ?? NSColor.secondaryLabelColor
        }
    }

    private func setButtonIcon(button: NSButton, iconPath: String, isSelected: Bool, item: TabBarItem) {
        var image: NSImage?

        // Use selected icon if available and selected
        let actualIconPath = (isSelected && !item.selectedIconPath.isEmpty) ? item.selectedIconPath : iconPath

        if actualIconPath.hasPrefix("SF:") {
            // System SF Symbol
            let symbolName = String(actualIconPath.dropFirst(3))
            if #available(macOS 11.0, *) {
                image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)
                image?.isTemplate = true
            }
        } else if actualIconPath.hasPrefix("/") {
            // Absolute path
            image = NSImage(contentsOfFile: actualIconPath)
        } else {
            // Try bundle first
            image = NSImage(named: actualIconPath)

            // If not found in bundle, try with appId in Resources directory
            if image == nil && !appId.isEmpty {
                let resourcesPath = getResourcesPath()
                let fullPath = "\(resourcesPath)/\(appId)/\(actualIconPath)"
                image = NSImage(contentsOfFile: fullPath)
                os_log("Loading icon from: %@", log: Self.log, type: .debug, fullPath)
            }
        }

        if let image = image {
            let iconSize: CGFloat = 24
            let resizedImage = resizeImage(image, to: NSSize(width: iconSize, height: iconSize))
            button.image = resizedImage
        }
    }

    private func resizeImage(_ image: NSImage, to size: NSSize) -> NSImage {
        let resizedImage = NSImage(size: size)
        resizedImage.lockFocus()

        // Draw image to fit size
        let drawRect = NSRect(origin: .zero, size: size)
        image.draw(in: drawRect)

        resizedImage.unlockFocus()
        resizedImage.isTemplate = image.isTemplate

        return resizedImage
    }

    // Helper method to check if a color is transparent
    private func isTransparentColor(_ color: NSColor) -> Bool {
        // Convert to calibrated RGB color space to access components
        let rgbColor = color.usingColorSpace(.sRGB) ?? color
        return rgbColor.alphaComponent < 0.1
    }

    // Helper method to check if a color string represents transparency
    private func isTransparentColor(_ colorString: String) -> Bool {
        return colorString.lowercased() == "transparent" || colorString.isEmpty
    }
}

#endif
