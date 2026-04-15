import Foundation
import OSLog
import WebKit
import CLingXiaRustAPI
import CLingXiaSwiftAPI
import Darwin

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// FFI event constants for `onLxappEvent()` calls.
struct LxAppEvent {
    static var tabBarClick: LxAppUiEventType { LxAppUiEventType.TabBarClick }
    static var capsuleClick: LxAppUiEventType { LxAppUiEventType.CapsuleClick }
    static var navigationClick: LxAppUiEventType { LxAppUiEventType.NavigationClick }
    static var backPress: LxAppUiEventType { LxAppUiEventType.BackPress }
    static var pullDownRefresh: LxAppUiEventType { LxAppUiEventType.PullDownRefresh }

    static let capsuleActionClose = "close"
    static let navigationActionBack = "back"
    static let navigationActionHome = "home"
}

struct AppEvent {
    static var panelIconClick: AppUiEventType { AppUiEventType.PanelIconClick }
}

let ACTION_CLOSE_LXAPP = "com.lingxia.CLOSE_LXAPP_ACTION"

/// Core LxApp management logic shared between platforms
@MainActor
final class LxAppCore {
    private typealias NativeHostAddonInstaller = @convention(c) () -> Void
    private static let log = OSLog(subsystem: "LingXia", category: "LxAppCore")
    static var resourceBundle: Bundle {
#if SWIFT_PACKAGE
        return Bundle.module
#else
        return Bundle(for: LxAppCore.self)
#endif
    }

    /// Singleton instance
    private static var instance: LxAppCore?

    /// Home LxApp configuration
    internal static var homeLxAppId: String?

    /// Bitmask of host app capabilities, queried from Rust after init.
    static var capabilities: UInt32 = 0
    static let capShell: UInt32 = 0x1

    /// Global current app state - shared across iOS and macOS
    internal private(set) static var currentAppId: String?
    private static var currentPath: String = ""
    private static var appSessions: [String: UInt64] = [:]

    /// Current WebView - cached to avoid frequent findWebView calls
    private static var currentWebView: WKWebView?

    private init() {}

    /// Shared openLxApp logic - used by both iOS and macOS platforms
    internal static func executeOpenLxApp(
        appId: String,
        path: String,
        sessionId: UInt64,
        presentation: Int32 = 0,
        panelId: String = ""
    ) -> Bool {
        guard sessionId > 0 else {
            os_log("executeOpenLxApp rejected invalid session for %@", log: log, type: .error, appId)
            return false
        }

        // Call onLxappOpened to get the resolved path
        let resolvedPath = onLxappOpened(appId, path, sessionId)
        let finalPath = resolvedPath.toString()
        guard !finalPath.isEmpty else {
            os_log("executeOpenLxApp rejected by Rust (stale session?) for %@ session=%{public}llu", log: log, type: .info, appId, sessionId)
            return false
        }
        appSessions[appId] = sessionId
        let isPanel = (presentation == 1)

        // Check for custom handler first (e.g., Runner's Capsule mode)
        if let handler = openLxAppHandler, handler(appId, finalPath) {
            return true
        }

        // Panel presentation bypasses normal tab routing on macOS.
        #if os(macOS)
        if isPanel,
           macOSLxApp.handlePanelLxAppOpened(
            appId: appId,
            path: finalPath,
            sessionId: sessionId,
            panelId: panelId
           ) {
            return true
        }
        #endif

        // Direct platform calls instead of using renderer protocol
        #if os(iOS)
        iOSLxApp.openLxAppDirect(appId: appId, path: finalPath, sessionId: sessionId)
        #elseif os(macOS)
        macOSLxApp.openLxAppDirect(appId: appId, path: finalPath, sessionId: sessionId)
        #endif
        return true
    }

    /// Shared navigate logic - used by both iOS and macOS platforms
    internal static func executeNavigation(appId: String, path: String, animationType: LxAppAnimation) {
        os_log("Core executeNavigation: %@ to %@ with type: %@", log: log, type: .info, appId, path, String(describing: animationType))

        guard !appId.isEmpty else {
            os_log("Empty appId provided for navigation", log: log, type: .error)
            return
        }

        // Check for custom handler first (e.g., Runner's Capsule mode)
        if let handler = navigationHandler, handler(appId, path, animationType) {
            return
        }

        // Direct platform calls - no need for complex preparation logic
        #if os(iOS)
        iOSLxApp.handleNavigationDirect(appId: appId, path: path, animationType: animationType)
        #elseif os(macOS)
        macOSLxApp.handleNavigationDirect(appId: appId, path: path, animationType: animationType)
        #endif
    }

