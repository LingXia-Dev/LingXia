/// Named interceptor kinds for decision points on `LxAppController`.
///
/// Interceptors are registered by kind and return an async decision. The last
/// registration wins.
public enum LxAppControllerInterceptor: String, Codable, Sendable {
    /// Host decides how to present the opening LxApp. Return
    /// `.mountInHost(id:)` to point the controller at a pre-created host view,
    /// `.handled` to take over entirely, or `nil` to defer to default.
    case willOpen = "will_open"

    /// Host can veto a close. Return `.handled` to take over (host closes),
    /// `.reject` to block, or `nil` to defer.
    case shouldClose = "should_close"
}

/// Context passed to an interceptor handler.
public struct LxAppInterceptContext: Codable, Sendable {
    public let controllerId: LxAppControllerID
    public let payload: LxAppJSONValue

    public init(controllerId: LxAppControllerID, payload: LxAppJSONValue) {
        self.controllerId = controllerId
        self.payload = payload
    }
}

/// The decision an interceptor handler returns.
public enum LxAppInterceptDecision: Codable, Sendable {
    /// Caller took over entirely; controller does nothing.
    case handled
    /// Controller mounts the LxApp inside a pre-existing host view.
    case mountInHost(id: LxAppHostViewID)
    /// Block the action (only valid for `shouldClose`).
    case reject(reason: String)
}
