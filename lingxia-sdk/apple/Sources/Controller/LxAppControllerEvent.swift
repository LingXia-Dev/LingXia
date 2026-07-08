/// Events emitted by an `LxAppController` through its `events` stream.
public enum LxAppControllerEvent: Codable, Sendable {
    /// A session is about to be opened.
    case willOpen(LxAppOpenRequest)
    /// A session was successfully opened.
    case didOpen(LxAppSession)
    /// Navigation occurred within a session. `animation` is the transition the
    /// navigation requested (push/pop/fade/none) so hosts can animate the swap.
    case didNavigate(sessionId: LxAppSessionID, to: String, animation: LxAppAnimation)
    /// A session was closed.
    case didClose(LxAppSession)
    /// Opening a session failed.
    case didFailOpen(request: LxAppOpenRequest, error: LxAppErrorPayload)
}
