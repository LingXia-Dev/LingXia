#if os(macOS)
import AppKit

/// VoiceOver's view of a GPU page on macOS: one element per semantic node of the presented
/// frame. The core decides roles, names, states and actions; this maps them to AppKit.
@MainActor
final class MacGpuAccessibility {
    private unowned let view: GpuPageView
    private var elements: [UInt32: MacGpuAccessibilityElement] = [:]
    private var order: [MacGpuAccessibilityElement]?

    init(view: GpuPageView) { self.view = view }

    var list: [MacGpuAccessibilityElement] {
        if let order { return order }
        var seen = Set<UInt32>()
        var built: [MacGpuAccessibilityElement] = []
        func visit(_ id: UInt32) {
            guard seen.insert(id).inserted, let node = node(id) else { return }
            if node.visible {
                let element = elements[id] ?? MacGpuAccessibilityElement(page: view, id: id)
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

    private func node(_ id: UInt32) -> GpuFrameSemanticNode? {
        let json = gpuPageSemanticNode(view.token, id).toString()
        guard !json.isEmpty else { return nil }
        return try? JSONDecoder().decode(GpuFrameSemanticNode.self, from: Data(json.utf8))
    }

    func invalidate() { order = nil }

    /// An event from the core: kinds as `Shell::accessibility_event` documents.
    func event(kind: UInt8, id: UInt32, text: String) {
        invalidate()
        switch kind {
        case 0, 2:
            if let element = list.first(where: { $0.id == id }) {
                NSAccessibility.post(element: element, notification: .focusedUIElementChanged)
            }
        case 5:
            if let element = list.first(where: { $0.id == id }) {
                NSAccessibility.post(element: element, notification: .valueChanged)
            }
        case 6:
            NSAccessibility.post(element: view, notification: .layoutChanged)
        case 7, 8:
            NSAccessibility.post(
                element: view.window ?? view, notification: .announcementRequested,
                userInfo: [
                    .announcement: text,
                    .priority: (kind == 8 ? NSAccessibilityPriorityLevel.high : .medium).rawValue,
                ])
        default:
            break
        }
    }
}

/// One semantic node; actions go back to the core by node id.
final class MacGpuAccessibilityElement: NSAccessibilityElement {
    let id: UInt32
    private let token: Int64
    private weak var page: GpuPageView?

    @MainActor init(page: GpuPageView, id: UInt32) {
        self.id = id
        self.token = page.token
        self.page = page
        super.init()
        setAccessibilityParent(page)
    }

    @MainActor func apply(_ node: GpuFrameSemanticNode) {
        setAccessibilityIdentifier(node.testId.isEmpty ? nil : node.testId)
        setAccessibilityLabel(node.name.isEmpty ? (node.textField?.hint ?? node.text) : node.name)
        setAccessibilityValue(Self.value(node))
        setAccessibilityRole(Self.role(node))
        setAccessibilityEnabled(node.enabled)
        setAccessibilitySelected(node.selected == true || node.checked == true)
        // Frames are in screen space; the node's bounds are in the page view's points.
        if let page, node.bounds.count == 4 {
            let local = NSRect(
                x: CGFloat(node.bounds[0]), y: CGFloat(node.bounds[1]),
                width: CGFloat(node.bounds[2]), height: CGFloat(node.bounds[3]))
            let inWindow = page.convert(local, to: nil)
            setAccessibilityFrame(page.window?.convertToScreen(inWindow) ?? inWindow)
        }
    }

    private static func value(_ node: GpuFrameSemanticNode) -> Any? {
        if let value = node.value { return value }
        if let field = node.textField { return field.secure ? nil : field.text }
        if let range = node.range { return range.value }
        if let checked = node.checked { return checked ? 1 : 0 }
        return nil
    }

    private static func role(_ node: GpuFrameSemanticNode) -> NSAccessibility.Role {
        switch node.role {
        case "button", "combo-box": return .button
        case "check-box", "switch": return .checkBox
        case "radio-button": return .radioButton
        case "radio-group": return .radioGroup
        case "link": return .link
        case "heading", "text": return .staticText
        case "image": return .image
        case "slider": return .slider
        case "text-field": return .textField
        case "scroll-view", "horizontal-scroll-view": return .scrollArea
        case "list": return .list
        case "dialog", "alert": return .group
        default: return node.clickable ? .button : .group
        }
    }

    private func send(_ code: UInt8) -> Bool {
        gpuPageAccessibility(token, id, code, 0, "")
    }

    override func accessibilityPerformPress() -> Bool { send(2) }

    override func accessibilityPerformIncrement() -> Bool { send(4) }

    override func accessibilityPerformDecrement() -> Bool { send(5) }

    override func isAccessibilityFocused() -> Bool { false }

    override func setAccessibilityFocused(_ focused: Bool) { _ = send(focused ? 0 : 1) }
}
#endif
