/// A request to navigate within an existing LxApp session.
public struct LxAppNavigateRequest: Codable, Sendable {
    public var sessionId: LxAppSessionID
    public var path: String
    public var animation: LxAppAnimation

    public init(
        sessionId: LxAppSessionID,
        path: String,
        animation: LxAppAnimation = .none
    ) {
        self.sessionId = sessionId
        self.path = path
        self.animation = animation
    }
}