    nonisolated(unsafe) private static var nativeRegistrationPerformed = false

    /// Skip auto-opening window after initialization (for tools like Runner that manage their own windows)
    nonisolated(unsafe) internal static var skipAutoOpenWindow = false

    /// Custom handler for openLxApp - for tools like Runner that manage their own windows
    /// Return true to indicate the call was handled, false to use default behavior
    nonisolated(unsafe) internal static var openLxAppHandler: ((String, String) -> Bool)?

    /// Custom handler for navigation - for tools like Runner that manage their own windows
    /// Return true to indicate the call was handled, false to use default behavior
    nonisolated(unsafe) internal static var navigationHandler: ((String, String, LxAppAnimation) -> Bool)?

    /// Initialize the LxApp system (internal core initialization)
    internal static func initializeCore(autoOpenHome: Bool = true) {
        if instance != nil {
            return
        }

        WebViewManager.registerRuntimeClasses()

        // Discover and invoke native host registration once before initialization.
        if !nativeRegistrationPerformed {
            installNativeHostAddon()
            nativeRegistrationPerformed = true
        }

        if let info = LxAppRuntime.shared.info {
            bootstrapFromRuntimeInfo(info, autoOpenHome: autoOpenHome)
        } else {
            performInitialization(autoOpenHome: autoOpenHome)
        }
    }

    /// Check if LxApp system is initialized and ready for use
    internal static func isInitialized() -> Bool {
        return instance != nil && homeLxAppId != nil
    }

    private static func bootstrapFromRuntimeInfo(
        _ info: LxAppRuntimeInfo,
        autoOpenHome: Bool
    ) {
        instance = LxAppCore()
        homeLxAppId = info.homeAppId
        capabilities = info.capabilities.rawValue

        if autoOpenHome && !skipAutoOpenWindow {
            DispatchQueue.main.async {
                LxAppPlatform.openHomeLxApp()
            }
        }
    }

    private static func performInitialization(autoOpenHome: Bool) {
        instance = LxAppCore()

        // Get platform-specific directory configuration
        let directoryConfig = LxAppDirectoryFactory.createDirectoryConfig()

        // Get system locale
        let locale = Locale.current.identifier

        let initResult = lingxiaInit(directoryConfig.dataPath, directoryConfig.cachesPath, locale)
        let initResultString = initResult?.toString()

        if let homeAppId = initResultString {
            homeLxAppId = homeAppId
            capabilities = getAppCapabilities()
            os_log("LxApp initialized successfully with home app: %{public}@", log: log, type: .info, homeAppId)

            // Auto-open home lxapp after initialization (unless skipped by external tools)
            if autoOpenHome && !skipAutoOpenWindow {
                DispatchQueue.main.async {
                    LxAppPlatform.openHomeLxApp()
                }
            }
        } else {
            os_log("Failed to get home LxApp ID from native init", log: log, type: .error)
        }
    }

    /// Enable WebView debugging
    internal static func enableWebViewDebugging() {
        WebViewManager.enableDebugging()
    }

    /// Called by `LxAppRuntime.initialize()` to ensure the native host addon
    /// is installed exactly once. Safe to call multiple times.
    internal static func installNativeHostAddonOnce() {
        guard !nativeRegistrationPerformed else { return }
        installNativeHostAddon()
        nativeRegistrationPerformed = true
    }

    private static func installNativeHostAddon() {
        guard let installer = resolveNativeHostAddonInstaller() else {
            os_log("Native host addon installer not found", log: log, type: .info)
            return
        }
        os_log("Installing native host addon", log: log, type: .info)
        installer()
    }

    /// Discover the native host addon installer symbol via `dlsym(RTLD_DEFAULT, ...)`.
    ///
    /// The host app (or its static Rust library) is expected to define:
    /// ```c
    /// void lingxia_install_host_addon(void);
    /// ```
    /// The corresponding `-u` linker flag in Package.swift ensures the symbol is
    /// not stripped even when the only reference is this runtime lookup.
    private static func resolveNativeHostAddonInstaller() -> NativeHostAddonInstaller? {
        // RTLD_DEFAULT (-2): search all loaded images in default order.
        let rtldDefault = UnsafeMutableRawPointer(bitPattern: -2)
        guard let raw = dlsym(rtldDefault, "lingxia_install_host_addon") else {
            return nil
        }
        return unsafeBitCast(raw, to: NativeHostAddonInstaller.self)
    }

