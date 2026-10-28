import Foundation
#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// A UTF-16 offset into the connected editor's text.
final class GpuTextPosition: UITextPosition {
    let offset: Int
    init(_ offset: Int) { self.offset = offset }
}

final class GpuTextRange: UITextRange {
    let range: NSRange
    init(_ range: NSRange) { self.range = range }
    override var start: UITextPosition { GpuTextPosition(range.location) }
    override var end: UITextPosition { GpuTextPosition(range.location + range.length) }
    override var isEmpty: Bool { range.length == 0 }
}

/// UIKit text input for a GPU page's focused editor; the text lives in the core.
extension GpuPageView: GpuImeHost, UITextInput {
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
        inputDelegate?.textWillChange(self)
        inputDelegate?.textDidChange(self)
        // A picked value takes no keyboard.
        if ime.ime?.picker != nil {
            if isFirstResponder { super.resignFirstResponder() }
            if valuePicker == nil { showValuePicker() }
            return
        }
        if isFirstResponder {
            reloadInputViews()
        } else {
            becomeFirstResponder()
        }
    }

    func imeDisconnected(hide: Bool) {
        inputDelegate?.textWillChange(self)
        inputDelegate?.textDidChange(self)
        let picker = valuePicker
        valuePicker = nil
        picker?.close()
        if hide, isFirstResponder { resignFirstResponder() }
    }

    /// The core changed the text behind the keyboard's back: it re-reads everything.
    func imeInvalidated() {
        inputDelegate?.textWillChange(self)
        inputDelegate?.textDidChange(self)
    }

    func imeHideKeyboard() {
        if isFirstResponder { resignFirstResponder() }
    }

    func imeCaret(eligible: Bool, editor: GpuEditorMirror?) {
        let shown = eligible && UIApplication.shared.applicationState == .active
        guard let editor else { return gpuPageCaret(token, shown, false, "", 0, 0, -1, -1) }
        gpuPageCaret(token, shown, true, editor.text, Int32(editor.anchor), Int32(editor.focus),
                     Int32(editor.composeStart), Int32(editor.composeEnd))
    }

    // MARK: Responder

    /// An editor takes the keyboard; with a hardware keyboard the page takes key presses too.
    override var canBecomeFirstResponder: Bool { ime.editor != nil || Self.hardwareKeyboard }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let rest = pressesToPage(presses)
        if !rest.isEmpty { super.pressesBegan(rest, with: event) }
    }

    @discardableResult
    override func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()
        if resigned, ime.isActive { ime.clear(blur: true) }
        return resigned
    }

    // MARK: UITextInputTraits, from the core's keyboard configuration

    var keyboardType: UIKeyboardType {
        get {
            switch ime.ime.flatMap({ GpuFrameKeyboard(rawValue: $0.keyboard) }) {
            case .number: .decimalPad
            case .phone: .phonePad
            case .email: .emailAddress
            case .search: .webSearch
            default: .default
            }
        }
        set {}
    }

    var returnKeyType: UIReturnKeyType {
        get {
            switch ime.ime.flatMap({ GpuFrameReturnKey(rawValue: $0.returnKey) }) {
            case .done: .done
            case .go: .go
            case .next: .next
            case .search: .search
            case .send: .send
            default: .default
            }
        }
        set {}
    }

    var autocapitalizationType: UITextAutocapitalizationType {
        get {
            switch ime.ime.flatMap({ GpuFrameCapitalize(rawValue: $0.capitalize) }) {
            case .sentences: .sentences
            case .words: .words
            case .characters: .allCharacters
            default: .none
            }
        }
        set {}
    }

    var autocorrectionType: UITextAutocorrectionType {
        get { ime.ime?.suggestions == false ? .no : .default }
        set {}
    }

    var spellCheckingType: UITextSpellCheckingType {
        get { ime.ime?.suggestions == false ? .no : .default }
        set {}
    }

    var isSecureTextEntry: Bool {
        get { ime.ime?.secure == true }
        set {}
    }

    // MARK: UIKeyInput

    var hasText: Bool { (ime.editor?.length ?? 0) > 0 }

    func insertText(_ text: String) {
        guard let editor = ime.editor else { return }
        // UIKit sends Return as a newline.
        if text == "\n" {
            if editor.returnInsertsNewline {
                editor.replace("\n", composing: false, key: true)
            } else if editor.multiline, editor.marked != nil {
                editor.finishComposition()
            } else {
                editor.submit()
            }
            return
        }
        editor.replace(text, composing: false)
    }

    func deleteBackward() {
        guard let editor = ime.editor else { return }
        if editor.marked != nil {
            editor.replace("", composing: false)
        } else if let range = editor.deletionRange(forward: false) {
            editor.delete(range)
        }
    }

    // MARK: UITextInput

    private func range(_ range: UITextRange) -> NSRange? { (range as? GpuTextRange)?.range }
    private func offset(_ position: UITextPosition) -> Int? { (position as? GpuTextPosition)?.offset }

    func text(in range: UITextRange) -> String? {
        guard let editor = ime.editor, let range = self.range(range) else { return nil }
        return editor.substring(range)
    }

    func replace(_ range: UITextRange, withText text: String) {
        guard let editor = ime.editor, let range = self.range(range), editor.beginBatch() else { return }
        defer { editor.endBatch() }
        editor.select(range.location, range.location + range.length)
        editor.replace(text, composing: false, key: true)
    }

    var selectedTextRange: UITextRange? {
        get { ime.editor.map { GpuTextRange($0.selection) } }
        set {
            guard let editor = ime.editor, let newValue, let range = self.range(newValue) else { return }
            editor.setSelection(range.location, range.location + range.length)
        }
    }

    var markedTextRange: UITextRange? { ime.editor?.marked.map(GpuTextRange.init) }

    var markedTextStyle: [NSAttributedString.Key: Any]? {
        get { nil }
        set {}
    }

    func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
        guard let editor = ime.editor, editor.beginBatch() else { return }
        defer { editor.endBatch() }
        let text = markedText ?? ""
        let start = editor.marked?.location ?? editor.selection.location
        guard editor.replace(text, composing: true) else { return }
        let length = (text as NSString).length
        guard selectedRange.location != NSNotFound, selectedRange.location < length || selectedRange.length > 0
        else { return }
        let caret = start + min(selectedRange.location, length)
        editor.setSelection(caret, min(start + length, caret + selectedRange.length))
    }

    func unmarkText() { ime.editor?.finishComposition() }

    var beginningOfDocument: UITextPosition { GpuTextPosition(0) }
    var endOfDocument: UITextPosition { GpuTextPosition(ime.editor?.length ?? 0) }

    func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
        guard let a = offset(fromPosition), let b = offset(toPosition) else { return nil }
        return GpuTextRange(NSRange(location: min(a, b), length: abs(a - b)))
    }

    func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
        guard let start = self.offset(position) else { return nil }
        let next = start + offset
        guard next >= 0, next <= (ime.editor?.length ?? 0) else { return nil }
        return GpuTextPosition(next)
    }

    func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int) -> UITextPosition? {
        switch direction {
        case .left, .up: return self.position(from: position, offset: -offset)
        default: return self.position(from: position, offset: offset)
        }
    }

    func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
        guard let a = offset(position), let b = offset(other) else { return .orderedSame }
        return a < b ? .orderedAscending : a > b ? .orderedDescending : .orderedSame
    }

    func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
        (offset(toPosition) ?? 0) - (offset(from) ?? 0)
    }

    var tokenizer: UITextInputTokenizer {
        if let existing = textTokenizer { return existing }
        let made = UITextInputStringTokenizer(textInput: self)
        textTokenizer = made
        return made
    }

    func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
        switch direction {
        case .left, .up: return range.start
        default: return range.end
        }
    }

    func characterRange(byExtending position: UITextPosition, in direction: UITextLayoutDirection) -> UITextRange? {
        guard let at = offset(position) else { return nil }
        switch direction {
        case .left, .up: return GpuTextRange(NSRange(location: 0, length: at))
        default: return GpuTextRange(NSRange(location: at, length: (ime.editor?.length ?? at) - at))
        }
    }

    func baseWritingDirection(for position: UITextPosition, in direction: UITextStorageDirection) -> NSWritingDirection {
        .natural
    }

    func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {}

    /// The caret the core drew, in this view's points.
    private var caretRect: CGRect {
        guard let caret = ime.node?.input?.geometry?.caret, caret.count == 4 else { return .zero }
        return CGRect(x: CGFloat(caret[0]), y: CGFloat(caret[1]), width: CGFloat(caret[2]), height: CGFloat(caret[3]))
    }

    func firstRect(for range: UITextRange) -> CGRect { caretRect }

    func caretRect(for position: UITextPosition) -> CGRect { caretRect }

    func selectionRects(for range: UITextRange) -> [UITextSelectionRect] { [] }

    func closestPosition(to point: CGPoint) -> UITextPosition? { ime.editor.map { GpuTextPosition($0.focus) } }

    func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? { range.end }

    func characterRange(at point: CGPoint) -> UITextRange? { nil }
}
#endif
