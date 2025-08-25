#if os(iOS)
import Foundation
import UIKit
import SwiftUI
import WebKit
import os.log
import CLingXiaFFI
import UserNotifications

/// iOS LxApp manager
@MainActor
public class iOSLxApp {
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "iOSLxApp")
    private static var instance: iOSLxApp?
    private let context: UIApplication

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

    /// Configure transparent system bars
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if let navController = viewController.navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    /// Opens a mini app in a new view controller
    public static func openLxApp(appId: String, path: String) {
        // Get app info and cache initial route for navigation logic
        let lxappInfo = getLxAppInfo(appId)
        let initialRoute = lxappInfo.initial_route.toString()
        LxPageNavigation.cacheInitialRoute(appId: appId, initialRoute: initialRoute)

        // Use initial route if path is empty
        let actualPath = path.isEmpty ? initialRoute : path

        let instance = getInstance()
        instance.openInNewViewController(appId: appId, path: actualPath)
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

        NotificationCenter.default.post(
            name: NSNotification.Name(ACTION_CLOSE_LXAPP),
            object: nil,
            userInfo: ["appId": appId]
        )
    }

    /// Switches the current page within a running LxAppViewController
    public static func switchPage(appId: String, path: String) {
        os_log("Switching page for %@ to %@", log: log, type: .info, appId, path)

        NotificationCenter.default.post(
            name: NSNotification.Name(ACTION_SWITCH_PAGE),
            object: nil,
            userInfo: ["appId": appId, "path": path]
        )
    }

    /// Find WebView for the given appId and path
    internal static func findWebView(appId: String, path: String) -> WKWebView? {
        return WebViewManager.findWebView(appId: appId, path: path)
    }

    private func openInNewViewController(appId: String, path: String) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("Failed to get window for presenting LxAppViewController", log: Self.log, type: .error)
            return
        }

        let actualPath: String
        let storedPath = LxAppCore.getLastActivePath(for: appId, defaultPath: path)

        if storedPath != path && !LxAppCore.isHomeLxApp(appId) {
            actualPath = storedPath
        } else {
            actualPath = path
        }

        // Call onLxappOpened FIRST to ensure WebView is created before we try to find it
        let _ = onLxappOpened(appId, actualPath)

        // Create LxAppViewController - it will find and setup WebView in viewDidLoad
        let miniAppVC = iOSLxAppViewController(appId: appId, path: actualPath)

        setupNavigationStack(window: window, newController: miniAppVC)
    }

    /// Sets up navigation stack for lxapp management
    private func setupNavigationStack(window: UIWindow, newController: iOSLxAppViewController) {

        guard let currentRootVC = window.rootViewController else {
            // No existing root - set as root
            let navController = UINavigationController(rootViewController: newController)
            navController.setNavigationBarHidden(true, animated: false)
            window.rootViewController = navController
            window.makeKeyAndVisible()
            return
        }

        // Find the topmost view controller to present from
        let topVC = findTopmostViewController(from: currentRootVC)

        if let existingNavController = topVC as? UINavigationController,
           existingNavController.viewControllers.first is iOSLxAppViewController {
            // Already in LxApp navigation - push new LxApp
            os_log(.info, log: Self.log, "Pushing new LxApp onto existing navigation stack")
            existingNavController.pushViewController(newController, animated: false)
        } else {
            // Present modally to preserve SwiftUI URL handling or stack on existing LxApp
            os_log(.info, log: Self.log, "Presenting LxApp modally from: %{public}@", String(describing: type(of: topVC)))
            let navController = UINavigationController(rootViewController: newController)
            navController.setNavigationBarHidden(true, animated: false)
            navController.modalPresentationStyle = .fullScreen
            topVC.present(navController, animated: false)
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

    /// Set badge text for a specific tab
    public static func setTabBarBadge(index: Int, text: String) {
        // Find the current LxAppViewController and delegate to its TabBar
        DispatchQueue.main.async {
            if let currentViewController = getCurrentLxAppViewController() {
                currentViewController.setTabBarBadge(index: index, text: text)
            }
        }
    }

    /// Remove badge from a specific tab
    public static func removeTabBarBadge(index: Int) {
        DispatchQueue.main.async {
            if let currentViewController = getCurrentLxAppViewController() {
                currentViewController.removeTabBarBadge(index: index)
            }
        }
    }

    /// Show red dot for a specific tab
    public static func showTabBarRedDot(index: Int) {
        DispatchQueue.main.async {
            if let currentViewController = getCurrentLxAppViewController() {
                currentViewController.showTabBarRedDot(index: index)
            }
        }
    }

    /// Hide red dot for a specific tab
    public static func hideTabBarRedDot(index: Int) {
        DispatchQueue.main.async {
            if let currentViewController = getCurrentLxAppViewController() {
                currentViewController.hideTabBarRedDot(index: index)
            }
        }
    }

    /// Get the current LxAppViewController from the view hierarchy
    private static func getCurrentLxAppViewController() -> iOSLxAppViewController? {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            return nil
        }

        return findLxAppViewController(in: window.rootViewController)
    }

    /// Recursively find iOSLxAppViewController in the view hierarchy
    private static func findLxAppViewController(in viewController: UIViewController?) -> iOSLxAppViewController? {
        guard let viewController = viewController else { return nil }

        if let lxAppVC = viewController as? iOSLxAppViewController {
            return lxAppVC
        }

        if let navController = viewController as? UINavigationController {
            return findLxAppViewController(in: navController.topViewController)
        }

        if let presentedVC = viewController.presentedViewController {
            return findLxAppViewController(in: presentedVC)
        }

        return nil
    }
}

/// Simple controller stack to simulate Android's Activity stack behavior
/// This helps maintain state when switching between lxapps
@MainActor
class LxAppControllerStack {

    /// Represents the state of a previous controller
    struct ControllerState {
        let appId: String
        let path: String
        let webView: WKWebView?
    }

    /// Stack to store previous controller states
    private static var controllerStack: [ControllerState] = []

    /// Push current controller state to stack before opening new lxapp
    static func pushCurrentController(appId: String, path: String, webView: WKWebView?) {
        let state = ControllerState(appId: appId, path: path, webView: webView)
        controllerStack.append(state)
    }

    /// Pop previous controller state when closing current lxapp
    static func popPreviousController() -> ControllerState? {
        guard !controllerStack.isEmpty else {
            return nil
        }

        let state = controllerStack.removeLast()
        return state
    }

    /// Clear the entire stack (useful for debugging or reset)
    static func clearStack() {
        controllerStack.removeAll()
    }
}

#endif
