import Foundation

/// Represents an active LxApp session managed by an `LxAppController`.
public struct LxAppSession: Hashable, Codable, Sendable, Identifiable {
    public let id: LxAppSessionID
    public let appId: String
    public internal(set) var path: String
    public let presentation: LxAppOpenPresentation
    public internal(set) var userInfo: [String: LxAppJSONValue]
    public let openedAt: Date

    public init(
        id: LxAppSessionID,
        appId: String,
        path: String,
        presentation: LxAppOpenPresentation = .normal,
        userInfo: [String: LxAppJSONValue] = [:],
        openedAt: Date = Date()
    ) {
        self.id = id
        self.appId = appId
        self.path = path
        self.presentation = presentation
        self.userInfo = userInfo
        self.openedAt = openedAt
    }
}
