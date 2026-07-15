#if os(macOS)
import AppKit
import CLingXiaRustAPI
import OSLog

private struct LingXiaTerminalSnapshot: Decodable {
    let cols: UInt16
    let rows: UInt16
    let lines: [String]
    let cells: [LingXiaTerminalCell]
    let defaultForeground: String?
    let defaultBackground: String?
    let cursorRow: UInt16
    let cursorCol: UInt16
    let cursorVisible: Bool
    let cursorStyle: String?
    let applicationCursor: Bool
    let bracketedPaste: Bool
    let alternateScreen: Bool
    let scrollbar: LingXiaTerminalScrollbar?
    let processTitle: String?
    let title: String?
    let generation: UInt64
    let exited: Bool

    enum CodingKeys: String, CodingKey {
        case cols
        case rows
        case lines
        case cells
        case defaultForeground = "default_foreground"
        case defaultBackground = "default_background"
        case cursorRow = "cursor_row"
        case cursorCol = "cursor_col"
        case cursorVisible = "cursor_visible"
        case cursorStyle = "cursor_style"
        case applicationCursor = "application_cursor"
        case bracketedPaste = "bracketed_paste"
        case alternateScreen = "alternate_screen"
        case scrollbar
        case processTitle = "process_title"
        case title
        case generation
        case exited
    }
}

private struct LingXiaTerminalScrollbar: Decodable {
    let total: UInt64
    let offset: UInt64
    let len: UInt64
}

private struct LingXiaTerminalCell: Decodable {
    let row: UInt16
    let col: UInt16
    let text: String
    let fg: String?
    let bg: String?
    let bold: Bool
    let dim: Bool
    let italic: Bool
    let underline: Bool
    let inverse: Bool
    let wide: Bool
}

private struct LingXiaTerminalRenderStyle: Equatable {
    let fg: String?
    let bg: String?
    let bold: Bool
    let dim: Bool
    let italic: Bool
    let underline: Bool
    let inverse: Bool
}

private struct LingXiaTerminalGridPoint: Equatable {
    var row: Int
    var col: Int
}

@MainActor
final class LingXiaTerminalPaneView: NSView {
    let paneID = UUID()
    var onActivated: ((UUID) -> Void)?
    var onSplitRequested: ((UUID, LingXiaTerminalSplitDirection) -> Void)?
    var onZoomRequested: ((UUID) -> Void)?
    var onTitleChanged: ((UUID, String?, String?) -> Void)?
    var onManualTitleChanged: ((UUID, String) -> Void)?
    var onTitleEditRequested: ((UUID) -> Void)?
    var onExited: ((UUID) -> Void)?

    private let terminalView = LingXiaTerminalCanvasView()
    private let session: LingXiaPTYTerminalSession
    private let font = LingXiaTerminalFont.regular()

