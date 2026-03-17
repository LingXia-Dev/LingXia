import SwiftUI
import os.log
import CLingXiaRustAPI

#if os(macOS)
import AppKit

/// Shared UI layout constants for macOS windows
public struct LxAppWindowLayout {
    public static let titleBarHeight: CGFloat = 32        // SwiftUI custom title bar height
}

/// macOS LxApp implementation
@MainActor
public class macOSLxApp: ObservableObject {
    public static let shared = macOSLxApp()
    private static var isInitialized = false
    private static let log = OSLog(subsystem: "LingXia", category: "macOSLxApp")

    internal static var tabWindowController: LxAppWindowController?

    /// Lifecycle event observers
    nonisolated(unsafe) private static var lifecycleObservers: [NSObjectProtocol] = []
    nonisolated(unsafe) private static var hasResignedActive = false

    private init() {}

    /// Open specific LxApp
    public static func openLxApp(appId: String, path: String, sessionId: UInt64) {
        os_log("macOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)
        LxAppCore.executeOpenLxApp(appId: appId, path: path, sessionId: sessionId)
    }

    private static func openTabStyleWindow() {
        if tabWindowController == nil {
            tabWindowController = LxAppWindowController()
            tabWindowController?.showWindow(nil)
            NSApp.activate(ignoringOtherApps: true)
        } else {
            tabWindowController?.window?.makeKeyAndOrderFront(nil)
        }
    }

    internal static func handleAppClosing(appId: String) {
        guard let sessionId = LxAppCore.sessionId(for: appId), sessionId > 0 else {
            os_log("handleAppClosing missing session for %@", log: log, type: .error, appId)
            return
        }
        // Call FFI close handler first and ignore stale callbacks.
        let accepted = onLxappClosed(appId, sessionId)
        guard accepted else {
            os_log("Ignoring stale close callback for %@ (session=%{public}llu)", log: log, type: .info, appId, sessionId)
            return
        }
        LxAppCore.removeSessionId(for: appId)

        // Get next LxApp from Rust stack and open it
        let currentLxApp = getCurrentLxApp()
        let appidStr = currentLxApp.appid.toString()
        let pathStr = currentLxApp.path.toString()
        let nextSession = currentLxApp.session_id
        if !appidStr.isEmpty && nextSession > 0 {
            os_log("Opening next LxApp from stack: %@:%@", log: log, type: .info, appidStr, pathStr)
            openLxApp(appId: appidStr, path: pathStr, sessionId: nextSession)
        } else {
            os_log("No more LxApps in stack", log: log, type: .info)
        }
    }

    /// Navigate to page with specific animation type
    public static func navigate(appId: String, path: String, animationType: AnimationType) {
        LxAppCore.executeNavigation(appId: appId, path: path, animationType: animationType)
    }

    /// Remove tab window controller
    public static func removeTabWindowController(_ controller: LxAppWindowController) {
        if tabWindowController === controller {
            tabWindowController = nil
        }
    }

    /// Open home LxApp
    internal static func openHomeLxApp() {
        guard let _ = LxAppCore.getHomeLxAppId() else { return }

        Task { @MainActor in
            openTabStyleWindow()
        }
    }

    /// Initialize LxApps system
    public static func initialize() -> Bool {
        if isInitialized { return true }

        LxAppCore.initializeCore()
        isInitialized = LxAppCore.getHomeLxAppId() != nil

        if !isInitialized {
            os_log("Failed to initialize LxApps - no home app ID", log: log, type: .error)
        } else {
            loadPanelConfig()
            setupLifecycleObservers()
        }
        return isInitialized
    }

    /// Setup observers for app lifecycle events
    private static func setupLifecycleObservers() {
        // App became active (foreground)
        let activeObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { _ in
            Task { @MainActor in
                handleAppShow()
            }
        }
        lifecycleObservers.append(activeObserver)

        // App resigned active (background)
        let resignObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didResignActiveNotification,
            object: nil,
            queue: .main
        ) { _ in
            Task { @MainActor in
                handleAppHide()
            }
        }
        lifecycleObservers.append(resignObserver)
    }

    /// Handle app becoming active
    @MainActor
    private static func handleAppShow() {
        guard hasResignedActive else { return }
        hasResignedActive = false
        guard let currentAppId = LxAppCore.currentAppId else { return }
        os_log("App became active, notifying appId: %@", log: log, type: .info, currentAppId)
        lingxia.onAppShow(currentAppId)
    }

