#if os(iOS)
import Foundation
import UIKit
import SwiftUI
import WebKit
import os.log
import CLingXiaFFI

/// Presentation modes for LxApp in SwiftUI (iOS only)
internal enum LxAppPresentationMode {
    case replaceRoot
    case modal
    case sheet
    case fullScreenCover
}

/// iOS LxApp manager with SwiftUI integration
@MainActor
public class iOSLxApp: ObservableObject {
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "iOSLxApp")
    private static var instance: iOSLxApp?
    private let context: UIApplication

    // SwiftUI state management
    @Published internal var currentAppId: String?
    @Published internal var currentPath: String?
    @Published internal var isLxAppPresented: Bool = false
    @Published internal var presentationMode: LxAppPresentationMode = .replaceRoot

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

        // Use replaceRoot mode (only supported mode)
        setupNavigationStack(window: window, newController: miniAppVC)
    }

    /// Sets up navigation stack for lxapp management
    private func setupNavigationStack(window: UIWindow, newController: iOSLxAppViewController) {
        if let currentRootVC = window.rootViewController {
            if let navController = currentRootVC as? UINavigationController {
                navController.pushViewController(newController, animated: false)
            } else if let currentLxAppVC = currentRootVC as? iOSLxAppViewController {
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
            // Always wrap in UINavigationController to enable transparent system bars
            let navController = UINavigationController(rootViewController: newController)
            navController.setNavigationBarHidden(true, animated: false)
            window.rootViewController = navController

            // Try to make window cover status bar
            window.windowLevel = UIWindow.Level.statusBar - 1
            window.backgroundColor = UIColor.clear
            window.isOpaque = false

            window.makeKeyAndVisible()
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
