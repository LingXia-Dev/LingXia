import Foundation

/// The platform half of a GPU page's input method: requests to the core, and what the
/// view must do when an editor connects, changes or goes away.
@MainActor
protocol GpuImeHost: AnyObject {
    func imeRequest(_ message: GpuFrameInputRequestMessage, reply: @escaping (GpuFrameInputReply?) -> Void)
    /// Present again: `rebuild` reruns the View, otherwise only redraws.
    func imeRender(rebuild: Bool)
    /// The presented node `identity`, for the editor a NEXT/PREVIOUS action moved to.
    func imeNode(_ identity: String) -> GpuFrameNode?
    /// An editor connected: take keyboard focus.
    func imeConnected(_ editor: GpuEditorMirror)
    /// The connected editor went away; `hide` when the keyboard should go with it.
    func imeDisconnected(hide: Bool)
    /// The editor's text or selection changed outside the platform's own edits.
    func imeInvalidated()
    func imeCaret(eligible: Bool, editor: GpuEditorMirror?)
    /// A submit asked for the on-screen keyboard to go; the editor stays focused.
    func imeHideKeyboard()
}

/// The main-thread mirror of a connected editor. The core's editor stays authoritative:
/// edits apply here at once, go to the core in batches, and its acknowledged state wins.
@MainActor
final class GpuEditorMirror {
    typealias Edit = GpuFrameInputEditMessage
    private static let maxBytes = GpuFrameInputLimits.editorTextBytes
    private static let maxPendingBytes = GpuFrameInputLimits.inputRequestBytes
    private static let maxEdits = GpuFrameInputLimits.inputBatchEdits
    /// A request's envelope per edit, beyond the edit itself.
    private static let editOverhead = 64

    let identity: String
    let token: UInt64
    private let send: (UInt64, [Edit], Bool) -> Void
    private let notify: () -> Void
    private(set) var text = ""
    private(set) var anchor = 0
    private(set) var focus = 0
    private(set) var composeStart = -1
    private(set) var composeEnd = -1
    private var depth = 0
    private var queuedBytes = 0
    private var pendingBytes = 0
    private(set) var closed = false
    private var submitting = false
    private var historyPending = false
    private var undoable = false
    private var redoable = false
    private var queued: [Edit] = []
    /// Sent revisions not yet acknowledged, with their byte cost.
    private var pending: [(revision: UInt64, bytes: Int)] = []
    private(set) var revision: UInt64 = 0
    var multiline = false
    var secure = false
    var returnInsertsNewline = false
    /// `maxLength` in UTF-16 units like the core, or -1; inserts are cut to fit.
    var maxLength = -1

    init?(identity: String, token: UInt64, state: GpuFrameEditorState,
          send: @escaping (UInt64, [Edit], Bool) -> Void, changed: @escaping () -> Void) {
        self.identity = identity
        self.token = token
        self.send = send
        self.notify = changed
        guard install(state) else { return nil }
    }

    private var utf16: NSString { text as NSString }
    var length: Int { utf16.length }
    var selection: NSRange { NSRange(location: min(anchor, focus), length: abs(anchor - focus)) }
    var marked: NSRange? {
        composeStart >= 0 ? NSRange(location: composeStart, length: composeEnd - composeStart) : nil
    }
    func substring(_ range: NSRange) -> String {
        let clamped = NSIntersectionRange(range, NSRange(location: 0, length: length))
        return utf16.substring(with: clamped)
    }

    @discardableResult
    private func install(_ state: GpuFrameEditorState) -> Bool {
        let value = state.text
        let ns = value as NSString
        let anchor = state.selection.count >= 2 ? Int(state.selection[0]) : 0
        let focus = state.selection.count >= 2 ? Int(state.selection[1]) : 0
        let composing = (state.composition?.count ?? 0) >= 2
        let start = composing ? Int(state.composition![0]) : -1
        let end = composing ? Int(state.composition![1]) : -1
        guard Self.valid(value), Self.boundary(ns, anchor), Self.boundary(ns, focus),
              start == -1 || (Self.boundary(ns, start) && Self.boundary(ns, end) && start < end)
        else { return false }
        text = value
        self.anchor = anchor
        self.focus = focus
        composeStart = start
        composeEnd = end
        undoable = state.canUndo
        redoable = state.canRedo
        return true
    }

