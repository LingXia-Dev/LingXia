#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// One ordered NSView host for an inline native Root on macOS.
/// Sibling order comes from the committed tree, never from message arrival.
@MainActor
final class MacInlineNativeIsland {
    static let allowedKinds: Set<String> = ["root", "view", "text", "tappable", "video"]

    static func isIslandAction(_ action: String) -> Bool {
        action == "root.commit" || action == "root.destroy" || action == "geometry.snapshot"
            || action == "root.leaseAccept" || action == "video.command"
    }

    private let container = IslandContainerView()
    private weak var manager: MacNativeComponentManager?
    private let appId: String
    private var nodes: [String: IslandNode] = [:]
    private(set) var lastAppliedRevision: UInt64 = 0
    private var revisions: [String: UInt64] = [:]
    private var roots: [String: [String: Any]] = [:]
    private var rootOrders: [String: Int] = [:]
    private let eventSink: (_ componentId: String, _ event: String, _ detail: [String: Any]) -> Void
    /// The lease is per root, not per host: a page may mount several
    /// `LxNativeRoot`s into one island and each negotiates its own.
    private struct RootLease {
        var granted = false
        var active = false
        var id = ""
        var sequence: UInt64 = 1
        var root: [String: Any]
    }
    private var leases: [String: RootLease] = [:]
    private var pageActive = true
    private var pendingOutgoing: [[String: Any]] = []
    private var scrollOffset = CGPoint.zero

    init(
        host: NSView,
        manager: MacNativeComponentManager,
        appId: String,
        eventSink: @escaping (_ componentId: String, _ event: String, _ detail: [String: Any]) -> Void
    ) {
        self.manager = manager
        self.appId = appId
        self.eventSink = eventSink
        container.wantsLayer = true
        container.layer?.masksToBounds = true
        if container.superview == nil {
            attach(to: host)
        }
    }

