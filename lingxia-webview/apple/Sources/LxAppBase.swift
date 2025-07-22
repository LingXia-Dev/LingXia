import Foundation
import WebKit
import os.log
import CLingXiaFFI

/// Notification action identifiers
public let ACTION_SWITCH_PAGE = "com.lingxia.SWITCH_PAGE_ACTION"
public let ACTION_CLOSE_LXAPP = "com.lingxia.CLOSE_LXAPP_ACTION"

/// LxApp launch mode for iOS
public enum LxAppLaunchMode {
    case replaceRoot
    case modal
}

/// Platform-specific directory configuration
public struct LxAppDirectoryConfig {
    public let dataPath: String
    public let cachesPath: String

    public init(dataPath: String, cachesPath: String) {
        self.dataPath = dataPath
        self.cachesPath = cachesPath
    }
}

/// Protocol for platform-specific directory configuration
public protocol LxAppPlatformDirectoryProvider {
    static func getDirectoryConfig() -> LxAppDirectoryConfig
}

/// Common directory provider utilities
public struct LxAppDirectoryProviderUtils {

    /// Creates directory if it doesn't exist
    public static func ensureDirectoryExists(at path: String) throws {
        try FileManager.default.createDirectory(
            atPath: path,
            withIntermediateDirectories: true,
            attributes: nil
        )
    }

    /// Gets bundle identifier safely
    public static func getBundleIdentifier() throws -> String {
        guard let bundleId = Bundle.main.bundleIdentifier else {
            throw LxAppDirectoryError.bundleIdentifierNotFound
        }
        return bundleId
    }

    /// Gets system directory URL safely
    public static func getSystemDirectoryURL(for directory: FileManager.SearchPathDirectory) throws -> URL {
        guard let url = FileManager.default.urls(for: directory, in: .userDomainMask).first else {
            throw LxAppDirectoryError.systemDirectoryNotFound(directory)
        }
        return url
    }

    /// Creates app-specific directory configuration
    public static func createAppDirectoryConfig(
        dataDirectory: FileManager.SearchPathDirectory,
        cacheDirectory: FileManager.SearchPathDirectory
    ) throws -> LxAppDirectoryConfig {
        let bundleId = try getBundleIdentifier()

        let dataURL = try getSystemDirectoryURL(for: dataDirectory)
        let cacheURL = try getSystemDirectoryURL(for: cacheDirectory)

        let dataPath = dataURL.appendingPathComponent(bundleId).path
        let cachePath = cacheURL.appendingPathComponent(bundleId).path

        try ensureDirectoryExists(at: dataPath)
        try ensureDirectoryExists(at: cachePath)

        return LxAppDirectoryConfig(dataPath: dataPath, cachesPath: cachePath)
    }
}

/// Directory provider errors
public enum LxAppDirectoryError: Error {
    case bundleIdentifierNotFound
    case systemDirectoryNotFound(FileManager.SearchPathDirectory)
    case directoryCreationFailed(String)
}

/// Shared FFI utilities for handling cross-platform FFI calls
@MainActor
public struct FFIUtils {

    /// Execute FFI call on main thread with proper isolation handling
    nonisolated public static func executeFFICall<T: Sendable>(
        _ call: @MainActor () -> T
    ) -> T {
        if Thread.isMainThread {
            return MainActor.assumeIsolated(call)
        } else {
            return DispatchQueue.main.sync {
                MainActor.assumeIsolated(call)
            }
        }
    }

    /// Convert RustStr parameters and execute FFI call
    nonisolated public static func executeFFICallWithRustStr<T: Sendable>(
        appid: RustStr,
        path: RustStr? = nil,
        _ call: @MainActor (String, String?) -> T
    ) -> T {
        let appIdString = appid.toString()
        let pathString = path?.toString()

        return executeFFICall {
            call(appIdString, pathString)
        }
    }

    /// Convert single RustStr parameter and execute FFI call
    nonisolated public static func executeFFICallWithSingleRustStr<T: Sendable>(
        appid: RustStr,
        _ call: @MainActor (String) -> T
    ) -> T {
        let appIdString = appid.toString()

        return executeFFICall {
            call(appIdString)
        }
    }
}

/// Unified directory provider factory for all platforms
public struct LxAppDirectoryFactory {

    /// Create platform-specific directory configuration
    public static func createDirectoryConfig() -> LxAppDirectoryConfig {
        do {
            #if os(iOS)
            return try LxAppDirectoryProviderUtils.createAppDirectoryConfig(
                dataDirectory: .documentDirectory,
                cacheDirectory: .cachesDirectory
            )
            #elseif os(macOS)
            return try LxAppDirectoryProviderUtils.createAppDirectoryConfig(
                dataDirectory: .applicationSupportDirectory,
                cacheDirectory: .cachesDirectory
            )
            #endif
        } catch {
            fatalError("Failed to create directory config: \(error)")
        }
    }
}