    var settled: Bool { !closed && depth == 0 && queued.isEmpty && pending.isEmpty && !submitting }
    var canUndo: Bool { settled && undoable }
    var canRedo: Bool { settled && redoable }

    /// The core answered revision `acknowledged`; true when the platform must resync
    /// its view of the text (after undo or redo).
    func acknowledge(_ acknowledged: UInt64, state: GpuFrameEditorState?) -> Bool {
        guard !closed, pending.contains(where: { $0.revision == acknowledged }) else { return false }
        for entry in pending where entry.revision <= acknowledged { pendingBytes -= entry.bytes }
        pending.removeAll { $0.revision <= acknowledged }
        if acknowledged == revision, queued.isEmpty, let state, install(state) {
            let resync = historyPending
            historyPending = false
            notify()
            return resync
        }
        return false
    }

    /// A presented frame's state, taken only when nothing was edited since `captured`.
    func acceptFrame(_ captured: UInt64, state: GpuFrameEditorState?) {
        if !closed, captured == revision, pending.isEmpty, queued.isEmpty, let state, install(state) {
            notify()
        }
    }

    func close() {
        closed = true
        queued = []
        pending = []
        pendingBytes = 0
        queuedBytes = 0
        depth = 0
        historyPending = false
    }

    func beginBatch() -> Bool {
        if closed || historyPending || depth >= Self.maxEdits { return false }
        depth += 1
        return true
    }

    func endBatch() {
        guard !closed, depth > 0 else { return }
        depth -= 1
        if depth == 0 { flush() }
    }

    private func reserve(_ edits: [Edit]) -> Bool {
        if closed || historyPending || submitting || queued.count + edits.count > Self.maxEdits
            || pending.count >= Self.maxEdits { return false }
        let encoder = JSONEncoder()
        var bytes = 0
        for edit in edits {
            guard let data = try? encoder.encode(edit) else { return false }
            bytes += data.count + Self.editOverhead
        }
        if pendingBytes + queuedBytes + bytes > Self.maxPendingBytes { return false }
        queued.append(contentsOf: edits)
        queuedBytes += bytes
        return true
    }

    private func changed() {
        if depth == 0 { flush() }
    }

    private func flush() {
        if queued.isEmpty && !submitting { return }
        let edits = queued
        let submit = submitting
        let bytes = queuedBytes
        queued = []
        queuedBytes = 0
        submitting = false
        revision += 1
        pending.append((revision, bytes))
        pendingBytes += bytes
        send(revision, edits, submit)
        notify()
    }

    static func boundary(_ value: NSString, _ offset: Int) -> Bool {
        guard offset >= 0, offset <= value.length else { return false }
        guard offset > 0, offset < value.length else { return true }
        return !(UTF16.isLeadSurrogate(value.character(at: offset - 1))
            && UTF16.isTrailSurrogate(value.character(at: offset)))
    }

    static func valid(_ value: String) -> Bool { value.utf8.count <= maxBytes }

    /// Replace the composition (or the selection, for `key` input) with `value`; the caret
    /// lands as Android's `commitText` cursor says: past the text for 1.
    @discardableResult
    func replace(_ value: String, cursor: Int = 1, composing: Bool, key: Bool = false,
                 before: (() -> Bool)? = nil) -> Bool {
        guard !closed, Self.valid(value) else { return false }
        let start = composeStart >= 0 && !key ? composeStart : min(anchor, focus)
        let end = composeEnd >= 0 && !key ? composeEnd : max(anchor, focus)
        let insert = fit(value, start, end)
        let inserted = (insert as NSString).length
        let next = utf16.replacingCharacters(in: NSRange(location: start, length: end - start), with: insert)
        let position = cursor > 0 ? start + inserted + cursor - 1 : start + cursor
        let caret = max(0, min((next as NSString).length, position))
        let replacement = Edit.replace(text: insert, cursor: Int64(cursor), composing: composing)
        let edits = key && composeStart >= 0 ? [Edit.finishComposition, replacement] : [replacement]
        let (count, bytes) = (queued.count, queuedBytes)
        guard Self.valid(next), Self.boundary(next as NSString, caret), reserve(edits) else { return false }
        if let before, !before() {
            queued.removeLast(queued.count - count)
            queuedBytes = bytes
            return false
        }
        text = next
        anchor = caret
        focus = caret
        composeStart = composing && inserted > 0 ? start : -1
        composeEnd = composeStart >= 0 ? start + inserted : -1
        changed()
        return true
    }

