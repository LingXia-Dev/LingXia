import Foundation

public struct LxAppMoreActionItem: Decodable {
    public let label: String
    public let iconPath: String

    private enum CodingKeys: String, CodingKey {
        case label, iconPath
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        label = try values.decode(String.self, forKey: .label)
        iconPath = try values.decodeIfPresent(String.self, forKey: .iconPath) ?? ""
    }
}

public struct LxAppMoreActionSnapshot: Decodable {
    public let generation: UInt64
    public let items: [LxAppMoreActionItem]

    public static func load(appId: String) -> Self {
        let json = getLxAppMoreActions(appId).toString()
        guard let data = json.data(using: .utf8), !json.isEmpty else {
            return Self(generation: 0, items: [])
        }
        do {
            let snapshot = try JSONDecoder().decode(Self.self, from: data)
            return Self(generation: snapshot.generation, items: Array(snapshot.items.prefix(7)))
        } catch {
            LXLog.error(
                "more-actions decode failed app=\(appId): \(error)",
                category: "MoreActions"
            )
            return Self(generation: 0, items: [])
        }
    }

    public func token(at index: Int) -> String {
        "more:\(generation):\(index)"
    }
}
