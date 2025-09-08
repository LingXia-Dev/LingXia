#if os(iOS)
import Foundation
import UIKit
import SwiftUI
import WebKit
import os.log
import CLingXiaRustAPI
import UserNotifications

/// iOS LxApp manager
@MainActor
public class iOSLxApp: LxAppRenderer {
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "iOSLxApp")
    private static var instance: iOSLxApp?
    private let context: UIApplication

    /// Single manager instance for all LxApps
    private var lxAppManager: LxAppViewController?

    private init(context: UIApplication) {
        self.context = context
    }

    /// Gets the singleton iOSLxApp instance
    public static func getInstance() -> iOSLxApp {
        guard let instance = instance else {
            fatalError("iOSLxApp not initialized")
        }
        return instance
    }

    /// Initialize the iOS LxApp system
    public static func initialize() {
        if instance != nil {
            return
        }

        instance = iOSLxApp(context: UIApplication.shared)

        // Initialize core system
        LxAppCore.initializeCore()

        configureGlobalSystemBars()

        // Initialize push notifications
        iOSPushManager.shared.initialize()
    }

    /// Opens a lxapp
    public static func openLxApp(appId: String, path: String) {
        os_log("iOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)

        let instance = getInstance()
        LxAppCore.executeOpenLxApp(appId: appId, path: path, renderer: instance)
    }

    /// Opens the home mini app
    public static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            os_log("Home app details not available", log: log, type: .error)
            return
        }

        // Pass empty path - openLxApp will use initial route
        openLxApp(appId: homeLxAppId, path: "")
    }

    /// Closes a mini app with the specified appId
    public static func closeLxApp(appId: String) {
        os_log("Closing LxApp: %@", log: log, type: .info, appId)

        let instance = getInstance()
        instance.lxAppManager?.closeLxApp(appId: appId)
    }

    /// Navigate to a page with specific navigation type
    public static func navigate(appId: String, path: String, navigationType: NavigationType) {
        os_log("iOS navigate: %@ to %@ with type: %@", log: log, type: .info, appId, path, String(describing: navigationType))

        let instance = getInstance()
        LxAppCore.executeNavigation(appId: appId, path: path, navigationType: navigationType, renderer: instance)
    }

    /// Find WebView for the given appId and path
    internal static func findWebView(appId: String, path: String) -> WKWebView? {
        return WebViewManager.findWebView(appId: appId, path: path)
    }

    private func openLxAppInManager(appId: String, path: String) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("Failed to get window for presenting LxAppManager", log: Self.log, type: .error)
            return
        }

        let actualPath: String
        let storedPath = LxAppCore.getLastActivePath(for: appId, defaultPath: path)

        if storedPath != path && !LxAppCore.isHomeLxApp(appId) {
            actualPath = storedPath
        } else {
            actualPath = path
        }

        // Ensure LxAppManager exists
        if lxAppManager == nil {
            setupLxAppManager(window: window)
        }

        // Open LxApp in manager
        lxAppManager?.openLxApp(appId: appId, path: actualPath)
    }

    /// Sets up the single LxAppManager for all lxapps
    private func setupLxAppManager(window: UIWindow) {
        guard let currentRootVC = window.rootViewController else {
            // No existing root - create LxAppManager as root
            let manager = LxAppViewController()
            let navController = UINavigationController(rootViewController: manager)
            navController.setNavigationBarHidden(true, animated: false)
            window.rootViewController = navController
            window.makeKeyAndVisible()
            self.lxAppManager = manager
            return
        }

        // Find the topmost view controller to present from
        let topVC = findTopmostViewController(from: currentRootVC)

        if let existingManager = topVC as? LxAppViewController {
            // Already have LxAppManager - reuse it
            self.lxAppManager = existingManager
        } else if let existingNavController = topVC as? UINavigationController,
                  let existingManager = existingNavController.viewControllers.first as? LxAppViewController {
            // LxAppManager exists in navigation stack - reuse it
            self.lxAppManager = existingManager
        } else {
            // Present new LxAppManager modally
            os_log(.info, log: Self.log, "Presenting LxAppManager modally from: %{public}@", String(describing: type(of: topVC)))
            let manager = LxAppViewController()
            let navController = UINavigationController(rootViewController: manager)
            navController.setNavigationBarHidden(true, animated: false)
            navController.modalPresentationStyle = UIModalPresentationStyle.fullScreen
            topVC.present(navController, animated: false)
            self.lxAppManager = manager
        }
    }

    /// Finds the topmost view controller in the hierarchy
    private func findTopmostViewController(from viewController: UIViewController) -> UIViewController {
        if let presentedVC = viewController.presentedViewController {
            return findTopmostViewController(from: presentedVC)
        }

        if let navController = viewController as? UINavigationController,
           let topVC = navController.topViewController {
            return findTopmostViewController(from: topVC)
        }

        if let tabController = viewController as? UITabBarController,
           let selectedVC = tabController.selectedViewController {
            return findTopmostViewController(from: selectedVC)
        }

        return viewController
    }

    /// Configure transparent system bars for a specific view controller
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if let navController = viewController.navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    /// Configures global system bars for the mini app system
    private static func configureGlobalSystemBars() {
        let appearance = UINavigationBarAppearance()
        appearance.configureWithTransparentBackground()
        appearance.backgroundColor = UIColor.clear
        appearance.shadowColor = UIColor.clear

        UINavigationBar.appearance().standardAppearance = appearance
        UINavigationBar.appearance().compactAppearance = appearance
        UINavigationBar.appearance().scrollEdgeAppearance = appearance
        UINavigationBar.appearance().compactScrollEdgeAppearance = appearance
    }

    /// Get the current LxAppManager from the view hierarchy
    private static func getCurrentLxAppManager() -> LxAppViewController? {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            return nil
        }

        return findLxAppManager(in: window.rootViewController)
    }

    /// Recursively find iOSLxAppManager in the view hierarchy
    private static func findLxAppManager(in viewController: UIViewController?) -> LxAppViewController? {
        guard let viewController = viewController else { return nil }

        if let lxAppManager = viewController as? LxAppViewController {
            return lxAppManager
        }

        if let navController = viewController as? UINavigationController {
            return findLxAppManager(in: navController.topViewController)
        }

        if let presentedVC = viewController.presentedViewController {
            return findLxAppManager(in: presentedVC)
        }

        return nil
    }
}

