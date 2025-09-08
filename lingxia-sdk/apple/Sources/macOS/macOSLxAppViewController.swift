#if os(macOS)
import Foundation
import WebKit
import os.log
import AppKit
import SwiftUI
import CLingXiaRustAPI

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
    internal var currentPath: String
    private var webViewContainer: NSView!
    private var tabBarView: NSView?
    public var tabBarConfig: TabBar?
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?

    public init(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
        super.init(nibName: nil, bundle: nil)

        // Initialize top margin based on current page
        self.currentTopMargin = calculateInitialTopMargin()
    }

    private func calculateInitialTopMargin() -> CGFloat {
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            // Get config from window controller's cache to avoid duplicate calls
            if let windowController = view.window?.windowController as? LxAppWindowController {
                return windowController.getTopMarginForCurrentPage() - LxAppWindowController.Layout.systemStatusBarHeight
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

        setupLayout()
        setupNotificationObservers()
        setupKeyboardShortcuts()
        loadWebViewContent()
    }

    // UI Setup
    private func setupLayout() {
        view.wantsLayer = true
        view.layer?.backgroundColor = AppKit.NSColor.windowBackgroundColor.cgColor

        setupTabBar()
        setupWebViewContainer()

        if let tabBar = tabBarView, let tabBarConfig = lingxia.getTabBar(appId) {
            view.addSubview(tabBar)
            setupTabBarConstraints(tabBar: tabBar, config: tabBarConfig)
        } else {
            setupWebViewConstraintsWithoutTabBar()
        }

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }

    private func setupTabBarConstraints(tabBar: NSView, config: TabBar) {
        let isTransparent = TabBar.isTransparent(config.background_color)
        let dimension = CGFloat(config.dimension)

        // TabBar constraints
        let tabBarConstraints = createTabBarConstraints(tabBar: tabBar, position: config.position, dimension: dimension)
        NSLayoutConstraint.activate(tabBarConstraints)

        // WebView constraints
        let webViewConstraints = createWebViewConstraints(tabBar: tabBar, position: config.position, isTransparent: isTransparent)
        NSLayoutConstraint.activate(webViewConstraints)

        if isTransparent {
            tabBar.wantsLayer = true
            tabBar.layer?.backgroundColor = NSColor.clear.cgColor
        }
    }

    private func createTabBarConstraints(tabBar: NSView, position: Int32, dimension: CGFloat) -> [NSLayoutConstraint] {
        switch position {
        case 0: // bottom
            return [
                tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.heightAnchor.constraint(equalToConstant: dimension)
            ]
        case 1: // left
            return [
                tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.widthAnchor.constraint(equalToConstant: dimension)
            ]
        case 2: // right
            return [
                tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.widthAnchor.constraint(equalToConstant: dimension)
            ]
        default: // fallback to bottom
            return createTabBarConstraints(tabBar: tabBar, position: 0, dimension: dimension)
        }
    }

    private func createWebViewConstraints(tabBar: NSView, position: Int32, isTransparent: Bool) -> [NSLayoutConstraint] {
        let topMargin = getTopMargin()

        if isTransparent {
            return [
                webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: topMargin),
                webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
            ]
        }

        switch position {
        case 0: // bottom
            return [
                webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: topMargin),
                webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                webViewContainer.bottomAnchor.constraint(equalTo: tabBar.topAnchor)
            ]
        case 1: // left
            return [
                webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: topMargin),
                webViewContainer.leadingAnchor.constraint(equalTo: tabBar.trailingAnchor),
                webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
            ]
        case 2: // right
            return [
                webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: topMargin),
                webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                webViewContainer.trailingAnchor.constraint(equalTo: tabBar.leadingAnchor),
                webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
            ]
        default:
            return createWebViewConstraints(tabBar: tabBar, position: 0, isTransparent: false)
        }
    }

    private func setupWebViewConstraintsWithoutTabBar() {
        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: getTopMargin()),
            webViewContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
    }

    private func setupWebViewContainer() {
        webViewContainer = NSView()
        webViewContainer.wantsLayer = true
        webViewContainer.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(webViewContainer)
    }

    private func setupTabBar(config: TabBar? = nil) {
        guard let tabBarConfig = lingxia.getTabBar(appId) else { return }

        self.tabBarConfig = tabBarConfig
        selectedTabIndex = findTabIndexByPath(currentPath) ?? 0

        let tabBarView = NSHostingView(rootView: LxAppTabBar(
            appId: appId,
            config: tabBarConfig,
            selectedIndex: Binding(
                get: { self.selectedTabIndex },
                set: { self.selectedTabIndex = $0 }
            ),
            onTabSelected: { [self] index, path in
                LxAppPageNavigation.handleTabBarItemSelected(appId: self.appId, index: index)
            }
        ))

        tabBarView.translatesAutoresizingMaskIntoConstraints = false
        self.tabBarView = tabBarView
    }

    private func loadWebViewContent() {
        if let webView = WebViewManager.findWebView(appId: appId, path: currentPath) {
            showWebViewToUser(webView, path: currentPath)
        }
    }

    /// Unified method to show a WebView to the user
    private func showWebViewToUser(_ webView: WKWebView, path: String) {
        LxAppCore.getCurrentWebView()?.removeFromSuperview()
        WebViewManager.attachWebViewToContainer(webView, container: webViewContainer)
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
        // macOS keyboard shortcuts trigger navigation back action
        let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionBack)
    }

    /// Navigate - using shared navigation logic
    @MainActor
    public func navigate(appId: String, to path: String, with navigationType: NavigationType) {
        guard !appId.isEmpty else { return }

        self.currentPath = path

        // Update UI components
        updateNavigationBar(appId: appId, path: path)

        // Show WebView
        if let webView = WebViewManager.findWebView(appId: appId, path: path) {
            showWebViewToUser(webView, path: path)
        }

        // Update app state
        LxAppCore.updateCurrentPath(path)

        // macOS-specific: Update window title
        if let windowController = view.window?.windowController as? LxAppWindowController {
            windowController.updateWindowTitle(for: path)
        }
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

    /// Sync TabBar selection with current path
    public func syncTabBarSelection(path: String) {
        if let tabIndex = findTabIndexByPath(path) {
            selectedTabIndex = tabIndex
        }
    }

    //  - Helper Methods
    public func findTabIndexByPath(_ targetPath: String) -> Int? {
        guard let tabBarConfig = tabBarConfig else { return nil }
        let items = tabBarConfig.getItems(appId: appId)
        return items.firstIndex { $0.page_path.toString() == targetPath }
    }

    // Method required by WindowController
    func updateLayoutForNavigationStyle(currentPath: String) {
        self.currentPath = currentPath
        selectedTabIndex = findTabIndexByPath(currentPath) ?? selectedTabIndex
    }

    /// Update capsule button visibility
    @MainActor
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
    @MainActor
    public func findCapsuleButtonView() -> NSView? {
        let identifier = NSUserInterfaceItemIdentifier("CapsuleButton_\(LxAppCapsuleButtons.CAPSULE_BUTTON_TAG)")
        return view.subviews.first { $0.identifier == identifier }
    }



    /// Update navigation bar
    @MainActor
    public func updateNavigationBar(appId: String, path: String) {
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)

        if let navState = LxPageNavigation.getNavigationBarState(appId: appId, path: path),
           let windowController = view.window?.windowController as? LxAppWindowController {
            windowController.updateNavigationBarWithState(navState)
        }
    }
}

#endif
