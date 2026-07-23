/// Errors thrown by `LxAppRuntime.initialize()`.
public enum LxAppRuntimeError: Error, Codable, Sendable {
    /// `initialize()` was called more than once.
    case alreadyInitialized
    /// The Rust runtime reported an initialization failure.
    case initializationFailed(message: String)
}
