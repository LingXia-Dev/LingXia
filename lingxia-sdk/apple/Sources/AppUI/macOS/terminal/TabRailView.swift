#if os(macOS)
import AppKit

@MainActor
private final class LingXiaTerminalInlineTitleTextView: NSTextView {
    var onCommit: ((String) -> Void)?
    var onCancel: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        configure()
    }

    override init(frame frameRect: NSRect, textContainer container: NSTextContainer?) {
        super.init(frame: frameRect, textContainer: container)
        configure()
    }

    private func configure() {
        drawsBackground = true
        backgroundColor = .lxTerminalBackground
        insertionPointColor = NSColor.white.withAlphaComponent(0.95)
        textColor = NSColor.white.withAlphaComponent(0.96)
        font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        isRichText = false
        isAutomaticQuoteSubstitutionEnabled = false
        isAutomaticDashSubstitutionEnabled = false
        isAutomaticTextReplacementEnabled = false
        isHorizontallyResizable = false
        isVerticallyResizable = false
        textContainerInset = NSSize(width: 0, height: 1)
        textContainer?.lineFragmentPadding = 0
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var acceptsFirstResponder: Bool { true }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36 || event.keyCode == 76 {
            onCommit?(string)
            return
        }
        if event.keyCode == 53 {
            onCancel?()
            return
        }
        super.keyDown(with: event)
    }

    override func insertNewline(_ sender: Any?) {
        onCommit?(string)
    }

    override func insertTab(_ sender: Any?) {
        onCommit?(string)
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        onCommit?(string)
        return result
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        nil
    }
}

@MainActor
final class LingXiaTerminalTabRailView: NSView {
    struct Item: Equatable {
        let id: UUID
        let title: String
        let subtitle: String?
        let active: Bool
    }

    var items: [Item] = [] {
        didSet {
            if let editingID, !items.contains(where: { $0.id == editingID }) {
                cancelEditing()
            }
            if editingID != nil {
                invalidateIntrinsicContentSize()
                needsDisplay = true
                needsLayout = true
                return
            }
            rebuildTabs()
            invalidateIntrinsicContentSize()
            needsDisplay = true
            needsLayout = true
        }
    }

    var onSelect: ((UUID) -> Void)?
    var onRenameRequest: ((UUID) -> Void)?
    var onClose: ((UUID) -> Void)?
    var onNewTab: (() -> Void)?
    var onToggleSurfaceZoom: (() -> Void)?
    var onCommitTitle: ((UUID, String) -> Void)?
    var isSurfaceZoomed: Bool = false {
        didSet {
            zoomButton.setZoomed(isSurfaceZoomed)
        }
    }

    private let stackView = NSStackView()
    private let zoomButton = LingXiaTerminalZoomButton()
    private let addButton = LingXiaTerminalAddTabButton()
    private let titleEditor = LingXiaTerminalInlineTitleTextView(frame: .zero)
    private var tabViews: [UUID: LingXiaTerminalTabChromeView] = [:]
    private var editingID: UUID?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        layer?.backgroundColor = NSColor.lxTerminalChrome.cgColor

        stackView.orientation = .horizontal
        stackView.alignment = .centerY
        stackView.distribution = .fill
        stackView.spacing = 2
        stackView.edgeInsets = NSEdgeInsets(top: 2, left: 0, bottom: 0, right: 6)
        stackView.translatesAutoresizingMaskIntoConstraints = false
        stackView.setContentHuggingPriority(.defaultLow, for: .horizontal)
        stackView.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        addSubview(stackView)

        titleEditor.translatesAutoresizingMaskIntoConstraints = true
        titleEditor.isHidden = true
        titleEditor.wantsLayer = true
        titleEditor.layer?.zPosition = 100
        titleEditor.onCommit = { [weak self] text in
            self?.commitTitleEditor(text)
        }
        titleEditor.onCancel = { [weak self] in
            self?.cancelEditing()
        }
        addSubview(titleEditor, positioned: .above, relativeTo: stackView)

        zoomButton.target = self
        zoomButton.action = #selector(toggleSurfaceZoom)
        zoomButton.setContentHuggingPriority(.required, for: .horizontal)
        zoomButton.setContentCompressionResistancePriority(.required, for: .horizontal)
        zoomButton.widthAnchor.constraint(equalToConstant: 22).isActive = true
        zoomButton.heightAnchor.constraint(equalToConstant: 22).isActive = true

