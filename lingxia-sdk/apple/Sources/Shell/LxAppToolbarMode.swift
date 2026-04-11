/// How the toolbar is configured in an `LxAppShell`.
public enum LxAppToolbarMode: Sendable {
    /// No toolbar.
    case hidden

    /// Toolbar rendered from a declarative `LxAppToolbarSpec`.
    case declarative(LxAppToolbarSpec)

    /// Toolbar provided by a Swift-native `LxAppToolbarProviding` implementation.
    case swiftNative(LxAppToolbarHandle)
}

// MARK: - Codable (partial)

extension LxAppToolbarMode: Codable {
    private enum CodingKeys: String, CodingKey {
        case type, spec
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "hidden":
            self = .hidden
        case "declarative":
            let spec = try container.decode(LxAppToolbarSpec.self, forKey: .spec)
            self = .declarative(spec)
        case "__swiftNative":
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
        case .declarative(let spec):
            try container.encode("declarative", forKey: .type)
            try container.encode(spec, forKey: .spec)
        case .swiftNative:
            try container.encode("__swiftNative", forKey: .type)
        }
    }
}
