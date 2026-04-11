/// Errors thrown by `LxAppRuntime.initialize()`.
public enum LxAppRuntimeError: Error, Codable, Sendable {
    /// `initialize()` was called more than once.
    case alreadyInitialized
    /// The Rust `lingxiaInit` call returned nil (no home app configured).
    case initializationFailed(message: String)
}