        addButton.target = self
        addButton.action = #selector(addTab)
        addButton.setContentHuggingPriority(.required, for: .horizontal)
        addButton.setContentCompressionResistancePriority(.required, for: .horizontal)
        addButton.widthAnchor.constraint(equalToConstant: 22).isActive = true
        addButton.heightAnchor.constraint(equalToConstant: 22).isActive = true

        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: topAnchor),
            stackView.leadingAnchor.constraint(equalTo: leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: trailingAnchor),
            stackView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var intrinsicContentSize: NSSize {
        NSSize(width: NSView.noIntrinsicMetric, height: 34)
    }

    override func layout() {
        super.layout()
        if let editingID {
            positionTitleEditor(for: editingID)
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.lxTerminalChrome.setFill()
        bounds.fill()
        let separatorHeight = 1 / max(1, window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2)
        NSColor.lxTerminalBorder.withAlphaComponent(0.16).setFill()
        NSRect(x: 0, y: 0, width: bounds.width, height: separatorHeight).fill()
    }

    override func rightMouseDown(with event: NSEvent) {
        // Keep the tab strip inert on right click; terminal context actions belong to the pane.
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        nil
    }

    func beginEditing(tabID: UUID) {
        guard let item = items.first(where: { $0.id == tabID }) else {
            return
        }
        editingID = tabID
        titleEditor.string = item.title.isEmpty ? "terminal" : item.title
        titleEditor.isHidden = false
        tabViews.values.forEach { $0.setEditing(false) }
        tabViews[tabID]?.setEditing(true)
        needsDisplay = true
        layoutSubtreeIfNeeded()
        positionTitleEditor(for: tabID)
        window?.makeFirstResponder(titleEditor)
        titleEditor.setSelectedRange(NSRange(location: 0, length: titleEditor.string.utf16.count))
        DispatchQueue.main.async { [weak self] in
            guard let self, self.editingID == tabID else { return }
            self.positionTitleEditor(for: tabID)
            self.window?.makeFirstResponder(self.titleEditor)
            self.titleEditor.setSelectedRange(NSRange(location: 0, length: self.titleEditor.string.utf16.count))
        }
    }

    var isEditingTitle: Bool {
        editingID != nil
    }

    private func rebuildTabs() {
        stackView.arrangedSubviews.forEach {
            stackView.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        tabViews.removeAll(keepingCapacity: true)

        for item in items {
            let tabView = LingXiaTerminalTabChromeView(item: item)
            tabView.onSelect = { [weak self] id in
                self?.onSelect?(id)
            }
            tabView.onRenameRequest = { [weak self] id in
                self?.onRenameRequest?(id)
            }
            tabView.onClose = { [weak self] id in
                self?.onClose?(id)
            }
            let minimumWidth = tabView.widthAnchor.constraint(greaterThanOrEqualToConstant: 122)
            minimumWidth.priority = .defaultHigh
            minimumWidth.isActive = true
            if items.count > 1 {
                let preferredWidth = tabView.widthAnchor.constraint(equalToConstant: 188)
                preferredWidth.priority = .defaultLow
                preferredWidth.isActive = true
            }
            if items.count > 1 {
                tabView.widthAnchor.constraint(lessThanOrEqualToConstant: 260).isActive = true
            }
            tabView.heightAnchor.constraint(equalToConstant: 30).isActive = true
            tabView.setContentHuggingPriority(.defaultLow, for: .horizontal)
            tabView.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            stackView.addArrangedSubview(tabView)
            tabViews[item.id] = tabView
        }

        stackView.addArrangedSubview(zoomButton)
        stackView.addArrangedSubview(addButton)

        if let editingID {
            DispatchQueue.main.async { [weak self] in
                self?.positionTitleEditor(for: editingID)
            }
        }
    }

    private func cancelEditing() {
        guard let editingID else { return }
        self.editingID = nil
        titleEditor.isHidden = true
        tabViews[editingID]?.setEditing(false)
    }

    @objc private func addTab() {
        onNewTab?()
    }

    @objc private func toggleSurfaceZoom() {
        onToggleSurfaceZoom?()
    }

    private func commitTitleEditor(_ text: String? = nil) {
        guard let editingID else {
            return
        }
        let currentText: String = text ?? titleEditor.string
        let cleaned = currentText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleaned.isEmpty else {
            cancelEditing()
            return
        }
        commitTitle(id: editingID, title: cleaned)
    }

    private func commitTitle(id: UUID, title: String) {
        editingID = nil
        titleEditor.isHidden = true
        tabViews[id]?.setEditing(false)
        tabViews[id]?.setTitle(title)
        onCommitTitle?(id, title)
    }

    private func positionTitleEditor(for tabID: UUID) {
        guard let tabView = tabViews[tabID] else { return }
        let tabFrame = convert(tabView.bounds, from: tabView)
        let editorX = tabFrame.minX + 28
        let editorHeight: CGFloat = 18
        titleEditor.frame = NSRect(
            x: editorX,
            y: max(0, tabFrame.midY - editorHeight / 2),
            width: max(32, tabFrame.width - 56),
            height: editorHeight
        )
    }
}

@MainActor
private final class LingXiaTerminalTabChromeView: NSView {
    var onSelect: ((UUID) -> Void)?
    var onRenameRequest: ((UUID) -> Void)?
    var onClose: ((UUID) -> Void)?

    private let itemID: UUID
    private var isActive: Bool
    private var titleValue: String
    private let closeButton = LingXiaTerminalCloseTabButton()
    private var tracking: NSTrackingArea?
    private var editing = false
    private var isHovered = false {
        didSet {
            guard oldValue != isHovered else { return }
            updateChrome()
        }
    }

    init(item: LingXiaTerminalTabRailView.Item) {
        self.itemID = item.id
        self.isActive = item.active
        self.titleValue = item.title.isEmpty ? "terminal" : item.title
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        layer?.masksToBounds = false
        setContentHuggingPriority(.defaultLow, for: .horizontal)
        setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        toolTip = "Double-click to rename"

        closeButton.target = self
        closeButton.action = #selector(closeTab)

        addSubview(closeButton)
        updateChrome()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isFlipped: Bool { true }

    override var intrinsicContentSize: NSSize {
        NSSize(width: 188, height: 28)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func layout() {
        super.layout()
        let closeSize: CGFloat = 16
        closeButton.frame = NSRect(
            x: max(0, bounds.width - closeSize - 10),
            y: max(0, (bounds.height - closeSize) / 2),
            width: closeSize,
            height: closeSize
        )

    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        drawTabBackground()
        drawTabTitle()
    }

    override func mouseDown(with event: NSEvent) {
        if event.clickCount >= 2 {
            onRenameRequest?(itemID)
            return
        }
        onSelect?(itemID)
    }

    override func rightMouseDown(with event: NSEvent) {
        // The tab itself has no context menu; pane-level right click owns terminal actions.
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        nil
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking {
            removeTrackingArea(tracking)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
    }

    func setEditing(_ editing: Bool) {
        self.editing = editing
        needsDisplay = true
    }

    @objc private func closeTab() {
        onClose?(itemID)
    }

    func setTitle(_ title: String) {
        titleValue = title.trimmingCharacters(in: .whitespacesAndNewlines)
        needsDisplay = true
    }

    private func drawTabBackground() {
        let scale = max(1, window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2)
        let pixel = 1 / scale
        let rect = NSRect(x: 0, y: 2, width: bounds.width, height: bounds.height - 2)

        if isActive {
            let activePath = topRoundedPath(in: rect, radius: 8)
            NSColor.lxTerminalBackground.setFill()
            activePath.fill()

            NSColor.white.withAlphaComponent(0.075).setStroke()
            activePath.lineWidth = pixel
            activePath.stroke()

            NSColor.white.withAlphaComponent(0.07).setFill()
            NSRect(x: 10, y: 3, width: max(0, bounds.width - 20), height: pixel).fill()

            // Active tabs visually connect into the terminal body, like Ghostty/iTerm tab strips.
            NSColor.lxTerminalBackground.setFill()
            NSRect(x: 0, y: bounds.height - 5, width: bounds.width, height: 5).fill()
        } else {
            if isHovered {
                let inactivePath = NSBezierPath(roundedRect: rect.insetBy(dx: 2, dy: 5), xRadius: 7, yRadius: 7)
                NSColor.white.withAlphaComponent(0.045).setFill()
                inactivePath.fill()
            }

            if !isHovered {
                NSColor.white.withAlphaComponent(0.055).setFill()
                NSRect(x: bounds.width - pixel, y: 9, width: pixel, height: max(0, bounds.height - 18)).fill()
            }
        }
    }

    private func drawTabTitle() {
        let markerRect = NSRect(x: 14, y: max(0, (bounds.height - 6) / 2), width: 6, height: 6)
        let titleRect = NSRect(x: 28, y: 7, width: max(0, closeButton.frame.minX - 36), height: 17)

        let markerColor = isActive
            ? NSColor(red: 0.682, green: 0.812, blue: 0.735, alpha: 1)
            : NSColor.white.withAlphaComponent(isHovered ? 0.58 : 0.40)
        markerColor.setFill()
        NSBezierPath(ovalIn: markerRect).fill()

        guard !editing else { return }
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        paragraph.alignment = .left
        let titleColor = isActive
            ? NSColor.white.withAlphaComponent(0.97)
            : NSColor.white.withAlphaComponent(isHovered ? 0.78 : 0.66)
        (titleValue as NSString).draw(
            in: titleRect,
            withAttributes: [
                .font: NSFont.systemFont(ofSize: 12, weight: isActive ? .semibold : .medium),
                .foregroundColor: titleColor,
                .paragraphStyle: paragraph
            ]
        )
    }

    private func topRoundedPath(in rect: NSRect, radius: CGFloat) -> NSBezierPath {
        let radius = min(radius, rect.width / 2, rect.height)
        let path = NSBezierPath()
        path.move(to: NSPoint(x: rect.minX, y: rect.maxY))
        path.line(to: NSPoint(x: rect.minX, y: rect.minY + radius))
        path.curve(
            to: NSPoint(x: rect.minX + radius, y: rect.minY),
            controlPoint1: NSPoint(x: rect.minX, y: rect.minY + radius * 0.45),
            controlPoint2: NSPoint(x: rect.minX + radius * 0.45, y: rect.minY)
        )
        path.line(to: NSPoint(x: rect.maxX - radius, y: rect.minY))
        path.curve(
            to: NSPoint(x: rect.maxX, y: rect.minY + radius),
            controlPoint1: NSPoint(x: rect.maxX - radius * 0.45, y: rect.minY),
            controlPoint2: NSPoint(x: rect.maxX, y: rect.minY + radius * 0.45)
        )
        path.line(to: NSPoint(x: rect.maxX, y: rect.maxY))
        path.close()
        return path
    }

    private func updateChrome() {
        closeButton.alphaValue = isActive || isHovered ? 1 : 0.48
        closeButton.contentTintColor = NSColor.white.withAlphaComponent(isActive ? 0.62 : 0.40)
        needsDisplay = true
    }
}

@MainActor
private final class LingXiaTerminalCloseTabButton: NSButton {
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        title = "×"
        isBordered = false
        font = NSFont.systemFont(ofSize: 12, weight: .medium)
        focusRingType = .none
        wantsLayer = true
        layer?.cornerRadius = 5
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        nil
    }
}

@MainActor
private class LingXiaTerminalRailIconButton: NSButton {
    private var hovered = false {
        didSet { updateAppearance() }
    }
    private var tracking: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        isBordered = false
        imagePosition = .imageOnly
        font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        focusRingType = .none
        wantsLayer = true
        layer?.cornerRadius = 4
        updateAppearance()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking {
            removeTrackingArea(tracking)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
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

    func applySymbol(systemName: String, fallbackTitle: String) {
        if let symbol = NSImage(systemSymbolName: systemName, accessibilityDescription: nil) {
            symbol.isTemplate = true
            image = symbol
            title = ""
            imagePosition = .imageOnly
        } else {
            image = nil
            title = fallbackTitle
            imagePosition = .noImage
        }
    }

    private func updateAppearance() {
        layer?.backgroundColor = hovered
            ? NSColor.white.withAlphaComponent(0.065).cgColor
            : NSColor.clear.cgColor
        layer?.borderWidth = 0
        layer?.borderColor = NSColor.clear.cgColor
        contentTintColor = NSColor.white.withAlphaComponent(hovered ? 0.92 : 0.68)
    }
}

@MainActor
private final class LingXiaTerminalAddTabButton: LingXiaTerminalRailIconButton {
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        applySymbol(systemName: "plus", fallbackTitle: "+")
        toolTip = "New Terminal Tab"
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

@MainActor
private final class LingXiaTerminalZoomButton: LingXiaTerminalRailIconButton {
    private var zoomed = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setZoomed(false)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setZoomed(_ zoomed: Bool) {
        self.zoomed = zoomed
        if zoomed {
            applySymbol(systemName: "arrow.down.right.and.arrow.up.left", fallbackTitle: "[]")
            toolTip = "Restore Terminal Size"
        } else {
            applySymbol(systemName: "arrow.up.left.and.arrow.down.right", fallbackTitle: "[ ]")
            toolTip = "Zoom Terminal to Full Window"
        }
    }
}

#endif
