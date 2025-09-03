#if os(iOS)
import UIKit
import SwiftUI
import WebKit
import os.log
import Combine
import CLingXiaFFI
@preconcurrency import ObjectiveC

// Log instance outside of @MainActor to avoid isolation issues
private let lxAppViewControllerLog = OSLog(subsystem: "LingXia", category: "LxAppViewController")

@MainActor
public class LxAppViewController: UIViewController, ObservableObject {
    private static let log = lxAppViewControllerLog

    public let stateManager = LxAppStateManager.shared

    public var currentAppId: String? {
        get { stateManager.currentAppId }
        set {
            if let newValue = newValue {
                stateManager.setCurrentApp(newValue)
            }
        }
    }

    internal var rootContainer: UIView!
    private var webViewContainer: UIView!
    private var globalCapsuleButton: UIView?
    private var globalNavigationBar: LingXiaNavigationBar?
    private var tabBarCache: [String: LingXiaTabBar] = [:]
    public var currentTabBar: LingXiaTabBar?
    private var cancellables = Set<AnyCancellable>()

    // Store pending navigation state for deferred NavigationBar initialization
    private var pendingNavigationState: (appId: String, path: String)?
    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?

    private var statusBarHeight: CGFloat {
        return LxAppTheme.getStatusBarHeight()
    }

    private var navigationAreaHeight: CGFloat {
        guard let currentAppId = currentAppId,
              let appState = stateManager.getState(for: currentAppId) else {
            return 0
        }

        if shouldUseTransparentMode(for: currentAppId, path: appState.currentPath) {
            return 0
        } else {
            return statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
        }
    }

