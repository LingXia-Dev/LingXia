/// Stable identifier for an LxApp session.
public struct LxAppSessionID: Hashable, Codable, Sendable {
    public let rawValue: UInt64

    public init(rawValue: UInt64) {
        self.rawValue = rawValue
    }
}
