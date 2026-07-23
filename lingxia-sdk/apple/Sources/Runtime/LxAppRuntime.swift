import Foundation
import OSLog

/// The entry point for the LingXia runtime.
///
/// `LxAppRuntime` owns initialization and exposes read-only runtime info.
/// It deliberately does **not** touch UIKit/AppKit — no windows, no views.
///
/// ```swift
/// let info = try await LxAppRuntime.shared.initialize()
/// print(info.lxAppId)
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

    /// Nonisolated mirror of `isInitialized`, set once at the end of a
    /// successful `initialize()`. `LxAppRuntime` is `@MainActor`-isolated, so
    /// `shared.isInitialized` cannot be read synchronously from a nonisolated
    /// context; callers that must (e.g. `Lingxia.displayLanguage`, reachable
    /// from background threads) use this instead. Single writer (`initialize()`
    /// on the main actor), many readers — safe without a lock.
    private nonisolated(unsafe) static var didInitializeUnsafe = false

    /// Nonisolated-safe equivalent of `shared.isInitialized`.
    public nonisolated static var isInitializedUnsafe: Bool { didInitializeUnsafe }

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
    /// - Throws: `LxAppRuntimeError.initializationFailed` if Rust reports failure.
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
        let initResult = lingxiaInit(dirs.dataPath, dirs.cachesPath, locale)
        guard initResult.ok else {
            throw LxAppRuntimeError.initializationFailed(
                message: initResult.error.toString()
            )
        }

        let rawLxAppId = initResult.home_app_id.toString()
        let lxAppId = rawLxAppId.isEmpty ? nil : rawLxAppId

        let caps = LxAppCapabilities(rawValue: getAppCapabilities())

        let runtimeInfo = LxAppRuntimeInfo(
            lxAppId: lxAppId,
            capabilities: caps,
            dataPath: dirs.dataPath,
            cachesPath: dirs.cachesPath
        )

        self.info = runtimeInfo
        Self.didInitializeUnsafe = true

        LxAppCore.homeLxAppId = lxAppId
        LxAppCore.capabilities = caps.rawValue

        os_log(
            "LxAppRuntime initialized — lxapp: %{public}@ capabilities=%{public}u browser=%{public}@",
            log: Self.log,
            type: .info,
            lxAppId ?? "none",
            caps.rawValue,
            caps.contains(.browser) ? "true" : "false"
        )

        return runtimeInfo
    }
}
