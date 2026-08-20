import AppKit

/// One ordered NSView host for an inline native Root on macOS.
/// Sibling order comes from the committed tree, never from message arrival.
@MainActor
final class MacInlineNativeIsland {
    static let allowedKinds: Set<String> = ["root", "view", "text", "tappable", "slider", "video"]

    private let container = NSView()
    private var nodes: [String: IslandNode] = [:]
    private(set) var lastAppliedRevision: UInt64 = 0
    private let eventSink: (_ componentId: String, _ event: String, _ detail: [String: Any]) -> Void
    private var leaseGranted = false
    private var leaseActive = false
    private var leaseId = ""
    private var leaseSequence: UInt64 = 1
    private var lastRoot: [String: Any]?
    private var pendingOutgoing: [[String: Any]] = []

    init(
        host: NSView,
        eventSink: @escaping (_ componentId: String, _ event: String, _ detail: [String: Any]) -> Void
    ) {
        self.eventSink = eventSink
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
        case "geometry.snapshot":
            applyGeometry(message)
            return true
        case "root.leaseAccept":
            acceptLease(message)
            return true
        case "video.command":
            applyVideoCommand(message)
            return true
        default:
            return false
        }
    }

    func drainOutgoing() -> [[String: Any]] {
        let outgoing = pendingOutgoing
        pendingOutgoing.removeAll()
        return outgoing
    }

    private func applyCommit(_ message: [String: Any]) {
        guard let operations = message["operations"] as? [[String: Any]] else { return }
        for operation in operations {
            switch operation["op"] as? String {
            case "mount":
                if let node = operation["node"] as? [String: Any] {
                    mount(node)
                }
            case "update":
                update(operation)
            case "unmount":
                if let ref = operation["node"] as? [String: Any],
                   let key = nodeKey(ref)
                {
                    removeNode(key)
                }
            case "reorder":
                if let ref = operation["node"] as? [String: Any],
                   let key = nodeKey(ref),
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
        } else if let revision = message["revision"] as? Int {
            lastAppliedRevision = UInt64(revision)
        }
        if let root = message["root"] as? [String: Any] {
            lastRoot = root
            grantLeaseIfNeeded(root)
        }
    }

    private func applyGeometry(_ message: [String: Any]) {
        guard let entries = message["nodes"] as? [[String: Any]] else { return }
        for entry in entries {
            guard let ref = entry["ref"] as? [String: Any],
                  let key = ref["nodeKey"] as? String,
                  let node = nodes[key],
                  let rect = entry["contentRect"] as? [String: Any]
            else { continue }
            node.rect = NSRect(
                x: cg(rect["x"]),
                y: cg(rect["y"]),
                width: max(cg(rect["width"]), 1),
                height: max(cg(rect["height"]), 1)
            )
            node.visible = entry["visible"] as? Bool ?? true
            applyFrame(node)
        }
    }

    private func acceptLease(_ message: [String: Any]) {
        guard leaseGranted, !leaseActive else { return }
        let incomingId = message["leaseId"] as? String ?? ""
        let incomingSeq = (message["sequence"] as? UInt64) ?? UInt64((message["sequence"] as? Int) ?? 0)
        if !incomingId.isEmpty, incomingId != leaseId { return }
        if incomingSeq > 0, incomingSeq != leaseSequence { return }
        leaseActive = true
        var payload: [String: Any] = [
            "action": "root.leaseActive",
            "id": lastRoot?["rootKey"] as? String ?? "island",
            "leaseId": leaseId,
            "sequence": leaseSequence,
        ]
        if let lastRoot { payload["root"] = lastRoot }
        pendingOutgoing.append(payload)
    }

    private func grantLeaseIfNeeded(_ root: [String: Any]) {
        guard !leaseGranted else { return }
        let rootKey = root["rootKey"] as? String ?? "island"
        leaseGranted = true
        leaseId = "lease-\(rootKey)"
        leaseSequence = 1
        pendingOutgoing.append([
            "action": "root.leaseGranted",
            "id": rootKey,
            "root": root,
            "leaseId": leaseId,
            "sequence": leaseSequence,
            "leaseDurationMs": 8000,
        ])
    }

    private func applyVideoCommand(_ message: [String: Any]) {
        let owner = message["owner"] as? [String: Any]
        let key = owner?["nodeKey"] as? String
        let node = key.flatMap { nodes[$0] } ?? nodes.values.first(where: { $0.kind == "video" })
        guard let video = node?.video else { return }
        let command = message["command"] as? [String: Any]
        let name = command?["name"] as? String ?? ""
        var params: [String: Any] = [:]
        if name == "seek", let seconds = command?["seconds"] {
            params["time"] = seconds
        }
        video.handleCommand(name: name, params: params)
    }

    private func mount(_ node: [String: Any]) {
        guard let ref = node["ref"] as? [String: Any],
              let key = ref["nodeKey"] as? String,
              let kind = node["kind"] as? String,
              Self.allowedKinds.contains(kind)
        else { return }
        let authorId = (node["authorId"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? key
        let props = node["props"] as? [String: Any] ?? [:]
        let order = node["order"] as? Int ?? 0
        let item = IslandNode(key: key, kind: kind, authorId: authorId, order: order, props: props)
        factoryView(item)
        nodes[key] = item
        if item.view.superview == nil {
            container.addSubview(item.view)
        }
        applyProps(item)
        applyFrame(item)
    }

    private func update(_ operation: [String: Any]) {
        guard let ref = operation["node"] as? [String: Any],
              let key = nodeKey(ref),
              let item = nodes[key]
        else { return }
        let patch = operation["patch"] as? [String: Any] ?? [:]
        for (name, value) in patch {
            item.props[name] = value
        }
        applyProps(item)
    }

    private func factoryView(_ item: IslandNode) {
        switch item.kind {
        case "video":
            let video = MacVideoComponent(id: item.authorId, initialProps: item.props) { [weak self] event in
                guard let name = event["event"] as? String else { return }
                let detail = event["detail"] as? [String: Any] ?? [:]
                self?.eventSink(item.authorId, name, detail)
            }
            video.mount(in: container)
            item.video = video
            item.view = video.view
        case "text":
            let label = NSTextField(labelWithString: "")
            label.textColor = .white
            label.font = .systemFont(ofSize: 12)
            label.isBezeled = false
            label.drawsBackground = false
            item.label = label
            item.view = label
        case "tappable":
            let button = NSButton(title: "", target: item, action: #selector(IslandNode.press))
            button.bezelStyle = .rounded
            item.onPress = { [weak self, weak item] in
                guard let item, !(item.props["disabled"] as? Bool ?? false) else { return }
                self?.eventSink(item.authorId, "press", ["source": "pointer"])
            }
            item.button = button
            item.view = button
        case "slider":
            let slider = NSSlider()
            slider.minValue = 0
            slider.maxValue = 100
            slider.target = item
            slider.action = #selector(IslandNode.sliderChanged)
            item.onSliderChange = { [weak self, weak item] value, commit in
                guard let item else { return }
                let event = commit ? "valuecommit" : "valuechange"
                self?.eventSink(item.authorId, event, ["value": value])
            }
            item.slider = slider
            item.view = slider
        default:
            let view = IslandPassthroughView()
            view.wantsLayer = true
            item.view = view
        }
        item.view.wantsLayer = true
        applyPointerEvents(item)
    }

    private func applyProps(_ item: IslandNode) {
        switch item.kind {
        case "video":
            item.video?.update(props: item.props)
        case "text":
            item.label?.stringValue = item.props["text"] as? String ?? ""
        case "tappable":
            let content = item.props["content"] as? [String: Any]
            let icon = content?["icon"] as? [String: Any]
            item.button?.title = (content?["text"] as? String)
                ?? (icon?["name"] as? String)
                ?? (item.props["label"] as? String)
                ?? (item.props["icon"] as? String)
                ?? ""
            item.button?.isEnabled = !boolValue(item.props["disabled"])
        case "slider":
            guard let slider = item.slider, !item.dragging else { return }
            let min = Double(cg(item.props["min"]))
            let maximum = Swift.max(Double(cg(item.props["max"])), min + 1)
            slider.minValue = min
            slider.maxValue = maximum
            slider.doubleValue = Double(cg(item.props["value"]))
            slider.isEnabled = !boolValue(item.props["disabled"])
        case "view":
            applyScrim(item)
            applyPointerEvents(item)
        default:
            applyPointerEvents(item)
        }
    }

    private func applyPointerEvents(_ item: IslandNode) {
        guard let view = item.view as? IslandPassthroughView else { return }
        if let mode = item.props["pointerEvents"] as? String {
            view.pointerEvents = mode
            return
        }
        if item.props["scrimPaint"] != nil {
            view.pointerEvents = "box-none"
            return
        }
        view.pointerEvents = item.kind == "text" ? "none" : "auto"
    }

    private func applyScrim(_ item: IslandNode) {
        item.view.wantsLayer = true
        guard let paint = item.props["scrimPaint"] as? [String: Any] else {
            item.view.layer?.backgroundColor = NSColor.clear.cgColor
            return
        }
        let scrim = paint["scrim"] as? String ?? "none"
        let opacity = cg(paint["opacity"])
        if scrim == "none" {
            item.view.layer?.backgroundColor = NSColor.clear.cgColor
            return
        }
        item.view.layer?.backgroundColor = NSColor.black.withAlphaComponent(opacity).cgColor
    }

    private func applyFrame(_ item: IslandNode) {
        item.view.frame = item.rect
        item.view.isHidden = !item.visible || item.rect.width <= 0 || item.rect.height <= 0
        item.video?.setFrame(item.rect)
    }

    private func restack() {
        let ordered = nodes.values.sorted { lhs, rhs in
            if lhs.order != rhs.order { return lhs.order < rhs.order }
            return lhs.key < rhs.key
        }
        for node in ordered {
            container.addSubview(node.view, positioned: .above, relativeTo: nil)
        }
    }

    private func removeNode(_ key: String) {
        guard let node = nodes.removeValue(forKey: key) else { return }
        node.video?.unmount()
        node.view.removeFromSuperview()
    }

    private func nodeKey(_ node: [String: Any]) -> String? {
        if let ref = node["ref"] as? [String: Any] {
            return ref["nodeKey"] as? String
        }
        return node["nodeKey"] as? String
    }

    private func cg(_ value: Any?) -> CGFloat {
        if let number = value as? CGFloat { return number }
        if let number = value as? Double { return CGFloat(number) }
        if let number = value as? Int { return CGFloat(number) }
        if let number = value as? NSNumber { return CGFloat(truncating: number) }
        if let text = value as? String, let number = Double(text) { return CGFloat(number) }
        return 0
    }

    private func boolValue(_ value: Any?) -> Bool {
        if let flag = value as? Bool { return flag }
        if let text = value as? String { return text == "true" || text == "1" }
        if let number = value as? NSNumber { return number.boolValue }
        return false
    }

    private final class IslandNode: NSObject {
        let key: String
        let kind: String
        let authorId: String
        var order: Int
        var props: [String: Any]
        var view = NSView()
        var video: MacVideoComponent?
        var label: NSTextField?
        var button: NSButton?
        var slider: NSSlider?
        var rect: NSRect = .zero
        var visible = true
        var dragging = false
        var onPress: (() -> Void)?
        var onSliderChange: ((Double, Bool) -> Void)?

        init(key: String, kind: String, authorId: String, order: Int, props: [String: Any]) {
            self.key = key
            self.kind = kind
            self.authorId = authorId
            self.order = order
            self.props = props
        }

        @objc func press() {
            onPress?()
        }

        @objc func sliderChanged(_ sender: NSSlider) {
            let commit = NSEvent.pressedMouseButtons == 0
            dragging = !commit
            onSliderChange?(sender.doubleValue, commit)
        }
    }
}

/// Cover/view `box-none`: the view itself does not hit, siblings below can.
private final class IslandPassthroughView: NSView {
    var pointerEvents: String = "auto"

    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        switch pointerEvents {
        case "none":
            return nil
        case "box-none":
            if hit == nil || hit === self {
                return nil
            }
            return hit
        default:
            return hit
        }
    }
}
