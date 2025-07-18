import Foundation
import UIKit
import WebKit
import os.log
import CLingXiaFFI

/// LxApp launch mode
public enum LxAppLaunchMode {
    /// Replace root view controller - for LxApp as main app
    case replaceRoot
    /// Present modally - for LxApp as sub-module in existing app
    case modal
}

/// Notification action identifier for mini app page switching
public let ACTION_SWITCH_PAGE = "com.lingxia.SWITCH_PAGE_ACTION"

/// Notification action identifier for mini app close requests
public let ACTION_CLOSE_MINIAPP = "com.lingxia.CLOSE_MINIAPP_ACTION"

/// Central manager for the mini app system
///
/// This class provides the main interface for initializing and managing mini apps.
/// It handles system initialization, app launching, and lifecycle management.
/// It uses the iOS NotificationCenter for inter-component communication.
///
/// Features:
/// - Singleton pattern for system-wide access
/// - Home mini app configuration management
/// - Page navigation coordination
/// - Integration with native layer
///
/// Usage:
/// ```swift
/// // Initialize the system
/// LxApp.initialize()
///
/// // Open the home mini app
/// LxApp.openHomeLxApp()
/// ```
@MainActor
public class LxApp {
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "LxApp")

    /// Singleton instance
    private static var instance: LxApp?

    /// Launch mode for LxApp behavior
    private static var launchMode: LxAppLaunchMode = .replaceRoot

    /// Home mini app identifier obtained from native initialization
    internal static var homeLxAppId: String?

    /// Home mini app initial route obtained from native initialization
    internal static var homeLxAppInitialRoute: String?

    /// Storage for last active paths of miniapps to restore state when reopening
    private static var lastActivePaths: [String: String] = [:]

    /// Application context
    private let context: UIApplication

    private init(context: UIApplication) {
        self.context = context
    }

    /// Initializes the LxApp system
    ///
    /// This method must be called before any other LxApp operations.
    /// It sets up the necessary infrastructure and obtains configuration
    /// from the native layer.
    ///
    /// - Parameter mode: Launch mode (.replaceRoot for main app, .modal for sub-module)
    /// - Warning: Must be called on the main thread
    public static func initialize(mode: LxAppLaunchMode = .replaceRoot) {
        if homeLxAppId != nil {
            os_log("LxApp.initialize() already called (homeLxAppId exists), skipping", log: log, type: .info)
            return
        }

        self.launchMode = mode
        performInitialization(mode: mode)
    }

    private static func performInitialization(mode: LxAppLaunchMode) {
        instance = LxApp(context: UIApplication.shared)
        configureGlobalSystemBars()

        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
        let cachesPath = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?.path ?? ""

        let initResult = lingxia.lxappInit(documentsPath, cachesPath)
        let initResultString = initResult?.toString()

        if let initResult = initResultString {
            let parts = initResult.components(separatedBy: ":")
            if parts.count >= 2 {
                homeLxAppId = parts[0]
                homeLxAppInitialRoute = Array(parts[1...]).joined(separator: ":")
                os_log("Initialized with home app: %@ at %@", log: log, type: .info, homeLxAppId!, homeLxAppInitialRoute!)
            } else {
                os_log("Failed to parse home LxApp details: %@", log: log, type: .error, initResult)
            }
        } else {
            os_log("Failed to get home LxApp details from native init", log: log, type: .error)
        }
    }

    /// Gets the singleton LxApp instance
    public static func getInstance() -> LxApp {
        guard let instance = instance else {
            fatalError("LxApp not initialized")
        }
        return instance
    }

    /// Opens a mini app in a new view controller
    ///
    /// This method creates and presents a new LxAppViewController for the
    /// specified mini app. The view controller is presented modally from the
    /// current root view controller.
    ///
    /// - Parameters:
    ///   - appId: The unique identifier of the mini app
    ///   - path: The initial page path within the mini app
    /// - Note: Automatically ensures execution on main thread for UI operations
    nonisolated public static func openLxApp(appid: RustStr, path: RustStr) -> Bool {
        let appidString = appid.toString()
        let pathString = path.toString()
        os_log("Opening app: %@ at %@", log: log, type: .info, appidString, pathString)

        DispatchQueue.main.async {
            let instance = getInstance()
            instance.openInNewViewController(appId: appidString, path: pathString)
        }
        return true
    }

    /// Opens the home mini app
    ///
    /// This method opens the home mini app using the configuration obtained
    /// during system initialization. If home app details are not available,
    /// an error is logged and no action is taken.
    ///
    /// - Note: The home app ID and route are set during initialize()
    public static func openHomeLxApp() {
        if let homeLxAppId = homeLxAppId, let homeLxAppInitialRoute = homeLxAppInitialRoute {
            _ = homeLxAppId.toRustStr { appidRustStr in
                homeLxAppInitialRoute.toRustStr { pathRustStr in
                    LxApp.openLxApp(appid: appidRustStr, path: pathRustStr)
                }
            }
        } else {
            os_log("Home app details not available", log: log, type: .error)
        }
    }

    /// Closes a mini app with the specified appId (called from Rust layer)
    /// This is the main API for programmatic miniapp closure
    nonisolated public static func closeLxApp(appid: RustStr) -> Bool {
        let appidString = appid.toString()
        os_log("Closing LxApp: %@", log: log, type: .info, appidString)

        DispatchQueue.main.async {
            NotificationCenter.default.post(
                name: NSNotification.Name(ACTION_CLOSE_MINIAPP),
                object: nil,
                userInfo: ["appId": appidString]
            )
        }
        return true
    }

    /**
     * Switches the current page within a running LxAppViewController
     * - Note: Automatically ensures execution on main thread for UI operations
     */
    nonisolated public static func switchPage(appid: RustStr, path: RustStr) -> Bool {
        let appidString = appid.toString()
        let pathString = path.toString()
        os_log("Switching page for %@ to %@", log: log, type: .info, appidString, pathString)
        // Always dispatch to main thread for notification posting
        DispatchQueue.main.async {
            NotificationCenter.default.post(
                name: NSNotification.Name(ACTION_SWITCH_PAGE),
                object: nil,
                userInfo: ["appId": appidString, "path": pathString]
            )
        }
        return true
    }

    /// Stores the last active path for a miniapp to enable state restoration
    internal static func storeLastActivePath(appId: String, path: String) {
        lastActivePaths[appId] = path
    }

    /// Retrieves the last active path for a miniapp, or returns the initial route if none stored
    internal static func getLastActivePath(appId: String, defaultPath: String) -> String {
        let storedPath = lastActivePaths[appId] ?? defaultPath
        return storedPath
    }

    // Note: WebView creation is now handled directly by Rust layer using objc2
    // Swift only needs to find and manage existing WebViews

    private func openInNewViewController(appId: String, path: String) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("Failed to get window for presenting LxAppViewController", log: Self.log, type: .error)
            return
        }

        let actualPath: String
        let storedPath = LxApp.getLastActivePath(appId: appId, defaultPath: path)

        if storedPath != path && appId != LxApp.homeLxAppId {
            actualPath = storedPath
            os_log("openInNewViewController: Using stored path for state restoration: %@ (requested: %@)",
                   log: Self.log, type: .info, actualPath, path)
        } else {
            actualPath = path
            os_log("openInNewViewController: Using requested path: %@", log: Self.log, type: .info, actualPath)
        }

        // Call onLxappOpened FIRST to ensure WebView is created before we try to find it
        let openResult = lingxia.onLxappOpened(appId, actualPath)
        os_log("onLxappOpened completed with result=%d for appId=%@ path=%@", log: Self.log, type: .info, openResult, appId, actualPath)

        // Create LxAppViewController - it will find and setup WebView in viewDidLoad
        let miniAppVC = LxAppViewController(appId: appId, path: actualPath)

        switch LxApp.launchMode {
        case .replaceRoot:
            setupNavigationStack(window: window, newController: miniAppVC)
        case .modal:
            if let rootVC = window.rootViewController {
                rootVC.present(miniAppVC, animated: true)
            } else {
                os_log("Failed to get root view controller for modal presentation", log: Self.log, type: .error)
                return
            }
        }
    }

    /// Sets up navigation stack for miniapp management
    private func setupNavigationStack(window: UIWindow, newController: LxAppViewController) {
        if let currentRootVC = window.rootViewController {
            if let navController = currentRootVC as? UINavigationController {
                navController.pushViewController(newController, animated: false)
            } else if let currentLxAppVC = currentRootVC as? LxAppViewController {
                window.rootViewController = nil
                let navController = UINavigationController(rootViewController: currentLxAppVC)
                navController.setNavigationBarHidden(true, animated: false)
                navController.pushViewController(newController, animated: false)
                window.rootViewController = navController
                window.makeKeyAndVisible()
            } else {
                window.rootViewController = newController
                window.makeKeyAndVisible()
            }
        } else {
            window.rootViewController = newController
            window.makeKeyAndVisible()
        }
    }



    /// Find WebView for the given appId and path
    internal static func findWebView(appId: String, path: String) -> WKWebView? {
        return WebViewManager.findWebView(appId: appId, path: path)
    }

    /// Configures global system bars for the mini app system
    ///
    /// This method sets up application-wide appearance for transparent system bars
    /// to support the mini app overlay system.
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
}

