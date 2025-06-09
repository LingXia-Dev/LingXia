import UIKit
import WebKit
import os.log
@preconcurrency import ObjectiveC

// Log instance outside of @MainActor to avoid isolation issues
private let miniAppViewControllerLog = OSLog(subsystem: "LingXia", category: "MiniAppView")

@MainActor
public class MiniAppViewController: UIViewController {
    private static let log = miniAppViewControllerLog

    public static let EXTRA_APP_ID = "appId"
    public static let EXTRA_PATH = "path"
    internal static let DEFAULT_NAV_BAR_HEIGHT: CGFloat = 44
    internal static let DEFAULT_TAB_BAR_SIZE: CGFloat = 64
    internal static let STATUS_BAR_HEIGHT: CGFloat = 48

    // MARK: - UI Element Tags
    private static let CAPSULE_BUTTON_TAG = 9999
    private static let CURRENT_WEBVIEW_CONTAINER_TAG = 999
    private static let OLD_WEBVIEW_CONTAINER_TAG = 998

    private var appId: String
    private var initialPath: String
    private var rootContainer: UIView!
    private var statusBarBackground: UIView!
    private var webViewContainer: UIView!
    private var tabBar: LingXiaTabBar?
    private var navigationBar: LingXiaNavigationBar?
    private var isDestroyed = false
    private var pendingWebViewSetup = false
    private var isDisplayingHomeMiniApp: Bool = false

    private var currentWebView: LingXiaWebView?

    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var switchPageObserver: NSObjectProtocol?



    private static func dummyNativeOnBackPressed(appId: String) -> Int32 {
        os_log("[DUMMY] Back pressed for: %{public}@", log: miniAppViewControllerLog, type: .debug, appId)
        return 0
    }

    /// Configures the system bars to be transparent and edge-to-edge
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

    /// Configures true edge-to-edge display with transparent status bar
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
        self.isDisplayingHomeMiniApp = (appId == MiniApp.homeMiniAppId)
        self.initialPath = path
        super.init(nibName: nil, bundle: nil)

        MiniAppViewController.configureTransparentSystemBars(viewController: self)
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

        let tabBarJson = lingxia.getTabBarConfig(appId)?.toString()
        let tabBarConfig = TabBarConfig.fromJson(tabBarJson)

        setupRootContainer()
        setupWebViewContainer()

        if let tabBarConfig = tabBarConfig {
            setupTabBar(config: tabBarConfig)

            let isTabBarTransparent = TabBarConfig.isTransparent(tabBarConfig.backgroundColor)
            os_log("MiniAppViewController.viewDidLoad: isTabBarTransparent=%@ backgroundColor=%@",
                   log: Self.log, type: .debug,
                   String(isTabBarTransparent), tabBarConfig.backgroundColor?.description ?? "nil")
        }

        addCapsuleButton()
        setupInitialContent(path: initialPath)

        // Sync TabBar selected state with current path
        if let tabBar = tabBar {
            tabBar.syncSelectedTabWithCurrentPath(initialPath)
        }

