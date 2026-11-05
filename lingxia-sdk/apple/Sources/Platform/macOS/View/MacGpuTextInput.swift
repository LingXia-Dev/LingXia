import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// AppKit text input for a GPU page's focused editor; the text lives in the core.
extension GpuPageView: GpuImeHost, @preconcurrency NSTextInputClient {
    // MARK: GpuImeHost

    func imeRequest(_ message: GpuFrameInputRequestMessage, reply: @escaping (GpuFrameInputReply?) -> Void) {
        guard let data = try? JSONEncoder().encode(message), let request = String(data: data, encoding: .utf8)
        else { return reply(nil) }
        nextInputReply += 1
        inputReplies[nextInputReply] = reply
        gpuPageInput(token, request, nextInputReply)
    }

    func inputReply(_ id: UInt64, value: String?) {
        guard let reply = inputReplies.removeValue(forKey: id) else { return }
        reply(value.flatMap { try? JSONDecoder().decode(GpuFrameInputReply.self, from: Data($0.utf8)) })
    }

    func imeRender(rebuild: Bool) { gpuPageRender(token, rebuild) }

    func imeNode(_ identity: String) -> GpuFrameNode? { Self.decodeNode(gpuPageNode(token, identity).toString()) }

    static func decodeNode(_ json: String) -> GpuFrameNode? {
        json.isEmpty ? nil : try? JSONDecoder().decode(GpuFrameNode.self, from: Data(json.utf8))
    }

    func imeConnected(_ editor: GpuEditorMirror) {
        inputContext?.discardMarkedText()
        if window?.firstResponder !== self { window?.makeFirstResponder(self) }
    }

    func imeDisconnected(hide: Bool) {
        inputContext?.discardMarkedText()
    }

    func imeInvalidated() { inputContext?.discardMarkedText() }

    func imeHideKeyboard() {}

    func imeCaret(eligible: Bool, editor: GpuEditorMirror?) {
        let shown = eligible && window?.isKeyWindow == true
        guard let editor else { return gpuPageCaret(token, shown, false, "", 0, 0, -1, -1) }
        gpuPageCaret(token, shown, true, editor.text, Int32(editor.anchor), Int32(editor.focus),
                     Int32(editor.composeStart), Int32(editor.composeEnd))
        inputContext?.invalidateCharacterCoordinates()
    }

    // MARK: Keys while an editor has focus

    /// True when the focused editor took the key.
    func editorKeyDown(_ event: NSEvent) -> Bool {
        guard let editor = ime.editor else { return false }
        // Tab leaves the editor for focus traversal.
        if event.keyCode == 48 {
            ime.clear(blur: true)
            return false
        }
        let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
        if flags.contains(.command), let key = event.charactersIgnoringModifiers?.lowercased() {
            switch key {
            case "a": return editor.selectAll()
            case "c": return copyEditor(cut: false)
            case "x": return copyEditor(cut: true)
            case "v": return pasteEditor()
            case "z": return editor.history(undo: !flags.contains(.shift))
            default: return false
            }
        }
        return inputContext?.handleEvent(event) ?? false
    }

    private func copyEditor(cut: Bool) -> Bool {
        guard let editor = ime.editor, !editor.secure, editor.anchor != editor.focus else { return false }
        let selected = editor.substring(editor.selection)
        let copy = {
            NSPasteboard.general.clearContents()
            return NSPasteboard.general.setString(selected, forType: .string)
        }
        return cut ? editor.replace("", composing: false, key: true, before: copy) : copy()
    }

    private func pasteEditor() -> Bool {
        guard let editor = ime.editor, let value = NSPasteboard.general.string(forType: .string),
              GpuEditorMirror.valid(value) else { return false }
        return editor.replace(value, composing: false, key: true)
    }

    /// Copy from the focused editor, or else the page's static-text selection.
    @objc func copy(_ sender: Any?) {
        if ime.editor == nil { return gpuPageCopySelection(token) }
        _ = copyEditor(cut: false)
    }
    @objc func cut(_ sender: Any?) { _ = copyEditor(cut: true) }
    @objc func paste(_ sender: Any?) { _ = pasteEditor() }
    override func selectAll(_ sender: Any?) { _ = ime.editor?.selectAll() }

    // MARK: NSTextInputClient

    private static func string(_ value: Any) -> String {
        (value as? NSAttributedString)?.string ?? (value as? String) ?? ""
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        guard let editor = ime.editor, editor.beginBatch() else { return }
        defer { editor.endBatch() }
        if replacementRange.location != NSNotFound, editor.marked == nil {
            editor.setSelection(replacementRange.location, replacementRange.location + replacementRange.length)
        }
        editor.replace(Self.string(string), composing: false)
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        guard let editor = ime.editor, editor.beginBatch() else { return }
        defer { editor.endBatch() }
        if replacementRange.location != NSNotFound, editor.marked == nil {
            editor.setSelection(replacementRange.location, replacementRange.location + replacementRange.length)
        }
        let text = Self.string(string)
        let start = editor.marked?.location ?? editor.selection.location
        guard editor.replace(text, composing: true) else { return }
        // The caret inside the marked text: the replace put it past the end.
        let length = (text as NSString).length
        guard selectedRange.location != NSNotFound, selectedRange.location < length || selectedRange.length > 0
        else { return }
        let caret = start + min(selectedRange.location, length)
        editor.setSelection(caret, min(start + length, caret + selectedRange.length))
    }

