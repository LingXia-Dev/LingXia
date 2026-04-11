import Foundation

/// Stable identifier for an `LxAppHostView` instance.
public struct LxAppHostViewID: Hashable, Codable, Sendable {
    public let rawValue: UUID

    public init() {
        self.rawValue = UUID()
    }

    public init(rawValue: UUID) {
        self.rawValue = rawValue
    }
}