/// Simple controller stack to simulate Android's Activity stack behavior
/// This helps maintain state when switching between miniapps
@MainActor
class LxAppControllerStack {
    private static let log = OSLog(subsystem: "LingXia", category: "ControllerStack")

    /// Represents the state of a previous controller
    struct ControllerState {
        let appId: String
        let path: String
        let webView: WKWebView?
    }

    /// Stack to store previous controller states
    private static var controllerStack: [ControllerState] = []

    /// Push current controller state to stack before opening new miniapp
    static func pushCurrentController(appId: String, path: String, webView: WKWebView?) {
        let state = ControllerState(appId: appId, path: path, webView: webView)
        controllerStack.append(state)
        os_log("pushCurrentController: Pushed controller for %@ at %@ (stack size: %d)",
               log: log, type: .info, appId, path, controllerStack.count)
    }

    /// Pop previous controller state when closing current miniapp
    static func popPreviousController() -> ControllerState? {
        guard !controllerStack.isEmpty else {
            os_log("popPreviousController: Stack is empty", log: log, type: .info)
            return nil
        }

        let state = controllerStack.removeLast()
        os_log("popPreviousController: Popped controller for %@ at %@ (stack size: %d)",
               log: log, type: .info, state.appId, state.path, controllerStack.count)
        return state
    }

    /// Clear the entire stack (useful for debugging or reset)
    static func clearStack() {
        controllerStack.removeAll()
        os_log("clearStack: Cleared controller stack", log: log, type: .info)
    }
}
