#if os(macOS)
import AppKit

/// The centered "restart to update" modal. Shown for a *forced* host-app
/// update once the package has downloaded silently: there is no dismiss — the
/// user must click Restart Now to apply the staged update. Normal updates use
/// the dismissible sidebar callout instead.
@MainActor
final class UpdateAvailableCard: NSObject {
    private static var current: UpdateAvailableCard?

    private let panel: NSPanel
    private let onRestart: () -> Void
    private let root = NSStackView()

    private enum Style {
        static let width: CGFloat = 380
        static let padding: CGFloat = 24
        static let sectionGap: CGFloat = 14
    }

    /// Present the blocking "ready to restart" modal for a forced update.
    static func presentReady(over window: NSWindow?, onRestart: @escaping () -> Void) {
        current?.close()
        let card = UpdateAvailableCard(onRestart: onRestart)
        current = card
        card.enterReady()
        card.show(over: window)
    }

    private init(onRestart: @escaping () -> Void) {
        self.onRestart = onRestart
        self.panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: Style.width, height: 160),
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

        let row = NSStackView(views: [icon, title])
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

    private func enterReady() {
        root.arrangedSubviews.forEach { $0.removeFromSuperview() }
        root.addArrangedSubview(header())
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
    }

    @objc private func restartClicked() {
        onRestart()
        // The app quits / relaunches; nothing more to do.
    }

    private func show(over window: NSWindow?) {
        root.layoutSubtreeIfNeeded()
        let size = NSSize(width: Style.width, height: root.fittingSize.height)
        panel.setContentSize(size)
        if let window, window.isVisible {
            let f = window.frame
            panel.setFrameOrigin(NSPoint(x: f.midX - size.width / 2, y: f.midY - size.height / 2))
            // Attach as a child window so the prompt tracks the app: it follows
            // when the window is dragged and hides/miniaturizes with it, rather
            // than floating free over the whole screen.
            window.addChildWindow(panel, ordered: .above)
        } else {
            // No host window (shouldn't happen for the card path): fall back to a
            // centered floating panel.
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

    private static func string(_ key: String) -> String {
        NSLocalizedString(key, bundle: Bundle.module, comment: "")
    }
}
#endif
