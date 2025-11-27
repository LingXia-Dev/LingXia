#if os(iOS)
import UIKit
import SwiftUI
import WebKit
import os.log
import Combine
import CLingXiaRustAPI
@preconcurrency import ObjectiveC

// Log instance outside of @MainActor to avoid isolation issues
private let lxAppViewControllerLog = OSLog(subsystem: "LingXia", category: "LxAppViewController")

@MainActor
public class LxAppViewController: UIViewController, ObservableObject {
    private static let log = lxAppViewControllerLog

    // Platform-specific UI constraint only - WebView is managed by WebViewManager
    private var currentWebViewTopConstraint: NSLayoutConstraint?

    internal var rootContainer: UIView!
    private var webViewContainer: UIView!
    private var globalCapsuleButton: UIView?
    public var globalNavigationBar: LingXiaNavigationBar?
    public var currentTabBar: LingXiaTabBar?
    private var cancellables = Set<AnyCancellable>()
    private var backEdgePanGesture: UIScreenEdgePanGestureRecognizer?

    // Store pending navigation state for deferred NavigationBar initialization
    private var pendingNavigationState: (appId: String, path: String)?
    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?
    nonisolated(unsafe) private var tabBarObserver: NSObjectProtocol?

    private func getCurrentWebView() -> WKWebView? {
        return LxAppCore.getCurrentWebView()
    }

    private var statusBarHeight: CGFloat {
        return LxAppTheme.getStatusBarHeight()
    }

    private var navigationAreaHeight: CGFloat {
        guard let currentAppId = LxAppCore.currentAppId else {
            return 0
        }

        if shouldUseTransparentMode(for: currentAppId) {
            return 0
        } else {
            return statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
        }
    }

    private func shouldUseTransparentMode(for appId: String, path: String? = nil) -> Bool {
        guard !appId.isEmpty, isViewLoaded else { return false }

        let currentPath = path ?? getCurrentPath()
        guard !currentPath.isEmpty else { return false }

        guard LxAppCore.isInitialized(), !appId.isEmpty, !currentPath.isEmpty else {
            return false
        }

        let navState = lingxia.getNavigationBarState(appId, currentPath)
        return !navState.show_navbar
    }

    public override init(nibName nibNameOrNil: String?, bundle nibBundleOrNil: Bundle?) {
        super.init(nibName: nibNameOrNil, bundle: nibBundleOrNil)
        setupNotificationObservers()
        configureSystemNavigationBar()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    public override func viewDidLoad() {
        super.viewDidLoad()

        configureEdgeToEdgeDisplay()

        //  Allow animation to extend beyond main view bounds
        view.clipsToBounds = false

        // Set initial background to prevent black flash
        view.backgroundColor = UIColor.black

        setupUI()
    }

    public override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)

