#if os(macOS)
import AppKit

/// An NSMenuItem that runs a closure when chosen. NSMenuItem does not retain
/// its target, so the item is its own target: the closure lives as long as
/// the item does. Used by shell menus that are rebuilt on every open.
@MainActor
final class MenuActionItem: NSMenuItem {
    private let handler: () -> Void

    init(title: String, keyEquivalent: String = "", handler: @escaping () -> Void) {
        self.handler = handler
        super.init(title: title, action: #selector(fire(_:)), keyEquivalent: keyEquivalent)
        target = self
    }

    required init(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @objc private func fire(_ sender: Any?) { handler() }
}
#endif
