#if os(macOS)
import AppKit

/// Which stage of the update flow the sidebar callout represents.
enum UpdateCalloutState {
    /// The update was deferred ("Later") and is available to install.
    case available
    /// The update is downloaded and staged; clicking restarts to apply it.
    case ready
}

/// A small two-line callout shown above the bottom-left sidebar icon. Depending
/// on `state` it reads either:
///
///     <AppName> update available        <AppName> is ready to update!
///     Click to install            or    Click to restart
///
/// The whole bubble is one click target; clicking it invokes `onClick`, which
/// the shell routes to the matching update action.
@MainActor
final class UpdateReadyCallout: NSView {
    private let onClick: () -> Void
    private var trackingArea: NSTrackingArea?

    private enum Style {
        static let cornerRadius: CGFloat = 8
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 8
        static let lineSpacing: CGFloat = 2
        static let maxWidth: CGFloat = 220

        // Neutral dark "ink" bubble (matches the brand mark's dark U), not the
        // system accent — reads as a calm notification rather than a loud blue.
        static let background = NSColor(calibratedRed: 0.13, green: 0.15, blue: 0.17, alpha: 0.97)
        static func hovered() -> NSColor {
            background.blended(withFraction: 0.10, of: .white) ?? background
        }
        static func pressed() -> NSColor {
            background.blended(withFraction: 0.18, of: .white) ?? background
        }
    }

    init(appName: String, state: UpdateCalloutState, onClick: @escaping () -> Void) {
        self.onClick = onClick
        super.init(frame: .zero)
        setup(appName: appName, state: state)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setup(appName: String, state: UpdateCalloutState) {
        wantsLayer = true
        layer?.cornerRadius = Style.cornerRadius
        layer?.backgroundColor = Style.background.cgColor
        // Soft drop shadow so it reads as a floating bubble.
        shadow = NSShadow()
        layer?.shadowColor = NSColor.black.cgColor
        layer?.shadowOpacity = 0.22
        layer?.shadowRadius = 6
        layer?.shadowOffset = CGSize(width: 0, height: -1)

        let titleKey = (state == .ready) ? "lx_update_ready_to_install" : "lx_update_available_title"
        let subtitleKey = (state == .ready) ? "lx_update_click_to_restart" : "lx_update_click_to_install"

        let title = NSTextField(labelWithString: L10n.string(titleKey, appName))
        title.font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        title.textColor = .white
        title.lineBreakMode = .byWordWrapping
        title.maximumNumberOfLines = 2
        title.preferredMaxLayoutWidth = Style.maxWidth - 2 * Style.horizontalPadding
        title.translatesAutoresizingMaskIntoConstraints = false

        let subtitle = NSTextField(labelWithString: L10n.string(subtitleKey))
        subtitle.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        subtitle.textColor = NSColor.white.withAlphaComponent(0.85)
        subtitle.translatesAutoresizingMaskIntoConstraints = false

        addSubview(title)
        addSubview(subtitle)

        NSLayoutConstraint.activate([
            widthAnchor.constraint(lessThanOrEqualToConstant: Style.maxWidth),

            title.topAnchor.constraint(equalTo: topAnchor, constant: Style.verticalPadding),
            title.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Style.horizontalPadding),
            title.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Style.horizontalPadding),

            subtitle.topAnchor.constraint(equalTo: title.bottomAnchor, constant: Style.lineSpacing),
            subtitle.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Style.horizontalPadding),
            subtitle.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -Style.horizontalPadding),
            subtitle.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -Style.verticalPadding),
        ])

        toolTip = subtitle.stringValue
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp],
            owner: self,
            userInfo: nil)
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        NSCursor.pointingHand.set()
        layer?.backgroundColor = Style.hovered().cgColor
    }

    override func mouseExited(with event: NSEvent) {
        NSCursor.arrow.set()
        layer?.backgroundColor = Style.background.cgColor
    }

    override func mouseDown(with event: NSEvent) {
        // Brief press feedback, then fire.
        layer?.backgroundColor = Style.pressed().cgColor
    }

    override func mouseUp(with event: NSEvent) {
        let inside = bounds.contains(convert(event.locationInWindow, from: nil))
        layer?.backgroundColor = Style.background.cgColor
        if inside {
            onClick()
        }
    }
}
#endif
