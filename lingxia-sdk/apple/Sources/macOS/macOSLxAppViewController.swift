#if os(macOS)
import Foundation
import WebKit
import os.log
import AppKit
import SwiftUI
import CLingXiaRustAPI

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
    internal var currentPath: String
    private var webViewContainer: NSView!
    internal var tabBarView: NSView?
    public var tabBarConfig: TabBar?
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var tabBarObserver: NSObjectProtocol?

    public init(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
        super.init(nibName: nil, bundle: nil)

        // Initialize top margin based on current page
        self.currentTopMargin = calculateInitialTopMargin()
    }

    private func calculateInitialTopMargin() -> CGFloat {
        // Tab style: 0pt - tab bar handles layout
        return 0
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
        tabBarObserver.map(NotificationCenter.default.removeObserver)
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
        let isTransparent = TabBarHelper.isTransparent(config.background_color)
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

        // Use native macOS wrapper so selection updates via ObservableObject are reflected in UI
        let tabBar = LingXiaTabBar()
        tabBar.initialize(config: tabBarConfig, appId: appId)
        tabBar.setOnTabSelectedListener { [weak self] index, _ in
            guard let self = self else { return }
            let _ = onUiEvent(self.appId, LxAppUIEvent.tabBarClick, String(index))
        }
        // Initialize selection based on Rust state
        let initIndex = Int(tabBarConfig.selected_index)
        tabBar.setSelectedIndex(initIndex, notifyListener: false)

        tabBar.translatesAutoresizingMaskIntoConstraints = false
        self.tabBarView = tabBar
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

        // Add TabBar state change observer
        tabBarObserver = NotificationCenter.default.addObserver(
            forName: .tabBarStateChanged,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard let self = self else { return }

            Task { @MainActor in
                // The tab bar state has changed, tell the current tab bar to refresh itself.
                if let wrapper = self.tabBarView as? LingXiaTabBar {
                    wrapper.refreshLayout()
                }
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
    public func navigate(appId: String, to path: String, with animationType: AnimationType) {
        guard !appId.isEmpty else { return }

        self.currentPath = path

        // Update UI components
        updateNavigationBar(appId: appId, path: path)

        // Show WebView
        if let webView = WebViewManager.findWebView(appId: appId, path: path) {
            showWebViewToUser(webView, path: path)
        }

        // Update app state
        LxAppCore.setCurrentPath(path)
    }

    public func setSelectedTabIndex(_ index: Int) {
        selectedTabIndex = index
    }



    // Method required by WindowController
    func updateLayoutForNavigationStyle(currentPath: String) {
        self.currentPath = currentPath

        // Tell TabBar to refresh its state from Rust - this will handle visibility and content for the new page
        if let wrapper = tabBarView as? LingXiaTabBar {
            wrapper.refreshLayout()
        }
    }

    /// Update navigation bar state
    @MainActor
    public func updateNavigationBar(appId: String, path: String) {
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
        // Tab mode: navigation bar state is managed by NavigationBarStateManager
    }
}


#endif