    private func fit(_ insert: String, _ start: Int, _ end: Int) -> String {
        guard maxLength >= 0 else { return insert }
        var room = maxLength - (start + length - end)
        if room <= 0 { return "" }
        let ns = insert as NSString
        if ns.length <= room { return insert }
        // Never split a surrogate pair.
        if UTF16.isLeadSurrogate(ns.character(at: room - 1)) { room -= 1 }
        return ns.substring(to: room)
    }

    func replaceAll(_ value: String) -> Bool {
        let value = fit(value, 0, length)
        guard Self.valid(value), reserve([
            .finishComposition, .selection(anchor: 0, focus: UInt32(length)),
            .replace(text: value, cursor: 1, composing: false),
        ]) else { return false }
        text = value
        anchor = (value as NSString).length
        focus = anchor
        composeStart = -1
        composeEnd = -1
        changed()
        return true
    }

    @discardableResult
    func setSelection(_ start: Int, _ end: Int) -> Bool {
        guard Self.boundary(utf16, start), Self.boundary(utf16, end),
              reserve([.selection(anchor: UInt32(start), focus: UInt32(end))]) else { return false }
        anchor = start
        focus = end
        changed()
        return true
    }

    /// A selection from a pointer or an assistive action: any composition ends first.
    @discardableResult
    func select(_ start: Int, _ end: Int) -> Bool {
        guard Self.boundary(utf16, start), Self.boundary(utf16, end) else { return false }
        let selection = Edit.selection(anchor: UInt32(start), focus: UInt32(end))
        guard reserve(composeStart >= 0 ? [.finishComposition, selection] : [selection]) else { return false }
        composeStart = -1
        composeEnd = -1
        anchor = start
        focus = end
        changed()
        return true
    }

    @discardableResult
    func finishComposition() -> Bool {
        guard reserve([.finishComposition]) else { return false }
        composeStart = -1
        composeEnd = -1
        changed()
        return true
    }

    /// Delete `range` as a key would, ending any composition.
    @discardableResult
    func delete(_ range: NSRange) -> Bool {
        let (start, end) = (range.location, range.location + range.length)
        guard range.length > 0, Self.boundary(utf16, start), Self.boundary(utf16, end) else { return false }
        let edits: [Edit] = [.selection(anchor: UInt32(start), focus: UInt32(end)),
                             .replace(text: "", cursor: 1, composing: false)]
        guard reserve(composeStart >= 0 ? [.finishComposition] + edits : edits) else { return false }
        text = utf16.replacingCharacters(in: range, with: "")
        anchor = start
        focus = start
        composeStart = -1
        composeEnd = -1
        changed()
        return true
    }

    /// The composed character before (`forward` false) or after the caret, or the selection.
    func deletionRange(forward: Bool) -> NSRange? {
        if anchor != focus { return selection }
        if forward {
            guard focus < length else { return nil }
            return utf16.rangeOfComposedCharacterSequence(at: focus)
        }
        guard focus > 0 else { return nil }
        return utf16.rangeOfComposedCharacterSequence(at: focus - 1)
    }

    @discardableResult
    func submit() -> Bool {
        if closed || historyPending || submitting || queued.count >= Self.maxEdits
            || pending.count >= Self.maxEdits { return false }
        submitting = true
        composeStart = -1
        composeEnd = -1
        changed()
        return true
    }

    /// The mirror keeps no history: undo and redo wait for the core's state.
    @discardableResult
    func history(undo: Bool) -> Bool {
        guard !closed, !historyPending, undo ? canUndo : canRedo, reserve([undo ? .undo : .redo])
        else { return false }
        historyPending = true
        changed()
        return true
    }

    func selectAll() -> Bool { select(0, length) }
}

/// Focus and editor lifecycle for one GPU page, shared by AppKit and UIKit. Every
/// request goes to the core; its replies decide what is connected.
@MainActor
final class GpuIme {
    struct Ticket {
        let epoch: UInt64
        let editor: GpuEditorMirror?
        let revision: UInt64
    }

    /// `GpuPages.imeEdit` codes: the core resends a not-ready edit and fails a failed one.
    static let applied: UInt8 = 0
    static let notReady: UInt8 = 1
    static let failed: UInt8 = 2

