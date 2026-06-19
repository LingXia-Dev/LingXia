#if os(macOS)
import AppKit
import CLingXiaRustAPI
import CoreText
import OSLog

extension NSColor {
    static let lxTerminalBackground = NSColor(red: 0.157, green: 0.173, blue: 0.204, alpha: 1)
    static let lxTerminalForeground = NSColor.white
    static let lxTerminalChrome = NSColor(red: 0.129, green: 0.145, blue: 0.169, alpha: 1)
    static let lxTerminalChromeRaised = NSColor(red: 0.173, green: 0.192, blue: 0.227, alpha: 1)
    static let lxTerminalBorder = NSColor(red: 0.243, green: 0.267, blue: 0.318, alpha: 1)
    static let lxTerminalAccent = NSColor(red: 0.773, green: 0.784, blue: 0.776, alpha: 1)
    /// Split divider — deliberately lighter than the pane background so it reads
    /// clearly between two dark terminal panes.
    static let lxTerminalDivider = NSColor(red: 0.36, green: 0.39, blue: 0.44, alpha: 1)
}

/// Container for a terminal pane split: a visible, draggable divider between the
/// two panes. (The previous NSStackView had neither — no rendered divider line
/// and no drag-to-resize.) Panes start at an even 50/50 and the divider can be
/// dragged to rebalance them.
@MainActor
final class LingXiaTerminalSplitView: NSSplitView {
    private var didEqualize = false

    /// A roomy grab zone for comfortable dragging, but only a thin hairline is
    /// painted — the rest blends into the pane background, so the divider reads
    /// as a subtle 1pt line rather than a heavy bar.
    override var dividerThickness: CGFloat { 5 }
    private static let lineThickness: CGFloat = 1

    override func drawDivider(in rect: NSRect) {
        NSColor.lxTerminalBackground.setFill()
        rect.fill()
        var line = rect
        let t = Self.lineThickness
        if isVertical {
            line.origin.x += (rect.width - t) / 2
            line.size.width = t
        } else {
            line.origin.y += (rect.height - t) / 2
            line.size.height = t
        }
        NSColor.lxTerminalDivider.setFill()
        line.fill()
    }

    override func layout() {
        super.layout()
        guard !didEqualize,
              arrangedSubviews.count == 2,
              bounds.width > 1, bounds.height > 1 else { return }
        didEqualize = true
        let total = isVertical ? bounds.width : bounds.height
        setPosition((total - dividerThickness) / 2, ofDividerAt: 0)
    }
}

enum LingXiaTerminalFont {
    static let defaultSize: CGFloat = 13
    private static let cascadeNames = [
        "Apple Color Emoji",
        "Symbols Nerd Font Mono",
        "Symbols Nerd Font",
        "MesloLGS NF",
        "MesloLGS NF Regular",
        "Hack Nerd Font Mono",
        "JetBrainsMono Nerd Font",
        "FiraCode Nerd Font Mono",
        "CaskaydiaCove Nerd Font Mono",
        "Noto Color Emoji",
        "Menlo",
        "SF Mono",
        "Monaco",
    ]

    static func regular(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "Menlo-Regular", size: size)
            ?? NSFont(name: "Menlo", size: size)
            ?? NSFont(name: "SFMono-Regular", size: size)
            ?? NSFont(name: "Monaco", size: size)
            ?? NSFont.monospacedSystemFont(ofSize: size, weight: .regular))
    }

    static func bold(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "Menlo-Bold", size: size)
            ?? NSFont(name: "SFMono-Semibold", size: size)
            ?? NSFont.monospacedSystemFont(ofSize: size, weight: .semibold))
    }

    static func italic(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "Menlo-Italic", size: size)
            ?? NSFontManager.shared.convert(regular(size: size), toHaveTrait: .italicFontMask))
    }

    static func boldItalic(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "Menlo-BoldItalic", size: size)
            ?? NSFontManager.shared.convert(bold(size: size), toHaveTrait: .italicFontMask))
    }

    static func make(size: CGFloat = defaultSize, bold: Bool, italic: Bool) -> NSFont {
        switch (bold, italic) {
        case (true, true): return boldItalic(size: size)
        case (true, false): return self.bold(size: size)
        case (false, true): return self.italic(size: size)
        case (false, false): return regular(size: size)
        }
    }

    static func bestFont(for text: String, base: NSFont) -> NSFont {
        guard !text.isEmpty else { return base }
        let cfText = text as CFString
        let range = CFRange(location: 0, length: CFStringGetLength(cfText))
        guard let fallback = CTFontCreateForString(base as CTFont, cfText, range) as NSFont? else {
            return base
        }
        return withCascade(fallback)
    }

    private static func withCascade(_ font: NSFont) -> NSFont {
        let cascade = cascadeNames.compactMap { NSFontDescriptor(name: $0, size: font.pointSize) }
        guard !cascade.isEmpty else { return font }
        let descriptor = font.fontDescriptor.addingAttributes([
            NSFontDescriptor.AttributeName.cascadeList: cascade
        ])
        return NSFont(descriptor: descriptor, size: font.pointSize) ?? font
    }
}

