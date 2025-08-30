#if os(iOS)
import Foundation
import UIKit
import SwiftUI
import WebKit
import os.log
import CLingXiaFFI
import UserNotifications

/// iOS LxApp manager - Single Controller Architecture
@MainActor
public class iOSLxApp {
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



    /// Opens a mini app using single controller architecture
    public static func openLxApp(appId: String, path: String) {
        // Get app info for initial route logic
        let lxappInfo = getLxAppInfo(appId)
        let initialRoute = lxappInfo.initial_route.toString()

        // Use initial route if path is empty
        let actualPath = path.isEmpty ? initialRoute : path

        let instance = getInstance()
        instance.openLxAppInManager(appId: appId, path: actualPath)
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
        os_log("Navigate for %@ to %@ with type %@", log: log, type: .info, appId, path, String(describing: navigationType))

        let instance = getInstance()
        instance.lxAppManager?.navigate(appId: appId, to: path, with: navigationType)
    }

    /// Switches the current page within a running LxApp (deprecated - use navigate instead)
    public static func switchPage(appId: String, path: String) {
        os_log("Switching page for %@ to %@ (deprecated)", log: log, type: .info, appId, path)

        let instance = getInstance()
        instance.lxAppManager?.navigate(appId: appId, to: path, with: NavigationType.forward)
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



#endif
