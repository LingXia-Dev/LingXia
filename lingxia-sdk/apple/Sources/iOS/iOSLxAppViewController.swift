#if os(iOS)
import UIKit
import WebKit
import os.log
@preconcurrency import ObjectiveC

// Log instance outside of @MainActor to avoid isolation issues
private let miniAppViewControllerLog = OSLog(subsystem: "LingXia", category: "iOSLxAppView")

@MainActor
public class iOSLxAppViewController: UIViewController {
    private static let log = miniAppViewControllerLog

    public static let EXTRA_APP_ID = "appId"
    public static let EXTRA_PATH = "path"
    // NavigationBar title and capsule button vertical position (status bar + margin)
    internal static let NAV_TITLE_VERTICAL_POSITION: CGFloat = PLATFORM_STATUS_BAR_HEIGHT + 8

    // UI Element Tags
    private static let CAPSULE_BUTTON_TAG = 9999
    private static let CURRENT_WEBVIEW_CONTAINER_TAG = 999
    private static let OLD_WEBVIEW_CONTAINER_TAG = 998

    internal var appId: String
    private var initialPath: String
    private var rootContainer: UIView!
    private var statusBarBackground: UIView!
    private var webViewContainer: UIView!
    internal var tabBar: LingXiaTabBar?
    internal var navigationBar: NavigationBar?
    internal var isDestroyed = false
    private var pendingWebViewSetup = false
    private var isDisplayingHomeLxApp: Bool = false
    private var hasInitializedTransparency: Bool = false

    internal var currentWebView: WKWebView?

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

    /// Check if current page should use transparent mode
    private var shouldUseTransparentMode: Bool {
        let isInitialRoute = PageNavigationCore.isInitialRoute(appId: appId, path: initialPath)
        let isTabBarTransparent = tabBar != nil && TabBarConfig.isTransparent(tabBar!.config?.background_color.toString() ?? "")
        return isInitialRoute || isTabBarTransparent
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

        if #available(iOS 11.0, *) {
            additionalSafeAreaInsets = .zero
        } else {
            automaticallyAdjustsScrollViewInsets = false
        }
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

    public static func updateNavigationBarTransparency(viewController: UIViewController, isTabBarTransparent: Bool, tabBarBackgroundColor: UIColor? = nil) {
        if let navController = viewController.navigationController {
            if isTabBarTransparent {
                navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
                navController.navigationBar.shadowImage = UIImage()
            } else {
                let navBarColor = tabBarBackgroundColor ?? UIColor.white
                navController.navigationBar.backgroundColor = navBarColor
            }
        }
    }

    public init(appId: String, path: String) {
        self.appId = appId
        self.isDisplayingHomeLxApp = LxAppCore.isHomeLxApp(appId)
        self.initialPath = path
        super.init(nibName: nil, bundle: nil)

        iOSLxAppViewController.configureTransparentSystemBars(viewController: self)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    public override func viewDidLoad() {
        super.viewDidLoad()

        configureEdgeToEdgeDisplay()

        if let navController = navigationController {
            navController.setNavigationBarHidden(true, animated: false)
        }

        // Set initial transparent background to prevent black flash during startup
        view.backgroundColor = UIColor.clear
        view.isOpaque = false

        // Mark transparency as initialized early to prevent redundant operations
        hasInitializedTransparency = true

        navigationBar = nil

        setupNotificationObservers()

        let tabBarConfig = getTabBarConfig(self.appId)

        setupRootContainer()
        setupWebViewContainer()

        // Ensure transparent navigation area after containers are set up (without layout forcing)
        ensureTransparentNavigationAreaWithoutLayout()

        if let tabBarConfig = tabBarConfig {
            setupTabBar(config: tabBarConfig)
        }

        // Sync TabBar selected state with current path BEFORE setting up WebView
        if let tabBar = tabBar {
            tabBar.syncSelectedTabWithCurrentPath(initialPath)
        }

        if currentWebView == nil {
            setupInitialContent(path: initialPath)
        } else {
            attachWebViewToUI(webView: currentWebView!)
            updateNavigationBar(appId: appId, path: initialPath, isBackNavigation: false, disableAnimation: true)
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
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") {
            // Use immediate application without delays to avoid startup flicker
            tabBar.forceTransparencyMode()
        }

        // Create capsule button after view appears to minimize startup operations
        addCapsuleButton()
    }

    public override var preferredStatusBarStyle: UIStatusBarStyle {
        if shouldUseTransparentMode {
            if #available(iOS 13.0, *) {
                return .darkContent
            } else {
                return .default
            }
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

        // Only set window background once to avoid status bar flicker
        if let window = view.window, window.backgroundColor != UIColor.clear {
            window.backgroundColor = UIColor.clear
            window.isOpaque = false
        }

        // Force all view hierarchy to be transparent
        view.backgroundColor = UIColor.clear
        view.isOpaque = false

        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.clear
        }

        hasInitializedTransparency = true
    }

