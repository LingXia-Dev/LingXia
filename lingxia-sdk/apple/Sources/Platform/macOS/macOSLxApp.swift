import SwiftUI
import os.log
import CLingXiaRustAPI

#if os(macOS)
import AppKit

/// Shared UI layout constants for macOS windows
struct LxAppWindowLayout {
    static let titleBarHeight: CGFloat = 32
}

/// macOS LxApp implementation
@MainActor
class macOSLxApp: ObservableObject {
    static let shared = macOSLxApp()
    private static var isInitialized = false
    private static let log = OSLog(subsystem: "LingXia", category: "macOSLxApp")

    nonisolated(unsafe) private static var lifecycleObservers: [NSObjectProtocol] = []
    nonisolated(unsafe) private static var hasResignedActive = false

    private init() {}

    internal static func activeShell() -> LxAppShell? {
        LxAppActiveHost.activeShell
    }

    static func openLxApp(appId: String, path: String, sessionId: UInt64) {
        os_log("macOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)
        _ = LxAppCore.executeOpenLxApp(appId: appId, path: path, sessionId: sessionId)
    }

    private static func openShellWindow() {
        if activeShell() == nil {
            let config = Lingxia.resolvedShellConfiguration(
                from: LxAppShellConfiguration(),
                capabilities: LxAppCapabilities(rawValue: LxAppCore.capabilities),
                homeAppId: LxAppCore.getHomeLxAppId()
            )
            let shell = LxAppShell(configuration: config)
            shell.show()
        } else {
            activeShell()?.window?.makeKeyAndOrderFront(nil)
        }
    }

    internal static func handleAppClosing(appId: String) {
        guard let sessionId = LxAppCore.sessionId(for: appId), sessionId > 0 else {
            LXLog.error("handleAppClosing missing session for \(appId)", category: "macOSLxApp")
            return
        }
        let accepted = onLxappClosed(appId, sessionId)
        guard accepted else {
            os_log("Ignoring stale close callback for %@ (session=%{public}llu)", log: log, type: .info, appId, sessionId)
            return
        }
        LxAppCore.removeSessionId(for: appId)

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

    static func navigate(appId: String, path: String, animationType: LxAppAnimation) {
        LxAppCore.executeNavigation(appId: appId, path: path, animationType: animationType)
    }

    internal static func openHomeLxApp() {
        guard let _ = LxAppCore.getHomeLxAppId() else { return }

        Task { @MainActor in
            openShellWindow()
        }
    }

    static func initialize() -> Bool {
        if isInitialized { return true }

        LxAppCore.initializeCore()
        isInitialized = LxAppCore.getHomeLxAppId() != nil

        if !isInitialized {
            LXLog.error("Failed to initialize LxApps - no home app ID", category: "macOSLxApp")
        } else {
            setupLifecycleObservers()
        }
        return isInitialized
    }

    private static func setupLifecycleObservers() {
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

    @MainActor
    private static func handleAppShow() {
        guard hasResignedActive else { return }
        hasResignedActive = false
        guard let currentAppId = LxAppCore.currentAppId else { return }
        os_log("App became active, notifying appId: %@", log: log, type: .info, currentAppId)
        lingxia.onAppShow(currentAppId)
    }

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
    internal static func openLxAppDirect(appId: String, path: String, sessionId: UInt64) {
        shared.handleOpenLxApp(appId: appId, path: path, sessionId: sessionId)
    }

    internal static func handleNavigationDirect(appId: String, path: String, animationType: LxAppAnimation) {
        shared.handleRegularNavigation(appId: appId, path: path, animationType: animationType)
        updateNavigationBarDirect(appId: appId, path: path)
        updateSidebarDirect(appId: appId, path: path)
    }

    private static func updateNavigationBarDirect(appId: String, path: String) {
        if let s = Self.activeShell(),
           let viewController = s.getViewController(for: appId) {
            viewController.updateNavigationBar(appId: appId, path: path)
        }
    }

    private static func updateSidebarDirect(appId: String, path: String) {
        NotificationCenter.default.post(name: .sidebarNeedsRefresh, object: appId)
    }

    private func handleOpenLxApp(appId: String, path: String, sessionId: UInt64) {
        if let s = Self.activeShell() {
            s.openLxApp(appId: appId, path: path, sessionId: sessionId)
            s.window?.makeKeyAndOrderFront(nil)
        } else {
            Self.openShellWindow()
            Self.activeShell()?.openLxApp(appId: appId, path: path, sessionId: sessionId)
        }
    }

    fileprivate func handleRegularNavigation(appId: String, path: String, animationType: LxAppAnimation) {
        if let s = Self.activeShell() {
           s.browserCoordinator.deactivate()
        }

        if let s = Self.activeShell(),
           let viewController = s.ensureViewController(for: appId, path: path) {
            viewController.navigate(appId: appId, to: path, with: animationType)
        }
    }

    @MainActor
    internal static func getViewController(for appId: String) -> macOSLxAppViewController? {
        activeShell()?.getViewController(for: appId)
    }

    @MainActor
    internal static func refreshNavigationBar(appId: String) {
        activeShell()?.refreshNavigationBar(for: appId)
    }

    @MainActor
    internal static var contentPanelView: NSView? {
        activeShell()?.contentPanelView
    }

    @MainActor
    internal static func presentInternalBrowserTab(tabId: String) -> Bool {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            LXLog.error("presentInternalBrowserTab invalid tab id: \(tabId)", category: "macOSLxApp")
            return false
        }

        if activeShell() == nil {
            openShellWindow()
        }

        guard let s = activeShell() else { return false }
        s.presentInternalBrowserTab(id: normalized)
        s.window?.makeKeyAndOrderFront(nil)
        return true
    }

    @MainActor
    internal static func prepareInternalBrowserTabForInput(tabId: String) -> Bool {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        if activeShell() == nil {
            openShellWindow()
        }
        guard let s = activeShell() else { return false }
        return s.prepareInternalBrowserTabForInput(id: normalized)
    }

    @MainActor
    internal static func consumeSelfTargetNavigationInActiveBrowserTab(urlString: String) -> Bool {
        guard let s = activeShell() else { return false }
        return s.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString)
    }
}

// MARK: - Pull-to-Refresh Bridge Functions
extension LxApp {
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
