import Foundation
import UIKit
import os.log
import CLingXiaFFI

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
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "MiniApp")

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
        if homeMiniAppId != nil {
            os_log("MiniApp.initialize() already called (homeMiniAppId exists), skipping", log: log, type: .info)
            return
        }

        self.launchMode = mode
        performInitialization(mode: mode)
    }

    private static func performInitialization(mode: MiniAppLaunchMode) {
        instance = MiniApp(context: UIApplication.shared)
        configureGlobalSystemBars()

        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
        let cachesPath = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?.path ?? ""

        let initResult = lingxia.miniappInit(documentsPath, cachesPath)
        let initResultString = initResult?.toString()

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
    nonisolated public static func openMiniApp(appid: RustStr, path: RustStr) -> Bool {
        let appidString = appid.toString()
        let pathString = path.toString()
        // Always dispatch to main thread for UI operations
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
    public static func openHomeMiniApp() {
        if let homeMiniAppId = homeMiniAppId, let homeMiniAppInitialRoute = homeMiniAppInitialRoute {
            os_log("Opening home app: %@ at %@", log: log, type: .info, homeMiniAppId, homeMiniAppInitialRoute)
            _ = homeMiniAppId.toRustStr { appidRustStr in
                homeMiniAppInitialRoute.toRustStr { pathRustStr in
                    MiniApp.openMiniApp(appid: appidRustStr, path: pathRustStr)
                }
            }
        } else {
            os_log("Home app details not available", log: log, type: .error)
        }
    }

    /**
     * Notifies the system to close a mini app with the specified appId
     * - Note: Automatically ensures execution on main thread for UI operations
     */
    nonisolated public static func closeMiniApp(appid: RustStr) -> Bool {
        let appidString = appid.toString()
            os_log("Closing MiniApp: %@", log: log, type: .info, appidString)
        // Always dispatch to main thread for notification posting
        DispatchQueue.main.async {
            NotificationCenter.default.post(
                name: NSNotification.Name("com.lingxia.CLOSE_MINIAPP_ACTION"),
                object: nil,
                userInfo: ["appId": appidString]
            )
        }
        return true
    }

    /**
     * Switches the current page within a running MiniAppViewController
     * - Note: Automatically ensures execution on main thread for UI operations
     */
    nonisolated public static func switchPage(appid: RustStr, path: RustStr) -> Bool {
        let appidString = appid.toString()
        let pathString = path.toString()
        os_log("Switching page for %@ to %@", log: log, type: .info, appidString, pathString)
        // Always dispatch to main thread for notification posting
        DispatchQueue.main.async {
            NotificationCenter.default.post(
                name: NSNotification.Name("com.lingxia.SWITCH_PAGE_ACTION"),
                object: nil,
                userInfo: ["appId": appidString, "path": pathString]
            )
        }
        return true
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

        // Asynchronously notify Rust to initialize the MiniApp
        Task {
            lingxia.onMiniappOpened(appId, path)
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
    }
}

/// Get device model (e.g., "iPhone14,2")
func getDeviceModel() -> String {
    var systemInfo = utsname()
    uname(&systemInfo)
    let machineMirror = Mirror(reflecting: systemInfo.machine)
    let identifier = machineMirror.children.reduce("") { identifier, element in
        guard let value = element.value as? Int8, value != 0 else { return identifier }
        return identifier + String(UnicodeScalar(UInt8(value)))
    }

    return identifier
}

/// Get system version (e.g., "17.0")
nonisolated func getSystemVersion() -> String {
    return DispatchQueue.main.sync {
        UIDevice.current.systemVersion
    }
}