        // Apply transparency and styling for current app
        if let currentAppId = LxAppCore.currentAppId {
            // CRITICAL FIX: Ensure NavigationBarStateManager is updated before applying styling
            let currentPath = LxAppCore.getCurrentPath()
            NavigationBarStateManager.shared.updateState(appId: currentAppId, path: currentPath)
            applyAppStyling(for: currentAppId)
        }
    }

    private func configureEdgeToEdgeDisplay() {
        modalPresentationStyle = .fullScreen
        edgesForExtendedLayout = [.top, .bottom, .left, .right]
        extendedLayoutIncludesOpaqueBars = true
        additionalSafeAreaInsets = .zero
    }

    private func setupUI() {
        if let navController = navigationController {
            navController.setNavigationBarHidden(true, animated: false)
        }

        // Set initial background - use black to prevent white flash
        view.backgroundColor = UIColor.black
        view.isOpaque = true

        setupRootContainer()
        setupWebViewContainer()
        setupGlobalNavigationBar()
        setupBackGestureRecognizer()
    }

    private func setupRootContainer() {
        rootContainer = UIView()
        rootContainer.backgroundColor = UIColor.white
        rootContainer.translatesAutoresizingMaskIntoConstraints = false
        rootContainer.clipsToBounds = false  // 🎬 Allow animation to extend beyond bounds
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
        // Start with transparent background to avoid black/white flash
        // Background will be set appropriately by applyAppStyling based on page requirements
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

    private func setupBackGestureRecognizer() {
        // Mirror Android's back press by listening for a left-edge swipe.
        let edgePan = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(handleBackEdgePan(_:)))
        edgePan.edges = .left
        edgePan.delegate = self
        edgePan.requiresExclusiveTouchType = false
        edgePan.name = "LxAppBackEdgePan"
        view.addGestureRecognizer(edgePan)
        backEdgePanGesture = edgePan
    }

    private func configureSystemNavigationBar() {
        if let navController = navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    /// Unified navigation entry point - handles all animation types
    public func navigate(appId: String, to path: String, with animationType: AnimationType) {
        os_log("Navigate: %@ to %@ with type: %@", log: Self.log, type: .info, appId, path, String(describing: animationType))

        // Ensure view is loaded before navigation
        if !isViewLoaded {
            DispatchQueue.main.async { [weak self] in
                self?.navigate(appId: appId, to: path, with: animationType)
            }
            return
        }

        // Set current app ID immediately to ensure all subsequent logic
        // operates on the correct app context.
        LxAppCore.setCurrentApp(appId: appId, path: path)

        // Update NavigationBar state and UI
        updateNavigationBar(appId: appId, path: path)

        // Update all UI components based on current navigation state
        updateCapsuleButton(for: appId)
        updateTabBar(for: appId, path: path)

        // Apply app styling to handle transparency and backgrounds
        applyAppStyling(for: appId, path: path)

        // Setup or switch WebView
        handleNavigation(appId: appId, path: path, animationType: animationType)

        // Update status bar style
        setNeedsStatusBarAppearanceUpdate()

        // Ensure UI elements are properly layered
        bringUIElementsToFront()
    }

    /// Opens a LxApp - creates new state if needed, switches if already exists
    public func openLxApp(appId: String, path: String) {
        os_log("Opening LxApp: %@ at path: %@", log: Self.log, type: .info, appId, path)

        // Set current app state
        LxAppCore.setCurrentApp(appId: appId, path: path)

        // Use unified navigation entry point
        navigate(appId: appId, to: path, with: .none)
    }

    /// Closes a LxApp and removes its state
    public func closeLxApp(appId: String) {
        os_log("Closing LxApp: %@", log: Self.log, type: .info, appId)

        guard LxAppCore.currentAppId == appId else {
            os_log("LxApp %@ not current app for closing", log: Self.log, type: .error, appId)
            return
        }

        // Hide the app if it's currently active
        if LxAppCore.currentAppId == appId {
            hideCurrentLxApp()
        }

        // Clean up app state
        cleanupLxAppState(appId: appId)

        // Clear WebView constraint only, WebView is managed by WebViewManager
        currentWebViewTopConstraint = nil

        // Call FFI close handler first
        let _ = onLxappClosed(appId)

        // Get next LxApp from Rust stack and open it
        let currentLxApp = getCurrentLxApp()
        let appidStr = currentLxApp.appid.toString()
        let pathStr = currentLxApp.path.toString()
        if !appidStr.isEmpty {
            os_log("Opening next LxApp from stack: %@:%@", log: Self.log, type: .info, appidStr, pathStr)
            // Use openLxApp instead of navigate since this is opening a new LxApp
            iOSLxApp.openLxApp(appId: appidStr, path: pathStr)
        } else {
            os_log("No more LxApps in stack, view controller will remain empty", log: Self.log, type: .info)
        }
    }

    public func handleNavigation(appId: String, path: String, animationType: AnimationType) {
        guard LxAppCore.currentAppId == appId else { return }

        let currentPath = getCurrentPath()

        if let existingWebView = getCurrentWebView(),
           currentPath != path {
            SameLevelBridge.notifyPageInactive(for: existingWebView)
        }

        if let targetWebView = iOSLxApp.findWebView(appId: appId, path: path) {

            // Handle navigation animations for all cases
            if let existingWebView = getCurrentWebView() {

                // Choose animation based on animation type
                switch animationType {
                case .forward, .backward:
                    // Forward/backward use slide animation
                    performSlideTransition(from: existingWebView, to: targetWebView, animationType: animationType, appId: appId, path: path)
                    return // Early return as performSlideTransition handles the rest
                case .none:
                    // No animation - immediate transition
                    existingWebView.isHidden = true
                    existingWebView.pauseWebView()
                }
            }

            // Show target WebView using shared logic
            attachWebViewToUI(webView: targetWebView, for: appId, path: path)

        }

        // Update WebView constraints if needed
        updateWebViewConstraints(for: appId)

        // Ensure UI elements are properly layered above WebView content
        bringUIElementsToFront()
    }

    private func updateCapsuleButton(for appId: String) {
        let shouldShow = !LxAppCore.isHomeLxApp(appId)

        if shouldShow {
            // Create capsule button if it doesn't exist
            if globalCapsuleButton == nil {
                LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
                globalCapsuleButton = view.viewWithTag(9999) // CAPSULE_BUTTON_TAG
            }
            globalCapsuleButton?.isHidden = false
        } else {
            // Hide capsule button for home app
            globalCapsuleButton?.isHidden = true
        }
    }

    private func updateTabBar(for appId: String, path: String) {
        // Tear down the existing tab bar if it belongs to a different mini app
        if let tabBar = currentTabBar, tabBar.appId != appId {
            tabBar.removeFromSuperview()
            currentTabBar = nil
        }

        // If TabBar doesn't exist, create it with fresh config.
        if currentTabBar == nil {
            guard let tabConfig = lingxia.getTabBar(appId) else {
                return
            }
            currentTabBar = createTabBar(config: tabConfig, appId: appId)
            return
        }

        // Existing tab bar already matches the current mini app; refresh its state from Rust.
        currentTabBar?.refreshLayout()
    }

    private func hideCurrentLxApp() {
        guard LxAppCore.currentAppId != nil else { return }

        // Hide WebView
        getCurrentWebView()?.isHidden = true
        getCurrentWebView()?.pauseWebView()

        currentTabBar?.isHidden = true
        globalNavigationBar?.isHidden = true
        globalCapsuleButton?.isHidden = true
    }

    @objc
    private func handleBackEdgePan(_ gesture: UIScreenEdgePanGestureRecognizer) {
        guard let appId = LxAppCore.currentAppId else { return }

        switch gesture.state {
        case .ended, .recognized:
            let translation = gesture.translation(in: view).x
            let velocity = gesture.velocity(in: view).x
            let translationThreshold: CGFloat = 60
            let velocityThreshold: CGFloat = 600

            if translation > translationThreshold || velocity > velocityThreshold {
                let _ = onUiEvent(appId, LxAppUIEvent.backPress, "")
            }
        default:
            break
        }
    }

    private func attachWebViewToUI(webView: WKWebView, for appId: String, path: String) {
        // Check if WebView is already properly attached
        if webView.superview == rootContainer && !webView.isHidden {
            // WebView is already attached and visible, just ensure it's configured
            configureWebView(webView, transparent: shouldUseTransparentMode(for: appId, path: path))
            // Always ensure WebView is resumed, even if already visible
            webView.resumeWebView()

            // Update current app state AFTER successful attach/switch
            LxAppCore.setCurrentPath(path)

            // Always trigger onPageShow for page content changes, even if same WebView
            lingxia.onPageShow(appId, path)

            // Update UI components after onPageShow
            updateNavigationBar(appId: appId, path: path)
            updateTabBar(for: appId, path: path)

            return
        }

        // Ensure UI is set up before adding WebView
        if rootContainer == nil {
            // If view hasn't loaded yet, defer WebView attachment
            DispatchQueue.main.async { [weak self] in
                self?.attachWebViewToUI(webView: webView, for: appId, path: path)
            }
            return
        }

        // Setup WebView with app info before attachment
        webView.setup(appId: appId, path: path)

        // Configure WebView appearance
        configureWebView(webView, transparent: shouldUseTransparentMode(for: appId, path: path))

        // Calculate correct top offset based on the target page's NavigationBar state
        // This prevents the timing issue where navigationAreaHeight might not be updated yet
        let topOffset = calculateTopOffset(for: appId, path: path)
        let constraints = [
            webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webView.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: topOffset),
            webView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ]

        // Use shared WebViewManager logic which will trigger onPageShow
        WebViewManager.attachWebViewToContainer(webView, container: rootContainer, constraints: constraints)

        // Install SameLevel overlay for native UI components (video, input, etc.)
        SameLevelBridge.attachIfNeeded(to: webView)

        // Store the top constraint reference for future updates
        if let topConstraint = constraints.first(where: { $0.firstAnchor == webView.topAnchor }) {
            currentWebViewTopConstraint = topConstraint
        }

        // Update current app state AFTER successful attach/switch
        LxAppCore.setCurrentPath(path)

        // Update UI components after WebView attachment and onPageShow
        updateNavigationBar(appId: appId, path: path)
        updateTabBar(for: appId, path: path)
    }

    /// Calculate the correct top offset for a WebView based on the target page's NavigationBar state
    /// This prevents timing issues where navigationAreaHeight might not be updated yet
    private func calculateTopOffset(for appId: String, path: String) -> CGFloat {
        // Get the NavigationBar state for this specific page
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
        let state = NavigationBarStateManager.shared.currentState

        let showNavbar = state?.show_navbar ?? false

        if showNavbar {
            return statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
        } else {
            return 0
        }
    }

    private func updateWebViewConstraints(for appId: String, topOffset: CGFloat? = nil) {
        guard LxAppCore.currentAppId == appId,
              let webView = getCurrentWebView(),
              rootContainer != nil else { return }

        // Only update constraints if WebView is properly attached to the view hierarchy
        guard webView.superview == rootContainer else {
            return
        }

        // Remove old constraint
        if let oldConstraint = currentWebViewTopConstraint {
            oldConstraint.isActive = false
            rootContainer.removeConstraint(oldConstraint)
        }

        // Use provided topOffset or calculate from current state
        let actualTopOffset = topOffset ?? navigationAreaHeight
        let newConstraint = webView.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: actualTopOffset)
        newConstraint.isActive = true

        // Store constraint reference
        currentWebViewTopConstraint = newConstraint

        // Force layout update
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()
    }

    private func setupNavigationBar(appId: String) {
        guard LxAppCore.currentAppId == appId,
              rootContainer != nil else { return }

        // Use global navigation bar instead of per-app
        if globalNavigationBar == nil {
            setupGlobalNavigationBar()
        }

        // Update navigation bar with current state
        let currentPath = LxAppCore.getCurrentPath()
        updateNavigationBar(appId: appId, path: currentPath)
    }

    public func updateNavigationBar(appId: String, path: String) {
        guard let navigationBar = globalNavigationBar else {
            os_log("updateNavigationBar: NavigationBar not initialized", log: Self.log, type: .error)
            return
        }

        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
        let state = NavigationBarStateManager.shared.currentState

        navigationBar.updateWithState(state)

        // Update WebView constraints to match NavigationBar state without animation
        UIView.performWithoutAnimation {
            updateWebViewConstraints(for: appId)
            view.layoutIfNeeded()
        }

        setNeedsStatusBarAppearanceUpdate()
    }

    private func cleanupLxAppState(appId: String) {
        guard LxAppCore.currentAppId == appId else { return }

        // Remove WebView
        getCurrentWebView()?.removeFromSuperview()
        getCurrentWebView()?.pauseWebView()

        // Remove TabBar
        if let tabBar = currentTabBar {
            tabBar.removeFromSuperview()
            currentTabBar = nil
        }

        // Clean up constraints
        if let constraint = currentWebViewTopConstraint {
            constraint.isActive = false
        }
    }

    internal func applyAppStyling(for appId: String, path: String? = nil) {
        guard LxAppCore.currentAppId == appId else { return }

        let currentPath = path ?? LxAppCore.getCurrentPath()
        let shouldUseTransparent = shouldUseTransparentMode(for: appId, path: currentPath)

        if shouldUseTransparent {
            setCompleteTransparency()
            if let webView = getCurrentWebView() {
                configureWebView(webView, transparent: true)
            }
        } else {
            setOpaqueBackgrounds()
        }
    }

    private func setCompleteTransparency() {
        // Main view controller view
        view.backgroundColor = UIColor.clear
        view.isOpaque = false
        view.layer.backgroundColor = UIColor.clear.cgColor

        // Root container
        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.clear
            rootContainer.isOpaque = false
            rootContainer.layer.backgroundColor = UIColor.clear.cgColor
        }

        // WebView container
        if let webViewContainer = webViewContainer {
            webViewContainer.backgroundColor = UIColor.clear
            webViewContainer.isOpaque = false
            webViewContainer.layer.backgroundColor = UIColor.clear.cgColor
        }

        // Window transparency
        if let window = view.window {
            window.backgroundColor = UIColor.clear
            window.isOpaque = false
        }

        // All windows in scene
        if let windowScene = view.window?.windowScene {
            for window in windowScene.windows {
                window.backgroundColor = UIColor.clear
                window.isOpaque = false
            }
        }

        // Navigation controller transparency
        if let navController = navigationController {
            navController.view.backgroundColor = UIColor.clear
            navController.view.isOpaque = false
        }

        if let navBar = globalNavigationBar {
            navBar.backgroundColor = UIColor.clear
            navBar.layer.backgroundColor = UIColor.clear.cgColor
            navBar.isOpaque = false
        }
    }

    private func setOpaqueBackgrounds() {
        view.backgroundColor = UIColor.white
        rootContainer?.backgroundColor = UIColor.white
        webViewContainer?.backgroundColor = UIColor.white

        // Configure current WebView only - no need to iterate all apps
        if LxAppCore.currentAppId != nil,
           let webView = getCurrentWebView() {
            configureWebView(webView, transparent: false)
        }
    }

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

    private func applyTabBarLayoutParams(tabBar: LingXiaTabBar, config: TabBar, for appId: String) {
        let isVertical = config.position == 1 || config.position == 2 // 1=left, 2=right
        let tabBarSize = CGFloat(config.dimension)

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

            // For bottom position, extend to view.bottomAnchor to cover safe area
            tabBar.bottomAnchor.constraint(equalTo: view.bottomAnchor).isActive = true
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
                  let appId = userInfo["appId"] as? String else { return }

            DispatchQueue.main.async {
                self.closeLxApp(appId: appId)
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
                self.currentTabBar?.refreshLayout()

                // After refreshing, the bar might have become visible, so we must ensure
                // it's correctly layered in front of the webview.
                self.bringUIElementsToFront()
            }
        }
    }

    deinit {
        if let closeAppObserver = closeAppObserver {
            NotificationCenter.default.removeObserver(closeAppObserver)
        }

        if let tabBarObserver = tabBarObserver {
            NotificationCenter.default.removeObserver(tabBarObserver)
        }
    }

    public override var preferredStatusBarStyle: UIStatusBarStyle {
        let currentPath = LxAppCore.getCurrentPath()
        let transparent = shouldUseTransparentMode(for: LxAppCore.currentAppId ?? "", path: currentPath)
        let navState = NavigationBarStateManager.shared.currentState

        // When navbar is hidden (transparent mode), check for custom statusBarStyle
        if transparent {
            if let navState = navState {
                let statusBarStyle = navState.text_style.toString()

                if statusBarStyle == "dark" {
                    return .darkContent
                } else if statusBarStyle == "light" {
                    return .lightContent
                }
            }

            // Default for transparent mode: light content (white text)
            return .lightContent
        }

        // When navbar is shown, determine style based on navbar background color
        if let navState = navState {
            // Check if navbar background is dark
            let backgroundColor = navState.background_color
            let isDark = isColorDark(backgroundColor)

            if isDark {
                return .lightContent
            } else {
                return .darkContent
            }
        }

        return .lightContent // Default to light content for better visibility
    }

    public override var preferredStatusBarUpdateAnimation: UIStatusBarAnimation {
        return .fade
    }

    /// Helper function to determine if a color is dark
    private func isColorDark(_ argbColor: UInt32) -> Bool {
        // Extract RGB components from ARGB
        let red = CGFloat((argbColor >> 16) & 0xFF) / 255.0
        let green = CGFloat((argbColor >> 8) & 0xFF) / 255.0
        let blue = CGFloat(argbColor & 0xFF) / 255.0

        // Calculate luminance using standard formula
        let luminance = 0.299 * red + 0.587 * green + 0.114 * blue

        // Consider colors with luminance < 0.5 as dark
        return luminance < 0.5
    }

    public override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
        return [.bottom]
    }

    public override var prefersHomeIndicatorAutoHidden: Bool {
        return false
    }

    /// Get current path for the active LxApp - always returns definitive value from Rust
    public func getCurrentPath() -> String {
        return LxAppCore.getCurrentPath()
    }

    private func setupGlobalNavigationBar() {
        guard globalNavigationBar == nil else { return }
        guard rootContainer != nil else {
            os_log("setupGlobalNavigationBar: rootContainer is nil", log: Self.log, type: .error)
            return
        }

        globalNavigationBar = LingXiaNavigationBar()
        globalNavigationBar?.translatesAutoresizingMaskIntoConstraints = false
        rootContainer.addSubview(globalNavigationBar!)

        // Store height constraint for dynamic updates - include status bar height
        let totalHeight = statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
        let heightConstraint = globalNavigationBar!.heightAnchor.constraint(equalToConstant: totalHeight)
        globalNavigationBar?.heightConstraint = heightConstraint

        NSLayoutConstraint.activate([
            globalNavigationBar!.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            globalNavigationBar!.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            globalNavigationBar!.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            heightConstraint
        ])

        globalNavigationBar?.isHidden = true // Initially hidden
    }

    private func createTabBar(config: TabBar, appId: String) -> LingXiaTabBar {
        let tabBar = LingXiaTabBar()
        tabBar.initialize(config: config, appId: appId)
        tabBar.translatesAutoresizingMaskIntoConstraints = false
        tabBar.alpha = 1.0

        // Use universal tab click handler (navigation handled by Rust)
        tabBar.setOnTabSelectedListener { index, _ in
            if let appId = LxAppCore.currentAppId {
                let _ = onUiEvent(appId, LxAppUIEvent.tabBarClick, String(index))
            }
        }

        rootContainer.addSubview(tabBar)
        applyTabBarLayoutParams(tabBar: tabBar, config: config, for: appId)

        return tabBar
    }

    /// Ensures UI elements are properly layered above WebView content
    /// Call this after WebView transitions or when UI elements need to be visible
    private func bringUIElementsToFront() {
        // Bring UI elements to front in correct z-order: NavBar -> TabBar -> Capsule
        if let navBar = globalNavigationBar, !navBar.isHidden {
            rootContainer.bringSubviewToFront(navBar)
        }
        if let tabBar = currentTabBar, !tabBar.isHidden {
            rootContainer.bringSubviewToFront(tabBar)
        }
        if let capsule = globalCapsuleButton, !capsule.isHidden {
            rootContainer.bringSubviewToFront(capsule)
        }
    }

    /// Finalize WebView attachment after animation completes
    private func finalizeWebViewAttachment(webView: WKWebView, appId: String, path: String) {
        // Update current app state first
        LxAppCore.setCurrentPath(path)

        // Always trigger onPageShow for navigation transitions
        lingxia.onPageShow(appId, path)

        // Apply proper styling first (this will set transparency if needed)
        applyAppStyling(for: appId, path: path)

        // Update navigation bar and tabBar for the current path after onPageShow
        updateNavigationBar(appId: appId, path: path)
        updateTabBar(for: appId, path: path)

        // Ensure navigation bar transparency after forward/backward navigation
        let shouldUseTransparent = shouldUseTransparentMode(for: appId, path: path)
        if shouldUseTransparent {
            if let navigationBar = globalNavigationBar {
                navigationBar.backgroundColor = UIColor.clear
                navigationBar.layer.backgroundColor = UIColor.clear.cgColor
                navigationBar.isOpaque = false
            }
        }

        // Calculate correct navigation area height based on the target state
        let correctTopOffset = shouldUseTransparent ? 0 : (statusBarHeight + NavigationBarState.DEFAULT_HEIGHT)

        // Update constraints with the correct offset immediately
        updateWebViewConstraints(for: appId, topOffset: correctTopOffset)

        // Ensure WebView is visible and active
        webView.isHidden = false
        webView.resumeWebView()

        bringUIElementsToFront()
    }

    /// Screenshot-based animation for same WebView navigation
    private func performSameWebViewAnimation(webView: WKWebView, animationType: AnimationType, appId: String, path: String) {
        let isBackward = animationType == .backward

        // Prepare snapshot early for backward BEFORE any UI updates
        var preSnapshot: UIView?
        rootContainer.layoutIfNeeded()
        if isBackward {
            preSnapshot = rootContainer.snapshotView(afterScreenUpdates: false)
            if let s = preSnapshot {
                s.frame = rootContainer.bounds
                s.backgroundColor = .white
            }
        }

        // Ensure WebView is visible and active, and backgrounds are correct
        rootContainer.backgroundColor = .white
        view.backgroundColor = .white
        webView.backgroundColor = .white
        webView.isHidden = false
        webView.alpha = 1.0
        webView.resumeWebView()

        if let navigationBar = globalNavigationBar {
            navigationBar.backgroundColor = .white
            navigationBar.isHidden = false
            navigationBar.alpha = 1.0
        }

        // Update constraints with the correct offset for the TARGET state
        let shouldUseTransparent = shouldUseTransparentMode(for: appId, path: path)
        let correctTopOffset = shouldUseTransparent ? 0 : (statusBarHeight + NavigationBarState.DEFAULT_HEIGHT)
        updateWebViewConstraints(for: appId, topOffset: correctTopOffset)
        bringUIElementsToFront()
        rootContainer.layoutIfNeeded()

        // Use prepared snapshot for backward, otherwise capture after updates for forward
        let containerSnapshot: UIView = {
            if let s = preSnapshot { return s }
            let v = rootContainer.snapshotView(afterScreenUpdates: true) ?? UIView()
            v.frame = rootContainer.bounds
            v.backgroundColor = .white
            return v
        }()
        containerSnapshot.frame = rootContainer.bounds
        containerSnapshot.backgroundColor = .white
        rootContainer.addSubview(containerSnapshot)

        // Robust width fallback to avoid zero-distance animations
        let screenWidth: CGFloat =
            rootContainer.bounds.width > 0 ? rootContainer.bounds.width :
            (view.bounds.width > 0 ? view.bounds.width : UIScreen.main.bounds.width)

        // Set initial position for slide animation
        let slideDistance: CGFloat = isBackward ? -screenWidth : screenWidth
        webView.transform = CGAffineTransform(translationX: slideDistance, y: 0)
        globalNavigationBar?.transform = CGAffineTransform(translationX: slideDistance, y: 0)

        // Animate the slide transition
        UIView.animate(withDuration: 0.35, delay: 0, options: [.curveEaseInOut], animations: {
            webView.transform = .identity
            self.globalNavigationBar?.transform = .identity

            let snapshotSlide: CGFloat = isBackward ? screenWidth : -screenWidth
            containerSnapshot.transform = CGAffineTransform(translationX: snapshotSlide, y: 0)
        }, completion: { _ in
            containerSnapshot.removeFromSuperview()
            self.finalizeWebViewAttachment(webView: webView, appId: appId, path: path)
        })
    }

    /// Perform slide transition between WebViews for forward/backward navigation
    private func performSlideTransition(from currentWebView: WKWebView, to targetWebView: WKWebView, animationType: AnimationType, appId: String, path: String) {
        let isBackNavigation = animationType == .backward
        let animationDuration: TimeInterval = 0.3

        // Set up initial positions - use view bounds as fallback if rootContainer bounds is zero
        let screenWidth = rootContainer.bounds.width > 0 ? rootContainer.bounds.width : view.bounds.width
        let slideInTranslation: CGFloat = isBackNavigation ? -screenWidth : screenWidth
        let slideOutTranslation: CGFloat = isBackNavigation ? screenWidth : -screenWidth

        // Update navigation bar and tabBar state first (before animation)
        updateNavigationBar(appId: appId, path: path)
        updateTabBar(for: appId, path: path)

        // Ensure target WebView is properly configured for animation
        if targetWebView.superview != rootContainer {
            rootContainer.addSubview(targetWebView)
            targetWebView.translatesAutoresizingMaskIntoConstraints = false

            // Set up basic constraints without calling updateWebViewConstraints during animation
            // Use rootContainer as reference instead of view to ensure proper containment
            NSLayoutConstraint.activate([
                targetWebView.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
                targetWebView.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
                targetWebView.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: statusBarHeight + NavigationBarState.DEFAULT_HEIGHT),
                targetWebView.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor)
            ])
        }

        // Configure WebView appearance
        configureWebView(targetWebView, transparent: shouldUseTransparentMode(for: appId, path: path))
        targetWebView.resumeWebView()

        // Handle same WebView case (forward/backward to same page)
        if currentWebView == targetWebView {
            performSameWebViewAnimation(webView: currentWebView, animationType: animationType, appId: appId, path: path)
            return

        } else {
            // Different WebViews - normal slide transition
            // Ensure target WebView is properly configured and visible
            targetWebView.isHidden = false
            targetWebView.alpha = 1.0

            // Force white background during animation to prevent black screen
            targetWebView.backgroundColor = UIColor.white
            targetWebView.scrollView.backgroundColor = UIColor.white
            targetWebView.layer.backgroundColor = UIColor.white.cgColor

            // Set initial position for target WebView
            targetWebView.transform = CGAffineTransform(translationX: slideInTranslation, y: 0)

            // Set initial position for navbar
            if let navigationBar = globalNavigationBar {
                navigationBar.transform = CGAffineTransform(translationX: slideInTranslation, y: 0)
            }

            // Force layout update to ensure proper frame sizes
            rootContainer.layoutIfNeeded()

            // Perform slide animation - WebView and navbar together
            UIView.animate(withDuration: animationDuration, delay: 0, options: [.curveEaseInOut], animations: {
                // Slide target WebView in
                targetWebView.transform = .identity

                // Slide current WebView out
                currentWebView.transform = CGAffineTransform(translationX: slideOutTranslation, y: 0)

                // Animate navbar with WebViews
                if let navigationBar = self.globalNavigationBar {
                    navigationBar.transform = .identity
                }
            }, completion: { _ in
                // Clean up after animation
                currentWebView.isHidden = true
                currentWebView.pauseWebView()
                currentWebView.transform = .identity

                // Properly attach WebView to UI after animation
                self.finalizeWebViewAttachment(webView: targetWebView, appId: appId, path: path)
            })
        }
    }

    /// Perform slide out transition when no target WebView is available
    private func performSlideOutTransition(from currentWebView: WKWebView, animationType: AnimationType) {
        let isBackward = animationType == .backward
        let animationDuration: TimeInterval = 0.3

        let screenWidth = rootContainer.bounds.width
        let slideOutTranslation: CGFloat = isBackward ? screenWidth : -screenWidth

        UIView.animate(withDuration: animationDuration, delay: 0, options: [.curveEaseInOut], animations: {
            currentWebView.transform = CGAffineTransform(translationX: slideOutTranslation, y: 0)
        }, completion: { _ in
            currentWebView.isHidden = true
            currentWebView.pauseWebView()
            currentWebView.transform = .identity
        })
    }
}

extension LxAppViewController: UIGestureRecognizerDelegate {
    public func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        // Allow the back edge swipe to coexist with WebView scrolling.
        gestureRecognizer === backEdgePanGesture
    }
}

#endif
