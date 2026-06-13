#if os(macOS)
import AppKit

/// The centered "update" card. A small state machine that floats over the
/// screen and walks the host-app update through its stages:
///
///   prompt        → version + release notes + [Later] [Download & Install]
///   downloading   → live progress bar + "Downloading… N%"
///   ready         → "Update ready" + [Restart Now]
///
/// Force updates omit "Later". When release notes are absent the card stays
/// compact (no empty section).
@MainActor
final class UpdateAvailableCard: NSObject {
    private static var current: UpdateAvailableCard?

    private let panel: NSPanel
    private let appName: String
    private let onDownload: () -> Void
    private let onLater: () -> Void
    private let onRestart: () -> Void
    private var didChoose = false

    private let root = NSStackView()
    private let actionContainer = NSView()
    private let progressBar = NSProgressIndicator()
    private let statusLabel: NSTextField

    private enum Style {
        static let width: CGFloat = 380
        static let padding: CGFloat = 24
        static let sectionGap: CGFloat = 14
        static let notesMaxHeight: CGFloat = 150
    }

    // MARK: - Presentation

    static func present(
        appName: String,
        infoJSON: String,
        over window: NSWindow?,
        onDownload: @escaping () -> Void,
        onLater: @escaping () -> Void,
        onRestart: @escaping () -> Void
    ) {
        current?.close()
        let card = UpdateAvailableCard(
            appName: appName,
            info: UpdateCardInfo(json: infoJSON),
            onDownload: onDownload, onLater: onLater, onRestart: onRestart)
        current = card
        card.show(over: window)
    }

    /// Push download progress (0-100) into an open card. No-op if none is open.
    static func reportProgress(_ percent: Int) {
        current?.setDownloading(percent: percent)
    }

    /// If a card is open, switch it to the "ready / restart" state and return
    /// true. Returns false when no card is open (caller falls back to the
    /// sidebar callout).
    static func handleReady() -> Bool {
        guard let card = current else { return false }
        card.enterReady()
        return true
    }

