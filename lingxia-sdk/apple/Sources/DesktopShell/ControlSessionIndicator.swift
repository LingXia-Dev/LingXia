#if os(macOS)
import AppKit

/// What the shell shows while an AI assistant drives the app over its control
/// socket, modelled on computer-use agents: hard to miss, never in the way.
///
/// - a pulsing border around the whole window, which ignores the mouse;
/// - a capsule at the bottom centre of the content, "● An AI assistant is in
///   control   Stop";
/// - an orange dot on the Dock icon, for when the app is in the background.
@MainActor
final class ControlSessionIndicator {
    private let border = ControlSessionBorderView()
    private let capsule: ControlSessionCapsule
    private var dockMark: NSView?

    fileprivate enum Style {
        static let tint = NSColor.systemOrange
        static let pulse = (from: 1.0, to: 0.45, duration: 1.2)
    }

    init(onStop: @escaping () -> Void) {
        capsule = ControlSessionCapsule(onStop: onStop)
    }

    /// `content` is the view the capsule centres on; both views go on the
    /// window's top layer.
    func show(in window: NSWindow, content: NSView) {
        guard let root = window.contentView else { return }
        border.translatesAutoresizingMaskIntoConstraints = false
        capsule.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(border, positioned: .above, relativeTo: nil)
        root.addSubview(capsule, positioned: .above, relativeTo: nil)
        NSLayoutConstraint.activate([
            border.topAnchor.constraint(equalTo: root.topAnchor),
            border.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            border.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            border.bottomAnchor.constraint(equalTo: root.bottomAnchor),

            capsule.centerXAnchor.constraint(equalTo: content.centerXAnchor),
            capsule.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -16),
            capsule.leadingAnchor.constraint(greaterThanOrEqualTo: content.leadingAnchor, constant: 12),
            capsule.trailingAnchor.constraint(lessThanOrEqualTo: content.trailingAnchor, constant: -12),
        ])
        border.startPulse()
        capsule.startPulse()
        showDockMark()
    }

    func dismiss() {
        border.removeFromSuperview()
        capsule.removeFromSuperview()
        hideDockMark()
    }

    /// Draws the app icon with a dot in the corner. The Dock keeps drawing a
    /// product badge label on top, so `setAppBadge` still works meanwhile.
    private func showDockMark() {
        let tile = NSApp.dockTile
        let icon = NSImageView(image: NSApp.applicationIconImage)
        icon.frame = NSRect(origin: .zero, size: tile.size)
        let side = tile.size.width * 0.28
        let dot = NSView(frame: NSRect(x: 0, y: 0, width: side, height: side))
        dot.wantsLayer = true
        dot.layer?.cornerRadius = side / 2
        dot.layer?.backgroundColor = Style.tint.cgColor
        dot.layer?.borderColor = NSColor.white.cgColor
        dot.layer?.borderWidth = side * 0.12
        icon.addSubview(dot)
        tile.contentView = icon
        tile.display()
        dockMark = icon
    }

    private func hideDockMark() {
        guard dockMark != nil else { return }
        NSApp.dockTile.contentView = nil
        NSApp.dockTile.display()
        dockMark = nil
    }
}

private func addPulse(to layer: CALayer?) {
    guard let layer, !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion else { return }
    let pulse = CABasicAnimation(keyPath: "opacity")
    pulse.fromValue = ControlSessionIndicator.Style.pulse.from
    pulse.toValue = ControlSessionIndicator.Style.pulse.to
    pulse.duration = ControlSessionIndicator.Style.pulse.duration
    pulse.autoreverses = true
    pulse.repeatCount = .infinity
    pulse.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
    layer.add(pulse, forKey: "pulse")
}

/// A tinted frame around the window. Clicks pass straight through.
@MainActor
private final class ControlSessionBorderView: NSView {
    private static let width: CGFloat = 3
    // macOS window corners; the window clips anything outside them.
    private static let cornerRadius: CGFloat = 10

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.borderWidth = Self.width
        layer?.cornerRadius = Self.cornerRadius
        layer?.borderColor = themeCGColor(ControlSessionIndicator.Style.tint)
        setAccessibilityElement(false)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    func startPulse() { addPulse(to: layer) }
}

/// "● An AI assistant is in control   Stop"
@MainActor
private final class ControlSessionCapsule: NSView {
    private let onStop: () -> Void
    private let dot = NSView()

    private enum Style {
        static let height: CGFloat = 32
        static let horizontalPadding: CGFloat = 14
        static let dotSize: CGFloat = 8
        static let gap: CGFloat = 8
        static let stopGap: CGFloat = 14
        // The ink of the other shell notices.
        static let background = NSColor(calibratedRed: 0.13, green: 0.15, blue: 0.17, alpha: 0.97)
    }

    init(onStop: @escaping () -> Void) {
        self.onStop = onStop
        super.init(frame: .zero)
        setup()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setup() {
        wantsLayer = true
        layer?.cornerRadius = Style.height / 2
        layer?.backgroundColor = themeCGColor(Style.background)
        layer?.borderWidth = 1
        layer?.borderColor = themeCGColor(ControlSessionIndicator.Style.tint.withAlphaComponent(0.6))
        shadow = NSShadow()
        layer?.shadowColor = NSColor.black.cgColor
        layer?.shadowOpacity = 0.25
        layer?.shadowRadius = 8
        layer?.shadowOffset = CGSize(width: 0, height: -2)

        dot.wantsLayer = true
        dot.layer?.cornerRadius = Style.dotSize / 2
        dot.layer?.backgroundColor = themeCGColor(ControlSessionIndicator.Style.tint)
        dot.translatesAutoresizingMaskIntoConstraints = false

        let title = NSTextField(labelWithString: L10n.string("lx_control_session_active"))
        title.font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        title.textColor = .white
        title.lineBreakMode = .byTruncatingTail
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        title.translatesAutoresizingMaskIntoConstraints = false

        let stopTitle = L10n.string("lx_control_session_stop")
        let stop = NSButton(title: stopTitle, target: self, action: #selector(stopClicked))
        stop.isBordered = false
        stop.attributedTitle = NSAttributedString(
            string: stopTitle,
            attributes: [
                .font: NSFont.systemFont(ofSize: 12, weight: .semibold),
                .foregroundColor: ControlSessionIndicator.Style.tint,
            ])
        stop.setContentCompressionResistancePriority(.required, for: .horizontal)
        stop.translatesAutoresizingMaskIntoConstraints = false

        addSubview(dot)
        addSubview(title)
        addSubview(stop)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: Style.height),

            dot.widthAnchor.constraint(equalToConstant: Style.dotSize),
            dot.heightAnchor.constraint(equalToConstant: Style.dotSize),
            dot.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Style.horizontalPadding),
            dot.centerYAnchor.constraint(equalTo: centerYAnchor),

            title.leadingAnchor.constraint(equalTo: dot.trailingAnchor, constant: Style.gap),
            title.centerYAnchor.constraint(equalTo: centerYAnchor),

            stop.leadingAnchor.constraint(equalTo: title.trailingAnchor, constant: Style.stopGap),
            stop.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Style.horizontalPadding),
            stop.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])

        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel(title.stringValue)
    }

    override func resetCursorRects() {
        for case let button as NSButton in subviews {
            addCursorRect(button.frame, cursor: .pointingHand)
        }
    }

    func startPulse() { addPulse(to: dot.layer) }

    @objc private func stopClicked() {
        onStop()
    }
}
#endif
