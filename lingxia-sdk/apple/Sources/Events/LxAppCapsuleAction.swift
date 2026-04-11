/// Actions a user can trigger from the capsule menu.
public enum LxAppCapsuleAction: String, Codable, Sendable, CaseIterable {
    case close
    case cleanCacheRestart = "clean_cache_restart"
    case restart
    case uninstall
}
