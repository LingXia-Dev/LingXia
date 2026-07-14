#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// A single page entry in the sidebar (~28pt high)
/// Displays icon + title + optional badge/red dot, with hover/selection highlighting
@MainActor
class SidebarItemView: NSView {

    struct Layout {
        static let height: CGFloat = 32
        static let iconSize: CGFloat = 16
        static let leadingPadding: CGFloat = 32
        static let trailingPadding: CGFloat = 8
        static let iconTitleSpacing: CGFloat = 8
        static let cornerRadius: CGFloat = 6
        /// Inset from item leading edge so the selection background
        /// aligns with the connector line (SidebarGroupView.Layout.groupInset + 14 - groupInset = 14).
        static let selectionLeadingInset: CGFloat = 14
    }

    private let selectionBackground = NSView()
    private let iconView = NSImageView()
    private let titleLabel = NSTextField(labelWithString: "")
    private let badgeLabel = NSTextField(labelWithString: "")
    private let badgeBackground = NSView()
    private let redDotView = NSView()

    private var trackingArea: NSTrackingArea?
    private(set) var isHovered = false
    var isSelected = false { didSet { updateAppearance() } }

    let itemIndex: Int
    let appId: String
    var onClick: ((Int) -> Void)?

    init(appId: String, itemIndex: Int) {
        self.appId = appId
        self.itemIndex = itemIndex
        super.init(frame: .zero)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var mouseDownCanMoveWindow: Bool { false }

    private func setupViews() {
        wantsLayer = true

        // Selection/hover background — inset to align with the connector line
        selectionBackground.translatesAutoresizingMaskIntoConstraints = false
        selectionBackground.wantsLayer = true
        selectionBackground.layer?.cornerRadius = Layout.cornerRadius
        addSubview(selectionBackground)

        // Icon
        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.imageScaling = .scaleProportionallyUpOrDown
        addSubview(iconView)

        // Title
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 12)
        titleLabel.textColor = NSColor.labelColor
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.maximumNumberOfLines = 1
        addSubview(titleLabel)

        // Badge background
        badgeBackground.translatesAutoresizingMaskIntoConstraints = false
        badgeBackground.wantsLayer = true
        badgeBackground.layer?.backgroundColor = NSColor.systemRed.cgColor
        badgeBackground.layer?.cornerRadius = 8
        badgeBackground.isHidden = true
        addSubview(badgeBackground)

        // Badge label
        badgeLabel.translatesAutoresizingMaskIntoConstraints = false
        badgeLabel.font = NSFont.systemFont(ofSize: 9, weight: .medium)
        badgeLabel.textColor = NSColor.white
        badgeLabel.alignment = .center
        badgeLabel.isHidden = true
        badgeBackground.addSubview(badgeLabel)

        // Red dot
        redDotView.translatesAutoresizingMaskIntoConstraints = false
        redDotView.wantsLayer = true
        redDotView.layer?.backgroundColor = NSColor.systemRed.cgColor
        redDotView.layer?.cornerRadius = 4
        redDotView.isHidden = true
        addSubview(redDotView)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: Layout.height),

