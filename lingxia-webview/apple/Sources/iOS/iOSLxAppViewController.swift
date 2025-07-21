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
    internal static let DEFAULT_NAV_BAR_HEIGHT: CGFloat = 44
    internal static let DEFAULT_TAB_BAR_SIZE: CGFloat = 64
    internal static let STATUS_BAR_HEIGHT: CGFloat = 48
    // NavigationBar title and capsule button vertical position (status bar + margin)
    internal static let NAV_TITLE_VERTICAL_POSITION: CGFloat = 48 + 8

    // UI Element Tags
    private static let CAPSULE_BUTTON_TAG = 9999
    private static let CURRENT_WEBVIEW_CONTAINER_TAG = 999
    private static let OLD_WEBVIEW_CONTAINER_TAG = 998

    internal var appId: String
    private var initialPath: String
    private var rootContainer: UIView!
    private var statusBarBackground: UIView!
    private var tabBar: LingXiaTabBar?
    private var navigationBar: NavigationBar?
    private var isDestroyed = false
    private var pendingWebViewSetup = false
    private var isDisplayingHomeLxApp: Bool = false

    private var currentWebView: WKWebView?

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?

    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if #available(iOS 13.0, *) {
            viewController.overrideUserInterfaceStyle = lightStatusBarIcons ? .light : .dark
        }

        if let navController = viewController.navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    private func configureEdgeToEdgeDisplay() {
        // Configure status bar style
        if #available(iOS 13.0, *) {
            overrideUserInterfaceStyle = .light
        }

        // Ensure modal presentation style allows edge-to-edge
        modalPresentationStyle = .fullScreen

        // Configure view controller for edge-to-edge
        edgesForExtendedLayout = [.top, .bottom]
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

        os_log("ViewDidLoad started for appId: %@", log: Self.log, type: .info, self.appId)

        // Configure for true edge-to-edge display
        configureEdgeToEdgeDisplay()

        if let navController = navigationController {
            navController.setNavigationBarHidden(true, animated: false)
        }

        // FORCE transparent background at all levels to prevent black background
        view.backgroundColor = UIColor.clear
        view.isOpaque = false

        // Also force the parent view controller's view to be transparent
        if let parentVC = parent {
            parentVC.view.backgroundColor = UIColor.clear
            parentVC.view.isOpaque = false
        }

        // Force the navigation controller's view to be transparent
        if let navController = navigationController {
            navController.view.backgroundColor = UIColor.clear
            navController.view.isOpaque = false
        }

        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.clear
        }

        navigationBar = nil

        setupNotificationObservers()

        let tabBarJson = getTabBarConfig(appId)?.toString()
        let tabBarConfig = TabBarConfig.fromJson(tabBarJson)

        setupRootContainer()

        // Ensure transparent navigation area after containers are set up
        ensureTransparentNavigationArea()

        if let tabBarConfig = tabBarConfig {
            setupTabBar(config: tabBarConfig)

            let isTabBarTransparent = TabBarConfig.isTransparent(tabBarConfig.backgroundColor)
            os_log("iOSLxAppViewController.viewDidLoad: isTabBarTransparent=%{public}@ backgroundColor=%{public}@",
                   log: Self.log, type: .info,
                   String(isTabBarTransparent), tabBarConfig.backgroundColor?.description ?? "nil")
        }

        addCapsuleButton()

        if currentWebView == nil {
            setupInitialContent(path: initialPath)
        } else {
            os_log("Using existing WebView for appId=%@ path=%@",
                   log: Self.log, type: .info, appId, initialPath)

            attachWebViewToUI(webView: currentWebView!)
        }

        // Sync TabBar selected state with current path
        if let tabBar = tabBar {
            tabBar.syncSelectedTabWithCurrentPath(initialPath)
        }

        // Set complete transparency for TabBar scenarios
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config.backgroundColor) {
            setCompleteTransparency()
        }
    }

    public override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)

        os_log("viewDidAppear: Window is now available, processing transparency", log: Self.log, type: .info)

        // Set transparent background once when view appears
        setTransparentBackground()
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        // Transparency is set in viewDidAppear when window is available
    }

    public override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()

        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)

            DispatchQueue.main.async {
                tabBar.setNeedsLayout()
                tabBar.layoutIfNeeded()
            }
        }
    }

    private func setTransparentBackground() {
        if let window = view.window {
            window.backgroundColor = UIColor.clear
            window.isOpaque = false
        }

        if #available(iOS 13.0, *) {
            if let windowScene = view.window?.windowScene {
                for window in windowScene.windows {
                    window.backgroundColor = UIColor.clear
                    window.isOpaque = false
                }
            }
        }

        view.backgroundColor = UIColor.clear
        view.isOpaque = false

        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.clear
        }
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

                os_log("Received close request for appId: %@", log: Self.log, type: .info, self.appId)

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

                os_log("Received switch page notification - appId: %@ path: %@", log: Self.log, type: .info, self.appId, targetPath)
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

    private func setupStatusBarBackground() {
        statusBarBackground = UIView()
        statusBarBackground.backgroundColor = UIColor.clear
        statusBarBackground.isOpaque = false
        statusBarBackground.translatesAutoresizingMaskIntoConstraints = false
        rootContainer.addSubview(statusBarBackground)

        NSLayoutConstraint.activate([
            statusBarBackground.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            statusBarBackground.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            statusBarBackground.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            statusBarBackground.heightAnchor.constraint(equalToConstant: iOSLxAppViewController.STATUS_BAR_HEIGHT)
        ])
    }

    private func setupInitialContent(path: String) {
        LxAppCore.setLastActivePath(path, for: appId)
        setupWebViewIfReady(appId: appId, path: path)
    }

    private func setupWebViewIfReady(appId: String, path: String) {
        if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
            attachWebViewToUI(webView: webView)
            updateNavigationBar(appId: appId, path: path, isBackNavigation: false, disableAnimation: true)
        } else {
            // Retry once after a short delay
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
                if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                    self?.attachWebViewToUI(webView: webView)
                    self?.updateNavigationBar(appId: appId, path: path, isBackNavigation: false, disableAnimation: true)
                }
            }
        }
    }

    private func attachWebViewToUI(webView: WKWebView) {
        currentWebView = webView
        addWebViewToContainer(webView)

        // Resume WebView operations
        webView.resumeWebView()

        // Remove any existing loading indicator
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            if let loadingIndicator = self.webViewContainer.viewWithTag(9997) {
                loadingIndicator.removeFromSuperview()
            }
        }

        if let appId = webView.appId, let currentPath = webView.currentPath {
            os_log("attachWebViewToUI: Calling onPageShow for appId=%{public}@ path=%{public}@",
                   log: Self.log, type: .info, appId, currentPath)
            onPageShow(appId, currentPath)

        } else {
            os_log("attachWebViewToUI: CRITICAL - onPageShow NOT called because appId or currentPath is nil!",
                   log: Self.log, type: .error)
        }

        updateNavigationBar(appId: appId, path: webView.currentPath ?? "", isBackNavigation: false, disableAnimation: true)
    }

    private func addWebViewToContainer(_ webView: WKWebView) {
        if webView.superview != rootContainer {
            if webView.superview != nil {
                webView.removeFromSuperview()
            }

            rootContainer.addSubview(webView)
            webView.translatesAutoresizingMaskIntoConstraints = false

            NSLayoutConstraint.activate([
                webView.topAnchor.constraint(equalTo: rootContainer.topAnchor),
                webView.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
                webView.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
                webView.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
            ])
        } else {
            rootContainer.bringSubviewToFront(webView)
        }

        webView.isHidden = false
        webView.alpha = 1.0

        // Force transparency for transparent TabBar
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config.backgroundColor) {
            applyMinimalTabBarTransparency()
        }
    }

    private func setupTabBar(config: TabBarConfig?) {
        guard let config = config else {
            os_log("Invalid or insufficient TabBar config", log: Self.log, type: .error)
            return
        }

        let isTabBarTransparent = TabBarConfig.isTransparent(config.backgroundColor)

        // Update system navigation bar transparency based on TabBar transparency and color
        iOSLxAppViewController.updateNavigationBarTransparency(
            viewController: self,
            isTabBarTransparent: isTabBarTransparent,
            tabBarBackgroundColor: config.parseColor(config.backgroundColor)
        )

        if tabBar == nil {
            tabBar = LingXiaTabBar()
            tabBar?.setConfig(config: config)

            tabBar?.setOnTabSelectedListener { [weak self] index, path in
                self?.switchToTab(targetPath: path)
            }

            if let tabBar = tabBar {
                rootContainer.addSubview(tabBar)
                applyTabBarLayoutParams(tabBar: tabBar, config: config)

                // TabBar transparency will be configured in viewDidAppear when window is available
                os_log("setupTabBar: TabBar created, transparency will be configured when window is available", log: Self.log, type: .info)
            }
        } else {
            tabBar?.setConfig(config: config)
            if let tabBar = tabBar {
                // TabBar is already added to view hierarchy, just update layout
                applyTabBarLayoutParams(tabBar: tabBar, config: config)

                // TabBar transparency will be re-configured in viewDidAppear when window is available
                os_log("setupTabBar: TabBar config updated, transparency will be re-configured when window is available", log: Self.log, type: .info)
            }
        }

        os_log("setupTabBar: TabBar setup complete, transparency will be configured when window is available", log: Self.log, type: .info)

        // Always update layout margins after TabBar setup
        updateLayoutMargins()
    }

    private func configureTabBarTransparencyMode(_ isTransparent: Bool) {
        // Configure background colors based on transparency mode
        configureBackgroundColors(isTransparent)

        // Configure WebView edge-to-edge behavior
        configureWebViewEdgeToEdgeBehavior(isTransparent)

        // Configure TabBar overlay positioning
        configureTabBarOverlay(isTransparent)

        // Update WebView container constraints
        updateWebViewContainerForTransparency(isTransparent)

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
            setCompleteTransparency()
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

    private func configureWebViewEdgeToEdgeBehavior(_ isTransparent: Bool) {
        guard let currentWebView = currentWebView else {
            // Store the transparency state for later when WebView is available
            return
        }

        // Delay the configuration to ensure WebView is fully ready
        DispatchQueue.main.async { [weak currentWebView] in
            guard let webView = currentWebView else { return }

            if isTransparent {
                // For transparent TabBar: allow content to extend to all edges
                webView.scrollView.contentInsetAdjustmentBehavior = .never
            } else {
                // For opaque TabBar: respect safe areas to avoid content being hidden
                webView.scrollView.contentInsetAdjustmentBehavior = .automatic
            }
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

    private func updateWebViewContainerForTransparency(_ isTransparent: Bool) {
        guard let webViewContainer = webViewContainer else { return }

        // Remove existing constraints
        NSLayoutConstraint.deactivate(webViewContainer.constraints)
        if let superview = webViewContainer.superview {
            let containerConstraints = superview.constraints.filter { constraint in
                constraint.firstItem === webViewContainer || constraint.secondItem === webViewContainer
            }
            NSLayoutConstraint.deactivate(containerConstraints)
        }

        // Calculate anchors based on transparency and TabBar position
        let (topAnchor, topConstant) = calculateTopAnchor()
        let bottomAnchor = calculateBottomAnchor(isTransparent: isTransparent)

        // Apply new constraints
        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: topAnchor, constant: topConstant),
            webViewContainer.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    private func calculateTopAnchor() -> (NSLayoutYAxisAnchor, CGFloat) {
        let isTopTabBar = tabBar?.config.position == .top
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
        let isBottomTabBar = tabBar?.config.position == .bottom

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

    private func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBarConfig) {
        let isVertical = config.position == .left || config.position == .right
        let defaultTabBarSize = iOSLxAppViewController.DEFAULT_TAB_BAR_SIZE

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        if isVertical {
            NSLayoutConstraint.activate([
                tabBar.widthAnchor.constraint(equalToConstant: defaultTabBarSize),
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: iOSLxAppViewController.STATUS_BAR_HEIGHT),
                tabBar.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
            ])

            if config.position == .left {
                tabBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor).isActive = true
            } else {
                tabBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor).isActive = true
            }
        } else {
            NSLayoutConstraint.activate([
                tabBar.heightAnchor.constraint(equalToConstant: defaultTabBarSize),
                tabBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
                tabBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor)
            ])

            if config.position == .top {
                // For top position, place TabBar right after the fixed status bar area (48pt)
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: iOSLxAppViewController.STATUS_BAR_HEIGHT).isActive = true
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

        // Force TabBar transparency if it exists
        if let tabBar = tabBar {
            tabBar.backgroundColor = UIColor.clear
            tabBar.layer.backgroundColor = UIColor.clear.cgColor
            tabBar.isOpaque = false
            tabBar.layer.isOpaque = false
            tabBar.layer.shadowOpacity = 0
            tabBar.layer.borderWidth = 0
            tabBar.forceTransparencyMode()
        }

        os_log("iOSLxAppViewController.setCompleteTransparency: Complete transparency applied",
               log: Self.log, type: .info)
    }

    private func performLxAppClose() {
        // Notify Rust layer that miniapp is being closed
        let _ = onLxappClosed(appId)
        os_log("performLxAppClose: onLxappClosed called for appId=%@", log: Self.log, type: .info, appId)

        if presentingViewController != nil {
            dismiss(animated: false)
        } else if let navController = navigationController {
            if navController.viewControllers.count > 1 {
                navController.popViewController(animated: false)
            } else {
                // This is the root view controller, dismiss the entire navigation controller
                navController.dismiss(animated: false, completion: nil)
            }
        } else {
            // Fallback: remove from parent or dismiss
            if parent != nil {
                removeFromParent()
                view.removeFromSuperview()
            } else {
                dismiss(animated: false)
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

        let isTopTabBar = tabBar?.config.position == .top
        let hasNavigationBar = navigationBar != nil

        if isTopTabBar {
            // WebView starts from TabBar bottom when TabBar is at top
            topAnchor = tabBar?.bottomAnchor ?? rootContainer.topAnchor
            topConstant = 0
        } else if hasNavigationBar {
            // WebView starts from NavigationBar bottom when NavigationBar exists
            topAnchor = navigationBar!.bottomAnchor
            topConstant = 0
        } else {
            // WebView fills entire screen when no NavigationBar or top TabBar
            topAnchor = rootContainer.topAnchor
            topConstant = 0
        }

        let bottomAnchor: NSLayoutYAxisAnchor
        let isBottomTabBar = tabBar?.config.position == .bottom

        if isBottomTabBar {
            // Check if TabBar is transparent using the proper method
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBar?.config.backgroundColor)

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

        let isLeftTabBar = tabBar?.config.position == .left
        let isRightTabBar = tabBar?.config.position == .right

        if isLeftTabBar {
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBar?.config.backgroundColor)
            if isTabBarTransparent {
                // For transparent left TabBar, WebView extends to screen edge (TabBar overlays)
                leadingAnchor = rootContainer.leadingAnchor
            } else {
                // For opaque left TabBar, WebView starts from TabBar's right edge
                leadingAnchor = tabBar?.trailingAnchor ?? rootContainer.leadingAnchor
            }
            trailingAnchor = rootContainer.trailingAnchor
        } else if isRightTabBar {
            let isTabBarTransparent = TabBarConfig.isTransparent(tabBar?.config.backgroundColor)
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

        bringUIElementsToFront()

        webViewContainer.setNeedsLayout()
        webViewContainer.layoutIfNeeded()

        // Force TabBar to be on top with higher z-position for transparent effect
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config.backgroundColor) {
            tabBar.layer.zPosition = 1000
        }
    }

    private func bringUIElementsToFront() {
        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)
        }

        // CRITICAL: Find capsule button by tag and bring to front
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

        return iOSLxAppViewController.STATUS_BAR_HEIGHT + navBarContentHeight
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
        capsule.layer.zPosition = 1000 // CRITICAL: Set highest z-position to ensure always on top
        capsule.tag = Self.CAPSULE_BUTTON_TAG // Special tag for identification and management
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

    @objc private func moreButtonTapped() {
        // More options functionality to be implemented
    }

    @objc private func closeButtonTapped() {
        guard !isDisplayingHomeLxApp else { return }
        performLxAppClose()
    }

    private func switchToTab(targetPath: String) {
        // Bail early if trying to switch to the current path
        if currentWebView?.currentPath == targetPath {
            return
        }

        // Capture reference to previous WebView before changing anything
        let previousWebView = currentWebView
        os_log("switchToTab: Previous WebView path=%{public}@", log: Self.log, type: .info, previousWebView?.currentPath ?? "nil")

        // Find target tab index
        guard let targetIndex = tabBar?.findTabIndexByPath(targetPath), targetIndex >= 0 else {
            os_log("switchToTab failed: Path '%{public}@' not found in TabBar items.", log: Self.log, type: .error, targetPath)
            return
        }
        os_log("switchToTab: Found target tab index=%d", log: Self.log, type: .info, targetIndex)

        // Find target WebView (should be created by Rust layer when needed)
        guard let targetWebView = iOSLxApp.findWebView(appId: appId, path: targetPath) else {
            os_log("switchToTab failed: WebView not found for %{public}@, should be created by Rust system", log: Self.log, type: .error, targetPath)
            return
        }

        // Set current WebView to target for tracking
        currentWebView = targetWebView

        // Update TabBar UI (without triggering listener)
        tabBar?.setSelectedIndex(targetIndex, notifyListener: false)

        // Handle NavigationBar using the new unified approach
        updateNavigationBar(appId: appId, path: targetPath, isBackNavigation: false, disableAnimation: true)

        // CRITICAL: Update layout margins to ensure WebView container is positioned correctly
        // This ensures WebView starts below NavigationBar when it exists, or from top when hidden
        updateLayoutMargins()

        os_log("switchToTab: Attaching WebView", log: Self.log, type: .info)
        attachWebViewToUI(webView: targetWebView)

        // Hide and pause previous WebView (but don't release it)
        if let previousWebView = previousWebView, previousWebView != targetWebView {
            previousWebView.pauseWebView()
            previousWebView.isHidden = true
        }

        // CRITICAL: Ensure capsule button stays on top
        bringUIElementsToFront()
    }

    private func switchPage(targetPath: String) {
        guard !appId.isEmpty else {
            os_log("Cannot switch page: appId not initialized", log: Self.log, type: .error)
            return
        }

        // Check if trying to navigate to current page
        if currentWebView?.currentPath == targetPath {
            return
        }

        // Store the target path as the last active path for state restoration
        LxAppCore.setLastActivePath(targetPath, for: appId)

        // Check if this is a tab page
        if let tabIndex = tabBar?.findTabIndexByPath(targetPath), tabIndex >= 0 {
            switchToTab(targetPath: targetPath)
        } else {
            // Handle non-tab page navigation

            // Determine if this is back navigation (simplistically by path length)
            let currentPath = currentWebView?.currentPath
            let isBackNavigation = currentPath != nil && currentPath!.count > targetPath.count

            navigateToPage(targetPath: targetPath, isReplace: false, isBackNavigation: isBackNavigation)
        }
    }

    private func navigateToPage(targetPath: String, isReplace: Bool = false, isBackNavigation: Bool = false) {
        // Get current WebView before changes
        let oldWebView = currentWebView

        // Find WebView for the target page
        guard let newWebView = iOSLxApp.findWebView(appId: appId, path: targetPath) else {
            os_log("WebView not found for path: %{public}@, should be created by Rust system", log: Self.log, type: .info, targetPath)
            return
        }

        // Update navigation bar configuration
        updateNavigationBar(appId: appId, path: targetPath, isBackNavigation: isBackNavigation, disableAnimation: false)

        // Remove from existing parent if any
        if newWebView.superview != nil {
            newWebView.removeFromSuperview()
        }

        // Make sure the new WebView is fully prepared before animation
        newWebView.isHidden = false
        newWebView.resumeWebView()

        // Create a new container for the WebView
        let newContainer = UIView()
        newContainer.translatesAutoresizingMaskIntoConstraints = false
        newContainer.addSubview(newWebView)

        // Setup WebView constraints within container
        newWebView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            newWebView.topAnchor.constraint(equalTo: newContainer.topAnchor),
            newWebView.leadingAnchor.constraint(equalTo: newContainer.leadingAnchor),
            newWebView.trailingAnchor.constraint(equalTo: newContainer.trailingAnchor),
            newWebView.bottomAnchor.constraint(equalTo: newContainer.bottomAnchor)
        ])

        // Get reference to old container
        let oldContainer = webViewContainer.subviews.first { $0.tag == Self.CURRENT_WEBVIEW_CONTAINER_TAG } // Use tag to identify current container
        oldContainer?.tag = Self.OLD_WEBVIEW_CONTAINER_TAG // Re-tag old container

        // Add new container
        newContainer.tag = Self.CURRENT_WEBVIEW_CONTAINER_TAG // Tag the new container
        webViewContainer.addSubview(newContainer)

        // Setup container constraints
        NSLayoutConstraint.activate([
            newContainer.topAnchor.constraint(equalTo: webViewContainer.topAnchor),
            newContainer.leadingAnchor.constraint(equalTo: webViewContainer.leadingAnchor),
            newContainer.trailingAnchor.constraint(equalTo: webViewContainer.trailingAnchor),
            newContainer.bottomAnchor.constraint(equalTo: webViewContainer.bottomAnchor)
        ])

        // Update layout margins to position the new container vertically
        updateLayoutMargins()

        // Set initial horizontal position for animation
        let startX: CGFloat = isBackNavigation ? -webViewContainer.frame.width : webViewContainer.frame.width
        newContainer.transform = CGAffineTransform(translationX: startX, y: 0)

        // Animation parameters
        let duration: TimeInterval = 0.25
        let endXOld: CGFloat = isBackNavigation ? webViewContainer.frame.width : -webViewContainer.frame.width

        // Update the current WebView reference BEFORE animating
        currentWebView = newWebView

        // Animate the new container in
        UIView.animate(withDuration: duration, animations: {
            newContainer.transform = .identity
        }) { _ in
            // Trigger onPageShow after animation completes
            if let appId = newWebView.appId, let currentPath = newWebView.currentPath {
                onPageShow(appId, currentPath)
                os_log("navigateToPage: Triggered onPageShow for appId=%@ path=%@",
                       log: Self.log, type: .info, appId, currentPath)
            }

            // CRITICAL: Ensure capsule button stays on top after animation
            self.bringUIElementsToFront()
        }

        // Animate the old container out
        if let oldContainer = oldContainer, let _ = oldWebView {
            UIView.animate(withDuration: duration, animations: {
                oldContainer.transform = CGAffineTransform(translationX: endXOld, y: 0)
            }) { _ in
                // Remove old container after animation (don't pause WebView - let Rust manage)
                if !self.isDestroyed {
                    oldContainer.removeFromSuperview()
                    // CRITICAL: Ensure capsule button stays on top
                    self.bringUIElementsToFront()
                }
            }
        }
    }

    private func updateNavigationBar(appId: String, path: String, isBackNavigation: Bool, disableAnimation: Bool = false) {
        let pageConfig = getPageConfig(appId: appId, path: path)

        // Determine NavigationBar visibility
        let shouldShowNavigationBar: Bool
        if let config = pageConfig {
            // Use explicit configuration from Rust
            shouldShowNavigationBar = !config.hidden
        } else {
            // No configuration available - use default behavior (show NavigationBar)
            // This provides a safe fallback when Rust doesn't have page config
            shouldShowNavigationBar = true
            os_log("updateNavigationBar: No page config found for appId=%@ path=%@, using default (show NavigationBar)",
                   log: Self.log, type: .info, appId, path)
        }

        if shouldShowNavigationBar {
            // Ensure NavigationBar exists
            ensureNavigationBarExists()

            guard let navigationBar = navigationBar else {
                os_log("updateNavigationBar: Failed to create NavigationBar", log: Self.log, type: .error)
                return
            }

            // Update NavigationBar with configuration
            let _ = navigationBar.updateWithConfig(
                pageConfig: pageConfig,
                isBackNavigation: isBackNavigation,
                disableAnimation: disableAnimation,
                onBackClickListener: { [weak self] in
                    self?.handleBackButtonClick()
                },
                onAnimationEnd: { [weak self] in
                    if isBackNavigation {
                        let currentPath = self?.currentWebView?.currentPath ?? ""
                        let isNowOnTabRoot = (self?.tabBar?.findTabIndexByPath(currentPath) ?? -1) != -1
                        if isNowOnTabRoot {
                            self?.navigationBar?.setBackButtonVisible(false)
                        }
                    }
                }
            )
        } else {
            // NavigationBar should be hidden - remove it completely
            removeNavigationBar()
            ensureTransparentNavigationArea()
        }
    }

    private func getPageConfig(appId: String, path: String) -> NavigationBarConfig? {
        let configJson = lingxia.getPageConfig(appId, path)?.toString()
        return NavigationBarConfig.fromJson(configJson)
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

        os_log("iOSLxAppViewController.ensureTransparentNavigationArea: Applied transparent background for hidden NavigationBar",
               log: Self.log, type: .info)
    }

    private func ensureNavigationBarExists() {
        guard navigationBar == nil else {
            return
        }
        let newNavBar = NavigationBar()
        let navBarContentHeight = newNavBar.getCalculatedContentHeight()

        newNavBar.translatesAutoresizingMaskIntoConstraints = false
        // Set external status bar height to our fixed value (NavigationBar will handle status bar area)
        newNavBar.setExternalStatusBarHeight(iOSLxAppViewController.STATUS_BAR_HEIGHT)

        newNavBar.backgroundColor = UIColor.white

        // Set back button click handler
        newNavBar.setOnBackButtonClickListener { [weak self] in
            self?.handleBackButtonClick()
        }

        navigationBar = newNavBar
        rootContainer.addSubview(newNavBar)

        // Position NavigationBar from screen top (full screen)
        NSLayoutConstraint.activate([
            newNavBar.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            newNavBar.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            newNavBar.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            newNavBar.heightAnchor.constraint(equalToConstant: navBarContentHeight + iOSLxAppViewController.STATUS_BAR_HEIGHT)
        ])

        rootContainer.bringSubviewToFront(newNavBar)
    }

    private func removeNavigationBar() {
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
    }

    private func handleBackButtonClick() {
        let result = onBackPressed(appId)
        if result {
            return
        }

        // No back navigation available, close activity (same as Android's finish())
        if !isDisplayingHomeLxApp {
            performLxAppClose()
        }
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
