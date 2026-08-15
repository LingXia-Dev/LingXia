#if os(macOS)
import AppKit
import CLingXiaRustAPI
import CoreText
import OSLog

/// Chrome colors for a terminal surface, from the scheme in effect.
///
/// The tab rail belongs to the terminal, not to the app — its `+` opens
/// another PTY — so it is tinted from the scheme rather than fixed. The rule
/// lives in the shared configuration layer, which the Windows host reads too;
/// fixed colors here meant a theme change moved the grid and left a dark rail
/// bolted to a light terminal.
enum LingXiaTerminalChrome {
    private struct Palette: Decodable {
        var surface: String
        var header: String
        var separator: String
        var text: String
        var textMuted: String
        var cursor: String
        var selectionBackground: String
        var selectionForeground: String
    }

    nonisolated(unsafe) private(set) static var surface = NSColor(
        red: 0.157, green: 0.173, blue: 0.204, alpha: 1)
    nonisolated(unsafe) private(set) static var header = NSColor(
        red: 0.129, green: 0.145, blue: 0.169, alpha: 1)
    nonisolated(unsafe) private(set) static var separator = NSColor(
        red: 0.243, green: 0.267, blue: 0.318, alpha: 1)
    nonisolated(unsafe) private(set) static var text = NSColor.white
    nonisolated(unsafe) private(set) static var textMuted = NSColor.white.withAlphaComponent(0.66)
    nonisolated(unsafe) private(set) static var cursor = NSColor.white
    nonisolated(unsafe) private(set) static var selectionBackground = NSColor.white
    nonisolated(unsafe) private(set) static var selectionForeground = NSColor.black

    /// Posted when the scheme moved, so views holding a chrome color re-apply it.
    static let didChangeNotification = Notification.Name("LingXiaTerminalChromeDidChange")

    /// Re-read after the configuration generation moves. Cheap enough to call
    /// on every change; the values only move at human speed.
    static func reload() {
        let json = terminalSurfaceChrome().toString()
        guard let data = json.data(using: .utf8),
            let palette = try? JSONDecoder().decode(Palette.self, from: data),
            let surface = NSColor(lxHex: palette.surface),
            let header = NSColor(lxHex: palette.header),
            let separator = NSColor(lxHex: palette.separator),
            let text = NSColor(lxHex: palette.text),
            let textMuted = NSColor(lxHex: palette.textMuted),
            let cursor = NSColor(lxHex: palette.cursor),
            let selectionBackground = NSColor(lxHex: palette.selectionBackground),
            let selectionForeground = NSColor(lxHex: palette.selectionForeground)
        else { return }
        guard surface != Self.surface
            || header != Self.header
            || text != Self.text
            || cursor != Self.cursor
            || selectionBackground != Self.selectionBackground
            || selectionForeground != Self.selectionForeground
        else { return }
        Self.surface = surface
        Self.header = header
        Self.separator = separator
        Self.text = text
        Self.textMuted = textMuted
        Self.cursor = cursor
        Self.selectionBackground = selectionBackground
        Self.selectionForeground = selectionForeground
        NotificationCenter.default.post(name: Self.didChangeNotification, object: nil)
    }
}

extension NSColor {
    /// `#rrggbb`, which is how the shared layer spells a color.
    convenience init?(lxHex: String) {
        let hex = lxHex.hasPrefix("#") ? String(lxHex.dropFirst()) : lxHex
        guard hex.count == 6, let value = UInt32(hex, radix: 16) else { return nil }
        self.init(
            srgbRed: CGFloat((value >> 16) & 0xff) / 255,
            green: CGFloat((value >> 8) & 0xff) / 255,
            blue: CGFloat(value & 0xff) / 255,
            alpha: 1)
    }