    private func setupNotificationObservers() {
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_CLOSE_LXAPP),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let targetAppId = userInfo["appId"] as? String else { return }

            Task { @MainActor in
                guard targetAppId == self.appId else { return }

                if self.presentingViewController != nil {
                    self.dismiss(animated: false)
                } else if !self.isDisplayingHomeLxApp {
                    self.performLxAppClose()
                }
            }
        }

        switchPageObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let targetAppId = userInfo["appId"] as? String,
                  let targetPath = userInfo["path"] as? String else { return }

            Task { @MainActor in
                guard targetAppId == self.appId else { return }
                self.switchPage(targetPath: targetPath)
            }
        }
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

    /// Gets the actual status bar height from the window scene
    private func getActualStatusBarHeight() -> CGFloat {
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
            return windowScene.statusBarManager?.statusBarFrame.height ?? 20
        }
        return 20 // Default fallback height
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

    internal func setupWebViewIfReady(appId: String, path: String) {
        if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
            // Pause current WebView if it exists and is different from target
            if let currentWebView = currentWebView, currentWebView != webView {
                currentWebView.pauseWebView()
                currentWebView.isHidden = true

            }

            attachWebViewToUI(webView: webView)
            updateNavigationBar(appId: appId, path: path, isBackNavigation: false, disableAnimation: true)

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
                    self?.updateNavigationBar(appId: appId, path: path, isBackNavigation: false, disableAnimation: true)

                    // Force onPageShow to ensure WebView loads content
                    lingxia.onPageShow(appId, path)
                }
            }
        }
    }

    /// Setup WebView without NavigationBar update (for TabBar switches where NavBar is already handled)
    internal func setupWebViewWithoutNavBarUpdate(appId: String, path: String) {
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
    internal func applyTransparencyEffectsAfterTabSwitch() {
        // For TabBar root to root switches, minimize operations to avoid any flicker
        guard let tabBar = tabBar else { return }

        // Only apply transparency if TabBar is actually transparent
        if TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") {
            // Use immediate transparency application without any delays
            tabBar.forceTransparencyMode()

            // Apply WebView transparency if current WebView exists
            if let currentWebView = currentWebView {
                forceWebViewTransparency(webView: currentWebView)
            }
        }

        // Minimal UI layering - only bring TabBar to front for root-to-root switches
        rootContainer.bringSubviewToFront(tabBar)

        // Bring capsule button to front if it exists
        if let capsuleButton = rootContainer.viewWithTag(Self.CAPSULE_BUTTON_TAG) {
            rootContainer.bringSubviewToFront(capsuleButton)
        }
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
        applyTransparencyEffectsIfNeeded(for: webView)
    }

    /// Apply transparency effects without triggering layout changes
    private func applyTransparencyEffectsIfNeeded(for webView: WKWebView) {
        // 1. Apply TabBar transparency if needed
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") {
            // Use immediate transparency application to avoid delays
            tabBar.forceTransparencyMode()

            // Apply WebView transparency
            forceWebViewTransparency(webView: webView)
        }

        // 2. Ensure NavigationBar area transparency for pages without NavigationBar
        // Skip this for TabBar root pages to avoid unnecessary layout updates
        if navigationBar == nil {
            ensureTransparentNavigationAreaWithoutLayout()
        }

        // 3. Bring UI elements to front without triggering layout
        bringUIElementsToFrontWithoutLayout()
    }

    /// Bring UI elements to front without triggering expensive layout operations
    private func bringUIElementsToFrontWithoutLayout() {
        // Bring NavigationBar to front
        if let navigationBar = navigationBar {
            rootContainer.bringSubviewToFront(navigationBar.view)
        }

        // Bring TabBar to front
        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)
        }

        // Bring capsule button to front
        if let capsuleButton = rootContainer.viewWithTag(Self.CAPSULE_BUTTON_TAG) {
            rootContainer.bringSubviewToFront(capsuleButton)
        }
    }

    private func setupTabBar(config: TabBarConfig?) {
        guard let config = config else {
            os_log("Invalid or insufficient TabBar config", log: Self.log, type: .error)
            return
        }

        let isTabBarTransparent = TabBarConfig.isTransparent(config.background_color.toString())

        // Update system navigation bar transparency based on TabBar transparency and color
        iOSLxAppViewController.updateNavigationBarTransparency(
            viewController: self,
            isTabBarTransparent: isTabBarTransparent,
            tabBarBackgroundColor: TabBarConfig.parseColor(config.background_color.toString())
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
        let isTopTabBar = tabBar?.config?.position == 1 // 1 = top
        let hasNavigationBar = navigationBar != nil

        if isTopTabBar {
            return (tabBar?.bottomAnchor ?? rootContainer.topAnchor, 0)
        } else if hasNavigationBar {
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
        guard let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") else {
            return
        }

        // Apply complete transparency to all UI elements
        setCompleteTransparency()

        // Re-apply navbar configuration after transparency changes
        // This ensures page-specific navbar colors are preserved
        let currentPath = webView?.currentPath ?? initialPath
        updateNavigationBar(appId: appId, path: currentPath, isBackNavigation: false, disableAnimation: true)

        // Apply specific WebView transparency if provided
        if let webView = webView {
            forceWebViewTransparency(webView: webView)
        }
    }

    private func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBarConfig) {
        let isVertical = config.position == 2 || config.position == 3 // 2=left, 3=right
        let tabBarSize = CGFloat(config.dimension) // Use configured dimension instead of default

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        if isVertical {
            NSLayoutConstraint.activate([
                tabBar.widthAnchor.constraint(equalToConstant: tabBarSize),
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: PLATFORM_STATUS_BAR_HEIGHT),
                tabBar.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
            ])

            if config.position == 2 { // left
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

            if config.position == 1 { // top
                // For top position, place TabBar right after the fixed status bar area (48pt)
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: PLATFORM_STATUS_BAR_HEIGHT).isActive = true
            } else {
                // For bottom position, always extend to view.bottomAnchor to cover safe area
                // Both transparent and opaque TabBars extend to actual screen bottom
                // The difference is handled internally by the TabBar component
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor).isActive = true
            }
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
            if TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") {
                tabBar.forceTransparencyMode()
            } else {
                // If TabBar is not transparent, restore its original background color
                let resolvedColor = tabBar.controller.getResolvedBackgroundColor()
                tabBar.backgroundColor = resolvedColor
                tabBar.layer.backgroundColor = resolvedColor.cgColor
                tabBar.isOpaque = true
                tabBar.layer.isOpaque = true
            }
        }

        // Ensure NavigationBar remains visible (don't make it transparent)
        if let navigationBar = navigationBar {
            // Keep NavigationBar opaque with white background
            navigationBar.view.backgroundColor = UIColor.white
            navigationBar.view.isOpaque = true
            navigationBar.view.layer.backgroundColor = UIColor.white.cgColor

        }
    }

    /// Forces transparency on a specific WebView after it's added to view hierarchy
    private func forceWebViewTransparency(webView: WKWebView) {
        configureWebView(webView, transparent: true)
    }

    internal func performLxAppClose() {
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

    private func updateLayoutMargins() {

        guard let webViewContainer = webViewContainer else {
            os_log("LingXia: updateLayoutMargins - webViewContainer is nil!", log: Self.log, type: .error)
            return
        }

        webViewContainer.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.deactivate(webViewContainer.constraints)
        if let superview = webViewContainer.superview {
            let containerConstraints = superview.constraints.filter { constraint in
                constraint.firstItem === webViewContainer || constraint.secondItem === webViewContainer
            }
            NSLayoutConstraint.deactivate(containerConstraints)
        }

        if let currentWebView = currentWebView {
            if currentWebView.superview != webViewContainer {
                currentWebView.removeFromSuperview()
                webViewContainer.addSubview(currentWebView)
            }
            currentWebView.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                currentWebView.topAnchor.constraint(equalTo: webViewContainer.topAnchor),
                currentWebView.leadingAnchor.constraint(equalTo: webViewContainer.leadingAnchor),
                currentWebView.trailingAnchor.constraint(equalTo: webViewContainer.trailingAnchor),
                currentWebView.bottomAnchor.constraint(equalTo: webViewContainer.bottomAnchor)
            ])
        }

        let topAnchor: NSLayoutYAxisAnchor
        let topConstant: CGFloat

        let isTopTabBar = tabBar?.config?.position == 1 // 1 = top
        let hasNavigationBar = navigationBar != nil

        if isTopTabBar {
            // WebView starts from TabBar bottom when TabBar is at top
            topAnchor = tabBar?.bottomAnchor ?? rootContainer.topAnchor
            topConstant = 0
        } else if hasNavigationBar {
            // WebView starts from NavigationBar bottom when NavigationBar exists
            // FORCE correct positioning: NavigationBar is at Y=0 with height=STATUS_BAR_HEIGHT+44
            topAnchor = rootContainer.topAnchor
            topConstant = PLATFORM_STATUS_BAR_HEIGHT + 44 // NavigationBar total height (status bar + nav content)
            os_log("updateLayoutMargins: FORCED WebView below NavigationBar, topConstant=%f, NavigationBar frame=(%f,%f,%f,%f)",
                   log: Self.log, type: .info, topConstant,
                   navigationBar!.view.frame.origin.x, navigationBar!.view.frame.origin.y,
                   navigationBar!.view.frame.size.width, navigationBar!.view.frame.size.height)
        } else {
            // WebView fills entire screen when no NavigationBar or top TabBar
            topAnchor = rootContainer.topAnchor
            topConstant = 0
        }

        let bottomAnchor: NSLayoutYAxisAnchor
        let isBottomTabBar = tabBar?.config?.position == 0 // 0 = bottom

        if isBottomTabBar {
            // Check if TabBar is transparent using the proper method
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBar?.config?.background_color.toString() ?? "")

            if isTabBarTransparent {
                // For transparent TabBar, WebView extends to actual screen bottom (including home indicator area)
                bottomAnchor = view.bottomAnchor
            } else {
                // For opaque TabBar, WebView stops at TabBar top
                bottomAnchor = tabBar?.topAnchor ?? rootContainer.bottomAnchor
            }
        } else {
            bottomAnchor = rootContainer.bottomAnchor
        }

        // Handle horizontal (leading/trailing) constraints based on TabBar position
        let leadingAnchor: NSLayoutXAxisAnchor
        let trailingAnchor: NSLayoutXAxisAnchor

        let isLeftTabBar = tabBar?.config?.position == 2 // 2 = left
        let isRightTabBar = tabBar?.config?.position == 3 // 3 = right

        if isLeftTabBar {
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBar?.config?.background_color.toString() ?? "")
            if isTabBarTransparent {
                // For transparent left TabBar, WebView extends to screen edge (TabBar overlays)
                leadingAnchor = rootContainer.leadingAnchor
            } else {
                // For opaque left TabBar, WebView starts from TabBar's right edge
                leadingAnchor = tabBar?.trailingAnchor ?? rootContainer.leadingAnchor
            }
            trailingAnchor = rootContainer.trailingAnchor
        } else if isRightTabBar {
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBar?.config?.background_color.toString() ?? "")
            leadingAnchor = rootContainer.leadingAnchor
            if isTabBarTransparent {
                // For transparent right TabBar, WebView extends to screen edge (TabBar overlays)
                trailingAnchor = rootContainer.trailingAnchor
            } else {
                // For opaque right TabBar, WebView ends at TabBar's left edge
                trailingAnchor = tabBar?.leadingAnchor ?? rootContainer.trailingAnchor
            }
        } else {
            // No horizontal TabBar, use full width
            leadingAnchor = rootContainer.leadingAnchor
            trailingAnchor = rootContainer.trailingAnchor
        }

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: topAnchor, constant: topConstant),
            webViewContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        // FORCE layout update immediately
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()
        webViewContainer.setNeedsLayout()
        webViewContainer.layoutIfNeeded()

        bringUIElementsToFront()

        // Debug WebView container position
        os_log("updateLayoutMargins: WebView container frame=(%f,%f,%f,%f)",
               log: Self.log, type: .info,
               webViewContainer.frame.origin.x, webViewContainer.frame.origin.y,
               webViewContainer.frame.size.width, webViewContainer.frame.size.height)

        // Force TabBar to be on top with higher z-position for transparent effect
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") {
            tabBar.layer.zPosition = 1000
        }
    }

    private func bringUIElementsToFront() {
        // Bring NavigationBar to front first
        if let navigationBar = navigationBar {
            rootContainer.bringSubviewToFront(navigationBar.view)
            os_log("bringUIElementsToFront: NavigationBar brought to front", log: Self.log, type: .info)
        }

        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)

            // Re-apply transparency immediately after bringSubviewToFront
            if TabBarConfig.isTransparent(tabBar.config?.background_color.toString() ?? "") {
                tabBar.forceTransparencyMode()
            }
        }

        if let capsuleButton = rootContainer.viewWithTag(Self.CAPSULE_BUTTON_TAG) {
            rootContainer.bringSubviewToFront(capsuleButton)
        }
    }

    private func calculateWebViewTranslationY() -> CGFloat {
        guard let navigationBar = navigationBar, !navigationBar.isHidden else {
            return 0
        }

        // Use fixed status bar height
        let navBarContentHeight = navigationBar.getCalculatedContentHeight()

        return PLATFORM_STATUS_BAR_HEIGHT + navBarContentHeight
    }

    private func addCapsuleButton() {
        // Don't show capsule button for the main/home app
        if isDisplayingHomeLxApp {
            return
        }

        // Create capsule container (matching Android dimensions and styling)
        let capsule = UIView()
        capsule.backgroundColor = UIColor.white
        capsule.layer.cornerRadius = 18 // Half of height (36/2) for perfect rounded corners
        capsule.layer.borderWidth = 0.5 // 0.5f * density
        capsule.layer.borderColor = UIColor(red: 0.867, green: 0.867, blue: 0.867, alpha: 1.0).cgColor // #DDDDDD
        capsule.layer.shadowColor = UIColor.black.cgColor
        capsule.layer.shadowOffset = CGSize(width: 0, height: 1)
        capsule.layer.shadowOpacity = 0.1
        capsule.layer.shadowRadius = 2
        capsule.layer.zPosition = 1000
        capsule.tag = Self.CAPSULE_BUTTON_TAG
        capsule.translatesAutoresizingMaskIntoConstraints = false

        // Create more button with custom dots
        let btnMore = UIButton(type: .custom)
        btnMore.backgroundColor = UIColor.clear
        btnMore.setImage(createMoreDotsImage(), for: .normal)
        btnMore.addTarget(self, action: #selector(moreButtonTapped), for: .touchUpInside)

        // Create divider
        let divider = UIView()
        divider.backgroundColor = UIColor(red: 0.867, green: 0.867, blue: 0.867, alpha: 1.0) // #DDDDDD
        divider.translatesAutoresizingMaskIntoConstraints = false

        // Create close button with custom X
        let btnClose = UIButton(type: .custom)
        btnClose.backgroundColor = UIColor.clear
        btnClose.setImage(createCloseButtonImage(), for: .normal)
        btnClose.addTarget(self, action: #selector(closeButtonTapped), for: .touchUpInside)

        // Add buttons to capsule with proper layout
        btnMore.translatesAutoresizingMaskIntoConstraints = false
        btnClose.translatesAutoresizingMaskIntoConstraints = false

        capsule.addSubview(btnMore)
        capsule.addSubview(divider)
        capsule.addSubview(btnClose)
        rootContainer.addSubview(capsule)

        // Bring capsule to front to ensure it's visible
        rootContainer.bringSubviewToFront(capsule)

        // Set up constraints to match Android layout exactly
        NSLayoutConstraint.activate([
            // Capsule positioning (aligned with NavigationBar title - same as NavigationBar title position)
            // NavigationBar title and capsule button share the same vertical position
            capsule.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: iOSLxAppViewController.NAV_TITLE_VERTICAL_POSITION - 4),
            capsule.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor, constant: -12),
            capsule.heightAnchor.constraint(equalToConstant: 36), // Android: 36dp

            // More button
            btnMore.leadingAnchor.constraint(equalTo: capsule.leadingAnchor, constant: 2), // Android padding
            btnMore.topAnchor.constraint(equalTo: capsule.topAnchor),
            btnMore.bottomAnchor.constraint(equalTo: capsule.bottomAnchor),
            btnMore.widthAnchor.constraint(equalToConstant: 44),

            // Divider
            divider.leadingAnchor.constraint(equalTo: btnMore.trailingAnchor),
            divider.centerYAnchor.constraint(equalTo: capsule.centerYAnchor),
            divider.widthAnchor.constraint(equalToConstant: 1),
            divider.heightAnchor.constraint(equalToConstant: 20),

            // Close button
            btnClose.leadingAnchor.constraint(equalTo: divider.trailingAnchor),
            btnClose.trailingAnchor.constraint(equalTo: capsule.trailingAnchor, constant: -2), // Android padding
            btnClose.topAnchor.constraint(equalTo: capsule.topAnchor),
            btnClose.bottomAnchor.constraint(equalTo: capsule.bottomAnchor),
            btnClose.widthAnchor.constraint(equalToConstant: 44)
        ])
    }

    private func createMoreDotsImage() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)

        guard let context = UIGraphicsGetCurrentContext() else { return nil }

        // Enable anti-aliasing for smoother drawing
        context.setShouldAntialias(true)
        context.setAllowsAntialiasing(true)

        UIColor.black.setFill()

        let centerY = size.height / 2
        let centerX = size.width / 2

        // Center dot is larger, side dots are smaller
        let centerDotRadius = size.height / 7  // Larger center dot
        let sideDotRadius = size.height / 10   // Smaller side dots
        let spacing = centerDotRadius * 2.8    // Adjusted spacing

        // Draw side dots (smaller)
        let leftDotRect = CGRect(
            x: centerX - spacing - sideDotRadius,
            y: centerY - sideDotRadius,
            width: sideDotRadius * 2,
            height: sideDotRadius * 2
        )
        context.fillEllipse(in: leftDotRect)

        let rightDotRect = CGRect(
            x: centerX + spacing - sideDotRadius,
            y: centerY - sideDotRadius,
            width: sideDotRadius * 2,
            height: sideDotRadius * 2
        )
        context.fillEllipse(in: rightDotRect)

        // Draw center dot (larger)
        let centerDotRect = CGRect(
            x: centerX - centerDotRadius,
            y: centerY - centerDotRadius,
            width: centerDotRadius * 2,
            height: centerDotRadius * 2
        )
        context.fillEllipse(in: centerDotRect)

        let image = UIGraphicsGetImageFromCurrentImageContext()
        UIGraphicsEndImageContext()
        return image
    }

    private func createCloseButtonImage() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)

        guard let context = UIGraphicsGetCurrentContext() else { return nil }

        // Enable anti-aliasing for smoother drawing
        context.setShouldAntialias(true)
        context.setAllowsAntialiasing(true)

        let centerX = size.width / 2
        let centerY = size.height / 2

        let circleRadius = size.width * 0.35
        UIColor.black.setStroke()
        context.setLineWidth(2.2)
        context.setLineCap(.round)
        context.setLineJoin(.round)

        // Draw circle outline
        let circleRect = CGRect(
            x: centerX - circleRadius,
            y: centerY - circleRadius,
            width: circleRadius * 2,
            height: circleRadius * 2
        )
        context.strokeEllipse(in: circleRect)

        UIColor.black.setFill()
        let dotRadius: CGFloat = 2.5
        let dotRect = CGRect(
            x: centerX - dotRadius,
            y: centerY - dotRadius,
            width: dotRadius * 2,
            height: dotRadius * 2
        )
        context.fillEllipse(in: dotRect)

        let image = UIGraphicsGetImageFromCurrentImageContext()
        UIGraphicsEndImageContext()
        return image
    }

    @objc internal func moreButtonTapped() {
        // More options functionality to be implemented
    }

    @objc internal func closeButtonTapped() {
        guard !isDisplayingHomeLxApp else {
            return
        }
        performLxAppClose()
    }

    private func ensureTransparentNavigationArea() {
        // When NavigationBar is hidden, ensure the entire status bar and navigation area is transparent
        // Force transparent backgrounds in the view hierarchy
        view.backgroundColor = UIColor.clear
        view.isOpaque = false
        view.layer.backgroundColor = UIColor.clear.cgColor

        rootContainer.backgroundColor = UIColor.clear
        rootContainer.isOpaque = false
        rootContainer.layer.backgroundColor = UIColor.clear.cgColor

        // Keep webViewContainer transparent to allow WebView transparency
        webViewContainer?.backgroundColor = UIColor.clear
        webViewContainer?.isOpaque = false

        // Force layout update to ensure transparency takes effect
        view.setNeedsLayout()
        view.layoutIfNeeded()
    }

    /// Ensure transparent navigation area without triggering layout updates (for TabBar switches)
    private func ensureTransparentNavigationAreaWithoutLayout() {
        // Set transparent backgrounds without forcing layout updates
        view.backgroundColor = UIColor.clear
        view.isOpaque = false
        view.layer.backgroundColor = UIColor.clear.cgColor

        rootContainer.backgroundColor = UIColor.clear
        rootContainer.isOpaque = false
        rootContainer.layer.backgroundColor = UIColor.clear.cgColor

        // Keep webViewContainer transparent
        webViewContainer?.backgroundColor = UIColor.clear
        webViewContainer?.isOpaque = false
    }

    internal func ensureNavigationBarExists() {
        guard navigationBar == nil else {
            return
        }
        // NavigationBar should include status bar area in its total height
        let navBarContentHeight: CGFloat = 44 // Content area height
        let totalNavBarHeight = navBarContentHeight + PLATFORM_STATUS_BAR_HEIGHT
        let frame = CGRect(x: 0, y: 0, width: view.bounds.width, height: totalNavBarHeight)
        let newNavBar = NavigationBar(frame: frame)

        newNavBar.view.translatesAutoresizingMaskIntoConstraints = false
        newNavBar.view.backgroundColor = UIColor.white

        // Set back button click handler using the cross-platform API
        newNavBar.setOnBackButtonClickListener { [weak self] in
            self?.handleBackButtonClick()
        }

        navigationBar = newNavBar
        rootContainer.addSubview(newNavBar.view)

        // Position NavigationBar from screen top (like independent implementation)
        NSLayoutConstraint.activate([
            newNavBar.view.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            newNavBar.view.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            newNavBar.view.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            newNavBar.view.heightAnchor.constraint(equalToConstant: totalNavBarHeight)
        ])

        rootContainer.bringSubviewToFront(newNavBar.view)

        // Force WebView container constraints to update after NavigationBar creation
        if let currentWebView = currentWebView {
            addWebViewToContainer(currentWebView)
        }
    }

    internal func removeNavigationBar() {
        guard let navigationBar = navigationBar else {
            // Even if no navigation bar exists, ensure transparent area
            ensureTransparentNavigationArea()
            return
        }

        os_log("removeNavigationBar: Removing navigation bar", log: Self.log, type: .info)

        navigationBar.view.removeFromSuperview()
        self.navigationBar = nil

        // Ensure transparent area after removal
        ensureTransparentNavigationArea()

        // Force WebView container constraints to update after NavigationBar removal
        if let currentWebView = currentWebView {
            addWebViewToContainer(currentWebView)
        }
    }

    /// Optimized NavigationBar removal for TabBar switches to minimize flicker
    internal func removeNavigationBarForTabSwitch() {
        guard let navigationBar = navigationBar else {
            return
        }

        os_log("removeNavigationBarForTabSwitch: Removing navigation bar for tab switch", log: Self.log, type: .info)

        navigationBar.view.removeFromSuperview()
        self.navigationBar = nil

        // Use the optimized transparent area method that doesn't force layout
        ensureTransparentNavigationAreaWithoutLayout()

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
        // Ensure cleanup happens even if viewWillDisappear wasn't called
        if !isDestroyed {
            // Perform immediate cleanup of observers to prevent leaks
            if let closeAppObserver = closeAppObserver {
                NotificationCenter.default.removeObserver(closeAppObserver)
            }
            if let switchPageObserver = switchPageObserver {
                NotificationCenter.default.removeObserver(switchPageObserver)
            }

            // Mark as destroyed to prevent further operations
            isDestroyed = true
        }
        os_log("iOSLxAppViewController: iOSLxAppViewController deinitialized", log: miniAppViewControllerLog, type: .debug)
    }
}
#endif
