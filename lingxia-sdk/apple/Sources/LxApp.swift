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

public struct LxAppEvent {
    // lxapp-scoped event type constants.
    public static var tabBarClick: LxAppUiEventType { LxAppUiEventType.TabBarClick }
    public static var capsuleClick: LxAppUiEventType { LxAppUiEventType.CapsuleClick }
    public static var navigationClick: LxAppUiEventType { LxAppUiEventType.NavigationClick }
    public static var backPress: LxAppUiEventType { LxAppUiEventType.BackPress }
    public static var pullDownRefresh: LxAppUiEventType { LxAppUiEventType.PullDownRefresh }

    // UI Event Data Constants
    public static let capsuleActionClose = "close"
    public static let navigationActionBack = "back"
    public static let navigationActionHome = "home"
}

public struct AppEvent {
    // host-app scoped events.
    public static var panelIconClick: AppUiEventType { AppUiEventType.PanelIconClick }
}

// Legacy compatibility alias used by tools/lingxia-runner.
public typealias LxAppUIEvent = LxAppEvent

/// Animation type enum for page transitions
public enum AnimationType: Sendable {
    case none      // No animation
    case forward   // Forward animation (push-style)
    case backward  // Backward animation (pop-style)
}

public enum LxAppOpenPresentation: Int32, Sendable {
    case normal = 0
    case panel = 1
}

// Sendable Conformance for FFI Types
extension ToastIcon: @unchecked Sendable {}
extension ToastPosition: @unchecked Sendable {}
extension GroupAlignment: @unchecked Sendable {}

/// Platform-specific directory configuration
public struct LxAppDirectoryConfig {
    public let dataPath: String
    public let cachesPath: String

    public init(dataPath: String, cachesPath: String) {
        self.dataPath = dataPath
        self.cachesPath = cachesPath
    }
}

/// Directory provider errors
public enum LxAppDirectoryError: Error {
    case bundleIdentifierNotFound
    case systemDirectoryNotFound(FileManager.SearchPathDirectory)
}

/// Simplified directory provider for all platforms
public struct LxAppDirectoryFactory {

    private static func resolveBundleIdentifier() -> String {
        if let bundleId = Bundle.main.bundleIdentifier, !bundleId.isEmpty {
            return bundleId
        }

        if let infoBundleId = Bundle.main.object(forInfoDictionaryKey: "CFBundleIdentifier") as? String,
           !infoBundleId.isEmpty
        {
            return infoBundleId
        }

        let processName = ProcessInfo.processInfo.processName
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if !processName.isEmpty {
            return "com.lingxia.\(processName.lowercased())"
        }

        return "com.lingxia.app"
    }

    /// Create platform-specific directory configuration
    public static func createDirectoryConfig() -> LxAppDirectoryConfig {
        do {
            let bundleId = resolveBundleIdentifier()

            #if os(iOS)
            let dataDirectory: FileManager.SearchPathDirectory = .documentDirectory
            #elseif os(macOS)
            let dataDirectory: FileManager.SearchPathDirectory = .applicationSupportDirectory
            #endif

            guard let dataURL = FileManager.default.urls(for: dataDirectory, in: .userDomainMask).first,
                  let cacheURL = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else {
                throw LxAppDirectoryError.systemDirectoryNotFound(dataDirectory)
            }

            let dataPath = dataURL.appendingPathComponent(bundleId).path
            let cachePath = cacheURL.appendingPathComponent(bundleId).path

            // Create directories if they don't exist
            try FileManager.default.createDirectory(atPath: dataPath, withIntermediateDirectories: true, attributes: nil)
            try FileManager.default.createDirectory(atPath: cachePath, withIntermediateDirectories: true, attributes: nil)

            return LxAppDirectoryConfig(dataPath: dataPath, cachesPath: cachePath)
        } catch {
            fatalError("Failed to create directory config: \(error)")
        }
    }
}

/// Notification action identifiers
public let ACTION_CLOSE_LXAPP = "com.lingxia.CLOSE_LXAPP_ACTION"



