#if os(iOS)
import UIKit

/// VoiceOver's view of a GPU page: one element per semantic node of the presented frame.
/// The core decides roles, names, states and actions; this maps them to UIKit.
@MainActor
final class GpuAccessibility {
    private unowned let view: GpuPageView
    private var elements: [UInt32: GpuAccessibilityElement] = [:]
    private var order: [GpuAccessibilityElement]?

    init(view: GpuPageView) { self.view = view }

    /// Elements in reading order, rebuilt after the content changed.
    var list: [GpuAccessibilityElement] {
        if let order { return order }
        var seen = Set<UInt32>()
        var built: [GpuAccessibilityElement] = []
        func visit(_ id: UInt32) {
            guard seen.insert(id).inserted, let node = node(id) else { return }
            if node.visible {
                let element = elements[id] ?? GpuAccessibilityElement(page: view, id: id)
                element.apply(node)
                elements[id] = element
                built.append(element)
            }
            node.children.forEach(visit)
        }
        let roots = gpuPageSemanticRoots(view.token)
        for index in 0..<roots.len() { visit(roots.get(index: UInt(index)) ?? 0) }
        elements = elements.filter { seen.contains($0.key) }
        order = built
        return built
    }

    func node(_ id: UInt32) -> GpuFrameSemanticNode? {
        let json = gpuPageSemanticNode(view.token, id).toString()
        guard !json.isEmpty else { return nil }
        return try? JSONDecoder().decode(GpuFrameSemanticNode.self, from: Data(json.utf8))
    }

    func invalidate() { order = nil }

    /// An event from the core: kinds as `Shell::accessibility_event` documents.
    func event(kind: UInt8, id: UInt32, text: String) {
        switch kind {
        case 0, 2:
            invalidate()
            UIAccessibility.post(notification: .layoutChanged, argument: list.first { $0.id == id })
        case 5:
            invalidate()
        case 6:
            invalidate()
            UIAccessibility.post(notification: .layoutChanged, argument: nil)
        case 7, 8:
            // A polite region waits for current speech; an assertive one interrupts it.
            let spoken = NSAttributedString(
                string: text, attributes: [.accessibilitySpeechQueueAnnouncement: kind == 7])
            UIAccessibility.post(notification: .announcement, argument: spoken)
        default:
            break
        }
    }
}

/// One semantic node; actions go back to the core by node id.
final class GpuAccessibilityElement: UIAccessibilityElement {
    let id: UInt32
    private weak var page: GpuPageView?
    private var actions: [GpuFrameSemanticAction] = []
    private var scrollableHorizontally = false

    @MainActor init(page: GpuPageView, id: UInt32) {
        self.id = id
        self.page = page
        super.init(accessibilityContainer: page)
    }

    @MainActor func apply(_ node: GpuFrameSemanticNode) {
        accessibilityIdentifier = node.testId.isEmpty ? nil : node.testId
        accessibilityLabel = node.name.isEmpty ? (node.textField?.hint ?? node.text) : node.name
        accessibilityValue = Self.value(node)
        accessibilityTraits = Self.traits(node)
        if node.bounds.count == 4 {
            accessibilityFrameInContainerSpace = CGRect(
                x: CGFloat(node.bounds[0]), y: CGFloat(node.bounds[1]),
                width: CGFloat(node.bounds[2]), height: CGFloat(node.bounds[3]))
        }
        actions = node.actions
        scrollableHorizontally = node.scroll?.horizontal ?? false
        accessibilityCustomActions = node.actions.compactMap { action in
            let kind = action.kind
            guard ["copy", "cut", "paste", "select-item"].contains(kind) else { return nil }
            let name = action.label ?? Self.actionName(kind)
            return UIAccessibilityCustomAction(name: name) { [weak self] _ in
                MainActor.assumeIsolated { self?.perform(kind, index: action.index) ?? false }
            }
        }
    }

    private static func actionName(_ kind: String) -> String {
        switch kind {
        case "copy": return NSLocalizedString("Copy", comment: "")
        case "cut": return NSLocalizedString("Cut", comment: "")
        case "paste": return NSLocalizedString("Paste", comment: "")
        default: return kind
        }
    }

    private static func value(_ node: GpuFrameSemanticNode) -> String? {
        if let value = node.value { return value }
        if let field = node.textField { return field.secure ? nil : field.text }
        if let range = node.range { return String(format: "%g", range.value) }
        if let checked = node.checked { return checked ? "1" : "0" }
        return nil
    }

    private static func traits(_ node: GpuFrameSemanticNode) -> UIAccessibilityTraits {
        var traits: UIAccessibilityTraits = []
        switch node.role {
        case "button", "combo-box", "radio-button": traits.insert(.button)
        case "check-box", "switch":
            if #available(iOS 17.0, *) { traits.insert(.toggleButton) } else { traits.insert(.button) }
        case "link": traits.insert(.link)
        case "heading": traits.insert(.header)
        case "image": traits.insert(.image)
        case "slider": traits.insert(.adjustable)
        case "text": traits.insert(.staticText)
        default: break
        }
        if node.clickable, traits.isEmpty { traits.insert(.button) }
        if !node.enabled { traits.insert(.notEnabled) }
        if node.selected == true || node.checked == true { traits.insert(.selected) }
        if node.liveRegion != nil { traits.insert(.updatesFrequently) }
        return traits
    }

    @MainActor private func perform(_ kind: String, index: UInt64? = nil, argument: String = "") -> Bool {
        guard let page else { return false }
        let code: UInt8
        var number = 0.0
        var argument = argument
        switch kind {
        case "click": code = 2
        case "scroll-forward": code = 4
        case "scroll-backward": code = 5
        case "copy": code = 11
        case "cut": code = 12
        case "paste":
            code = 13
            argument = UIPasteboard.general.string ?? ""
        case "select-item":
            code = 10
            number = Double(index ?? 0)
        default: return false
        }
        return gpuPageAccessibility(page.token, id, code, number, argument)
    }

    private func has(_ kind: String) -> Bool { actions.contains { $0.kind == kind } }

    override func accessibilityActivate() -> Bool {
        MainActor.assumeIsolated { perform("click") }
    }

    override func accessibilityIncrement() {
        MainActor.assumeIsolated { _ = perform("scroll-forward") }
    }

    override func accessibilityDecrement() {
        MainActor.assumeIsolated { _ = perform("scroll-backward") }
    }

    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
        // A three-finger swipe up (or left, or "next") brings the following content in.
        let forward: Bool
        switch direction {
        case .up, .left, .next: forward = true
        default: forward = false
        }
        return MainActor.assumeIsolated {
            perform(forward ? "scroll-forward" : "scroll-backward")
        }
    }

    override func accessibilityElementDidBecomeFocused() {
        MainActor.assumeIsolated {
            guard let page else { return }
            _ = gpuPageAccessibility(page.token, id, 0, 0, "")
        }
    }

    override func accessibilityElementDidLoseFocus() {
        MainActor.assumeIsolated {
            guard let page else { return }
            _ = gpuPageAccessibility(page.token, id, 1, 0, "")
        }
    }
}
#endif
