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
        var controls: [(MacGpuAccessibilityElement, [UInt32])] = []
        func visit(_ id: UInt32) {
            guard seen.insert(id).inserted, let node = node(id) else { return }
            if node.visible {
                let element = elements[id] ?? MacGpuAccessibilityElement(page: view, id: id)
                element.apply(node)
                elements[id] = element
                built.append(element)
                controls.append((element, node.controls ?? []))
            }
            node.children.forEach(visit)
        }
        let roots = gpuPageSemanticRoots(view.token)
        for index in 0..<roots.len() { visit(roots.get(index: UInt(index)) ?? 0) }
        elements = elements.filter { seen.contains($0.key) }
        // `aria-controls`: VoiceOver's "jump to linked item" reaches the list a combobox drives.
        for (element, ids) in controls {
            element.setAccessibilityLinkedUIElements(ids.compactMap { elements[$0] })
        }
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
    private var dismissable = false
    private var adjustable = (forward: false, backward: false)

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
        setAccessibilityHelp(node.description)
        setAccessibilityRequired(node.required == true)
        setAccessibilityEnabled(node.enabled)
        setAccessibilitySelected(node.selected == true || node.checked == true)
        // An outline row's depth counts from 0.
        setAccessibilityDisclosureLevel(max(Int(node.level ?? 1) - 1, 0))
        // A swipe row's `aria-actions`: the same actions VoiceOver's rotor and automation perform.
        let token = self.token, id = self.id
        setAccessibilityCustomActions(node.actions.compactMap { action in
            guard action.kind == "custom", let label = action.label else { return nil }
            let index = Double(action.index ?? 0)
            return NSAccessibilityCustomAction(name: label) { gpuPageAccessibility(token, id, 14, index, "") }
        })
        dismissable = node.actions.contains { $0.kind == "dismiss" }
        adjustable = (node.actions.contains { $0.kind == "scroll-forward" },
                      node.actions.contains { $0.kind == "scroll-backward" })
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

    /// Roles already reported as unmapped, so each is logged once.
    @MainActor private static var unmappedRoles = Set<String>()

    @MainActor private static func role(_ node: GpuFrameSemanticNode) -> NSAccessibility.Role {
        // Exhaustive over the generated roles: a role the core adds fails this build until mapped.
        switch GpuSemantics.Role(rawValue: node.role) {
        case .comboBox where node.textField != nil: return .comboBox
        case .button, .comboBox: return .button
        case .checkBox, .switch: return .checkBox
        case .radioButton: return .radioButton
        case .radioGroup: return .radioGroup
        case .link: return .link
        case .heading, .text: return .staticText
        case .image, .video: return .image
        case .slider: return .slider
        case .progressBar: return .progressIndicator
        case .textField: return .textField
        case .scrollView, .horizontalScrollView: return .scrollArea
        case .list, .listBox: return .list
        case .grid: return .grid
        case .gridCell, .option: return .cell
        case .menu: return .menu
        case .menuItem, .menuItemCheckBox, .menuItemRadio: return .menuItem
        case .tabList: return .tabGroup
        case .tab: return .radioButton
        case .toolbar: return .toolbar
        case .tree: return .outline
        case .treeItem, .row: return .row
        case .dialog, .alertDialog, .alert, .listItem, .status, .group, .tabPanel, .webView: return .group
        case .tooltip: return .staticText
        case nil:
            // A core newer than this adapter; a shipping host reads it as a group.
            if unmappedRoles.insert(node.role).inserted {
                LXLog.warn("unmapped semantic role \(node.role)", category: "Accessibility")
            }
            return node.clickable ? .button : .group
        }
    }

    private func send(_ code: UInt8) -> Bool {
        gpuPageAccessibility(token, id, code, 0, "")
    }

    override func accessibilityPerformPress() -> Bool { send(2) }

    override func accessibilityPerformIncrement() -> Bool { send(4) }

    override func accessibilityPerformDecrement() -> Bool { send(5) }

    /// Escape for assistive tech: an open dialog closes as its close request would.
    override func accessibilityPerformCancel() -> Bool { dismissable && send(15) }

    /// Only the actions the node has: AppKit would otherwise list every overridden one.
    override func isAccessibilitySelectorAllowed(_ selector: Selector) -> Bool {
        switch selector {
        case #selector(accessibilityPerformCancel): return dismissable
        case #selector(accessibilityPerformIncrement): return adjustable.forward
        case #selector(accessibilityPerformDecrement): return adjustable.backward
        default: return super.isAccessibilitySelectorAllowed(selector)
        }
    }

    override func isAccessibilityFocused() -> Bool { false }

    override func setAccessibilityFocused(_ focused: Bool) { _ = send(focused ? 0 : 1) }
}
#endif
