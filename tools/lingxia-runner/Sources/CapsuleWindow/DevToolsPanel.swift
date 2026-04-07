import AppKit

// MARK: - DevToolsLogger

/// Lightweight in-process log bus for the DevTools console.
/// Call `DevToolsLogger.shared.log(...)` from anywhere on the main thread.
@MainActor
public final class DevToolsLogger {
    public static let shared = DevToolsLogger()

    public enum LogLevel { case info, warn, error, debug, nav }

    public struct LogEntry {
        public let timestamp: Date
        public let level: LogLevel
        public let message: String
    }

    private(set) var entries: [LogEntry] = []
    /// Called on main thread whenever a new entry is appended.
    var onNewEntry: ((LogEntry) -> Void)?

    private init() {}

    public func log(_ message: String, level: LogLevel = .info) {
        let entry = LogEntry(timestamp: Date(), level: level, message: message)
        entries.append(entry)
        onNewEntry?(entry)
    }

    func clear() {
        entries.removeAll()
    }
}

// MARK: - DevToolsPanel

/// Right-side DevTools panel docked to the simulator window.
/// Shows a Console tab (navigation events + app logs) and an Info tab (device details).
@MainActor
class DevToolsPanel: NSView {

    static let panelWidth: CGFloat = 380

    // MARK: - Private types

    private enum Tab: Int { case console = 0, info = 1 }

    // MARK: - UI

    private var headerView: NSView!
    private var tabControl: NSSegmentedControl!
    private var consoleScrollView: NSScrollView!
    private var consoleTextView: NSTextView!
    private var infoScrollView: NSScrollView!
    private var infoTextView: NSTextView!

    private var currentTab: Tab = .console

    private static let timeFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()

    // MARK: - Init

