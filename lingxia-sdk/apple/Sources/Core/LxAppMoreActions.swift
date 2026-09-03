import Foundation

public struct LxAppMoreActionItem: Decodable {
    public let label: String
    public let iconPath: String
}

public struct LxAppMoreActionSnapshot: Decodable {
    public let generation: UInt64
    public let items: [LxAppMoreActionItem]

    public static func load(appId: String) -> Self {
        let json = getLxAppMoreActions(appId).toString()
        guard let data = json.data(using: .utf8),
              let snapshot = try? JSONDecoder().decode(Self.self, from: data) else {
            return Self(generation: 0, items: [])
        }
        return Self(generation: snapshot.generation, items: Array(snapshot.items.prefix(7)))
    }

    public func token(at index: Int) -> String {
        "more:\(generation):\(index)"
    }
}
