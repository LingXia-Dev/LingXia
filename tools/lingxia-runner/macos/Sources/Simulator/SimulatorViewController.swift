import AppKit
import WebKit
import SwiftUI
import os.log
@_spi(Runner) import lingxia

/// View controller for Runner Simulator mode - mirrors macOSLxAppViewController functionality
@MainActor
public class SimulatorViewController: NSViewController, WKNavigationDelegate {
    
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "SimulatorViewController")
    
    // MARK: - Properties
    
    public let appId: String
    public private(set) var currentPath: String
    
    private var webViewContainer: NSView!
    internal var tabBarView: NSView?
    internal var tabBarConfig: RunnerTabBarConfig?
    internal var selectedTabIndex: Int = 0
    public var isDestroyed: Bool = false
    
    private var currentTopMargin: CGFloat = 0
    private var tabBarConstraints: [NSLayoutConstraint] = []
    private var tabBarTopConstraint: NSLayoutConstraint?
    
    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var tabBarObserver: NSObjectProtocol?
    
    // MARK: - Initialization
    
    public init(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
        super.init(nibName: nil, bundle: nil)
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    deinit {
        closeAppObserver.map(NotificationCenter.default.removeObserver)
        tabBarObserver.map(NotificationCenter.default.removeObserver)
    }
    
    // MARK: - Layout

    private var webViewTopConstraint: NSLayoutConstraint?

    internal func updateTopMargin(_ newMargin: CGFloat) {
        currentTopMargin = newMargin
        webViewTopConstraint?.constant = newMargin
        tabBarTopConstraint?.constant = newMargin
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }
    
    // MARK: - View Lifecycle
    
    public override func loadView() {
        view = NSView()
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
    }
    
    public override func viewDidLoad() {
        super.viewDidLoad()
        
        setupLayout()
        setupNotificationObservers()
        loadWebViewContent()
    }
    
    // MARK: - UI Setup
    
    private func setupLayout() {
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        
        setupWebViewContainer()
        setupWebViewConstraintsWithoutTabBar()
        updateTabBar()
        
        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }
    
    private func setupTabBarConstraints(tabBar: NSView, config: RunnerTabBarConfig) {
        let dimension = CGFloat(config.dimension)
        NSLayoutConstraint.deactivate(tabBarConstraints)
        tabBarConstraints = createTabBarConstraints(tabBar: tabBar, position: config.position, dimension: dimension)
        NSLayoutConstraint.activate(tabBarConstraints)

        if RunnerSupport.TabBar.isTransparent(config.background_color) {
            tabBar.wantsLayer = true
            tabBar.layer?.backgroundColor = NSColor.clear.cgColor
        }
    }
    
    private func createTabBarConstraints(tabBar: NSView, position: Int32, dimension: CGFloat) -> [NSLayoutConstraint] {
        switch position {
        case 0: // bottom
            tabBarTopConstraint = nil
            return [
                tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.heightAnchor.constraint(equalToConstant: dimension)
            ]
        case 1: // left
            let top = tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
            tabBarTopConstraint = top
            return [
                tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                top,
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.widthAnchor.constraint(equalToConstant: dimension)
            ]
        case 2: // right
            let top = tabBar.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
            tabBarTopConstraint = top
            return [
                tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                top,
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                tabBar.widthAnchor.constraint(equalToConstant: dimension)
            ]
        default: // fallback to bottom
            return createTabBarConstraints(tabBar: tabBar, position: 0, dimension: dimension)
        }
    }
    
    private func makeWebViewTopConstraint() -> NSLayoutConstraint {
        let c = webViewContainer.topAnchor.constraint(equalTo: view.topAnchor, constant: currentTopMargin)
        webViewTopConstraint = c
        return c
    }

    private func setupWebViewConstraintsWithoutTabBar() {
        NSLayoutConstraint.activate([
            makeWebViewTopConstraint(),
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
    
    private func createTabBar(config tabBarConfig: RunnerTabBarConfig) {
        self.tabBarConfig = tabBarConfig

        let tabBar = RunnerSupport.TabBar.makeView(config: tabBarConfig, appId: appId) { [weak self] index, _ in
            guard let self = self else { return }
            let _ = onLxappEvent(self.appId, LxAppUiEventType.TabBarClick, String(index))
        }
        tabBar.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(tabBar)
        self.tabBarView = tabBar
        setupTabBarConstraints(tabBar: tabBar, config: tabBarConfig)
    }

    private func updateTabBar() {
        guard let tabBarConfig = RunnerSupport.TabBar.config(for: appId) else {
            tabBarView?.removeFromSuperview()
            tabBarView = nil
            self.tabBarConfig = nil
            NSLayoutConstraint.deactivate(tabBarConstraints)
            tabBarConstraints = []
            tabBarTopConstraint = nil
            view.needsLayout = true
            view.layoutSubtreeIfNeeded()
            return
        }

        self.tabBarConfig = tabBarConfig

        if tabBarView == nil {
            createTabBar(config: tabBarConfig)
        } else if let tabBar = tabBarView {
            setupTabBarConstraints(tabBar: tabBar, config: tabBarConfig)
            RunnerSupport.TabBar.refresh(tabBar)
        }

        if let tabBar = tabBarView {
            view.addSubview(tabBar, positioned: .above, relativeTo: webViewContainer)
        }

        view.needsLayout = true
        view.layoutSubtreeIfNeeded()
    }
    
    private func loadWebViewContent() {
        if let webView = RunnerSupport.WebView.resolve(appId: appId, path: currentPath) {
            showWebViewToUser(webView, path: currentPath)
        }
    }
    
    /// Unified method to show a WebView to the user
    private func showWebViewToUser(_ webView: WKWebView, path: String) {
        RunnerSupport.WebView.removeCurrentFromSuperview()
        RunnerSupport.WebView.attachLxApp(webView, to: webViewContainer)
        if let tabBar = tabBarView {
            view.addSubview(tabBar, positioned: .above, relativeTo: webViewContainer)
        }
    }
    
    // MARK: - Notification Observers
    
    private func setupNotificationObservers() {
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name("com.lingxia.CLOSE_LXAPP_ACTION"), object: nil, queue: .main
        ) { [weak self] notification in
            let appId = notification.userInfo?["appId"] as? String
            Task { @MainActor in
                guard let self = self, let targetAppId = appId, targetAppId == self.appId else { return }
                self.view.window?.close()
            }
        }
        
        // Add TabBar state change observer
        tabBarObserver = NotificationCenter.default.addObserver(
            forName: RunnerSupport.TabBar.stateChangedNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard let self = self else { return }
            
            Task { @MainActor in
                self.updateTabBar()
            }
        }
    }
    
    // MARK: - Navigation
    
    @MainActor
    public func navigate(to path: String, animationType: LxAppAnimation = .none) {
        self.currentPath = path
        
        // Update UI components
        updateNavigationBar(appId: appId, path: path)
        updateTabBar()
        
        // Show WebView
        if let webView = RunnerSupport.WebView.resolve(appId: appId, path: path) {
            showWebViewToUser(webView, path: path)
        }
        
        // Update app state
        RunnerSupport.Runtime.setCurrentPath(path)
    }
    
    public func setSelectedTabIndex(_ index: Int) {
        selectedTabIndex = index
    }
    
    // Method required by WindowController
    func updateLayoutForNavigationStyle(currentPath: String) {
        self.currentPath = currentPath
        
        // Tell TabBar to refresh its state from Rust - this will handle visibility and content for the new page
        updateTabBar()
    }
    
    /// Update navigation bar state
    @MainActor
    public func updateNavigationBar(appId: String, path: String) {
        RunnerSupport.Navigation.updateState(appId: appId, path: path)
        // Simulator mode: notify window controller to update its custom navigation bar
        NotificationCenter.default.post(
            name: .capsuleNavigationBarStateChanged,
            object: nil,
            userInfo: ["appId": appId, "path": path]
        )
    }
}