    weak var host: GpuImeHost?
    private(set) var editor: GpuEditorMirror?
    /// The connected editor's `input` from the frame: the core's keyboard configuration.
    private(set) var input: GpuFrameInput?
    /// The connected editor's presented node, for its geometry.
    private(set) var node: GpuFrameNode?
    private var pendingFocus = false
    /// The token this side last blurred: a frame drawn before the core saw the blur still
    /// shows it focused.
    private var blurred: UInt64?
    private var epoch: UInt64 = 0
    private var touchSerial: UInt64 = 0
    var caretActive = true { didSet { if !caretActive { touchSerial += 1 }; updateCaret() } }

    init(host: GpuImeHost) { self.host = host }

    var hasFocus: Bool { editor != nil }
    var isActive: Bool { editor != nil || pendingFocus }
    var ime: GpuFrameIme? { input?.ime }
    func capture() -> Ticket { Ticket(epoch: epoch, editor: editor, revision: editor?.revision ?? 0) }

    static func editable(_ node: GpuFrameNode?) -> Bool { node?.input?.enabled == true }

    func updateCaret() { host?.imeCaret(eligible: caretActive, editor: editor) }

    func focus(_ node: GpuFrameNode, generation: UInt64) {
        focus(node, generation: generation, selection: nil, then: nil)
    }

    /// A press in an editor at a logical point: focus it with the caret (or word) there.
    func touch(_ node: GpuFrameNode, generation: UInt64, x: Float, y: Float, word: Bool) {
        guard caretActive, Self.editable(node), !pendingFocus, x.isFinite, y.isFinite else { return }
        if let editor, !editor.settled { return }
        let ticket = capture()
        touchSerial += 1
        let serial = touchSerial
        host?.imeRequest(.hitTest(identity: node.identity, generation: generation, point: [x, y], word: word)) { [weak self] reply in
            guard let self, serial == self.touchSerial, ticket.epoch == self.epoch, ticket.editor === self.editor
            else { return }
            if let editor = self.editor, ticket.revision != editor.revision || !editor.settled { return }
            guard let selection = reply?.selection, selection.count == 2 else { return }
            self.focus(node, generation: generation, selection: (Int(selection[0]), Int(selection[1])), then: nil)
        }
    }

    /// An assistive or automation edit: replace the text, or select `start..end`.
    func edit(_ node: GpuFrameNode, generation: UInt64, text: String?, start: Int, end: Int) -> UInt8 {
        guard Self.editable(node), let input = node.input else { return Self.failed }
        if !caretActive || pendingFocus { return Self.notReady }
        let current = editor.map { $0.identity == node.identity } ?? false
        let value = current ? editor!.text : input.value
        let ns = value as NSString
        var (start, end) = (start, end)
        if text == nil, start == -1, end == -1 {
            start = current ? editor!.focus : ns.length
            end = start
        }
        if let text {
            if !GpuEditorMirror.valid(text) { return Self.failed }
        } else if !GpuEditorMirror.boundary(ns, start) || !GpuEditorMirror.boundary(ns, end) {
            return Self.failed
        }
        if current, let editor {
            // The mirror refuses only while its edit queue is full.
            let done = text.map { editor.replaceAll($0) } ?? editor.select(start, end)
            if done, text != nil { host?.imeInvalidated() }
            return done ? Self.applied : Self.notReady
        }
        focus(node, generation: generation, selection: nil) { editor in
            if let text { _ = editor.replaceAll(text) } else { editor.select(start, end) }
        }
        return Self.applied
    }

    private func focus(_ node: GpuFrameNode, generation: UInt64, selection: (Int, Int)?,
                       then: ((GpuEditorMirror) -> Void)?) {
        touchSerial += 1
        guard Self.editable(node) else { return }
        if let editor, editor.identity == node.identity {
            if let selection { editor.select(selection.0, selection.1) }
            then?(editor)
            host?.imeConnected(editor)
            return
        }
        drop(hide: false)
        pendingFocus = true
        let requested = epoch
        let identity = node.identity
        host?.imeRequest(.focus(identity: identity, generation: generation)) { [weak self] reply in
            guard let self else { return }
            if requested != self.epoch {
                // Superseded: give back the focus the core granted meanwhile.
                if !self.pendingFocus, self.editor == nil, let token = reply?.token {
                    self.blurred = token
                    self.host?.imeRequest(.blur(identity: identity, token: token)) { [weak self] _ in
                        self?.host?.imeRender(rebuild: false)
                    }
                }
                return
            }
            self.pendingFocus = false
            if let reply {
                self.activate(node, token: reply.token ?? 0, state: reply.state) { editor in
                    if let selection { editor.select(selection.0, selection.1) }
                    then?(editor)
                }
            }
            self.host?.imeRender(rebuild: false)
        }
    }

