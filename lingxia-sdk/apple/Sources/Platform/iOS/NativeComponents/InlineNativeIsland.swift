#if os(iOS)
import UIKit

/// One ordered container for an inline native Root on iOS.
/// Kind-specific views share this container; only public UIKit APIs are used.
@MainActor
final class InlineNativeIsland {
    static let allowedKinds: Set<String> = ["root", "view", "text", "tappable", "slider", "video"]

    static func isIslandAction(_ action: String) -> Bool {
        action == "root.commit" || action == "geometry.snapshot"
            || action == "root.leaseAccept" || action == "video.command"
    }

    private let container = IslandContainerView()
    private weak var manager: NativeComponentManager?
    private weak var host: UIView?
    private var nodes: [String: IslandNode] = [:]
    private(set) var lastAppliedRevision: UInt64 = 0
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
    private var lastRoot: [String: Any]?
    private var pageActive = true
    private var pendingOutgoing: [[String: Any]] = []

    init(
        host: UIView,
        manager: NativeComponentManager,
        eventSink: @escaping (_ componentId: String, _ event: String, _ detail: [String: Any]) -> Void
    ) {
        self.manager = manager
        self.host = host
        self.eventSink = eventSink
        container.isUserInteractionEnabled = true
        // The overlay host pins to the scroll view's content layout guide, which
        // resolves empty because nothing sizes it; clipping to it erases the island.
        container.clipsToBounds = false
        if container.superview == nil {
            host.addSubview(container)
        }
        syncContainerFrame()
    }

    /// Cover every committed node so the container both paints and hit-tests.
    private func syncContainerFrame() {
        var size = host?.bounds.size ?? .zero
        for node in nodes.values where node.visible {
            size.width = max(size.width, node.rect.maxX)
            size.height = max(size.height, node.rect.maxY)
        }
        container.frame = CGRect(origin: .zero, size: size)
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

    /// Hide the island while its page is not the interactive one: a native surface
    /// outlives the WebView's own visibility, and a visible node on an inactive page
    /// still answers hit tests meant for the page now on screen.
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
        lastRoot = nil
        leases.removeAll()
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
        syncContainerFrame()
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
            node.rect = CGRect(
                x: cg(rect["x"]),
                y: cg(rect["y"]),
                width: max(cg(rect["width"]), 1),
                height: max(cg(rect["height"]), 1)
            )
            node.visible = entry["visible"] as? Bool ?? true
            applyFrame(node)
        }
        syncContainerFrame()
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
        if name == "setStreamSource", let options = command?["options"] as? [String: Any] {
            params = options
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
        if item.kind == "tappable" || item.kind == "slider" || item.kind == "video" {
            manager?.registerIslandTouchTarget(item.view)
        }
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
            // The player retains this sink for its lifetime, so hold the node weakly.
            let authorId = item.authorId
            let video = VideoComponent(id: authorId, initialProps: item.props) { [weak self] event in
                guard let name = event["event"] as? String else { return }
                let detail = event["detail"] as? [String: Any] ?? [:]
                self?.eventSink(authorId, name, detail)
            }
            video.mount(in: container)
            item.video = video
            item.view = video.view
            manager?.attachIslandVideo(id: item.authorId, component: video)
            return
        case "text":
            let label = UILabel()
            label.isUserInteractionEnabled = false
            item.label = label
            item.view = label
        case "tappable":
            let button = IslandButton(frame: .zero)
            button.addAction(UIAction { [weak self, weak item] _ in
                guard let item, !(self?.boolValue(item.props["disabled"]) ?? false) else { return }
                self?.eventSink(item.authorId, "press", ["source": "pointer"])
            }, for: .touchUpInside)
            item.button = button
            item.view = button
        case "slider":
            let slider = UISlider()
            slider.addAction(UIAction { [weak self, weak item] _ in
                guard let item, let slider = item.slider else { return }
                item.dragging = true
                let value = self?.sliderValue(item, proposed: Double(slider.value)) ?? Double(slider.value)
                slider.value = Float(value)
                self?.eventSink(item.authorId, "valuechange", ["value": value])
            }, for: .valueChanged)
            slider.addAction(UIAction { [weak self, weak item] _ in
                guard let item, let slider = item.slider else { return }
                item.dragging = false
                let value = self?.sliderValue(item, proposed: Double(slider.value)) ?? Double(slider.value)
                slider.value = Float(value)
                self?.eventSink(item.authorId, "valuecommit", ["value": value])
            }, for: [.touchUpInside, .touchUpOutside, .touchCancel])
            item.slider = slider
            item.view = slider
        default:
            let view = UIView()
            view.isUserInteractionEnabled = false
            item.view = view
        }
        item.view.isUserInteractionEnabled = item.kind == "tappable" || item.kind == "slider" || item.kind == "video"
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
                let minimum = cg(item.props["min"], fallback: 0)
                let maximum = Swift.max(cg(item.props["max"], fallback: 100), minimum + 1)
                slider.minimumValue = Float(minimum)
                slider.maximumValue = Float(maximum)
                slider.value = Float(Swift.min(Swift.max(cg(item.props["value"], fallback: minimum), minimum), maximum))
                slider.isEnabled = !boolValue(item.props["disabled"])
                slider.accessibilityValue = formatSliderValue(item.props, value: Double(slider.value))
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

