import Foundation
import OSLog
import CLingXiaFFI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

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
public let ACTION_SWITCH_PAGE = "com.lingxia.SWITCH_PAGE_ACTION"
public let ACTION_CLOSE_LXAPP = "com.lingxia.CLOSE_LXAPP_ACTION"

/// Core LxApp management logic shared between platforms
@MainActor
public class LxAppCore {
    private static let log = OSLog(subsystem: "LingXia", category: "LxAppCore")

    /// Singleton instance
    private static var instance: LxAppCore?

    /// Home LxApp configuration
    internal static var homeLxAppId: String?

    /// Active paths tracking
    private static var lastActivePaths: [String: String] = [:]

    private init() {}

    /// Initialize the LxApp system (internal core initialization)
    internal static func initializeCore() {
        if homeLxAppId != nil {
            return
        }
        performInitialization()
    }

    private static func performInitialization() {
        instance = LxAppCore()

        // Get platform-specific directory configuration
        let directoryConfig = LxAppDirectoryFactory.createDirectoryConfig()

        let initResult = lxappInit(directoryConfig.dataPath, directoryConfig.cachesPath)
        let initResultString = initResult?.toString()

        if let homeAppId = initResultString {
            homeLxAppId = homeAppId
        } else {
            os_log("Failed to get home LxApp ID from native init", log: log, type: .error)
        }
    }

    /// Enable WebView debugging
    internal static func enableWebViewDebugging() {
        WebViewManager.enableDebugging()
    }

    /// Set home LxApp configuration
    public static func setHomeLxApp(appId: String, initialRoute: String = "/") {
        homeLxAppId = appId
    }

    /// Get last active path for app
    public static func getLastActivePath(for appId: String, defaultPath: String = "") -> String {
        return lastActivePaths[appId] ?? defaultPath
    }

    /// Set last active path for app
    public static func setLastActivePath(_ path: String, for appId: String) {
        lastActivePaths[appId] = path
    }

    /// Check if app is home LxApp
    public static func isHomeLxApp(_ appId: String) -> Bool {
        return appId == homeLxAppId
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

    /// Initialize the LxApp system
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

    /// Set home LxApp configuration
    public static func setHomeLxApp(appId: String, initialRoute: String = "/") {
        LxAppCore.setHomeLxApp(appId: appId, initialRoute: initialRoute)
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

    /// Open home LxApp
    public static func openHomeLxApp() {
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

    /// Switch to page in LxApp
    nonisolated public static func switchPage(appid: RustStr, path: RustStr) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()

        return executeOnMain {
            LxAppPlatform.switchPage(appId: appIdString, path: pathString)
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
        // Extract values to avoid data races
        let title = options.title.toString()
        let iconInt = options.icon  // Direct i32 usage
        let image = options.image.toString()
        let duration = options.duration
        let mask = options.mask
        let position = options.position.toString()

        executeOnMain {
            LxAppToast.showToast(
                title: title,
                icon: ToastIcon.fromInt(Int(iconInt)),
                image: image.isEmpty ? nil : image,
                duration: duration,
                mask: mask,
                position: ToastPosition.fromString(position)
            )
        }
    }

    /// Show modal
    nonisolated public static func showModal(options: ModalOptions) -> ModalResult {
        return LxAppModal.showModal(options: options)
    }

    /// Show modal with dictionary (convenience method)
    @MainActor public static func showModal(_ options: [String: Any]) -> ModalResult {
        return LxAppModal.showModal(options)
    }

    /// Show toast
    public static func showToast(_ options: [String: Any]) {
        LxAppToast.showToast(options)
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