    private func shouldUseTransparentMode(for appId: String, path: String? = nil) -> Bool {
        guard !appId.isEmpty, isViewLoaded else { return false }

        let currentPath = path ?? getCurrentPath() ?? ""
        guard !currentPath.isEmpty else { return false }

        guard let navState = LxPageNavigation.getNavigationBarState(appId: appId, path: currentPath) else {
            return false
        }

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
        if let currentAppId = currentAppId {
            // CRITICAL FIX: Ensure NavigationBarStateManager is updated before applying styling
            if let appState = stateManager.getState(for: currentAppId) {
                NavigationBarStateManager.shared.updateState(appId: currentAppId, path: appState.currentPath)
            }
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
        setupGlobalUIComponents()
        setupNotificationObservers()
    }

    private func setupGlobalUIComponents() {
        // Initialize NavigationBar immediately when rootContainer is ready
        setupGlobalNavigationBar()
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

    private func configureSystemNavigationBar() {
        if let navController = navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    /// Unified navigation entry point - handles all navigation types
    public func navigate(appId: String, to path: String, with navigationType: NavigationType) {
        os_log("Navigate: %@ to %@ with type: %@", log: Self.log, type: .info, appId, path, String(describing: navigationType))

        // Ensure view is loaded before navigation
        if !isViewLoaded {
            DispatchQueue.main.async { [weak self] in
                self?.navigate(appId: appId, to: path, with: navigationType)
            }
            return
        }

        // Set current app ID immediately to ensure all subsequent logic
        // operates on the correct app context.
        currentAppId = appId

        // Update app state based on navigation type
        updateAppStateForNavigation(appId: appId, path: path, navigationType: navigationType)

        // CRITICAL: Initialize UI components FIRST before any navigation logic
        // This ensures NavigationBar is ready when renderNavigationBar is called
        updateGlobalUIComponents(for: appId, path: path, navigationType: navigationType)

        // Update NavigationBar state
        updateNavigationBar(appId: appId, path: path)

        // Apply app styling to handle transparency changes
        applyAppStyling(for: appId, path: path)

        // Setup or switch WebView
        setupOrSwitchWebView(appId: appId, path: path, navigationType: navigationType)

        // Update status bar style
        setNeedsStatusBarAppearanceUpdate()

        // Handle TabBar visibility for launch
        if navigationType == .launch {
            if let tabBar = currentTabBar {
                let tabIndex = tabBar.findTabIndexByPath(path)
                if tabIndex >= 0 {
                    // This is a TabBar item, ensure visible and update selection
                    tabBar.isHidden = false
                    syncTabBarWithCurrentPathInternal(path)
                    bringUIElementsToFront()
                } else {
                    // Not a TabBar item, hide TabBar
                    tabBar.isHidden = true
                }
            }
        }
    }

    /// Opens a LxApp - creates new state if needed, switches if already exists
    public func openLxApp(appId: String, path: String) {
        os_log("Opening LxApp: %@ at path: %@", log: Self.log, type: .info, appId, path)

        // Create or update app state using state manager
        let _ = stateManager.createOrUpdateState(appId: appId, path: path)

        // Use unified navigation entry point
        navigate(appId: appId, to: path, with: .launch)
    }

    /// Closes a LxApp and removes its state
    public func closeLxApp(appId: String) {
        os_log("Closing LxApp: %@", log: Self.log, type: .info, appId)

        guard stateManager.getState(for: appId) != nil else {
            os_log("LxApp %@ not found for closing", log: Self.log, type: .error, appId)
            return
        }

        // Hide the app if it's currently active
        if currentAppId == appId {
            hideCurrentLxApp()

            // Switch to another app if available
            let activeAppIds = stateManager.activeAppIds.filter { $0 != appId }
            if let nextAppId = activeAppIds.first,
               let nextState = stateManager.getState(for: nextAppId) {
                currentAppId = nextAppId
                navigate(appId: nextAppId, to: nextState.currentPath, with: .launch)
            } else {
                currentAppId = nil
            }
        }

        // Clean up app state
        cleanupLxAppState(appId: appId)
        stateManager.removeState(for: appId)

        // Call FFI close handler
        let _ = onLxappClosed(appId)
    }



    public func updateAppStateForNavigation(appId: String, path: String, navigationType: NavigationType) {
        // Create or update app state using state manager
        let _ = stateManager.createOrUpdateState(appId: appId, path: path)

        // Update navigation history based on type
        stateManager.updateStateForNavigation(appId: appId, path: path, navigationType: navigationType)
    }

    public func setupOrSwitchWebView(appId: String, path: String, navigationType: NavigationType) {
        guard let appState = stateManager.getState(for: appId) else { return }

        if let targetWebView = iOSLxApp.findWebView(appId: appId, path: path) {
            // Handle navigation animations for all cases
            if let currentWebView = appState.webView {

                // Choose animation based on navigation type
                switch navigationType {
                case .switchTab:
                    if currentWebView != targetWebView {
                        // Different WebView - smooth fade transition to avoid flashing
                        UIView.transition(with: rootContainer, duration: 0.1, options: [.transitionCrossDissolve], animations: {
                            currentWebView.alpha = 0.0
                        }, completion: { _ in
                            currentWebView.isHidden = true
                            currentWebView.pauseWebView()
                            currentWebView.alpha = 1.0

                            // For switchTab navigation, ensure TabBar remains visible after WebView transition
                            if navigationType == .switchTab {
                                self.bringUIElementsToFront()
                            }
                        })
                    }
                case .forward, .backward:
                    // Forward/backward use slide animation
                    performSlideTransition(from: currentWebView, to: targetWebView, navigationType: navigationType, appId: appId, path: path)
                    return // Early return as performSlideTransition handles the rest
                default:
                    // Other types use immediate transition
                    currentWebView.isHidden = true
                    currentWebView.pauseWebView()
                }
            }

            // Show target WebView
            stateManager.updateWebView(targetWebView, for: appId)
            attachWebViewToUI(webView: targetWebView, for: appId, path: path)

        }

        // Update TabBar selection only for switchTab navigation
        if navigationType == .switchTab {
            syncTabBarWithCurrentPathInternal(path)
        }

        // Update WebView constraints if needed
        updateWebViewConstraints(for: appId)
    }

    private func updateGlobalUIComponents(for appId: String, path: String, navigationType: NavigationType) {
        updateCapsuleButton(for: appId)
        updateNavigationBar(for: appId, path: path)

        // Only update TabBar for specific navigation types
        switch navigationType {
        case .launch, .switchTab:
            updateTabBar(for: appId, path: path, navigationType: navigationType)
        default:
            if let tabBar = currentTabBar {
                let shouldShowTabBar = tabBar.findTabIndexByPath(path) >= 0
                tabBar.isHidden = !shouldShowTabBar
            }
        }

        bringUIElementsToFront()
    }

    private func updateCapsuleButton(for appId: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        let shouldShow = !appState.isDisplayingHomeLxApp

        if shouldShow && globalCapsuleButton == nil {
            LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
            globalCapsuleButton = view.viewWithTag(9999) // CAPSULE_BUTTON_TAG
        }

        globalCapsuleButton?.isHidden = !shouldShow
    }

    private func updateNavigationBar(for appId: String, path: String) {
        // NavigationBar is already initialized in setupGlobalUIComponents()
        globalNavigationBar?.isHidden = false
    }

    private func updateTabBar(for appId: String, path: String, navigationType: NavigationType) {
        guard let tabConfig = lingxia.getTabBar(appId) else {
            currentTabBar?.isHidden = true
            currentTabBar = nil
            return
        }

        if let cachedTabBar = tabBarCache[appId] {
            currentTabBar = cachedTabBar
            currentTabBar?.isHidden = false
            bringUIElementsToFront()
        } else {
            // Hide current TabBar before creating new one
            currentTabBar?.isHidden = true
            currentTabBar = createTabBar(config: tabConfig, appId: appId)
            tabBarCache[appId] = currentTabBar!
        }

        if navigationType == .switchTab {
            currentTabBar?.syncSelectedTabWithCurrentPath(path)
        }

        // Ensure TabBar is visible and brought to front after any updates
        if let tabBar = currentTabBar {
            tabBar.isHidden = false
            bringUIElementsToFront()
        }
    }

    private func hideCurrentLxApp() {
        guard let currentAppId = currentAppId,
              let appState = stateManager.getState(for: currentAppId) else { return }

        // Hide WebView
        appState.webView?.isHidden = true
        appState.webView?.pauseWebView()

        // Hide global UI components
        currentTabBar?.isHidden = true
        globalNavigationBar?.isHidden = true
        globalCapsuleButton?.isHidden = true
    }

    private func showLxApp(appId: String, path: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        // Ensure view is loaded before setting up UI components
        if !isViewLoaded {
            // Defer UI setup until view is loaded
            DispatchQueue.main.async { [weak self] in
                self?.showLxApp(appId: appId, path: path)
            }
            return
        }

        // Setup WebView if needed
        setupWebView(appId: appId, path: path)

        // Setup UI components
        setupTabBar(appId: appId)
        setupNavigationBar(appId: appId)

        // Show WebView and UI
        appState.webView?.isHidden = false
        appState.webView?.resumeWebView()

        if let tabBar = currentTabBar {
            tabBar.isHidden = false
            tabBar.alpha = 1.0
            // Ensure TabBar is brought to front
            rootContainer.bringSubviewToFront(tabBar)
        } else {
            os_log("No TabBar to show for %@", log: Self.log, type: .info, appId)
        }

        globalNavigationBar?.isHidden = false

        // Update navigation state
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)

        // Apply styling with explicit path
        applyAppStyling(for: appId, path: path)

        // Update status bar style
        setNeedsStatusBarAppearanceUpdate()
    }

    private func setupWebView(appId: String, path: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        if appState.webView == nil {

            // Try to find existing WebView
            if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                attachWebViewToUI(webView: webView, for: appId, path: path)
            }
        } else {
            // WebView exists, just update its content if needed
            updateNavigationBar(appId: appId, path: path)
            applyAppStyling(for: appId, path: path)
            currentTabBar?.syncSelectedTabWithCurrentPath(path)
            bringUIElementsToFront()
        }
    }

    private func attachWebViewToUI(webView: WKWebView, for appId: String, path: String) {
        // Store WebView reference in state manager
        stateManager.updateWebView(webView, for: appId)

        // Check if WebView is already properly attached
        if webView.superview == rootContainer && !webView.isHidden {
            // WebView is already attached and visible, just ensure it's configured
            configureWebView(webView, transparent: shouldUseTransparentMode(for: appId, path: path))
            // Always ensure WebView is resumed, even if already visible
            webView.resumeWebView()

            bringUIElementsToFront()

            return
        }

        // Remove from previous parent if any
        if webView.superview != nil && webView.superview != rootContainer {
            webView.removeFromSuperview()
        }

        // Ensure UI is set up before adding WebView
        if rootContainer == nil {
            // If view hasn't loaded yet, defer WebView attachment
            DispatchQueue.main.async { [weak self] in
                self?.attachWebViewToUI(webView: webView, for: appId, path: path)
            }
            return
        }

        // Add to container if not already added
        if webView.superview != rootContainer {
            rootContainer.addSubview(webView)
            webView.translatesAutoresizingMaskIntoConstraints = false

            // Setup constraints
            updateWebViewConstraints(for: appId)

            NSLayoutConstraint.activate([
                webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                webView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
            ])
        }

        configureWebView(webView, transparent: shouldUseTransparentMode(for: appId, path: path))

        // Show WebView without hiding first to reduce flashing
        webView.resumeWebView()
        if webView.isHidden {
            webView.isHidden = false
        }
    }

    private func updateWebViewConstraints(for appId: String, topOffset: CGFloat? = nil) {
        guard let appState = stateManager.getState(for: appId),
              let webView = appState.webView,
              rootContainer != nil else { return }

        // Remove old constraint
        if let oldConstraint = appState.webViewTopConstraint {
            oldConstraint.isActive = false
            rootContainer.removeConstraint(oldConstraint)
        }

        // Use provided topOffset or calculate from current state
        let actualTopOffset = topOffset ?? navigationAreaHeight
        let newConstraint = webView.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: actualTopOffset)
        newConstraint.isActive = true

        // Store constraint reference in state manager
        stateManager.updateWebViewConstraint(newConstraint, for: appId)

        // Force layout update
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()
    }