    private init(
        appName: String,
        info: UpdateCardInfo,
        onDownload: @escaping () -> Void,
        onLater: @escaping () -> Void,
        onRestart: @escaping () -> Void
    ) {
        self.appName = appName
        self.onDownload = onDownload
        self.onLater = onLater
        self.onRestart = onRestart
        self.statusLabel = UpdateAvailableCard.label("", size: 12, weight: .regular)
        self.panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: Style.width, height: 200),
            styleMask: [.titled, .fullSizeContentView],
            backing: .buffered, defer: true)
        super.init()
        buildChrome()
        enterPrompt(info: info)
    }

    // MARK: - Chrome (constant header + swappable action area)

    private func buildChrome() {
        panel.titlebarAppearsTransparent = true
        panel.titleVisibility = .hidden
        panel.isMovableByWindowBackground = true
        panel.standardWindowButton(.closeButton)?.isHidden = true
        panel.standardWindowButton(.miniaturizeButton)?.isHidden = true
        panel.standardWindowButton(.zoomButton)?.isHidden = true

        root.orientation = .vertical
        root.alignment = .width  // arranged subviews stretch to the card width
        root.spacing = Style.sectionGap
        root.translatesAutoresizingMaskIntoConstraints = false
        root.edgeInsets = NSEdgeInsets(
            top: Style.padding, left: Style.padding,
            bottom: Style.padding, right: Style.padding)

        let container = NSView()
        container.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            root.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            root.topAnchor.constraint(equalTo: container.topAnchor),
            root.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            root.widthAnchor.constraint(equalToConstant: Style.width),
        ])
        panel.contentView = container

        actionContainer.translatesAutoresizingMaskIntoConstraints = false

        progressBar.translatesAutoresizingMaskIntoConstraints = false
        progressBar.style = .bar
        progressBar.isIndeterminate = false
        progressBar.minValue = 0
        progressBar.maxValue = 100
    }

    private func header(info: UpdateCardInfo?) -> NSView {
        let icon = NSImageView()
        icon.image = NSApp.applicationIconImage
        icon.imageScaling = .scaleProportionallyUpOrDown
        icon.translatesAutoresizingMaskIntoConstraints = false
        icon.setContentHuggingPriority(.required, for: .horizontal)
        icon.widthAnchor.constraint(equalToConstant: 52).isActive = true
        icon.heightAnchor.constraint(equalToConstant: 52).isActive = true

        let title = Self.label(Self.string("lx_update_card_title"), size: 16, weight: .semibold)
        title.lineBreakMode = .byTruncatingTail
        let stack = NSStackView(views: [title])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 3

        if let info {
            var parts: [String] = []
            if !info.version.isEmpty {
                parts.append(Self.string("lx_update_card_version", info.version))
            }
            if let size = info.humanSize { parts.append(size) }
            if !parts.isEmpty {
                let subtitle = Self.label(parts.joined(separator: " · "), size: 12, weight: .regular)
                subtitle.textColor = .secondaryLabelColor
                stack.addArrangedSubview(subtitle)
            }
        }

        let row = NSStackView(views: [icon, stack])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 14
        row.distribution = .fill
        row.translatesAutoresizingMaskIntoConstraints = false
        return row
    }

    private func divider() -> NSBox {
        let box = NSBox()
        box.boxType = .separator
        box.translatesAutoresizingMaskIntoConstraints = false
        return box
    }

    // MARK: - States

    private func enterPrompt(info: UpdateCardInfo) {
        root.arrangedSubviews.forEach { $0.removeFromSuperview() }
        root.addArrangedSubview(header(info: info))

        // Release notes — only when present, so the card stays compact otherwise.
        if !info.releaseNotes.isEmpty {
            root.addArrangedSubview(divider())
            root.addArrangedSubview(notesView(info.releaseNotes))
        }
        root.addArrangedSubview(divider())

        let install = NSButton(
            title: Self.string("lx_update_card_install"), target: self,
            action: #selector(downloadClicked))
        install.bezelStyle = .rounded
        install.keyEquivalent = "\r"

        let row = NSStackView()
        row.orientation = .horizontal
        row.spacing = 10
        row.translatesAutoresizingMaskIntoConstraints = false
        let spacer = NSView()
        row.addArrangedSubview(spacer)
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        if !info.isForceUpdate {
            let later = NSButton(
                title: Self.string("lx_update_card_later"), target: self,
                action: #selector(laterClicked))
            later.bezelStyle = .rounded
            later.keyEquivalent = "\u{1b}"
            row.addArrangedSubview(later)
        }
        row.addArrangedSubview(install)
        root.addArrangedSubview(row)
        relayout()
    }

    private func enterDownloading() {
        root.arrangedSubviews.forEach { $0.removeFromSuperview() }
        root.addArrangedSubview(header(info: nil))
        root.addArrangedSubview(divider())

        statusLabel.stringValue = Self.string("lx_update_card_downloading", "0%")
        statusLabel.textColor = .secondaryLabelColor

        progressBar.doubleValue = 0

        let stack = NSStackView(views: [statusLabel, progressBar])
        stack.orientation = .vertical
        stack.alignment = .width
        stack.spacing = 10
        stack.translatesAutoresizingMaskIntoConstraints = false
        root.addArrangedSubview(stack)
        relayout()
    }

    private func setDownloading(percent: Int) {
        let clamped = max(0, min(100, percent))
        if root.arrangedSubviews.count < 2 || progressBar.superview == nil {
            enterDownloading()
        }
        progressBar.doubleValue = Double(clamped)
        statusLabel.stringValue = Self.string("lx_update_card_downloading", "\(clamped)%")
    }

    private func enterReady() {
        root.arrangedSubviews.forEach { $0.removeFromSuperview() }
        root.addArrangedSubview(header(info: nil))
        root.addArrangedSubview(divider())

        let ready = Self.label(Self.string("lx_update_card_ready"), size: 13, weight: .regular)
        ready.textColor = .secondaryLabelColor
        root.addArrangedSubview(ready)

        let restart = NSButton(
            title: Self.string("lx_update_card_restart"), target: self,
            action: #selector(restartClicked))
        restart.bezelStyle = .rounded
        restart.keyEquivalent = "\r"

        let row = NSStackView()
        row.orientation = .horizontal
        row.translatesAutoresizingMaskIntoConstraints = false
        let spacer = NSView()
        row.addArrangedSubview(spacer)
        row.addArrangedSubview(restart)
        root.addArrangedSubview(row)
        relayout()
    }

    private func notesView(_ notes: [String]) -> NSView {
        let title = Self.label(Self.string("lx_update_card_notes_title"), size: 11, weight: .semibold)
        title.textColor = .secondaryLabelColor

        let body = NSTextField(labelWithAttributedString: Self.notesAttributed(notes))
        body.lineBreakMode = .byWordWrapping
        body.maximumNumberOfLines = 0
        body.preferredMaxLayoutWidth = Style.width - 2 * Style.padding
        body.translatesAutoresizingMaskIntoConstraints = false

        let textStack = NSStackView(views: [title, body])
        textStack.orientation = .vertical
        textStack.alignment = .leading
        textStack.spacing = 8
        textStack.translatesAutoresizingMaskIntoConstraints = false

        // Scroll only when the notes are long; short lists size to content.
        let scroll = NSScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.hasVerticalScroller = true
        scroll.scrollerStyle = .overlay
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        scroll.documentView = textStack
        textStack.layoutSubtreeIfNeeded()
        let contentHeight = textStack.fittingSize.height
        scroll.heightAnchor.constraint(
            equalToConstant: min(contentHeight, Style.notesMaxHeight)).isActive = true
        NSLayoutConstraint.activate([
            textStack.leadingAnchor.constraint(equalTo: scroll.contentView.leadingAnchor),
            textStack.trailingAnchor.constraint(equalTo: scroll.contentView.trailingAnchor),
            textStack.topAnchor.constraint(equalTo: scroll.contentView.topAnchor),
            textStack.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor),
        ])
        return scroll
    }

    /// Build the release-notes text with hanging-indent bullets (wrapped lines
    /// align under the text, not the bullet) and comfortable line spacing.
    private static func notesAttributed(_ notes: [String]) -> NSAttributedString {
        let indent: CGFloat = 16
        let para = NSMutableParagraphStyle()
        para.firstLineHeadIndent = 0
        para.headIndent = indent
        para.tabStops = [NSTextTab(textAlignment: .left, location: indent)]
        para.defaultTabInterval = indent
        para.paragraphSpacing = 7
        para.lineSpacing = 2
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12.5),
            .foregroundColor: NSColor.labelColor,
            .paragraphStyle: para,
        ]
        let result = NSMutableAttributedString()
        for (index, note) in notes.enumerated() {
            let line = "•\t\(note)" + (index < notes.count - 1 ? "\n" : "")
            result.append(NSAttributedString(string: line, attributes: attrs))
        }
        return result
    }

    // MARK: - Actions

    @objc private func downloadClicked() {
        guard !didChoose else { return }
        didChoose = true
        onDownload()
        enterDownloading()
    }

    @objc private func laterClicked() {
        guard !didChoose else { return }
        didChoose = true
        onLater()
        close()
    }

    @objc private func restartClicked() {
        onRestart()
        // The app quits / relaunches; nothing more to do.
    }

    // MARK: - Window plumbing

    private func show(over window: NSWindow?) {
        relayout(center: true, over: window)
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        NSApp.activate(ignoringOtherApps: true)
        panel.makeKeyAndOrderFront(nil)
    }

    private func relayout(center: Bool = false, over window: NSWindow? = nil) {
        root.layoutSubtreeIfNeeded()
        let size = NSSize(width: Style.width, height: root.fittingSize.height)
        let previousTop = panel.frame.maxY
        panel.setContentSize(size)
        if center {
            if let window, window.isVisible {
                let f = window.frame
                panel.setFrameOrigin(NSPoint(x: f.midX - size.width / 2, y: f.midY - size.height / 2))
            } else {
                panel.center()
            }
        } else {
            // Keep the top edge pinned while the height changes between states.
            var origin = panel.frame.origin
            origin.y = previousTop - panel.frame.height
            panel.setFrameOrigin(origin)
        }
    }

    private func close() {
        panel.orderOut(nil)
        if UpdateAvailableCard.current === self {
            UpdateAvailableCard.current = nil
        }
    }

    // MARK: - Helpers

    private static func label(_ text: String, size: CGFloat, weight: NSFont.Weight) -> NSTextField {
        let field = NSTextField(labelWithString: text)
        field.font = NSFont.systemFont(ofSize: size, weight: weight)
        field.translatesAutoresizingMaskIntoConstraints = false
        return field
    }

    private static func string(_ key: String, _ args: CVarArg...) -> String {
        let format = NSLocalizedString(key, bundle: Bundle.module, comment: "")
        return args.isEmpty ? format : String(format: format, arguments: args)
    }
}

/// Parsed fields from the update-info JSON the Rust side passes.
private struct UpdateCardInfo {
    let version: String
    let sizeBytes: UInt64?
    let releaseNotes: [String]
    let isForceUpdate: Bool

    init(json: String) {
        var version = ""
        var sizeBytes: UInt64?
        var notes: [String] = []
        var force = false
        if let data = json.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            version = (obj["version"] as? String) ?? ""
            if let n = obj["size"] as? NSNumber { sizeBytes = n.uint64Value }
            if let arr = obj["releaseNotes"] as? [Any] {
                notes = arr.compactMap { ($0 as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) }
                    .filter { !$0.isEmpty }
            }
            force = (obj["isForceUpdate"] as? Bool) ?? false
        }
        self.version = version
        self.sizeBytes = sizeBytes
        self.releaseNotes = notes
        self.isForceUpdate = force
    }

    var humanSize: String? {
        guard let sizeBytes else { return nil }
        return ByteCountFormatter.string(fromByteCount: Int64(sizeBytes), countStyle: .file)
    }
}
#endif