enum LingXiaTerminalSplitDirection {
    case left
    case right
    case up
    case down
}

enum LingXiaTerminalKeyMapper {
    static func input(for event: NSEvent, applicationCursor: Bool = false) -> String? {
        if event.modifierFlags.contains(.command) {
            if Int(event.keyCode) == 9,
               let text = NSPasteboard.general.string(forType: .string),
               !text.isEmpty {
                return text
            }
            return nil
        }

        if let sequence = escapeSequence(for: event, applicationCursor: applicationCursor) {
            return sequence
        }

        if event.modifierFlags.contains(.control),
           let control = controlSequence(for: event) {
            return control
        }

        if let chars = event.characters, !chars.isEmpty {
            return chars
        }
        if let chars = event.charactersIgnoringModifiers, !chars.isEmpty {
            return chars
        }
        return nil
    }

    private static func escapeSequence(for event: NSEvent, applicationCursor: Bool) -> String? {
        let modifiers = event.modifierFlags
        switch Int(event.keyCode) {
        case 123: return applicationCursor ? "\u{1B}OD" : "\u{1B}[D" // left
        case 124: return applicationCursor ? "\u{1B}OC" : "\u{1B}[C" // right
        case 125: return applicationCursor ? "\u{1B}OB" : "\u{1B}[B" // down
        case 126: return applicationCursor ? "\u{1B}OA" : "\u{1B}[A" // up
        case 48: return modifiers.contains(.shift) ? "\u{1B}[Z" : "\t"
        case 51: return "\u{7F}"    // delete/backspace
        case 53: return "\u{1B}"    // escape
        case 36, 76: return "\r"    // return / keypad enter
        case 115: return "\u{1B}[H" // home
        case 119: return "\u{1B}[F" // end
        case 116: return "\u{1B}[5~" // page up
        case 121: return "\u{1B}[6~" // page down
        case 117: return "\u{1B}[3~" // forward delete
        case 122: return "\u{1B}OP" // F1
        case 120: return "\u{1B}OQ" // F2
        case 99: return "\u{1B}OR" // F3
        case 118: return "\u{1B}OS" // F4
        case 96: return "\u{1B}[15~" // F5
        case 97: return "\u{1B}[17~" // F6
        case 98: return "\u{1B}[18~" // F7
        case 100: return "\u{1B}[19~" // F8
        case 101: return "\u{1B}[20~" // F9
        case 109: return "\u{1B}[21~" // F10
        case 103: return "\u{1B}[23~" // F11
        case 111: return "\u{1B}[24~" // F12
        default: return nil
        }
    }

    private static func controlSequence(for event: NSEvent) -> String? {
        guard let chars = event.charactersIgnoringModifiers,
              let scalar = chars.unicodeScalars.first else {
            return nil
        }
        switch scalar.value {
        case 0x61...0x7A:
            return UnicodeScalar(scalar.value - 96).map(String.init)
        case 0x40, 0x20:
            return "\u{0}"
        case 0x5B:
            return "\u{1B}"
        case 0x5C:
            return "\u{1C}"
        case 0x5D:
            return "\u{1D}"
        case 0x5E:
            return "\u{1E}"
        case 0x5F, 0x2F:
            return "\u{1F}"
        default:
            return nil
        }
    }
}

@MainActor
func lxTerminalIsNoisyDiagnostic(_ message: String) -> Bool {
    message.hasPrefix("pane.snapshot ")
        || message.hasPrefix("workspace.layout ")
}

private let lxTerminalViewOSLog = OSLog(subsystem: "LingXia", category: "MacTerminal")

