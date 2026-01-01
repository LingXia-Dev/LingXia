import Foundation
import OSLog
import WebKit
import CLingXiaRustAPI
import CLingXiaSwiftAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public struct LxAppUIEvent {
    // UI Event Type Constants - using functions to avoid concurrency issues
    public static var tabBarClick: UiEventType { UiEventType.TabBarClick }
    public static var capsuleClick: UiEventType { UiEventType.CapsuleClick }
    public static var navigationClick: UiEventType { UiEventType.NavigationClick }
    public static var backPress: UiEventType { UiEventType.BackPress }
    public static var pullDownRefresh: UiEventType { UiEventType.PullDownRefresh }

    // UI Event Data Constants
    public static let capsuleActionMore = "more"
    public static let capsuleActionClose = "close"
    public static let navigationActionBack = "back"
    public static let navigationActionHome = "home"
}

/// Animation type enum for page transitions
public enum AnimationType: Sendable {
    case none      // No animation
    case forward   // Forward animation (push-style)
    case backward  // Backward animation (pop-style)
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

    /// Create platform-specific directory configuration
    public static func createDirectoryConfig() -> LxAppDirectoryConfig {
        do {
            guard let bundleId = Bundle.main.bundleIdentifier else {
                throw LxAppDirectoryError.bundleIdentifierNotFound
            }

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
public class LxAppViewHierarchyHelper {
    /// Finds the topmost view controller in the hierarchy
    public static func findTopmostViewController(from viewController: UIViewController) -> UIViewController {
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
    public static func findSpecificViewController<T>(in viewController: UIViewController?) -> T? {
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

    /// Global current app state - shared across iOS and macOS
    public private(set) static var currentAppId: String?
    private static var currentPath: String = ""

    /// Current WebView - cached to avoid frequent findWebView calls
    private static var currentWebView: WKWebView?

    private init() {}

    /// Shared openLxApp logic - used by both iOS and macOS platforms
    internal static func executeOpenLxApp(appId: String, path: String) {

        // Call onLxappOpened to get the resolved path
        let resolvedPath = onLxappOpened(appId, path)
        let finalPath = resolvedPath.toString()

        // Direct platform calls instead of using renderer protocol
        #if os(iOS)
        iOSLxApp.openLxAppDirect(appId: appId, path: finalPath)
        #elseif os(macOS)
        macOSLxApp.openLxAppDirect(appId: appId, path: finalPath)
        #endif
    }

    /// Shared navigate logic - used by both iOS and macOS platforms
    internal static func executeNavigation(appId: String, path: String, animationType: AnimationType) {
        os_log("Core executeNavigation: %@ to %@ with type: %@", log: log, type: .info, appId, path, String(describing: animationType))

        guard !appId.isEmpty else {
            os_log("Empty appId provided for navigation", log: log, type: .error)
            return
        }

        // Direct platform calls - no need for complex preparation logic
        #if os(iOS)
        iOSLxApp.handleNavigationDirect(appId: appId, path: path, animationType: animationType)
        #elseif os(macOS)
        macOSLxApp.handleNavigationDirect(appId: appId, path: path, animationType: animationType)
        #endif
    }

    /// Closure to register custom extensions. Set this before calling initialize().
    nonisolated(unsafe) internal static var registerExtensions: (() -> Void)?

    nonisolated(unsafe) private static var extensionsRegistered = false

    /// Initialize the LxApp system (internal core initialization)
    internal static func initializeCore() {
        if instance != nil {
            return
        }

        // Register extensions once before initialization
        if !extensionsRegistered {
            registerExtensions?()
            extensionsRegistered = true
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

        let initResult = lxappInit(directoryConfig.dataPath, directoryConfig.cachesPath, locale)
        let initResultString = initResult?.toString()

        if let homeAppId = initResultString {
            homeLxAppId = homeAppId
            os_log("LxApp initialized successfully with home app: %{public}@", log: log, type: .info, homeAppId)

            // Auto-open home lxapp after initialization
            DispatchQueue.main.async {
                LxAppPlatform.openHomeLxApp()
            }
        } else {
            os_log("Failed to get home LxApp ID from native init", log: log, type: .error)
        }
    }

    /// Enable WebView debugging
    internal static func enableWebViewDebugging() {
        WebViewManager.enableDebugging()
    }

    /// Check if app is home LxApp
    public static func isHomeLxApp(_ appId: String) -> Bool {
        return appId == homeLxAppId
    }

    /// Set current app state - shared across platforms
    public static func setCurrentApp(appId: String, path: String) {
        currentAppId = appId
        currentPath = path

        // Update WebView cache when app/path changes
        currentWebView = WebViewManager.findWebView(appId: appId, path: path)
    }

    /// Get current path for active app - always returns definitive value, never nil
    public static func getCurrentPath() -> String {
        guard currentAppId != nil else { return "/" }
        return currentPath.isEmpty ? "/" : currentPath
    }

    /// Update current path
    public static func setCurrentPath(_ path: String) {
        guard let appId = currentAppId else { return }
        currentPath = path

        // Update WebView cache when path changes
        currentWebView = WebViewManager.findWebView(appId: appId, path: path)
    }

    /// Get current WebView - cached for efficiency
    public static func getCurrentWebView() -> WKWebView? {
        return currentWebView
    }

    /// Get home LxApp ID
    public static func getHomeLxAppId() -> String? {
        return homeLxAppId
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

    /// Closure to register custom extensions. Set this before calling initialize().
    nonisolated(unsafe) public static var registerExtensions: (() -> Void)? {
        get { LxAppCore.registerExtensions }
        set { LxAppCore.registerExtensions = newValue }
    }

    /// Initialize the LxApp system and automatically open Home LxApp
    public static func initialize() {
        // Initialize core first
        LxAppCore.initializeCore()

        // Then initialize platform-specific components
        #if os(iOS)
        iOSLxApp.initialize()
        #elseif os(macOS)
        _ = macOSLxApp.initialize()
        #endif
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
    nonisolated public static func openLxApp(appid: RustStr, path: RustStr) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()

        return executeOnMain {
            LxAppPlatform.openLxApp(appId: appIdString, path: pathString)
            return true
        }
    }

    /// Close LxApp
    nonisolated public static func closeLxApp(appid: RustStr) -> Bool {
        let appIdString = appid.toString()

        return executeOnMain {
            LxAppPlatform.closeLxApp(appId: appIdString)
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
            #if os(macOS)
            let activeControllers = macOSLxApp.getActiveWindowControllers()
            for windowController in activeControllers {
                if windowController.appId == appIdString, let path = windowController.path {
                    let navState = lingxia.getNavigationBarState(appIdString, path)
                    windowController.updateNavigationBarWithState(navState)
                    break
                }
            }
            #elseif os(iOS)
            NavigationBarStateManager.shared.refreshState(for: appIdString)
            #endif
            return true
        }
    }

    nonisolated public static func launchWithUrl(url: RustStr) {
        let urlString = url.toString()
        guard let url = URL(string: urlString) else {
            os_log(.error, log: Self.log, "Invalid URL for launchWithUrl: %{public}@", urlString)
            return
        }
        #if os(iOS)
        DispatchQueue.main.async {
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }
        #elseif os(macOS)
        NSWorkspace.shared.open(url)
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

    /// Open a document using the system viewer
    nonisolated public static func openDocument(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        #if os(iOS)
        let pathString = filePath.toString()
        let mimeString = mimeType.toString()
        return executeOnMain {
            LxAppDocument.openDocument(
                path: pathString,
                mimeType: mimeString.isEmpty ? nil : mimeString,
                showMenu: showMenu
            )
        }
        #elseif os(macOS)
        let pathString = filePath.toString()
        let url = URL(fileURLWithPath: pathString)
        let _ = (mimeType, showMenu)
        return NSWorkspace.shared.open(url)
        #else
        let _ = (filePath, mimeType, showMenu)
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
