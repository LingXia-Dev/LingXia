import Foundation
import OSLog

/// The entry point for the LingXia runtime.
///
/// `LxAppRuntime` owns initialization and exposes read-only runtime info.
/// It deliberately does **not** touch UIKit/AppKit — no windows, no views.
///
/// ```swift
/// let info = try await LxAppRuntime.shared.initialize()
/// print(info.homeAppId)
/// ```
@MainActor
public final class LxAppRuntime {

    /// Singleton instance.
    public static let shared = LxAppRuntime()

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppRuntime")

    /// Post-init snapshot; `nil` until `initialize()` succeeds.
    public private(set) var info: LxAppRuntimeInfo?

    /// Whether the runtime has been successfully initialized.
    public var isInitialized: Bool { info != nil }

    private init() {}

    // MARK: - Initialize

    /// Initialize the LingXia runtime.
    ///
    /// This method:
    /// 1. Registers WebView runtime classes.
    /// 2. Installs the native host addon (Rust FFI discovery, once).
    /// 3. Calls `lingxiaInit` with platform-specific directories.
    /// 4. Populates `info` with the results.
    ///
    /// - Throws: `LxAppRuntimeError.alreadyInitialized` on double-init.
    /// - Throws: `LxAppRuntimeError.initializationFailed` if Rust returns nil.
    /// - Returns: The `LxAppRuntimeInfo` snapshot.
    @discardableResult
    public func initialize() throws -> LxAppRuntimeInfo {
        guard info == nil else {
            throw LxAppRuntimeError.alreadyInitialized
        }

        // 1. Register WKWebView runtime classes (idempotent internally).
        WebViewManager.registerRuntimeClasses()

        // 2. Native host addon (Rust FFI symbol discovery).
        LxAppCore.registerNativeHostAddonOnce()

        // 3. Resolve directories.
        let dirs = LxAppDirectoryFactory.createDirectoryConfig()

        // 4. Call Rust init.
        let locale = Locale.current.identifier
        guard let initResult = lingxiaInit(dirs.dataPath, dirs.cachesPath, locale) else {
            throw LxAppRuntimeError.initializationFailed(
                message: "lingxiaInit returned nil — check lingxia.config.json"
            )
        }

        let homeAppId = initResult.toString()
        guard !homeAppId.isEmpty else {
            throw LxAppRuntimeError.initializationFailed(
                message: "lingxiaInit returned empty home app id"
            )
        }

        let caps = LxAppCapabilities(rawValue: getAppCapabilities())

        let runtimeInfo = LxAppRuntimeInfo(
            homeAppId: homeAppId,
            capabilities: caps,
            dataPath: dirs.dataPath,
            cachesPath: dirs.cachesPath
        )

        self.info = runtimeInfo

        LxAppCore.homeLxAppId = homeAppId
        LxAppCore.capabilities = caps.rawValue

        os_log(
            "LxAppRuntime initialized — home: %{public}@ capabilities=%{public}u browser=%{public}@",
            log: Self.log,
            type: .info,
            homeAppId,
            caps.rawValue,
            caps.contains(.browser) ? "true" : "false"
        )

        return runtimeInfo
    }
}