#if os(iOS)
/// iOS directory provider (legacy compatibility)
public struct iOSDirectoryProvider: LxAppPlatformDirectoryProvider {
    public static func getDirectoryConfig() -> LxAppDirectoryConfig {
        return LxAppDirectoryFactory.createDirectoryConfig()
    }
}
#endif

#if os(macOS)
/// macOS Directory Provider (legacy compatibility)
public struct macOSDirectoryProvider: LxAppPlatformDirectoryProvider {
    public static func getDirectoryConfig() -> LxAppDirectoryConfig {
        return LxAppDirectoryFactory.createDirectoryConfig()
    }
}
#endif

/// Core LxApp management logic shared between platforms
@MainActor
public class LxAppCore {
    private static let log = OSLog(subsystem: "LingXia", category: "LxAppCore")

    /// Singleton instance
    private static var instance: LxAppCore?

    /// Home LxApp configuration
    internal static var homeLxAppId: String?
    internal static var homeLxAppInitialRoute: String?

    /// Active paths tracking
    private static var lastActivePaths: [String: String] = [:]

    /// Launch mode for iOS
    private static var launchMode: LxAppLaunchMode = .replaceRoot

    /// Window size for macOS
    private static var windowSize: (width: CGFloat, height: CGFloat) = (414, 896)

    /// Platform directory provider
    private static var directoryProvider: LxAppPlatformDirectoryProvider.Type?

    private init() {}

    /// Set the platform directory provider
    public static func setPlatformDirectoryProvider(_ provider: LxAppPlatformDirectoryProvider.Type) {
        directoryProvider = provider
    }

    /// Initialize the LxApp system
    public static func initialize() {
        if homeLxAppId != nil {
            os_log("LxAppCore.initialize() already called, skipping", log: log, type: .info)
            return
        }
        performInitialization()
    }

    private static func performInitialization() {
        instance = LxAppCore()

        // Get platform-specific directory configuration
        guard let provider = directoryProvider else {
            fatalError("Platform directory provider not set. Call setPlatformDirectoryProvider() before initialize()")
        }

        let directoryConfig = provider.getDirectoryConfig()
        os_log("Initializing LxApp with data_dir: %@, cache_dir: %@", log: log, type: .info, directoryConfig.dataPath, directoryConfig.cachesPath)

        let initResult = lxappInit(directoryConfig.dataPath, directoryConfig.cachesPath)
        let initResultString = initResult?.toString()

        if let initResult = initResultString {
            let parts = initResult.components(separatedBy: ":")
            if parts.count >= 2 {
                homeLxAppId = parts[0]
                homeLxAppInitialRoute = Array(parts[1...]).joined(separator: ":")
                os_log("Initialized with home app: %@ at %@", log: log, type: .info, homeLxAppId!, homeLxAppInitialRoute!)
            } else {
                os_log("Failed to parse home LxApp details: %@", log: log, type: .error, initResult)
            }
        } else {
            os_log("Failed to get home LxApp details from native init", log: log, type: .error)
        }


    }

    /// Set home LxApp configuration
    public static func setHomeLxApp(appId: String, initialRoute: String = "/") {
        homeLxAppId = appId
        homeLxAppInitialRoute = initialRoute
    }

    /// Set home LxApp ID
    public static func setHomeLxAppId(_ appId: String) {
        homeLxAppId = appId
    }

    /// Set home LxApp initial route
    public static func setHomeLxAppInitialRoute(_ route: String) {
        homeLxAppInitialRoute = route
    }

    /// Get last active path for app
    public static func getLastActivePath(for appId: String, defaultPath: String = "") -> String {
        return lastActivePaths[appId] ?? defaultPath
    }

    /// Set last active path for app
    public static func setLastActivePath(_ path: String, for appId: String) {
        lastActivePaths[appId] = path
    }

    /// Set launch mode for iOS
    public static func setLaunchMode(_ mode: LxAppLaunchMode) {
        launchMode = mode
    }

    /// Get launch mode for iOS
    public static func getLaunchMode() -> LxAppLaunchMode {
        return launchMode
    }

    /// Set window size for macOS
    public static func setWindowSize(width: CGFloat, height: CGFloat) {
        windowSize = (width, height)
    }

    /// Get window size for macOS
    internal static func getWindowSize() -> (width: CGFloat, height: CGFloat) {
        return windowSize
    }

    /// Check if app is home LxApp
    public static func isHomeLxApp(_ appId: String) -> Bool {
        return appId == homeLxAppId
    }

    /// Get home LxApp ID
    public static func getHomeLxAppId() -> String? {
        return homeLxAppId
    }

    /// Get home LxApp initial route
    public static func getHomeLxAppInitialRoute() -> String {
        return homeLxAppInitialRoute ?? "/"
    }
}