    static var lxTerminalBackground: NSColor { LingXiaTerminalChrome.surface }
    static var lxTerminalForeground: NSColor { LingXiaTerminalChrome.text }
    static var lxTerminalChrome: NSColor { LingXiaTerminalChrome.header }
    /// One step off the rail, for a control that has to sit on it.
    static var lxTerminalChromeRaised: NSColor {
        LingXiaTerminalChrome.header.blended(
            withFraction: 0.08, of: LingXiaTerminalChrome.text) ?? LingXiaTerminalChrome.header
    }
    static var lxTerminalBorder: NSColor { LingXiaTerminalChrome.separator }
    static var lxTerminalCursor: NSColor { LingXiaTerminalChrome.cursor }
    static var lxTerminalSelectionBackground: NSColor {
        LingXiaTerminalChrome.selectionBackground
    }
    static var lxTerminalSelectionForeground: NSColor {
        LingXiaTerminalChrome.selectionForeground
    }
    static let lxTerminalAccent = NSColor(red: 0.773, green: 0.784, blue: 0.776, alpha: 1)
    /// Split divider — deliberately further from the pane background than the
    /// rail's separator, so it reads clearly between two panes of the same color.
    static var lxTerminalDivider: NSColor {
        LingXiaTerminalChrome.surface.blended(
            withFraction: 0.30, of: LingXiaTerminalChrome.text) ?? LingXiaTerminalChrome.separator
    }
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
        equalizeIfReady()
    }

    func equalizeAfterInsertion() {
        didEqualize = false
        needsLayout = true
        layoutSubtreeIfNeeded()
        if !didEqualize {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.needsLayout = true
                self.layoutSubtreeIfNeeded()
            }
        }
    }

    /// Replace one leaf without letting the remaining sibling consume the
    /// removed leaf's entire extent. NSSplitView eagerly expands its sole
    /// child between remove/insert, so the nested replacement otherwise lands
    /// at zero width or height until another structural change occurs.
    func replaceArrangedSubviewPreservingDivider(
        _ target: NSView,
        with replacement: NSView
    ) -> Bool {
        guard arrangedSubviews.count == 2,
              let index = arrangedSubviews.firstIndex(of: target) else {
            return false
        }
        let firstExtent = isVertical
            ? arrangedSubviews[0].frame.width
            : arrangedSubviews[0].frame.height

        removeArrangedSubview(target)
        target.removeFromSuperview()
        replacement.translatesAutoresizingMaskIntoConstraints = true
        insertArrangedSubview(replacement, at: index)
        needsLayout = true
        layoutSubtreeIfNeeded()

        let total = isVertical ? bounds.width : bounds.height
        let position = min(max(firstExtent, 0), max(total - dividerThickness, 0))
        setPosition(position, ofDividerAt: 0)
        return true
    }

    private func equalizeIfReady() {
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

    // JetBrains Mono first, for its programming ligatures and its wider
    // coverage of the symbols shells actually print; Menlo, which ships with
    // macOS, is the guaranteed fallback. Once the font is bundled with the
    // SDK the default stops depending on what the machine happens to have.
    static func regular(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "JetBrainsMono-Regular", size: size)
            ?? NSFont(name: "Menlo-Regular", size: size)
            ?? NSFont(name: "Menlo", size: size)
            ?? NSFont(name: "SFMono-Regular", size: size)
            ?? NSFont(name: "Monaco", size: size)
            ?? NSFont.monospacedSystemFont(ofSize: size, weight: .regular))
    }

    static func bold(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "JetBrainsMono-Bold", size: size)
            ?? NSFont(name: "Menlo-Bold", size: size)
            ?? NSFont(name: "SFMono-Semibold", size: size)
            ?? NSFont.monospacedSystemFont(ofSize: size, weight: .semibold))
    }

    static func italic(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "JetBrainsMono-Italic", size: size)
            ?? NSFont(name: "Menlo-Italic", size: size)
            ?? NSFontManager.shared.convert(regular(size: size), toHaveTrait: .italicFontMask))
    }

    static func boldItalic(size: CGFloat = defaultSize) -> NSFont {
        withCascade(NSFont(name: "JetBrainsMono-BoldItalic", size: size)
            ?? NSFont(name: "Menlo-BoldItalic", size: size)
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

    /// Attach the symbol/emoji fallback chain to a face.
    static func withCascade(_ font: NSFont) -> NSFont {
        let cascade = cascadeNames.compactMap { NSFontDescriptor(name: $0, size: font.pointSize) }
        guard !cascade.isEmpty else { return font }
        let descriptor = font.fontDescriptor.addingAttributes([
            NSFontDescriptor.AttributeName.cascadeList: cascade
        ])
        return NSFont(descriptor: descriptor, size: font.pointSize) ?? font
    }
}

enum LingXiaTerminalSplitDirection: Equatable {
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

private struct LingXiaTerminalAutomationCommand: Decodable {
    struct Params: Decodable {
        var direction: String?
        var maximized: Bool?
        var text: String?
    }

    let id: UInt64
    let action: String
    let params: Params
}

@MainActor
final class LingXiaTerminalWorkspaceView: NSView {
    enum Presentation: Equatable {
        case main
        case aside
    }

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
    private var presentation: Presentation

    var onRequestClosePanel: (() -> Void)?
    var onToggleSurfaceZoom: ((Bool) -> Void)?
    var onActiveTitleChanged: ((String) -> Void)?

    var activeTitle: String? { activeTab()?.displayTitle }

    private let toolbarStack = NSView()
    private let tabRailView = LingXiaTerminalTabRailView()
    private let contentHost = NSView()
    private var tabs: [TerminalTab] = []
    private var activeTabID: UUID?
    private var surfaceZoomed = false
    nonisolated(unsafe) private var mouseEventMonitor: Any?
    nonisolated(unsafe) private var keyEventMonitor: Any?
    nonisolated(unsafe) private var chromeObserver: NSObjectProtocol?
    private var visualTimer: Timer?
    private var lastConfigGeneration = terminalConfigGeneration()
    private var lastVisualGeneration = terminalVisualGeneration()
    private var inputArmed = false

    nonisolated override var isFlipped: Bool { true }

    init(surfaceID: String, presentation: Presentation = .aside) {
        self.surfaceID = surfaceID
        self.presentation = presentation
        super.init(frame: .zero)
        lxTerminalLog("workspace.init surface=\(surfaceID)")
        setupLayout()
        installEventMonitorsIfNeeded()
        createTabAndActivate()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setPresentation(_ presentation: Presentation) {
        guard self.presentation != presentation else { return }
        if presentation == .main {
            setSurfaceZoomEnabled(false, notifyRuntime: false)
        }
        self.presentation = presentation
        tabRailView.showsSurfaceZoomControl = presentation == .aside
        needsLayout = true
    }

    deinit {
        terminalAutomationRemoveWorkspace(surfaceID)
        if let mouseEventMonitor {
            NSEvent.removeMonitor(mouseEventMonitor)
        }
        if let keyEventMonitor {
            NSEvent.removeMonitor(keyEventMonitor)
        }
        if let chromeObserver {
            NotificationCenter.default.removeObserver(chromeObserver)
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        lxTerminalLog("workspace.viewDidMoveToWindow surface=\(surfaceID) hasWindow=\(window != nil)")
        updateEventMonitors()
        if window == nil {
            visualTimer?.invalidate()
            visualTimer = nil
        } else {
            startVisualUpdates()
        }
        focusActiveTerminal()
    }

    private func startVisualUpdates() {
        guard visualTimer == nil else { return }
        let timer = Timer(
            timeInterval: 0.05,
            target: self,
            selector: #selector(refreshVisualIfNeeded),
            userInfo: nil,
            repeats: true
        )
        RunLoop.main.add(timer, forMode: .common)
        visualTimer = timer
    }

    @objc private func refreshVisualIfNeeded() {
        processAutomationCommands()
        let configGeneration = terminalConfigGeneration()
        let visualGeneration = terminalVisualGeneration()
        let configChanged = configGeneration != lastConfigGeneration
        let visualChanged = visualGeneration != lastVisualGeneration
        guard configChanged || visualChanged else { return }
        lastConfigGeneration = configGeneration
        lastVisualGeneration = visualGeneration
        if visualChanged {
            LingXiaTerminalChrome.reload()
        }
        publishAutomationSnapshot()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        guard window != nil else { return }
        let dark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        terminalRefreshAppearance(dark)
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
        publishAutomationSnapshot()
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
        let tabID = tab.id
        let paneID = activePane.paneID
        DispatchQueue.main.async { [weak self, weak activePane] in
            guard let self,
                  let tab = self.tabs.first(where: { $0.id == tabID }),
                  tab.activePaneID == paneID else {
                return
            }
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
        tabRailView.showsSurfaceZoomControl = presentation == .aside

        toolbarStack.addSubview(tabRailView)

        contentHost.wantsLayer = true
        contentHost.layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        contentHost.layer?.zPosition = 0
        contentHost.translatesAutoresizingMaskIntoConstraints = true
        contentHost.setContentHuggingPriority(.defaultLow, for: .horizontal)
        contentHost.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        addSubview(contentHost)
        addSubview(toolbarStack, positioned: .above, relativeTo: contentHost)

        chromeObserver = NotificationCenter.default.addObserver(
            forName: LingXiaTerminalChrome.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.applyChromeColors() }
        }
    }

    /// Re-apply every chrome color a layer is holding a copy of. Layer colors
    /// are snapshots, so a scheme change has to walk them; anything drawn in
    /// `draw(_:)` only needs the redraw below.
    private func applyChromeColors() {
        layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        toolbarStack.layer?.backgroundColor = NSColor.lxTerminalChrome.cgColor
        contentHost.layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
        tabRailView.refreshChromeColors()
        for tab in tabs {
            tab.rootContainer.layer?.backgroundColor = NSColor.lxTerminalBackground.cgColor
            for pane in tab.panes.values {
                pane.refreshChromeColors()
            }
        }
        needsDisplay = true
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
            if pane.shouldPreserveFocus(for: event) {
                return event
            }
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
        let inheritedDirectory = activeTab()
            .flatMap { activePane(in: $0) }
            .flatMap { $0.currentWorkingDirectory() }
        let tab = TerminalTab(processTitle: "terminal")
        let firstPane = makePane(for: tab, initialDirectory: inheritedDirectory)
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
        if let title = activeTitle {
            onActiveTitleChanged?(title)
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

    @discardableResult
    private func splitActivePane(direction: LingXiaTerminalSplitDirection) -> Bool {
        guard let tab = activeTab(),
              let activePane = activePane(in: tab) else {
            lxTerminalLog("workspace.split no-active-pane surface=\(surfaceID) direction=\(direction)", type: .error)
            return false
        }
        lxTerminalLog("workspace.split start surface=\(surfaceID) direction=\(direction) activePane=\(activePane.paneID.uuidString)")

        let newPane = makePane(
            for: tab,
            initialDirectory: activePane.currentWorkingDirectory()
        )
        newPane.translatesAutoresizingMaskIntoConstraints = false
        activePane.translatesAutoresizingMaskIntoConstraints = false
        let split = LingXiaTerminalSplitView()
        // left/right → panes side by side (vertical divider); up/down → stacked.
        split.isVertical = (direction == .left || direction == .right)
        split.translatesAutoresizingMaskIntoConstraints = false

        guard replaceNodeView(activePane, with: split, in: tab.rootContainer) else {
            LXLog.error("terminal split failed: cannot replace pane surface=\(surfaceID)", category: "MacTerminal")
            lxTerminalLog("workspace.split failed-replace surface=\(surfaceID) direction=\(direction)", type: .error)
            return false
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
        tab.rootContainer.needsLayout = true
        tab.rootContainer.layoutSubtreeIfNeeded()
        split.equalizeAfterInsertion()
        updatePaneHighlight(in: tab)
        newPane.focusTerminal()
        lxTerminalLog("workspace.split complete surface=\(surfaceID) direction=\(direction) newPane=\(newPane.paneID.uuidString) totalPanes=\(tab.panes.count)")
        publishAutomationSnapshot()
        return true
    }

    private func processAutomationCommands() {
        for _ in 0..<8 {
            let raw = terminalAutomationTakeCommand(surfaceID).toString()
            guard !raw.isEmpty else { return }
            guard let data = raw.data(using: .utf8),
                  let command = try? JSONDecoder().decode(
                    LingXiaTerminalAutomationCommand.self,
                    from: data
                  ) else {
                continue
            }

            switch command.action {
            case "split":
                let direction: LingXiaTerminalSplitDirection?
                switch command.params.direction {
                case "left": direction = .left
                case "right": direction = .right
                case "up": direction = .up
                case "down": direction = .down
                default: direction = nil
                }
                guard let direction else {
                    _ = terminalAutomationCompleteCommand(
                        command.id,
                        false,
                        "split requires left, right, up, or down"
                    )
                    continue
                }
                guard splitActivePane(direction: direction) else {
                    _ = terminalAutomationCompleteCommand(
                        command.id,
                        false,
                        "terminal surface has no active pane"
                    )
                    continue
                }
                layoutSubtreeIfNeeded()
                let snapshot = automationSnapshotJSON()
                _ = terminalAutomationPublishSnapshot(surfaceID, snapshot)
                _ = terminalAutomationCompleteCommand(command.id, true, snapshot)
            case "setMaximized":
                guard let maximized = command.params.maximized else {
                    _ = terminalAutomationCompleteCommand(
                        command.id,
                        false,
                        "setMaximized requires a boolean 'maximized'"
                    )
                    continue
                }
                setSurfaceZoomEnabled(maximized, notifyRuntime: true)
                layoutSubtreeIfNeeded()
                let zoomSnapshot = automationSnapshotJSON()
                _ = terminalAutomationPublishSnapshot(surfaceID, zoomSnapshot)
                _ = terminalAutomationCompleteCommand(command.id, true, zoomSnapshot)
            case "newTab":
                createTabAndActivate()
                layoutSubtreeIfNeeded()
                let snapshot = automationSnapshotJSON()
                _ = terminalAutomationPublishSnapshot(surfaceID, snapshot)
                _ = terminalAutomationCompleteCommand(command.id, true, snapshot)
            case "input":
                guard let text = command.params.text else {
                    _ = terminalAutomationCompleteCommand(command.id, false, "input requires text")
                    continue
                }
                guard let tab = activeTab(), let activePane = activePane(in: tab) else {
                    _ = terminalAutomationCompleteCommand(
                        command.id,
                        false,
                        "terminal surface has no active pane"
                    )
                    continue
                }
                activePane.sendInput(text)
                let snapshot = automationSnapshotJSON()
                _ = terminalAutomationCompleteCommand(command.id, true, snapshot)
            default:
                _ = terminalAutomationCompleteCommand(
                    command.id,
                    false,
                    "unknown terminal automation action '\(command.action)'"
                )
            }
        }
    }

    private func publishAutomationSnapshot() {
        _ = terminalAutomationPublishSnapshot(surfaceID, automationSnapshotJSON())
    }

    private func automationSnapshotJSON() -> String {
        let tabsSnapshot: [[String: Any]] = tabs.map { tab in
            let active = tab.id == activeTabID
            var value: [String: Any] = [
                "id": tab.id.uuidString,
                "active": active,
                "paneCount": tab.panes.count,
            ]
            if let activePaneID = tab.activePaneID {
                value["activePaneId"] = activePaneID.uuidString
            }
            if let root = tab.rootContainer.subviews.first,
               let tree = automationPaneTree(root, tab: tab, tabActive: active) {
                value["tree"] = tree
            }
            return value
        }
        var value: [String: Any] = [
            "surfaceId": surfaceID,
            "presentation": presentation == .main ? "main" : "aside",
            "visible": window != nil && !isHidden,
            // Expanded to the full content area. A layout sync used to reset
            // this silently, so automation has to be able to assert it holds.
            "maximized": surfaceZoomed,
            "tabCount": tabs.count,
            "paneCount": tabs.reduce(0) { $0 + $1.panes.count },
            "configGeneration": terminalConfigGeneration(),
            "visualGeneration": terminalVisualGeneration(),
            "config": terminalAutomationJSONObject(terminalCurrentConfig().toString()),
            "chrome": terminalAutomationJSONObject(terminalSurfaceChrome().toString()),
            "tabs": tabsSnapshot,
        ]
        if let activeTabID {
            value["activeTabId"] = activeTabID.uuidString
        }
        guard JSONSerialization.isValidJSONObject(value),
              let data = try? JSONSerialization.data(
                withJSONObject: value,
                options: [.sortedKeys]
              ) else {
            return "{}"
        }
        return String(decoding: data, as: UTF8.self)
    }

    private func automationPaneTree(
        _ view: NSView,
        tab: TerminalTab,
        tabActive: Bool
    ) -> [String: Any]? {
        if let pane = view as? LingXiaTerminalPaneView {
            return [
                "kind": "leaf",
                "pane": pane.automationSnapshot(
                    in: tab.rootContainer,
                    active: tabActive && tab.activePaneID == pane.paneID
                ),
            ]
        }
        if let split = view as? LingXiaTerminalSplitView {
            let children = split.arrangedSubviews.compactMap {
                automationPaneTree($0, tab: tab, tabActive: tabActive)
            }
            return [
                "kind": "split",
                "axis": split.isVertical ? "horizontal" : "vertical",
                "children": children,
            ]
        }
        return view.subviews.first.flatMap {
            automationPaneTree($0, tab: tab, tabActive: tabActive)
        }
    }

    private func terminalAutomationJSONObject(_ json: String) -> Any {
        guard let data = json.data(using: .utf8),
              let value = try? JSONSerialization.jsonObject(with: data) else {
            return [String: Any]()
        }
        return value
    }

    /// Reparents an existing pane beside another leaf in the same tab. The
    /// pane view owns its PTY session, so moving the view preserves the
    /// running process, scrollback, current directory, and title state.
    private func movePane(
        sourceID: UUID,
        targetID: UUID,
        direction: LingXiaTerminalSplitDirection,
        tabID: UUID
    ) -> Bool {
        guard sourceID != targetID,
              let tab = tabs.first(where: { $0.id == tabID }),
              tab.id == activeTabID,
              tab.panes.count > 1,
              let source = tab.panes[sourceID],
              let target = tab.panes[targetID],
              source.superview != nil,
              target.superview != nil,
              detachPaneForMove(source, in: tab.rootContainer) else {
            return false
        }

        let split = LingXiaTerminalSplitView()
        split.isVertical = (direction == .left || direction == .right)
        split.translatesAutoresizingMaskIntoConstraints = false
        guard replaceNodeView(target, with: split, in: tab.rootContainer) else {
            LXLog.error("terminal pane move failed: cannot replace target surface=\(surfaceID)", category: "MacTerminal")
            installRootView(source, into: tab.rootContainer)
            return false
        }

        source.translatesAutoresizingMaskIntoConstraints = false
        target.translatesAutoresizingMaskIntoConstraints = false
        if direction == .left || direction == .up {
            split.addArrangedSubview(source)
            split.addArrangedSubview(target)
        } else {
            split.addArrangedSubview(target)
            split.addArrangedSubview(source)
        }

        tab.activePaneID = sourceID
        tab.zoomedPaneID = nil
        applyZoomState(in: tab)
        tab.rootContainer.needsLayout = true
        tab.rootContainer.layoutSubtreeIfNeeded()
        split.equalizeAfterInsertion()
        updatePaneHighlight(in: tab)
        source.focusTerminal()
        lxTerminalLog(
            "workspace.movePane surface=\(surfaceID) tab=\(tabID.uuidString) source=\(sourceID.uuidString) target=\(targetID.uuidString) direction=\(direction)"
        )
        return true
    }

    private func detachPaneForMove(_ pane: LingXiaTerminalPaneView, in root: NSView) -> Bool {
        guard let split = pane.superview as? NSSplitView else {
            return false
        }
        split.removeArrangedSubview(pane)
        pane.removeFromSuperview()
        collapseSingleChildSplit(split, in: root)
        return true
    }

    private func replaceNodeView(_ target: NSView, with replacement: NSView, in root: NSView) -> Bool {
        guard let parent = target.superview else { return false }
        if let split = parent as? NSSplitView {
            if let terminalSplit = split as? LingXiaTerminalSplitView {
                return terminalSplit.replaceArrangedSubviewPreservingDivider(
                    target,
                    with: replacement
                )
            }
            guard let index = split.arrangedSubviews.firstIndex(of: target) else { return false }
            split.removeArrangedSubview(target)
            target.removeFromSuperview()
            replacement.translatesAutoresizingMaskIntoConstraints = true
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

    private func makePane(
        for tab: TerminalTab,
        initialDirectory: String? = nil
    ) -> LingXiaTerminalPaneView {
        let pane = LingXiaTerminalPaneView(initialDirectory: initialDirectory)
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
        pane.onCloseRequested = { [weak self] paneID in
            self?.closePane(paneID, forTabID: tab.id)
        }
        pane.onPaneMoveRequested = { [weak self] sourceID, targetID, direction in
            self?.movePane(
                sourceID: sourceID,
                targetID: targetID,
                direction: direction,
                tabID: tab.id
            ) ?? false
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
            survivor.translatesAutoresizingMaskIntoConstraints = true
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
        let dragEnabled = tab.panes.count > 1 && tab.zoomedPaneID == nil
        for (paneID, pane) in tab.panes {
            pane.isHidden = tab.zoomedPaneID.map { $0 != paneID } ?? false
            pane.setPaneDragEnabled(dragEnabled)
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
