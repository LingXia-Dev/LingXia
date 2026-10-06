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
class iOSLxApp {
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
    static func getInstance() -> iOSLxApp? {
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
    static func initialize(autoOpenHome: Bool = true) {
        if instance != nil { return }

        instance = iOSLxApp(context: UIApplication.shared)
        LxAppCore.initializeCore(autoOpenHome: autoOpenHome)
        configureGlobalSystemBars()
        if (LxAppCore.capabilities & LxAppCore.capNotifications) != 0 {
            iOSPushManager.shared.initialize()
        }

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

        observeHostForeground()

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

    /// Process-level foreground for `lingxia::app::is_foreground`, separate
    /// from the per-lxapp show/hide above. Foreground while any connected
    /// scene is foregroundActive or foregroundInactive, so a transient
    /// inactive state (system alert, VPN prompt, Control Center) is not a
    /// background. Non-scene hosts fall back to the UIApplication events.
    /// Rust drops repeats, so overlapping notifications are harmless.
    private func observeHostForeground() {
        let names: [Notification.Name] = [
            UIScene.willEnterForegroundNotification,
            UIScene.didActivateNotification,
            UIScene.didEnterBackgroundNotification,
            UIScene.didDisconnectNotification,
            UIApplication.willEnterForegroundNotification,
            UIApplication.didEnterBackgroundNotification,
        ]
        for name in names {
            let observer = NotificationCenter.default.addObserver(
                forName: name,
                object: nil,
                queue: .main
            ) { [weak self] note in
                // Only Sendable values cross into the main actor.
                let name = note.name
                let scene = (note.object as AnyObject?).map(ObjectIdentifier.init)
                // queue: .main delivers on the main thread.
                MainActor.assumeIsolated {
                    self?.reportHostForeground(name, scene: scene)
                }
            }
            lifecycleObservers.append(observer)
        }
        // A background launch (push, VPN on demand) starts in the background.
        Self.reportHostForeground(
            context.applicationState != .background || Self.anySceneForeground(excluding: nil)
        )
    }

    private func reportHostForeground(_ name: Notification.Name, scene: ObjectIdentifier?) {
        let foreground: Bool
        switch name {
        case UIScene.willEnterForegroundNotification,
             UIScene.didActivateNotification,
             UIApplication.willEnterForegroundNotification:
            foreground = true
        case UIApplication.didEnterBackgroundNotification:
            foreground = false
        default:
            foreground = Self.anySceneForeground(excluding: scene)
        }
        Self.reportHostForeground(foreground)
    }

    /// The value last sent to Rust, so one transition takes one task even
    /// though the scene and the application both report it.
    private static var reportedForeground: Bool?

    /// A move to the background takes a background task first, so Rust's
    /// `watch_foreground` callbacks run before iOS suspends the process.
    /// Rust hands the token back through `LxApp.endBackgroundGrace` once they
    /// have all returned.
    private static func reportHostForeground(_ foreground: Bool) {
        let entersBackground = !foreground && reportedForeground != false
        reportedForeground = foreground
        lingxia.onHostForegroundChanged(foreground, entersBackground ? HostBackgroundGrace.begin() : 0)
    }

    /// Whether a scene other than `excluded` is in the foreground. Without
    /// scenes this is the application state.
    private static func anySceneForeground(excluding excluded: ObjectIdentifier?) -> Bool {
        let scenes = UIApplication.shared.connectedScenes.filter { ObjectIdentifier($0) != excluded }
        if UIApplication.shared.connectedScenes.isEmpty {
            return UIApplication.shared.applicationState != .background
        }
        return scenes.contains {
            $0.activationState == .foregroundActive || $0.activationState == .foregroundInactive
        }
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
    static func openLxApp(appId: String, path: String, pageInstanceId: String?, sessionId: UInt64) {
        os_log("iOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)
        _ = LxAppCore.executeOpenLxApp(
            appId: appId,
            path: path,
            sessionId: sessionId,
            pageInstanceId: pageInstanceId
        )
    }

    /// Opens the home mini app
    static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            LXLog.error("Home app details not available", category: "iOSLxApp")
            return
        }

        // Runtime is the source of truth for app session; local cache may still be empty
        // during early bootstrap.
        var sessionId: UInt64 = getLxAppSessionId(homeLxAppId)
        if sessionId == 0 {
            let current = getCurrentLxApp()
            let currentAppId = current.appid.toString()
            if currentAppId == homeLxAppId && current.session_id > 0 {
                sessionId = current.session_id
            } else {
                sessionId = LxAppCore.sessionId(for: homeLxAppId) ?? 0
            }
        }
        guard sessionId > 0 else {
            LXLog.error("Invalid home app session for \(homeLxAppId)", category: "iOSLxApp")
            return
        }
        LxAppCore.setSessionId(sessionId, for: homeLxAppId)
        openLxApp(appId: homeLxAppId, path: "", pageInstanceId: nil, sessionId: sessionId)
    }

    /// Closes a mini app with the specified appId
    @discardableResult
    static func closeLxApp(appId: String, sessionId: UInt64, notifyRuntime: Bool = true) -> Bool {
        os_log("Closing LxApp: %@", log: log, type: .info, appId)
        guard let manager = getInstanceUnsafe()?.lxAppManager else {
            LXLog.error("closeLxApp rejected: iOS host is not initialized", category: "iOSLxApp")
            return false
        }
        manager.closeLxApp(appId: appId, sessionId: sessionId, notifyRuntime: notifyRuntime)
        return true
    }

    /// Navigate to a page with specific animation type
    @discardableResult
    static func navigate(
        appId: String,
        path: String,
        pageInstanceId: String?,
        animationType: LxAppAnimation
    ) -> Bool {
        os_log("iOS navigate: %@ to %@ with type: %@", log: log, type: .info, appId, path, String(describing: animationType))
        return LxAppCore.executeNavigation(
            appId: appId,
            path: path,
            pageInstanceId: pageInstanceId,
            animationType: animationType
        )
    }

    /// The WebView of a page of the app: the instance named, or its current page.
    internal static func pageWebView(
        appId: String,
        sessionId: UInt64,
        pageInstanceId: String?
    ) -> WKWebView? {
        return WebViewManager.pageWebView(
            appId: appId,
            sessionId: sessionId,
            pageInstanceId: pageInstanceId
        )
    }

    private func openLxAppInManager(
        appId: String,
        path: String,
        pageInstanceId: String?,
        sessionId: UInt64
    ) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            LXLog.error("Failed to get window for presenting LxAppManager", category: "iOSLxApp")
            return
        }

        // Home cold start: keep the launch screen up until first render (or timeout).
        if appId == LxAppCore.getHomeLxAppId() {
            LingXiaSplashOverlay.attachIfNeeded(to: window)
        }

        // Use the provided path directly since we now have centralized state management
        let actualPath = path

        // Ensure LxAppManager exists
        if lxAppManager == nil {
            setupLxAppManager(window: window)
        }

        // Open LxApp in manager
        lxAppManager?.openLxApp(
            appId: appId,
            path: actualPath,
            pageInstanceId: pageInstanceId,
            sessionId: sessionId
        )
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
    static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
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
    internal static func openLxAppDirect(
        appId: String,
        path: String,
        pageInstanceId: String,
        sessionId: UInt64
    ) -> Bool {
        guard let instance = getInstanceUnsafe() else {
            LXLog.error("openLxApp rejected: iOS host is not initialized", category: "iOSLxApp")
            return false
        }

        // Ensure LxAppManager exists for iOS
        instance.setupLxAppManagerIfNeeded()

        // Open LxApp in manager
        guard let manager = instance.lxAppManager else {
            LXLog.error("openLxApp rejected: no active iOS app manager", category: "iOSLxApp")
            return false
        }
        manager.openLxApp(
            appId: appId,
            path: path,
            pageInstanceId: pageInstanceId,
            sessionId: sessionId
        )
        return true
    }

    /// Direct navigation implementation (called from LxAppCore)
    internal static func handleNavigationDirect(
        appId: String,
        path: String,
        pageInstanceId: String?,
        animationType: LxAppAnimation
    ) -> Bool {
        guard let manager = getInstanceUnsafe()?.lxAppManager else {
            LXLog.error("navigate rejected: iOS host is not initialized", category: "iOSLxApp")
            return false
        }

        // Platform-specific setup/switch WebView - this will handle all UI updates internally
        manager.handleNavigation(
            appId: appId,
            path: path,
            pageInstanceId: pageInstanceId,
            animationType: animationType
        )
        return true
    }

    private func setupLxAppManagerIfNeeded() {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first else {
            LXLog.error("Failed to get window for presenting LxAppManager", category: "iOSLxApp")
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
    @objc nonisolated public static func startPullDownRefresh(appid: RustStr, webtag: RustStr) -> Bool {
        let appidStr = appid.toString()
        let webtagStr = webtag.toString()
        
        DispatchQueue.main.async {
            // Access instance through a non-isolated path
            guard let instance = iOSLxApp.getInstanceUnsafe() else { return }
            guard let manager = instance.currentLxAppManager else { return }
            
            manager.startPullDownRefresh(webtag: webtagStr)
            
            os_log("startPullDownRefresh called for %@ %@", log: OSLog(subsystem: "LingXia", category: "PullToRefresh"), type: .info, appidStr, webtagStr)
        }
        return true
    }

    /// Stop pull-to-refresh animation
    @objc nonisolated public static func stopPullDownRefresh(appid: RustStr, webtag: RustStr) -> Bool {
        let appidStr = appid.toString()
        let webtagStr = webtag.toString()
        
        DispatchQueue.main.async {
            // Access instance through a non-isolated path
            guard let instance = iOSLxApp.getInstanceUnsafe() else { return }
            guard let manager = instance.currentLxAppManager else { return }
            
            manager.stopPullDownRefresh(webtag: webtagStr)
            
            os_log("stopPullDownRefresh called for %@ %@", log: OSLog(subsystem: "LingXia", category: "PullToRefresh"), type: .info, appidStr, webtagStr)
        }
        return true
    }
}

/// Background tasks held for Rust's background callbacks, by token. 0 is
/// "no task"; iOS ends a task itself at expiry if Rust has not.
@MainActor
enum HostBackgroundGrace {
    private static var tasks: [UInt64: UIBackgroundTaskIdentifier] = [:]
    private static var lastToken: UInt64 = 0

    static func begin() -> UInt64 {
        lastToken += 1
        let token = lastToken
        let task = UIApplication.shared.beginBackgroundTask(withName: "LingXia foreground callbacks") {
            // The expiration handler runs on the main thread.
            MainActor.assumeIsolated { end(token) }
        }
        guard task != .invalid else { return 0 }
        tasks[token] = task
        return token
    }

    /// Ends the task once, whether Rust or the expiration handler gets here first.
    static func end(_ token: UInt64) {
        guard let task = tasks.removeValue(forKey: token) else { return }
        UIApplication.shared.endBackgroundTask(task)
    }
}

#endif
