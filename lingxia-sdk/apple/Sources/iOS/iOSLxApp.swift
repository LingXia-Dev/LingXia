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
public class iOSLxApp {
    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "iOSLxApp")
    nonisolated(unsafe) private static var instance: iOSLxApp?
    private let context: UIApplication

    /// Single manager instance for all LxApps
    private var lxAppManager: LxAppViewController?

    /// Lifecycle event observers
    private var lifecycleObservers: [NSObjectProtocol] = []
    private var lastDeviceOrientationValue: String?

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
    
    /// Gets the singleton instance in a non-isolated context (for FFI bridges)
    nonisolated static func getInstanceUnsafe() -> iOSLxApp? {
        return instance
    }

    /// Gets the current LxAppViewController (for internal use)
    internal var currentLxAppManager: LxAppViewController? {
        return lxAppManager
    }

    /// Initialize the iOS LxApp system
    public static func initialize() {
        if instance != nil { return }

        instance = iOSLxApp(context: UIApplication.shared)
        LxAppCore.initializeCore()
        configureGlobalSystemBars()
        iOSPushManager.shared.initialize()

        // Setup lifecycle observers
        instance?.setupLifecycleObservers()
    }

    /// Setup observers for app lifecycle events
    private func setupLifecycleObservers() {
        UIDevice.current.beginGeneratingDeviceOrientationNotifications()

        // App entered foreground
        let foregroundObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.willEnterForegroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleAppShow()
        }
        lifecycleObservers.append(foregroundObserver)

        // App entered background
        let backgroundObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleAppHide()
        }
        lifecycleObservers.append(backgroundObserver)

        // User took screenshot
        let screenshotObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.userDidTakeScreenshotNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleUserCaptureScreen()
        }
        lifecycleObservers.append(screenshotObserver)

        // Device orientation changed
        let orientationObserver = NotificationCenter.default.addObserver(
            forName: UIDevice.orientationDidChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleDeviceOrientationChange()
        }
        lifecycleObservers.append(orientationObserver)
    }

    /// Handle app entering foreground
    private func handleAppShow() {
        guard let currentAppId = LxAppCore.currentAppId else { return }
        os_log("App entering foreground, notifying appId: %@", log: Self.log, type: .info, currentAppId)
        lingxia.onAppShow(currentAppId)
    }

    /// Handle app entering background
    private func handleAppHide() {
        guard let currentAppId = LxAppCore.currentAppId else { return }
        os_log("App entering background, notifying appId: %@", log: Self.log, type: .info, currentAppId)
        lingxia.onAppHide(currentAppId)
    }

    /// Handle user taking screenshot
    private func handleUserCaptureScreen() {
        guard let currentAppId = LxAppCore.currentAppId else { return }
        os_log("User captured screenshot, notifying appId: %@", log: Self.log, type: .info, currentAppId)
        lingxia.onUserCaptureScreen(currentAppId)
    }

    /// Handle device orientation changes and forward to runtime event bus.
    private func handleDeviceOrientationChange() {
        guard let currentAppId = LxAppCore.currentAppId else { return }
        guard let sessionId = LxAppCore.sessionId(for: currentAppId), sessionId > 0 else { return }

        let value: String?
        switch UIDevice.current.orientation {
        case .portrait, .portraitUpsideDown:
            value = "portrait"
        case .landscapeLeft, .landscapeRight:
            value = "landscape"
        default:
            value = nil
        }

        guard let orientationValue = value else { return }
        if lastDeviceOrientationValue == orientationValue {
            return
        }

        let accepted = lingxia.onDeviceOrientationChanged(currentAppId, sessionId, orientationValue)
        if accepted {
            lastDeviceOrientationValue = orientationValue
        }
    }

    /// Opens a lxapp
    public static func openLxApp(appId: String, path: String, sessionId: UInt64) {
        os_log("iOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)
        LxAppCore.executeOpenLxApp(appId: appId, path: path, sessionId: sessionId)
    }

    /// Opens the home mini app
    public static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            os_log("Home app details not available", log: log, type: .error)
            return
        }
        let current = getCurrentLxApp()
        let currentAppId = current.appid.toString()
        let sessionId: UInt64 = (currentAppId == homeLxAppId)
            ? current.session_id
            : (LxAppCore.sessionId(for: homeLxAppId) ?? 0)
        guard sessionId > 0 else {
            os_log("Invalid home app session for %@", log: log, type: .error, homeLxAppId)
            return
        }
        openLxApp(appId: homeLxAppId, path: "", sessionId: sessionId)
    }

    /// Closes a mini app with the specified appId
    public static func closeLxApp(appId: String, sessionId: UInt64) {
        os_log("Closing LxApp: %@", log: log, type: .info, appId)
        getInstance().lxAppManager?.closeLxApp(appId: appId, sessionId: sessionId)
    }

    /// Navigate to a page with specific animation type
    public static func navigate(appId: String, path: String, animationType: AnimationType) {
        os_log("iOS navigate: %@ to %@ with type: %@", log: log, type: .info, appId, path, String(describing: animationType))
        LxAppCore.executeNavigation(appId: appId, path: path, animationType: animationType)
    }

    /// Find WebView for the given appId and path
    internal static func findWebView(appId: String, path: String, sessionId: UInt64) -> WKWebView? {
        return WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId)
    }

    private func openLxAppInManager(appId: String, path: String, sessionId: UInt64) {
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
        lxAppManager?.openLxApp(appId: appId, path: actualPath, sessionId: sessionId)
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
    /// Direct openLxApp implementation (called from LxAppCore)
    internal static func openLxAppDirect(appId: String, path: String, sessionId: UInt64) {
        let instance = getInstance()

        // Ensure LxAppManager exists for iOS
        instance.setupLxAppManagerIfNeeded()

        // Open LxApp in manager
        instance.lxAppManager?.openLxApp(appId: appId, path: path, sessionId: sessionId)
    }

    /// Direct navigation implementation (called from LxAppCore)
    internal static func handleNavigationDirect(appId: String, path: String, animationType: AnimationType) {
        let instance = getInstance()
        guard let manager = instance.lxAppManager else { return }

        // Platform-specific setup/switch WebView - this will handle all UI updates internally
        manager.handleNavigation(appId: appId, path: path, animationType: animationType)
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

// MARK: - Pull-to-Refresh Bridge Functions

extension LxApp {
    /// Start pull-to-refresh animation programmatically
    @objc nonisolated public static func startPullDownRefresh(appid: RustStr, path: RustStr) -> Bool {
        let appidStr = appid.toString()
        let pathStr = path.toString()
        
        DispatchQueue.main.async {
            // Access instance through a non-isolated path
            guard let instance = iOSLxApp.getInstanceUnsafe() else { return }
            guard let manager = instance.currentLxAppManager else { return }
            
            manager.startPullDownRefreshProgrammatically()
            
            os_log("startPullDownRefresh called for %@:%@", log: OSLog(subsystem: "LingXia", category: "PullToRefresh"), type: .info, appidStr, pathStr)
        }
        return true
    }

    /// Stop pull-to-refresh animation
    @objc nonisolated public static func stopPullDownRefresh(appid: RustStr, path: RustStr) -> Bool {
        let appidStr = appid.toString()
        let pathStr = path.toString()
        
        DispatchQueue.main.async {
            // Access instance through a non-isolated path
            guard let instance = iOSLxApp.getInstanceUnsafe() else { return }
            guard let manager = instance.currentLxAppManager else { return }
            
            manager.stopPullDownRefreshProgrammatically()
            
            os_log("stopPullDownRefresh called for %@:%@", log: OSLog(subsystem: "LingXia", category: "PullToRefresh"), type: .info, appidStr, pathStr)
        }
        return true
    }
}

#endif
