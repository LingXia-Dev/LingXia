import Foundation
import UIKit
import os.log

/// MiniApp launch mode
public enum MiniAppLaunchMode {
    /// Replace root view controller - for MiniApp as main app
    case replaceRoot
    /// Present modally - for MiniApp as sub-module in existing app
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
/// - WebView creation and management
/// - Page navigation coordination
/// - Integration with native layer
///
/// Usage:
/// ```swift
/// // Initialize the system
/// MiniApp.initialize()
///
/// // Open the home mini app
/// MiniApp.openHomeMiniApp()
/// ```
@MainActor
public class MiniApp {
    private static let log = OSLog(subsystem: "com.lingxia.miniapp", category: "MiniApp")

    /// Singleton instance
    private static var instance: MiniApp?

    /// Launch mode for MiniApp behavior
    private static var launchMode: MiniAppLaunchMode = .replaceRoot

    /// Home mini app identifier obtained from native initialization
    internal static var homeMiniAppId: String?

    /// Home mini app initial route obtained from native initialization
    private static var homeMiniAppInitialRoute: String?

    /// Application context
    private let context: UIApplication

    private init(context: UIApplication) {
        self.context = context
    }

    /// Initializes the MiniApp system
    ///
    /// This method must be called before any other MiniApp operations.
    /// It sets up the necessary infrastructure and obtains configuration
    /// from the native layer.
    ///
    /// - Parameter mode: Launch mode (.replaceRoot for main app, .modal for sub-module)
    /// - Warning: Must be called on the main thread
    public static func initialize(mode: MiniAppLaunchMode = .replaceRoot) {
        self.launchMode = mode

        guard Thread.isMainThread else {
            DispatchQueue.main.async {
                initialize(mode: mode)
            }
            return
        }

        instance = MiniApp(context: UIApplication.shared)
        configureGlobalSystemBars()

        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
        let cachesPath = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?.path ?? ""

        let initResultString = dummyNativeOnMiniAppInited(dataDir: documentsPath, cacheDir: cachesPath)

        if let initResult = initResultString {
            let parts = initResult.components(separatedBy: ":")
            if parts.count >= 2 {
                homeMiniAppId = parts[0]
                homeMiniAppInitialRoute = Array(parts[1...]).joined(separator: ":")
                os_log("Initialized with home app: %@ at %@", log: log, type: .info, homeMiniAppId!, homeMiniAppInitialRoute!)
            } else {
                os_log("Failed to parse home MiniApp details: %@", log: log, type: .error, initResult)
            }
        } else {
            os_log("Failed to get home MiniApp details from native init", log: log, type: .error)
        }
    }

    // Dummy native function - replace with actual native call
    private static func dummyNativeOnMiniAppInited(dataDir: String, cacheDir: String) -> String? {
        os_log("[DUMMY] Native init called with dataDir: %@ cacheDir: %@", log: log, type: .debug, dataDir, cacheDir)
        return "homeminiapp:pages/home/index.html"
    }

    private static func dummyNativeOnMiniAppOpened(appId: String, path: String) -> Int32 {
        os_log("[DUMMY] Native app opened: %@ at %@", log: log, type: .debug, appId, path)
        return 0
    }

    public static func dummyNativeGetTabBarConfig(appId: String) -> String? {
        let jsonString = """
        {
            "color": "#999999",
            "selectedColor": "#1677ff",
            "backgroundColor": "transparent",
            "borderStyle": "#eeeeee",
            "position": "bottom",
            "list": [
                {
                    "text": "Home",
                    "pagePath": "pages/home/index.html",
                    "iconPath": "house",
                    "selectedIconPath": "house.fill",
                    "selected": true
                },
                {
                    "text": "API",
                    "pagePath": "pages/API/index.html",
                    "iconPath": "globe",
                    "selectedIconPath": "globe"
                },
                {
                    "pagePath": "pages/todo/index.html",
                    "iconPath": "list.bullet",
                    "selectedIconPath": "list.bullet"
                }
            ]
        }
        """

        os_log("[DUMMY] Getting TabBar config for app: %@ - JSON: %@", log: log, type: .info, appId, jsonString)
        return jsonString
    }

    /// Gets the singleton MiniApp instance
    public static func getInstance() -> MiniApp {
        guard let instance = instance else {
            fatalError("MiniApp not initialized")
        }
        return instance
    }

    /// Opens a mini app in a new view controller
    ///
    /// This method creates and presents a new MiniAppViewController for the
    /// specified mini app. The view controller is presented modally from the
    /// current root view controller.
    ///
    /// - Parameters:
    ///   - appId: The unique identifier of the mini app
    ///   - path: The initial page path within the mini app
    /// - Note: Automatically ensures execution on main thread for UI operations
    public static func openMiniApp(appId: String, path: String) {
        if Thread.isMainThread {
            // Already on main thread, execute directly
            let instance = getInstance()
            instance.openInNewViewController(appId: appId, path: path)
        } else {
            // Not on main thread, dispatch to main thread
            DispatchQueue.main.async {
                let instance = getInstance()
                instance.openInNewViewController(appId: appId, path: path)
            }
        }
    }