    init() {
        self.session = LingXiaPTYTerminalSession()
        super.init(frame: .zero)
        lxTerminalLog("pane.init pane=\(paneID.uuidString)")
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        layer?.cornerRadius = 0
        layer?.borderColor = NSColor.clear.cgColor
        layer?.borderWidth = 0
        layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor

        setupTerminalView()
        setupLayout()

        terminalView.onInput = { [weak self] input in
            if let self {
                lxTerminalLog("pane.textInput pane=\(self.paneID.uuidString) bytes=\(input.utf8.count)")
            }
            self?.session.send(input)
        }
        terminalView.onActivated = { [weak self] in
            guard let self else { return }
            self.onActivated?(self.paneID)
        }
        terminalView.onSplitRequested = { [weak self] direction in
            guard let self else { return }
            self.onActivated?(self.paneID)
            self.onSplitRequested?(self.paneID, direction)
        }
        terminalView.onZoomRequested = { [weak self] in
            guard let self else { return }
            self.onActivated?(self.paneID)
            self.onZoomRequested?(self.paneID)
        }
        terminalView.onResize = { [weak self] cols, rows in
            self?.session.resize(cols: cols, rows: rows)
        }
        terminalView.onScroll = { [weak self] rows, col, row, allowApplicationInput in
            self?.session.scroll(
                rows: rows,
                col: col,
                row: row,
                allowApplicationInput: allowApplicationInput
            )
        }
        terminalView.onResetRequested = { [weak self] in
            self?.session.restart()
        }
        terminalView.onTitleEditRequested = { [weak self] in
            guard let self else { return }
            self.onTitleEditRequested?(self.paneID)
        }

        session.onSnapshot = { [weak self] snapshot in
            Task { @MainActor [weak self] in
                if let self {
                    lxTerminalLog("pane.snapshot pane=\(self.paneID.uuidString) generation=\(snapshot.generation) cols=\(snapshot.cols) rows=\(snapshot.rows)")
                }
                self?.applySnapshot(snapshot)
            }
        }
        session.onError = { [weak self] error in
            Task { @MainActor [weak self] in
                if let self {
                    lxTerminalLog("pane.error pane=\(self.paneID.uuidString) error=\(error)", type: .error)
                }
                self?.appendOutput("\r\n[terminal error] \(error)\r\n")
            }
        }
        session.onExit = { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                lxTerminalLog("pane.exit pane=\(self.paneID.uuidString)")
                self.onExited?(self.paneID)
            }
        }
        session.start()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        lxTerminalLogAsync("pane.deinit pane=\(paneID.uuidString)")
        session.stop()
    }

    func setActive(_ active: Bool) {
        layer?.borderWidth = 0
    }

    func setZoomed(_ zoomed: Bool) {
        terminalView.zoomed = zoomed
    }

    func focusTerminal() {
        guard let window else {
            lxTerminalLog("pane.focusTerminal no-window pane=\(paneID.uuidString)")
            return
        }
        layoutSubtreeIfNeeded()
        window.makeKey()
        let accepted = window.makeFirstResponder(terminalView)
        let responder = window.firstResponder.map { String(describing: type(of: $0)) } ?? "nil"
        lxTerminalLog(
            "pane.focusTerminal pane=\(paneID.uuidString) accepted=\(accepted) firstResponder=\(responder) terminalWindow=\(terminalView.window != nil) terminalBounds=\(String(format: "%.0fx%.0f", terminalView.bounds.width, terminalView.bounds.height))"
        )
        if !accepted {
            let fallbackAccepted = window.makeFirstResponder(self)
            let fallbackResponder = window.firstResponder.map { String(describing: type(of: $0)) } ?? "nil"
            lxTerminalLog(
                "pane.focusTerminal fallback pane=\(paneID.uuidString) accepted=\(fallbackAccepted) firstResponder=\(fallbackResponder)"
            )
        }
        onActivated?(paneID)
    }

    func sendInput(_ input: String) {
        lxTerminalLog("pane.sendInput pane=\(paneID.uuidString) bytes=\(input.utf8.count)")
        session.send(input)
    }

    func ownsFirstResponder(_ responder: NSResponder?) -> Bool {
        guard let responder else { return false }
        if responder === self || responder === terminalView {
            return true
        }
        guard let responderView = responder as? NSView else {
            return false
        }
        return responderView === self
            || responderView === terminalView
            || responderView.isDescendant(of: self)
    }

    @discardableResult
    func consumeKeyDown(_ event: NSEvent, source: String) -> Bool {
        terminalView.consumeKeyDown(event, source: source)
    }

    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func becomeFirstResponder() -> Bool {
        lxTerminalLog("pane.becomeFirstResponder pane=\(paneID.uuidString)")
        onActivated?(paneID)
        return true
    }

    override func mouseDown(with event: NSEvent) {
        lxTerminalLog("pane.mouseDown pane=\(paneID.uuidString)")
        let accepted = window?.makeFirstResponder(terminalView) ?? false
        lxTerminalLog("pane.mouseDown focusCanvas pane=\(paneID.uuidString) accepted=\(accepted)")
        onActivated?(paneID)
    }

    override func keyDown(with event: NSEvent) {
        if !consumeKeyDown(event, source: "pane") {
            lxTerminalLog("pane.keyDown pass pane=\(paneID.uuidString) keyCode=\(event.keyCode)")
            super.keyDown(with: event)
        }
    }

    private func setupTerminalView() {
        terminalView.translatesAutoresizingMaskIntoConstraints = false
        terminalView.font = font
    }

    func showContextMenu(fromWindowEvent event: NSEvent) {
        terminalView.showContextMenu(fromWindowEvent: event)
    }

    private func setupLayout() {
        addSubview(terminalView)
        NSLayoutConstraint.activate([
            terminalView.topAnchor.constraint(equalTo: topAnchor),
            terminalView.leadingAnchor.constraint(equalTo: leadingAnchor),
            terminalView.trailingAnchor.constraint(equalTo: trailingAnchor),
            terminalView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    private func appendOutput(_ output: String) {
        terminalView.append(output)
    }

    private func applySnapshot(_ snapshot: LingXiaTerminalSnapshot) {
        onTitleChanged?(paneID, snapshot.processTitle, snapshot.title)
        terminalView.applySnapshot(snapshot)
    }

}

@MainActor
private final class LingXiaTerminalCanvasView: NSView {
    var onInput: ((String) -> Void)?
    var onActivated: (() -> Void)?
    var onSplitRequested: ((LingXiaTerminalSplitDirection) -> Void)?
    var onZoomRequested: (() -> Void)?
    var onResize: ((UInt16, UInt16) -> Void)?
    var onScroll: ((Int, UInt16, UInt16, Bool) -> Void)?
    var onResetRequested: (() -> Void)?
    var onTitleEditRequested: (() -> Void)?
    var zoomed = false

    var font = LingXiaTerminalFont.regular() {
        didSet {
            recalculateGridSize()
            needsDisplay = true
        }
    }

    private var cols = 120
    private var rows = 32
    private var lines: [String] = Array(repeating: "", count: 32)
    private var cells: [LingXiaTerminalCell] = []
    private var defaultForeground = NSColor.lxTerminalForeground
    private var defaultBackground = NSColor.lxTerminalBackground
    private var cursorRow = 0
    private var cursorCol = 0
    private var cursorVisible = true
    private var cursorStyle = "block"
    private var applicationCursor = false
    private var bracketedPaste = false
    private var alternateScreen = false
    private var scrollbar: LingXiaTerminalScrollbar?
    private var scrollbarVisible = false
    private var scrollbarVisibilityToken: UInt64 = 0
    private var charSize = NSSize(width: 7.2, height: 15)
    private var lastSentSize: (cols: UInt16, rows: UInt16)?
    private var selectionAnchor: LingXiaTerminalGridPoint?
    private var selectionFocus: LingXiaTerminalGridPoint?
    private var scrollRowRemainder: CGFloat = 0
    private var readOnly = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        layerContentsRedrawPolicy = .onSetNeedsDisplay
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func becomeFirstResponder() -> Bool {
        lxTerminalLog("canvas.becomeFirstResponder bounds=\(String(format: "%.0fx%.0f", bounds.width, bounds.height)) cols=\(cols) rows=\(rows)")
        onActivated?()
        needsDisplay = true
        return true
    }

    override func resignFirstResponder() -> Bool {
        lxTerminalLog("canvas.resignFirstResponder")
        needsDisplay = true
        return super.resignFirstResponder()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        lxTerminalLog("canvas.viewDidMoveToWindow hasWindow=\(window != nil) bounds=\(String(format: "%.0fx%.0f", bounds.width, bounds.height))")
        layer?.contentsScale = backingScale
        recalculateGridSize()
    }

    override func mouseDown(with event: NSEvent) {
        lxTerminalLog("canvas.mouseDown keyWindow=\(window?.isKeyWindow ?? false)")
        _ = window?.makeFirstResponder(self)
        onActivated?()
        let point = gridPoint(for: convert(event.locationInWindow, from: nil))
        selectionAnchor = point
        selectionFocus = point
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        selectionFocus = gridPoint(for: convert(event.locationInWindow, from: nil))
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        guard selectionAnchor == selectionFocus else { return }
        selectionAnchor = nil
        selectionFocus = nil
        needsDisplay = true
    }

    override func rightMouseDown(with event: NSEvent) {
        lxTerminalLog("canvas.rightMouseDown showContextMenu")
        _ = window?.makeFirstResponder(self)
        onActivated?()
        NSMenu.popUpContextMenu(splitMenu(), with: event, for: self)
    }

    override func scrollWheel(with event: NSEvent) {
        let rows = event.hasPreciseScrollingDeltas
            ? event.scrollingDeltaY / max(charSize.height, 1)
            : event.scrollingDeltaY * 3
        scrollRowRemainder += rows
        let wholeRows = Int(scrollRowRemainder.rounded(.towardZero))
        guard wholeRows != 0 else { return }
        scrollRowRemainder -= CGFloat(wholeRows)
        revealScrollbar()
        selectionAnchor = nil
        selectionFocus = nil
        needsDisplay = true
        let point = gridPoint(for: convert(event.locationInWindow, from: nil))
        onScroll?(-wholeRows, UInt16(point.col), UInt16(point.row), !readOnly)
    }

    func showContextMenu(fromWindowEvent event: NSEvent) {
        lxTerminalLog("canvas.showContextMenuFromWorkspace")
        _ = window?.makeFirstResponder(self)
        onActivated?()
        NSMenu.popUpContextMenu(splitMenu(), with: event, for: self)
    }

    override func layout() {
        super.layout()
        recalculateGridSize()
    }

    @discardableResult
    func consumeKeyDown(_ event: NSEvent, source: String) -> Bool {
        if event.modifierFlags.contains(.command) {
            switch Int(event.keyCode) {
            case 8:
                lxTerminalLog("\(source).keyDown commandCopy")
                copy(nil)
                return true
            case 9:
                lxTerminalLog("\(source).keyDown commandPaste")
                paste(nil)
                return true
            default:
                break
            }
        }
        guard !readOnly else {
            lxTerminalLog("\(source).keyDown ignoredReadOnly keyCode=\(event.keyCode)")
            return true
        }
        guard let input = LingXiaTerminalKeyMapper.input(for: event, applicationCursor: applicationCursor) else {
            lxTerminalLog("\(source).keyDown pass keyCode=\(event.keyCode)")
            return false
        }
        lxTerminalLog("\(source).keyDown input keyCode=\(event.keyCode) bytes=\(input.utf8.count) appCursor=\(applicationCursor)")
        onInput?(input)
        return true
    }

    override func keyDown(with event: NSEvent) {
        if !consumeKeyDown(event, source: "canvas") {
            super.keyDown(with: event)
        }
    }

    @objc func paste(_ sender: Any?) {
        guard !readOnly else {
            lxTerminalLog("canvas.paste ignoredReadOnly")
            return
        }
        if let text = NSPasteboard.general.string(forType: .string), !text.isEmpty {
            let payload = pastePayload(for: text)
            lxTerminalLog("canvas.paste chars=\(text.count) bytes=\(payload.utf8.count) bracketed=\(bracketedPaste) alternate=\(alternateScreen)")
            onInput?(payload)
        } else {
            lxTerminalLog("canvas.paste empty")
        }
    }

    @objc func copy(_ sender: Any?) {
        guard let text = selectedText(), !text.isEmpty else {
            lxTerminalLog("canvas.copy emptySelection")
            return
        }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        lxTerminalLog("canvas.copy chars=\(text.count)")
    }

    override func draw(_ dirtyRect: NSRect) {
        defaultBackground.setFill()
        dirtyRect.fill()
        configureTextRendering()

        let attributes = textAttributes(bold: false, italic: false, underline: false, foreground: defaultForeground)
        let insetX: CGFloat = 0
        let insetTop: CGFloat = 0
        let baselineOffset = terminalBaselineOffset()
        if cells.isEmpty {
            for row in 0..<rows {
                let line = row < lines.count ? lines[row] : ""
                guard !line.isEmpty else { continue }
                let y = bounds.height - insetTop - CGFloat(row + 1) * charSize.height + baselineOffset
                drawTerminalText(line, at: NSPoint(x: insetX, y: y), attributes: attributes)
            }
        }

        // Draw all backgrounds first. Otherwise a following cell background can
        // cover the right half of a wide CJK glyph drawn by the previous cell.
        let orderedCells = cells.sorted {
            if $0.row == $1.row {
                return $0.col < $1.col
            }
            return $0.row < $1.row
        }

        for cell in orderedCells {
            let bg = cell.inverse
                ? terminalColor(cell.fg, fallback: defaultForeground)
                : terminalColor(cell.bg, fallback: nil)
            let x = pixelFloor(insetX + CGFloat(cell.col) * charSize.width)
            let y = pixelFloor(bounds.height - insetTop - CGFloat(cell.row + 1) * charSize.height)
            if let bg {
                bg.setFill()
                pixelAlignedRect(x: x, y: y, width: charSize.width * (cell.wide ? 2 : 1), height: charSize.height).fill()
            }
        }

        drawSelectionOverlay()

        var runText = ""
        var runRow = -1
        var runStartCol = 0
        var runNextCol = 0
        var runStyle: LingXiaTerminalRenderStyle?

        func flushRun() {
            guard !runText.isEmpty, let style = runStyle else { return }
            let defaultColor = style.inverse ? defaultBackground : defaultForeground
            let fg = terminalColor(
                style.inverse ? style.bg : style.fg,
                fallback: defaultColor
            )?.withAlphaComponent(style.dim ? 0.58 : 1) ?? defaultColor
            let attrs = textAttributes(bold: style.bold, italic: style.italic, underline: style.underline, foreground: fg)
            let x = pixelFloor(insetX + CGFloat(runStartCol) * charSize.width)
            let y = pixelFloor(bounds.height - insetTop - CGFloat(runRow + 1) * charSize.height)
            drawTerminalText(runText, at: NSPoint(x: x, y: y + baselineOffset), attributes: attrs)
            runText.removeAll(keepingCapacity: true)
            runStyle = nil
        }

        for cell in orderedCells {
            if !cell.text.isEmpty {
                let style = LingXiaTerminalRenderStyle(
                    fg: cell.fg,
                    bg: cell.bg,
                    bold: cell.bold,
                    dim: cell.dim,
                    italic: cell.italic,
                    underline: cell.underline,
                    inverse: cell.inverse
                )
                let cellCol = Int(cell.col)
                if runStyle != style || runRow != Int(cell.row) || runNextCol != cellCol {
                    flushRun()
                    runStyle = style
                    runRow = Int(cell.row)
                    runStartCol = cellCol
                    runNextCol = cellCol
                }
                runText += cell.text
                runNextCol = cellCol + (cell.wide ? 2 : 1)
            }
        }
        flushRun()

        if window?.firstResponder === self, cursorVisible {
            let x = pixelFloor(insetX + CGFloat(cursorCol) * charSize.width)
            let y = pixelFloor(bounds.height - insetTop - CGFloat(cursorRow + 1) * charSize.height)
            drawCursor(at: NSPoint(x: x, y: y))
        }
        drawScrollbar()
    }

    func append(_ output: String) {
        if !output.isEmpty {
            lines.append(contentsOf: output.components(separatedBy: .newlines))
            if lines.count > rows {
                lines = Array(lines.suffix(rows))
            }
        }
        needsDisplay = true
    }

    func applySnapshot(_ snapshot: LingXiaTerminalSnapshot) {
        cols = max(1, Int(snapshot.cols))
        rows = max(1, Int(snapshot.rows))
        lines = snapshot.lines
        cells = snapshot.cells
        defaultForeground = terminalColor(snapshot.defaultForeground, fallback: .lxTerminalForeground) ?? .lxTerminalForeground
        defaultBackground = terminalColor(snapshot.defaultBackground, fallback: .lxTerminalBackground) ?? .lxTerminalBackground
        layer?.backgroundColor = defaultBackground.cgColor
        cursorRow = Int(snapshot.cursorRow)
        cursorCol = Int(snapshot.cursorCol)
        cursorVisible = snapshot.cursorVisible
        cursorStyle = snapshot.cursorStyle ?? "block"
        applicationCursor = snapshot.applicationCursor
        bracketedPaste = snapshot.bracketedPaste
        alternateScreen = snapshot.alternateScreen
        scrollbar = snapshot.scrollbar
        needsDisplay = true
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        onActivated?()
        return splitMenu()
    }

    private func splitMenu() -> NSMenu {
        lxTerminalLog("canvas.splitMenu readOnly=\(readOnly) hasSelection=\(selectedText()?.isEmpty == false)")
        let menu = NSMenu(title: "Terminal")
        let copyItem = menuItem("Copy", action: #selector(copy(_:)))
        copyItem.isEnabled = selectedText()?.isEmpty == false
        menu.addItem(copyItem)
        menu.addItem(menuItem("Paste", action: #selector(paste(_:))))
        menu.addItem(.separator())
        menu.addItem(menuItem("Split Right", action: #selector(splitRightFromMenu)))
        menu.addItem(menuItem("Split Left", action: #selector(splitLeftFromMenu)))
        menu.addItem(menuItem("Split Down", action: #selector(splitBottomFromMenu)))
        menu.addItem(menuItem("Split Up", action: #selector(splitTopFromMenu)))
        menu.addItem(.separator())
        menu.addItem(menuItem("Change Tab Title...", action: #selector(changeTabTitleFromMenu)))
        menu.addItem(menuItem("Reset Terminal", action: #selector(resetTerminalFromMenu)))
        let readOnlyItem = menuItem("Terminal Read-only", action: #selector(toggleReadOnlyFromMenu))
        readOnlyItem.state = readOnly ? .on : .off
        menu.addItem(readOnlyItem)
        return menu
    }

    private func menuItem(_ title: String, action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        return item
    }

    @objc private func splitLeftFromMenu() {
        lxTerminalLog("canvas.menu splitLeft")
        onSplitRequested?(.left)
    }

    @objc private func splitRightFromMenu() {
        lxTerminalLog("canvas.menu splitRight")
        onSplitRequested?(.right)
    }

    @objc private func splitTopFromMenu() {
        lxTerminalLog("canvas.menu splitTop")
        onSplitRequested?(.up)
    }

    @objc private func splitBottomFromMenu() {
        lxTerminalLog("canvas.menu splitBottom")
        onSplitRequested?(.down)
    }

    @objc private func changeTabTitleFromMenu() {
        lxTerminalLog("canvas.menu changeTabTitle")
        onTitleEditRequested?()
    }

    @objc private func resetTerminalFromMenu() {
        lxTerminalLog("canvas.menu reset")
        onResetRequested?()
    }

    @objc private func toggleReadOnlyFromMenu() {
        readOnly.toggle()
        lxTerminalLog("canvas.menu toggleReadOnly readOnly=\(readOnly)")
    }

    private func pastePayload(for text: String) -> String {
        // Shell line editors often enable bracketed paste, but that also keeps
        // zle/readline highlighting suspended while the text is inserted. Use
        // bracketed paste for full-screen terminal apps; let shells repaint
        // pasted input normally so the visual state does not collapse to white.
        if bracketedPaste && alternateScreen {
            return "\u{1B}[200~\(text)\u{1B}[201~"
        }
        return text
    }

    private func gridPoint(for point: NSPoint) -> LingXiaTerminalGridPoint {
        let row = Int((bounds.height - point.y) / max(1, charSize.height))
        let col = Int(point.x / max(1, charSize.width))
        return LingXiaTerminalGridPoint(
            row: min(max(row, 0), max(0, rows - 1)),
            col: min(max(col, 0), max(0, cols))
        )
    }

    private func normalizedSelection() -> (start: LingXiaTerminalGridPoint, end: LingXiaTerminalGridPoint)? {
        guard var start = selectionAnchor,
              var end = selectionFocus,
              start != end else {
            return nil
        }
        if start.row > end.row || (start.row == end.row && start.col > end.col) {
            swap(&start, &end)
        }
        return (start, end)
    }

    private func drawSelectionOverlay() {
        guard let selection = normalizedSelection() else { return }
        NSColor.selectedContentBackgroundColor.withAlphaComponent(0.46).setFill()
        for row in selection.start.row...selection.end.row {
            let startCol = row == selection.start.row ? selection.start.col : 0
            let endCol = row == selection.end.row ? selection.end.col : cols
            guard endCol > startCol else { continue }
            let x = pixelFloor(CGFloat(startCol) * charSize.width)
            let y = pixelFloor(bounds.height - CGFloat(row + 1) * charSize.height)
            pixelAlignedRect(
                x: x,
                y: y,
                width: CGFloat(endCol - startCol) * charSize.width,
                height: charSize.height
            ).fill()
        }
    }

    private func revealScrollbar() {
        scrollbarVisibilityToken &+= 1
        let token = scrollbarVisibilityToken
        scrollbarVisible = true
        needsDisplay = true
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(900)) { [weak self] in
            guard let self, self.scrollbarVisibilityToken == token else { return }
            self.scrollbarVisible = false
            self.needsDisplay = true
        }
    }

    private func drawScrollbar() {
        guard scrollbarVisible,
              let scrollbar,
              scrollbar.total > scrollbar.len,
              scrollbar.len > 0 else {
            return
        }
        let margin: CGFloat = 2
        let width: CGFloat = 3
        let trackHeight = bounds.height - margin * 2
        guard trackHeight > 0 else { return }

        let visibleRows = min(scrollbar.len, scrollbar.total)
        let thumbHeight = min(
            trackHeight,
            max(
                12,
                min(40, trackHeight * CGFloat(visibleRows) / CGFloat(scrollbar.total))
            )
        )
        let maxOffset = scrollbar.total - visibleRows
        let offset = min(scrollbar.offset, maxOffset)
        let available = trackHeight - thumbHeight
        let topOffset = maxOffset == 0
            ? 0
            : available * CGFloat(offset) / CGFloat(maxOffset)
        let y = bounds.height - margin - topOffset - thumbHeight

        defaultForeground.withAlphaComponent(0.38).setFill()
        pixelAlignedRect(
            x: bounds.width - margin - width,
            y: y,
            width: width,
            height: thumbHeight
        ).fill()
    }

    private func selectedText() -> String? {
        guard let selection = normalizedSelection() else { return nil }
        var selectedLines: [String] = []
        for row in selection.start.row...selection.end.row {
            let startCol = row == selection.start.row ? selection.start.col : 0
            let endCol = row == selection.end.row ? selection.end.col : cols
            guard endCol > startCol else {
                selectedLines.append("")
                continue
            }
            selectedLines.append(textInRow(row, startCol: startCol, endCol: endCol))
        }
        return selectedLines.joined(separator: "\n")
    }

    private func textInRow(_ row: Int, startCol: Int, endCol: Int) -> String {
        guard row >= 0, row < lines.count else { return "" }
        var chars = Array(lines[row])
        if chars.count < endCol {
            chars.append(contentsOf: Array(repeating: Character(" "), count: endCol - chars.count))
        }
        let safeStart = min(max(startCol, 0), chars.count)
        let safeEnd = min(max(endCol, safeStart), chars.count)
        return String(chars[safeStart..<safeEnd]).trimmingCharacters(in: .whitespaces)
    }

    private func recalculateGridSize() {
        let sample = "W" as NSString
        let measured = sample.size(withAttributes: [.font: font])
        charSize = NSSize(
            width: max(1, pixelCeil(measured.width)),
            height: max(1, pixelCeil(font.ascender - font.descender + max(2, font.leading)))
        )
        let horizontalInset: CGFloat = 0
        let verticalInset: CGFloat = 4
        let nextCols = max(20, Int((bounds.width - horizontalInset) / charSize.width))
        let nextRows = max(4, Int((bounds.height - verticalInset) / charSize.height))
        let safeCols = UInt16(max(1, min(nextCols, Int(UInt16.max))))
        let safeRows = UInt16(max(1, min(nextRows, Int(UInt16.max))))
        if lastSentSize?.cols != safeCols || lastSentSize?.rows != safeRows {
            lxTerminalLog(
                "canvas.resizeGrid bounds=\(String(format: "%.0fx%.0f", bounds.width, bounds.height)) char=\(String(format: "%.1fx%.1f", charSize.width, charSize.height)) cols=\(safeCols) rows=\(safeRows) scale=\(String(format: "%.1f", backingScale))"
            )
            lastSentSize = (safeCols, safeRows)
            onResize?(safeCols, safeRows)
        }
        needsDisplay = true
    }

    private func textAttributes(
        bold: Bool,
        italic: Bool,
        underline: Bool,
        foreground: NSColor
    ) -> [NSAttributedString.Key: Any] {
        let resolvedFont = LingXiaTerminalFont.make(size: font.pointSize, bold: bold, italic: italic)
        var attrs: [NSAttributedString.Key: Any] = [
            .font: resolvedFont,
            .foregroundColor: foreground,
            .kern: 0,
            .ligature: 0,
        ]
        if underline {
            attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue
        }
        return attrs
    }

    private var backingScale: CGFloat {
        window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
    }

    private func pixelFloor(_ value: CGFloat) -> CGFloat {
        floor(value * backingScale) / backingScale
    }

    private func pixelCeil(_ value: CGFloat) -> CGFloat {
        ceil(value * backingScale) / backingScale
    }

    private func pixelAlignedRect(x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat) -> NSRect {
        NSRect(
            x: pixelFloor(x),
            y: pixelFloor(y),
            width: max(1 / backingScale, pixelCeil(width)),
            height: max(1 / backingScale, pixelCeil(height))
        )
    }

    private func terminalBaselineOffset() -> CGFloat {
        let glyphHeight = font.ascender - font.descender
        let centeredTopPadding = max(0, (charSize.height - glyphHeight) / 2)
        return pixelFloor(centeredTopPadding - font.descender)
    }

    private func configureTextRendering() {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        context.setShouldAntialias(true)
        context.setShouldSmoothFonts(true)
        context.setShouldSubpixelPositionFonts(true)
        context.setShouldSubpixelQuantizeFonts(true)
        context.textMatrix = .identity
    }

    private func drawTerminalText(
        _ text: String,
        at point: NSPoint,
        attributes: [NSAttributedString.Key: Any]
    ) {
        guard let context = NSGraphicsContext.current?.cgContext else {
            text.draw(at: point, withAttributes: attributes)
            return
        }
        context.saveGState()
        context.textMatrix = .identity
        context.textPosition = CGPoint(x: pixelFloor(point.x), y: pixelFloor(point.y))
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attributes))
        CTLineDraw(line, context)
        context.restoreGState()
    }

    private func drawCursor(at origin: NSPoint) {
        let rect = pixelAlignedRect(x: origin.x, y: origin.y, width: charSize.width, height: charSize.height)
        defaultForeground.withAlphaComponent(0.62).setFill()
        switch cursorStyle {
        case "bar":
            pixelAlignedRect(x: origin.x, y: origin.y, width: 1.5, height: charSize.height).fill()
        case "underline":
            pixelAlignedRect(x: origin.x, y: origin.y, width: charSize.width, height: 1.5).fill()
        case "hollow":
            defaultForeground.setStroke()
            let path = NSBezierPath(rect: rect.insetBy(dx: 0.5, dy: 0.5))
            path.lineWidth = max(1 / backingScale, 1)
            path.stroke()
        default:
            pixelAlignedRect(x: origin.x, y: origin.y, width: 1.5, height: charSize.height).fill()
        }
    }

    private func terminalColor(_ token: String?, fallback: NSColor?) -> NSColor? {
        guard let token else { return fallback }
        if token.hasPrefix("#"), token.count == 7 {
            let hex = String(token.dropFirst())
            guard let value = UInt32(hex, radix: 16) else { return fallback }
            return NSColor(
                red: CGFloat((value >> 16) & 0xff) / 255.0,
                green: CGFloat((value >> 8) & 0xff) / 255.0,
                blue: CGFloat(value & 0xff) / 255.0,
                alpha: 1
            )
        }
        if token.hasPrefix("idx:"),
           let index = Int(token.dropFirst(4)) {
            return Self.palette[index % Self.palette.count]
        }
        return fallback
    }

    private static let palette: [NSColor] = [
        NSColor(red: 0.08, green: 0.09, blue: 0.11, alpha: 1),
        NSColor(red: 0.86, green: 0.20, blue: 0.22, alpha: 1),
        NSColor(red: 0.33, green: 0.75, blue: 0.35, alpha: 1),
        NSColor(red: 0.92, green: 0.72, blue: 0.31, alpha: 1),
        NSColor(red: 0.31, green: 0.58, blue: 0.98, alpha: 1),
        NSColor(red: 0.74, green: 0.38, blue: 0.91, alpha: 1),
        NSColor(red: 0.35, green: 0.78, blue: 0.86, alpha: 1),
        NSColor(red: 0.86, green: 0.88, blue: 0.90, alpha: 1),
        NSColor(red: 0.38, green: 0.42, blue: 0.48, alpha: 1),
        NSColor(red: 1.00, green: 0.36, blue: 0.38, alpha: 1),
        NSColor(red: 0.52, green: 0.90, blue: 0.48, alpha: 1),
        NSColor(red: 1.00, green: 0.84, blue: 0.42, alpha: 1),
        NSColor(red: 0.48, green: 0.70, blue: 1.00, alpha: 1),
        NSColor(red: 0.86, green: 0.52, blue: 1.00, alpha: 1),
        NSColor(red: 0.50, green: 0.92, blue: 0.98, alpha: 1),
        NSColor(red: 1.00, green: 1.00, blue: 1.00, alpha: 1),
    ]
}

private final class LingXiaPTYTerminalSession: @unchecked Sendable {
    private static let log = OSLog(subsystem: "LingXia", category: "MacTerminalPTY")

    var onSnapshot: ((LingXiaTerminalSnapshot) -> Void)?
    var onError: ((String) -> Void)?
    var onExit: (() -> Void)?

    private let ioQueue = DispatchQueue(label: "app.lingxia.terminal.pty", qos: .userInitiated)
    private let decoder = JSONDecoder()
    private var sessionID: UInt64 = 0
    private var readTimer: DispatchSourceTimer?
    private var pendingInput = ""

    func start() {
        ioQueue.async { [weak self] in
            guard let self, self.sessionID == 0 else { return }
            self.startOnIOQueue()
        }
    }

    func restart() {
        ioQueue.async { [weak self] in
            guard let self else { return }
            self.stopOnIOQueue()
            self.startOnIOQueue()
        }
    }

    private func startOnIOQueue() {
        lxTerminalLogAsync("pty.start create cols=120 rows=32")
        let id = terminalSessionCreate(120, 32)
        guard id != 0 else {
            lxTerminalLogAsync("pty.start failed create", type: .error)
            emitError("terminal runtime failed to start")
            return
        }
        sessionID = id
        lxTerminalLogAsync("pty.start created session=\(id)")
        if !pendingInput.isEmpty {
            let pendingBytes = pendingInput.utf8.count
            let ok = terminalSessionWrite(id, pendingInput)
            lxTerminalLogAsync("pty.flushPending session=\(id) bytes=\(pendingBytes) ok=\(ok)")
            pendingInput.removeAll(keepingCapacity: true)
        }
        startReadTimerOnIOQueue()
    }

    func send(_ input: String) {
        guard !input.isEmpty else { return }
        ioQueue.async { [weak self] in
            guard let self else { return }
            guard self.sessionID != 0 else {
                self.pendingInput += input
                lxTerminalLogAsync("pty.send queued bytes=\(input.utf8.count) pending=\(self.pendingInput.utf8.count)")
                return
            }
            let ok = terminalSessionWrite(self.sessionID, input)
            lxTerminalLogAsync("pty.send write session=\(self.sessionID) bytes=\(input.utf8.count) ok=\(ok)")
            if !ok {
                LXLog.error("terminal write failed session=\(self.sessionID)", category: "MacTerminalPTY")
            }
        }
    }

    func resize(cols: UInt16, rows: UInt16) {
        ioQueue.async { [weak self] in
            guard let self, self.sessionID != 0 else { return }
            let ok = terminalSessionResize(self.sessionID, cols, rows)
            lxTerminalLogAsync("pty.resize session=\(self.sessionID) cols=\(cols) rows=\(rows) ok=\(ok)")
        }
    }

    func scroll(rows: Int, col: UInt16, row: UInt16, allowApplicationInput: Bool) {
        guard rows != 0 else { return }
        ioQueue.async { [weak self] in
            guard let self, self.sessionID != 0 else { return }
            let delta = Int32(clamping: rows)
            let ok = terminalSessionScroll(self.sessionID, delta, col, row, allowApplicationInput)
            lxTerminalLogAsync("pty.scroll session=\(self.sessionID) rows=\(delta) cell=\(col),\(row) ok=\(ok)")
        }
    }

    func stop() {
        ioQueue.async { [weak self] in
            self?.stopOnIOQueue()
        }
    }

    private func startReadTimerOnIOQueue() {
        let timer = DispatchSource.makeTimerSource(queue: ioQueue)
        timer.schedule(deadline: .now(), repeating: .milliseconds(16), leeway: .milliseconds(8))
        timer.setEventHandler { [weak self] in
            self?.drainOutputOnIOQueue()
        }
        timer.resume()
        readTimer = timer
    }

    private func stopOnIOQueue() {
        readTimer?.cancel()
        readTimer = nil
        pendingInput.removeAll(keepingCapacity: false)
        if sessionID != 0 {
            lxTerminalLogAsync("pty.stop close session=\(sessionID)")
            terminalSessionClose(sessionID)
            sessionID = 0
        }
    }

    private func drainOutputOnIOQueue() {
        guard sessionID != 0 else { return }
        let id = sessionID
        let json = terminalSessionSnapshot(id).toString()
        guard let data = json.data(using: .utf8) else {
            lxTerminalLogAsync("pty.snapshot invalid-utf8 session=\(id)", type: .error)
            return
        }
        do {
            let snapshot = try decoder.decode(LingXiaTerminalSnapshot.self, from: data)
            emit(snapshot)
            if snapshot.exited {
                lxTerminalLogAsync("pty.exited session=\(id)")
                stopOnIOQueue()
                emitExit()
            }
        } catch {
            lxTerminalLogAsync("pty.snapshot decode-failed session=\(id) error=\(error)", type: .error)
            emitError("terminal snapshot decode failed: \(error.localizedDescription)")
        }
    }

    private func emit(_ snapshot: LingXiaTerminalSnapshot) {
        DispatchQueue.main.async { [onSnapshot] in
            onSnapshot?(snapshot)
        }
    }

    private func emitError(_ error: String) {
        DispatchQueue.main.async { [onError] in
            onError?(error)
        }
    }

    private func emitExit() {
        DispatchQueue.main.async { [onExit] in
            onExit?()
        }
    }
}
#endif
