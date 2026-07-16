#if os(macOS)
import AppKit

/// The centered "ready to update" card. Shown once the package has downloaded
/// silently — its job is to surface *what's new* and let the user restart to
/// apply. Reached two ways:
///   - normal update: clicking the bottom-left "ready" sidebar callout
///   - forced update: presented directly (blocking, no Later)
///
/// It attaches as a child window of the app so it follows the window on drag
/// and hides/miniaturizes with it.
@MainActor
final class UpdateAvailableCard: NSObject {
    private static var current: UpdateAvailableCard?

    private let panel: NSPanel
    private let info: UpdateReadyInfo
    private let onRestart: () -> Void
    private let onLater: () -> Void
    private var didChoose = false
    private let root = NSStackView()

    private enum Style {
        static let width: CGFloat = 380
        static let padding: CGFloat = 24
        static let sectionGap: CGFloat = 14
        static let notesMaxHeight: CGFloat = 150
    }

    /// Present the "ready to update" card with release notes. A forced update
    /// omits the Later button (blocking).
    static func presentReady(
        info: UpdateReadyInfo,
        over window: NSWindow?,
        onRestart: @escaping () -> Void,
        onLater: @escaping () -> Void
    ) {
        current?.close()
        let card = UpdateAvailableCard(info: info, onRestart: onRestart, onLater: onLater)
        current = card
        card.build()
        card.show(over: window)
    }

    private init(
        info: UpdateReadyInfo,
        onRestart: @escaping () -> Void,
        onLater: @escaping () -> Void
    ) {
        self.info = info
        self.onRestart = onRestart
        self.onLater = onLater
        self.panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: Style.width, height: 200),
            styleMask: [.titled, .fullSizeContentView],
            backing: .buffered, defer: true)
        super.init()
        buildChrome()
    }

    private func buildChrome() {
        panel.titlebarAppearsTransparent = true
        panel.titleVisibility = .hidden
        panel.isMovableByWindowBackground = true
        panel.standardWindowButton(.closeButton)?.isHidden = true
        panel.standardWindowButton(.miniaturizeButton)?.isHidden = true
        panel.standardWindowButton(.zoomButton)?.isHidden = true

        root.orientation = .vertical
        root.alignment = .width
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
    }

    private func build() {
        root.arrangedSubviews.forEach { $0.removeFromSuperview() }
        root.addArrangedSubview(header())

        if !info.releaseNotes.isEmpty {
            root.addArrangedSubview(divider())
            // Added directly to the (width-aligned) root so the notes fill the
            // card and stay left-aligned — not boxed inside a nested container.
            let notesTitle = Self.label(
                Self.string("lx_update_card_notes_title"), size: 11, weight: .semibold)
            notesTitle.textColor = .secondaryLabelColor
            root.addArrangedSubview(notesTitle)

            let body = NSTextField(labelWithAttributedString: Self.notesAttributed(info.releaseNotes))
            body.lineBreakMode = .byWordWrapping
            body.maximumNumberOfLines = 0
            body.preferredMaxLayoutWidth = Style.width - 2 * Style.padding
            body.setContentHuggingPriority(.defaultLow, for: .horizontal)
            root.addArrangedSubview(body)
        }
        root.addArrangedSubview(divider())

        let restart = NSButton(
            title: Self.string("lx_update_card_restart"), target: self,
            action: #selector(restartClicked))
        restart.bezelStyle = .rounded
        restart.keyEquivalent = "\r"

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
        row.addArrangedSubview(restart)
        root.addArrangedSubview(row)
    }

    private func header() -> NSView {
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
        if !info.version.isEmpty {
            let subtitle = Self.label(
                Self.string("lx_update_card_version", info.version), size: 12, weight: .regular)
            subtitle.textColor = .secondaryLabelColor
            stack.addArrangedSubview(subtitle)
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

    /// Release-notes text with hanging-indent bullets and comfortable spacing.
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

    @objc private func restartClicked() {
        guard !didChoose else { return }
        didChoose = true
        onRestart()
        // The app quits / relaunches; nothing more to do.
    }

    @objc private func laterClicked() {
        guard !didChoose else { return }
        didChoose = true
        onLater()
        close()
    }

    private func show(over window: NSWindow?) {
        root.layoutSubtreeIfNeeded()
        let size = NSSize(width: Style.width, height: root.fittingSize.height)
        panel.setContentSize(size)
        if let window, window.isVisible {
            let f = window.frame
            panel.setFrameOrigin(NSPoint(x: f.midX - size.width / 2, y: f.midY - size.height / 2))
            // Attach as a child window so the card follows the app on drag and
            // hides/miniaturizes with it instead of floating free.
            window.addChildWindow(panel, ordered: .above)
        } else {
            panel.center()
            panel.level = .floating
            panel.isFloatingPanel = true
        }
        NSApp.activate(ignoringOtherApps: true)
        panel.makeKeyAndOrderFront(nil)
    }

    private func close() {
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
        if UpdateAvailableCard.current === self {
            UpdateAvailableCard.current = nil
        }
    }

    private static func label(_ text: String, size: CGFloat, weight: NSFont.Weight) -> NSTextField {
        let field = NSTextField(labelWithString: text)
        field.font = NSFont.systemFont(ofSize: size, weight: weight)
        field.translatesAutoresizingMaskIntoConstraints = false
        return field
    }

    private static func string(_ key: String, _ args: CVarArg...) -> String {
        return L10n.string(key, arguments: args)
    }
}

/// Parsed fields from the `{version, releaseNotes, isForceUpdate}` JSON the Rust
/// side passes to the ready prompt.
struct UpdateReadyInfo {
    let version: String
    let releaseNotes: [String]
    let isForceUpdate: Bool

    init(json: String) {
        var version = ""
        var notes: [String] = []
        var force = false
        if let data = json.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            version = (obj["version"] as? String) ?? ""
            if let arr = obj["releaseNotes"] as? [Any] {
                notes = arr.compactMap { ($0 as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) }
                    .filter { !$0.isEmpty }
            }
            force = (obj["isForceUpdate"] as? Bool) ?? false
        }
        self.version = version
        self.releaseNotes = notes
        self.isForceUpdate = force
    }
}
#endif
