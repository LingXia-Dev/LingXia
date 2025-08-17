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
    internal static let NAV_TITLE_VERTICAL_POSITION: CGFloat = PLATFORM_STATUS_BAR_HEIGHT + 8


    // MARK: - Published Properties for SwiftUI Integration
    public let appId: String  // Non-@Published to avoid MainActor issues
    @Published public var currentPath: String
    public var isDestroyed = false
    public var isDisplayingHomeLxApp: Bool = false
    public var hasInitializedTransparency: Bool = false
    public var isWebViewLoading: Bool = false
    @Published public var navigationTitle: String = ""
    @Published public var showBackButton: Bool = false

    // MARK: - Private Properties
    private var initialPath: String
    private var rootContainer: UIView!
    private var statusBarBackground: UIView!
    private var webViewContainer: UIView!
    internal var tabBar: LingXiaTabBar?
    internal var navigationBar: LingXiaNavigationBar?
    private var pendingWebViewSetup = false
    private var cancellables = Set<AnyCancellable>()

    internal var currentWebView: WKWebView?

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

    /// Check if current page should use transparent mode
    private var shouldUseTransparentMode: Bool {
        let isInitialRoute = LxPageNavigation.isInitialRoute(appId: appId, path: currentPath)
        let isTabBarTransparent = tabBar != nil && TabBarConfig.isTransparent(tabBar!.config?.background_color ?? 0)
        return isInitialRoute || isTabBarTransparent
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

    // MARK: - Reactive Setup

    private func setupReactiveBindings() {
        // Update navigation title when path changes
        $currentPath
            .map { [weak self] path in
                self?.getNavigationTitle(for: path) ?? ""
            }
            .assign(to: \.navigationTitle, on: self)
            .store(in: &cancellables)

        // Update back button visibility when path changes
        $currentPath
            .map { [weak self] path in
                guard let self = self else { return false }
                return LxPageNavigation.shouldShowBackButton(for: path, appId: self.appId, tabBarConfig: self.tabBar?.config)
            }
            .assign(to: \.showBackButton, on: self)
            .store(in: &cancellables)
    }

    private func getNavigationTitle(for path: String) -> String {
        if let config = LxPageNavigation.getNavigationBarConfig(appId: appId, path: path) {
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

        navigationBar = nil

        setupRootContainer()
        setupWebViewContainer()
        setupTabBarIfNeeded()
        setupNavigationBar()

        // Ensure transparent navigation area after containers are set up
        ensureTransparentNavigationArea(forceLayout: false)
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
        let tabBarConfig = getTabBarConfig(self.appId)

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
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config?.background_color ?? 0) {
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

    /// Setup WebView without NavigationBar update (for TabBar switches where NavBar is already handled)
    public func setupWebViewWithoutNavBarUpdate(appId: String, path: String) {
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

    /// Apply transparency effects specifically after TabBar switches
    public func applyTransparencyEffectsAfterTabSwitch() {
        // For TabBar root to root switches, minimize operations to avoid any flicker
        guard let tabBar = tabBar else { return }

        // Only apply transparency if TabBar is actually transparent
        if TabBarConfig.isTransparent(tabBar.config?.background_color ?? 0) {
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

    private func addWebViewToContainer(_ webView: WKWebView) {
        // Skip complex constraint updates for TabBar switches to avoid screen flicker
        if webView.superview == rootContainer {
            // WebView already in container, just ensure it's visible
            webView.isHidden = false
            return
        }

        if webView.superview != nil {
            webView.removeFromSuperview()
        }

        rootContainer.addSubview(webView)

        webView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: view.topAnchor),
            webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        configureWebView(webView, transparent: shouldUseTransparentMode)

        // Set content inset based on UI layout
        let topInset: CGFloat = (shouldUseTransparentMode && navigationBar == nil) ? 0 :
                               (navigationBar != nil) ? 0 : PLATFORM_STATUS_BAR_HEIGHT

        webView.scrollView.contentInset = UIEdgeInsets(top: topInset, left: 0, bottom: 0, right: 0)
        webView.scrollView.scrollIndicatorInsets = webView.scrollView.contentInset

        webView.isHidden = false

        // Apply transparency effects after WebView is positioned
        applyTransparencyIfNeeded(for: webView)
    }

    private func setupTabBar(config: TabBarConfig?) {
        guard let config = config else {
            os_log("Invalid or insufficient TabBar config", log: Self.log, type: .error)
            return
        }

        let isTabBarTransparent = TabBarConfig.isTransparent(config.background_color)

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
        guard let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config?.background_color ?? 0) else {
            return
        }

        // Apply complete transparency to all UI elements
        setCompleteTransparency()

        // Re-apply navbar configuration after transparency changes
        // This ensures page-specific navbar colors are preserved
        let currentPath = webView?.currentPath ?? initialPath
        updateNavigationBar(appId: appId, path: currentPath)

        // Apply specific WebView transparency if provided
        if let webView = webView {
            forceWebViewTransparency(webView: webView)
        }
    }

    private func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBarConfig) {
        let isVertical = config.position == 1 || config.position == 2 // 1=left, 2=right
        let tabBarSize = CGFloat(config.dimension) // Use configured dimension instead of default

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        if isVertical {
            NSLayoutConstraint.activate([
                tabBar.widthAnchor.constraint(equalToConstant: tabBarSize),
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: PLATFORM_STATUS_BAR_HEIGHT),
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
            if TabBarConfig.isTransparent(tabBar.config?.background_color ?? 0) {
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
            // Keep NavigationBar opaque with white background
            navigationBar.backgroundColor = UIColor.white
            navigationBar.isOpaque = true
            navigationBar.layer.backgroundColor = UIColor.white.cgColor

        }
    }

    /// Forces transparency on a specific WebView after it's added to view hierarchy
    private func forceWebViewTransparency(webView: WKWebView) {
        configureWebView(webView, transparent: true)
    }

    public func performLxAppClose() {
        // Notify Rust layer that lxapp is being closed
        let _ = onLxappClosed(appId)
        os_log("performLxAppClose: onLxappClosed called for appId=%@", log: Self.log, type: .info, appId)

        // Use the same approach as independent iOS implementation
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("performLxAppClose: Could not find window scene or window", log: Self.log, type: .error)
            return
        }

        if let navController = window.rootViewController as? UINavigationController {
            navController.popViewController(animated: false)
            os_log("performLxAppClose: Popped view controller from navigation stack", log: Self.log, type: .info)
        } else {
            os_log("performLxAppClose: No navigation controller found, using fallback", log: Self.log, type: .info)
            // Fallback: try to dismiss or remove from parent
            if presentingViewController != nil {
                dismiss(animated: false)
            } else if parent != nil {
                removeFromParent()
                view.removeFromSuperview()
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
            if TabBarConfig.isTransparent(tabBar.config?.background_color ?? 0) {
                tabBar.forceTransparencyMode()
            }
        }
    }

    private func addCapsuleButton() {
        // Use the unified SwiftUI implementation
        LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
    }

    @objc func moreButtonTapped() {
        print("More button tapped")
        // Implement more functionality
    }

    @objc func closeButtonTapped() {
        print("Close button tapped")
        // Close the current app
        iOSLxApp.closeLxApp(appId: appId)
    }

    private func ensureTransparentNavigationArea(forceLayout: Bool = true) {
        // When NavigationBar is hidden, ensure the entire status bar and navigation area is transparent
        view.backgroundColor = UIColor.clear
        view.isOpaque = false
        view.layer.backgroundColor = UIColor.clear.cgColor

        rootContainer.backgroundColor = UIColor.clear
        rootContainer.isOpaque = false
        rootContainer.layer.backgroundColor = UIColor.clear.cgColor

        // Keep webViewContainer transparent to allow WebView transparency
        webViewContainer?.backgroundColor = UIColor.clear
        webViewContainer?.isOpaque = false

        // Force layout update only if requested
        if forceLayout {
            view.setNeedsLayout()
            view.layoutIfNeeded()
        }
    }

    public func ensureNavigationBarExists() {
        guard navigationBar == nil else {
            return
        }
        // NavigationBar should include status bar area in its total height
        let navBarContentHeight: CGFloat = 44 // Content area height
        let totalNavBarHeight = navBarContentHeight + PLATFORM_STATUS_BAR_HEIGHT
        let frame = CGRect(x: 0, y: 0, width: view.bounds.width, height: totalNavBarHeight)
        let newNavBar = LingXiaNavigationBar(frame: frame)

        newNavBar.translatesAutoresizingMaskIntoConstraints = false
        newNavBar.backgroundColor = UIColor.white

        // Set back button click handler using the cross-platform API
        newNavBar.setOnBackButtonClickListener { [weak self] in
            self?.handleBackButtonClick()
        }

        navigationBar = newNavBar
        rootContainer.addSubview(newNavBar)

        // Position NavigationBar from screen top (like independent implementation)
        NSLayoutConstraint.activate([
            newNavBar.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            newNavBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            newNavBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            newNavBar.heightAnchor.constraint(equalToConstant: totalNavBarHeight)
        ])

        rootContainer.bringSubviewToFront(newNavBar)

        // Force WebView container constraints to update after NavigationBar creation
        if let currentWebView = currentWebView {
            addWebViewToContainer(currentWebView)
        }
    }

    public func removeNavigationBar() {
        guard let navigationBar = navigationBar else {
            // Even if no navigation bar exists, ensure transparent area
            ensureTransparentNavigationArea()
            return
        }

        os_log("removeNavigationBar: Removing navigation bar", log: Self.log, type: .info)

        navigationBar.removeFromSuperview()
        self.navigationBar = nil

        // Ensure transparent area after removal
        ensureTransparentNavigationArea()

        // Force WebView container constraints to update after NavigationBar removal
        if let currentWebView = currentWebView {
            addWebViewToContainer(currentWebView)
        }
    }

    /// Optimized NavigationBar removal for TabBar switches to minimize flicker
    public func removeNavigationBarForTabSwitch() {
        guard let navigationBar = navigationBar else {
            return
        }

        os_log("removeNavigationBarForTabSwitch: Removing navigation bar for tab switch", log: Self.log, type: .info)

        navigationBar.removeFromSuperview()
        self.navigationBar = nil

        // Use the optimized transparent area method that doesn't force layout
        ensureTransparentNavigationArea(forceLayout: false)

        // Skip the WebView container constraint update to avoid layout flicker
        // The frame-based layout will handle positioning automatically
    }

    public override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)

        if isBeingDismissed {
            cleanupResources()
        }
    }

    private func cleanupResources() {
        // Remove notification observers safely
        if let observer = closeAppObserver {
            NotificationCenter.default.removeObserver(observer)
            closeAppObserver = nil
        }
        if let observer = switchPageObserver {
            NotificationCenter.default.removeObserver(observer)
            switchPageObserver = nil
        }

        // Pause current WebView but don't release it - Rust manages lifecycle
        currentWebView?.pauseWebView()

        // Clear our reference but don't release the WebView
        currentWebView = nil

        // Mark as destroyed to prevent further operations
        isDestroyed = true
    }

    private func performCleanupBeforeReplacement() {
        os_log("performCleanupBeforeReplacement: Starting UI cleanup for appId=%@", log: Self.log, type: .info, appId)

        // Stop background monitoring
        isDestroyed = true

        // Remove notification observers
        cleanupResources()

        // Hide and pause current WebView but don't release it
        if let currentWebView = currentWebView {
            currentWebView.isHidden = true
            currentWebView.pauseWebView()
            os_log("performCleanupBeforeReplacement: Paused and hidden current WebView", log: Self.log, type: .info)
        }

        // Clear current WebView reference
        self.currentWebView = nil

        os_log("performCleanupBeforeReplacement: Completed UI cleanup", log: Self.log, type: .info)
    }

    deinit {
        // Cleanup observers to prevent leaks
        if let closeAppObserver = closeAppObserver {
            NotificationCenter.default.removeObserver(closeAppObserver)
        }
        if let switchPageObserver = switchPageObserver {
            NotificationCenter.default.removeObserver(switchPageObserver)
        }

        os_log("iOSLxAppViewController: iOSLxAppViewController deinitialized", log: miniAppViewControllerLog, type: .debug)
    }
}
#endif
