/// A structured error that can cross the Swift ↔ JS boundary.
///
/// `code` is a stable snake_case identifier (e.g. `"home_app_unavailable"`,
/// `"rust_rejected_session"`). `message` is human-readable. `details` carries
/// any additional structured context.
public struct LxAppErrorPayload: Codable, Sendable, Error, Hashable {
    /// Stable, cross-language error code (snake_case).
    public let code: String
    /// Human-readable description.
    public let message: String
    /// Optional structured context.
    public let details: LxAppJSONValue?

    public init(code: String, message: String, details: LxAppJSONValue? = nil) {
        self.code = code
        self.message = message
        self.details = details
    }
}