    /// Pin by constraint: the host's bounds can still be empty on the first commit.
    private func attach(to host: NSView) {
        host.addSubview(container)
        container.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            container.topAnchor.constraint(equalTo: host.topAnchor),
            container.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            container.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            container.bottomAnchor.constraint(equalTo: host.bottomAnchor),
        ])
    }

    func handle(message: [String: Any]) -> Bool {
        guard let action = message["action"] as? String else { return false }
        switch action {
        case "root.commit":
            applyCommit(message)
            return true
        case "root.destroy":
            destroyRoot(message)
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

    func rebind(to host: NSView) {
        guard container.superview !== host else { return }
        container.removeFromSuperview()
        attach(to: host)
    }

    func updateScrollOffset(x: CGFloat, y: CGFloat) {
        scrollOffset = CGPoint(x: x, y: y)
        nodes.values.forEach(applyFrame)
    }

    /// Hide the island while its page is not the interactive one: a native video
    /// surface outlives the WebView's own visibility.
    func setPageActive(_ active: Bool) {
        pageActive = active
        container.isHidden = !active
        nodes.values.forEach(applyFrame)
    }

    /// Remove the island tree when its WebView is being destroyed.
    /// The next root commit must start with a fresh lease and native views.
    func teardown() {
        for key in Array(nodes.keys) {
            removeNode(key)
        }
        container.removeFromSuperview()
        pendingOutgoing.removeAll()
        leases.removeAll()
        lastAppliedRevision = 0
        revisions.removeAll()
        roots.removeAll()
        rootOrders.removeAll()
    }

    private func applyCommit(_ message: [String: Any]) {
        guard let root = message["root"] as? [String: Any],
              let rootKey = root["rootKey"] as? String
        else { return }
        let baseInt = (message["baseRevision"] as? Int) ?? 0
        let revisionInt = (message["revision"] as? Int) ?? 0
        let base = (message["baseRevision"] as? UInt64) ?? (baseInt >= 0 ? UInt64(baseInt) : 0)
        let revision = (message["revision"] as? UInt64) ?? (revisionInt >= 0 ? UInt64(revisionInt) : 0)
        guard revision > base, revision > 0 else {
            pendingOutgoing.append([
                "action": "root.error", "id": rootKey, "root": root,
                "message": "commit revision must be greater than baseRevision", "recoverable": true,
            ])
            return
        }
        let policyError = validateMediaURLs(in: message)
        guard policyError.isEmpty else {
            pendingOutgoing.append([
                "action": "root.error", "id": rootKey, "root": root,
                "message": policyError, "recoverable": true,
            ])
            return
        }
        let last = revisions[rootKey] ?? 0
        if let existingRoot = roots[rootKey], !sameRootGeneration(existingRoot, root) {
            pendingOutgoing.append([
                "action": "root.error", "id": rootKey, "root": root,
                "message": "root generation changed without destroy", "recoverable": true,
            ])
            return
        }
        if base == 0 {
            nodes.values.filter { $0.rootKey == rootKey }.map(\.key).forEach(removeNode)
            revisions.removeValue(forKey: rootKey)
            leases.removeValue(forKey: rootKey)
        } else if base != last {
            pendingOutgoing.append([
                "action": "root.resyncRequired", "id": rootKey, "root": root,
                "lastAppliedRevision": last,
            ])
            return
        }
        guard let operations = message["operations"] as? [[String: Any]] else { return }
        for operation in operations {
            switch operation["op"] as? String {
            case "mount":
                if let node = operation["node"] as? [String: Any] {
                    mount(node, expectedRootKey: rootKey)
                }
            case "update":
                update(operation)
            case "unmount":
                if let ref = operation["node"] as? [String: Any],
                   let key = nodeKey(ref)
                {
                    guard (ref["rootKey"] as? String) == rootKey else { continue }
                    removeNode(key)
                }
            case "reorder":
                if let ref = operation["node"] as? [String: Any],
                   let key = nodeKey(ref),
                   let order = operation["order"] as? Int
                {
                    guard (ref["rootKey"] as? String) == rootKey else { continue }
                    nodes[key]?.order = order
                }
            case "reparent":
                if let ref = operation["node"] as? [String: Any],
                   let key = nodeKey(ref)
                {
                    guard (ref["rootKey"] as? String) == rootKey else { continue }
                    let parent = operation["parent"] as? [String: Any]
                    guard parent == nil || (parent?["rootKey"] as? String) == rootKey else { continue }
                    nodes[key]?.parentKey = parent?["nodeKey"] as? String
                }
            default:
                break
            }
        }
        restack()
        revisions[rootKey] = revision
        roots[rootKey] = root
        lastAppliedRevision = revisions.values.max() ?? 0
        pendingOutgoing.append([
            "action": "root.applied", "id": rootKey, "root": root, "revision": revision,
        ])
        grantLeaseIfNeeded(root)
    }

    private func destroyRoot(_ message: [String: Any]) {
        guard let root = message["root"] as? [String: Any],
              let rootKey = root["rootKey"] as? String
        else { return }
        guard let current = roots[rootKey], sameRootGeneration(current, root) else { return }
        nodes.values.filter { $0.rootKey == rootKey }.map(\.key).forEach(removeNode)
        revisions.removeValue(forKey: rootKey)
        roots.removeValue(forKey: rootKey)
        rootOrders.removeValue(forKey: rootKey)
        leases.removeValue(forKey: rootKey)
        lastAppliedRevision = revisions.values.max() ?? 0
        restack()
    }

    private func applyGeometry(_ message: [String: Any]) {
        for entry in message["roots"] as? [[String: Any]] ?? [] {
            guard let ref = entry["ref"] as? [String: Any],
                  let rootKey = ref["rootKey"] as? String,
                  let current = roots[rootKey], sameRootGeneration(current, ref)
            else { continue }
            rootOrders[rootKey] = entry["rootOrder"] as? Int ?? Int.max
        }
        guard let entries = message["nodes"] as? [[String: Any]] else { return }
        for entry in entries {
            guard let ref = entry["ref"] as? [String: Any],
                  let key = ref["nodeKey"] as? String,
                  let node = nodes[key],
                  (ref["rootKey"] as? String) == node.rootKey,
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
        restack()
    }

    private func acceptLease(_ message: [String: Any]) {
        let incomingId = message["leaseId"] as? String ?? ""
        let incomingSeq = (message["sequence"] as? UInt64) ?? UInt64((message["sequence"] as? Int) ?? 0)
        let rootKey = (message["id"] as? String)
            ?? (message["root"] as? [String: Any])?["rootKey"] as? String
            ?? leases.first(where: { $0.value.id == incomingId })?.key
        guard let rootKey, var lease = leases[rootKey], lease.granted, !lease.active else { return }
        if !incomingId.isEmpty, incomingId != lease.id { return }
        if incomingSeq > 0, incomingSeq != lease.sequence { return }
        lease.active = true
        leases[rootKey] = lease
        pendingOutgoing.append([
            "action": "root.leaseActive",
            "id": rootKey,
            "root": lease.root,
            "leaseId": lease.id,
            "sequence": lease.sequence,
        ])
    }

    private func grantLeaseIfNeeded(_ root: [String: Any]) {
        let rootKey = root["rootKey"] as? String ?? "island"
        guard leases[rootKey] == nil else { return }
        let lease = RootLease(granted: true, active: false, id: "lease-\(rootKey)", sequence: 1, root: root)
        leases[rootKey] = lease
        pendingOutgoing.append([
            "action": "root.leaseGranted",
            "id": rootKey,
            "root": root,
            "leaseId": lease.id,
            "sequence": lease.sequence,
            "leaseDurationMs": 8000,
        ])
    }

    private func applyVideoCommand(_ message: [String: Any]) {
        guard validateMediaURLs(in: message).isEmpty else { return }
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

    private func validateMediaURLs(in value: Any) -> String {
        var urls: [String] = []
        func collect(_ current: Any, key: String? = nil) {
            if let dictionary = current as? [String: Any] {
                dictionary.forEach { collect($0.value, key: $0.key) }
            } else if let list = current as? [Any] {
                list.forEach { collect($0, key: key) }
            } else if let text = current as? String,
                      ["src", "url", "uri", "poster"].contains(key ?? "") {
                urls.append(text)
            }
        }
        collect(value)
        guard let data = try? JSONSerialization.data(withJSONObject: urls),
              let json = String(data: data, encoding: .utf8)
        else { return "invalid media URL list" }
        return validateInlineNativeMediaUrls(appId, json).toString()
    }

    private func sameRootGeneration(_ lhs: [String: Any], _ rhs: [String: Any]) -> Bool {
        func epoch(_ root: [String: Any]) -> UInt64? {
            if let value = root["rootEpoch"] as? UInt64 { return value }
            if let value = root["rootEpoch"] as? Int { return UInt64(value) }
            return nil
        }
        return lhs["surfaceInstanceId"] as? String == rhs["surfaceInstanceId"] as? String
            && lhs["pageInstanceId"] as? String == rhs["pageInstanceId"] as? String
            && lhs["documentInstanceId"] as? String == rhs["documentInstanceId"] as? String
            && lhs["rootKey"] as? String == rhs["rootKey"] as? String
            && epoch(lhs) == epoch(rhs)
    }

    private func mount(_ node: [String: Any], expectedRootKey: String) {
        guard let ref = node["ref"] as? [String: Any],
              let key = ref["nodeKey"] as? String,
              let rootKey = ref["rootKey"] as? String,
              let kind = node["kind"] as? String,
              Self.allowedKinds.contains(kind)
        else { return }
        guard rootKey == expectedRootKey else { return }
        let authorId = (node["authorId"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? key
        let automationId = (node["automationId"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        let props = node["props"] as? [String: Any] ?? [:]
        let order = node["order"] as? Int ?? 0
        let parentKey = (node["parent"] as? [String: Any])?["nodeKey"] as? String
        guard (node["parent"] as? [String: Any])?["rootKey"] as? String == nil
            || (node["parent"] as? [String: Any])?["rootKey"] as? String == rootKey
        else { return }
        if let existing = nodes[key], existing.rootKey != rootKey { return }
        removeNode(key)
        let item = IslandNode(
            key: key,
            rootKey: rootKey,
            kind: kind,
            authorId: authorId,
            automationId: automationId,
            parentKey: parentKey,
            order: order,
            props: props
        )
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
        guard (ref["rootKey"] as? String) == item.rootKey else { return }
        let patch = operation["patch"] as? [String: Any] ?? [:]
        for (name, value) in patch {
            if value is NSNull {
                item.props.removeValue(forKey: name)
            } else {
                item.props[name] = value
            }
        }
        applyProps(item)
    }

    private func factoryView(_ item: IslandNode) {
        switch item.kind {
        case "video":
            // The player retains this sink for its lifetime, so hold the node weakly.
            let authorId = item.authorId
            let video = MacVideoComponent(id: authorId, initialProps: item.props) { [weak self] event in
                guard let name = event["event"] as? String else { return }
                let detail = event["detail"] as? [String: Any] ?? [:]
                self?.eventSink(authorId, name, detail)
            }
            video.mount(in: container)
            item.video = video
            item.view = video.view
            manager?.attachIslandVideo(id: item.authorId, component: video)
        case "text":
            let label = IslandTextField()
            label.stringValue = ""
            label.isEditable = false
            label.isSelectable = false
            label.isBezeled = false
            label.drawsBackground = false
            item.label = label
            item.view = label
        case "tappable":
            let button = IslandButton()
            button.title = ""
            button.target = item
            button.action = #selector(IslandNode.press)
            button.bezelStyle = .rounded
            item.onPress = { [weak self, weak item] in
                guard let item, !(self?.boolValue(item.props["disabled"]) ?? false) else { return }
                let source = NSApp.currentEvent?.type == .keyDown ? "keyboard" : "pointer"
                self?.eventSink(item.authorId, "press", ["source": source])
            }
            item.button = button
            item.view = button
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
            applyText(item)
        case "tappable":
            applyButton(item)
        case "view":
            applyScrim(item)
        default:
            break
        }
        applyNativeStyle(item)
        applyAccessibility(item)
        applyPointerEvents(item)
    }

    private func applyPointerEvents(_ item: IslandNode) {
        let mode = item.props["pointerEvents"] as? String
            ?? ((item.kind == "text" || item.kind == "view") ? "box-none" : "auto")
        (item.view as? IslandPassthroughView)?.pointerEvents = mode
        (item.view as? IslandButton)?.pointerEvents = mode
        container.setPointerEvents(mode, for: item.view)
    }

    private func applyText(_ item: IslandNode) {
        guard let label = item.label else { return }
        let text = item.props["text"] as? String ?? ""
        label.textColor = color(item.props["color"]) ?? .white
        label.font = .systemFont(
            ofSize: cg(item.props["fontSize"], fallback: 12),
            weight: fontWeight(item.props["fontWeight"])
        )
        let maxLines = Int(cg(item.props["maxLines"]))
        label.maximumNumberOfLines = maxLines > 0 ? maxLines : 0
        label.lineBreakMode = maxLines > 0 ? .byTruncatingTail : .byWordWrapping
        switch item.props["textAlign"] as? String {
        case "center": label.alignment = .center
        case "end": label.alignment = .right
        default: label.alignment = .left
        }
        let lineHeight = cg(item.props["lineHeight"])
        if lineHeight > 0, let font = label.font, let textColor = label.textColor {
            let paragraph = NSMutableParagraphStyle()
            paragraph.minimumLineHeight = lineHeight
            paragraph.maximumLineHeight = lineHeight
            paragraph.alignment = label.alignment
            paragraph.baseWritingDirection = item.props["dir"] as? String == "rtl" ? .rightToLeft : .leftToRight
            label.attributedStringValue = NSAttributedString(
                string: text,
                attributes: [.font: font, .foregroundColor: textColor, .paragraphStyle: paragraph]
            )
        } else {
            label.stringValue = text
        }
    }

    private func applyButton(_ item: IslandNode) {
        guard let button = item.button else { return }
        let content = item.props["content"] as? [String: Any]
        let icon = content?["icon"] as? [String: Any]
        let text = (content?["text"] as? String) ?? (item.props["label"] as? String) ?? ""
        let iconName = (icon?["name"] as? String) ?? (item.props["icon"] as? String)
        let loading = boolValue(item.props["loading"])
        let glyph = semanticIcon(iconName) ?? iconName
        if loading {
            button.title = "…"
        } else if let glyph, !text.isEmpty {
            button.title = item.props["iconPosition"] as? String == "end" ? "\(text)  \(glyph)" : "\(glyph)  \(text)"
        } else {
            button.title = glyph ?? text
        }
        let disabled = boolValue(item.props["disabled"])
        button.isEnabled = !disabled && !loading
        button.isBordered = false
        let colors = buttonColors(
            intent: item.props["intent"] as? String ?? "neutral",
            emphasis: item.props["emphasis"] as? String ?? "secondary",
            pressed: boolValue(item.props["pressed"]),
            disabled: disabled || loading
        )
        let foreground = color(item.props["color"]) ?? colors.foreground
        let font = NSFont.systemFont(
            ofSize: cg(style(item.props)["fontSize"], fallback: item.props["size"] as? String == "compact" ? 12 : 14),
            weight: .semibold
        )
        button.contentTintColor = foreground
        button.font = font
        button.attributedTitle = NSAttributedString(
            string: button.title,
            attributes: [.foregroundColor: foreground, .font: font]
        )
        button.wantsLayer = true
        button.layer?.backgroundColor = colors.background.cgColor
        button.layer?.cornerRadius = cg(style(item.props)["borderRadius"], fallback: 10)
        (button as? IslandButton)?.hitSlop = cg(item.props["hitSlop"])
    }

    private func applyNativeStyle(_ item: IslandNode) {
        let nativeStyle = style(item.props)
        item.view.alphaValue = min(max(cg(nativeStyle["opacity"], fallback: 1), 0), 1)
        item.view.wantsLayer = true
        if let background = color(nativeStyle["backgroundColor"]), item.kind != "video" {
            item.view.layer?.backgroundColor = background.cgColor
        } else if item.kind == "view", item.props["scrimPaint"] == nil {
            item.view.layer?.backgroundColor = NSColor.clear.cgColor
        }
        if nativeStyle["borderRadius"] != nil || item.kind == "view" {
            item.view.layer?.cornerRadius = cg(nativeStyle["borderRadius"])
        }
        item.view.layer?.borderWidth = cg(nativeStyle["borderWidth"])
        item.view.layer?.borderColor = color(nativeStyle["borderColor"])?.cgColor
        item.view.layer?.masksToBounds = (item.view.layer?.cornerRadius ?? 0) > 0
    }

    private func applyAccessibility(_ item: IslandNode) {
        let hidden = boolValue(item.props["aria-hidden"]) || boolValue(item.props["ariaHidden"])
        item.view.setAccessibilityElement(!hidden && ["text", "tappable", "video"].contains(item.kind))
        if let automationId = item.automationId {
            item.view.identifier = NSUserInterfaceItemIdentifier(automationId)
        }
        let label = (item.props["aria-label"] as? String)
            ?? (item.props["ariaLabel"] as? String)
            ?? (item.kind == "tappable" ? item.button?.title : nil)
        item.view.setAccessibilityLabel(label)
        item.view.setAccessibilityHelp(
            (item.props["aria-description"] as? String) ?? (item.props["ariaDescription"] as? String)
        )
        switch item.kind {
        case "tappable": item.view.setAccessibilityRole(.button)
        case "text": item.view.setAccessibilityRole(.staticText)
        default: break
        }
        item.view.setAccessibilityEnabled(item.view.isHidden == false && item.button?.isEnabled != false)
    }

    private func applyScrim(_ item: IslandNode) {
        item.view.wantsLayer = true
        item.scrim?.removeFromSuperlayer()
        item.scrim = nil
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
        let gradient = CAGradientLayer()
        gradient.name = "lx-scrim"
        gradient.frame = item.view.bounds
        let shade = NSColor.black.withAlphaComponent(min(max(opacity, 0), 1)).cgColor
        switch scrim {
        case "top": gradient.colors = [shade, NSColor.clear.cgColor]
        case "bottom": gradient.colors = [NSColor.clear.cgColor, shade]
        default: gradient.colors = [shade, shade]
        }
        item.view.layer?.insertSublayer(gradient, at: 0)
        item.scrim = gradient
    }

    private func applyFrame(_ item: IslandNode) {
        // A fullscreen video lives in its own window until it exits; leave its layout alone.
        guard item.view.superview === container else { return }
        let rect = textFittedRect(item)
        let viewportRect = NSRect(
            x: rect.origin.x - scrollOffset.x,
            y: rect.origin.y - scrollOffset.y,
            width: rect.width,
            height: rect.height
        )
        if let video = item.video {
            // The player owns its own view's layout.
            video.setFrame(viewportRect)
        } else {
            item.view.frame = viewportRect
        }
        item.view.isHidden = !pageActive || !item.visible || item.rect.width <= 0 || item.rect.height <= 0
        item.scrim?.frame = item.view.bounds
    }

    /// Web and native shapers space the same nominal font differently, so a box the
    /// page measured can fall a few points short. Widen rather than drop characters.
    private func textFittedRect(_ item: IslandNode) -> NSRect {
        guard item.kind == "text", let label = item.label, item.rect.width > 0 else { return item.rect }
        let fitting = label.fittingSize.width
        guard fitting > item.rect.width else { return item.rect }
        var rect = item.rect
        switch label.alignment {
        case .center: rect.origin.x -= (fitting - rect.width) / 2
        case .right: rect.origin.x -= fitting - rect.width
        default: break
        }
        rect.size.width = ceil(fitting)
        return rect
    }

    private func restack() {
        var ordered: [IslandNode] = []
        func append(parentKey: String?) {
            let children = nodes.values.filter { $0.parentKey == parentKey }.sorted { lhs, rhs in
                if parentKey == nil {
                    let lhsRoot = rootOrders[lhs.rootKey] ?? Int.max
                    let rhsRoot = rootOrders[rhs.rootKey] ?? Int.max
                    if lhsRoot != rhsRoot { return lhsRoot < rhsRoot }
                }
                if lhs.order != rhs.order { return lhs.order < rhs.order }
                return lhs.key < rhs.key
            }
            for child in children {
                ordered.append(child)
                append(parentKey: child.key)
            }
        }
        append(parentKey: nil)
        for node in ordered where node.view.superview === container {
            container.addSubview(node.view, positioned: .above, relativeTo: nil)
        }
    }

    private func removeNode(_ key: String) {
        guard let node = nodes.removeValue(forKey: key) else { return }
        container.removePointerEvents(for: node.view)
        if node.video != nil {
            manager?.detachIslandVideo(id: node.authorId)
        }
        node.video?.unmount()
        node.view.removeFromSuperview()
    }

    private func nodeKey(_ node: [String: Any]) -> String? {
        if let ref = node["ref"] as? [String: Any] {
            return ref["nodeKey"] as? String
        }
        return node["nodeKey"] as? String
    }

    private func cg(_ value: Any?, fallback: CGFloat = 0) -> CGFloat {
        if let number = value as? CGFloat { return number }
        if let number = value as? Double { return CGFloat(number) }
        if let number = value as? Int { return CGFloat(number) }
        if let number = value as? NSNumber { return CGFloat(truncating: number) }
        if let text = value as? String {
            let scanner = Scanner(string: text.trimmingCharacters(in: .whitespacesAndNewlines))
            if let number = scanner.scanDouble() { return CGFloat(number) }
        }
        return fallback
    }

    private func style(_ props: [String: Any]) -> [String: Any] {
        props["nativeStyle"] as? [String: Any] ?? [:]
    }

    private func color(_ value: Any?) -> NSColor? {
        guard let raw = value as? String else { return nil }
        return NativeComponentColorStyle.parseColor(raw)
    }

    /// Map the CSS weight onto the full system scale: collapsing everything above
    /// 500 to `.bold` renders text wider than the CSS box it was measured into.
    private func fontWeight(_ value: Any?) -> NSFont.Weight {
        switch cssFontWeight(value) {
        case ..<200: return .ultraLight
        case ..<300: return .thin
        case ..<400: return .light
        case ..<500: return .regular
        case ..<600: return .medium
        case ..<700: return .semibold
        case ..<800: return .bold
        case ..<900: return .heavy
        default: return .black
        }
    }

    private func cssFontWeight(_ value: Any?) -> Int {
        if let number = value as? NSNumber { return number.intValue }
        switch (value as? String)?.lowercased() {
        case "lighter": return 300
        case "normal", .none, .some(""): return 400
        case "bold": return 700
        case "bolder": return 800
        case .some(let raw): return Int(raw) ?? 400
        }
    }

    private func semanticIcon(_ name: String?) -> String? {
        switch name {
        case "close": return "×"
        case "play": return "▶"
        case "pause": return "Ⅱ"
        case "mute": return "🔇"
        case "unmute": return "🔊"
        case "fullscreen": return "⛶"
        case "more": return "⋯"
        default: return nil
        }
    }

    private func buttonColors(
        intent: String,
        emphasis: String,
        pressed: Bool,
        disabled: Bool
    ) -> (background: NSColor, foreground: NSColor) {
        if disabled {
            return (NSColor(calibratedRed: 0.61, green: 0.64, blue: 0.69, alpha: 1), .white)
        }
        let base: NSColor
        switch intent {
        case "accent": base = NSColor(calibratedRed: 0.15, green: 0.39, blue: 0.92, alpha: 1)
        case "destructive": base = NSColor(calibratedRed: 0.86, green: 0.15, blue: 0.15, alpha: 1)
        default: base = NSColor(calibratedRed: 0.22, green: 0.25, blue: 0.32, alpha: 1)
        }
        if emphasis == "quiet" {
            return (.clear, base)
        }
        let alpha: CGFloat = emphasis == "secondary" ? (pressed ? 0.44 : 0.31) : (pressed ? 0.82 : 1)
        return (base.withAlphaComponent(alpha), .white)
    }

    private func boolValue(_ value: Any?) -> Bool {
        if let flag = value as? Bool { return flag }
        // `reflectBoolean` writes a bare attribute as "", which parseBooleanAttr reads as true.
        if let text = value as? String {
            let normalized = text.trimmingCharacters(in: .whitespaces).lowercased()
            return normalized == "" || normalized == "true" || normalized == "1"
        }
        if let number = value as? NSNumber { return number.boolValue }
        return false
    }

    @MainActor
    private final class IslandNode: NSObject {
        let key: String
        let rootKey: String
        let kind: String
        let authorId: String
        let automationId: String?
        var parentKey: String?
        var order: Int
        var props: [String: Any]
        var view = NSView()
        var video: MacVideoComponent?
        var label: NSTextField?
        var button: NSButton?
        var scrim: CAGradientLayer?
        var rect: NSRect = .zero
        var visible = true
        var onPress: (() -> Void)?

        init(
            key: String,
            rootKey: String,
            kind: String,
            authorId: String,
            automationId: String?,
            parentKey: String?,
            order: Int,
            props: [String: Any]
        ) {
            self.key = key
            self.rootKey = rootKey
            self.kind = kind
            self.authorId = authorId
            self.automationId = automationId
            self.parentKey = parentKey
            self.order = order
            self.props = props
        }

        @objc func press() {
            onPress?()
        }
    }
}

private final class IslandContainerView: NSView {
    private var pointerEvents: [ObjectIdentifier: String] = [:]

    nonisolated override var isFlipped: Bool { true }

    func setPointerEvents(_ mode: String, for view: NSView) {
        pointerEvents[ObjectIdentifier(view)] = mode
    }

    func removePointerEvents(for view: NSView) {
        pointerEvents.removeValue(forKey: ObjectIdentifier(view))
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        if let button = subviews.reversed().compactMap({ $0 as? IslandButton }).first(where: { button in
            guard button.hitSlop > 0, button.pointerEvents == "auto" || button.pointerEvents == "box-only" else {
                return false
            }
            let local = convert(point, to: button)
            return button.bounds.insetBy(dx: -button.hitSlop, dy: -button.hitSlop).contains(local)
        }) {
            return button
        }
        guard let hit = super.hitTest(point), hit !== self else { return nil }
        var directChild = hit
        while let parent = directChild.superview, parent !== self {
            directChild = parent
        }
        switch pointerEvents[ObjectIdentifier(directChild)] ?? "auto" {
        case "none", "box-none": return nil
        case "box-only": return directChild
        default: return hit
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

private final class IslandTextField: NSTextField {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

private final class IslandButton: NSButton {
    var pointerEvents: String = "auto"
    var hitSlop: CGFloat = 0

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard pointerEvents == "auto" || pointerEvents == "box-only" else { return nil }
        return super.hitTest(point)
    }
}
#endif
