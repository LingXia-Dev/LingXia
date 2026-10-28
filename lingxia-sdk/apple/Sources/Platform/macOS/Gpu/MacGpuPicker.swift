import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// AppKit pickers for GPU page controls: a menu for `<select>`, a date or time picker for
/// editors whose value is picked. The choice goes back to the core.
extension GpuPageView {
    /// `node` bounds as a rect in this view, which is flipped like the frame.
    private func rect(_ node: GpuFrameNode) -> NSRect {
        let b = node.visibleBounds.count == 4 ? node.visibleBounds : node.bounds
        guard b.count == 4 else { return .zero }
        return NSRect(x: CGFloat(b[0]), y: CGFloat(b[1]), width: CGFloat(b[2]), height: CGFloat(b[3]))
    }

    func openPicker(generation: UInt64, node: GpuFrameNode) {
        guard let select = node.select, !select.options.isEmpty else { return }
        let menu = NSMenu()
        menu.autoenablesItems = false
        let target = GpuPickerTarget { [weak self] index in
            guard let self else { return }
            self.imeRequest(.select(identity: node.identity, generation: generation, index: UInt64(index))) { [weak self] reply in
                self?.imeRender(rebuild: reply?.needsViewRender == true)
            }
        }
        for (index, option) in select.options.enumerated() {
            let item = NSMenuItem(title: option.label, action: #selector(GpuPickerTarget.choose(_:)), keyEquivalent: "")
            item.target = target
            item.tag = index
            item.isEnabled = option.enabled
            item.state = select.selected == UInt64(index) ? .on : .off
            menu.addItem(item)
        }
        let box = rect(node)
        let selected = select.selected.flatMap { Int(exactly: $0) }.flatMap { menu.items.indices.contains($0) ? menu.items[$0] : nil }
        // Pop up after the effect returns: the menu runs its own tracking loop.
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window != nil else { return }
            withExtendedLifetime(target) {
                menu.minimumWidth = box.width
                _ = menu.popUp(positioning: selected, at: NSPoint(x: box.minX, y: selected == nil ? box.maxY : box.minY), in: self)
            }
        }
    }

    /// A connected editor whose value comes from a picker shows it in place of typing.
    func showValuePicker() {
        guard let editor = ime.editor, let spec = ime.ime?.picker, let node = ime.node else { return }
        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        let time = spec.kind == GpuFramePickerKind.time.rawValue
        picker.datePickerElements = time ? .hourMinute : .yearMonthDay
        let calendar = Calendar(identifier: .gregorian)
        func date(_ parts: [UInt32]?) -> Date? {
            guard let parts, parts.count == 3 else { return nil }
            var components = DateComponents()
            if time {
                (components.hour, components.minute) = (Int(parts[0]), Int(parts[1]))
                (components.year, components.month, components.day) = (2000, 1, 1)
            } else {
                (components.year, components.month, components.day) = (Int(parts[0]), Int(parts[1]), Int(parts[2]))
            }
            return calendar.date(from: components)
        }
        picker.calendar = calendar
        picker.dateValue = date(spec.seed) ?? Date()
        picker.minDate = date(spec.minParts)
        picker.maxDate = date(spec.maxParts)
        picker.sizeToFit()

        let controller = NSViewController()
        let done = NSButton(title: NSLocalizedString("Done", comment: ""), target: nil, action: nil)
        done.keyEquivalent = "\r"
        let stack = NSStackView(views: [picker, done])
        stack.orientation = .vertical
        stack.edgeInsets = NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        controller.view = stack
        let popover = NSPopover()
        popover.contentViewController = controller
        popover.behavior = .transient
        let delegate = GpuValuePickerDelegate(popover: popover) { [weak self, weak editor] chosen in
            guard let self else { return }
            self.valuePicker = nil
            guard let editor, self.ime.editor === editor else { return }
            guard chosen else { return self.ime.pick(nil) }
            let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute], from: picker.dateValue)
            self.ime.pick(time
                ? [UInt32(parts.hour ?? 0), UInt32(parts.minute ?? 0), 0]
                : [UInt32(parts.year ?? 0), UInt32(parts.month ?? 1), UInt32(parts.day ?? 1)])
        }
        done.target = delegate
        done.action = #selector(GpuValuePickerDelegate.confirm)
        popover.delegate = delegate
        valuePicker = delegate
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window != nil else { return }
            popover.show(relativeTo: self.rect(node), of: self, preferredEdge: .maxY)
        }
    }
}

/// Menu items' target, alive while the menu is.
@MainActor
private final class GpuPickerTarget: NSObject {
    let chosen: (Int) -> Void
    init(_ chosen: @escaping (Int) -> Void) { self.chosen = chosen }
    @objc func choose(_ item: NSMenuItem) { chosen(item.tag) }
}

/// Reports a value picker's outcome once: confirmed, or closed without a choice.
@MainActor
final class GpuValuePickerDelegate: NSObject, NSPopoverDelegate {
    private weak var popover: NSPopover?
    private var outcome: ((Bool) -> Void)?
    init(popover: NSPopover, outcome: @escaping (Bool) -> Void) {
        self.popover = popover
        self.outcome = outcome
    }
    @objc func confirm() {
        finish(true)
        popover?.close()
    }
    func dismiss() { popover?.close() }
    nonisolated func popoverDidClose(_ notification: Notification) {
        MainActor.assumeIsolated { finish(false) }
    }
    private func finish(_ chosen: Bool) {
        let outcome = self.outcome
        self.outcome = nil
        outcome?(chosen)
    }
}
#endif
