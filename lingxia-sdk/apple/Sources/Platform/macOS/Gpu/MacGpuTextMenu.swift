import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// The text menu the core asks for, as an AppKit context menu at its anchor.
extension GpuPageView {
    private static func title(_ action: GpuFrameMenuAction) -> String {
        switch action {
        case .cut: NSLocalizedString("Cut", comment: "")
        case .copy: NSLocalizedString("Copy", comment: "")
        case .paste: NSLocalizedString("Paste", comment: "")
        case .selectAll: NSLocalizedString("Select All", comment: "")
        case .undo: NSLocalizedString("Undo", comment: "")
        case .redo: NSLocalizedString("Redo", comment: "")
        }
    }

    private static func code(_ action: GpuFrameMenuAction) -> UInt8 {
        switch action {
        case .cut: 0
        case .copy: 1
        case .paste: 2
        case .selectAll: 3
        case .undo: 4
        case .redo: 5
        }
    }

    func showTextMenu(_ json: String) {
        guard !json.isEmpty,
              let menu = try? JSONDecoder().decode(GpuFrameTextMenu.self, from: Data(json.utf8)),
              menu.anchor.count == 4
        else {
            textMenu?.cancelTracking()
            return
        }
        // An editor's items run in its input connection; static text's in the core.
        let editor = ime.editor.flatMap { $0.identity == menu.target.identity ? $0 : nil }
        let native = NSMenu()
        native.autoenablesItems = false
        let target = GpuMenuTarget { [weak self] action in
            guard let self else { return }
            if editor != nil { self.runEditorAction(action) } else { _ = gpuPageTextMenuAction(self.token, Self.code(action)) }
        }
        for action in menu.actions.compactMap(GpuFrameMenuAction.init(rawValue:)) {
            let item = NSMenuItem(title: Self.title(action), action: #selector(GpuMenuTarget.choose(_:)), keyEquivalent: "")
            item.target = target
            item.representedObject = action.rawValue
            native.addItem(item)
        }
        guard !native.items.isEmpty else { return }
        let anchor = NSPoint(x: CGFloat(menu.anchor[0]), y: CGFloat(menu.anchor[1] + menu.anchor[3]))
        textMenu?.cancelTracking()
        textMenu = native
        // Pop up after the call returns: the menu runs its own tracking loop.
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window != nil, self.textMenu === native else { return }
            withExtendedLifetime(target) {
                native.popUp(positioning: nil, at: anchor, in: self)
            }
            if self.textMenu === native { self.textMenu = nil }
            if !target.chose { gpuPageTextMenuDismissed(self.token) }
        }
    }

    private func runEditorAction(_ action: GpuFrameMenuAction) {
        switch action {
        case .cut: cut(nil)
        case .copy: copy(nil)
        case .paste: paste(nil)
        case .selectAll: selectAll(nil)
        case .undo: _ = ime.editor?.history(undo: true)
        case .redo: _ = ime.editor?.history(undo: false)
        }
    }
}

/// Menu items' target, alive while the menu is.
@MainActor
private final class GpuMenuTarget: NSObject {
    let chosen: (GpuFrameMenuAction) -> Void
    private(set) var chose = false
    init(_ chosen: @escaping (GpuFrameMenuAction) -> Void) { self.chosen = chosen }
    @objc func choose(_ item: NSMenuItem) {
        guard let raw = item.representedObject as? String, let action = GpuFrameMenuAction(rawValue: raw) else { return }
        chose = true
        chosen(action)
    }
}
#endif
