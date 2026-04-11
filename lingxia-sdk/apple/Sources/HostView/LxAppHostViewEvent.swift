/// Events emitted by an `LxAppHostView` through its `events` stream.
public enum LxAppHostViewEvent: Codable, Sendable {
    case didChangeTitle(String?)
    case didUpdateCanGoBack(Bool)
    case didStartLoading
    case didFinishLoading
    case didFail(LxAppErrorPayload)
}