#if os(iOS)
/// iOS-specific view hierarchy helper
@MainActor
class LxAppViewHierarchyHelper {
    /// Finds the topmost view controller in the hierarchy
    static func findTopmostViewController(from viewController: UIViewController) -> UIViewController {
        if let presentedVC = viewController.presentedViewController {
            return findTopmostViewController(from: presentedVC)
        }

        if let navController = viewController as? UINavigationController,
           let topVC = navController.topViewController {
            return findTopmostViewController(from: topVC)
        }

        if let tabController = viewController as? UITabBarController,
           let selectedVC = tabController.selectedViewController {
            return findTopmostViewController(from: selectedVC)
        }

        return viewController
    }

    /// Find specific view controller type in hierarchy
    static func findSpecificViewController<T>(in viewController: UIViewController?) -> T? {
        guard let viewController = viewController else { return nil }

        if let targetVC = viewController as? T {
            return targetVC
        }

        if let navController = viewController as? UINavigationController {
            return findSpecificViewController(in: navController.topViewController)
        }

        if let presentedVC = viewController.presentedViewController {
            return findSpecificViewController(in: presentedVC)
        }

        return nil
    }
}
#endif

/// Core LxApp management logic shared between platforms
@MainActor
public class LxAppCore {
    private typealias NativeHostAddonInstaller = @convention(c) () -> Void
    private static let log = OSLog(subsystem: "LingXia", category: "LxAppCore")
    public static var resourceBundle: Bundle {
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

    /// Panels configuration JSON (populated after lingxiaInit)
    internal static var panelsConfigJson: String?

    /// Bitmask of host app capabilities, queried from Rust after init.
    public static var capabilities: UInt32 = 0
    public static let capShell: UInt32 = 0x1

    /// Global current app state - shared across iOS and macOS
    public private(set) static var currentAppId: String?
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
        presentation: Int32 = LxAppOpenPresentation.normal.rawValue,
        panelId: String = ""
    ) {
        guard sessionId > 0 else {
            os_log("executeOpenLxApp rejected invalid session for %@", log: log, type: .error, appId)
            return
        }

        // Call onLxappOpened to get the resolved path
        let resolvedPath = onLxappOpened(appId, path, sessionId)
        let finalPath = resolvedPath.toString()
        guard !finalPath.isEmpty else {
            os_log("executeOpenLxApp rejected by Rust (stale session?) for %@ session=%{public}llu", log: log, type: .info, appId, sessionId)
            return
        }
        appSessions[appId] = sessionId
        let openPresentation = LxAppOpenPresentation(rawValue: presentation) ?? .normal

        // Check for custom handler first (e.g., Runner's Capsule mode)
        if let handler = openLxAppHandler, handler(appId, finalPath) {
            return
        }

        // Panel presentation bypasses normal tab routing on macOS.
        #if os(macOS)
        if openPresentation == .panel,
           macOSLxApp.handlePanelLxAppOpened(
            appId: appId,
            path: finalPath,
            sessionId: sessionId,
            panelId: panelId
           ) {
            return
        }
        #endif

        // Direct platform calls instead of using renderer protocol
        #if os(iOS)
        iOSLxApp.openLxAppDirect(appId: appId, path: finalPath, sessionId: sessionId)
        #elseif os(macOS)
        macOSLxApp.openLxAppDirect(appId: appId, path: finalPath, sessionId: sessionId)
        #endif
    }

