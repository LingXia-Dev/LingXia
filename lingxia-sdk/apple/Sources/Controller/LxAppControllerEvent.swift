/// Events emitted by an `LxAppController` through its `events` stream.
public enum LxAppControllerEvent: Codable, Sendable {
    /// A session is about to be opened.
    case willOpen(LxAppOpenRequest)
    /// A session was successfully opened.
    case didOpen(LxAppSession)
    /// Navigation occurred within a session.
    case didNavigate(sessionId: LxAppSessionID, to: String)
    /// A session was closed.
    case didClose(LxAppSession)
    /// Opening a session failed.
    case didFailOpen(request: LxAppOpenRequest, error: LxAppErrorPayload)
}
