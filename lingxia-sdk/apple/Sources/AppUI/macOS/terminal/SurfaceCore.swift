#if os(macOS)
import AppKit
import CLingXiaRustAPI
import OSLog

private struct LingXiaTerminalTitleState: Decodable {
    let processTitle: String
    let title: String?
    let titleGeneration: UInt64
}

private struct LingXiaTerminalScrollbar: Decodable {
    let total: UInt64
    let offset: UInt64
    let len: UInt64
}

private struct LingXiaTerminalImageSnapshot: Decodable, Sendable {
    let changed: Bool
    let generation: UInt64
    let images: [LingXiaTerminalImage]
    let placements: [LingXiaTerminalImagePlacement]

    private enum CodingKeys: String, CodingKey {
        case changed, generation, images, placements
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        changed = try values.decode(Bool.self, forKey: .changed)
        generation = try values.decode(UInt64.self, forKey: .generation)
        images = try values.decodeIfPresent([LingXiaTerminalImage].self, forKey: .images) ?? []
        placements = try values.decodeIfPresent([LingXiaTerminalImagePlacement].self, forKey: .placements) ?? []
    }
}

private struct LingXiaTerminalImage: Decodable, Sendable {
    let id: UInt32
    let width: UInt32
    let height: UInt32
    let pngBase64: String
}

private struct LingXiaTerminalImagePlacement: Decodable, Sendable {
    let imageId: UInt32
    let placementId: UInt32
    let line: Int64
    let col: UInt16
    let columns: UInt16
    let rows: UInt16
    let xOffset: UInt16
    let yOffset: UInt16
    let sourceX: UInt32
    let sourceY: UInt32
    let sourceWidth: UInt32
    let sourceHeight: UInt32
    let zIndex: Int32
    let alternateScreen: Bool
    let virtualPlacement: Bool
}

private struct LingXiaTerminalSearchMatch: Decodable, Sendable {
    let startLine: Int64
    let startCol: UInt16
    let endLine: Int64
    let endCol: UInt16
}

private struct LingXiaTerminalSearchResults: Decodable, Sendable {
    let matches: [LingXiaTerminalSearchMatch]
    let total: UInt64
    let truncated: Bool
    let cancelled: Bool
}

@MainActor
private final class LingXiaTerminalImagePreviewController: NSWindowController {
    init(image: NSImage, title: String) {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 760, height: 560),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.isReleasedWhenClosed = false
        window.backgroundColor = .black
        let imageView = NSImageView(frame: window.contentView?.bounds ?? .zero)
        imageView.autoresizingMask = [.width, .height]
        imageView.image = image
        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.wantsLayer = true
        imageView.layer?.backgroundColor = NSColor.black.cgColor
        window.contentView = imageView
        super.init(window: window)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

struct LingXiaTerminalGridPoint: Equatable {
    var row: Int
    var col: Int
}

private extension NSPasteboard.PasteboardType {
    static let lxTerminalPane = NSPasteboard.PasteboardType("dev.lingxia.terminal-pane")
}

@MainActor
private final class LingXiaTerminalPaneDragHandleView: NSView {
    var onBeginDrag: ((NSEvent) -> Void)?
    var dragEnabled = false {
        didSet {
            if !dragEnabled {
                mouseDownEvent = nil
                mouseDownLocation = nil
            }
            isHidden = !dragEnabled
            needsDisplay = true
        }
    }
    private var trackingArea: NSTrackingArea?
    private var hovered = false
    private var mouseDownEvent: NSEvent?
    private var mouseDownLocation: NSPoint?

    private static let dragThreshold: CGFloat = 4

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea {
            removeTrackingArea(trackingArea)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseEnteredAndExited, .mouseMoved, .cursorUpdate],
            owner: self
        )
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        hovered = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        hovered = false
        needsDisplay = true
    }

    override func cursorUpdate(with event: NSEvent) {
        NSCursor.openHand.set()
    }

    override func mouseDown(with event: NSEvent) {
        guard dragEnabled else { return }
        mouseDownEvent = event
        mouseDownLocation = convert(event.locationInWindow, from: nil)
    }

    override func mouseDragged(with event: NSEvent) {
        guard dragEnabled, let mouseDownEvent, let mouseDownLocation else { return }
        let current = convert(event.locationInWindow, from: nil)
        guard hypot(current.x - mouseDownLocation.x, current.y - mouseDownLocation.y)
            >= Self.dragThreshold else { return }
        self.mouseDownEvent = nil
        self.mouseDownLocation = nil
        onBeginDrag?(mouseDownEvent)
    }

    override func mouseUp(with event: NSEvent) {
        mouseDownEvent = nil
        mouseDownLocation = nil
    }

    override func draw(_ dirtyRect: NSRect) {
        guard dragEnabled else { return }
        if hovered {
            NSColor.lxTerminalChromeRaised.withAlphaComponent(0.94).setFill()
            NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 8, yRadius: 8).fill()
        }
        (hovered ? NSColor.lxTerminalForeground.withAlphaComponent(0.72) : NSColor.lxTerminalDivider).setFill()
        let diameter: CGFloat = 3
        let spacing: CGFloat = 4
        let totalWidth = diameter * 3 + spacing * 2
        let origin = NSPoint(x: (bounds.width - totalWidth) / 2, y: (bounds.height - diameter) / 2)
        for index in 0..<3 {
            NSBezierPath(
                ovalIn: NSRect(
                    x: origin.x + CGFloat(index) * (diameter + spacing),
                    y: origin.y,
                    width: diameter,
                    height: diameter
                )
            ).fill()
        }

    }
}

@MainActor
private final class LingXiaTerminalPaneCloseButton: NSButton {
    private var hovered = false {
        didSet { updateAppearance() }
    }
    private var trackingArea_: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        isBordered = false
        bezelStyle = .regularSquare
        imagePosition = .imageOnly
        imageScaling = .scaleProportionallyDown
        image = LxIcon.image(named: "icon_close_x", size: CGSize(width: 12, height: 12))
        toolTip = L10n.string("lx_common_close")
        setAccessibilityLabel(L10n.string("lx_common_close"))
        focusRingType = .none
        wantsLayer = true
        layer?.cornerRadius = 12
        updateAppearance()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea_ {
            removeTrackingArea(trackingArea_)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseEnteredAndExited, .inVisibleRect],
            owner: self
        )
        addTrackingArea(area)
        trackingArea_ = area
    }

    override func mouseEntered(with event: NSEvent) {
        hovered = true
    }

    override func mouseExited(with event: NSEvent) {
        hovered = false
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        nil
    }

    private func updateAppearance() {
        contentTintColor = hovered
            ? NSColor.lxTerminalForeground.withAlphaComponent(0.92)
            : NSColor.lxTerminalForeground.withAlphaComponent(0.52)
        layer?.backgroundColor = hovered
            ? NSColor.lxTerminalForeground.withAlphaComponent(0.16).cgColor
            : NSColor.lxTerminalForeground.withAlphaComponent(0.09).cgColor
    }
}

@MainActor
private final class LingXiaTerminalSearchBar: NSVisualEffectView, NSSearchFieldDelegate {
    var onSearch: ((String, Bool, Bool) -> Void)?
    var onNavigate: ((Int) -> Void)?
    var onClose: (() -> Void)?