    /// After each presented frame: follow the core's focused editor (`node`, or none).
    func presented(_ node: GpuFrameNode?, ticket: Ticket) {
        guard ticket.epoch == epoch, !pendingFocus else { return }
        guard let node, Self.editable(node), let input = node.input, let token = input.editToken, token > 0 else {
            if editor != nil { drop(hide: true) }
            return
        }
        if let editor, editor.token == token, editor.identity == node.identity {
            if ticket.editor === editor { editor.acceptFrame(ticket.revision, state: Self.state(input.edit)) }
            self.input = input
            self.node = node
            configure(editor)
        } else if editor != nil || (input.raiseKeyboard && token != blurred) {
            activate(node, token: token, state: Self.state(input.edit))
        }
    }

    private static func state(_ edit: GpuFrameEdit?) -> GpuFrameEditorState? {
        guard let edit else { return nil }
        return GpuFrameEditorState(
            text: edit.text, selection: edit.selection, composition: edit.composition,
            focused: edit.focused, canUndo: edit.canUndo, canRedo: edit.canRedo)
    }

    private func configure(_ editor: GpuEditorMirror) {
        guard let ime = input?.ime else { return }
        editor.multiline = ime.multiline
        editor.returnInsertsNewline = ime.returnInsertsNewline
        editor.secure = ime.secure
        editor.maxLength = ime.maxLength.map(Int.init) ?? -1
    }

    private func activate(_ node: GpuFrameNode, token: UInt64, state: GpuFrameEditorState?,
                          initialize: ((GpuEditorMirror) -> Void)? = nil) {
        guard let state, token > 0 else { return }
        drop(hide: false)
        let created = epoch
        var made: GpuEditorMirror?
        made = GpuEditorMirror(identity: node.identity, token: token, state: state, send: { [weak self] revision, edits, submit in
            guard let self, let current = made else { return }
            self.host?.imeRequest(.edit(identity: current.identity, token: current.token, edits: edits, submit: submit)) { [weak self] reply in
                guard let self, created == self.epoch, current === self.editor else { return }
                if let moved = reply?.movedTo {
                    // NEXT/PREVIOUS: the reply's token and state belong to the editor focus moved to.
                    if let next = self.host?.imeNode(moved), Self.editable(next) {
                        self.activate(next, token: reply?.token ?? 0, state: reply?.state)
                    } else {
                        self.drop(hide: true)
                    }
                    self.host?.imeRender(rebuild: reply?.needsViewRender == true)
                    return
                }
                guard let reply, reply.token == current.token else {
                    self.drop(hide: true)
                    self.host?.imeRender(rebuild: false)
                    return
                }
                if current.acknowledge(revision, state: reply.state) { self.host?.imeInvalidated() }
                if submit, self.input?.ime.hideOnSubmit == true { self.host?.imeHideKeyboard() }
                self.host?.imeRender(rebuild: reply.needsViewRender == true)
            }
        }, changed: { [weak self] in self?.updateCaret() })
        guard let editor = made else { return }
        self.editor = editor
        self.input = node.input
        self.node = node
        configure(editor)
        // Publish external text or selection before the platform reads the initial state.
        initialize?(editor)
        host?.imeConnected(editor)
        updateCaret()
    }

    private func drop(hide: Bool) {
        epoch += 1
        pendingFocus = false
        let editing = editor != nil
        editor?.close()
        editor = nil
        input = nil
        node = nil
        updateCaret()
        if editing { host?.imeDisconnected(hide: hide) }
    }

    /// End editing, telling the core when `blur`.
    func clear(blur: Bool) {
        let previous = editor
        drop(hide: true)
        if blur, let previous {
            blurred = previous.token
            host?.imeRequest(.blur(identity: previous.identity, token: previous.token)) { [weak self] _ in
                self?.host?.imeRender(rebuild: false)
            }
        }
    }

    func reset() {
        clear(blur: false)
        blurred = nil
    }
}
