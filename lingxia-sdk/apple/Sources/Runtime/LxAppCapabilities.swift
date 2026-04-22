/// Bitmask of host-app capabilities, queried from Rust after initialization.
public struct LxAppCapabilities: OptionSet, Sendable, Codable, Hashable {
    public let rawValue: UInt32

    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    /// The host app includes the shell feature (sidebar, toolbar, chrome).
    public static let shell = LxAppCapabilities(rawValue: 0x1)

    /// The host app enables push/notification integration.
    public static let notifications = LxAppCapabilities(rawValue: 0x2)
}
