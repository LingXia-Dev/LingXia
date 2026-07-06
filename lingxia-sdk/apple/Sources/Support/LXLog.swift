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
    ///
    /// `message` is a plain `String`, not an autoclosure: `forwardHostLog`
    /// evaluates and dispatches unconditionally, so a deferred closure would
    /// buy nothing and only imply a laziness that doesn't exist.
    /// - Parameters:
    ///   - message: Fully-formatted message.
    ///   - category: Subsystem/category label, surfaced as the log target.
    ///   - appId: Owning lxapp id, when known. Empty for host-global logs.
    ///   - path: Page path within the lxapp, when known.
    @discardableResult
    static func log(
        _ level: Level,
        _ message: String,
        category: String,
        appId: String = "",
        path: String = ""
    ) -> Bool {
        forwardHostLog(level.rawValue, category, appId, path, message)
    }

    @discardableResult
    static func verbose(_ message: String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.verbose, message, category: category, appId: appId, path: path)
    }

    @discardableResult
    static func debug(_ message: String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.debug, message, category: category, appId: appId, path: path)
    }

    @discardableResult
    static func info(_ message: String, category: String, appId: String = "", path: String = "") -> Bool {
        log(.info, message, category: category, appId: appId, path: path)
    }

    /// `error:` mirrors Android/Harmony `w(tag, msg, tr)`: when present its
    /// description is appended so caught errors carry context across platforms.
    @discardableResult
    static func warn(_ message: String, category: String, appId: String = "", path: String = "", error: Error? = nil) -> Bool {
        log(.warn, Self.appending(error, to: message), category: category, appId: appId, path: path)
    }

    @discardableResult
    static func error(_ message: String, category: String, appId: String = "", path: String = "", error: Error? = nil) -> Bool {
        log(.error, Self.appending(error, to: message), category: category, appId: appId, path: path)
    }

    private static func appending(_ error: Error?, to message: String) -> String {
        guard let error else { return message }
        return "\(message)\n\(error)"
    }
}