@MainActor
func lxTerminalLog(_ message: String, type: OSLogType = .info) {
    let traceFrames = ProcessInfo.processInfo.environment["LX_TERMINAL_TRACE_FRAMES"] == "1"
    let debugEnabled = ProcessInfo.processInfo.environment["LX_TERMINAL_DEBUG_LOGS"] == "1"
    let stdoutEnabled = ProcessInfo.processInfo.environment["LX_TERMINAL_STDOUT_LOGS"] == "1"
    let noisy = lxTerminalIsNoisyDiagnostic(message)
    guard !noisy || traceFrames || debugEnabled || type == .error || type == .fault else {
        return
    }

    if stdoutEnabled {
        let line = "[LingXia][Terminal] \(message)\n"
        FileHandle.standardOutput.write(Data(line.utf8))
        NSLog("%@", line.trimmingCharacters(in: .newlines))
    }

    guard debugEnabled || type == .error || type == .fault else {
        return
    }
    os_log("%{public}@", log: lxTerminalViewOSLog, type: type, message)
}

func lxTerminalLogAsync(_ message: String, type: OSLogType = .info) {
    DispatchQueue.main.async {
        lxTerminalLog(message, type: type)
    }
}

@MainActor
final class LingXiaTerminalWorkspaceView: NSView {
    private static let log = lxTerminalViewOSLog
    private static let toolbarHeight: CGFloat = 34

    @MainActor
    private final class TerminalTab {
        let id = UUID()
        var processTitle: String
        var detailTitle: String?
        var customTitle: String?
        var titlePinnedByUser = false
        let rootContainer = NSView()
        var panes: [UUID: LingXiaTerminalPaneView] = [:]
        var activePaneID: UUID?
        var zoomedPaneID: UUID?

        init(processTitle: String) {
            self.processTitle = processTitle
            rootContainer.wantsLayer = true
            rootContainer.translatesAutoresizingMaskIntoConstraints = false
            rootContainer.layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        }

        var displayTitle: String {
            let cleaned = (customTitle ?? processTitle).trimmingCharacters(in: .whitespacesAndNewlines)
            return cleaned.isEmpty ? "terminal" : cleaned
        }

        var displaySubtitle: String? {
            let cleaned = detailTitle?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            guard !cleaned.isEmpty, cleaned != displayTitle else {
                return nil
            }
            return cleaned
        }
    }

    private let surfaceID: String

    var onRequestClosePanel: (() -> Void)?
    var onToggleSurfaceZoom: ((Bool) -> Void)?

    private let toolbarStack = NSView()
    private let tabRailView = LingXiaTerminalTabRailView()
    private let contentHost = NSView()
    private var tabs: [TerminalTab] = []
    private var activeTabID: UUID?
    private var surfaceZoomed = false
    nonisolated(unsafe) private var mouseEventMonitor: Any?
    nonisolated(unsafe) private var keyEventMonitor: Any?
    private var inputArmed = false

    override var isFlipped: Bool { true }

