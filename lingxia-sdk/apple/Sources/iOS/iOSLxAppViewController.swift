#if os(iOS)
import UIKit
import SwiftUI
import WebKit
import os.log
import Combine
import CLingXiaFFI
@preconcurrency import ObjectiveC

// Log instance outside of @MainActor to avoid isolation issues
private let miniAppViewControllerLog = OSLog(subsystem: "LingXia", category: "iOSLxAppView")

/// SwiftUI-based LxApp view controller with modern reactive architecture
public class iOSLxAppViewController: UIViewController, ObservableObject {
    private static let log = miniAppViewControllerLog

    public static let EXTRA_APP_ID = "appId"
    public static let EXTRA_PATH = "path"
    // NavigationBar title and capsule button vertical position (status bar + margin)
    internal static let NAV_TITLE_VERTICAL_POSITION: CGFloat = 44 + 8 // Default fallback

    // Published Properties for SwiftUI Integration
    public let appId: String  // Non-@Published to avoid MainActor issues
    @Published public var currentPath: String
    public var isDestroyed = false
    public var isDisplayingHomeLxApp: Bool = false
    public var hasInitializedTransparency: Bool = false
    public var isWebViewLoading: Bool = false
    @Published public var navigationTitle: String = ""
    @Published public var showBackButton: Bool = false

    // Private Properties
    private var initialPath: String
    internal var rootContainer: UIView!
    private var webViewContainer: UIView!
    internal var tabBar: LingXiaTabBar?
    internal var navigationBar: LingXiaNavigationBar?
    private var pendingWebViewSetup = false
    private var cancellables = Set<AnyCancellable>()

    internal var currentWebView: WKWebView?

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

    /// Computed navigation area height based on global state
    private var navigationAreaHeight: CGFloat {
        guard let state = NavigationBarStateManager.shared.currentState, state.show_navbar else {
            return statusBarHeight
        }
        return statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
    }

    /// Get actual status bar height - single source of truth
    private var statusBarHeight: CGFloat {
        return view.window?.windowScene?.statusBarManager?.statusBarFrame.height ?? LxAppTheme.Metrics.statusBarHeight
    }

    /// Check if current page should use transparent mode
    private var shouldUseTransparentMode: Bool {
        // For now, just check if TabBar is transparent
        let isTabBarTransparent = tabBar != nil && TabBar.isTransparent(tabBar!.config?.background_color ?? 0)
        return isTabBarTransparent
    }

    public init(appId: String, path: String) {
        self.appId = appId
        self.initialPath = path
        self.currentPath = path
        self.isDisplayingHomeLxApp = LxAppCore.isHomeLxApp(appId)
        super.init(nibName: nil, bundle: nil)

        setupReactiveBindings()
        setupNotificationObservers()
        iOSLxAppViewController.configureTransparentSystemBars(viewController: self)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupReactiveBindings() {
        // Path changes automatically update global navigation state
        $currentPath
            .sink { [weak self] path in
                guard let self = self else { return }
                NavigationBarStateManager.shared.updateState(appId: self.appId, path: path)
            }
            .store(in: &cancellables)

        // Global state changes automatically update WebView layout
        NavigationBarStateManager.shared.$currentState
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in
                self?.updateWebViewConstraints()
            }
            .store(in: &cancellables)

        // Global state changes automatically apply transparency
        NavigationBarStateManager.shared.$currentState
            .receive(on: DispatchQueue.main)
            .sink { [weak self] state in
                if state?.show_navbar != true {
                    self?.applyTransparentBackground(forceLayout: false)
                }
            }
            .store(in: &cancellables)
    }

    private func getNavigationTitle(for path: String) -> String {
        if let config = LxPageNavigation.getNavigationBarState(appId: appId, path: path) {
            return config.title_text.toString()
        }
        return ""
    }