extension iOSLxApp {
    /// Handle platform-specific openLxApp setup
    public func openLxApp(appId: String, path: String) {
        // Ensure LxAppManager exists for iOS
        setupLxAppManagerIfNeeded()

        // Open LxApp in manager
        lxAppManager?.openLxApp(appId: appId, path: path)
    }

    /// Render TabBar based on state
    public func renderTabBar(_ state: TabBarState, appId: String, path: String) {

        guard let manager = lxAppManager else { return }

        if state.show {
            manager.setupTabBar(appId: appId)
        } else {
            manager.currentTabBar?.isHidden = true
        }
    }

    /// Render NavigationBar based on state
    public func renderNavigationBar(_ state: NavBarState) {
        guard state.shouldUpdate else { return }
        lxAppManager?.updateNavigationBar(appId: state.appId, path: state.path)
    }

    /// Render Capsule button - only home app hides it
    public func renderCapsuleButton(appId: String) {
        lxAppManager?.updateCapsuleButtonVisibility(appId: appId)
    }

    /// Execute lifecycle action
    public func executeLifecycleAction(_ action: LifecycleAction, appId: String, path: String) {
        switch action {
        case .openApp:
            // onLxappOpened already called in prepareOpenLxApp
            lingxia.onPageShow(appId, path)
        case .switchTab:
            // TabSwitch is just like pageShow, but TabBar selection is handled in renderTabBar
            lingxia.onPageShow(appId, path)
        case .pageShow:
            lingxia.onPageShow(appId, path)
        case .backPressed:
            let _ = onUiEvent(appId, LxAppUIEvent.backPress, "")
        }
    }

    /// Handle platform-specific navigation logic
    public func handlePlatformSpecificNavigation(_ plan: NavigationPlan) {

        guard let manager = lxAppManager else { return }

        // Handle iOS-specific navigation setup - DO NOT call UI updates here
        // UI updates are handled by the unified renderer (renderNavigationBar, renderTabBar, etc.)
        if plan.navigationType != .launch {
            // Only update app state and WebView, no UI updates
            manager.updateAppStateForNavigation(appId: plan.appId, path: plan.path, navigationType: plan.navigationType)
            manager.setupOrSwitchWebView(appId: plan.appId, path: plan.path, navigationType: plan.navigationType)
        }
    }

    /// Get current path for duplicate navigation check
    public func getCurrentPath(for appId: String) -> String? {
        guard let manager = lxAppManager,
              let appState = manager.stateManager.getState(for: appId) else {
            return nil
        }

        return appState.webView?.currentPath
    }

    private func setupLxAppManagerIfNeeded() {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("Failed to get window for presenting LxAppManager", log: Self.log, type: .error)
            return
        }

        if lxAppManager == nil {
            setupLxAppManager(window: window)
        }
    }
}

#endif
