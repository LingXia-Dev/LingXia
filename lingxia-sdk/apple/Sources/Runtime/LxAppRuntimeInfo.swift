/// Read-only snapshot of runtime state after successful initialization.
public struct LxAppRuntimeInfo: Codable, Sendable, Hashable {
    /// The app-id of the home LxApp configured in `lingxia.config.json`.
    public let homeAppId: String

    /// Bitmask of host-app capabilities (e.g. `.shell`).
    public let capabilities: LxAppCapabilities

    /// Raw JSON string of the panels configuration, if any.
    public let panelsConfigJson: String?

    /// Absolute path to the data directory used by this runtime.
    public let dataPath: String

    /// Absolute path to the caches directory used by this runtime.
    public let cachesPath: String
}
