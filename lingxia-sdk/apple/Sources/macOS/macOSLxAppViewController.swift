#if os(macOS)
import Foundation
import WebKit
import os.log
import AppKit
import SwiftUI
import CLingXiaFFI

private let lxAppViewControllerLog = OSLog(subsystem: "LingXia", category: "LxAppView")

@MainActor
public class macOSLxAppViewController: NSViewController, WKNavigationDelegate, NavigationTabBarController, NavigationUIUpdater {
    nonisolated private static let log = lxAppViewControllerLog

    private var currentTopMargin: CGFloat = 0

    private func getTopMargin() -> CGFloat {
        return currentTopMargin
    }

    internal func updateTopMargin(_ newMargin: CGFloat) {
        currentTopMargin = newMargin
        refreshWebViewLayout()
    }

    private func refreshWebViewLayout() {
        guard let webViewContainer = webViewContainer else { return }

        view.removeConstraints(view.constraints.filter { constraint in
            constraint.firstItem === webViewContainer && constraint.firstAttribute == .top
        })

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
        ])

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    // Properties
    public var appId: String
    private var initialPath: String
    internal var currentPath: String
    private var webViewContainer: NSView!
    private var tabBarView: NSView?
    public var currentWebView: WKWebView?
    public var tabBarConfig: TabBar?
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?

    public init(appId: String, path: String) {
        self.appId = appId
        self.initialPath = path
        self.currentPath = path
        super.init(nibName: nil, bundle: nil)

        // Initialize top margin based on current page
        self.currentTopMargin = calculateInitialTopMargin()
    }

    private func calculateInitialTopMargin() -> CGFloat {
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            // Get config from window controller's cache to avoid duplicate calls
            if let windowController = view.window?.windowController as? LxAppWindowController {
                return windowController.getTopMarginForCurrentPage() - LxAppWindowController.Layout.dragBarHeight
            } else {
                // Fallback: assume navbar is shown
                return LxAppTheme.Metrics.navigationBarHeight
            }
        } else {
            // Tab style: 0pt - SwiftUI handles tab layout
            return 0
        }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
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

        // Create TabBar first
        setupTabBar()

        // Create WebView container
        setupWebViewContainer()

        // Add TabBar to view hierarchy and set constraints based on position and transparency
        if let tabBar = tabBarView, let tabBarConfig = lingxia.getTabBar(appId) {
            view.addSubview(tabBar)

            // Check if TabBar is transparent using platform extension
            let isTabBarTransparent = TabBar.isTransparent(tabBarConfig.background_color)

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

            case 1: // left
                tabBarConstraints = [
                    tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                    tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                    tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                    tabBar.widthAnchor.constraint(equalToConstant: tabBarHeight) // Use configured dimension
                ]

            case 2: // right
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

                case 1: // left
                    webViewConstraints = [
                        webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                        webViewContainer.leadingAnchor.constraint(equalTo: tabBar.trailingAnchor),
                        webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                        webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
                    ]

                case 2: // right
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

            // Apply transparency mode if TabBar is configured as transparent
            if isTabBarTransparent {
                tabBar.wantsLayer = true
                tabBar.layer?.backgroundColor = NSColor.clear.cgColor
            }
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

    private func setupTabBar(config: TabBar? = nil) {
        guard let tabBarConfig = lingxia.getTabBar(appId) else {
            os_log("Failed to get TabBar config for appId: %@", log: Self.log, type: .error, appId)
            return
        }

        // Store config as instance property
        self.tabBarConfig = tabBarConfig

        // Set initial selectedTabIndex based on current path
        let items = tabBarConfig.getItems(appId: appId)
        if let tabIndex = items.firstIndex(where: { $0.page_path.toString() == currentPath }) {
            selectedTabIndex = tabIndex
        } else {
            selectedTabIndex = 0
        }

        // Create SwiftUI TabBar with simple binding
        let tabBarView = NSHostingView(rootView: LxAppTabBar(
            appId: appId,
            config: tabBarConfig,
            selectedIndex: Binding(
                get: { self.selectedTabIndex },
                set: { self.selectedTabIndex = $0 }
            ),
            // Use universal tab click handler
            onTabSelected: LxAppPageNavigation.tabClickHandler(appId: appId)
        ))

        tabBarView.translatesAutoresizingMaskIntoConstraints = false

        // Store the hosting view
        self.tabBarView = tabBarView
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

        // Use shared WebView attachment logic with default full-container constraints
        WebViewManager.attachWebViewToContainer(webView, container: webViewContainer)
    }

    /// Unified method to show a WebView to the user - this is the ONLY place where onPageShow should be called
    private func showWebViewToUser(_ webView: WKWebView, path: String) {
        // Attach WebView to container (handles UI setup)
        attachWebViewToContainer(webView)

        // Hide previous WebView if different
        if let previousWebView = currentWebView, previousWebView != webView {
            previousWebView.isHidden = true
        }
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

    /// Navigate - the single, unified navigation method
    /// Core job: Update UI based on navigation type
    /// All types share common process with specific differences
    public func navigate(appId: String, to path: String, with navigationType: NavigationType) {
        guard !appId.isEmpty else { return }

        self.initialPath = path

        // Resolve actual navigation type based on logic
        let actualNavigationType = resolveNavigationType(navigationType, for: path)

        // Execute common navigation process with type-specific differences
        performCommonNavigation(to: path, with: actualNavigationType)

        // Update app state
        LxAppCore.setLastActivePath(path, for: appId)

        if let windowController = view.window?.windowController as? LxAppWindowController {
            windowController.updateWindowTitle(for: path)
        }
    }

    /// Resolve navigation type based on path and logic - using shared utility
    private func resolveNavigationType(_ navigationType: NavigationType, for path: String) -> NavigationType {
        return LxAppSharedNavigation.resolveNavigationType(navigationType, for: path, isTabPage: isTabPage)
    }

    /// Common navigation process - all types share this flow
    private func performCommonNavigation(to path: String, with navigationType: NavigationType) {
        // Find or create WebView (common for all types)
        guard let targetWebView = WebViewManager.findWebView(appId: appId, path: path) else {
            return
        }

        applyNavigationTypeSpecificUpdates(for: navigationType, path: path)
        showWebViewToUser(targetWebView, path: path)
    }

    /// Apply navigation type specific UI updates - using shared logic
    private func applyNavigationTypeSpecificUpdates(for navigationType: NavigationType, path: String) {
        // Use shared navigation logic instead of duplicated code
        LxAppSharedNavigation.applyNavigationTypeSpecificUpdates(
            navigationType: navigationType,
            path: path,
            appId: appId,
            tabBarController: self,
            uiUpdater: self
        )
    }

    public func setSelectedTabIndex(_ index: Int) {
        selectedTabIndex = index
    }

    /// Check if a path is a tab page
    private func isTabPage(_ path: String) -> Bool {
        guard let tabBarConfig = tabBarConfig else { return false }
        let items = tabBarConfig.getItems(appId: appId)
        return items.contains { $0.page_path.toString() == path }
    }

    /// Show or hide TabBar dynamically
    public func showTabBar(_ show: Bool) {
        guard let tabBar = tabBarView else { return }
        tabBar.isHidden = !show
    }

    /// Trigger TabBar UI refresh for programmatic navigation
    public func triggerTabBarRefresh() {
        // Send notification to trigger TabBar refreshTrigger.toggle()
        NotificationCenter.default.post(
            name: .tabBarStateChanged,
            object: appId
        )
    }

    //  - Helper Methods
    public func findTabIndexByPath(_ targetPath: String) -> Int? {
        guard let tabBarConfig = tabBarConfig else { return nil }

        let items = tabBarConfig.getItems(appId: appId)
        for (index, item) in items.enumerated() {
            if item.page_path.toString() == targetPath {
                return index
            }
        }
        return nil
    }

    private func getResourcesPath() -> String {
        let executablePath = Bundle.main.executablePath ?? ""
        let executableDir = (executablePath as NSString).deletingLastPathComponent
        return "\(executableDir)/Resources"
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

    // Method required by WindowController
    func updateLayoutForNavigationStyle(currentPath: String) {
        self.currentPath = currentPath
        // Update TabBar selection if needed
        if let tabBarConfig = self.tabBarConfig {
            let items = tabBarConfig.getItems(appId: appId)
            if let tabIndex = items.firstIndex(where: { $0.page_path.toString() == currentPath }) {
                selectedTabIndex = tabIndex
            }
        }
    }

    /// Update capsule button visibility
    public func updateCapsuleButtonVisibility(appId: String) {
        let isHomeApp = LxAppCore.isHomeLxApp(appId)

        if !isHomeApp {
            if findCapsuleButtonView() == nil {
                LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
            }
            findCapsuleButtonView()?.isHidden = false
        } else {
            findCapsuleButtonView()?.removeFromSuperview()
        }
    }

    /// Find capsule button view using identifier
    public func findCapsuleButtonView() -> NSView? {
        let identifier = NSUserInterfaceItemIdentifier("CapsuleButton_\(LxAppCapsuleButtons.CAPSULE_BUTTON_TAG)")
        return view.subviews.first { $0.identifier == identifier }
    }

    /// Sync TabBar with specific path for unified navigation
    public func syncTabBarWithPath(_ path: String) {
        if let tabIndex = findTabIndexByPath(path) {
            selectedTabIndex = tabIndex
            triggerTabBarRefresh()
        }
    }

    /// Update navigation bar
    public func updateNavigationBar(appId: String, path: String) {
        if let navState = LxPageNavigation.getNavigationBarState(appId: appId, path: path) {
            if let windowController = view.window?.windowController as? LxAppWindowController {
                windowController.updateNavigationBarWithState(navState)
            }
            NavigationBarStateManager.shared.updateState(appId: appId, path: path)
        }
    }
}

#endif
