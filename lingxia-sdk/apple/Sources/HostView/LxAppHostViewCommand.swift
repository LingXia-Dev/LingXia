/// Commands that can be dispatched to an `LxAppHostView`.
public enum LxAppHostViewCommand: Codable, Sendable {
    case triggerCapsuleAction(LxAppCapsuleAction)
    case reload
    case goBack
    case scrollToTop
}
