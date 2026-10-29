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

    /// A connected editor whose value comes from a picker shows it in place of typing: a
    /// popover anchored to the field, on the side with room inside the window.
    func showValuePicker() {
        guard let editor = ime.editor, let spec = ime.ime?.picker, let node = ime.node else { return }
        let time = spec.kind == GpuFramePickerKind.time.rawValue
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
        let picker = GpuDatePicker()
        picker.datePickerStyle = time ? .textFieldAndStepper : .clockAndCalendar
        picker.datePickerElements = time ? .hourMinute : .yearMonthDay
        picker.isBezeled = false
        picker.drawsBackground = false
        picker.calendar = calendar
        picker.dateValue = date(spec.seed) ?? Date()
        picker.minDate = date(spec.minParts)
        picker.maxDate = date(spec.maxParts)
        picker.sizeToFit()

        let inset: CGFloat = 10
        let size = picker.fittingSize
        let content = GpuValuePickerView(frame: NSRect(x: 0, y: 0, width: size.width + 2 * inset, height: size.height + 2 * inset))
        picker.frame = NSRect(origin: NSPoint(x: inset, y: inset), size: size)
        content.addSubview(picker)
        let controller = NSViewController()
        controller.view = content

        let popover = NSPopover()
        popover.contentViewController = controller
        popover.contentSize = content.frame.size
        popover.behavior = .transient
        popover.animates = false
        if let dark = chrome?.dark {
            popover.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        }
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
        content.commit = { [weak delegate] in delegate?.close(choosing: true) }
        picker.commit = content.commit
        content.cancel = { [weak delegate] in delegate?.close(choosing: false) }
        popover.delegate = delegate
        valuePicker = delegate

        let field = rect(node)
        let below = bounds.maxY - field.maxY
        // This view is flipped: maxY is the field's lower edge.
        let edge: NSRectEdge = below >= content.frame.height + 12 || below >= field.minY ? .maxY : .minY
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window != nil, self.valuePicker === delegate else { return }
            popover.show(relativeTo: field, of: self, preferredEdge: edge)
            // Keys go to the calendar: arrows move the day, Return and Escape reach the content.
            picker.window?.makeFirstResponder(picker)
        }
    }
}

/// The date picker keeps Return for itself; here it commits the shown value.
private final class GpuDatePicker: NSDatePicker {
    var commit: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36 || event.keyCode == 76, let commit {
            return commit()
        }
        super.keyDown(with: event)
    }
}

/// The picker popover's content: Return commits, Escape cancels.
private final class GpuValuePickerView: NSView {
    var commit: (() -> Void)?
    var cancel: (() -> Void)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.keyCode == 36 || event.keyCode == 76 {
            commit?()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    override func insertNewline(_ sender: Any?) { commit?() }

    override func cancelOperation(_ sender: Any?) { cancel?() }
}

/// Menu items' target, alive while the menu is.
@MainActor
private final class GpuPickerTarget: NSObject {
    let chosen: (Int) -> Void
    init(_ chosen: @escaping (Int) -> Void) { self.chosen = chosen }
    @objc func choose(_ item: NSMenuItem) { chosen(item.tag) }
}

/// Reports a value picker's outcome once. Closing it any way but Escape keeps the shown
/// value, as clicking away from a field keeps what was picked.
@MainActor
final class GpuValuePickerDelegate: NSObject, NSPopoverDelegate {
    /// Held so editing that ends elsewhere can still close it.
    private let popover: NSPopover
    private var outcome: ((Bool) -> Void)?
    private var choosing = true
    init(popover: NSPopover, outcome: @escaping (Bool) -> Void) {
        self.popover = popover
        self.outcome = outcome
    }
    func close(choosing: Bool) {
        self.choosing = choosing
        popover.close()
    }
    /// Editing ended elsewhere: the core already knows, nothing is sent.
    func dismiss() {
        outcome = nil
        popover.close()
    }
    nonisolated func popoverDidClose(_ notification: Notification) {
        MainActor.assumeIsolated {
            let outcome = self.outcome
            self.outcome = nil
            outcome?(choosing)
        }
    }
}
#endif