    /// Called by `LxAppController.close()` to close an LxApp from the Swift side.
    internal static func executeCloseLxApp(appId: String, sessionId: UInt64) {
        #if os(iOS)
        if iOSLxApp.getInstanceUnsafe() != nil {
            iOSLxApp.closeLxApp(appId: appId, sessionId: sessionId, notifyRuntime: true)
            return
        }
        #endif

        let accepted = onLxappClosed(appId, sessionId)
        if !accepted {
            os_log(
                "executeCloseLxApp ignored stale session for %{public}@ session=%{public}llu",
                log: log,
                type: .info,
                appId,
                sessionId
            )
        }
    }

    /// Check if app is home LxApp
    static func isHomeLxApp(_ appId: String) -> Bool {
        return appId == homeLxAppId
    }

    /// Set current app state - shared across platforms
    static func setCurrentApp(appId: String, path: String) {
        currentAppId = appId
        currentPath = path

        // Update WebView cache when app/path changes
        if let sessionId = appSessions[appId], sessionId > 0 {
            currentWebView = WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId)
        } else {
            currentWebView = nil
        }
    }

    /// Get current path for active app - always returns definitive value, never nil
    static func getCurrentPath() -> String {
        guard currentAppId != nil else { return "/" }
        return currentPath.isEmpty ? "/" : currentPath
    }

    /// Update current path
    static func setCurrentPath(_ path: String) {
        guard let appId = currentAppId else { return }
        currentPath = path

        // Update WebView cache when path changes
        if let sessionId = appSessions[appId], sessionId > 0 {
            currentWebView = WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId)
        } else {
            currentWebView = nil
        }
    }

    /// Get current WebView - cached for efficiency
    static func getCurrentWebView() -> WKWebView? {
        return currentWebView
    }

    /// Get home LxApp ID
    static func getHomeLxAppId() -> String? {
        return homeLxAppId
    }

    static func sessionId(for appId: String) -> UInt64? {
        return appSessions[appId]
    }

    static func setSessionId(_ sessionId: UInt64, for appId: String) {
        if sessionId > 0 {
            appSessions[appId] = sessionId
        }
    }

    static func removeSessionId(for appId: String) {
        appSessions.removeValue(forKey: appId)
    }
}

/// Main LxApp interface - unified API for both iOS and macOS
/// This class provides a clean, consistent API that delegates to platform-specific implementations
@MainActor
final class LxApp {

    static let log = OSLog(subsystem: "LingXia", category: "LxApp")

    nonisolated static func executeOnMain<T: Sendable>(_ operation: @MainActor @Sendable () throws -> T) rethrows -> T {
        if Thread.isMainThread {
            return try MainActor.assumeIsolated {
                try operation()
            }
        } else {
            return try DispatchQueue.main.sync {
                try MainActor.assumeIsolated {
                    try operation()
                }
            }
        }
    }

    nonisolated(unsafe) internal static var skipAutoOpenWindow: Bool {
        get { LxAppCore.skipAutoOpenWindow }
        set { LxAppCore.skipAutoOpenWindow = newValue }
    }

    nonisolated(unsafe) internal static var openLxAppHandler: ((String, String) -> Bool)? {
        get { LxAppCore.openLxAppHandler }
        set { LxAppCore.openLxAppHandler = newValue }
    }

    nonisolated(unsafe) internal static var navigationHandler: ((String, String, LxAppAnimation) -> Bool)? {
        get { LxAppCore.navigationHandler }
        set { LxAppCore.navigationHandler = newValue }
    }

    public static func enableWebViewDebugging() {
        LxAppCore.enableWebViewDebugging()
    }

    #if os(iOS)
    /// Configure transparent system bars (iOS only)
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        LxAppPlatform.configureTransparentSystemBars(viewController: viewController, lightStatusBarIcons: lightStatusBarIcons)
    }
    #endif

    #if os(iOS)
    /// Get the topmost view controller (iOS only)
    public static func topViewController() -> UIViewController? {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first,
              let rootViewController = window.rootViewController else {
            return nil
        }
        return LxAppViewHierarchyHelper.findTopmostViewController(from: rootViewController)
    }
    #endif

    /// Open home LxApp (internal use)
    internal static func openHomeLxApp() {
        LxAppPlatform.openHomeLxApp()
    }
}