    private func applyText(_ item: IslandNode) {
        guard let label = item.label else { return }
        let text = item.props["text"] as? String ?? ""
        label.textColor = color(item.props["color"]) ?? .white
        let size = cg(item.props["fontSize"], fallback: 12)
        label.font = .systemFont(ofSize: size, weight: fontWeight(item.props["fontWeight"]))
        let maxLines = Int(cg(item.props["maxLines"]))
        label.numberOfLines = maxLines > 0 ? maxLines : 0
        label.lineBreakMode = maxLines > 0 ? .byTruncatingTail : .byWordWrapping
        switch item.props["textAlign"] as? String {
        case "center": label.textAlignment = .center
        case "end": label.textAlignment = .right
        default: label.textAlignment = .left
        }
        switch item.props["dir"] as? String {
        case "rtl": label.semanticContentAttribute = .forceRightToLeft
        case "ltr": label.semanticContentAttribute = .forceLeftToRight
        default: label.semanticContentAttribute = .unspecified
        }
        let lineHeight = cg(item.props["lineHeight"])
        if lineHeight > 0, let font = label.font, let textColor = label.textColor {
            let paragraph = NSMutableParagraphStyle()
            paragraph.minimumLineHeight = lineHeight
            paragraph.maximumLineHeight = lineHeight
            paragraph.alignment = label.textAlignment
            paragraph.baseWritingDirection = item.props["dir"] as? String == "rtl" ? .rightToLeft : .leftToRight
            label.attributedText = NSAttributedString(
                string: text,
                attributes: [.font: font, .foregroundColor: textColor, .paragraphStyle: paragraph]
            )
        } else {
            label.attributedText = nil
            label.text = text
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
        let title: String
        if loading {
            title = "…"
        } else if let glyph, !text.isEmpty {
            title = item.props["iconPosition"] as? String == "end" ? "\(text)  \(glyph)" : "\(glyph)  \(text)"
        } else {
            title = glyph ?? text
        }
        button.setTitle(title, for: .normal)
        let disabled = boolValue(item.props["disabled"])
        button.isEnabled = !disabled && !loading
        let colors = buttonColors(
            intent: item.props["intent"] as? String ?? "neutral",
            emphasis: item.props["emphasis"] as? String ?? "secondary",
            pressed: boolValue(item.props["pressed"]),
            disabled: disabled || loading
        )
        button.backgroundColor = colors.background
        button.setTitleColor(color(item.props["color"]) ?? colors.foreground, for: .normal)
        button.titleLabel?.font = .systemFont(
            ofSize: cg(style(item.props)["fontSize"], fallback: item.props["size"] as? String == "compact" ? 12 : 14),
            weight: .semibold
        )
        button.layer.cornerRadius = cg(style(item.props)["borderRadius"], fallback: 10)
        (button as? IslandButton)?.hitSlop = cg(item.props["hitSlop"])
    }

    private func applyNativeStyle(_ item: IslandNode) {
        let nativeStyle = style(item.props)
        item.view.alpha = min(max(cg(nativeStyle["opacity"], fallback: 1), 0), 1)
        if let background = color(nativeStyle["backgroundColor"]), item.kind != "video" {
            item.view.layer.backgroundColor = background.cgColor
        } else if item.kind == "view", item.props["scrimPaint"] == nil {
            item.view.layer.backgroundColor = UIColor.clear.cgColor
        }
        if nativeStyle["borderRadius"] != nil || item.kind == "view" {
            item.view.layer.cornerRadius = cg(nativeStyle["borderRadius"])
        }
        item.view.layer.borderWidth = cg(nativeStyle["borderWidth"])
        item.view.layer.borderColor = color(nativeStyle["borderColor"])?.cgColor
        item.view.clipsToBounds = item.view.layer.cornerRadius > 0
    }

    private func applyAccessibility(_ item: IslandNode) {
        let hidden = boolValue(item.props["aria-hidden"]) || boolValue(item.props["ariaHidden"])
        item.view.isAccessibilityElement = !hidden && ["text", "tappable", "slider", "video"].contains(item.kind)
        item.view.accessibilityElementsHidden = hidden
        item.view.accessibilityIdentifier = item.automationId
        item.view.accessibilityLabel = (item.props["aria-label"] as? String)
            ?? (item.props["ariaLabel"] as? String)
            ?? (item.kind == "tappable" ? item.button?.title(for: .normal) : nil)
        item.view.accessibilityHint = (item.props["aria-description"] as? String)
            ?? (item.props["ariaDescription"] as? String)
        if item.kind == "tappable" {
            item.view.accessibilityTraits = item.button?.isEnabled == true ? .button : [.button, .notEnabled]
        }
    }

    private func applyPointerEvents(_ item: IslandNode) {
        let mode = item.props["pointerEvents"] as? String ?? ((item.kind == "text" || item.kind == "view") ? "box-none" : "auto")
        let interactive = item.kind == "tappable" || item.kind == "slider" || item.kind == "video"
        item.view.isUserInteractionEnabled = interactive && (mode == "auto" || mode == "box-only")
    }

    private func applyScrim(_ item: IslandNode) {
        item.view.layer.sublayers?.removeAll(where: { $0.name == "lx-scrim" })
        guard let paint = item.props["scrimPaint"] as? [String: Any] else { return }
        let scrim = paint["scrim"] as? String ?? "none"
        let opacity = CGFloat(cg(paint["opacity"]))
        guard scrim != "none" else { return }
        let layer = CAGradientLayer()
        layer.name = "lx-scrim"
        layer.frame = item.view.bounds
        let color = UIColor.black.withAlphaComponent(opacity).cgColor
        switch scrim {
        case "top":
            layer.colors = [color, UIColor.clear.cgColor]
        case "bottom":
            layer.colors = [UIColor.clear.cgColor, color]
        default:
            layer.colors = [color, color]
        }
        item.view.layer.insertSublayer(layer, at: 0)
        item.scrim = layer
    }

    private func applyFrame(_ item: IslandNode) {
        // A fullscreen video lives in its own window until it exits; leave its layout alone.
        guard item.view.superview === container else { return }
        let rect = textFittedRect(item)
        if let video = item.video {
            // The player owns its view's layout: assigning the frame here first makes
            // its own setFrame a no-op, leaving the player layer at zero bounds.
            video.setFrame(rect)
        } else {
            item.view.frame = rect
        }
        item.view.isHidden = !pageActive || !item.visible || item.rect.width <= 0 || item.rect.height <= 0
        item.scrim?.frame = item.view.bounds
    }

    /// Web and native shapers space the same nominal font differently, so a box the
    /// page measured can fall a few points short. Widen rather than drop characters.
    private func textFittedRect(_ item: IslandNode) -> CGRect {
        guard item.kind == "text", let label = item.label, item.rect.width > 0 else { return item.rect }
        let fitting = label.sizeThatFits(
            CGSize(width: CGFloat.greatestFiniteMagnitude, height: item.rect.height)
        ).width
        guard fitting > item.rect.width else { return item.rect }
        var rect = item.rect
        switch label.textAlignment {
        case .center: rect.origin.x -= (fitting - rect.width) / 2
        case .right: rect.origin.x -= fitting - rect.width
        default: break
        }
        rect.size.width = ceil(fitting)
        return rect
    }

    private func restack() {
        let ordered = nodes.values.sorted { lhs, rhs in
            if lhs.order != rhs.order { return lhs.order < rhs.order }
            return lhs.key < rhs.key
        }
        var index = 0
        for node in ordered where node.view.superview === container {
            container.insertSubview(node.view, at: index)
            index += 1
        }
    }

    private func removeNode(_ key: String) {
        guard let node = nodes.removeValue(forKey: key) else { return }
        manager?.unregisterIslandTouchTarget(node.view)
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

    private func color(_ value: Any?) -> UIColor? {
        guard let raw = value as? String else { return nil }
        return NativeComponentColorStyle.parseColor(raw)
    }

    /// Map the CSS weight onto the full system scale: collapsing everything above
    /// 500 to `.bold` renders text wider than the CSS box it was measured into.
    private func fontWeight(_ value: Any?) -> UIFont.Weight {
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

    private func sliderValue(_ item: IslandNode, proposed: Double) -> Double {
        let minimum = Double(cg(item.props["min"], fallback: 0))
        let maximum = max(Double(cg(item.props["max"], fallback: 100)), minimum + 1)
        let step = Double(cg(item.props["step"]))
        let snapped = step > 0 ? minimum + ((proposed - minimum) / step).rounded() * step : proposed
        return min(max(snapped, minimum), maximum)
    }

    private func formatSliderValue(_ props: [String: Any], value: Double) -> String? {
        switch props["valueLabel"] as? String {
        case "value":
            return value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
        case "time":
            let total = max(Int(value.rounded()), 0)
            let hours = total / 3600
            let minutes = (total % 3600) / 60
            let seconds = total % 60
            return hours > 0
                ? String(format: "%d:%02d:%02d", hours, minutes, seconds)
                : String(format: "%d:%02d", minutes, seconds)
        default:
            return nil
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
    ) -> (background: UIColor, foreground: UIColor) {
        if disabled {
            return (UIColor(red: 0.61, green: 0.64, blue: 0.69, alpha: 1), .white)
        }
        let base: UIColor
        switch intent {
        case "accent": base = UIColor(red: 0.15, green: 0.39, blue: 0.92, alpha: 1)
        case "destructive": base = UIColor(red: 0.86, green: 0.15, blue: 0.15, alpha: 1)
        default: base = UIColor(red: 0.22, green: 0.25, blue: 0.32, alpha: 1)
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
    private final class IslandNode {
        let key: String
        let kind: String
        let authorId: String
        let automationId: String?
        var order: Int
        var props: [String: Any]
        var view = UIView()
        var video: VideoComponent?
        var label: UILabel?
        var button: UIButton?
        var slider: UISlider?
        var scrim: CAGradientLayer?
        var rect: CGRect = .zero
        var visible = true
        var dragging = false

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
    }
}

private final class IslandButton: UIButton {
    var hitSlop: CGFloat = 0

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        guard hitSlop > 0 else { return super.point(inside: point, with: event) }
        return bounds.insetBy(dx: -hitSlop, dy: -hitSlop).contains(point)
    }
}

private final class IslandContainerView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        for button in subviews.reversed().compactMap({ $0 as? IslandButton }) {
            guard !button.isHidden, button.isUserInteractionEnabled, button.hitSlop > 0 else { continue }
            let local = convert(point, to: button)
            if button.bounds.insetBy(dx: -button.hitSlop, dy: -button.hitSlop).contains(local) {
                return button
            }
        }
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }
}
#endif
