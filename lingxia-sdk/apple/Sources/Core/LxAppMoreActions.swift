import Foundation

struct LxAppMoreActionItem: Decodable {
    let label: String
    let iconPath: String
}

struct LxAppMoreActionSnapshot: Decodable {
    let generation: UInt64
    let items: [LxAppMoreActionItem]

    static func load(appId: String) -> Self {
        let json = getLxAppMoreActions(appId).toString()
        guard let data = json.data(using: .utf8),
              let snapshot = try? JSONDecoder().decode(Self.self, from: data) else {
            return Self(generation: 0, items: [])
        }
        return Self(generation: snapshot.generation, items: Array(snapshot.items.prefix(2)))
    }

    func token(at index: Int) -> String {
        "more:\(generation):\(index)"
    }
}