    /// Configure WebView appearance and behavior
    private func configureWebView(_ webView: WKWebView, transparent: Bool) {
        let backgroundColor = transparent ? UIColor.clear : UIColor.white
        let isOpaque = !transparent

        // Configure WebView
        webView.backgroundColor = backgroundColor
        webView.isOpaque = isOpaque
        webView.layer.backgroundColor = backgroundColor.cgColor

        // Configure ScrollView
        webView.scrollView.backgroundColor = backgroundColor
        webView.scrollView.isOpaque = isOpaque
        webView.scrollView.layer.backgroundColor = backgroundColor.cgColor
        webView.scrollView.layer.isOpaque = isOpaque

        // Configure scroll behavior
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.indicatorStyle = .default
        webView.scrollView.showsVerticalScrollIndicator = true
        webView.scrollView.showsHorizontalScrollIndicator = true
    }

    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if let navController = viewController.navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    private func configureEdgeToEdgeDisplay() {
        modalPresentationStyle = .fullScreen
        edgesForExtendedLayout = [.top, .bottom, .left, .right]
        extendedLayoutIncludesOpaqueBars = true
        additionalSafeAreaInsets = .zero
    }

    // Override to prevent black background in safe area
    public override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
        return [.bottom]
    }

    public override var prefersHomeIndicatorAutoHidden: Bool {
        return false
    }

    public override var childForHomeIndicatorAutoHidden: UIViewController? {
        return nil
    }

    public static func updateNavigationBarTransparency(viewController: UIViewController, isTabBarTransparent: Bool, tabBarBackgroundColor: UInt32? = nil) {
        if let navController = viewController.navigationController {
            if isTabBarTransparent {
                navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
                navController.navigationBar.shadowImage = UIImage()
            } else {
                let navBarColor = tabBarBackgroundColor != nil ? PlatformColor(argb: tabBarBackgroundColor!) : UIColor.white
                navController.navigationBar.backgroundColor = navBarColor
            }
        }
    }

    public override func viewDidLoad() {
        super.viewDidLoad()

        configureEdgeToEdgeDisplay()
        setupUI()
        setupWebView()
    }

    private func setupUI() {
        if let navController = navigationController {
            navController.setNavigationBarHidden(true, animated: false)
        }

        // Set initial transparent background to prevent black flash during startup
        view.backgroundColor = UIColor.clear
        view.isOpaque = false

        // Mark transparency as initialized early to prevent redundant operations
        hasInitializedTransparency = true

        setupRootContainer()
        setupWebViewContainer()
        setupTabBarIfNeeded()
        setupNavigationBar()
        setupNotificationObservers()

        // Initialize NavBar state
        NavigationBarStateManager.shared.updateState(appId: appId, path: currentPath)

        // Apply transparency based on TabBar configuration, not just NavBar state
        if let tabBar = tabBar, TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
            setCompleteTransparency()
        } else if NavigationBarStateManager.shared.currentState?.show_navbar != true {
            applyTransparentBackground(forceLayout: false)
        }
    }

    private func setupWebView() {
        if currentWebView == nil {
            setupInitialContent(path: currentPath)
        } else {
            attachWebViewToUI(webView: currentWebView!)
            updateNavigationBar(appId: appId, path: currentPath)
        }
    }

    private func setupTabBarIfNeeded() {
        let tabBarConfig = lingxia.getTabBar(self.appId)

        if let tabBarConfig = tabBarConfig {
            setupTabBar(config: tabBarConfig)

            // Sync TabBar selected state with current path
            if let tabBar = tabBar {
                tabBar.syncSelectedTabWithCurrentPath(currentPath)
            }
        }
    }

    private func setupNavigationBar() {
        updateNavigationBar(appId: appId, path: currentPath)
    }

    private func setupNotificationObservers() {
        // Setup notification observers for reactive updates
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_CLOSE_LXAPP),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let appId = userInfo["appId"] as? String,
                  appId == self.appId else { return }

            DispatchQueue.main.async {
                self.performLxAppClose()
            }
        }

        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let appId = userInfo["appId"] as? String,
                  let path = userInfo["path"] as? String,
                  appId == self.appId else { return }

            DispatchQueue.main.async {
                self.currentPath = path
                self.switchPage(targetPath: path)
            }
        }
    }

    public override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)

        forceTransparentBackground()

        if let currentWebView = currentWebView {
            forceWebViewTransparency(webView: currentWebView)
        }

        if shouldUseTransparentMode {
            setCompleteTransparency()

            view.backgroundColor = UIColor.clear
            view.isOpaque = false
            view.layer.backgroundColor = UIColor.clear.cgColor

            if let window = view.window {
                window.backgroundColor = UIColor.clear
                window.isOpaque = false

                if let rootVC = window.rootViewController {
                    rootVC.view.backgroundColor = UIColor.clear
                    rootVC.view.isOpaque = false
                    rootVC.view.layer.backgroundColor = UIColor.clear.cgColor
                }

                if let windowScene = window.windowScene {
                    for sceneWindow in windowScene.windows {
                        sceneWindow.backgroundColor = UIColor.clear
                        sceneWindow.isOpaque = false
                        if let sceneRootVC = sceneWindow.rootViewController {
                            sceneRootVC.view.backgroundColor = UIColor.clear
                            sceneRootVC.view.isOpaque = false
                            sceneRootVC.view.layer.backgroundColor = UIColor.clear.cgColor
                        }
                    }
                }
            }

            setNeedsStatusBarAppearanceUpdate()
        }

        // Apply transparency effects for TabBar if needed
        if let tabBar = tabBar, TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
            // Use immediate application without delays to avoid startup flicker
            tabBar.forceTransparencyMode()
        }

        // Create capsule button after view appears to minimize startup operations
        addCapsuleButton()
    }

    public override var preferredStatusBarStyle: UIStatusBarStyle {
        if shouldUseTransparentMode {
            return .darkContent
        } else {
            return .default
        }
    }

    public override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()

        // Only force transparent background once, not on every layout
        if view.backgroundColor != UIColor.clear && !hasInitializedTransparency {
            forceTransparentBackground()
        }
    }

    private func forceTransparentBackground() {
        // Only apply transparency once during app lifecycle to minimize startup flicker
        if hasInitializedTransparency {
            return
        }

        // Use the comprehensive transparency method
        setCompleteTransparency()
        hasInitializedTransparency = true
    }

    private func setupRootContainer() {
        rootContainer = UIView()
        rootContainer.backgroundColor = UIColor.clear
        rootContainer.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(rootContainer)

        NSLayoutConstraint.activate([
            rootContainer.topAnchor.constraint(equalTo: view.topAnchor),
            rootContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            rootContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            rootContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
    }

    private func setupWebViewContainer() {
        webViewContainer = UIView()
        webViewContainer.backgroundColor = UIColor.clear
        webViewContainer.isOpaque = false
        webViewContainer.translatesAutoresizingMaskIntoConstraints = false
        rootContainer.addSubview(webViewContainer)

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            webViewContainer.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
        ])
    }

    private func setupInitialContent(path: String) {
        LxAppCore.setLastActivePath(path, for: appId)
        setupWebViewIfReady(appId: appId, path: path)
    }

    public func setupWebViewIfReady(appId: String, path: String) {
        if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
            // Pause current WebView if it exists and is different from target
            if let currentWebView = currentWebView, currentWebView != webView {
                currentWebView.pauseWebView()
                currentWebView.isHidden = true

            }

            attachWebViewToUI(webView: webView)
            updateNavigationBar(appId: appId, path: path)

            // Force onPageShow to ensure WebView loads content
            lingxia.onPageShow(appId, path)
        } else {
            // Retry once after a short delay
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
                if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                    // Pause current WebView if it exists and is different from target
                    if let currentWebView = self?.currentWebView, currentWebView != webView {
                        currentWebView.pauseWebView()
                        currentWebView.isHidden = true
                    }

                    self?.attachWebViewToUI(webView: webView)
                    self?.updateNavigationBar(appId: appId, path: path)

                    // Force onPageShow to ensure WebView loads content
                    lingxia.onPageShow(appId, path)
                }
            }
        }
    }

    /// Setup WebView for the specified app and path
    public func setupWebView(appId: String, path: String) {
        if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
            // Pause current WebView if it exists and is different from target
            if let currentWebView = currentWebView, currentWebView != webView {
                currentWebView.pauseWebView()
                currentWebView.isHidden = true

            }

            attachWebViewToUI(webView: webView)

            // Force onPageShow to ensure WebView loads content
            lingxia.onPageShow(appId, path)
        } else {
            // Retry once after a short delay
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
                if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                    // Pause current WebView if it exists and is different from target
                    if let currentWebView = self?.currentWebView, currentWebView != webView {
                        currentWebView.pauseWebView()
                        currentWebView.isHidden = true
                    }

                    self?.attachWebViewToUI(webView: webView)

                    // Force onPageShow to ensure WebView loads content
                    lingxia.onPageShow(appId, path)
                }
            }
        }
    }

    /// Apply transparency effects to the view
    public func applyTransparencyEffects() {
        // For TabBar root to root switches, minimize operations to avoid any flicker
        guard let tabBar = tabBar else { return }

        // Only apply transparency if TabBar is actually transparent
        if TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
            // Use immediate transparency application without any delays
            tabBar.forceTransparencyMode()

            // Apply WebView transparency if current WebView exists
            if let currentWebView = currentWebView {
                forceWebViewTransparency(webView: currentWebView)
            }
        }

        // Minimal UI layering - only bring TabBar to front for root-to-root switches
        rootContainer.bringSubviewToFront(tabBar)
    }

    private func attachWebViewToUI(webView: WKWebView) {
        currentWebView = webView
        addWebViewToContainer(webView)

        // Resume WebView operations
        webView.resumeWebView()
        webView.isHidden = false

        // Remove any existing loading indicator immediately
        if let loadingIndicator = webViewContainer.viewWithTag(9997) {
            loadingIndicator.removeFromSuperview()
        }

        // Ensure UI elements are on top
        bringUIElementsToFront()
    }

    private var webViewTopConstraint: NSLayoutConstraint?

    private func addWebViewToContainer(_ webView: WKWebView) {
        if webView.superview == rootContainer {
            webView.isHidden = false
            updateWebViewConstraints()
            return
        }

        if webView.superview != nil {
            webView.removeFromSuperview()
        }

        // CRITICAL: Hide WebView first to prevent visual glitches during constraint setup
        webView.isHidden = true

        rootContainer.addSubview(webView)
        webView.translatesAutoresizingMaskIntoConstraints = false

        updateWebViewTopConstraint(for: webView)

        NSLayoutConstraint.activate([
            webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        configureWebView(webView, transparent: shouldUseTransparentMode)
        webView.scrollView.contentInset = UIEdgeInsets.zero
        webView.scrollView.scrollIndicatorInsets = UIEdgeInsets.zero

        // Force layout before showing WebView to prevent position jumping
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()

        // Now show WebView with correct position
        webView.isHidden = false
        applyTransparencyIfNeeded(for: webView)
    }

    /// Position WebView below the navigation area (status bar + navbar)
    private func updateWebViewTopConstraint(for webView: WKWebView) {
        // Remove old constraint
        if let oldConstraint = webViewTopConstraint {
            oldConstraint.isActive = false
            rootContainer.removeConstraint(oldConstraint)
        }

        // Use clean navigation area height calculation
        let topOffset = navigationAreaHeight

        webViewTopConstraint = webView.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: topOffset)
        webViewTopConstraint?.isActive = true

        // Force layout update
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()
    }

    /// Update WebView layout to account for navigation area changes
    private func updateWebViewConstraints() {
        guard let currentWebView = currentWebView else { return }
        updateWebViewTopConstraint(for: currentWebView)
    }

    private func setupTabBar(config: TabBar?) {
        guard let config = config else {
            os_log("Invalid or insufficient TabBar config", log: Self.log, type: .error)
            return
        }

        let isTabBarTransparent = TabBar.isTransparent(config.background_color)

        // Update system navigation bar transparency based on TabBar transparency and color
        iOSLxAppViewController.updateNavigationBarTransparency(
            viewController: self,
            isTabBarTransparent: isTabBarTransparent,
            tabBarBackgroundColor: config.background_color
        )

        if tabBar == nil {
            tabBar = LingXiaTabBar()
            tabBar?.setConfig(config: config, appId: self.appId)

            tabBar?.setOnTabSelectedListener { [weak self] index, path in
                self?.switchToTab(targetPath: path)
            }

            if let tabBar = tabBar {
                rootContainer.addSubview(tabBar)
                applyTabBarLayoutParams(tabBar: tabBar, config: config)
            }
        } else {
            tabBar?.setConfig(config: config, appId: self.appId)
            if let tabBar = tabBar {
                // TabBar is already added to view hierarchy, just update layout
                applyTabBarLayoutParams(tabBar: tabBar, config: config)
            }
        }
    }

    private func configureTabBarTransparencyMode(_ isTransparent: Bool) {
        // Configure background colors based on transparency mode
        configureBackgroundColors(isTransparent)

        // Configure TabBar overlay positioning
        configureTabBarOverlay(isTransparent)

        // Force immediate layout update
        view.setNeedsLayout()
        view.layoutIfNeeded()

        // For transparent mode, ensure TabBar transparency is set
        if isTransparent {
            enforceTabBarTransparency()
        }
    }

    private func configureBackgroundColors(_ isTransparent: Bool) {
        if isTransparent {
            // Apply complete transparency for transparent mode
            applyTransparencyIfNeeded()
        } else {
            // Apply appropriate opaque backgrounds for non-transparent mode
            setOpaqueBackgrounds()
        }
    }

    private func setOpaqueBackgrounds() {
        // Set main view controller view to white
        view.backgroundColor = UIColor.white
        view.isOpaque = true
        view.layer.backgroundColor = UIColor.white.cgColor

        // Set root container to white
        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.white
            rootContainer.isOpaque = true
            rootContainer.layer.backgroundColor = UIColor.white.cgColor
        }
    }

    private func configureTabBarOverlay(_ isTransparent: Bool) {
        guard let tabBar = tabBar else { return }

        if isTransparent {
            // Transparent TabBar: overlay mode with high z-position
            tabBar.layer.zPosition = 1000
            tabBar.backgroundColor = UIColor.clear
        } else {
            // Opaque TabBar: normal positioning
            tabBar.layer.zPosition = 0
        }
    }

    private func calculateTopAnchor() -> (NSLayoutYAxisAnchor, CGFloat) {
        let hasNavigationBar = navigationBar != nil

        if hasNavigationBar {
            return (navigationBar!.bottomAnchor, 0)
        } else {
            return (rootContainer.topAnchor, 0)
        }
    }

    private func calculateBottomAnchor(isTransparent: Bool) -> NSLayoutYAxisAnchor {
        let isBottomTabBar = tabBar?.config?.position == 0 // 0 = bottom

        if isBottomTabBar {
            if isTransparent {
                // Transparent TabBar: content extends to actual screen bottom
                return view.bottomAnchor
            } else {
                // Opaque TabBar: content stops at TabBar top
                return tabBar?.topAnchor ?? rootContainer.bottomAnchor
            }
        } else {
            return rootContainer.bottomAnchor
        }
    }

    private func enforceTabBarTransparency() {
        guard let tabBar = tabBar else { return }
        tabBar.forceTransparencyMode()
    }

    /// Applies transparency for transparent TabBar scenarios
    /// Combines setCompleteTransparency and forceWebViewTransparency when needed
    private func applyTransparencyIfNeeded(for webView: WKWebView? = nil) {
        guard let tabBar = tabBar, TabBar.isTransparent(tabBar.config?.background_color ?? 0) else {
            return
        }

        // Apply complete transparency to all UI elements
        setCompleteTransparency()

        // Re-apply navbar configuration after transparency changes
        let currentPath = webView?.currentPath ?? initialPath
        NavigationBarStateManager.shared.updateState(appId: appId, path: currentPath)

        // Apply specific WebView transparency if provided
        if let webView = webView {
            forceWebViewTransparency(webView: webView)
        }
    }

    private func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBar) {
        let isVertical = config.position == 1 || config.position == 2 // 1=left, 2=right
        let tabBarSize = CGFloat(config.dimension) // Use configured dimension instead of default

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        if isVertical {
            NSLayoutConstraint.activate([
                tabBar.widthAnchor.constraint(equalToConstant: tabBarSize),
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: statusBarHeight),
                tabBar.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
            ])

            if config.position == 1 { // left
                tabBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor).isActive = true
            } else {
                tabBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor).isActive = true
            }
        } else {
            NSLayoutConstraint.activate([
                tabBar.heightAnchor.constraint(equalToConstant: tabBarSize),
                tabBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
                tabBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor)
            ])

            // For bottom position, always extend to view.bottomAnchor to cover safe area
            // Both transparent and opaque TabBars extend to actual screen bottom
            // The difference is handled internally by the TabBar component
            tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor).isActive = true
        }
    }

    private func setCompleteTransparency() {
        // Force main view controller view
        view.backgroundColor = UIColor.clear
        view.isOpaque = false
        view.layer.backgroundColor = UIColor.clear.cgColor

        // Force root container
        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.clear
            rootContainer.isOpaque = false
            rootContainer.layer.backgroundColor = UIColor.clear.cgColor
        }

        // Force webViewContainer transparency
        if let webViewContainer = webViewContainer {
            webViewContainer.backgroundColor = UIColor.clear
            webViewContainer.isOpaque = false
            webViewContainer.layer.backgroundColor = UIColor.clear.cgColor
        }

        // Force window transparency
        if let window = view.window {
            window.backgroundColor = UIColor.clear
            window.isOpaque = false
        }

        // Force all windows in scene
        if let windowScene = view.window?.windowScene {
            for window in windowScene.windows {
                window.backgroundColor = UIColor.clear
                window.isOpaque = false
            }
        }

        // Force navigation controller transparency
        if let navController = navigationController {
            navController.view.backgroundColor = UIColor.clear
            navController.view.isOpaque = false
        }

        // Force parent view controller transparency
        if let parentVC = parent {
            parentVC.view.backgroundColor = UIColor.clear
            parentVC.view.isOpaque = false
        }

        // Force TabBar transparency only if it's configured to be transparent
        if let tabBar = tabBar {
            if TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
                tabBar.forceTransparencyMode()
            } else {
                // If TabBar is not transparent, restore its original background color
                let resolvedColor = TabBarHelper.resolvedBackgroundColor(tabBar.config?.background_color ?? 0, isVertical: false)
                tabBar.backgroundColor = resolvedColor
                tabBar.layer.backgroundColor = resolvedColor.cgColor
                tabBar.isOpaque = true
                tabBar.layer.isOpaque = true
            }
        }

        // Ensure NavigationBar remains visible (don't make it transparent)
        if let navigationBar = navigationBar {
            // Keep NavigationBar opaque but let SwiftUI handle the background color
            navigationBar.backgroundColor = UIColor.clear
            navigationBar.isOpaque = false
            navigationBar.layer.backgroundColor = UIColor.clear.cgColor

        }
    }

    /// Forces transparency on a specific WebView after it's added to view hierarchy
    private func forceWebViewTransparency(webView: WKWebView) {
        configureWebView(webView, transparent: true)
    }

    public func performLxAppClose() {
        let _ = onLxappClosed(appId)

        guard let navController = navigationController else {
            dismiss(animated: false)
            return
        }

        if navController.viewControllers.count <= 1 {
            navController.presentingViewController?.dismiss(animated: false)
        } else {
            let previousVC = navController.viewControllers[navController.viewControllers.count - 2] as? iOSLxAppViewController
            navController.popViewController(animated: false)
            if let previousVC = previousVC {
                NavigationBarStateManager.shared.updateState(appId: previousVC.appId, path: previousVC.currentPath)
                // Force NavBar to update with correct state
                previousVC.navigationBar?.updateWithState(NavigationBarStateManager.shared.currentState)
            }
        }
    }

    private func bringUIElementsToFront() {
        // Bring NavigationBar to front first
        if let navigationBar = navigationBar {
            rootContainer.bringSubviewToFront(navigationBar)
            os_log("bringUIElementsToFront: NavigationBar brought to front", log: Self.log, type: .info)
        }

        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)

            // Re-apply transparency immediately after bringSubviewToFront
            if TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
                tabBar.forceTransparencyMode()
            }
        }
    }

    private func addCapsuleButton() {
        if isDisplayingHomeLxApp { return }
        LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
    }

    private func applyTransparentBackground(forceLayout: Bool = true) {
        view.backgroundColor = UIColor.clear
        view.isOpaque = false
        view.layer.backgroundColor = UIColor.clear.cgColor

        rootContainer.backgroundColor = UIColor.clear
        rootContainer.isOpaque = false
        rootContainer.layer.backgroundColor = UIColor.clear.cgColor

        webViewContainer?.backgroundColor = UIColor.clear
        webViewContainer?.isOpaque = false

        if forceLayout {
            view.setNeedsLayout()
            view.layoutIfNeeded()
        }
    }

    /// Hide navigation bar by updating global state
    public func hideNavigationBar() {
        NavigationBarStateManager.shared.currentState = nil
    }

    /// Create navigation bar if needed
    public func createNavigationBarIfNeeded() {
        guard navigationBar == nil else { return }

        let navBar = LingXiaNavigationBar()
        navBar.translatesAutoresizingMaskIntoConstraints = false

        navigationBar = navBar
        rootContainer.addSubview(navBar)

        // Store height constraint for dynamic updates
        let heightConstraint = navBar.heightAnchor.constraint(equalToConstant: statusBarHeight + NavigationBarState.DEFAULT_HEIGHT)

        NSLayoutConstraint.activate([
            navBar.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            navBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            navBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            heightConstraint
        ])

        // Store reference for dynamic updates
        navigationBar?.heightConstraint = heightConstraint

        rootContainer.bringSubviewToFront(navBar)
        updateWebViewConstraints()
    }
}
#endif