    init(surfaceID: String) {
        self.surfaceID = surfaceID
        super.init(frame: .zero)
        lxTerminalLog("workspace.init surface=\(surfaceID)")
        setupLayout()
        installEventMonitorsIfNeeded()
        createTabAndActivate()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        if let mouseEventMonitor {
            NSEvent.removeMonitor(mouseEventMonitor)
        }
        if let keyEventMonitor {
            NSEvent.removeMonitor(keyEventMonitor)
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        lxTerminalLog("workspace.viewDidMoveToWindow surface=\(surfaceID) hasWindow=\(window != nil)")
        updateEventMonitors()
        focusActiveTerminal()
    }

    override func layout() {
        super.layout()
        let toolbarHeight = Self.toolbarHeight
        toolbarStack.frame = NSRect(x: 0, y: 0, width: bounds.width, height: toolbarHeight)
        tabRailView.frame = NSRect(x: 0, y: 0, width: bounds.width, height: toolbarHeight)
        contentHost.frame = NSRect(
            x: 0,
            y: toolbarHeight,
            width: bounds.width,
            height: max(0, bounds.height - toolbarHeight)
        )
        toolbarStack.layoutSubtreeIfNeeded()
        contentHost.layoutSubtreeIfNeeded()
        lxTerminalLog(
            "workspace.layout surface=\(surfaceID) frame=\(String(format: "%.0f,%.0f %.0fx%.0f", frame.minX, frame.minY, frame.width, frame.height)) bounds=\(String(format: "%.0f,%.0f %.0fx%.0f", bounds.minX, bounds.minY, bounds.width, bounds.height)) toolbarFrame=\(String(format: "%.0f,%.0f %.0fx%.0f", toolbarStack.frame.minX, toolbarStack.frame.minY, toolbarStack.frame.width, toolbarStack.frame.height)) contentFrame=\(String(format: "%.0f,%.0f %.0fx%.0f", contentHost.frame.minX, contentHost.frame.minY, contentHost.frame.width, contentHost.frame.height))"
        )
    }

    func focusActiveTerminal() {
        ensureOpenTab()
        guard let tab = activeTab(),
              let activePane = activePane(in: tab) else {
            lxTerminalLog("workspace.focusActiveTerminal no-active-pane surface=\(surfaceID)", type: .error)
            return
        }
        inputArmed = true
        lxTerminalLog("workspace.focusActiveTerminal surface=\(surfaceID) tab=\(tab.id.uuidString) pane=\(activePane.paneID.uuidString) window=\(window != nil)")
        activePane.focusTerminal()
        DispatchQueue.main.async { [weak activePane] in
            activePane?.focusTerminal()
        }
    }

    func ensureOpenTab() {
        if tabs.isEmpty {
            lxTerminalLog("workspace.ensureOpenTab creating surface=\(surfaceID)")
            createTabAndActivate()
        }
    }

    func disarmInput() {
        inputArmed = false
        lxTerminalLog("workspace.disarmInput surface=\(surfaceID)")
    }

    func setSurfaceZoomEnabled(_ enabled: Bool, notifyRuntime: Bool = true) {
        guard surfaceZoomed != enabled else { return }
        surfaceZoomed = enabled
        tabRailView.isSurfaceZoomed = enabled
        lxTerminalLog("workspace.surfaceZoom surface=\(surfaceID) enabled=\(enabled) notify=\(notifyRuntime)")
        if notifyRuntime {
            onToggleSurfaceZoom?(enabled)
        }
        if enabled {
            focusActiveTerminal()
        }
    }

    private func toggleSurfaceZoomFromUI() {
        setSurfaceZoomEnabled(!surfaceZoomed, notifyRuntime: true)
    }

    private func setupLayout() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        translatesAutoresizingMaskIntoConstraints = false
        setContentHuggingPriority(.defaultLow, for: .horizontal)
        setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        toolbarStack.translatesAutoresizingMaskIntoConstraints = true
        toolbarStack.wantsLayer = true
        toolbarStack.layer?.backgroundColor = NSColor.lxTerminalChrome.cgColor
        toolbarStack.layer?.zPosition = 10
        toolbarStack.setContentHuggingPriority(.defaultLow, for: .horizontal)
        toolbarStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        tabRailView.translatesAutoresizingMaskIntoConstraints = true
        tabRailView.wantsLayer = true
        tabRailView.layer?.zPosition = 11
        tabRailView.setContentHuggingPriority(.defaultLow, for: .horizontal)
        tabRailView.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        tabRailView.onSelect = { [weak self] id in
            self?.activateTab(id: id, focusPane: true)
        }
        tabRailView.onRenameRequest = { [weak self] id in
            guard let self else { return }
            self.activateTab(id: id, focusPane: false)
            self.tabRailView.beginEditing(tabID: id)
        }
        tabRailView.onClose = { [weak self] id in
            self?.closeTab(id: id)
        }
        tabRailView.onNewTab = { [weak self] in
            self?.createTabAndActivate()
        }
        tabRailView.onToggleSurfaceZoom = { [weak self] in
            self?.toggleSurfaceZoomFromUI()
        }
        tabRailView.onCommitTitle = { [weak self] id, title in
            guard let self,
                  let tab = self.tabs.first(where: { $0.id == id }) else {
                return
            }
            self.updateManualTitle(title, tabID: tab.id)
            self.activateTab(id: tab.id, focusPane: true)
        }
        tabRailView.isSurfaceZoomed = surfaceZoomed

        toolbarStack.addSubview(tabRailView)

        contentHost.wantsLayer = true
        contentHost.layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        contentHost.layer?.zPosition = 0
        contentHost.translatesAutoresizingMaskIntoConstraints = true
        contentHost.setContentHuggingPriority(.defaultLow, for: .horizontal)
        contentHost.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        addSubview(toolbarStack)
        addSubview(contentHost)
    }

    private func updateEventMonitors() {
        installEventMonitorsIfNeeded()
    }

    private func installEventMonitorsIfNeeded() {
        if mouseEventMonitor == nil {
            lxTerminalLog("workspace.installMouseEventMonitor surface=\(surfaceID)")
            mouseEventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
                guard let self else { return event }
                return self.handleLocalMouseDown(event)
            }
        }

