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

    // Helper method to get top margin based on window style
    private func getTopMargin() -> CGFloat {
        // In tab style, the window controller handles the tab bar space
        // so we don't need additional top margin
        let currentStyle = macOSWindowController.getWindowStyle()
        if currentStyle == .tabStyle {
            return 0
        }
        return macOSWindowController.getTopMarginForCurrentStyle()
    }

    // Properties
    internal var appId: String
    private var initialPath: String
    private var webViewContainer: NSView!
    private var tabBarView: NSView?
    private var currentWebView: WKWebView?
    public var tabBarConfig: TabBarConfig?

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
        if let tabBar = tabBarView, let tabBarConfig = getTabBarConfig(appId) {
            view.addSubview(tabBar)

            // Check if TabBar is transparent using platform extension
            let isTabBarTransparent = TabBarHelper.isTransparent(tabBarConfig.background_color.toString())

            // Get TabBar height from config dimension
            let tabBarHeight: CGFloat = CGFloat(tabBarConfig.dimension)

            // Set TabBar position based on config - support all four positions
            var tabBarConstraints: [NSLayoutConstraint] = []

            switch tabBarConfig.position {
            case 0: // bottom
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.heightAnchor.constraint(equalToConstant: tabBarHeight)
                ]

            case 1: // top
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                    tabBar.heightAnchor.constraint(equalToConstant: tabBarHeight)
                ]

            case 2: // left
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.widthAnchor.constraint(equalToConstant: tabBarHeight) // Use configured dimension
                ]

            case 3: // right
                tabBarConstraints = [
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.widthAnchor.constraint(equalToConstant: tabBarHeight) // Use configured dimension
                ]

            default: // fallback to bottom
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.heightAnchor.constraint(equalToConstant: tabBarHeight)
                ]
            }

            NSLayoutConstraint.activate(tabBarConstraints)
            os_log("[TabBar] Activated TabBar constraints for position: %@", log: Self.log, type: .info, String(describing: tabBarConfig.position))

            // Set WebView container constraints based on TabBar position and transparency
            var webViewConstraints: [NSLayoutConstraint] = []

            if !isTabBarTransparent {
                // Non-transparent TabBar: WebView avoids TabBar area
                switch tabBarConfig.position {
                case 0: // bottom
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: tabBar.topAnchor)
                    ]

                case 1: // top
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: tabBar.bottomAnchor),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]

                case 2: // left
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: tabBar.trailingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]

                case 3: // right
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: tabBar.leadingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]

                default: // fallback to bottom
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: tabBar.topAnchor)
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
        guard let tabBarConfig = getTabBarConfig(appId) else {
            os_log("Failed to get TabBar config for appId: %@", log: Self.log, type: .error, appId)
            return
        }

        // Store config as instance property
        self.tabBarConfig = tabBarConfig

        // Create simple TabBar first - get it working, then add grouping
        let tabBar = NSView()
        tabBar.wantsLayer = true

        // Set background color using platform extension
        let resolvedColor = TabBarHelper.resolvedBackgroundColor(tabBarConfig.background_color.toString(), isVertical: true)
        tabBar.layer?.backgroundColor = resolvedColor.cgColor

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        // Set minimum size constraints based on position using configured dimension
        let isVertical = tabBarConfig.position == 2 || tabBarConfig.position == 3 // left, right
        let configuredDimension = CGFloat(tabBarConfig.dimension)
        if isVertical {
            // Vertical TabBar: minimum width
            tabBar.widthAnchor.constraint(greaterThanOrEqualToConstant: configuredDimension).isActive = true
        } else {
            // Horizontal TabBar: minimum height for proper icon-text layout
            tabBar.heightAnchor.constraint(greaterThanOrEqualToConstant: configuredDimension).isActive = true
        }

        // Create stack view with correct orientation based on TabBar position
        // isVertical already defined above
        let stackView = NSStackView()
        stackView.orientation = isVertical ? .vertical : .horizontal
        stackView.distribution = .fill
        stackView.spacing = 0  // Let spacers handle spacing
        stackView.translatesAutoresizingMaskIntoConstraints = false

        // Set alignment for centering
        if isVertical {
            stackView.alignment = .centerX
        } else {
            stackView.alignment = .centerY
        }

        // Use the config method for consistent grouping logic
        let (startItems, centerItems, endItems) = tabBarConfig.getGroupedItems(appId: appId)
        let hasAnyGroupField = !startItems.isEmpty || !endItems.isEmpty

        if hasAnyGroupField {

            // Create start container
            if !startItems.isEmpty {
                let startContainer = createGroupContainer(items: startItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                stackView.addArrangedSubview(startContainer)
            }

            // Add flexible spacer
            let spacer = createFlexibleSpacer(isVertical: isVertical)
            stackView.addArrangedSubview(spacer)

            // Create end container
            if !endItems.isEmpty {
                let endContainer = createGroupContainer(items: endItems, spacing: TabBarConstants.DEFAULT_SPACING, isVertical: isVertical)
                stackView.addArrangedSubview(endContainer)
            }

        } else {
            // Centered layout for non-grouped items
            let centerContainer = createGroupContainer(items: centerItems, spacing: TabBarConstants.CENTER_SPACING, isVertical: isVertical)
            stackView.addArrangedSubview(centerContainer)
        }

        tabBar.addSubview(stackView)

        // Set stack view constraints to fill the TabBar (let internal spacers handle positioning)
        NSLayoutConstraint.activate([
            stackView.leadingAnchor.constraint(equalTo: tabBar.leadingAnchor, constant: 4),
            stackView.trailingAnchor.constraint(equalTo: tabBar.trailingAnchor, constant: -4),
            stackView.topAnchor.constraint(equalTo: tabBar.topAnchor, constant: 8),
            stackView.bottomAnchor.constraint(equalTo: tabBar.bottomAnchor, constant: -8)
        ])

        self.tabBarView = tabBar
    }

    /// Create a container for a group of tab items
    private func createGroupContainer(items: [TabBarItem], spacing: CGFloat, isVertical: Bool) -> NSStackView {
        let container = NSStackView()
        container.orientation = isVertical ? .vertical : .horizontal
        container.distribution = isVertical ? .fill : .equalSpacing  // equalSpacing for horizontal to give more space
        container.spacing = spacing
        container.translatesAutoresizingMaskIntoConstraints = false

        // Set content hugging priority to prevent expansion
        if isVertical {
            container.setContentHuggingPriority(.defaultHigh, for: .vertical)
        } else {
            container.setContentHuggingPriority(.defaultHigh, for: .horizontal)
            // For horizontal, ensure minimum height
            container.heightAnchor.constraint(greaterThanOrEqualToConstant: 60).isActive = true
        }

        for (index, item) in items.enumerated() {
            // Find the global index of this item
            let allItems = tabBarConfig?.getItems(appId: appId) ?? []
            let globalIndex = allItems.firstIndex { $0.page_path.toString() == item.page_path.toString() } ?? index
            let button = createTabButton(item: item, index: globalIndex)
            container.addArrangedSubview(button)
        }

        return container
    }

    /// Create a flexible spacer for layout
    private func createFlexibleSpacer(isVertical: Bool) -> NSView {
        let spacer = NSView()
        if isVertical {
            spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
            spacer.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
            spacer.heightAnchor.constraint(greaterThanOrEqualToConstant: TabBarConstants.MINIMAL_SPACER_SIZE).isActive = true
        } else {
            spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
            spacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            spacer.widthAnchor.constraint(greaterThanOrEqualToConstant: TabBarConstants.MINIMAL_SPACER_SIZE).isActive = true
        }
        return spacer
    }

    /// Create a tab button with better layout for bottom tabbar
    private func createTabButton(item: TabBarItem, index: Int) -> NSButton {
        let button = NSButton()
        button.title = item.text.toString()
        button.isBordered = false
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor
        button.tag = index
        button.target = self
        button.action = #selector(tabButtonTapped(_:))
        button.translatesAutoresizingMaskIntoConstraints = false

        let isVertical = tabBarConfig?.position == 2 || tabBarConfig?.position == 3 // left, right
        let isSelected = item.page_path.toString() == initialPath

        // Configure image position and scaling
        button.imagePosition = .imageAbove
        button.imageScaling = .scaleProportionallyDown

        // Configure size and font based on orientation
        let fontSize: CGFloat = isVertical ? 10 : 11
        let buttonHeight: CGFloat = isVertical ? 50 : 56

        button.font = NSFont.systemFont(ofSize: fontSize, weight: .medium)
        button.heightAnchor.constraint(equalToConstant: buttonHeight).isActive = true

        if !isVertical {
            button.widthAnchor.constraint(greaterThanOrEqualToConstant: 80).isActive = true
        }

        // Set colors from config
        button.contentTintColor = getTabColor(selected: isSelected)

        // Set icon if available
        let iconPath = item.icon_path.toString()
        if !iconPath.isEmpty {
            setButtonIcon(button: button, iconPath: iconPath, isSelected: isSelected, item: item)
        }

        return button
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

        let items = tabBarConfig.getItems(appId: appId)
        for (index, item) in items.enumerated() {
            if item.page_path.toString() == targetPath {
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
        guard let tabBarView = tabBarView,
              let stackView = tabBarView.subviews.first as? NSStackView else { return }

        let items = tabBarConfig?.getItems(appId: appId) ?? []

        // Recursively find all buttons in the stack view hierarchy
        func findAllButtons(in view: NSView) -> [NSButton] {
            var buttons: [NSButton] = []

            if let button = view as? NSButton {
                buttons.append(button)
            } else if let stackView = view as? NSStackView {
                for arrangedSubview in stackView.arrangedSubviews {
                    buttons.append(contentsOf: findAllButtons(in: arrangedSubview))
                }
            }

            return buttons
        }

        let allButtons = findAllButtons(in: stackView)

        // Update all buttons
        for button in allButtons {
            let isSelected = button.tag == selectedIndex
            button.contentTintColor = getTabColor(selected: isSelected)

            if button.tag < items.count {
                let configItem = items[button.tag]
                let iconPath = configItem.icon_path.toString()
                if !iconPath.isEmpty {
                    setButtonIcon(button: button, iconPath: iconPath, isSelected: isSelected, item: configItem)
                }
            }
        }
    }

    @objc private func tabButtonTapped(_ sender: NSButton) {
        let index = sender.tag
        guard let tabBarConfig = tabBarConfig else { return }
        let items = tabBarConfig.getItems(appId: appId)
        guard index >= 0 && index < items.count else { return }

        let item = items[index]
        switchPage(targetPath: item.page_path.toString())
    }

    private func getResourcesPath() -> String {
        let executablePath = Bundle.main.executablePath ?? ""
        let executableDir = (executablePath as NSString).deletingLastPathComponent
        return "\(executableDir)/Resources"
    }

    private func getTabColor(selected: Bool) -> NSColor {
        guard let tabBarConfig = tabBarConfig else {
            return selected ? NSColor.controlAccentColor : NSColor.secondaryLabelColor
        }

        if selected {
            if let selectedColor = TabBarHelper.parseColor(tabBarConfig.selected_color.toString()) {
                return selectedColor
            }
            return NSColor.controlAccentColor
        } else {
            if let color = TabBarHelper.parseColor(tabBarConfig.color.toString()) {
                return color
            }
            return NSColor.secondaryLabelColor
        }
    }

    private func setButtonIcon(button: NSButton, iconPath: String, isSelected: Bool, item: TabBarItem) {
        var image: NSImage?

        // Use selected icon if available and selected
        let selectedIconPath = item.selected_icon_path.toString()
        let actualIconPath = (isSelected && !selectedIconPath.isEmpty) ? selectedIconPath : iconPath

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

        // Set icon size based on TabBar position
        let isVertical = tabBarConfig?.position == 2 || tabBarConfig?.position == 3

        if let image = image {
            let iconSize: CGFloat = isVertical ? 20 : 24
            button.image = resizeImage(image, to: NSSize(width: iconSize, height: iconSize))
            button.imageScaling = .scaleNone
            os_log(" Icon loaded successfully: size=%@x%@", log: Self.log, type: .debug, "\(iconSize)", "\(iconSize)")
        } else {
            os_log(" Failed to load icon: path=%@", log: Self.log, type: .error, actualIconPath)
        }

        // Add spacing between image and title for horizontal TabBar
        if !isVertical {
            // Create space between icon and text
            button.imagePosition = .imageAbove
            button.imageHugsTitle = false

            // Add padding between image and title
            if let cell = button.cell as? NSButtonCell {
                cell.imageDimsWhenDisabled = false
            }
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
