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

    private let stateManager = LxAppStateManager.shared

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
    private var currentTabBar: LingXiaTabBar?
    private var cancellables = Set<AnyCancellable>()
    nonisolated(unsafe) private var closeAppObserver: NSObjectProtocol?

    private var statusBarHeight: CGFloat {
        return LxAppTheme.getStatusBarHeight()
    }

    private var navigationAreaHeight: CGFloat {
        guard let currentAppId = currentAppId,
              let _ = stateManager.getState(for: currentAppId),
              let navState = NavigationBarStateManager.shared.currentState,
              navState.show_navbar else {
            // Transparent mode: WebView starts from top (0 offset)
            return 0
        }
        // Normal mode: WebView starts after status bar + navbar
        return statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
    }

    private var shouldUseTransparentMode: Bool {
        // Use transparent mode when navbar is hidden (like Home page)
        guard let navState = NavigationBarStateManager.shared.currentState else {
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

        // Set initial background to prevent black flash
        view.backgroundColor = UIColor.black

        setupUI()
    }

    public override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)

        // Apply transparency and styling for current app
        if let currentAppId = currentAppId {
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
        // Global UI components will be created on-demand
        // This ensures they're only created when needed
    }

    private func setupRootContainer() {
        rootContainer = UIView()
        rootContainer.backgroundColor = UIColor.white
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
        webViewContainer.backgroundColor = UIColor.white
        webViewContainer.isOpaque = true
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

        // Update app state based on navigation type
        updateAppStateForNavigation(appId: appId, path: path, navigationType: navigationType)

        // Setup or switch WebView
        setupOrSwitchWebView(appId: appId, path: path, navigationType: navigationType)

        // Update global UI components
        updateGlobalUIComponents(for: appId, path: path, navigationType: navigationType)

        // Apply app styling
        applyAppStyling(for: appId)

        // Update status bar style
        setNeedsStatusBarAppearanceUpdate()

        // Trigger page lifecycle
        triggerPageLifecycle(appId: appId, path: path, navigationType: navigationType)

        // Update current app tracking
        currentAppId = appId
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
                switchToLxApp(appId: nextAppId, path: nextState.currentPath)
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

    /// Switches page within the current LxApp (deprecated - use navigate instead)
    public func switchPage(appId: String, path: String) {
        // Deprecated: Use navigate(appId:to:with:.forward) instead
        navigate(appId: appId, to: path, with: .forward)
    }

    private func updateAppStateForNavigation(appId: String, path: String, navigationType: NavigationType) {
        // Create or update app state using state manager
        let _ = stateManager.createOrUpdateState(appId: appId, path: path)

        // Update navigation history based on type
        stateManager.updateStateForNavigation(appId: appId, path: path, navigationType: navigationType)
    }

    private func setupOrSwitchWebView(appId: String, path: String, navigationType: NavigationType) {
        guard let appState = stateManager.getState(for: appId) else { return }

        if appState.webView == nil {
            os_log("Creating WebView for %@ at %@", log: Self.log, type: .info, appId, path)

            triggerPageLifecycle(appId: appId, path: path, navigationType: .launch)

            if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                os_log("WebView found immediately", log: Self.log, type: .info)
                attachWebViewToUI(webView: webView, for: appId)
            } else {
                os_log("WebView not found immediately, will retry", log: Self.log, type: .info)
            }
        } else {
            // WebView exists, just show it
            appState.webView?.isHidden = false
            appState.webView?.resumeWebView()

            // Trigger appropriate lifecycle based on navigation type
            triggerPageLifecycle(appId: appId, path: path, navigationType: navigationType)
        }

        // Update WebView constraints if needed
        updateWebViewConstraints(for: appId)
    }

    private func updateGlobalUIComponents(for appId: String, path: String, navigationType: NavigationType) {
        ensureCapsuleButton(for: appId)
        ensureNavigationBar(for: appId, path: path)
        ensureTabBar(for: appId, path: path, navigationType: navigationType)
        bringUIElementsToFront()
    }

    private func ensureCapsuleButton(for appId: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        let shouldShow = !appState.isDisplayingHomeLxApp
        globalCapsuleButton?.isHidden = !shouldShow

        if shouldShow && globalCapsuleButton == nil {
            setupGlobalCapsuleButton()
        }
    }

    private func ensureNavigationBar(for appId: String, path: String) {
        if globalNavigationBar == nil {
            setupGlobalNavigationBar()
        }
        globalNavigationBar?.isHidden = false
    }

    private func ensureTabBar(for appId: String, path: String, navigationType: NavigationType) {
        currentTabBar?.isHidden = true

        guard let tabConfig = lingxia.getTabBar(appId) else {
            currentTabBar = nil
            return
        }

        if let cachedTabBar = tabBarCache[appId] {
            currentTabBar = cachedTabBar
        } else {
            currentTabBar = createTabBar(config: tabConfig, appId: appId)
            tabBarCache[appId] = currentTabBar!
        }

        currentTabBar?.isHidden = false
        if navigationType == .switchTab {
            currentTabBar?.syncSelectedTabWithCurrentPath(path)
        }
    }

    private func triggerPageLifecycle(appId: String, path: String, navigationType: NavigationType) {
        switch navigationType {
        case .launch:
            os_log("Launch: Opening LxApp %@ at %@", log: Self.log, type: .info, appId, path)
            let result = onLxappOpened(appId, path)
            os_log("onLxappOpened result: %d", log: Self.log, type: .info, result)

            // Check if this path is a TabBar item
            if let tabBar = currentTabBar, tabBar.findTabIndexByPath(path) >= 0 {
                os_log("🔧 Launch: Path %@ is a TabBar item, switching to switchTab mode", log: Self.log, type: .info, path)
                // This is a TabBar item, use switchTab logic instead
                handleTabSwitch(appId: appId, path: path)
            } else {
                os_log("🔧 Launch: Path %@ is not a TabBar item, using regular launch", log: Self.log, type: .info, path)
                // Not a TabBar item, hide TabBar and update navigation bar
                currentTabBar?.isHidden = true
                updateNavigationBarForApp(appId: appId, path: path)
                lingxia.onPageShow(appId, path)
            }

        case .forward:
            os_log("Forward: Navigating to %@ for %@", log: Self.log, type: .info, path, appId)
            lingxia.onPageShow(appId, path)

        case .backward:
            os_log("Backward: Going back for %@", log: Self.log, type: .info, appId)
            let handled = lingxia.onBackPressed(appId)
            if !handled {
                os_log("Back not handled by logic, showing page %@", log: Self.log, type: .info, path)
                lingxia.onPageShow(appId, path)
            }

        case .replace:
            os_log("Replace: Replacing with %@ for %@", log: Self.log, type: .info, path, appId)
            lingxia.onPageShow(appId, path)

        case .switchTab:
            os_log("SwitchTab: Switching to %@ for %@", log: Self.log, type: .info, path, appId)
            handleTabSwitch(appId: appId, path: path)
        }
    }

    private func handleTabSwitch(appId: String, path: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        // Update current path in state
        stateManager.updateCurrentPath(path, for: appId)

        // Update navigation bar for the new path
        os_log("🔧 handleTabSwitch: Updating navigation bar for %@:%@", log: Self.log, type: .info, appId, path)
        updateNavigationBarForApp(appId: appId, path: path)

        // Update TabBar selection to match the target path
        syncTabBarWithCurrentPathInternal(path)

        // Check if we need to switch to a different WebView for this tab
        if let currentWebView = appState.webView {
            if currentWebView.currentPath == path {
                os_log("Same WebView, triggering page show for tab", log: Self.log, type: .info)
                lingxia.onPageShow(appId, path)
                return
            }
        }

        if let targetWebView = iOSLxApp.findWebView(appId: appId, path: path) {
            os_log("Found existing WebView for tab switch", log: Self.log, type: .info)

            appState.webView?.isHidden = true
            appState.webView?.pauseWebView()

            stateManager.updateWebView(targetWebView, for: appId)
            attachWebViewToUI(webView: targetWebView, for: appId)

            lingxia.onPageShow(appId, path)
        } else {
            os_log("No WebView found for tab, creating new one", log: Self.log, type: .info)

            triggerPageLifecycle(appId: appId, path: path, navigationType: .launch)

            if let newWebView = iOSLxApp.findWebView(appId: appId, path: path) {
                os_log("New WebView created for tab", log: Self.log, type: .info)

                appState.webView?.isHidden = true
                appState.webView?.pauseWebView()

                stateManager.updateWebView(newWebView, for: appId)
                attachWebViewToUI(webView: newWebView, for: appId)
            } else {
                os_log("🔄 WebView not found immediately, retrying for tab", log: Self.log, type: .info)
                retryFindWebView(appId: appId, path: path, attempt: 1, maxAttempts: 5)
            }
        }
    }

    private func createNewLxApp(appId: String, path: String) {
        // Create new app state using state manager
        let _ = stateManager.createOrUpdateState(appId: appId, path: path)
    }

    private func switchToLxApp(appId: String, path: String) {
        // Hide current app if different
        if let currentAppId = currentAppId, currentAppId != appId {
            hideCurrentLxApp()
        }

        // Show target app
        showLxApp(appId: appId, path: path)

        // Update current app
        currentAppId = appId

        // Update path using state manager
        stateManager.updateStateForNavigation(appId: appId, path: path, navigationType: .switchTab)
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
        setupWebViewForApp(appId: appId, path: path)

        // Setup UI components
        setupUIComponentsForApp(appId: appId)

        // Show WebView and UI
        appState.webView?.isHidden = false
        appState.webView?.resumeWebView()

        if let tabBar = currentTabBar {
            tabBar.isHidden = false
            os_log("Showing TabBar for %@", log: Self.log, type: .info, appId)
        } else {
            os_log("No TabBar to show for %@", log: Self.log, type: .info, appId)
        }

        globalNavigationBar?.isHidden = false
        os_log("🔧 showLxApp: About to update navigation state for %@:%@", log: Self.log, type: .info, appId, path)

        // Update navigation state
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
        os_log("🔧 showLxApp: Navigation state updated, applying styling", log: Self.log, type: .info)

        // Apply styling
        applyAppStyling(for: appId)

        // Update status bar style
        setNeedsStatusBarAppearanceUpdate()
    }

    private func setupWebViewForApp(appId: String, path: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        if appState.webView == nil {
            os_log("🔧 Looking for WebView: %@ at %@", log: Self.log, type: .info, appId, path)

            // Try to find existing WebView
            if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                os_log("🔧 WebView found on first try", log: Self.log, type: .info)
                attachWebViewToUI(webView: webView, for: appId)
            } else {
                os_log("🔧 WebView not found, starting retry sequence", log: Self.log, type: .info)
                // Retry with increasing delays
                retryFindWebView(appId: appId, path: path, attempt: 1, maxAttempts: 10)
            }
        } else {
            // WebView exists, just update its content if needed
            updateCurrentAppUI(for: appId, path: path)
        }
    }

    private func retryFindWebView(appId: String, path: String, attempt: Int, maxAttempts: Int) {
        let delay = Double(attempt) * 0.2 // Increasing delay: 0.2s, 0.4s, 0.6s, etc.

        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            if let webView = iOSLxApp.findWebView(appId: appId, path: path) {
                os_log("🔧 WebView found on attempt %d", log: Self.log, type: .info, attempt)
                self?.attachWebViewToUI(webView: webView, for: appId)
            } else if attempt < maxAttempts {
                os_log("🔧 WebView not found on attempt %d, retrying...", log: Self.log, type: .info, attempt)
                self?.retryFindWebView(appId: appId, path: path, attempt: attempt + 1, maxAttempts: maxAttempts)
            } else {
                os_log("🔧 ❌ WebView not found after %d attempts", log: Self.log, type: .error, maxAttempts)
            }
        }
    }

    private func attachWebViewToUI(webView: WKWebView, for appId: String) {
        // Store WebView reference in state manager
        stateManager.updateWebView(webView, for: appId)

        // Remove from previous parent if any
        if webView.superview != nil {
            webView.removeFromSuperview()
        }

        // Hide WebView during setup to prevent visual glitches
        webView.isHidden = true

        // Ensure UI is set up before adding WebView
        if rootContainer == nil {
            // If view hasn't loaded yet, defer WebView attachment
            DispatchQueue.main.async { [weak self] in
                self?.attachWebViewToUI(webView: webView, for: appId)
            }
            return
        }

        // Add to container
        rootContainer.addSubview(webView)
        webView.translatesAutoresizingMaskIntoConstraints = false

        // Setup constraints
        updateWebViewConstraints(for: appId)

        NSLayoutConstraint.activate([
            webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        // Configure WebView appearance
        configureWebView(webView, transparent: shouldUseTransparentMode)

        // Force layout before showing
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()

        // Resume WebView and show
        webView.resumeWebView()
        webView.isHidden = false

        // Bring UI elements to front
        bringUIElementsToFront(for: appId)
    }

    private func updateWebViewConstraints(for appId: String) {
        guard let appState = stateManager.getState(for: appId),
              let webView = appState.webView,
              rootContainer != nil else { return }

        // Remove old constraint
        if let oldConstraint = appState.webViewTopConstraint {
            oldConstraint.isActive = false
            rootContainer.removeConstraint(oldConstraint)
        }

        // Create new constraint with current navigation area height
        let topOffset = navigationAreaHeight
        let newConstraint = webView.topAnchor.constraint(equalTo: rootContainer.topAnchor, constant: topOffset)
        newConstraint.isActive = true

        // Store constraint reference in state manager
        stateManager.updateWebViewConstraint(newConstraint, for: appId)

        // Force layout update
        rootContainer.setNeedsLayout()
        rootContainer.layoutIfNeeded()
    }

    private func setupUIComponentsForApp(appId: String) {
        setupTabBarForApp(appId: appId)
        setupNavigationBarForApp(appId: appId)
    }

    private func setupTabBarForApp(appId: String) {
        guard let appState = stateManager.getState(for: appId),
              rootContainer != nil else {
            os_log("setupTabBarForApp failed: appState or rootContainer is nil for %@", log: Self.log, type: .error, appId)
            return
        }

        let tabBarConfig = lingxia.getTabBar(appId)
        os_log("TabBar config for %@: %@", log: Self.log, type: .info, appId, tabBarConfig != nil ? "found" : "nil")

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

    private func setupNavigationBarForApp(appId: String) {
        guard let appState = stateManager.getState(for: appId),
              rootContainer != nil else { return }

        // Use global navigation bar instead of per-app
        if globalNavigationBar == nil {
            setupGlobalNavigationBar()
        }

        // Update navigation bar with current state
        updateNavigationBarForApp(appId: appId, path: appState.currentPath)
    }

    private func updateNavigationBarForApp(appId: String, path: String) {
        guard let navigationBar = globalNavigationBar else { return }

        NavigationBarStateManager.shared.updateState(appId: appId, path: path)

        // Debug: Print navigation bar state
        if let state = NavigationBarStateManager.shared.currentState {
            os_log("🔍 NavBar State for %@:%@ - show_navbar: %@, show_back: %@, show_home: %@, title: %@",
                   log: Self.log, type: .info,
                   appId, path,
                   state.show_navbar ? "true" : "false",
                   state.show_back_button ? "true" : "false",
                   state.show_home_button ? "true" : "false",
                   state.title_text.toString())
        } else {
            os_log("🔍 NavBar State for %@:%@ - NO STATE", log: Self.log, type: .info, appId, path)
        }

        navigationBar.updateWithState(NavigationBarStateManager.shared.currentState)

        // Update status bar style when navigation state changes
        setNeedsStatusBarAppearanceUpdate()
    }

    private func updateCurrentAppUI(for appId: String, path: String) {
        updateNavigationBarForApp(appId: appId, path: path)
        updateWebViewConstraints(for: appId)

        // Sync TabBar if needed
        currentTabBar?.syncSelectedTabWithCurrentPath(path)
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

    private func applyAppStyling(for appId: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        let shouldUseTransparent = shouldUseTransparentMode
        os_log("🎨 applyAppStyling for %@: shouldUseTransparent = %@", log: Self.log, type: .info, appId, shouldUseTransparent ? "true" : "false")

        if shouldUseTransparent {
            setCompleteTransparency()

            // Apply WebView transparency
            if let webView = appState.webView {
                forceWebViewTransparency(webView: webView)
            }

            // Apply TabBar transparency
            if let tabBar = currentTabBar {
                tabBar.forceTransparencyMode()
            }
        } else {
            setOpaqueBackgrounds()
        }

        // Add capsule button if not home app
        if !appState.isDisplayingHomeLxApp {
            addCapsuleButton(for: appId)
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
        webViewContainer?.backgroundColor = UIColor.clear
        webViewContainer?.isOpaque = false
        webViewContainer?.layer.backgroundColor = UIColor.clear.cgColor

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
    }

    private func setOpaqueBackgrounds() {
        view.backgroundColor = UIColor.white
        view.isOpaque = true
        view.layer.backgroundColor = UIColor.white.cgColor

        if let rootContainer = rootContainer {
            rootContainer.backgroundColor = UIColor.white
            rootContainer.isOpaque = true
            rootContainer.layer.backgroundColor = UIColor.white.cgColor
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

    private func forceWebViewTransparency(webView: WKWebView) {
        configureWebView(webView, transparent: true)
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

    private func bringUIElementsToFront(for appId: String) {
        guard let rootContainer = rootContainer else { return }

        // Bring NavigationBar to front first
        if let navigationBar = globalNavigationBar {
            rootContainer.bringSubviewToFront(navigationBar)
        }

        // Bring TabBar to front
        if let tabBar = currentTabBar {
            rootContainer.bringSubviewToFront(tabBar)

            // Re-apply transparency if needed
            if TabBar.isTransparent(tabBar.config?.background_color ?? 0) {
                tabBar.forceTransparencyMode()
            }
        }

        // Bring CapsuleButton to front (highest priority)
        if let capsule = globalCapsuleButton {
            rootContainer.bringSubviewToFront(capsule)
        }
    }

    private func addCapsuleButton(for appId: String) {
        LxAppCapsuleButtons.addCapsuleButton(to: self, appId: appId)
    }

    private func switchToTab(appId: String, targetPath: String) {
        guard let appState = stateManager.getState(for: appId) else { return }

        if appState.currentPath == targetPath {
            return
        }

        // Update state using state manager
        stateManager.updateStateForNavigation(appId: appId, path: targetPath, navigationType: .switchTab)

        // Update UI for the new path
        updateCurrentAppUI(for: appId, path: targetPath)

        // Store last active path
        LxAppCore.setLastActivePath(targetPath, for: appId)

        // Setup WebView for new path
        setupWebViewForApp(appId: appId, path: targetPath)
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
        let transparent = shouldUseTransparentMode
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

    /// Check if current app has navigation bar
    public func hasNavigationBar() -> Bool {
        return globalNavigationBar != nil && globalNavigationBar?.isHidden == false
    }

    /// Create navigation bar if needed for current app
    public func createNavigationBarIfNeeded() {
        guard let currentAppId = currentAppId else { return }
        setupNavigationBarForApp(appId: currentAppId)
    }

    /// Setup WebView if ready for specific app and path
    public func setupWebViewIfReady(appId: String, path: String) {
        setupWebViewForApp(appId: appId, path: path)
    }

    /// Setup WebView for specific app and path (protocol requirement)
    public func setupWebView(appId: String, path: String) {
        setupWebViewForApp(appId: appId, path: path)
    }

    /// Hide navigation bar (protocol requirement)
    public func hideNavigationBar() {
        NavigationBarStateManager.shared.currentState = nil
    }

    /// Apply transparency effects (protocol requirement)
    public func applyTransparencyEffects() {
        guard let currentAppId = currentAppId else { return }
        applyAppStyling(for: currentAppId)
    }

    /// Perform LxApp close (protocol requirement)
    public func performLxAppClose() {
        guard let currentAppId = currentAppId else { return }
        closeLxApp(appId: currentAppId)
    }

    /// Get TabBar for current app (internal implementation)
    internal func getTabBarInternal() -> (any TabBarProtocol)? {
        return currentTabBar as? (any TabBarProtocol)
    }

    /// Sync TabBar with current path (internal implementation)
    internal func syncTabBarWithCurrentPathInternal(_ path: String) {
        currentTabBar?.syncSelectedTabWithCurrentPath(path)
    }

    /// Get app ID (for protocol compatibility)
    public var appId: String {
        return currentAppId ?? ""
    }

    /// Check if destroyed (always false for manager)
    public var isDestroyed: Bool {
        return false
    }

    private func setupGlobalCapsuleButton() {
        guard globalCapsuleButton == nil else { return }

        // Use LxAppCapsuleButtons to add capsule button
        LxAppCapsuleButtons.addCapsuleButton(to: self, appId: currentAppId ?? "")

        // Find the added capsule button view
        globalCapsuleButton = view.viewWithTag(9999) // CAPSULE_BUTTON_TAG from LxAppCapsuleButtons
    }

    private func setupGlobalNavigationBar() {
        guard globalNavigationBar == nil else { return }

        globalNavigationBar = LingXiaNavigationBar()
        globalNavigationBar?.translatesAutoresizingMaskIntoConstraints = false
        rootContainer.addSubview(globalNavigationBar!)

        // Store height constraint for dynamic updates - include status bar height
        let totalHeight = statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
        let heightConstraint = globalNavigationBar!.heightAnchor.constraint(equalToConstant: totalHeight)
        (globalNavigationBar as? iOSNavigationBarWrapper)?.heightConstraint = heightConstraint

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

        // Use universal tab click handler
        tabBar.setOnTabSelectedListener(
            LxAppPageNavigation.tabClickHandler(appId: appId)
        )

        rootContainer.addSubview(tabBar)
        applyTabBarLayoutParams(tabBar: tabBar, config: config, for: appId)

        return tabBar
    }

    private func bringUIElementsToFront() {
        // Bring UI elements to front in correct order
        if let navBar = globalNavigationBar {
            rootContainer.bringSubviewToFront(navBar)
        }

        if let tabBar = currentTabBar {
            rootContainer.bringSubviewToFront(tabBar)
        }

        if let capsule = globalCapsuleButton {
            rootContainer.bringSubviewToFront(capsule)
        }
    }
}

#endif