            // Selection background: inset from item leading to align with connector line
            selectionBackground.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.selectionLeadingInset),
            selectionBackground.trailingAnchor.constraint(equalTo: trailingAnchor),
            selectionBackground.topAnchor.constraint(equalTo: topAnchor),
            selectionBackground.bottomAnchor.constraint(equalTo: bottomAnchor),

            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.leadingPadding),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: Layout.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: Layout.iconSize),

            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: Layout.iconTitleSpacing),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -Layout.trailingPadding),

            badgeBackground.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.trailingPadding),
            badgeBackground.centerYAnchor.constraint(equalTo: centerYAnchor),
            badgeBackground.heightAnchor.constraint(equalToConstant: 16),
            badgeBackground.widthAnchor.constraint(greaterThanOrEqualToConstant: 16),

            badgeLabel.centerXAnchor.constraint(equalTo: badgeBackground.centerXAnchor),
            badgeLabel.centerYAnchor.constraint(equalTo: badgeBackground.centerYAnchor),
            badgeLabel.leadingAnchor.constraint(greaterThanOrEqualTo: badgeBackground.leadingAnchor, constant: 4),
            badgeLabel.trailingAnchor.constraint(lessThanOrEqualTo: badgeBackground.trailingAnchor, constant: -4),

            redDotView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.trailingPadding),
            redDotView.centerYAnchor.constraint(equalTo: centerYAnchor),
            redDotView.widthAnchor.constraint(equalToConstant: 8),
            redDotView.heightAnchor.constraint(equalToConstant: 8),
        ])

        updateAppearance()
    }

    /// Configure with TabBarItem data from Rust
    func configure(item: TabBarItem) {
        titleLabel.stringValue = item.cachedText

        // Load icon
        let iconPath = item.cachedIconPath
        loadIcon(path: iconPath)

        // Badge / red dot from Rust state
        if let rustItem = getTabBarItem(appId, Int32(itemIndex)) {
            let badgeText = rustItem.badge.toString()
            if !badgeText.isEmpty {
                badgeLabel.stringValue = badgeText
                badgeLabel.isHidden = false
                badgeBackground.isHidden = false
                redDotView.isHidden = true
            } else if rustItem.has_red_dot {
                badgeLabel.isHidden = true
                badgeBackground.isHidden = true
                redDotView.isHidden = false
            } else {
                badgeLabel.isHidden = true
                badgeBackground.isHidden = true
                redDotView.isHidden = true
            }
        }
    }

    private func loadIcon(path: String) {
        if path.hasPrefix("SF:") {
            let symbolName = String(path.dropFirst(3))
            iconView.image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)
            iconView.contentTintColor = NSColor.secondaryLabelColor
        } else if path.hasPrefix("/") {
            if let image = NSImage(contentsOfFile: path) {
                iconView.image = image
                iconView.contentTintColor = NSColor.secondaryLabelColor
            }
        } else if !path.isEmpty {
            // Try bundle image first, then resources path
            if let bundleImage = NSImage(named: path) {
                iconView.image = bundleImage
                iconView.contentTintColor = NSColor.secondaryLabelColor
            } else {
                let resourcesPath = Bundle.main.resourcePath ?? ""
                let fullPath = "\(resourcesPath)/\(appId)/\(path)"
                if let image = NSImage(contentsOfFile: fullPath) {
                    iconView.image = image
                    iconView.contentTintColor = NSColor.secondaryLabelColor
                }
            }
        } else {
            // Fallback: generic page icon
            iconView.image = NSImage(systemSymbolName: "doc", accessibilityDescription: nil)
            iconView.contentTintColor = NSColor.tertiaryLabelColor
        }
    }

    private func updateAppearance() {
        if isSelected {
            selectionBackground.layer?.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.15).cgColor
            titleLabel.textColor = NSColor.controlAccentColor
            iconView.contentTintColor = NSColor.controlAccentColor
        } else if isHovered {
            selectionBackground.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
            titleLabel.textColor = NSColor.labelColor
            iconView.contentTintColor = NSColor.secondaryLabelColor
        } else {
            selectionBackground.layer?.backgroundColor = NSColor.clear.cgColor
            titleLabel.textColor = NSColor.labelColor
            iconView.contentTintColor = NSColor.secondaryLabelColor
        }
    }

    // MARK: - Hit testing

    override func hitTest(_ point: NSPoint) -> NSView? {
        // AppKit gives this point in the superview's coordinate space here.
        guard !isHidden, frame.contains(point) else { return nil }
        return self
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
        updateAppearance()
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
        updateAppearance()
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        return true
    }

    override func mouseDown(with event: NSEvent) {
        onClick?(itemIndex)
    }
}

#endif
