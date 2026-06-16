/// How the sidebar is configured in an `LxAppShell`.
public enum LxAppSidebarMode: Sendable {
    /// No sidebar. The content fills the entire window.
    case hidden

    /// Sidebar rendered from a declarative `LxAppSidebarTree`.
    case declarative(LxAppSidebarTree)

    /// Sidebar provided by a Swift-native `LxAppSidebarProviding` implementation.
    case swiftNative(LxAppSidebarHandle)
}

// MARK: - Codable (partial)

extension LxAppSidebarMode: Codable {
    private enum CodingKeys: String, CodingKey {
        case type, tree
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "hidden":
            self = .hidden
        case "declarative":
            let tree = try container.decode(LxAppSidebarTree.self, forKey: .tree)
            self = .declarative(tree)
        case "__swiftNative":
            // Cannot reconstruct from JSON; decode as hidden.
            self = .hidden
        default:
            self = .hidden
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .hidden:
            try container.encode("hidden", forKey: .type)
        case .declarative(let tree):
            try container.encode("declarative", forKey: .type)
            try container.encode(tree, forKey: .tree)
        case .swiftNative:
            try container.encode("__swiftNative", forKey: .type)
        }
    }
}