    /// Shared navigate logic - used by both iOS and macOS platforms
    internal static func executeNavigation(appId: String, path: String, animationType: AnimationType) {
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
    nonisolated(unsafe) internal static var navigationHandler: ((String, String, AnimationType) -> Bool)?

    /// Initialize the LxApp system (internal core initialization)
    internal static func initializeCore() {
        if instance != nil {
            return
        }

        WebViewManager.registerRuntimeClasses()

        // Discover and invoke native host registration once before initialization.
        if !nativeRegistrationPerformed {
            installNativeHostAddon()
            nativeRegistrationPerformed = true
        }

        performInitialization()
    }

    /// Check if LxApp system is initialized and ready for use
    internal static func isInitialized() -> Bool {
        return instance != nil && homeLxAppId != nil
    }

    private static func performInitialization() {
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
            panelsConfigJson = getPanelsConfigJson()?.toString()
            os_log("LxApp initialized successfully with home app: %{public}@", log: log, type: .info, homeAppId)

            // Auto-open home lxapp after initialization (unless skipped by external tools)
            if !skipAutoOpenWindow {
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

    /// Check if app is home LxApp
    public static func isHomeLxApp(_ appId: String) -> Bool {
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
    public static func getCurrentPath() -> String {
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
public class LxApp {

    nonisolated(unsafe) fileprivate static let log = OSLog(subsystem: "LingXia", category: "LxApp")

    /// Execute a closure on the main thread, ensuring MainActor isolation
    nonisolated private static func executeOnMain<T: Sendable>(_ operation: @MainActor @Sendable () throws -> T) rethrows -> T {
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

    /// Skip auto-opening window after initialization (for tools like Runner that manage their own windows).
    /// Set this to true before calling `Lingxia.initialize()` if you want to control window creation yourself.
    nonisolated(unsafe) public static var skipAutoOpenWindow: Bool {
        get { LxAppCore.skipAutoOpenWindow }
        set { LxAppCore.skipAutoOpenWindow = newValue }
    }

    /// Custom handler for openLxApp - for tools like Runner that manage their own windows.
    /// Set this before calling `Lingxia.initialize()`. Return true to indicate the call was handled.
    nonisolated(unsafe) public static var openLxAppHandler: ((String, String) -> Bool)? {
        get { LxAppCore.openLxAppHandler }
        set { LxAppCore.openLxAppHandler = newValue }
    }

    /// Custom handler for navigation - for tools like Runner that manage their own windows.
    /// Set this before calling `Lingxia.initialize()`. Return true to indicate the call was handled.
    nonisolated(unsafe) public static var navigationHandler: ((String, String, AnimationType) -> Bool)? {
        get { LxAppCore.navigationHandler }
        set { LxAppCore.navigationHandler = newValue }
    }

    /// Enable WebView debugging
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

/// FFI interface for LxApp
extension LxApp {
    /// Open specific LxApp
    nonisolated public static func openLxApp(
        appid: RustStr,
        path: RustStr,
        session_id: UInt64,
        presentation: Int32,
        panel_id: RustStr
    ) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()
        let panelIdString = panel_id.toString()
        guard session_id > 0 else {
            return false
        }

        return executeOnMain {
            LxAppCore.executeOpenLxApp(
                appId: appIdString,
                path: pathString,
                sessionId: session_id,
                presentation: presentation,
                panelId: panelIdString
            )
            return true
        }
    }

    /// Close LxApp
    nonisolated public static func closeLxApp(appid: RustStr, session_id: UInt64) -> Bool {
        let appIdString = appid.toString()
        guard session_id > 0 else {
            return false
        }

        return executeOnMain {
            #if os(iOS)
            iOSLxApp.closeLxApp(appId: appIdString, sessionId: session_id)
            #endif
            // macOS: Tab mode handles closing via tab manager
            return true
        }
    }

    /// Navigate to page with specific animation type
    nonisolated public static func navigate(appid: RustStr, path: RustStr, animation_type: Int32) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()

        // Convert Int32 to AnimationType enum
        let animationType: AnimationType
        switch animation_type {
        case 1: animationType = .forward
        case 2: animationType = .backward
        default: animationType = .none // 0 or any other value
        }

        return executeOnMain {
            LxAppPlatform.navigate(appId: appIdString, path: pathString, animationType: animationType)
            return true
        }
    }

    /// Update TabBar UI to refresh badge and red dot data etc
    nonisolated public static func updateTabBarUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()

        return executeOnMain {
            os_log("LxApp.updateTabBarUI called for appId: %@", log: log, type: .info, appIdString)

            // Notify all TabBar instances to refresh their data from Rust
            NotificationCenter.default.post(
                name: .tabBarStateChanged,
                object: appIdString
            )

            return true
        }
    }

    nonisolated public static func updateNavBarUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain {
            NavigationBarStateManager.shared.refreshState(for: appIdString)
            return true
        }
    }

    nonisolated public static func updateOrientationUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain {
            #if os(iOS)
            guard let instance = iOSLxApp.getInstanceUnsafe(),
                  let manager = instance.currentLxAppManager else {
                return false
            }
            return manager.applyOrientationFromRuntime(for: appIdString)
            #else
            return true
            #endif
        }
    }

    nonisolated private static func openExternalUrlString(_ urlString: String) -> Bool {
        guard let url = URL(string: urlString) else {
            os_log(.error, log: Self.log, "Invalid URL: %{public}@", urlString)
            return false
        }
        #if os(iOS)
        DispatchQueue.main.async {
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }
        return true
        #elseif os(macOS)
        return NSWorkspace.shared.open(url)
        #else
        return false
        #endif
    }

    nonisolated public static func openUrl(
        owner_appid: RustStr,
        owner_session_id: UInt64,
        url: RustStr,
        target: Int32
    ) -> Bool {
        let ownerAppId = owner_appid.toString()
        let urlString = url.toString()
        let selfTarget: Int32 = 1
        let newBrowserTab: Int32 = 2

        guard target == selfTarget || target == newBrowserTab else {
            return openExternalUrlString(urlString)
        }

        guard !ownerAppId.isEmpty, owner_session_id > 0 else {
            return false
        }
        #if os(macOS)
        // SelfTarget tries to navigate the current active tab first (in-place navigation).
        // NewBrowserTab always opens a new tab — skip the "navigate current" heuristic.
        if target == selfTarget && ownerAppId == getBuiltinBrowserAppId().toString() {
            let scheme = URL(string: urlString)?.scheme?.lowercased()
            if let scheme, scheme != "http", scheme != "https" {
                return openExternalUrlString(urlString)
            }
            if executeOnMain({ macOSLxApp.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString) }) {
                return true
            }
        }
        #endif
        guard let openedTab = openBrowserTab(ownerAppId, owner_session_id, urlString) else {
            os_log(.error, log: Self.log, "openBrowserTab failed for %{public}@/%{public}llu url=%{public}@",
                   ownerAppId, owner_session_id, urlString)
            return false
        }
        let tabId = openedTab.toString()
        guard !tabId.isEmpty else {
            return false
        }

        #if os(macOS)
        return executeOnMain {
            return macOSLxApp.presentInternalBrowserTab(tabId: tabId)
        }
        #elseif os(iOS)
        return executeOnMain {
            return LxAppBrowserOverlay.show(tabId: tabId)
        }
        #else
        return false
        #endif
    }

    /// Handle incoming app link URL
    public static func handleAppLink(url: URL) {
        // Only handle HTTPS URLs
        guard url.scheme == "https" else {
            os_log(.debug, log: Self.log, "Ignoring non-HTTPS URL: %{public}@", url.absoluteString)
            return
        }

        // Call Rust FFI function to process the URL
        let result = onApplinkReceived(url.absoluteString)
        os_log(.info, log: Self.log, "AppLink: %{public}@, returned: %d", url.absoluteString, result)
    }

    /// Check if push notifications are enabled
    /// Returns true if authorized or provisional, false otherwise
    nonisolated public static func isPushEnabled() -> Bool {
        #if os(iOS)
        return iOSPushManager.isPushEnabledSync()
        #else
        // macOS doesn't support push notifications yet
        return false
        #endif
    }

    /// Show toast
    nonisolated public static func showToast(options: ToastOptions) {
        let title = options.title.toString()
        let image = options.image.toString()
        let icon = options.icon
        let duration = options.duration
        let mask = options.mask
        let position = options.position

        executeOnMain {
            LxAppToast.showToast(
                title: title,
                icon: icon,
                image: image.isEmpty ? nil : image,
                duration: duration,
                mask: mask,
                position: position
            )
        }
    }

    /// Show modal
    nonisolated public static func showModal(options: ModalOptions, callback_id: UInt64) {
        LxAppModal.showModal(options: options, callback_id: callback_id)
    }

    /// Show modal with dictionary (convenience method)
    @MainActor public static func showModal(_ options: [String: Any], callback_id: UInt64) {
        LxAppModal.showModal(options, callback_id: callback_id)
    }

    /// Show action sheet
    nonisolated public static func showActionSheet(options: ActionSheetOptions, callback_id: UInt64) {
        LxAppActionSheet.showActionSheet(options: options, callback_id: callback_id)
    }

    /// Show action sheet with dictionary (convenience method)
    @MainActor public static func showActionSheet(_ options: [String: Any], callback_id: UInt64) {
        LxAppActionSheet.showActionSheet(options, callback_id: callback_id)
    }

    /// Show popup overlay
    nonisolated public static func showPopup(
        appid: RustStr,
        path: RustStr,
        width_ratio: Double,
        height_ratio: Double,
        position: PopupPositionBridge
    ) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()
        let displayPosition = position.toDisplayPosition()

        return executeOnMain {
            LxAppPopup.showPopup(
                appId: appIdString,
                path: pathString,
                widthRatio: width_ratio,
                heightRatio: height_ratio,
                position: displayPosition
            )
        }
    }

    /// Hide popup overlay
    nonisolated public static func hidePopup(appid: RustStr) -> Bool {
        let appIdString = appid.toString()

        return executeOnMain {
            LxAppPopup.hidePopup(appId: appIdString)
        }
    }

    /// Review a document using the native in-app viewer when supported.
    nonisolated public static func reviewDocument(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        let pathString = filePath.toString()
        let mimeString = mimeType.toString()
        return executeOnMain {
            LxAppDocument.reviewDocument(
                path: pathString,
                mimeType: mimeString.isEmpty ? nil : mimeString,
                showMenu: showMenu
            )
        }
    }

    /// Open a document with the system / external app.
    nonisolated public static func openDocumentExternal(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        let pathString = filePath.toString()
        let mimeString = mimeType.toString()
        return executeOnMain {
            LxAppDocument.openExternal(
                path: pathString,
                mimeType: mimeString.isEmpty ? nil : mimeString,
                showMenu: showMenu
            )
        }
    }

    /// Legacy compatibility wrapper kept for older bridge callers.
    /// Tries native review first, then falls back to external open.
    nonisolated public static func openDocument(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        if reviewDocument(file_path: filePath, mime_type: mimeType, show_menu: showMenu) {
            return true
        }
        return openDocumentExternal(file_path: filePath, mime_type: mimeType, show_menu: showMenu)
    }

    /// Reveal a file or directory in the system file manager.
    nonisolated public static func revealInFileManager(path: RustStr) -> Bool {
        let pathString = path.toString()
        #if os(macOS)
        return executeOnMain {
            var isDirectory: ObjCBool = false
            guard FileManager.default.fileExists(atPath: pathString, isDirectory: &isDirectory) else {
                return false
            }
            let url = URL(fileURLWithPath: pathString)
            if isDirectory.boolValue {
                return NSWorkspace.shared.open(url)
            }
            NSWorkspace.shared.activateFileViewerSelecting([url])
            return true
        }
        #else
        let _ = pathString
        return false
        #endif
    }

    /// Hide current toast immediately
    nonisolated public static func hideToast() {
        executeOnMain {
            LxAppToast.hideToast()
        }
    }

}

#if os(iOS)
typealias LxAppPlatform = iOSLxApp
#elseif os(macOS)
typealias LxAppPlatform = macOSLxApp
#endif