    public func setupTabBar(appId: String) {
        guard let appState = stateManager.getState(for: appId),
              rootContainer != nil else {
            os_log("setupTabBar failed: appState or rootContainer is nil for %@", log: Self.log, type: .error, appId)
            return
        }

        let tabBarConfig = lingxia.getTabBar(appId)
        if let config = tabBarConfig {
            // Use global TabBar cache instead of per-app state
            if tabBarCache[appId] == nil {
                // Create new TabBar and cache it
                let tabBar = createTabBar(config: config, appId: appId)
                tabBarCache[appId] = tabBar
            }

            // Set as current TabBar
            currentTabBar = tabBarCache[appId]

            // Sync TabBar with current path
            currentTabBar?.syncSelectedTabWithCurrentPath(appState.currentPath)
        }
    }

    private func setupNavigationBar(appId: String) {
        guard let appState = stateManager.getState(for: appId),
              rootContainer != nil else { return }

        // Use global navigation bar instead of per-app
        if globalNavigationBar == nil {
            setupGlobalNavigationBar()
        }

        // Update navigation bar with current state
        updateNavigationBar(appId: appId, path: appState.currentPath)
    }

    public func updateNavigationBar(appId: String, path: String) {
        guard let navigationBar = globalNavigationBar else {
            os_log("updateNavigationBar: NavigationBar not initialized", log: Self.log, type: .error)
            return
        }

        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
        navigationBar.updateWithState(NavigationBarStateManager.shared.currentState)
        setNeedsStatusBarAppearanceUpdate()
    }

