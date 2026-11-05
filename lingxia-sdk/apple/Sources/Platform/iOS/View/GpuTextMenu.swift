import Foundation
#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// The text menu the core asks for, as UIKit's edit menu at its anchor.
extension GpuPageView: @preconcurrency UIEditMenuInteractionDelegate {
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
        let interaction = editMenu ?? {
            let made = UIEditMenuInteraction(delegate: self)
            addInteraction(made)
            editMenu = made
            return made
        }()
        guard !json.isEmpty,
              let menu = try? JSONDecoder().decode(GpuFrameTextMenu.self, from: Data(json.utf8)),
              menu.anchor.count == 4
        else {
            textMenu = nil
            interaction.dismissMenu()
            return
        }
        textMenu = menu
        menuChosen = false
        let anchor = CGPoint(x: CGFloat(menu.anchor[0] + menu.anchor[2] / 2), y: CGFloat(menu.anchor[1]))
        interaction.presentEditMenu(with: UIEditMenuConfiguration(identifier: nil, sourcePoint: anchor))
    }

    func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration,
                             suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard let menu = textMenu else { return nil }
        // An editor's items run in its input connection; static text's in the core.
        let editor = ime.editor.flatMap { $0.identity == menu.target.identity ? $0 : nil }
        let actions = menu.actions.compactMap(GpuFrameMenuAction.init(rawValue:)).map { action in
            UIAction(title: Self.title(action)) { [weak self] _ in
                guard let self else { return }
                self.menuChosen = true
                if editor != nil { self.runEditorAction(action) } else { _ = gpuPageTextMenuAction(self.token, Self.code(action)) }
            }
        }
        return UIMenu(children: actions)
    }

    func editMenuInteraction(_ interaction: UIEditMenuInteraction, targetRectFor configuration: UIEditMenuConfiguration) -> CGRect {
        guard let anchor = textMenu?.anchor, anchor.count == 4 else { return .zero }
        return CGRect(x: CGFloat(anchor[0]), y: CGFloat(anchor[1]), width: CGFloat(anchor[2]), height: CGFloat(anchor[3]))
    }

    func editMenuInteraction(_ interaction: UIEditMenuInteraction, willDismissMenuFor configuration: UIEditMenuConfiguration,
                             animator: UIEditMenuInteractionAnimating) {
        guard textMenu != nil else { return }
        textMenu = nil
        if !menuChosen { gpuPageTextMenuDismissed(token) }
    }

    private func runEditorAction(_ action: GpuFrameMenuAction) {
        guard let editor = ime.editor else { return }
        switch action {
        case .cut, .copy:
            guard !editor.secure, editor.anchor != editor.focus else { return }
            let selected = editor.substring(editor.selection)
            if action == .copy {
                UIPasteboard.general.string = selected
            } else {
                editor.replace("", composing: false, key: true) {
                    UIPasteboard.general.string = selected
                    return true
                }
            }
        case .paste:
            if let value = UIPasteboard.general.string, GpuEditorMirror.valid(value) {
                editor.replace(value, composing: false, key: true)
            }
        case .selectAll: _ = editor.selectAll()
        case .undo: editor.history(undo: true)
        case .redo: editor.history(undo: false)
        }
    }
}
#endif
