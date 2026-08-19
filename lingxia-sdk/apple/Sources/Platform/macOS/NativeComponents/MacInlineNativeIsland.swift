import AppKit

/// One ordered NSView host for an inline native Root on macOS.
/// Sibling order comes from the committed tree, never from message arrival.
@MainActor
final class MacInlineNativeIsland {
    static let allowedKinds: Set<String> = ["root", "view", "text", "tappable", "slider", "video"]

    private let container = NSView()
    private var nodes: [String: IslandNode] = [:]
    private(set) var lastAppliedRevision: UInt64 = 0

    init(host: NSView) {
        container.wantsLayer = true
        container.layer?.masksToBounds = true
        if container.superview == nil {
            host.addSubview(container)
            container.frame = host.bounds
            container.autoresizingMask = [.width, .height]
        }
    }

    func handle(message: [String: Any]) -> Bool {
        guard let action = message["action"] as? String else { return false }
        switch action {
        case "root.commit":
            applyCommit(message)
            return true
        case "geometry.snapshot", "video.command", "root.leaseAccept":
            return true
        default:
            return false
        }
    }

    private func applyCommit(_ message: [String: Any]) {
        guard let operations = message["operations"] as? [[String: Any]] else { return }
        for operation in operations {
            switch operation["op"] as? String {
            case "mount":
                if let node = operation["node"] as? [String: Any] {
                    mount(node)
                }
            case "unmount":
                if let ref = operation["node"] as? [String: Any],
                   let key = ref["nodeKey"] as? String
                {
                    nodes.removeValue(forKey: key)?.view.removeFromSuperview()
                }
            case "reorder":
                if let ref = operation["node"] as? [String: Any],
                   let key = ref["nodeKey"] as? String,
                   let order = operation["order"] as? Int
                {
                    nodes[key]?.order = order
                }
            default:
                break
            }
        }
        restack()
        if let revision = message["revision"] as? UInt64 {
            lastAppliedRevision = revision
        }
    }

    private func mount(_ node: [String: Any]) {
        guard let ref = node["ref"] as? [String: Any],
              let key = ref["nodeKey"] as? String,
              let kind = node["kind"] as? String,
              Self.allowedKinds.contains(kind)
        else { return }
        let view = NSView()
        view.wantsLayer = true
        let order = node["order"] as? Int ?? 0
        nodes[key] = IslandNode(key: key, kind: kind, order: order, view: view)
        container.addSubview(view)
    }

    private func restack() {
        let ordered = nodes.values.sorted { lhs, rhs in
            if lhs.order != rhs.order { return lhs.order < rhs.order }
            return lhs.key < rhs.key
        }
        for (index, node) in ordered.enumerated() {
            container.addSubview(node.view, positioned: .above, relativeTo: nil)
            node.view.layer?.zPosition = CGFloat(index)
        }
    }

    private struct IslandNode {
        let key: String
        let kind: String
        var order: Int
        let view: NSView
    }
}
