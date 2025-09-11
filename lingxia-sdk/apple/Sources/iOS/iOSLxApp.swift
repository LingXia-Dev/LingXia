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
        if instance != nil { return }

        instance = iOSLxApp(context: UIApplication.shared)
        LxAppCore.initializeCore()
        configureGlobalSystemBars()
        iOSPushManager.shared.initialize()
    }

    /// Opens a lxapp
    public static func openLxApp(appId: String, path: String) {
        os_log("iOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)
        LxAppPlatformOperations.openLxApp(appId: appId, path: path, renderer: getInstance())
    }

    /// Opens the home mini app
    public static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            os_log("Home app details not available", log: log, type: .error)
            return
        }
        openLxApp(appId: homeLxAppId, path: "")
    }

    /// Closes a mini app with the specified appId
    public static func closeLxApp(appId: String) {
        os_log("Closing LxApp: %@", log: log, type: .info, appId)
        getInstance().lxAppManager?.closeLxApp(appId: appId)
    }

    /// Navigate to a page with specific navigation type
    public static func navigate(appId: String, path: String, navigationType: NavigationType) {
        os_log("iOS navigate: %@ to %@ with type: %@", log: log, type: .info, appId, path, String(describing: navigationType))
        LxAppPlatformOperations.navigate(appId: appId, path: path, navigationType: navigationType, renderer: getInstance())
    }

    /// Find WebView for the given appId and path
    internal static func findWebView(appId: String, path: String) -> WKWebView? {
        return LxAppPlatformOperations.findWebView(appId: appId, path: path)
    }

    private func openLxAppInManager(appId: String, path: String) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("Failed to get window for presenting LxAppManager", log: Self.log, type: .error)
            return
        }

        // Use the provided path directly since we now have centralized state management
        let actualPath = path

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
        return LxAppViewHierarchyHelper.findTopmostViewController(from: viewController)
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
        return LxAppViewHierarchyHelper.findSpecificViewController(in: viewController)
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

    /// Render TabBar based on state (visibility determined in prepareNavigation)
    public func renderTabBar(_ state: TabBarState, appId: String, path: String) {
        guard let manager = lxAppManager else { return }

        manager.setupTabBar(appId: appId)

        // Use state visibility (already determined in prepareNavigation)
        if state.show {
            manager.currentTabBar?.syncSelectedTabWithCurrentPath(path)
        }
        manager.showTabBar(state.show)
    }

    /// Render NavigationBar based on state
    public func renderNavigationBar(_ state: NavBarState) {
        guard state.shouldUpdate else { return }
        lxAppManager?.updateNavigationBar(appId: state.appId, path: state.path)
    }

    /// Render Capsule button - only home app hides it
    public func renderCapsuleButton(appId: String) {
    }

    /// Handle platform-specific navigation logic
    public func handlePlatformSpecificNavigation(_ plan: NavigationPlan) {

        guard let manager = lxAppManager else { return }
        manager.setupOrSwitchWebView(appId: plan.appId, path: plan.path, navigationType: plan.navigationType)
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