    override init(frame: NSRect) {
        super.init(frame: frame)
        buildUI()
        subscribeToLogger()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not implemented") }

    // MARK: - Setup

    private func buildUI() {
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0.11, alpha: 1.0).cgColor

        // 1px left divider
        let divider = NSView()
        divider.wantsLayer = true
        divider.layer?.backgroundColor = NSColor(white: 0.28, alpha: 1.0).cgColor
        divider.translatesAutoresizingMaskIntoConstraints = false
        addSubview(divider)

        // Header (tabs + clear button)
        headerView = buildHeader()
        addSubview(headerView)

        // Console text view
        let (cs, ct) = makeScrollableTextView()
        consoleScrollView = cs
        consoleTextView  = ct
        addSubview(consoleScrollView)

        // Info text view
        let (is_, it) = makeScrollableTextView()
        infoScrollView = is_
        infoTextView   = it
        infoScrollView.isHidden = true
        addSubview(infoScrollView)

        NSLayoutConstraint.activate([
            divider.leadingAnchor.constraint(equalTo: leadingAnchor),
            divider.topAnchor.constraint(equalTo: topAnchor),
            divider.bottomAnchor.constraint(equalTo: bottomAnchor),
            divider.widthAnchor.constraint(equalToConstant: 1),

            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: divider.trailingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerView.heightAnchor.constraint(equalToConstant: 36),

            consoleScrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            consoleScrollView.leadingAnchor.constraint(equalTo: divider.trailingAnchor),
            consoleScrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            consoleScrollView.bottomAnchor.constraint(equalTo: bottomAnchor),

            infoScrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            infoScrollView.leadingAnchor.constraint(equalTo: divider.trailingAnchor),
            infoScrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            infoScrollView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    private func buildHeader() -> NSView {
        let header = NSView()
        header.translatesAutoresizingMaskIntoConstraints = false
        header.wantsLayer = true
        header.layer?.backgroundColor = NSColor(white: 0.15, alpha: 1.0).cgColor

        // Bottom border
        let border = NSView()
        border.wantsLayer = true
        border.layer?.backgroundColor = NSColor(white: 0.25, alpha: 1.0).cgColor
        border.translatesAutoresizingMaskIntoConstraints = false
        header.addSubview(border)

        // Tab selector
        let seg = NSSegmentedControl(
            labels: ["Console", "Info"],
            trackingMode: .selectOne,
            target: self,
            action: #selector(tabChanged(_:))
        )
        seg.selectedSegment = 0
        seg.controlSize = .small
        seg.translatesAutoresizingMaskIntoConstraints = false
        header.addSubview(seg)
        tabControl = seg

        // Clear button (visible only on console tab)
        let clear = NSButton(title: "Clear", target: self, action: #selector(clearConsole))
        clear.bezelStyle = .inline
        clear.controlSize = .small
        clear.translatesAutoresizingMaskIntoConstraints = false
        clear.contentTintColor = NSColor.white.withAlphaComponent(0.6)
        header.addSubview(clear)

        NSLayoutConstraint.activate([
            border.leadingAnchor.constraint(equalTo: header.leadingAnchor),
            border.trailingAnchor.constraint(equalTo: header.trailingAnchor),
            border.bottomAnchor.constraint(equalTo: header.bottomAnchor),
            border.heightAnchor.constraint(equalToConstant: 1),

            seg.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: 8),
            seg.centerYAnchor.constraint(equalTo: header.centerYAnchor),

            clear.trailingAnchor.constraint(equalTo: header.trailingAnchor, constant: -8),
            clear.centerYAnchor.constraint(equalTo: header.centerYAnchor),
        ])

        return header
    }

    private func makeScrollableTextView() -> (NSScrollView, NSTextView) {
        let scroll = NSScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        scroll.backgroundColor = .clear

        let text = NSTextView()
        text.isEditable = false
        text.isSelectable = true
        text.drawsBackground = false
        text.backgroundColor = .clear
        text.textContainerInset = NSSize(width: 8, height: 8)
        text.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        text.textColor = NSColor(white: 0.85, alpha: 1.0)
        text.isAutomaticLinkDetectionEnabled = false
        text.isAutomaticDataDetectionEnabled = false
        scroll.documentView = text

        return (scroll, text)
    }

    private func subscribeToLogger() {
        // Replay existing entries
        for entry in DevToolsLogger.shared.entries {
            appendEntry(entry)
        }
        DevToolsLogger.shared.onNewEntry = { [weak self] entry in
            self?.appendEntry(entry)
        }
    }

    // MARK: - Actions

    @objc private func tabChanged(_ sender: NSSegmentedControl) {
        currentTab = Tab(rawValue: sender.selectedSegment) ?? .console
        consoleScrollView.isHidden = currentTab != .console
        infoScrollView.isHidden    = currentTab != .info
    }

    @objc private func clearConsole() {
        DevToolsLogger.shared.clear()
        consoleTextView.string = ""
    }

    // MARK: - Console output

    private func appendEntry(_ entry: DevToolsLogger.LogEntry) {
        let time = Self.timeFormatter.string(from: entry.timestamp)

        let (color, prefix): (NSColor, String) = {
            switch entry.level {
            case .error: return (.systemRed,                             "✗")
            case .warn:  return (.systemOrange,                          "⚠")
            case .nav:   return (NSColor(white: 0.55, alpha: 1.0),      "→")
            case .debug: return (.systemBlue,                            "◆")
            case .info:  return (NSColor(white: 0.65, alpha: 1.0),      "·")
            }
        }()

        let line = "\(prefix) \(time)  \(entry.message)\n"
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 11, weight: .regular),
            .foregroundColor: color,
        ]
        guard let storage = consoleTextView.textStorage else { return }
        storage.append(NSAttributedString(string: line, attributes: attrs))
        // Auto-scroll
        consoleTextView.scrollRangeToVisible(NSRange(location: storage.length, length: 0))
    }

    // MARK: - Info tab

    func updateInfo(device: MobileDeviceSize, path: String?) {
        let typeLabel = device.isDesktop ? "Desktop / Tablet" : "Phone"
        let info = """
        Device    \(device.displayName)
        Type      \(typeLabel)
        Viewport  \(Int(device.width)) × \(Int(device.height)) pt
        Path      \(path ?? "(none)")
        """
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 11, weight: .regular),
            .foregroundColor: NSColor(white: 0.8, alpha: 1.0),
        ]
        infoTextView.textStorage?.setAttributedString(
            NSAttributedString(string: info, attributes: attrs)
        )
    }
}