    private func cleanupLxAppState(appId: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        // Remove WebView
        appState.webView?.removeFromSuperview()
        appState.webView?.pauseWebView()

        // Remove TabBar from cache if it exists
        if let tabBar = tabBarCache[appId] {
            tabBar.removeFromSuperview()
            tabBarCache.removeValue(forKey: appId)
        }

        // Global navigation bar stays (shared across apps)

        // Clean up constraints
        if let constraint = appState.webViewTopConstraint {
            constraint.isActive = false
        }
    }

    internal func applyAppStyling(for appId: String, path: String? = nil) {
        guard let appState = stateManager.getState(for: appId) else { return }

        let currentPath = path ?? getCurrentPath() ?? ""
        let shouldUseTransparent = shouldUseTransparentMode(for: appId, path: currentPath)

        if shouldUseTransparent {
            setCompleteTransparency()
            if let webView = appState.webView {
                configureWebView(webView, transparent: true)
            }
            currentTabBar?.forceTransparencyMode()
        } else {
            setOpaqueBackgrounds()
        }

        // Add capsule button if not home app
        if !appState.isDisplayingHomeLxApp {
            LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
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
            navBar.isOpaque = false
        }
    }

    private func setOpaqueBackgrounds() {
        view.backgroundColor = UIColor.white
        rootContainer?.backgroundColor = UIColor.white
        webViewContainer?.backgroundColor = UIColor.white

        for appId in stateManager.activeAppIds {
            if let appState = stateManager.getState(for: appId), let webView = appState.webView {
                configureWebView(webView, transparent: false)
            }
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
    }

    deinit {
        if let closeAppObserver = closeAppObserver {
            NotificationCenter.default.removeObserver(closeAppObserver)
        }
    }

    public override var preferredStatusBarStyle: UIStatusBarStyle {
        let currentPath = getCurrentPath() ?? ""
        let transparent = shouldUseTransparentMode(for: currentAppId ?? "", path: currentPath)
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

    /// Get current path for the active LxApp
    public func getCurrentPath() -> String? {
        guard let currentAppId = currentAppId else { return nil }
        return stateManager.getCurrentPath(for: currentAppId)
    }



    /// Sync TabBar with current path (internal implementation)
    internal func syncTabBarWithCurrentPathInternal(_ path: String) {
        currentTabBar?.syncSelectedTabWithCurrentPath(path)
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
        tabBar.setConfig(config: config, appId: appId)
        tabBar.translatesAutoresizingMaskIntoConstraints = false
        tabBar.isHidden = false
        tabBar.alpha = 1.0

        // Use universal tab click handler
        tabBar.setOnTabSelectedListener(
            LxAppPageNavigation.tabClickHandler(appId: appId)
        )

        rootContainer.addSubview(tabBar)
        applyTabBarLayoutParams(tabBar: tabBar, config: config, for: appId)

        // Ensure TabBar is brought to front immediately after creation
        rootContainer.bringSubviewToFront(tabBar)

        return tabBar
    }

    private func bringUIElementsToFront() {
        // Bring UI elements to front in correct order
        if let navBar = globalNavigationBar {
            rootContainer.bringSubviewToFront(navBar)
        }
        if let tabBar = currentTabBar {
            tabBar.isHidden = false
            rootContainer.bringSubviewToFront(tabBar)

            // Re-apply transparency if needed
            if TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
                tabBar.forceTransparencyMode()
            }
        }
        if let capsule = globalCapsuleButton {
            rootContainer.bringSubviewToFront(capsule)
        }
    }

    /// Update capsule button visibility - only home app hides it
    public func updateCapsuleButtonVisibility(appId: String) {
        let isHomeApp = LxAppCore.isHomeLxApp(appId)
        if !isHomeApp {
            updateCapsuleButton(for: appId)
        } else {
            globalCapsuleButton?.isHidden = true
        }
    }

    /// Finalize WebView attachment after animation completes
    private func finalizeWebViewAttachment(webView: WKWebView, appId: String, path: String) {
        // Apply proper styling first (this will set transparency if needed)
        applyAppStyling(for: appId, path: path)

        // Update navigation bar for the current path
        updateNavigationBar(appId: appId, path: path)

        // Calculate correct navigation area height based on the target state
        let shouldUseTransparent = shouldUseTransparentMode(for: appId, path: path)
        let correctTopOffset = shouldUseTransparent ? 0 : (statusBarHeight + NavigationBarState.DEFAULT_HEIGHT)

        // Update constraints with the correct offset immediately
        updateWebViewConstraints(for: appId, topOffset: correctTopOffset)

        // Ensure WebView is visible and active
        webView.isHidden = false
        webView.resumeWebView()

        bringUIElementsToFront()
    }

    /// Screenshot-based animation for same WebView navigation
    private func performSameWebViewAnimation(webView: WKWebView, navigationType: NavigationType, appId: String, path: String) {
        let isBackward = navigationType == .backward

        // Pre-configure backgrounds to eliminate black shadows
        rootContainer.backgroundColor = UIColor.white
        view.backgroundColor = UIColor.white
        webView.backgroundColor = UIColor.white
        webView.isHidden = false
        webView.alpha = 1.0

        if let navigationBar = globalNavigationBar {
            navigationBar.backgroundColor = UIColor.white
            navigationBar.isHidden = false
            navigationBar.alpha = 1.0
        }

        // Force layout update
        rootContainer.layoutIfNeeded()

        // Create snapshot
        let containerSnapshot = rootContainer.snapshotView(afterScreenUpdates: true) ?? UIView()
        containerSnapshot.frame = rootContainer.bounds
        containerSnapshot.backgroundColor = UIColor.white

        // Set initial position for slide animation
        let screenWidth = rootContainer.bounds.width
        let slideDistance: CGFloat = isBackward ? -screenWidth : screenWidth

        webView.transform = CGAffineTransform(translationX: slideDistance, y: 0)
        globalNavigationBar?.transform = CGAffineTransform(translationX: slideDistance, y: 0)

        rootContainer.addSubview(containerSnapshot)

        // Animate the slide transition
        UIView.animate(withDuration: 0.35, delay: 0, options: [.curveEaseOut], animations: {
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
    private func performSlideTransition(from currentWebView: WKWebView, to targetWebView: WKWebView, navigationType: NavigationType, appId: String, path: String) {
        let isBackNavigation = navigationType == .backward
        let animationDuration: TimeInterval = 0.3

        // Set up initial positions - use view bounds as fallback if rootContainer bounds is zero
        let screenWidth = rootContainer.bounds.width > 0 ? rootContainer.bounds.width : view.bounds.width
        let slideInTranslation: CGFloat = isBackNavigation ? -screenWidth : screenWidth
        let slideOutTranslation: CGFloat = isBackNavigation ? screenWidth : -screenWidth

        // Prepare target WebView - but don't attach to UI yet to avoid constraint conflicts
        stateManager.updateWebView(targetWebView, for: appId)

        // Update navigation bar state first (before animation)
        updateNavigationBar(appId: appId, path: path)

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
            performSameWebViewAnimation(webView: currentWebView, navigationType: navigationType, appId: appId, path: path)
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
    private func performSlideOutTransition(from currentWebView: WKWebView, navigationType: NavigationType) {
        let isBackward = navigationType == .backward
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

#endif
