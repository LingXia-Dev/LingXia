#if os(iOS)
import Foundation
import UIKit
import WebKit
import os.log
import CLingXiaFFI

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
            os_log("iOSLxApp.initialize() already called, skipping", log: log, type: .info)
            return
        }

        instance = iOSLxApp(context: UIApplication.shared)

        // Set platform directory provider
        LxAppCore.setPlatformDirectoryProvider(iOSDirectoryProvider.self)

        // Initialize core system
        LxAppCore.initialize()

        configureGlobalSystemBars()
    }

    /// Configure transparent system bars (iOS only)
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        if #available(iOS 13.0, *) {
            viewController.overrideUserInterfaceStyle = lightStatusBarIcons ? .light : .dark
        }

        if let navController = viewController.navigationController {
            navController.navigationBar.setBackgroundImage(UIImage(), for: .default)
            navController.navigationBar.shadowImage = UIImage()
            navController.navigationBar.isTranslucent = true
        }
    }

    /// Opens a mini app in a new view controller
    public static func openLxApp(appId: String, path: String) {
        let instance = getInstance()
        instance.openInNewViewController(appId: appId, path: path)
    }

    /// Opens the home mini app
    public static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            os_log("Home app details not available", log: log, type: .error)
            return
        }

        let homeLxAppInitialRoute = LxAppCore.getHomeLxAppInitialRoute()
        openLxApp(appId: homeLxAppId, path: homeLxAppInitialRoute)
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

    /// Open specific LxApp (FFI compatible version)
    nonisolated public static func openLxApp(appid: RustStr, path: RustStr) -> Bool {
        let appId = appid.toString()
        let pathString = path.toString()

        if Thread.isMainThread {
            MainActor.assumeIsolated {
                openLxApp(appId: appId, path: pathString)
            }
        } else {
            DispatchQueue.main.sync {
                openLxApp(appId: appId, path: pathString)
            }
        }
        return true
    }

    /// Close LxApp (FFI compatible version)
    nonisolated public static func closeLxApp(appid: RustStr) -> Bool {
        let appId = appid.toString()
        if Thread.isMainThread {
            MainActor.assumeIsolated {
                closeLxApp(appId: appId)
            }
        } else {
            DispatchQueue.main.sync {
                closeLxApp(appId: appId)
            }
        }
        return true
    }

    /// Switch to page in LxApp (FFI compatible version)
    nonisolated public static func switchPage(appid: RustStr, path: RustStr) -> Bool {
        let appId = appid.toString()
        let pathString = path.toString()
        if Thread.isMainThread {
            MainActor.assumeIsolated {
                switchPage(appId: appId, path: pathString)
            }
        } else {
            DispatchQueue.main.sync {
                switchPage(appId: appId, path: pathString)
            }
        }
        return true
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
            os_log("openInNewViewController: Using stored path for state restoration: %@ (requested: %@)",
                   log: Self.log, type: .info, actualPath, path)
        } else {
            actualPath = path
            os_log("openInNewViewController: Using requested path: %@", log: Self.log, type: .info, actualPath)
        }

        // Call onLxappOpened FIRST to ensure WebView is created before we try to find it
        let openResult = onLxappOpened(appId, actualPath)
        os_log("onLxappOpened completed with result=%d for appId=%@ path=%@", log: Self.log, type: .info, openResult, appId, actualPath)

        // Create LxAppViewController - it will find and setup WebView in viewDidLoad
        let miniAppVC = iOSLxAppViewController(appId: appId, path: actualPath)

        switch LxAppCore.getLaunchMode() {
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
            window.rootViewController = newController
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
#endif
