#if os(macOS)
import AppKit

/// A sidebar entry for a browser tab (28pt high, matching SidebarItemView height).
/// Displays globe icon + page title + close button (visible on hover).
@MainActor
class SidebarBrowserItemView: NSView {

    struct Layout {
        static let height: CGFloat = 28
        static let iconSize: CGFloat = 16
        static let leadingPadding: CGFloat = 16
        static let trailingPadding: CGFloat = 8
        static let iconTitleSpacing: CGFloat = 8
        static let cornerRadius: CGFloat = 6
        static let closeButtonSize: CGFloat = 16
    }

    private let iconView = NSImageView()
    private let titleLabel = NSTextField(labelWithString: "")
    private let closeButton = NSButton()

    private var trackingArea: NSTrackingArea?
    private(set) var isHovered = false
    var isSelected = false { didSet { updateAppearance() } }

    let browserId: UUID
    var onClick: ((UUID) -> Void)?
    var onClose: ((UUID) -> Void)?

    init(id: UUID) {
        self.browserId = id
        super.init(frame: .zero)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupViews() {
        wantsLayer = true
        layer?.cornerRadius = Layout.cornerRadius

        // Globe icon
        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.image = NSImage(systemSymbolName: "globe", accessibilityDescription: "Browser")
        iconView.imageScaling = .scaleProportionallyUpOrDown
        iconView.contentTintColor = NSColor.secondaryLabelColor
        addSubview(iconView)

        // Title
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 12)
        titleLabel.textColor = NSColor.labelColor
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.maximumNumberOfLines = 1
        titleLabel.stringValue = "New Tab"
        addSubview(titleLabel)

        // Close button (hidden by default, shown on hover)
        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")
        closeButton.isBordered = false
        closeButton.bezelStyle = .regularSquare
        closeButton.imagePosition = .imageOnly
        closeButton.contentTintColor = NSColor.secondaryLabelColor
        closeButton.target = self
        closeButton.action = #selector(closeClicked)
        closeButton.isHidden = true
        closeButton.wantsLayer = true
        closeButton.layer?.cornerRadius = Layout.closeButtonSize / 2
        addSubview(closeButton)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: Layout.height),

            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.leadingPadding),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: Layout.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: Layout.iconSize),

            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: Layout.iconTitleSpacing),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: closeButton.leadingAnchor, constant: -4),

            closeButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.trailingPadding),
            closeButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: Layout.closeButtonSize),
            closeButton.heightAnchor.constraint(equalToConstant: Layout.closeButtonSize),
        ])

        updateAppearance()
    }

    func configure(title: String, isSelected: Bool) {
        titleLabel.stringValue = title.isEmpty ? "New Tab" : title
        self.isSelected = isSelected
    }

    private func updateAppearance() {
        if isSelected {
            layer?.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.15).cgColor
            titleLabel.textColor = NSColor.controlAccentColor
            iconView.contentTintColor = NSColor.controlAccentColor
        } else if isHovered {
            layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
            titleLabel.textColor = NSColor.labelColor
            iconView.contentTintColor = NSColor.secondaryLabelColor
        } else {
            layer?.backgroundColor = NSColor.clear.cgColor
            titleLabel.textColor = NSColor.labelColor
            iconView.contentTintColor = NSColor.secondaryLabelColor
        }
    }

    @objc private func closeClicked() {
        onClose?(browserId)
    }

    // MARK: - Hit testing

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, frame.contains(point) else { return nil }
        let localPoint = convert(point, from: superview)
        if !closeButton.isHidden {
            let closePoint = convert(localPoint, to: closeButton)
            if closeButton.bounds.contains(closePoint) {
                return closeButton
            }
        }
        return self
    }

    override func mouseDown(with event: NSEvent) {
        onClick?(browserId)
    }

    // MARK: - Mouse tracking

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
        closeButton.isHidden = false
        updateAppearance()
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
        closeButton.isHidden = true
        updateAppearance()
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        return true
    }
}

#endif
