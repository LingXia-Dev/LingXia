import Foundation

/// Stable identifier for an `LxAppController` instance.
public struct LxAppControllerID: Hashable, Codable, Sendable {
    public let rawValue: UUID

    public init() {
        self.rawValue = UUID()
    }

    public init(rawValue: UUID) {
        self.rawValue = rawValue
    }
}
