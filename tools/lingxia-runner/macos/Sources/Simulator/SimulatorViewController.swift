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
        // Clear, not windowBackgroundColor: this host fills the square phone area
        // behind the rounded device shape, so an opaque bg leaks (cream in Light
        // Mode) outside the rounded corners. The lxapp paints its own background.
        view.layer?.backgroundColor = NSColor.clear.cgColor
    }
    
    public override func viewDidLoad() {
        super.viewDidLoad()

        setupLayout()
        setupNotificationObservers()
        loadWebViewContent()
    }

    public override func viewDidLayout() {
        super.viewDidLayout()
        reportSurfaceWidth()
    }

    /// Report the device-screen width so the core derives the sizeClass: the
    /// iPhone shape resolves to compact, which makes `as: 'window'` unsupported and
    /// degrades URL asides to the in-app browser — exactly like a real phone.
    private func reportSurfaceWidth() {
        guard !appId.isEmpty else { return }
        let width = view.bounds.width
        guard width > 0 else { return }
        _ = setSurfaceWidth(appId, Double(width))
    }

    // MARK: - UI Setup
    
    private func setupLayout() {
        view.wantsLayer = true
        // Clear, not windowBackgroundColor: this host fills the square phone area
        // behind the rounded device shape, so an opaque bg leaks (cream in Light
        // Mode) outside the rounded corners. The lxapp paints its own background.
        view.layer?.backgroundColor = NSColor.clear.cgColor
        
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
    private func showWebViewToUser(
        _ webView: WKWebView,
        path: String,
        animation: LxAppAnimation = .none
    ) {
        if animation != .none {
            applyNavigationTransition(animation)
        }
        RunnerSupport.WebView.removeCurrentFromSuperview()
        RunnerSupport.WebView.attachLxApp(webView, to: webViewContainer)
        if let tabBar = tabBarView {
            view.addSubview(tabBar, positioned: .above, relativeTo: webViewContainer)
        }
    }

    /// Slide the container's contents in the navigation direction — mirrors the
    /// iOS/Android 300ms page transition. A layer `CATransition` animates the
    /// webview swap; the phone frame's container already clips to bounds.
    private func applyNavigationTransition(_ animation: LxAppAnimation) {
        webViewContainer.layer?.masksToBounds = true
        guard let layer = webViewContainer.layer else { return }
        let transition = CATransition()
        transition.duration = 0.3
        transition.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        switch animation {
        case .push:
            transition.type = .push
            transition.subtype = .fromRight
        case .pop:
            transition.type = .push
            transition.subtype = .fromLeft
        case .fade:
            transition.type = .fade
        case .none:
            return
        }
        layer.add(transition, forKey: "lxNavTransition")
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
            showWebViewToUser(webView, path: path, animation: animationType)
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
