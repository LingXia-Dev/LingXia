/// Bitmask of host-app capabilities, queried from Rust after initialization.
public struct LxAppCapabilities: OptionSet, Sendable, Codable, Hashable {
    public let rawValue: UInt32

    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    /// The host app includes the built-in browser runtime.
    public static let browser = LxAppCapabilities(rawValue: 0x1)

    /// The host app enables push/notification integration.
    public static let notifications = LxAppCapabilities(rawValue: 0x2)

    /// The host app includes the built-in terminal runtime.
    public static let terminal = LxAppCapabilities(rawValue: 0x4)

    /// The host app includes browser proxy support.
    public static let proxy = LxAppCapabilities(rawValue: 0x8)
}