        // Force complete transparency for TabBar scenarios
        if let tabBar = tabBar, TabBarConfig.isTransparent(tabBar.config.backgroundColor) {
            forceCompleteTransparency()
        }
    }

    public override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)

        // Force window background to be clear to prevent black safe area background
        forceTransparentBackground()

        // Start continuous monitoring to prevent background from turning black
        startBackgroundMonitoring()
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        forceTransparentBackground()
    }

    public override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()

        // Force transparent background on every layout
        forceTransparentBackground()

        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)

            DispatchQueue.main.async {
                tabBar.setNeedsLayout()
                tabBar.layoutIfNeeded()
            }
        }
    }

    private func forceTransparentBackground() {
        // Force window background to be clear
        if let window = view.window {
            window.backgroundColor = UIColor.clear
            window.isOpaque = false
        }

        // Also try to set the scene background if available
        if #available(iOS 13.0, *) {
            if let windowScene = view.window?.windowScene {
                for window in windowScene.windows {
                    window.backgroundColor = UIColor.clear
                    window.isOpaque = false
                }
            }
        }

        // Force all view hierarchy to be transparent
        view.backgroundColor = UIColor.clear
        view.isOpaque = false

        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.clear
        }
    }

    private func startBackgroundMonitoring() {
        // Use repeated async calls instead of timer to avoid concurrency issues
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            guard let self = self, !self.isDestroyed else { return }

            self.forceTransparentBackground()
            self.startBackgroundMonitoring() // Recursive call
        }
    }

    private func setupNotificationObservers() {
        closeAppObserver = NotificationCenter.default.addObserver(
            forName: NSNotification.Name(ACTION_CLOSE_MINIAPP),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let targetAppId = userInfo["appId"] as? String else { return }

            Task { @MainActor in
                guard targetAppId == self.appId else { return }

                os_log("Received close request for appId: %@", log: Self.log, type: .info, self.appId)
                self.dismiss(animated: true)
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
            statusBarBackground.heightAnchor.constraint(equalToConstant: MiniAppViewController.STATUS_BAR_HEIGHT)
        ])
    }

    private func setupWebViewContainer() {
        webViewContainer = UIView()
        // Use clean white background for better loading experience
        webViewContainer.backgroundColor = UIColor.white
        webViewContainer.translatesAutoresizingMaskIntoConstraints = false
        rootContainer.addSubview(webViewContainer)

        NSLayoutConstraint.activate([
            webViewContainer.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            webViewContainer.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            webViewContainer.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            webViewContainer.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
        ])

        // Add a subtle loading indicator
        let loadingIndicator = UIActivityIndicatorView(style: .medium)
        loadingIndicator.color = UIColor.systemGray
        loadingIndicator.translatesAutoresizingMaskIntoConstraints = false
        loadingIndicator.startAnimating()
        loadingIndicator.tag = 9997 // Special tag for removal later
        webViewContainer.addSubview(loadingIndicator)

        NSLayoutConstraint.activate([
            loadingIndicator.centerXAnchor.constraint(equalTo: webViewContainer.centerXAnchor),
            loadingIndicator.centerYAnchor.constraint(equalTo: webViewContainer.centerYAnchor)
        ])
    }

    private func setupInitialContent(path: String) {
        // NEW APPROACH: Register for WebView creation notification instead of polling
        // This eliminates the retry mechanism and makes the flow event-driven

        // Capture appId to avoid MainActor issues in closure
        let currentAppId = appId

        // Use a weak reference to avoid circular reference
        var observer: NSObjectProtocol?
        observer = NotificationCenter.default.addObserver(
            forName: NSNotification.Name("WebViewCreated"),
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let notificationAppId = userInfo["appId"] as? String,
                  let notificationPath = userInfo["path"] as? String,
                  notificationAppId == currentAppId,
                  notificationPath == path else {
                return
            }

            // Remove observer immediately to prevent multiple calls
            if let observer = observer {
                NotificationCenter.default.removeObserver(observer)
            }

            // Use Task to handle MainActor calls
            Task { @MainActor in
                // Find the WebView that was just created
                guard let webView = self.findWebView(appId: currentAppId, path: path) else {
                    os_log("WebView creation notification received but WebView not found for path: %{public}@", log: Self.log, type: .error, path)
                    return
                }

                self.attachWebViewToUI(webView: webView)
            }
        }
    }

    /// Attaches WebView to UI and triggers the attached event at the right time
    private func attachWebViewToUI(webView: LingXiaWebView) {
        // Add webview to container first (while hidden)
        webViewContainer.addSubview(webView)
        webView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: webViewContainer.topAnchor),
            webView.leadingAnchor.constraint(equalTo: webViewContainer.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: webViewContainer.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: webViewContainer.bottomAnchor)
        ])

        // Set as current webview
        currentWebView = webView

        // Force layout to ensure WebView has correct frame
        webViewContainer.layoutIfNeeded()

        // NOW trigger attached event - WebView is properly attached and laid out
        let attachResult = webView.dummyNativeOnWebViewAttached(appId: appId, path: webView.currentPath ?? "")
        os_log("WebView attached event triggered with result: %d", log: Self.log, type: .info, attachResult)

        // Start loading content
        webView.resumeWebView()

        // Show webview after brief delay to ensure rendering is ready
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            webView.isHidden = false

            // Remove loading indicator after a reasonable delay
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                if let loadingIndicator = self.webViewContainer.viewWithTag(9997) {
                    loadingIndicator.removeFromSuperview()
                }
            }
        }

        // Get page configuration and update NavigationBar
        let pageConfig = webView.getPageConfig()
        updateNavigationBar(pageConfig: pageConfig, isBackNavigation: false, disableAnimation: true)
    }

    private func setupTabBar(config: TabBarConfig?) {
        guard let config = config else {
            os_log("Invalid or insufficient TabBar config", log: Self.log, type: .error)
            return
        }

        let isTabBarTransparent = TabBarConfig.isTransparent(config.backgroundColor)

        // Update system navigation bar transparency based on TabBar transparency and color
        MiniAppViewController.updateNavigationBarTransparency(
            viewController: self,
            isTabBarTransparent: isTabBarTransparent,
            tabBarBackgroundColor: config.backgroundColor
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
            }
        } else {
            tabBar?.setConfig(config: config)
            if let tabBar = tabBar {
                // TabBar is already added to view hierarchy, just update layout
                applyTabBarLayoutParams(tabBar: tabBar, config: config)
            }
        }

        // Configure the entire app layout for transparency mode
        configureTabBarTransparencyMode(isTabBarTransparent)

        // Always update layout margins after TabBar setup
        updateLayoutMargins()
    }

    private func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBarConfig) {
        let isVertical = config.position == .left || config.position == .right
        let defaultTabBarSize = MiniAppViewController.DEFAULT_TAB_BAR_SIZE

        tabBar.translatesAutoresizingMaskIntoConstraints = false

        if isVertical {
            NSLayoutConstraint.activate([
                tabBar.widthAnchor.constraint(equalToConstant: defaultTabBarSize),
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: MiniAppViewController.STATUS_BAR_HEIGHT),
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
                tabBar.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: MiniAppViewController.STATUS_BAR_HEIGHT).isActive = true
            } else {
                // For bottom position, always extend to view.bottomAnchor to cover safe area
                // Both transparent and opaque TabBars extend to actual screen bottom
                // The difference is handled internally by the TabBar component
                tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor).isActive = true
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

    /// Brings UI elements (TabBar, capsule button) to front
    private func bringUIElementsToFront() {
        if let tabBar = tabBar {
            rootContainer.bringSubviewToFront(tabBar)
        }

        // CRITICAL: Find capsule button by tag and bring to front
        if let capsuleButton = rootContainer.viewWithTag(Self.CAPSULE_BUTTON_TAG) {
            rootContainer.bringSubviewToFront(capsuleButton)
        }
    }

    /// Calculates the Y translation needed for WebView based on navigation bar
    /// - Returns: Translation Y value in points
    private func calculateWebViewTranslationY() -> CGFloat {
        guard let navigationBar = navigationBar, !navigationBar.isHidden else {
            return 0
        }

        // Use fixed status bar height
        let navBarContentHeight = navigationBar.getCalculatedContentHeight()

        return MiniAppViewController.STATUS_BAR_HEIGHT + navBarContentHeight
    }

    private func addCapsuleButton() {
        // Don't show capsule button for the main/home app
        if isDisplayingHomeMiniApp {
            return
        }

        // Create capsule container (matching Android dimensions and styling)
        let capsule = UIView()
        capsule.backgroundColor = UIColor.white
        capsule.layer.cornerRadius = 20 // Android: 20f * density
        capsule.layer.borderWidth = 0.5 // Android: 0.5f * density
        capsule.layer.borderColor = UIColor(red: 0.867, green: 0.867, blue: 0.867, alpha: 1.0).cgColor // #DDDDDD
        capsule.layer.shadowColor = UIColor.black.cgColor
        capsule.layer.shadowOffset = CGSize(width: 0, height: 1)
        capsule.layer.shadowOpacity = 0.1
        capsule.layer.shadowRadius = 2
        capsule.layer.zPosition = 1000 // CRITICAL: Set highest z-position to ensure always on top
        capsule.tag = Self.CAPSULE_BUTTON_TAG // Special tag for identification and management
        capsule.translatesAutoresizingMaskIntoConstraints = false

        // Create more button with custom dots (matching Android MoreDotsDrawable)
        let btnMore = UIButton(type: .custom)
        btnMore.backgroundColor = UIColor.clear
        btnMore.setImage(createMoreDotsImage(), for: .normal)
        btnMore.addTarget(self, action: #selector(moreButtonTapped), for: .touchUpInside)

        // Create divider (matching Android implementation)
        let divider = UIView()
        divider.backgroundColor = UIColor(red: 0.867, green: 0.867, blue: 0.867, alpha: 1.0) // #DDDDDD
        divider.translatesAutoresizingMaskIntoConstraints = false

        // Create close button with custom X (matching Android CloseButtonDrawable)
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
            // Capsule positioning (using fixed status bar height + 8pt margin)
            capsule.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: MiniAppViewController.STATUS_BAR_HEIGHT + 8),
            capsule.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor, constant: -12),
            capsule.heightAnchor.constraint(equalToConstant: 36), // Android: 36dp

            // More button (Android: 44dp width)
            btnMore.leadingAnchor.constraint(equalTo: capsule.leadingAnchor, constant: 2), // Android padding
            btnMore.topAnchor.constraint(equalTo: capsule.topAnchor),
            btnMore.bottomAnchor.constraint(equalTo: capsule.bottomAnchor),
            btnMore.widthAnchor.constraint(equalToConstant: 44),

            // Divider (Android: 1dp width, 20dp height)
            divider.leadingAnchor.constraint(equalTo: btnMore.trailingAnchor),
            divider.centerYAnchor.constraint(equalTo: capsule.centerYAnchor),
            divider.widthAnchor.constraint(equalToConstant: 1),
            divider.heightAnchor.constraint(equalToConstant: 20),

            // Close button (Android: 44dp width)
            btnClose.leadingAnchor.constraint(equalTo: divider.trailingAnchor),
            btnClose.trailingAnchor.constraint(equalTo: capsule.trailingAnchor, constant: -2), // Android padding
            btnClose.topAnchor.constraint(equalTo: capsule.topAnchor),
            btnClose.bottomAnchor.constraint(equalTo: capsule.bottomAnchor),
            btnClose.widthAnchor.constraint(equalToConstant: 44)
        ])
    }

    /// Creates the more dots image (matching Android MoreDotsDrawable exactly)
    private func createMoreDotsImage() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)

        guard let context = UIGraphicsGetCurrentContext() else { return nil }

        // Enable anti-aliasing for smoother drawing
        context.setShouldAntialias(true)
        context.setAllowsAntialiasing(true)

        // Set color to match Android (black)
        UIColor.black.setFill()

        let centerY = size.height / 2
        let centerX = size.width / 2

        // Match Android dimensions exactly:
        // Center dot is larger, side dots are smaller
        let centerDotRadius = size.height / 7  // Larger center dot (Android: bounds.height() / 7f)
        let sideDotRadius = size.height / 10   // Smaller side dots (Android: bounds.height() / 10f)
        let spacing = centerDotRadius * 2.8    // Adjusted spacing (Android: centerDotRadius * 2.8f)

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

    /// Creates the close button image (matching Android CloseButtonDrawable exactly)
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
        dismiss(animated: true)
    }

    private func switchToTab(targetPath: String) {
        // Bail early if trying to switch to the current path
        if currentWebView?.currentPath == targetPath {
            return
        }

        // Capture reference to previous WebView before changing anything
        let previousWebView = currentWebView

        // Find target tab index
        guard let targetIndex = tabBar?.findTabIndexByPath(targetPath), targetIndex >= 0 else {
            os_log("switchToTab failed: Path '%{public}@' not found in TabBar items.", log: Self.log, type: .error, targetPath)
            return
        }

        // Find target WebView
        guard let targetWebView = findWebView(appId: appId, path: targetPath) else {
            os_log("switchToTab failed: WebView not found for %{public}@, should be created by Rust system", log: Self.log, type: .error, targetPath)
            return
        }

        // Set current WebView to target for tracking
        currentWebView = targetWebView

        // Update TabBar UI (without triggering listener)
        tabBar?.setSelectedIndex(targetIndex, notifyListener: false)

        // ZERO LAYOUT CHANGES - only show/hide NavigationBar without relayout
        let pageConfig = targetWebView.getPageConfig()
        let shouldShowNavigationBar = !(pageConfig?.hidden ?? false)

        if shouldShowNavigationBar {
            ensureNavigationBarExists()
            navigationBar?.isHidden = false
            configureNavigationBar(pageConfig: pageConfig, isBackNavigation: false, disableAnimation: true)
        } else {
            navigationBar?.isHidden = true
        }

        // Add target view if it's not already there
        if targetWebView.superview != webViewContainer {
            if targetWebView.superview != nil {
                targetWebView.removeFromSuperview()
            }

            webViewContainer.addSubview(targetWebView)
            targetWebView.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                targetWebView.topAnchor.constraint(equalTo: webViewContainer.topAnchor),
                targetWebView.leadingAnchor.constraint(equalTo: webViewContainer.leadingAnchor),
                targetWebView.trailingAnchor.constraint(equalTo: webViewContainer.trailingAnchor),
                targetWebView.bottomAnchor.constraint(equalTo: webViewContainer.bottomAnchor)
            ])
        } else {
            webViewContainer.bringSubviewToFront(targetWebView)
        }

        // Immediate switch without animation or layout changes
        targetWebView.isHidden = false
        targetWebView.resumeWebView()

        // Hide previous WebView
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

    /// Navigate to a non-tab page with animation
    /// - Parameters:
    ///   - targetPath: Path of the page to navigate to
    ///   - isReplace: Whether this replaces the current page
    ///   - isBackNavigation: Whether this is a back navigation
    private func navigateToPage(targetPath: String, isReplace: Bool = false, isBackNavigation: Bool = false) {
        // Get current WebView before changes
        let oldWebView = currentWebView

        // Find WebView for the target page
        guard let newWebView = findWebView(appId: appId, path: targetPath) else {
            os_log("WebView not found for path: %{public}@, should be created by Rust system", log: Self.log, type: .info, targetPath)
            return
        }

        let pageConfig = newWebView.getPageConfig()

        // Update navigation bar configuration
        updateNavigationBar(pageConfig: pageConfig, isBackNavigation: isBackNavigation, disableAnimation: false)

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
            // CRITICAL: Ensure capsule button stays on top after animation
            self.bringUIElementsToFront()
        }

        // Animate the old container out
        if let oldContainer = oldContainer, let oldWebView = oldWebView {
            UIView.animate(withDuration: duration, animations: {
                oldContainer.transform = CGAffineTransform(translationX: endXOld, y: 0)
            }) { _ in
                // Remove old container after animation
                if !self.isDestroyed {
                    oldWebView.pauseWebView()
                    oldContainer.removeFromSuperview()
                    // CRITICAL: Ensure capsule button stays on top
                    self.bringUIElementsToFront()
                }
            }
        }
    }

    /// Updates the navigation bar based on page configuration
    /// - Parameters:
    ///   - pageConfig: The navigation bar configuration for the target page
    ///   - isBackNavigation: Whether this is a back navigation
    ///   - disableAnimation: Whether to disable animation
    private func updateNavigationBar(pageConfig: NavigationBarConfig?, isBackNavigation: Bool, disableAnimation: Bool = false) {
        let shouldShowNavigationBar = !(pageConfig?.hidden ?? false)

        if shouldShowNavigationBar {
            // Need to show NavigationBar
            ensureNavigationBarExists()
            configureNavigationBar(pageConfig: pageConfig, isBackNavigation: isBackNavigation, disableAnimation: disableAnimation)
        } else {
            // Need to hide NavigationBar and ensure transparent background
            removeNavigationBar()
            ensureTransparentNavigationArea()
        }
    }

    /// Ensures the navigation area is transparent when NavigationBar is hidden
    private func ensureTransparentNavigationArea() {
        // When NavigationBar is hidden, ensure the entire status bar and navigation area is transparent
        // Force transparent backgrounds in the view hierarchy
        view.backgroundColor = UIColor.clear
        rootContainer.backgroundColor = UIColor.clear

        os_log("MiniAppViewController.ensureTransparentNavigationArea: Applied transparent background for hidden NavigationBar",
               log: Self.log, type: .info)
    }

    /// Ensures NavigationBar exists and is properly set up
    private func ensureNavigationBarExists() {
        guard navigationBar == nil else {
            return
        }
        let newNavBar = LingXiaNavigationBar()
        let navBarContentHeight = newNavBar.getCalculatedContentHeight()

        newNavBar.translatesAutoresizingMaskIntoConstraints = false
        // Set external status bar height to our fixed value (NavigationBar will handle status bar area)
        newNavBar.setExternalStatusBarHeight(MiniAppViewController.STATUS_BAR_HEIGHT)

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
            newNavBar.heightAnchor.constraint(equalToConstant: navBarContentHeight + MiniAppViewController.STATUS_BAR_HEIGHT)
        ])

        rootContainer.bringSubviewToFront(newNavBar)
    }

    /// Configures the existing NavigationBar with page settings
    private func configureNavigationBar(pageConfig: NavigationBarConfig?, isBackNavigation: Bool, disableAnimation: Bool) {
        guard let navigationBar = navigationBar else {
            os_log("LingXia: ERROR: Trying to configure non-existent NavigationBar", log: Self.log, type: .error)
            return
        }

        let titleText = pageConfig?.navigationBarTitleText ?? ""
        let backgroundColor = pageConfig?.navigationBarBackgroundColor ?? NavigationBarConfig.DEFAULT_BACKGROUND_COLOR
        let textStyle = pageConfig?.navigationBarTextStyle ?? "black"
        let textColor = textStyle == "white" ? UIColor.white : UIColor.black
        let showBackButton = !disableAnimation

        let onAnimationEnd = { [weak self] in
            if isBackNavigation {
                let currentPath = self?.currentWebView?.currentPath ?? ""
                let isNowOnTabRoot = (self?.tabBar?.findTabIndexByPath(currentPath) ?? -1) != -1
                if isNowOnTabRoot {
                    self?.navigationBar?.setBackButtonVisible(false)
                }
            }
        }

        navigationBar.updateStateAndAnimate(
            title: titleText,
            bgColor: backgroundColor,
            textColor: textColor,
            showBackButton: showBackButton,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            onBackClickListener: { [weak self] in
                self?.handleBackButtonClick()
            },
            onAnimationEnd: onAnimationEnd
        )
    }

    /// Removes NavigationBar completely
    private func removeNavigationBar() {
        guard let navigationBar = navigationBar else {
            return
        }

        navigationBar.removeFromSuperview()
        self.navigationBar = nil
    }

    /// Handles back button click events
    private func handleBackButtonClick() {
        let result = MiniAppViewController.dummyNativeOnBackPressed(appId: appId)
        os_log("[DUMMY] Back press handled by native: %d", log: Self.log, type: .debug, result)

        if result > 0 {
            return
        }

        // No back navigation available, close activity
        dismiss(animated: true)
    }

    public override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)

        if isBeingDismissed {
            cleanupResources()
        }
    }

    /// Cleanly removes all resources and observers to prevent memory leaks
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

        // Pause current WebView
        currentWebView?.pauseWebView()

        // Notify Rust layer that mini app is being closed
        let _ = lingxia.onMiniappClosed(appId)

        // Mark as destroyed to prevent further operations
        isDestroyed = true
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

            // Schedule UI cleanup on main thread without capturing self
            let appIdCopy = appId
            DispatchQueue.main.async {
                // Notify Rust layer about cleanup without retaining self
                let _ = lingxia.onMiniappClosed(appIdCopy)
            }
        }

        os_log("MiniAppViewController: MiniAppViewController deinitialized", log: miniAppViewControllerLog, type: .debug)
    }



    /// Forces complete transparency across the entire view hierarchy
    private func forceCompleteTransparency() {
        os_log("MiniAppViewController.forceCompleteTransparency: Applying complete transparency",
               log: Self.log, type: .info)

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

        // Force webview container
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
        }

        os_log("MiniAppViewController.forceCompleteTransparency: Complete transparency applied",
               log: Self.log, type: .info)
    }

    // MARK: - TabBar Transparency Management

    /// Configures the entire app layout based on TabBar transparency state
    /// This function intelligently adapts WebView behavior, safe area handling, and positioning
    /// - Parameter isTransparent: Whether the TabBar should be transparent
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

        // For transparent mode, add additional enforcement to ensure TabBar stays transparent
        if isTransparent {
            DispatchQueue.main.async { [weak self] in
                self?.enforceTabBarTransparency()

                // Additional transparency enforcement after a short delay
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                    self?.enforceTabBarTransparency()
                }
            }
        }
    }

    /// Configures background colors based on TabBar transparency mode
    /// - Parameter isTransparent: Whether the TabBar should be transparent
    private func configureBackgroundColors(_ isTransparent: Bool) {
        if isTransparent {
            // Apply complete transparency for transparent mode
            forceCompleteTransparency()
        } else {
            // Apply appropriate opaque backgrounds for non-transparent mode
            forceOpaqueBackgrounds()
        }
    }

    /// Sets opaque white backgrounds for non-transparent TabBar mode
    private func forceOpaqueBackgrounds() {
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

        // Set webview container to white
        if let webViewContainer = webViewContainer {
            webViewContainer.backgroundColor = UIColor.white
            webViewContainer.isOpaque = true
            webViewContainer.layer.backgroundColor = UIColor.white.cgColor
        }
    }

    /// Configures WebView's edge-to-edge content behavior based on TabBar transparency
    /// - Parameter isTransparent: Whether content should extend to screen edges
    private func configureWebViewEdgeToEdgeBehavior(_ isTransparent: Bool) {
        guard let currentWebView = currentWebView else { return }

        if isTransparent {
            // For transparent TabBar: allow content to extend to all edges
            currentWebView.scrollView.contentInsetAdjustmentBehavior = .never
        } else {
            // For opaque TabBar: respect safe areas to avoid content being hidden
            currentWebView.scrollView.contentInsetAdjustmentBehavior = .automatic
        }
    }

    /// Configures TabBar overlay positioning and z-order
    /// - Parameter isTransparent: Whether TabBar should overlay content
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

    /// Updates WebView container constraints based on TabBar transparency
    /// - Parameter isTransparent: Whether content should extend behind TabBar
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

    /// Calculates the appropriate top anchor for WebView container
    /// - Returns: Tuple of (anchor, constant) for top positioning
    private func calculateTopAnchor() -> (NSLayoutYAxisAnchor, CGFloat) {
        let isTopTabBar = tabBar?.config.position == .top
        let hasNavigationBar = navigationBar != nil

        if isTopTabBar {
            return (tabBar?.bottomAnchor ?? rootContainer.topAnchor, 0)
        } else if hasNavigationBar {
            return (navigationBar!.bottomAnchor, 0)
        } else {
            // For bottom TabBar or no TabBar, WebView should start from the very top for edge-to-edge display
            // Use rootContainer.topAnchor instead of safeAreaLayoutGuide.topAnchor to allow true full-screen content
            return (rootContainer.topAnchor, 0)
        }
    }

    /// Calculates the appropriate bottom anchor for WebView container
    /// - Parameter isTransparent: Whether TabBar is transparent
    /// - Returns: Bottom anchor for WebView positioning
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

    /// Aggressively enforces TabBar transparency to prevent any background color override
    private func enforceTabBarTransparency() {
        guard let tabBar = tabBar else { return }

        // Use the TabBar's built-in transparency enforcement
        tabBar.forceTransparencyMode()
    }

    /// Finds existing WebView from Rust
    /// - Parameters:
    ///   - appId: The miniapp ID
    ///   - path: The page path of miniapp
    /// - Returns: LingXiaWebView instance or nil if not found
    @MainActor
    private func findWebView(appId: String, path: String) -> LingXiaWebView? {
        // Find existing WebView from Rust
        let webViewPtr = lingxia.findWebView(appId, path)

        if webViewPtr != 0 {
            // WebView exists in Rust, restore from pointer
            let pointer = UnsafeRawPointer(bitPattern: webViewPtr)!
            return Unmanaged<LingXiaWebView>.fromOpaque(pointer).takeUnretainedValue()
        } else {
            // WebView doesn't exist in Rust
            return nil
        }
    }
}