    func unmarkText() { ime.editor?.finishComposition() }

    func selectedRange() -> NSRange { ime.editor?.selection ?? NSRange(location: NSNotFound, length: 0) }

    func markedRange() -> NSRange { ime.editor?.marked ?? NSRange(location: NSNotFound, length: 0) }

    func hasMarkedText() -> Bool { ime.editor?.marked != nil }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard let editor = ime.editor, !editor.secure else { return nil }
        let clamped = NSIntersectionRange(range, NSRange(location: 0, length: editor.length))
        actualRange?.pointee = clamped
        return NSAttributedString(string: editor.substring(clamped))
    }

    /// Clause segments and underlines tell input methods the page shows marked text
    /// inline; without them some compose in a window of their own.
    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        [.underlineStyle, .underlineColor, .markedClauseSegment]
    }

    /// The candidate window sits under the caret the core drew.
    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        actualRange?.pointee = range
        guard let caret = ime.node?.input?.geometry?.caret, caret.count == 4, let window else { return .zero }
        let local = NSRect(x: CGFloat(caret[0]), y: CGFloat(caret[1]), width: CGFloat(caret[2]), height: CGFloat(caret[3]))
        return window.convertToScreen(convert(local, to: nil))
    }

    func characterIndex(for point: NSPoint) -> Int { NSNotFound }

    override func doCommand(by selector: Selector) {
        guard let editor = ime.editor else { return }
        let length = editor.length
        let shift = NSApp.currentEvent?.modifierFlags.contains(.shift) == true
        switch selector {
        case #selector(deleteBackward(_:)), #selector(deleteBackwardByDecomposingPreviousCharacter(_:)):
            if let range = editor.deletionRange(forward: false) { editor.delete(range) }
        case #selector(deleteForward(_:)):
            if let range = editor.deletionRange(forward: true) { editor.delete(range) }
        case #selector(insertNewline(_:)), #selector(insertLineBreak(_:)):
            if editor.returnInsertsNewline || (editor.multiline && shift) {
                editor.replace("\n", composing: false, key: true)
            } else if editor.multiline, editor.marked != nil {
                editor.finishComposition()
            } else {
                editor.submit()
            }
        case #selector(cancelOperation(_:)):
            ime.clear(blur: true)
        case #selector(moveLeft(_:)), #selector(moveRight(_:)):
            let left = selector == #selector(moveLeft(_:))
            let range = editor.selection
            let caret: Int
            if range.length > 0 {
                caret = left ? range.location : range.location + range.length
            } else if left {
                caret = editor.deletionRange(forward: false)?.location ?? 0
            } else {
                caret = editor.deletionRange(forward: true).map { $0.location + $0.length } ?? length
            }
            editor.select(caret, caret)
        case #selector(moveLeftAndModifySelection(_:)), #selector(moveRightAndModifySelection(_:)):
            let ns = editor.text as NSString
            let left = selector == #selector(moveLeftAndModifySelection(_:))
            var focus = editor.focus
            if left, focus > 0 { focus = ns.rangeOfComposedCharacterSequence(at: focus - 1).location }
            if !left, focus < length {
                let range = ns.rangeOfComposedCharacterSequence(at: focus)
                focus = range.location + range.length
            }
            editor.select(editor.anchor, focus)
        case #selector(moveToBeginningOfLine(_:)), #selector(moveToBeginningOfDocument(_:)),
             #selector(moveToBeginningOfParagraph(_:)), #selector(moveUp(_:)):
            editor.select(0, 0)
        case #selector(moveToEndOfLine(_:)), #selector(moveToEndOfDocument(_:)),
             #selector(moveToEndOfParagraph(_:)), #selector(moveDown(_:)):
            editor.select(length, length)
        case #selector(moveToBeginningOfLineAndModifySelection(_:)), #selector(moveToBeginningOfDocumentAndModifySelection(_:)),
             #selector(moveUpAndModifySelection(_:)):
            editor.select(editor.anchor, 0)
        case #selector(moveToEndOfLineAndModifySelection(_:)), #selector(moveToEndOfDocumentAndModifySelection(_:)),
             #selector(moveDownAndModifySelection(_:)):
            editor.select(editor.anchor, length)
        case #selector(selectAll(_:)):
            _ = editor.selectAll()
        default:
            break
        }
    }
}
#endif
