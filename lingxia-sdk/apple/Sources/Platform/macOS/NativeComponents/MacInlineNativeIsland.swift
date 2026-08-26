#if os(macOS)
import AppKit

/// One ordered NSView host for an inline native Root on macOS.
/// Sibling order comes from the committed tree, never from message arrival.
@MainActor
final class MacInlineNativeIsland {
    static let allowedKinds: Set<String> = ["root", "view", "text", "tappable", "slider", "video"]

    private let container = IslandContainerView()
    private var nodes: [String: IslandNode] = [:]
    private(set) var lastAppliedRevision: UInt64 = 0
    private let eventSink: (_ componentId: String, _ event: String, _ detail: [String: Any]) -> Void
    private var leaseGranted = false
    private var leaseActive = false
    private var leaseId = ""
    private var leaseSequence: UInt64 = 1
    private var lastRoot: [String: Any]?
    private var pendingOutgoing: [[String: Any]] = []
    private var scrollOffset = CGPoint.zero

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

    func rebind(to host: NSView) {
        guard container.superview !== host else { return }
        container.removeFromSuperview()
        host.addSubview(container)
        container.frame = host.bounds
        container.autoresizingMask = [.width, .height]
    }

    func updateScrollOffset(x: CGFloat, y: CGFloat) {
        scrollOffset = CGPoint(x: x, y: y)
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
        lastRoot = nil
        leaseGranted = false
        leaseActive = false
        leaseId = ""
        leaseSequence = 1
        lastAppliedRevision = 0
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
        let automationId = (node["automationId"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        let props = node["props"] as? [String: Any] ?? [:]
        let order = node["order"] as? Int ?? 0
        removeNode(key)
        let item = IslandNode(
            key: key,
            kind: kind,
            authorId: authorId,
            automationId: automationId,
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
            let video = MacVideoComponent(id: item.authorId, initialProps: item.props) { [weak self] event in
                guard let name = event["event"] as? String else { return }
                let detail = event["detail"] as? [String: Any] ?? [:]
                self?.eventSink(item.authorId, name, detail)
            }
            video.mount(in: container)
            item.video = video
            item.view = video.view
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
                guard let item, !(item.props["disabled"] as? Bool ?? false) else { return }
                let source = NSApp.currentEvent?.type == .keyDown ? "keyboard" : "pointer"
                self?.eventSink(item.authorId, "press", ["source": source])
            }
            item.button = button
            item.view = button
        case "slider":
            let slider = IslandSlider()
            slider.minValue = 0
            slider.maxValue = 100
            slider.target = item
            slider.action = #selector(IslandNode.sliderChanged)
            item.onSliderChange = { [weak self, weak item] value, commit in
                guard let item else { return }
                let event = commit ? "valuecommit" : "valuechange"
                let snapped = self?.sliderValue(item, proposed: value) ?? value
                item.slider?.doubleValue = snapped
                self?.eventSink(item.authorId, event, ["value": snapped])
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
            applyText(item)
        case "tappable":
            applyButton(item)
        case "slider":
            if let slider = item.slider, !item.dragging {
                let minimum = Double(cg(item.props["min"], fallback: 0))
                let maximum = Swift.max(Double(cg(item.props["max"], fallback: 100)), minimum + 1)
                slider.minValue = minimum
                slider.maxValue = maximum
                slider.doubleValue = Swift.min(Swift.max(Double(cg(item.props["value"], fallback: CGFloat(minimum))), minimum), maximum)
                slider.isEnabled = !boolValue(item.props["disabled"])
                slider.setAccessibilityValue(formatSliderValue(item.props, value: slider.doubleValue))
            }
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
        (item.view as? IslandSlider)?.pointerEvents = mode
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
        item.view.setAccessibilityElement(!hidden && ["text", "tappable", "slider", "video"].contains(item.kind))
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
        case "slider": item.view.setAccessibilityRole(.slider)
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
        let viewportRect = NSRect(
            x: item.rect.origin.x - scrollOffset.x,
            y: item.rect.origin.y - scrollOffset.y,
            width: item.rect.width,
            height: item.rect.height
        )
        item.view.frame = viewportRect
        item.view.isHidden = !item.visible || item.rect.width <= 0 || item.rect.height <= 0
        item.scrim?.frame = item.view.bounds
        item.video?.setFrame(viewportRect)
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
        container.removePointerEvents(for: node.view)
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

    private func fontWeight(_ value: Any?) -> NSFont.Weight {
        if let number = value as? NSNumber {
            return number.intValue >= 600 ? .bold : .regular
        }
        let raw = (value as? String)?.lowercased() ?? ""
        return raw == "bold" || raw == "bolder" || (Int(raw) ?? 400) >= 600 ? .bold : .regular
    }

    private func sliderValue(_ item: IslandNode, proposed: Double) -> Double {
        let minimum = Double(cg(item.props["min"], fallback: 0))
        let maximum = max(Double(cg(item.props["max"], fallback: 100)), minimum + 1)
        let step = Double(cg(item.props["step"]))
        let snapped = step > 0 ? minimum + ((proposed - minimum) / step).rounded() * step : proposed
        return min(max(snapped, minimum), maximum)
    }

    private func formatSliderValue(_ props: [String: Any], value: Double) -> String? {
        switch props["valueLabel"] as? String {
        case "value": return value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
        case "time":
            let total = max(Int(value.rounded()), 0)
            let hours = total / 3600
            let minutes = (total % 3600) / 60
            let seconds = total % 60
            return hours > 0
                ? String(format: "%d:%02d:%02d", hours, minutes, seconds)
                : String(format: "%d:%02d", minutes, seconds)
        default: return nil
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
        if let text = value as? String { return text == "true" || text == "1" }
        if let number = value as? NSNumber { return number.boolValue }
        return false
    }

    @MainActor
    private final class IslandNode: NSObject {
        let key: String
        let kind: String
        let authorId: String
        let automationId: String?
        var order: Int
        var props: [String: Any]
        var view = NSView()
        var video: MacVideoComponent?
        var label: NSTextField?
        var button: NSButton?
        var slider: NSSlider?
        var scrim: CAGradientLayer?
        var rect: NSRect = .zero
        var visible = true
        var dragging = false
        var onPress: (() -> Void)?
        var onSliderChange: ((Double, Bool) -> Void)?

        init(
            key: String,
            kind: String,
            authorId: String,
            automationId: String?,
            order: Int,
            props: [String: Any]
        ) {
            self.key = key
            self.kind = kind
            self.authorId = authorId
            self.automationId = automationId
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

private final class IslandSlider: NSSlider {
    var pointerEvents: String = "auto"

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard pointerEvents == "auto" || pointerEvents == "box-only" else { return nil }
        return super.hitTest(point)
    }
}
#endif
