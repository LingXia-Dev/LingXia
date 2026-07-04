import CLingXiaRustAPI

/// Routes SDK-native logs into the LingXia Rust log pipeline.
///
/// Unlike `os_log`, records emitted here flow through the same pipeline as Rust
/// logs: they reach the platform sink *and* the dev-server stream, so they show
/// up in `lxdev logs` tagged with the originating `appId`/`path`.
///
/// Prefer this over `os_log` for any log a host/lxapp developer should be able
/// to observe. Pure platform-rendering / high-frequency traces may stay on
/// `os_log`.
enum LXLog {
    /// Mirrors the Rust FFI level contract (see `logging::forward_host_log`).
    enum Level: Int32 {
        case verbose = 0
        case debug = 1
        case info = 2
        case warn = 3
        case error = 4
    }

    /// Forward a log entry into the Rust pipeline.
    /// - Parameters:
    ///   - message: Fully-formatted message.
    ///   - category: Subsystem/category label, surfaced as the log target.
    ///   - appId: Owning lxapp id, when known. Empty for host-global logs.
    ///   - path: Page path within the lxapp, when known.
    @discardableResult
    static func log(
        _ level: Level,
        _ message: @autoclosure () -> String,
        category: String,
        appId: String = "",
        path: String = ""
    ) -> Bool {
        forwardHostLog(level.rawValue, category, appId, path, message())
    }

    @discardableResult
    static func verbose(_ message: @autoclosure () -> String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.verbose, message(), category: category, appId: appId, path: path)
    }

    @discardableResult
    static func debug(_ message: @autoclosure () -> String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.debug, message(), category: category, appId: appId, path: path)
    }

    @discardableResult
    static func info(_ message: @autoclosure () -> String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.info, message(), category: category, appId: appId, path: path)
    }

    @discardableResult
    static func warn(_ message: @autoclosure () -> String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.warn, message(), category: category, appId: appId, path: path)
    }

    @discardableResult
    static func error(_ message: @autoclosure () -> String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.error, message(), category: category, appId: appId, path: path)
    }
}