        if keyEventMonitor == nil {
            lxTerminalLog("workspace.installKeyEventMonitor surface=\(surfaceID)")
            keyEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self else { return event }
                return self.handleLocalKeyDown(event)
            }
        }
    }

    private func removeEventMonitors() {
        if let mouseEventMonitor {
            NSEvent.removeMonitor(mouseEventMonitor)
            self.mouseEventMonitor = nil
            lxTerminalLog("workspace.removeMouseEventMonitor surface=\(surfaceID)")
        }
        if let keyEventMonitor {
            NSEvent.removeMonitor(keyEventMonitor)
            self.keyEventMonitor = nil
            lxTerminalLog("workspace.removeKeyEventMonitor surface=\(surfaceID)")
        }
    }

    private func handleLocalMouseDown(_ event: NSEvent) -> NSEvent? {
        let inside = containsEventInWorkspace(event)
        inputArmed = inside
        lxTerminalLog("workspace.mouseDown surface=\(surfaceID) type=\(event.type.rawValue) inside=\(inside) inputArmed=\(inputArmed) window=\(window != nil)")
        guard inside else { return event }

        let point = convert(event.locationInWindow, from: nil)
        let hitTabRail = tabRailView.frame.contains(point)
        if hitTabRail {
            // Keep tab strip interactions isolated; avoid stealing focus while renaming.
            inputArmed = false
            return event
        }

        if let tab = activeTab(),
           let pane = activePane(in: tab) {
            lxTerminalLog("workspace.mouseDown focusPane surface=\(surfaceID) pane=\(pane.paneID.uuidString)")
            pane.focusTerminal()
        }

        if event.type == .rightMouseDown {
            lxTerminalLog("workspace.rightMouseDown forward surface=\(surfaceID)")
            return event
        }

        return event
    }

    private func handleLocalKeyDown(_ event: NSEvent) -> NSEvent? {
        guard inputArmed else { return event }
        guard let window, event.window === window else { return event }
        guard !tabRailView.isEditingTitle else { return event }
        guard let tab = activeTab(),
              let pane = activePane(in: tab) else {
            return event
        }

        if pane.ownsFirstResponder(window.firstResponder) {
            return event
        }

        let responder = window.firstResponder.map { String(describing: type(of: $0)) } ?? "nil"
        let consumed = pane.consumeKeyDown(event, source: "workspace.fallback")
        lxTerminalLog(
            "workspace.keyDownFallback surface=\(surfaceID) pane=\(pane.paneID.uuidString) keyCode=\(event.keyCode) consumed=\(consumed) firstResponder=\(responder)"
        )
        return consumed ? nil : event
    }

    private func containsEventInWorkspace(_ event: NSEvent) -> Bool {
        guard let window else { return false }
        let screenPoint: NSPoint
        if let eventWindow = event.window {
            screenPoint = eventWindow.convertToScreen(NSRect(origin: event.locationInWindow, size: .zero)).origin
        } else {
            screenPoint = NSEvent.mouseLocation
        }
        let rectInWindow = convert(bounds, to: nil)
        let rectInScreen = window.convertToScreen(rectInWindow)
        return rectInScreen.contains(screenPoint)
    }

    private func showSplitMenu(for event: NSEvent) {
        let menu = NSMenu(title: "Terminal")
        menu.addItem(splitMenuItem("Split Left", action: #selector(splitLeftFromMenu)))
        menu.addItem(splitMenuItem("Split Right", action: #selector(splitRightFromMenu)))
        menu.addItem(splitMenuItem("Split Top", action: #selector(splitTopFromMenu)))
        menu.addItem(splitMenuItem("Split Bottom", action: #selector(splitBottomFromMenu)))
        let point = convert(event.locationInWindow, from: nil)
        lxTerminalLog("workspace.showSplitMenu surface=\(surfaceID) point=\(point)")
        menu.popUp(positioning: nil, at: point, in: self)
    }

    private func splitMenuItem(_ title: String, action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        return item
    }

    @objc private func splitLeftFromMenu() {
        lxTerminalLog("workspace.menuAction splitLeft surface=\(surfaceID)")
        splitActivePane(direction: .left)
    }

    @objc private func splitRightFromMenu() {
        lxTerminalLog("workspace.menuAction splitRight surface=\(surfaceID)")
        splitActivePane(direction: .right)
    }

    @objc private func splitTopFromMenu() {
        lxTerminalLog("workspace.menuAction splitTop surface=\(surfaceID)")
        splitActivePane(direction: .up)
    }

    @objc private func splitBottomFromMenu() {
        lxTerminalLog("workspace.menuAction splitBottom surface=\(surfaceID)")
        splitActivePane(direction: .down)
    }

    private func makeToolbarButton(title: String, action: Selector, toolTip: String) -> NSButton {
        let button = NSButton(title: title, target: self, action: action)
        button.bezelStyle = .texturedRounded
        button.controlSize = .small
        button.toolTip = toolTip
        button.setContentHuggingPriority(.required, for: .horizontal)
        button.setContentCompressionResistancePriority(.required, for: .horizontal)
        return button
    }

    @objc private func didPressCloseActiveTab() {
        if let activeTabID {
            closeTab(id: activeTabID)
        }
    }

    @objc private func didPressSplitLeft() {
        splitActivePane(direction: .left)
    }

    @objc private func didPressSplitRight() {
        splitActivePane(direction: .right)
    }

    @objc private func didPressSplitUp() {
        splitActivePane(direction: .up)
    }

    @objc private func didPressSplitDown() {
        splitActivePane(direction: .down)
    }

    private func createTabAndActivate() {
        let tab = TerminalTab(processTitle: "terminal")
        let firstPane = makePane(for: tab)
        installRootView(firstPane, into: tab.rootContainer)
        tab.panes[firstPane.paneID] = firstPane
        tab.activePaneID = firstPane.paneID
        tabs.append(tab)
        lxTerminalLog("workspace.createTab surface=\(surfaceID) tab=\(tab.id.uuidString) pane=\(firstPane.paneID.uuidString) totalTabs=\(tabs.count)")
        activateTab(id: tab.id, focusPane: true)
    }

    private func refreshTabStrip() {
        tabRailView.items = tabs.map {
            LingXiaTerminalTabRailView.Item(
                id: $0.id,
                title: $0.displayTitle,
                subtitle: $0.displaySubtitle,
                active: $0.id == activeTabID
            )
        }
    }

    private func activateTab(id: UUID, focusPane: Bool) {
        guard let tab = tabs.first(where: { $0.id == id }) else { return }
        activeTabID = id
        lxTerminalLog("workspace.activateTab surface=\(surfaceID) tab=\(id.uuidString) focusPane=\(focusPane)")
        refreshTabStrip()
        installRootView(tab.rootContainer, into: contentHost)
        applyZoomState(in: tab)
        updatePaneHighlight(in: tab)
        if focusPane,
           let activePane = activePane(in: tab) {
            activePane.focusTerminal()
        }
    }

    private func closeTab(id: UUID) {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else { return }
        let closingActiveTab = activeTabID == id
        let tab = tabs.remove(at: index)
        lxTerminalLog("workspace.closeTab surface=\(surfaceID) tab=\(id.uuidString) remaining=\(tabs.count)")
        tab.rootContainer.removeFromSuperview()

        if tabs.isEmpty {
            activeTabID = nil
            contentHost.subviews.forEach { $0.removeFromSuperview() }
            refreshTabStrip()
            onRequestClosePanel?()
            return
        }

        if closingActiveTab {
            let nextIndex = min(index, tabs.count - 1)
            activateTab(id: tabs[nextIndex].id, focusPane: true)
        } else {
            refreshTabStrip()
        }
    }

    private func splitActivePane(direction: LingXiaTerminalSplitDirection) {
        guard let tab = activeTab(),
              let activePane = activePane(in: tab) else {
            lxTerminalLog("workspace.split no-active-pane surface=\(surfaceID) direction=\(direction)", type: .error)
            return
        }
        lxTerminalLog("workspace.split start surface=\(surfaceID) direction=\(direction) activePane=\(activePane.paneID.uuidString)")

        let newPane = makePane(for: tab)
        newPane.translatesAutoresizingMaskIntoConstraints = false
        activePane.translatesAutoresizingMaskIntoConstraints = false
        let split = LingXiaTerminalSplitView()
        // left/right → panes side by side (vertical divider); up/down → stacked.
        split.isVertical = (direction == .left || direction == .right)
        split.translatesAutoresizingMaskIntoConstraints = false

        guard replaceNodeView(activePane, with: split, in: tab.rootContainer) else {
            os_log(
                "terminal split failed: cannot replace pane surface=%{public}@",
                log: Self.log,
                type: .error,
                surfaceID
            )
            lxTerminalLog("workspace.split failed-replace surface=\(surfaceID) direction=\(direction)", type: .error)
            return
        }

        if direction == .left || direction == .up {
            split.addArrangedSubview(newPane)
            split.addArrangedSubview(activePane)
        } else {
            split.addArrangedSubview(activePane)
            split.addArrangedSubview(newPane)
        }

        tab.panes[newPane.paneID] = newPane
        tab.activePaneID = newPane.paneID
        tab.zoomedPaneID = nil
        applyZoomState(in: tab)
        updatePaneHighlight(in: tab)
        newPane.focusTerminal()
        lxTerminalLog("workspace.split complete surface=\(surfaceID) direction=\(direction) newPane=\(newPane.paneID.uuidString) totalPanes=\(tab.panes.count)")
    }

    private func replaceNodeView(_ target: NSView, with replacement: NSView, in root: NSView) -> Bool {
        guard let parent = target.superview else { return false }
        if let split = parent as? NSSplitView {
            guard let index = split.arrangedSubviews.firstIndex(of: target) else {
                return false
            }
            split.removeArrangedSubview(target)
            target.removeFromSuperview()
            split.insertArrangedSubview(replacement, at: index)
            return true
        }

        if parent === root {
            target.removeFromSuperview()
            installRootView(replacement, into: root)
            return true
        }

        target.removeFromSuperview()
        parent.addSubview(replacement)
        NSLayoutConstraint.activate([
            replacement.topAnchor.constraint(equalTo: parent.topAnchor),
            replacement.leadingAnchor.constraint(equalTo: parent.leadingAnchor),
            replacement.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
            replacement.bottomAnchor.constraint(equalTo: parent.bottomAnchor),
        ])
        return true
    }

    private func installRootView(_ view: NSView, into container: NSView) {
        if view.superview === container {
            return
        }
        container.subviews.forEach { $0.removeFromSuperview() }
        view.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(view)
        NSLayoutConstraint.activate([
            view.topAnchor.constraint(equalTo: container.topAnchor),
            view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            view.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])
    }

    private func makePane(for tab: TerminalTab) -> LingXiaTerminalPaneView {
        let pane = LingXiaTerminalPaneView()
        lxTerminalLog("workspace.makePane surface=\(surfaceID) tab=\(tab.id.uuidString) pane=\(pane.paneID.uuidString)")
        pane.onActivated = { [weak self] paneID in
            guard let self else { return }
            self.activatePane(paneID, forTabID: tab.id)
        }
        pane.onSplitRequested = { [weak self] paneID, direction in
            guard let self else { return }
            self.activatePane(paneID, forTabID: tab.id)
            self.splitActivePane(direction: direction)
        }
        pane.onZoomRequested = { [weak self] paneID in
            self?.togglePaneZoom(paneID, forTabID: tab.id)
        }
        pane.onTitleChanged = { [weak self] paneID, processTitle, detailTitle in
            self?.updateTitle(processTitle: processTitle, detailTitle: detailTitle, paneID: paneID, tabID: tab.id)
        }
        pane.onManualTitleChanged = { [weak self] paneID, title in
            let _ = paneID
            self?.updateManualTitle(title, tabID: tab.id)
        }
        pane.onTitleEditRequested = { [weak self] paneID in
            guard let self else { return }
            self.activatePane(paneID, forTabID: tab.id)
            self.tabRailView.beginEditing(tabID: tab.id)
        }
        pane.onExited = { [weak self] paneID in
            self?.closePane(paneID, forTabID: tab.id)
        }
        return pane
    }

    private func updateTitle(processTitle: String?, detailTitle: String?, paneID: UUID, tabID: UUID) {
        guard let tab = tabs.first(where: { $0.id == tabID }),
              tab.activePaneID == paneID else {
            return
        }
        guard !tab.titlePinnedByUser else {
            return
        }
        let previousTitle = tab.displayTitle
        let previousSubtitle = tab.displaySubtitle
        let cleanedProcess = processTitle?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let cleanedDetail = detailTitle?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if !cleanedProcess.isEmpty {
            tab.processTitle = cleanedProcess
        }
        tab.detailTitle = cleanedDetail.isEmpty ? nil : cleanedDetail
        if previousTitle != tab.displayTitle || previousSubtitle != tab.displaySubtitle {
            refreshTabStrip()
        }
    }

    private func updateManualTitle(_ title: String, tabID: UUID) {
        let cleaned = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleaned.isEmpty,
              let tab = tabs.first(where: { $0.id == tabID }) else {
            return
        }
        tab.customTitle = cleaned
        tab.titlePinnedByUser = true
        tab.detailTitle = nil
        refreshTabStrip()
    }

    private func closePane(_ paneID: UUID, forTabID tabID: UUID) {
        guard let tab = tabs.first(where: { $0.id == tabID }),
              let pane = tab.panes[paneID] else {
            return
        }

        lxTerminalLog("workspace.closePane surface=\(surfaceID) tab=\(tabID.uuidString) pane=\(paneID.uuidString)")

        if tab.panes.count <= 1 {
            closeTab(id: tabID)
            return
        }

        tab.panes.removeValue(forKey: paneID)
        if let split = pane.superview as? NSSplitView {
            split.removeArrangedSubview(pane)
            pane.removeFromSuperview()
            collapseSingleChildSplit(split, in: tab.rootContainer)
        } else {
            pane.removeFromSuperview()
        }

        if tab.activePaneID == paneID {
            tab.activePaneID = tab.panes.keys.first
        }
        if tab.zoomedPaneID == paneID {
            tab.zoomedPaneID = nil
        }
        applyZoomState(in: tab)
        updatePaneHighlight(in: tab)
        if let activePane = activePane(in: tab) {
            activePane.focusTerminal()
        }
    }

    private func collapseSingleChildSplit(_ split: NSSplitView, in root: NSView) {
        guard split.arrangedSubviews.count == 1,
              let survivor = split.arrangedSubviews.first else {
            return
        }

        split.removeArrangedSubview(survivor)
        survivor.removeFromSuperview()

        if let parentSplit = split.superview as? NSSplitView,
           let index = parentSplit.arrangedSubviews.firstIndex(of: split) {
            parentSplit.removeArrangedSubview(split)
            split.removeFromSuperview()
            parentSplit.insertArrangedSubview(survivor, at: index)
            collapseSingleChildSplit(parentSplit, in: root)
            return
        }

        if split.superview === root {
            split.removeFromSuperview()
            installRootView(survivor, into: root)
            return
        }

        guard let parent = split.superview else {
            return
        }
        split.removeFromSuperview()
        survivor.translatesAutoresizingMaskIntoConstraints = false
        parent.addSubview(survivor)
        NSLayoutConstraint.activate([
            survivor.topAnchor.constraint(equalTo: parent.topAnchor),
            survivor.leadingAnchor.constraint(equalTo: parent.leadingAnchor),
            survivor.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
            survivor.bottomAnchor.constraint(equalTo: parent.bottomAnchor),
        ])
    }

    private func activatePane(_ paneID: UUID, forTabID tabID: UUID) {
        guard let tab = tabs.first(where: { $0.id == tabID }) else { return }
        inputArmed = true
        tab.activePaneID = paneID
        lxTerminalLog("workspace.activatePane surface=\(surfaceID) tab=\(tabID.uuidString) pane=\(paneID.uuidString)")
        if activeTabID != tabID {
            activateTab(id: tabID, focusPane: false)
        }
        updatePaneHighlight(in: tab)
    }

    private func togglePaneZoom(_ paneID: UUID, forTabID tabID: UUID) {
        guard let tab = tabs.first(where: { $0.id == tabID }),
              tab.panes[paneID] != nil else {
            return
        }
        inputArmed = true
        tab.activePaneID = paneID
        tab.zoomedPaneID = (tab.zoomedPaneID == paneID) ? nil : paneID
        lxTerminalLog("workspace.toggleZoom surface=\(surfaceID) tab=\(tabID.uuidString) pane=\(paneID.uuidString) zoomed=\(tab.zoomedPaneID != nil)")
        applyZoomState(in: tab)
        updatePaneHighlight(in: tab)
        tab.panes[paneID]?.focusTerminal()
    }

    private func updatePaneHighlight(in tab: TerminalTab) {
        for (paneID, pane) in tab.panes {
            pane.setActive(paneID == tab.activePaneID)
            pane.setZoomed(tab.zoomedPaneID == paneID)
        }
    }

    private func applyZoomState(in tab: TerminalTab) {
        for (paneID, pane) in tab.panes {
            pane.isHidden = tab.zoomedPaneID.map { $0 != paneID } ?? false
        }
        tab.rootContainer.layoutSubtreeIfNeeded()
    }

    private func activeTab() -> TerminalTab? {
        guard let activeTabID else { return nil }
        return tabs.first(where: { $0.id == activeTabID })
    }

    private func activePane(in tab: TerminalTab) -> LingXiaTerminalPaneView? {
        if let paneID = tab.activePaneID,
           let pane = tab.panes[paneID] {
            return pane
        }
        if let first = tab.panes.values.first {
            tab.activePaneID = first.paneID
            return first
        }
        return nil
    }
}

#endif