    /// Opens the home mini app
    ///
    /// This method opens the home mini app using the configuration obtained
    /// during system initialization. If home app details are not available,
    /// an error is logged and no action is taken.
    ///
    /// - Note: The home app ID and route are set during initialize()
    public static func openHomeMiniApp() {
        if let homeMiniAppId = homeMiniAppId, let homeMiniAppInitialRoute = homeMiniAppInitialRoute {
            openMiniApp(appId: homeMiniAppId, path: homeMiniAppInitialRoute)
        } else {
            os_log("Home app details not available", log: log, type: .error)
        }
    }

    /**
     * Notifies the system to close a mini app with the specified appId
     * - Note: Automatically ensures execution on main thread for UI operations
     */
    public static func closeMiniApp(appId: String) {
            os_log("Closing MiniApp: %@", log: log, type: .info, appId)
        if Thread.isMainThread {
            // Already on main thread, execute directly
            NotificationCenter.default.post(
                name: NSNotification.Name(ACTION_CLOSE_MINIAPP),
                object: nil,
                userInfo: ["appId": appId]
            )
        } else {
            // Not on main thread, dispatch to main thread
            DispatchQueue.main.async {
                NotificationCenter.default.post(
                    name: NSNotification.Name(ACTION_CLOSE_MINIAPP),
                    object: nil,
                    userInfo: ["appId": appId]
                )
            }
        }
    }

    /**
     * Switches the current page within a running MiniAppViewController
     * - Note: Automatically ensures execution on main thread for UI operations
     */
    public static func switchPage(appId: String, path: String) {
        os_log("Switching page for %@ to %@", log: log, type: .info, appId, path)
        if Thread.isMainThread {
            // Already on main thread, execute directly
            NotificationCenter.default.post(
                name: NSNotification.Name(ACTION_SWITCH_PAGE),
                object: nil,
                userInfo: ["appId": appId, "path": path]
            )
        } else {
            // Not on main thread, dispatch to main thread
            DispatchQueue.main.async {
                NotificationCenter.default.post(
                    name: NSNotification.Name(ACTION_SWITCH_PAGE),
                    object: nil,
                    userInfo: ["appId": appId, "path": path]
                )
            }
        }
    }

    /**
     * Creates a WebView for the specified appId and path.
     * This method is called from the Rust layer to create WebViews.
     * - Note: Automatically ensures execution on main thread for UI operations
     */
    public static func createWebView(appId: String, path: String) -> LingXiaWebView? {
        if Thread.isMainThread {
            // Already on main thread, execute directly
            return createWebViewOnMainThread(appId: appId, path: path)
        } else {
            // Not on main thread, synchronously dispatch to main thread
            return DispatchQueue.main.sync {
                return createWebViewOnMainThread(appId: appId, path: path)
            }
        }
    }

    private static func createWebViewOnMainThread(appId: String, path: String) -> LingXiaWebView? {
        do {
            let webView = try LingXiaWebView.createWebView(appId: appId, path: path)
            os_log("Created WebView for %@ at %@", log: log, type: .info, appId, path)
            return webView
        } catch {
            os_log("Failed to create WebView for %@ at %@: %@", log: log, type: .error, appId, path, error.localizedDescription)
            return nil
        }
    }

    private func openInNewViewController(appId: String, path: String) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            os_log("Failed to get window for presenting MiniAppViewController", log: Self.log, type: .error)
            return
        }

        let miniAppVC = MiniAppViewController(appId: appId, path: path)

        switch MiniApp.launchMode {
        case .replaceRoot:
            window.rootViewController = miniAppVC
            window.makeKeyAndVisible()
        case .modal:
            if let rootVC = window.rootViewController {
                rootVC.present(miniAppVC, animated: true)
            } else {
                os_log("Failed to get root view controller for modal presentation", log: Self.log, type: .error)
                return
            }
        }

        Task {
            let _ = MiniApp.dummyNativeOnMiniAppOpened(appId: appId, path: path)
        }
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

        os_log("Global system bars configured for transparency", log: log, type: .info)
    }

    // Dummy native function - replace with actual native call
    public static func dummyNativeOnPageSwitched(appId: String, path: String) {
        os_log("[DUMMY] Page switched for %@ to %@", log: log, type: .debug, appId, path)
    }

    public static func dummyNativeOnAppClosed(appId: String) {
        os_log("[DUMMY] App closed: %@", log: log, type: .debug, appId)
    }
}