    private let field = NSSearchField()
    private let caseButton = NSButton(title: "Aa", target: nil, action: nil)
    private let wordButton = NSButton(title: "ab", target: nil, action: nil)
    private let statusLabel = NSTextField(labelWithString: "")
    private lazy var previousButton = iconButton("chevron.up", toolTip: "Previous Match", action: #selector(previousMatch))
    private lazy var nextButton = iconButton("chevron.down", toolTip: "Next Match", action: #selector(nextMatch))
    private lazy var clearButton = iconButton("trash", toolTip: "Clear Search", action: #selector(clearSearch))
    private lazy var closeButton = iconButton("xmark", toolTip: "Close Search", action: #selector(closeSearch))

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        material = .popover
        blendingMode = .withinWindow
        state = .active
        wantsLayer = true
        layer?.cornerRadius = 11
        layer?.borderWidth = 1
        layer?.borderColor = NSColor.lxTerminalDivider.withAlphaComponent(0.65).cgColor
        shadow = NSShadow()
        shadow?.shadowBlurRadius = 14
        shadow?.shadowOffset = NSSize(width: 0, height: -4)
        shadow?.shadowColor = NSColor.black.withAlphaComponent(0.28)

        field.placeholderString = "Search"
        field.delegate = self
        field.sendsSearchStringImmediately = true
        field.target = self
        field.action = #selector(searchChanged)

        configureToggle(caseButton, toolTip: "Match Case")
        configureToggle(wordButton, toolTip: "Match Whole Word")
        statusLabel.textColor = .secondaryLabelColor
        statusLabel.alignment = .center
        statusLabel.font = .monospacedDigitSystemFont(ofSize: 11, weight: .medium)

        let views: [NSView] = [
            field, caseButton, wordButton, statusLabel,
            previousButton, nextButton, clearButton, closeButton,
        ]
        for view in views {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        let minimumFieldWidth = field.widthAnchor.constraint(greaterThanOrEqualToConstant: 130)
        minimumFieldWidth.priority = .defaultHigh
        NSLayoutConstraint.activate([
            field.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            field.centerYAnchor.constraint(equalTo: centerYAnchor),
            minimumFieldWidth,
            caseButton.leadingAnchor.constraint(equalTo: field.trailingAnchor, constant: 8),
            caseButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            wordButton.leadingAnchor.constraint(equalTo: caseButton.trailingAnchor, constant: 4),
            wordButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            statusLabel.leadingAnchor.constraint(equalTo: wordButton.trailingAnchor, constant: 5),
            statusLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            previousButton.leadingAnchor.constraint(equalTo: statusLabel.trailingAnchor, constant: 4),
            previousButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            nextButton.leadingAnchor.constraint(equalTo: previousButton.trailingAnchor, constant: 2),
            nextButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            clearButton.leadingAnchor.constraint(equalTo: nextButton.trailingAnchor, constant: 7),
            clearButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            closeButton.leadingAnchor.constraint(equalTo: clearButton.trailingAnchor, constant: 4),
            closeButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            closeButton.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        setResult(active: nil, total: 0)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    var identity: (query: String, caseSensitive: Bool, wholeWord: Bool) {
        (field.stringValue, caseButton.state == .on, wordButton.state == .on)
    }

    func focus() {
        window?.makeFirstResponder(field)
        field.selectText(nil)
    }

    func setResult(active: Int?, total: UInt64) {
        if field.stringValue.isEmpty {
            statusLabel.stringValue = ""
        } else if let active, total > 0 {
            statusLabel.stringValue = "\(active + 1)/\(total)"
        } else {
            statusLabel.stringValue = "0/0"
        }
        previousButton.isEnabled = total > 0
        nextButton.isEnabled = total > 0
        clearButton.isEnabled = !field.stringValue.isEmpty
    }

    func controlTextDidChange(_ notification: Notification) {
        submit()
    }

    @objc private func searchChanged() {
        submit()
    }

    @objc private func toggleOption() {
        submit()
    }

    @objc private func previousMatch() {
        onNavigate?(-1)
    }

    @objc private func nextMatch() {
        onNavigate?(1)
    }

    @objc private func clearSearch() {
        field.stringValue = ""
        submit()
        focus()
    }

    @objc private func closeSearch() {
        onClose?()
    }

    private func submit() {
        let identity = identity
        onSearch?(identity.query, identity.caseSensitive, identity.wholeWord)
    }

    private func configureToggle(_ button: NSButton, toolTip: String) {
        button.setButtonType(.toggle)
        button.bezelStyle = .accessoryBarAction
        button.controlSize = .small
        button.toolTip = toolTip
        button.target = self
        button.action = #selector(toggleOption)
        button.widthAnchor.constraint(equalToConstant: 34).isActive = true
    }

    private func iconButton(_ symbol: String, toolTip: String, action: Selector) -> NSButton {
        let button = NSButton(image: NSImage(systemSymbolName: symbol, accessibilityDescription: toolTip) ?? NSImage(), target: self, action: action)
        button.bezelStyle = .accessoryBarAction
        button.controlSize = .small
        button.toolTip = toolTip
        button.imageScaling = .scaleProportionallyDown
        button.widthAnchor.constraint(equalToConstant: 28).isActive = true
        return button
    }
}

@MainActor
private final class LingXiaTerminalPaneDropOverlay: NSView {
    override func draw(_ dirtyRect: NSRect) {
        NSColor.controlAccentColor.withAlphaComponent(0.12).setFill()
        bounds.fill()
        NSColor.controlAccentColor.withAlphaComponent(0.95).setStroke()
        let path = NSBezierPath(rect: bounds.insetBy(dx: 1.5, dy: 1.5))
        path.lineWidth = 3
        path.stroke()
    }
}

@MainActor
final class LingXiaTerminalPaneView: NSView, NSDraggingSource {
    let paneID = UUID()
    var onActivated: ((UUID) -> Void)?
    var onSplitRequested: ((UUID, LingXiaTerminalSplitDirection) -> Void)?
    var onZoomRequested: ((UUID) -> Void)?
    var onTitleChanged: ((UUID, String?, String?) -> Void)?
    var onManualTitleChanged: ((UUID, String) -> Void)?
    var onTitleEditRequested: ((UUID) -> Void)?
    var onExited: ((UUID) -> Void)?
    var onCloseRequested: ((UUID) -> Void)?
    var onPaneMoveRequested: ((UUID, UUID, LingXiaTerminalSplitDirection) -> Bool)?

    private let terminalView = LingXiaTerminalCanvasView()
    private let dragHandle = LingXiaTerminalPaneDragHandleView()
    private let closeButton = LingXiaTerminalPaneCloseButton()
    private let searchBar = LingXiaTerminalSearchBar()
    private let dropOverlay = LingXiaTerminalPaneDropOverlay()
    private let session: LingXiaPTYTerminalSession
    private var font = LingXiaTerminalFont.regular()
    private var dropDirection: LingXiaTerminalSplitDirection?
    private var searchResults = LingXiaTerminalSearchResults(matches: [], total: 0, truncated: false, cancelled: false)
    private var activeSearchMatch: Int?
    private var paneTrackingArea: NSTrackingArea?
    private var paneControlsEnabled = false
    private var paneHovered = false

    init(initialDirectory: String? = nil) {
        self.session = LingXiaPTYTerminalSession(initialDirectory: initialDirectory)
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
        registerForDraggedTypes([.lxTerminalPane])

        dragHandle.onBeginDrag = { [weak self] event in
            self?.beginPaneDrag(with: event)
        }
        closeButton.target = self
        closeButton.action = #selector(closePane)

        searchBar.onSearch = { [weak self] query, caseSensitive, wholeWord in
            self?.performSearch(query: query, caseSensitive: caseSensitive, wholeWord: wholeWord)
        }
        searchBar.onNavigate = { [weak self] delta in
            self?.navigateSearch(delta: delta)
        }
        searchBar.onClose = { [weak self] in
            self?.hideSearch()
        }

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
        terminalView.onResize = { [weak self] cols, rows, cellWidth, cellHeight in
            self?.session.resize(
                cols: cols,
                rows: rows,
                cellWidth: cellWidth,
                cellHeight: cellHeight
            )
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
            self?.terminalView.clearImages()
            self?.session.restart()
        }
        terminalView.onTitleEditRequested = { [weak self] in
            guard let self else { return }
            self.onTitleEditRequested?(self.paneID)
        }
        terminalView.onSearchRequested = { [weak self] in
            self?.showSearch()
        }

        session.onUpdate = { [weak self] frame, images in
            if let images {
                self?.terminalView.applyImages(images)
            }
            self?.terminalView.applyFrame(frame)
        }
        session.onConfigChanged = { [weak self] in
            Task { @MainActor [weak self] in
                self?.applySettings(LingXiaTerminalSettings.current())
            }
        }
        session.onTitles = { [weak self] processTitle, title in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.onTitleChanged?(self.paneID, processTitle, title)
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

    func setPaneDragEnabled(_ enabled: Bool) {
        paneControlsEnabled = enabled
        dragHandle.dragEnabled = enabled
        updatePaneControlsVisibility()
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

    func shouldPreserveFocus(for event: NSEvent) -> Bool {
        guard !searchBar.isHidden else { return false }
        return searchBar.frame.contains(convert(event.locationInWindow, from: nil))
    }

    func currentWorkingDirectory() -> String? {
        session.currentWorkingDirectory()
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

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let paneTrackingArea {
            removeTrackingArea(paneTrackingArea)
        }
        let area = NSTrackingArea(
            rect: paneControlsHoverRect,
            options: [.activeInKeyWindow, .mouseEnteredAndExited],
            owner: self
        )
        addTrackingArea(area)
        paneTrackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        paneHovered = true
        updatePaneControlsVisibility()
    }

    override func mouseExited(with event: NSEvent) {
        paneHovered = false
        updatePaneControlsVisibility()
    }

    override func layout() {
        super.layout()
        dragHandle.frame = NSRect(
            x: max(0, (bounds.width - 34) / 2),
            y: max(0, bounds.height - 23),
            width: min(34, bounds.width),
            height: min(20, bounds.height)
        )
        closeButton.frame = NSRect(
            x: max(0, bounds.width - 30),
            y: max(0, bounds.height - 27),
            width: min(24, bounds.width),
            height: min(24, bounds.height)
        )
        let searchWidth = min(410, max(320, bounds.width - 24))
        searchBar.frame = NSRect(
            x: max(12, bounds.width - searchWidth - 12),
            y: max(12, bounds.height - 75),
            width: searchWidth,
            height: 46
        )
        updateDropOverlayFrame()
    }

    private var paneControlsHoverRect: NSRect {
        let height = min(32, bounds.height)
        return NSRect(x: 0, y: bounds.height - height, width: bounds.width, height: height)
    }

    override func keyDown(with event: NSEvent) {
        if !consumeKeyDown(event, source: "pane") {
            lxTerminalLog("pane.keyDown pass pane=\(paneID.uuidString) keyCode=\(event.keyCode)")
            super.keyDown(with: event)
        }
    }

    /// Re-apply the chrome colors this view's layers are holding a copy of.
    /// Layer colors are snapshots, so a scheme change has to walk them.
    func refreshChromeColors() {
        layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        terminalView.layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        session.refreshFrame()
        terminalView.refreshVisualColors()
        needsDisplay = true
    }

    func automationSnapshot(in root: NSView, active: Bool) -> [String: Any] {
        let rect = convert(bounds, to: root)
        return [
            "paneId": paneID.uuidString,
            "active": active,
            "visible": !isHidden && rect.width > 1 && rect.height > 1,
            "frame": [
                "x": rect.minX,
                "y": rect.minY,
                "width": rect.width,
                "height": rect.height,
            ],
            "grid": terminalView.automationSnapshot(),
        ]
    }

    private func setupTerminalView() {
        terminalView.translatesAutoresizingMaskIntoConstraints = false
        applySettings(LingXiaTerminalSettings.load())
    }

    /// Adopt a configuration. The theme is already in effect — the engine
    /// applies it — so only what the platform draws is set here.
    private func applySettings(_ settings: LingXiaTerminalSettings) {
        font = settings.makeFont()
        terminalView.font = font
        terminalView.lineHeightScale = settings.font.lineHeight
        terminalView.ligatures = settings.font.ligatures
    }

    func showContextMenu(fromWindowEvent event: NSEvent) {
        terminalView.showContextMenu(fromWindowEvent: event)
    }

    private func setupLayout() {
        addSubview(terminalView)
        dropOverlay.isHidden = true
        dropOverlay.wantsLayer = true
        addSubview(dropOverlay, positioned: .above, relativeTo: terminalView)
        searchBar.isHidden = true
        addSubview(searchBar, positioned: .above, relativeTo: dropOverlay)
        dragHandle.isHidden = true
        addSubview(dragHandle, positioned: .above, relativeTo: searchBar)
        closeButton.isHidden = true
        addSubview(closeButton, positioned: .above, relativeTo: dragHandle)
        NSLayoutConstraint.activate([
            terminalView.topAnchor.constraint(equalTo: topAnchor),
            terminalView.leadingAnchor.constraint(equalTo: leadingAnchor),
            terminalView.trailingAnchor.constraint(equalTo: trailingAnchor),
            terminalView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @objc private func closePane() {
        onCloseRequested?(paneID)
    }

    private func updatePaneControlsVisibility() {
        let visible = paneControlsEnabled && paneHovered
        dragHandle.isHidden = !visible
        closeButton.isHidden = !visible
    }

    private func beginPaneDrag(with event: NSEvent) {
        let pasteboardItem = NSPasteboardItem()
        pasteboardItem.setString(paneID.uuidString, forType: .lxTerminalPane)
        let draggingItem = NSDraggingItem(pasteboardWriter: pasteboardItem)
        let imageSize = NSSize(width: 32, height: 18)
        let image = NSImage(size: imageSize, flipped: false) { rect in
            NSColor.lxTerminalChromeRaised.withAlphaComponent(0.94).setFill()
            NSBezierPath(
                roundedRect: rect.insetBy(dx: 1, dy: 1),
                xRadius: 6,
                yRadius: 6
            ).fill()
            NSColor.lxTerminalAccent.withAlphaComponent(0.8).setFill()
            let diameter: CGFloat = 3
            let spacing: CGFloat = 4
            let totalWidth = diameter * 3 + spacing * 2
            for index in 0..<3 {
                NSBezierPath(
                    ovalIn: NSRect(
                        x: (rect.width - totalWidth) / 2 + CGFloat(index) * (diameter + spacing),
                        y: (rect.height - diameter) / 2,
                        width: diameter,
                        height: diameter
                    )
                ).fill()
            }
            return true
        }
        let point = convert(event.locationInWindow, from: nil)
        draggingItem.setDraggingFrame(
            NSRect(
                x: point.x - imageSize.width / 2,
                y: point.y - imageSize.height / 2,
                width: imageSize.width,
                height: imageSize.height
            ),
            contents: image
        )
        let session = beginDraggingSession(with: [draggingItem], event: event, source: self)
        session.animatesToStartingPositionsOnCancelOrFail = true
        session.draggingFormation = .none
        lxTerminalLog("pane.drag begin pane=\(paneID.uuidString)")
    }

    func draggingSession(
        _ session: NSDraggingSession,
        sourceOperationMaskFor context: NSDraggingContext
    ) -> NSDragOperation {
        .move
    }

    func ignoreModifierKeys(for session: NSDraggingSession) -> Bool {
        true
    }

    func draggingSession(
        _ session: NSDraggingSession,
        endedAt screenPoint: NSPoint,
        operation: NSDragOperation
    ) {
        lxTerminalLog("pane.drag end pane=\(paneID.uuidString) moved=\(operation == .move)")
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        updateDropTarget(sender)
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        updateDropTarget(sender)
    }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        clearDropTarget()
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        guard let sourceID = draggedPaneID(from: sender),
              sourceID != paneID,
              let dropDirection else {
            clearDropTarget()
            return false
        }
        clearDropTarget()
        let moved = onPaneMoveRequested?(sourceID, paneID, dropDirection) ?? false
        lxTerminalLog(
            "pane.drag drop source=\(sourceID.uuidString) target=\(paneID.uuidString) direction=\(dropDirection) moved=\(moved)"
        )
        return moved
    }

    private func updateDropTarget(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard let sourceID = draggedPaneID(from: sender),
              sourceID != paneID,
              onPaneMoveRequested != nil else {
            clearDropTarget()
            return []
        }
        let point = convert(sender.draggingLocation, from: nil)
        dropDirection = nearestDropDirection(to: point, keeping: dropDirection)
        dropOverlay.isHidden = dropDirection == nil
        if dropDirection != nil {
            updateDropOverlayFrame()
            return .move
        }
        return []
    }

    private func draggedPaneID(from sender: any NSDraggingInfo) -> UUID? {
        guard let value = sender.draggingPasteboard.string(forType: .lxTerminalPane) else {
            return nil
        }
        return UUID(uuidString: value)
    }

    private func nearestDropDirection(
        to point: NSPoint,
        keeping current: LingXiaTerminalSplitDirection?
    ) -> LingXiaTerminalSplitDirection? {
        let width = max(bounds.width, 1)
        let height = max(bounds.height, 1)
        let x = min(max(point.x / width, 0), 1)
        let y = min(max(point.y / height, 0), 1)
        let candidates: [(distance: CGFloat, direction: LingXiaTerminalSplitDirection)] = [
            (x, .left),
            (1 - x, .right),
            (1 - y, .up),
            (y, .down),
        ]
        guard let nearest = candidates.min(by: { $0.distance < $1.distance }) else {
            return nil
        }
        let zone: CGFloat = 0.35
        let hysteresis: CGFloat = 0.06
        if let current,
           let currentDistance = candidates.first(where: { $0.direction == current })?.distance,
           currentDistance <= zone + hysteresis,
           (nearest.distance > zone || nearest.distance + hysteresis >= currentDistance) {
            return current
        }
        return nearest.distance <= zone ? nearest.direction : nil
    }

    private func updateDropOverlayFrame() {
        guard let dropDirection else { return }
        switch dropDirection {
        case .left:
            dropOverlay.frame = NSRect(x: 0, y: 0, width: bounds.width / 2, height: bounds.height)
        case .right:
            dropOverlay.frame = NSRect(x: bounds.width / 2, y: 0, width: bounds.width / 2, height: bounds.height)
        case .up:
            dropOverlay.frame = NSRect(x: 0, y: bounds.height / 2, width: bounds.width, height: bounds.height / 2)
        case .down:
            dropOverlay.frame = NSRect(x: 0, y: 0, width: bounds.width, height: bounds.height / 2)
        }
        dropOverlay.needsDisplay = true
    }

    private func clearDropTarget() {
        dropDirection = nil
        dropOverlay.isHidden = true
    }

    private func appendOutput(_ output: String) {
        terminalView.append(output)
    }

    private func showSearch() {
        searchBar.isHidden = false
        // A context menu restores its previous responder while dismissing. Focus
        // on the next run-loop turn so Find consistently owns keyboard input.
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.searchBar.isHidden else { return }
            self.searchBar.focus()
        }
    }

    private func hideSearch() {
        searchBar.isHidden = true
        searchResults = LingXiaTerminalSearchResults(matches: [], total: 0, truncated: false, cancelled: false)
        activeSearchMatch = nil
        terminalView.applySearch(matches: [], active: nil)
        focusTerminal()
    }

    private func performSearch(query: String, caseSensitive: Bool, wholeWord: Bool) {
        guard !query.isEmpty else {
            searchResults = LingXiaTerminalSearchResults(matches: [], total: 0, truncated: false, cancelled: false)
            activeSearchMatch = nil
            terminalView.applySearch(matches: [], active: nil)
            searchBar.setResult(active: nil, total: 0)
            return
        }
        session.search(query: query, caseSensitive: caseSensitive, wholeWord: wholeWord) { [weak self] identity, results in
            guard let self, self.searchBar.identity == identity else { return }
            self.searchResults = results
            self.activeSearchMatch = results.matches.isEmpty ? nil : 0
            self.applyActiveSearchMatch()
        }
    }

    private func navigateSearch(delta: Int) {
        guard !searchResults.matches.isEmpty else { return }
        let current = activeSearchMatch ?? 0
        activeSearchMatch = (current + delta + searchResults.matches.count) % searchResults.matches.count
        applyActiveSearchMatch()
    }

    private func applyActiveSearchMatch() {
        searchBar.setResult(active: activeSearchMatch, total: searchResults.total)
        terminalView.applySearch(matches: searchResults.matches, active: activeSearchMatch)
        if let activeSearchMatch {
            session.scroll(toLine: searchResults.matches[activeSearchMatch].startLine)
        }
    }


}

@MainActor
private final class LingXiaTerminalCanvasView: NSView, @MainActor NSTextInputClient {
    var onInput: ((String) -> Void)?
    var onActivated: (() -> Void)?
    var onSplitRequested: ((LingXiaTerminalSplitDirection) -> Void)?
    var onZoomRequested: (() -> Void)?
    var onResize: ((UInt16, UInt16, UInt16, UInt16) -> Void)?
    var onScroll: ((Int, UInt16, UInt16, Bool) -> Void)?
    var onResetRequested: (() -> Void)?
    var onTitleEditRequested: (() -> Void)?
    var onSearchRequested: (() -> Void)?
    var zoomed = false

    var font = LingXiaTerminalFont.regular() {
        didSet {
            recalculateGridSize()
            setNeedsRender()
        }
    }

    /// Multiplier on the font's natural line height, from configuration.
    var lineHeightScale: CGFloat = 1 {
        didSet {
            recalculateGridSize()
            setNeedsRender()
        }
    }

    private var cols = 120
    private var rows = 32
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
    private var markedText = NSMutableAttributedString()
    private var markedTextSelection = NSRange(location: 0, length: 0)
    private var keyTextAccumulator: [String]?
    private let renderer = LingXiaTerminalMetalRenderer()
    private var frame_: LingXiaTerminalGPUFrame?
    private var terminalImages: [UInt32: CGImage] = [:]
    private var imagePlacements: [LingXiaTerminalImagePlacement] = []
    private var imagePreviewController: LingXiaTerminalImagePreviewController?
    private var pressedImageID: UInt32?
    private var searchMatches: [LingXiaTerminalSearchMatch] = []
    private var activeSearchMatch: Int?
    private var renderScheduled = false
    /// Shape runs with the font's ligatures; comes from configuration.
    var ligatures = true {
        didSet { setNeedsRender() }
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layerContentsRedrawPolicy = .onSetNeedsDisplay
        if renderer == nil {
            LXLog.error("terminal GPU renderer unavailable", category: "MacTerminal")
            layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        }
    }

    /// The grid is drawn by Metal, so the view's backing store *is* the
    /// drawable — there is no CoreGraphics pass over the cells at all.
    override func makeBackingLayer() -> CALayer {
        guard let renderer else { return super.makeBackingLayer() }
        let layer = renderer.makeLayer()
        layer.contentsScale = backingScale
        return layer
    }

    /// A custom backing layer makes this view *layer-hosting*: AppKit never
    /// calls `draw(_:)` or `updateLayer()` for it, so redraws are scheduled
    /// here instead of through `needsDisplay`.
    private func setNeedsRender() {
        guard !renderScheduled else { return }
        renderScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.renderScheduled = false
            self.renderGPUFrame()
        }
    }

    /// Theme chrome and the engine frame move on separate lightweight polls.
    /// Repaint the current frame immediately so cursor/selection colors do not
    /// wait for PTY input; the next engine frame then supplies the new palette.
    func refreshVisualColors() {
        needsDisplay = true
        setNeedsRender()
    }

    func automationSnapshot() -> [String: Any] {
        let frame = frame_ ?? LingXiaTerminalGPUFrame()
        return [
            "cols": cols,
            "rows": rows,
            "generation": frame.generation,
            "defaultForeground": frame.defaultForeground,
            "defaultBackground": frame.defaultBackground,
            "cursorRow": cursorRow,
            "cursorCol": cursorCol,
            "cursorVisible": cursorVisible,
            "cursorStyle": cursorStyle,
        ]
    }

    /// AppKit never calls this while the view is on screen — a layer-hosting
    /// view draws through its own layer. It runs only when something replays
    /// the view tree through CoreGraphics, which is exactly what window
    /// screenshot automation does, and which cannot see a Metal layer. Render
    /// the same frame offscreen so captures show the terminal rather than a
    /// blank rectangle.
    override func draw(_ dirtyRect: NSRect) {
        guard let renderer, let context = NSGraphicsContext.current?.cgContext else { return }
        guard let image = renderer.image(frame: frame_ ?? LingXiaTerminalGPUFrame(), context: renderContext()) else {
            return
        }
        context.draw(image, in: bounds)
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        setNeedsRender()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        setNeedsRender()
    }

    private var metalLayer: CAMetalLayer? { layer as? CAMetalLayer }

    /// Hand the engine's frame to the GPU. Nothing is converted per cell:
    /// the renderer walks the same buffers the engine produced.
    func applyFrame(_ frame: LingXiaTerminalGPUFrame) {
        cols = max(1, frame.cols)
        rows = max(1, frame.rows)
        cursorRow = frame.cursorRow
        cursorCol = frame.cursorCol
        cursorVisible = frame.cursorVisible
        cursorStyle = ["block", "bar", "underline", "block-hollow"][Int(min(frame.cursorStyle, 3))]
        applicationCursor = frame.applicationCursor
        bracketedPaste = frame.bracketedPaste
        alternateScreen = frame.alternateScreen
        if frame.scrollbarTotal > 0 {
            scrollbar = LingXiaTerminalScrollbar(
                total: frame.scrollbarTotal,
                offset: frame.scrollbarOffset,
                len: frame.scrollbarLen
            )
        } else {
            scrollbar = nil
        }
        frame_ = frame
        if hasMarkedText() {
            inputContext?.invalidateCharacterCoordinates()
        }
        setNeedsRender()
    }

    func applyImages(_ snapshot: LingXiaTerminalImageSnapshot) {
        guard snapshot.changed else { return }
        terminalImages = Dictionary(uniqueKeysWithValues: snapshot.images.compactMap { image in
            guard let data = Data(base64Encoded: image.pngBase64),
                  let decoded = NSImage(data: data) else { return nil }
            var rect = NSRect(origin: .zero, size: decoded.size)
            guard let cgImage = decoded.cgImage(forProposedRect: &rect, context: nil, hints: nil) else {
                return nil
            }
            return (image.id, cgImage)
        })
        imagePlacements = snapshot.placements.filter { terminalImages[$0.imageId] != nil }.sorted { lhs, rhs in
            if lhs.zIndex != rhs.zIndex { return lhs.zIndex < rhs.zIndex }
            if lhs.imageId != rhs.imageId { return lhs.imageId < rhs.imageId }
            return lhs.placementId < rhs.placementId
        }
        renderer?.updateImages(terminalImages)
        setNeedsRender()
    }

    func clearImages() {
        terminalImages.removeAll(keepingCapacity: false)
        imagePlacements.removeAll(keepingCapacity: false)
        renderer?.updateImages([:])
        setNeedsRender()
    }

    func applySearch(matches: [LingXiaTerminalSearchMatch], active: Int?) {
        searchMatches = matches
        activeSearchMatch = active
        setNeedsRender()
    }

    private func renderGPUFrame() {
        guard let renderer, let metalLayer else { return }
        let scale = backingScale
        metalLayer.contentsScale = scale
        let size = bounds.size
        guard size.width > 0, size.height > 0 else { return }
        metalLayer.drawableSize = CGSize(width: size.width * scale, height: size.height * scale)
        renderer.render(frame: frame_ ?? LingXiaTerminalGPUFrame(), context: renderContext(), in: metalLayer)
    }

    private func renderContext() -> LingXiaTerminalRenderContext {
        let scale = backingScale
        var context = LingXiaTerminalRenderContext(
            cellSize: charSize,
            baseline: terminalBaselineOffset(),
            font: font,
            scale: scale,
            viewSize: bounds.size
        )
        context.selection = selectionSpans()
        context.selectionColor = NSColor.lxTerminalSelectionBackground
        context.selectionForegroundColor = NSColor.lxTerminalSelectionForeground
        context.searchHighlights = searchHighlightSpans()
        context.cursorColor = NSColor.lxTerminalCursor
        context.drawCursor = window?.firstResponder === self && !hasMarkedText()
        context.markedText = markedText.string.isEmpty ? nil : markedText.string
        context.markedTextOrigin = LingXiaTerminalGridPoint(row: cursorRow, col: cursorCol)
        context.scrollbarColor = scrollbarVisible
            ? NSColor.lxTerminalForeground.withAlphaComponent(0.28)
            : NSColor.clear
        context.ligatures = ligatures
        context.images = visibleImagePlacements()
        return context
    }

    /// Selection as row spans, the form the renderer draws.
    private func selectionSpans() -> [(row: Int, startCol: Int, endCol: Int)] {
        guard let selection = normalizedSelection() else { return [] }
        var spans: [(row: Int, startCol: Int, endCol: Int)] = []
        for row in selection.start.row...selection.end.row where row >= 0 && row < rows {
            let startCol = row == selection.start.row ? selection.start.col : 0
            let endCol = row == selection.end.row ? selection.end.col : cols
            if endCol > startCol {
                spans.append((row: row, startCol: startCol, endCol: min(endCol, cols)))
            }
        }
        return spans
    }

    private func searchHighlightSpans() -> [(row: Int, startCol: Int, endCol: Int, active: Bool)] {
        guard let frame = frame_ else { return [] }
        let top = Int64(clamping: frame.scrollbarOffset)
        let bottom = top + Int64(frame.rows)
        var spans: [(row: Int, startCol: Int, endCol: Int, active: Bool)] = []
        for (index, match) in searchMatches.enumerated() {
            let first = max(match.startLine, top)
            let last = min(match.endLine, bottom - 1)
            guard first <= last else { continue }
            for line in first...last {
                let start = line == match.startLine ? Int(match.startCol) : 0
                let end = line == match.endLine ? Int(match.endCol) : frame.cols
                if end > start {
                    spans.append((
                        row: Int(line - top),
                        startCol: start,
                        endCol: min(end, frame.cols),
                        active: index == activeSearchMatch
                    ))
                }
            }
        }
        return spans
    }

    private func visibleImagePlacements() -> [LingXiaTerminalRenderImagePlacement] {
        guard let frame = frame_ else { return [] }
        let top = Int64(clamping: frame.scrollbarOffset)
        let physical = imagePlacements.compactMap { placement -> LingXiaTerminalRenderImagePlacement? in
            guard !placement.virtualPlacement else { return nil }
            let row = placement.line - top
            let visible = placement.alternateScreen == frame.alternateScreen
                && row < Int64(frame.rows)
                && row + Int64(placement.rows) > 0
            guard visible else { return nil }
            return LingXiaTerminalRenderImagePlacement(
                imageID: placement.imageId,
                rect: CGRect(
                    x: CGFloat(placement.col) * charSize.width + CGFloat(placement.xOffset) / backingScale,
                    y: CGFloat(row) * charSize.height + CGFloat(placement.yOffset) / backingScale,
                    width: CGFloat(placement.columns) * charSize.width,
                    height: CGFloat(placement.rows) * charSize.height
                ),
                source: CGRect(
                    x: CGFloat(placement.sourceX),
                    y: CGFloat(placement.sourceY),
                    width: CGFloat(placement.sourceWidth),
                    height: CGFloat(placement.sourceHeight)
                ),
                zIndex: placement.zIndex
            )
        }
        return (physical + unicodePlaceholderPlacements(frame: frame)).sorted { lhs, rhs in
            if lhs.zIndex != rhs.zIndex { return lhs.zIndex < rhs.zIndex }
            return lhs.imageID < rhs.imageID
        }
    }

    private struct PlaceholderCell {
        let imageID: UInt32
        let placementID: UInt32
        let imageRow: Int
        let imageCol: Int
    }

    private func unicodePlaceholderPlacements(
        frame: LingXiaTerminalGPUFrame
    ) -> [LingXiaTerminalRenderImagePlacement] {
        let prototypes = imagePlacements.filter(\.virtualPlacement)
        guard !prototypes.isEmpty else { return [] }
        var result: [LingXiaTerminalRenderImagePlacement] = []
        for row in 0..<frame.rows {
            var previous: PlaceholderCell?
            for col in 0..<frame.cols {
                guard let cell = frame.cell(row: row, col: col) else { continue }
                let scalars = Array(frame.clusterString(cell).unicodeScalars)
                guard scalars.first?.value == 0x10EEEE else {
                    previous = nil
                    continue
                }
                let diacritics = Array(scalars.dropFirst().prefix(3))
                let colorImageID = cell.fg >> 8
                let placementID = cell.underlineColor >> 8
                var imageRow = diacritics.indices.contains(0)
                    ? Self.placeholderDiacriticIndex(diacritics[0].value)
                    : nil
                var imageCol = diacritics.indices.contains(1)
                    ? Self.placeholderDiacriticIndex(diacritics[1].value)
                    : nil
                var highByte = diacritics.indices.contains(2)
                    ? Self.placeholderDiacriticIndex(diacritics[2].value)
                    : nil
                if let highByte, highByte > UInt8.max { continue }
                if let previous,
                   previous.imageID & 0x00FF_FFFF == colorImageID,
                   previous.placementID == placementID {
                    if imageRow == nil {
                        imageRow = previous.imageRow
                        imageCol = previous.imageCol + 1
                        highByte = Int(previous.imageID >> 24)
                    } else if imageCol == nil, imageRow == previous.imageRow {
                        imageCol = previous.imageCol + 1
                        highByte = Int(previous.imageID >> 24)
                    } else if highByte == nil,
                              imageRow == previous.imageRow,
                              imageCol == previous.imageCol + 1 {
                        highByte = Int(previous.imageID >> 24)
                    }
                }
                let resolvedRow = imageRow ?? 0
                let resolvedCol = imageCol ?? 0
                let imageID = colorImageID | UInt32(highByte ?? 0) << 24
                let decoded = PlaceholderCell(
                    imageID: imageID,
                    placementID: placementID,
                    imageRow: resolvedRow,
                    imageCol: resolvedCol
                )
                previous = decoded
                let prototype = prototypes.first {
                    $0.imageId == imageID
                        && (placementID == 0 || $0.placementId == placementID)
                }
                guard let prototype,
                      resolvedRow >= 0, resolvedCol >= 0,
                      resolvedRow < Int(prototype.rows),
                      resolvedCol < Int(prototype.columns) else { continue }
                let sourceWidth = CGFloat(prototype.sourceWidth) / CGFloat(prototype.columns)
                let sourceHeight = CGFloat(prototype.sourceHeight) / CGFloat(prototype.rows)
                result.append(LingXiaTerminalRenderImagePlacement(
                    imageID: imageID,
                    rect: CGRect(
                        x: CGFloat(col) * charSize.width,
                        y: CGFloat(row) * charSize.height,
                        width: charSize.width,
                        height: charSize.height
                    ),
                    source: CGRect(
                        x: CGFloat(prototype.sourceX) + CGFloat(resolvedCol) * sourceWidth,
                        y: CGFloat(prototype.sourceY) + CGFloat(resolvedRow) * sourceHeight,
                        width: sourceWidth,
                        height: sourceHeight
                    ),
                    zIndex: prototype.zIndex
                ))
            }
        }
        return result
    }

    private static func placeholderDiacriticIndex(_ scalar: UInt32) -> Int? {
        placeholderDiacriticIndices[scalar]
    }

    /// Kitty's canonical Unicode-placeholder row/column table. The complete
    /// table is required because terminal grids routinely exceed 30 cells.
    private static let placeholderDiacriticIndices: [UInt32: Int] = {
        let diacritics: [UInt32] = [
            0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F,
            0x0346, 0x034A, 0x034B, 0x034C, 0x0350, 0x0351, 0x0352, 0x0357,
            0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
            0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484,
            0x0485, 0x0486, 0x0487, 0x0592, 0x0593, 0x0594, 0x0595, 0x0597,
            0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
            0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611,
            0x0612, 0x0613, 0x0614, 0x0615, 0x0616, 0x0617, 0x0657, 0x0658,
            0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
            0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2,
            0x06E4, 0x06E7, 0x06E8, 0x06EB, 0x06EC, 0x0730, 0x0732, 0x0733,
            0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
            0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE,
            0x07EF, 0x07F0, 0x07F1, 0x07F3, 0x0816, 0x0817, 0x0818, 0x0819,
            0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
            0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C,
            0x082D, 0x0951, 0x0953, 0x0954, 0x0F82, 0x0F83, 0x0F86, 0x0F87,
            0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
            0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D,
            0x1B6E, 0x1B6F, 0x1B70, 0x1B71, 0x1B72, 0x1B73, 0x1CD0, 0x1CD1,
            0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
            0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1,
            0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5, 0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9,
            0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
            0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1,
            0x20D4, 0x20D5, 0x20D6, 0x20D7, 0x20DB, 0x20DC, 0x20E1, 0x20E7,
            0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
            0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA,
            0x2DEB, 0x2DEC, 0x2DED, 0x2DEE, 0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2,
            0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
            0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D,
            0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1, 0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5,
            0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
            0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7,
            0xAAB8, 0xAABE, 0xAABF, 0xAAC1, 0xFE20, 0xFE21, 0xFE22, 0xFE23,
            0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186, 0x1D187,
            0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243,
            0x1D244,
        ]
        return Dictionary(uniqueKeysWithValues: diacritics.enumerated().map { ($1, $0) })
    }()

    private func imageID(at point: NSPoint) -> UInt32? {
        let renderPoint = CGPoint(x: point.x, y: bounds.height - point.y)
        return visibleImagePlacements().reversed().first { $0.rect.contains(renderPoint) }?.imageID
    }

    private func preview(image: CGImage, imageID: UInt32) {
        let controller = LingXiaTerminalImagePreviewController(
            image: NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height)),
            title: "Terminal Image \(imageID)"
        )
        imagePreviewController = controller
        controller.showWindow(nil)
        controller.window?.center()
        controller.window?.makeKeyAndOrderFront(nil)
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
        setNeedsRender()
        return true
    }

    override func resignFirstResponder() -> Bool {
        lxTerminalLog("canvas.resignFirstResponder")
        setNeedsRender()
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
        let localPoint = convert(event.locationInWindow, from: nil)
        pressedImageID = imageID(at: localPoint)
        let point = gridPoint(for: localPoint)
        selectionAnchor = point
        selectionFocus = point
        setNeedsRender()
    }

    override func mouseDragged(with event: NSEvent) {
        pressedImageID = nil
        selectionFocus = gridPoint(for: convert(event.locationInWindow, from: nil))
        setNeedsRender()
    }

    override func mouseUp(with event: NSEvent) {
        defer { pressedImageID = nil }
        guard selectionAnchor == selectionFocus else { return }
        selectionAnchor = nil
        selectionFocus = nil
        setNeedsRender()
        let localPoint = convert(event.locationInWindow, from: nil)
        guard let pressedID = pressedImageID,
              imageID(at: localPoint) == pressedID,
              let image = terminalImages[pressedID] else { return }
        preview(image: image, imageID: pressedID)
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
        setNeedsRender()
        let point = gridPoint(for: convert(event.locationInWindow, from: nil))
        // gridPoint allows col == cols (selection end-of-line); mouse
        // reports need an in-range column.
        let reportCol = UInt16(min(point.col, max(0, cols - 1)))
        onScroll?(-wholeRows, reportCol, UInt16(point.row), !readOnly)
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
        if consumeEditingShortcut(event, source: source) { return true }
        guard !readOnly else {
            lxTerminalLog("\(source).keyDown ignoredReadOnly keyCode=\(event.keyCode)")
            return true
        }
        return sendMappedKeyDown(event, source: source)
    }

    override func keyDown(with event: NSEvent) {
        if consumeEditingShortcut(event, source: "canvas") { return }
        guard !readOnly else {
            lxTerminalLog("canvas.keyDown ignoredReadOnly keyCode=\(event.keyCode)")
            return
        }

        let markedTextBefore = hasMarkedText()
        keyTextAccumulator = []
        interpretKeyEvents([event])
        let committedText = keyTextAccumulator ?? []
        keyTextAccumulator = nil
        let composing = markedTextBefore || hasMarkedText()

        if !committedText.isEmpty {
            for text in committedText where !shouldSuppressComposingControlInput(text, composing: composing) {
                guard !text.isEmpty else { continue }
                lxTerminalLog("canvas.ime commit chars=\(text.count) bytes=\(text.utf8.count)")
                onInput?(text)
            }
            return
        }

        // While an input method owns the event, cursor movement, deletion, and
        // candidate selection must not leak through to the PTY.
        if composing { return }
        if !sendMappedKeyDown(event, source: "canvas") {
            super.keyDown(with: event)
        }
    }

    private func consumeEditingShortcut(_ event: NSEvent, source: String) -> Bool {
        guard event.modifierFlags.contains(.command) else { return false }
        switch Int(event.keyCode) {
        case 8:
            lxTerminalLog("\(source).keyDown commandCopy")
            copy(nil)
            return true
        case 9:
            lxTerminalLog("\(source).keyDown commandPaste")
            paste(nil)
            return true
        case 3:
            lxTerminalLog("\(source).keyDown commandFind")
            onSearchRequested?()
            return true
        default:
            return false
        }
    }

    private func sendMappedKeyDown(_ event: NSEvent, source: String) -> Bool {
        guard let input = LingXiaTerminalKeyMapper.input(for: event, applicationCursor: applicationCursor) else {
            lxTerminalLog("\(source).keyDown pass keyCode=\(event.keyCode)")
            return false
        }
        lxTerminalLog("\(source).keyDown input keyCode=\(event.keyCode) bytes=\(input.utf8.count) appCursor=\(applicationCursor)")
        onInput?(input)
        return true
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

    func hasMarkedText() -> Bool {
        markedText.length > 0
    }

    func markedRange() -> NSRange {
        guard hasMarkedText() else { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: 0, length: markedText.length)
    }

    func selectedRange() -> NSRange {
        hasMarkedText() ? markedTextSelection : NSRange(location: 0, length: 0)
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        switch string {
        case let value as NSAttributedString:
            markedText = NSMutableAttributedString(attributedString: value)
        case let value as String:
            markedText = NSMutableAttributedString(string: value)
        default:
            markedText = NSMutableAttributedString()
        }
        markedTextSelection = normalizedMarkedTextSelection(selectedRange)
        lxTerminalLog(
            "canvas.ime marked chars=\(markedText.string.count) selected=\(markedTextSelection.location):\(markedTextSelection.length)"
        )
        inputContext?.invalidateCharacterCoordinates()
        setNeedsRender()
    }

    func unmarkText() {
        guard hasMarkedText() else { return }
        markedText = NSMutableAttributedString()
        markedTextSelection = NSRange(location: 0, length: 0)
        lxTerminalLog("canvas.ime unmark")
        inputContext?.invalidateCharacterCoordinates()
        setNeedsRender()
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        [.underlineStyle, .markedClauseSegment]
    }

    func attributedSubstring(
        forProposedRange range: NSRange,
        actualRange: NSRangePointer?
    ) -> NSAttributedString? {
        guard hasMarkedText(),
              range.location != NSNotFound,
              range.location <= markedText.length,
              NSMaxRange(range) <= markedText.length else {
            actualRange?.pointee = NSRange(location: NSNotFound, length: 0)
            return nil
        }
        actualRange?.pointee = range
        return markedText.attributedSubstring(from: range)
    }

    func characterIndex(for point: NSPoint) -> Int {
        0
    }

    func firstRect(
        forCharacterRange range: NSRange,
        actualRange: NSRangePointer?
    ) -> NSRect {
        actualRange?.pointee = hasMarkedText() ? markedRange() : selectedRange()
        let x = pixelFloor(CGFloat(cursorCol) * charSize.width + markedTextCaretOffset())
        let y = pixelFloor(bounds.height - CGFloat(cursorRow + 1) * charSize.height)
        let localRect = NSRect(x: x, y: y, width: 0, height: charSize.height)
        let windowRect = convert(localRect, to: nil)
        return window?.convertToScreen(windowRect) ?? windowRect
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        let text: String
        switch string {
        case let value as NSAttributedString:
            text = value.string
        case let value as String:
            text = value
        default:
            return
        }

        unmarkText()
        if var accumulator = keyTextAccumulator {
            accumulator.append(text)
            keyTextAccumulator = accumulator
        } else if !readOnly, !text.isEmpty {
            lxTerminalLog("canvas.ime insert chars=\(text.count) bytes=\(text.utf8.count)")
            onInput?(text)
        }
    }

    override func doCommand(by selector: Selector) {
        // keyDown maps terminal commands after interpretKeyEvents returns. This
        // callback intentionally absorbs AppKit's command to avoid an NSBeep.
    }

    func append(_ output: String) {
        guard !output.isEmpty else { return }
        LXLog.error("terminal pane message: \(output.trimmingCharacters(in: .whitespacesAndNewlines))", category: "MacTerminal")
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
        let searchItem = menuItem("Search…", action: #selector(findFromMenu))
        searchItem.keyEquivalent = "f"
        menu.addItem(searchItem)
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

    @objc private func findFromMenu() {
        lxTerminalLog("canvas.menu find")
        onSearchRequested?()
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

    private func normalizedMarkedTextSelection(_ range: NSRange) -> NSRange {
        guard range.location != NSNotFound else {
            return NSRange(location: markedText.length, length: 0)
        }
        let location = min(max(0, range.location), markedText.length)
        let length = min(max(0, range.length), markedText.length - location)
        return NSRange(location: location, length: length)
    }

    private func shouldSuppressComposingControlInput(_ text: String, composing: Bool) -> Bool {
        guard composing else { return false }
        let scalars = text.unicodeScalars
        guard let scalar = scalars.first,
              scalars.index(after: scalars.startIndex) == scalars.endIndex else {
            return false
        }
        return scalar.value < 0x20
    }

    private func markedTextCaretOffset() -> CGFloat {
        guard hasMarkedText() else { return 0 }
        let text = markedText.string as NSString
        let location = min(markedTextSelection.location, text.length)
        let prefix = text.substring(to: location) as NSString
        return prefix.size(withAttributes: [.font: font]).width
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

    private func revealScrollbar() {
        scrollbarVisibilityToken &+= 1
        let token = scrollbarVisibilityToken
        scrollbarVisible = true
        setNeedsRender()
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(900)) { [weak self] in
            guard let self, self.scrollbarVisibilityToken == token else { return }
            self.scrollbarVisible = false
            self.setNeedsRender()
        }
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
        // Walk cells, not a flattened line: wide (CJK) glyphs are one
        // cluster over two columns, so slicing a string by column shears
        // mixed-width rows.
        guard let frame = frame_ else { return "" }
        var text = ""
        var col = startCol
        while col < min(endCol, frame.cols) {
            guard let cell = frame.cell(row: row, col: col) else { break }
            let span = max(Int(cell.columns), 1)
            if cell.textLen > 0 {
                text += frame.clusterString(cell)
            } else if cell.columns > 0 {
                text += " "
            }
            col += span
        }
        return text.trimmingCharacters(in: .whitespaces)
    }

    private func recalculateGridSize() {
        let sample = "W" as NSString
        let measured = sample.size(withAttributes: [.font: font])
        charSize = NSSize(
            // CoreText keeps the font's fractional advance when drawing a run.
            // Rounding the grid width accumulates visible drift on long boxes.
            width: max(1, measured.width),
            height: max(1, pixelCeil((font.ascender - font.descender + max(2, font.leading)) * lineHeightScale))
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
            let pixelWidth = UInt16(clamping: Int((charSize.width * backingScale).rounded()))
            let pixelHeight = UInt16(clamping: Int((charSize.height * backingScale).rounded()))
            onResize?(safeCols, safeRows, max(1, pixelWidth), max(1, pixelHeight))
        }
        setNeedsRender()
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

    private func terminalBaselineOffset() -> CGFloat {
        let glyphHeight = font.ascender - font.descender
        let centeredTopPadding = max(0, (charSize.height - glyphHeight) / 2)
        return pixelFloor(centeredTopPadding - font.descender)
    }

    private func appendRoundedCorner(
        _ scalar: UInt32,
        to path: NSBezierPath,
        left: CGFloat,
        right: CGFloat,
        bottom: CGFloat,
        top: CGFloat,
        centerX: CGFloat,
        centerY: CGFloat
    ) {
        let radius = min(2, (right - left) / 2, (top - bottom) / 2)
        switch scalar {
        case 0x256D: // ╭
            path.move(to: NSPoint(x: right, y: centerY))
            path.line(to: NSPoint(x: centerX + radius, y: centerY))
            path.appendArc(
                withCenter: NSPoint(x: centerX + radius, y: centerY - radius),
                radius: radius,
                startAngle: 90,
                endAngle: 180,
                clockwise: false
            )
            path.line(to: NSPoint(x: centerX, y: bottom))
        case 0x256E: // ╮
            path.move(to: NSPoint(x: left, y: centerY))
            path.line(to: NSPoint(x: centerX - radius, y: centerY))
            path.appendArc(
                withCenter: NSPoint(x: centerX - radius, y: centerY - radius),
                radius: radius,
                startAngle: 90,
                endAngle: 0,
                clockwise: true
            )
            path.line(to: NSPoint(x: centerX, y: bottom))
        case 0x256F: // ╯
            path.move(to: NSPoint(x: left, y: centerY))
            path.line(to: NSPoint(x: centerX - radius, y: centerY))
            path.appendArc(
                withCenter: NSPoint(x: centerX - radius, y: centerY + radius),
                radius: radius,
                startAngle: -90,
                endAngle: 0,
                clockwise: false
            )
            path.line(to: NSPoint(x: centerX, y: top))
        case 0x2570: // ╰
            path.move(to: NSPoint(x: right, y: centerY))
            path.line(to: NSPoint(x: centerX + radius, y: centerY))
            path.appendArc(
                withCenter: NSPoint(x: centerX + radius, y: centerY + radius),
                radius: radius,
                startAngle: -90,
                endAngle: -180,
                clockwise: true
            )
            path.line(to: NSPoint(x: centerX, y: top))
        default:
            break
        }
    }

}

private final class LingXiaPTYTerminalSession: @unchecked Sendable {
    private static let log = OSLog(subsystem: "LingXia", category: "MacTerminalPTY")

    var onUpdate: (@MainActor @Sendable (
        LingXiaTerminalGPUFrame,
        LingXiaTerminalImageSnapshot?
    ) -> Void)?
    var onTitles: ((String, String?) -> Void)?
    var onConfigChanged: (() -> Void)?
    var onError: ((String) -> Void)?
    var onExit: (() -> Void)?

    private let ioQueue = DispatchQueue(label: "app.lingxia.terminal.pty", qos: .userInitiated)
    private let decoder = JSONDecoder()
    private var sessionID: UInt64 = 0
    private var readTimer: DispatchSourceTimer?
    private var pendingInput = ""
    private var frameGeneration: UInt64 = 0
    private var imageGeneration: UInt64 = 0
    private var lastTitlePoll = DispatchTime.now()
    private var lastTitleState = ""
    private var lastConfigGeneration: UInt64 = 0
    private let initialDirectory: String?

    init(initialDirectory: String? = nil) {
        self.initialDirectory = initialDirectory
    }

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
        let id = terminalSessionCreate(120, 32, initialDirectory ?? "")
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

    func currentWorkingDirectory() -> String? {
        ioQueue.sync {
            guard sessionID != 0 else { return initialDirectory }
            let path = terminalSessionCurrentDirectory(sessionID).toString()
            return path.isEmpty ? initialDirectory : path
        }
    }

    func resize(cols: UInt16, rows: UInt16, cellWidth: UInt16, cellHeight: UInt16) {
        ioQueue.async { [weak self] in
            guard let self, self.sessionID != 0 else { return }
            let ok = terminalSessionResizePixels(self.sessionID, cols, rows, cellWidth, cellHeight)
            lxTerminalLogAsync("pty.resize session=\(self.sessionID) cols=\(cols) rows=\(rows) cell=\(cellWidth)x\(cellHeight) ok=\(ok)")
        }
    }

    func search(
        query: String,
        caseSensitive: Bool,
        wholeWord: Bool,
        completion: @escaping @MainActor @Sendable (
            (query: String, caseSensitive: Bool, wholeWord: Bool),
            LingXiaTerminalSearchResults
        ) -> Void
    ) {
        ioQueue.async { [weak self] in
            guard let self, self.sessionID != 0 else { return }
            let json = terminalSessionSearch(self.sessionID, query, caseSensitive, wholeWord).toString()
            guard let data = json.data(using: .utf8),
                  let results = try? self.decoder.decode(LingXiaTerminalSearchResults.self, from: data) else {
                return
            }
            let identity = (query, caseSensitive, wholeWord)
            DispatchQueue.main.async {
                completion(identity, results)
            }
        }
    }

    func scroll(toLine line: Int64) {
        ioQueue.async { [weak self] in
            guard let self, self.sessionID != 0 else { return }
            _ = terminalSessionScrollToLine(self.sessionID, line)
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

    func refreshFrame() {
        ioQueue.async { [weak self] in
            self?.drainOutputOnIOQueue()
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
        frameGeneration = 0
        imageGeneration = 0
        lastTitleState = ""
        if sessionID != 0 {
            lxTerminalLogAsync("pty.stop close session=\(sessionID)")
            terminalSessionClose(sessionID)
            sessionID = 0
        }
    }

    private func drainOutputOnIOQueue() {
        guard sessionID != 0 else { return }
        let id = sessionID
        // Frames come back as pointers into the engine's retained buffers:
        // no JSON, and a quiet poll allocates nothing at all.
        if let frame = LingXiaTerminalFrameSource.poll(sessionID: id, since: frameGeneration) {
            frameGeneration = frame.generation
            var images: LingXiaTerminalImageSnapshot?
            if frame.imageGeneration != imageGeneration {
                let json = terminalSessionImageSnapshot(id, imageGeneration).toString()
                if let data = json.data(using: .utf8),
                   let snapshot = try? decoder.decode(LingXiaTerminalImageSnapshot.self, from: data) {
                    imageGeneration = snapshot.generation
                    images = snapshot
                }
            }
            emit(frame, images: images)
            if frame.exited {
                lxTerminalLogAsync("pty.exited session=\(id)")
                stopOnIOQueue()
                emitExit()
                return
            }
        } else if LingXiaTerminalFrameSource.hasExited(sessionID: id, since: frameGeneration) {
            lxTerminalLogAsync("pty.exited session=\(id)")
            stopOnIOQueue()
            emitExit()
            return
        }
        pollTitlesOnIOQueue(id)
    }

    /// Titles resolve the foreground process, which costs syscalls — far too
    /// expensive for the frame cadence, and they change at human speed anyway.
    ///
    /// The configuration generation rides along: the engine watches the file
    /// and applies the theme itself, so all that is left is noticing that the
    /// font may have changed — one atomic read on a poll that already runs,
    /// rather than a watch implemented again per platform.
    private func pollTitlesOnIOQueue(_ id: UInt64) {
        let now = DispatchTime.now()
        guard now.uptimeNanoseconds &- lastTitlePoll.uptimeNanoseconds >= 250_000_000 else { return }
        lastTitlePoll = now

        let generation = terminalConfigGeneration()
        if generation != lastConfigGeneration {
            lastConfigGeneration = generation
            DispatchQueue.main.async { [onConfigChanged] in
                onConfigChanged?()
            }
        }
        let json = terminalSessionTitleState(id).toString()
        guard json != lastTitleState, let data = json.data(using: .utf8) else { return }
        lastTitleState = json
        guard let state = try? decoder.decode(LingXiaTerminalTitleState.self, from: data) else { return }
        let titles = (state.processTitle, state.title)
        DispatchQueue.main.async { [onTitles] in
            onTitles?(titles.0, titles.1)
        }
    }

    private func emit(
        _ frame: LingXiaTerminalGPUFrame,
        images: LingXiaTerminalImageSnapshot?
    ) {
        DispatchQueue.main.async { [onUpdate] in
            onUpdate?(frame, images)
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
