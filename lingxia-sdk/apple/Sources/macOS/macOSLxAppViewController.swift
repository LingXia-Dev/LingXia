#if os(macOS)
import Foundation
import WebKit
import os.log
import AppKit
import SwiftUI
import CLingXiaFFI

private let lxAppViewControllerLog = OSLog(subsystem: "LingXia", category: "LxAppView")

@MainActor
public class macOSLxAppViewController: NSViewController, WKNavigationDelegate {
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
    private var currentWebView: WKWebView?
    public var tabBarConfig: TabBar?
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

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
        if let tabBar = tabBarView, let tabBarConfig = getTabBar(appId) {
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
        guard let tabBarConfig = getTabBar(appId) else {
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
            onTabSelected: { index, path in
                self.selectedTabIndex = index
                self.switchPage(targetPath: path)
            }
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

        webView.translatesAutoresizingMaskIntoConstraints = false
        webViewContainer.addSubview(webView)

        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: webViewContainer.topAnchor),
            webView.leadingAnchor.constraint(equalTo: webViewContainer.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: webViewContainer.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: webViewContainer.bottomAnchor)
        ])

        // Force layout update of the entire view subtree to ensure all containers
        // have their final size before we make the webView visible.
        // This prevents visual glitches without needing to manually set the frame.
        view.layoutSubtreeIfNeeded()

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

        selectedTabIndex = tabIndex
        showWebViewToUser(targetWebView, path: targetPath)
    }

    private func navigateToPage(targetPath: String) {
        // Find WebView for the target page
        guard let newWebView = WebViewManager.findWebView(appId: appId, path: targetPath) else {
            return
        }

        showWebViewToUser(newWebView, path: targetPath)
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
}

#endif
