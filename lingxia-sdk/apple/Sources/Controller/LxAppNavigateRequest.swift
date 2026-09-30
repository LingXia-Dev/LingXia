/// A request to navigate within an existing LxApp session.
public struct LxAppNavigateRequest: Codable, Sendable {
    public var sessionId: LxAppSessionID
    public var path: String
    /// The page instance to present. Nil presents the session's current page.
    public var pageInstanceId: String?
    public var animation: LxAppAnimation

    public init(
        sessionId: LxAppSessionID,
        path: String,
        pageInstanceId: String? = nil,
        animation: LxAppAnimation = .none
    ) {
        self.sessionId = sessionId
        self.path = path
        self.pageInstanceId = pageInstanceId
        self.animation = animation
    }
}