    /// Handle app resigning active
    @MainActor
    private static func handleAppHide() {
        hasResignedActive = true
        guard let currentAppId = LxAppCore.currentAppId else { return }
        os_log("App resigned active, notifying appId: %@", log: log, type: .info, currentAppId)
        lingxia.onAppHide(currentAppId)
    }
}

// MARK: - Direct platform implementation
extension macOSLxApp {
    /// Direct openLxApp implementation (called from LxAppCore)
    internal static func openLxAppDirect(appId: String, path: String, sessionId: UInt64) {
        shared.handleTabStyleOpenLxApp(appId: appId, path: path, sessionId: sessionId)
    }

    /// Direct navigation implementation (called from LxAppCore)
    internal static func handleNavigationDirect(appId: String, path: String, animationType: AnimationType) {
        shared.handleRegularNavigation(appId: appId, path: path, animationType: animationType)

        // Update UI components based on Rust state
        updateNavigationBarDirect(appId: appId, path: path)
        updateSidebarDirect(appId: appId, path: path)
    }

    /// Update NavigationBar based on Rust state
    private static func updateNavigationBarDirect(appId: String, path: String) {
        if let tabController = Self.tabWindowController,
           let viewController = tabController.getViewController(for: appId) {
            viewController.updateNavigationBar(appId: appId, path: path)
        }
    }

    /// Notify sidebar to refresh for a specific app
    private static func updateSidebarDirect(appId: String, path: String) {
        NotificationCenter.default.post(name: .sidebarNeedsRefresh, object: appId)
    }

    private func handleTabStyleOpenLxApp(appId: String, path: String, sessionId: UInt64) {
        if let tabController = Self.tabWindowController {
            tabController.openLxApp(appId: appId, path: path, sessionId: sessionId)
            tabController.window?.makeKeyAndOrderFront(nil)
        } else {
            Self.openTabStyleWindow()
            Self.tabWindowController?.openLxApp(appId: appId, path: path, sessionId: sessionId)
        }
    }

    fileprivate func handleRegularNavigation(appId: String, path: String, animationType: AnimationType) {
        if let tabController = Self.tabWindowController,
           let viewController = tabController.getViewController(for: appId) {
            viewController.navigate(appId: appId, to: path, with: animationType)
        }
    }

    @MainActor
    internal static func getViewController(for appId: String) -> macOSLxAppViewController? {
        return tabWindowController?.getViewController(for: appId)
    }

    /// The content panel view that excludes the sidebar. Use as root container for popups.
    @MainActor
    internal static var contentPanelView: NSView? {
        tabWindowController?.contentPanelView
    }

    @MainActor
    internal static func presentInternalBrowserTab(tabId: String) -> Bool {
        let normalized = tabId.lowercased()
        guard let id = UUID(uuidString: normalized) else {
            os_log("presentInternalBrowserTab invalid tab id: %{public}@", log: log, type: .error, tabId)
            return false
        }

        if tabWindowController == nil {
            openTabStyleWindow()
        }

        guard let controller = tabWindowController else { return false }
        controller.presentInternalBrowserTab(id: id)
        controller.window?.makeKeyAndOrderFront(nil)
        return true
    }

    @MainActor
    internal static func consumeSelfTargetNavigationInActiveBrowserTab(urlString: String) -> Bool {
        guard let controller = tabWindowController else { return false }
        return controller.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString)
    }
}

// MARK: - Pull-to-Refresh Bridge Functions
extension LxApp {
    /// Start pull-to-refresh animation programmatically
    @objc nonisolated public static func startPullDownRefresh(appid: RustStr, path: RustStr) -> Bool {
        let appIdStr = appid.toString()
        let pathStr = path.toString()

        Task { @MainActor in
            guard let manager = macOSLxApp.getViewController(for: appIdStr) else { return }
            manager.startPullDownRefreshProgrammatically()
            os_log("startPullDownRefresh called for %@:%@", log: OSLog(subsystem: "LingXia", category: "PullToRefresh"), type: .info, appIdStr, pathStr)
        }
        return true
    }

    /// Stop pull-to-refresh animation
    @objc nonisolated public static func stopPullDownRefresh(appid: RustStr, path: RustStr) -> Bool {
        let appIdStr = appid.toString()
        let pathStr = path.toString()

        Task { @MainActor in
            guard let manager = macOSLxApp.getViewController(for: appIdStr) else { return }
            manager.stopPullDownRefreshProgrammatically()
            os_log("stopPullDownRefresh called for %@:%@", log: OSLog(subsystem: "LingXia", category: "PullToRefresh"), type: .info, appIdStr, pathStr)
        }
        return true
    }
}

#endif
