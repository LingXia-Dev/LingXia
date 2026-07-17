#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// A single page entry in the sidebar (~28pt high)
/// Displays icon + title + optional badge/red dot, with hover/selection highlighting
@MainActor
class SidebarItemView: NSView {

    struct Layout {
        static let height: CGFloat = 30
        static let iconSize: CGFloat = 16
        static let leadingPadding: CGFloat = 30
        static let trailingPadding: CGFloat = 8
        static let iconTitleSpacing: CGFloat = 8
        static let cornerRadius: CGFloat = 6
        /// Inset from item leading edge so the selection background
        /// aligns with the connector line (SidebarGroupView.Layout.groupInset + 14 - groupInset = 14).
        static let selectionLeadingInset: CGFloat = 22
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
    /// Whether this item currently shows a badge or red dot — feeds the
    /// group header's collapsed aggregate.
    private(set) var hasNotification = false
    /// Collapses the hidden badge capsule to zero width so it stops
    /// reserving title space on rows without a badge.
    private var badgeCollapsed: NSLayoutConstraint?
    private var badgeTextPins: [NSLayoutConstraint] = []
    /// Accent bar at the row's leading edge while selected (Windows-style),
    /// colored from the tabbar's selectedColor.
    private let accentBar = NSView()
    /// Selected-state tint from the lxapp's tabbar style (`selectedColor`);
    /// nil falls back to the system accent (spec: style follows the tabbar
    /// config, the shell injects no accent of its own).
    var selectedTint: NSColor? { didSet { updateAppearance() } }
    /// Unselected title tint from the tabbar's `color`; nil keeps the neutral
    /// label color.
    var unselectedTint: NSColor? { didSet { updateAppearance() } }
    /// Icon pair from the item config; selection swaps between them exactly
    /// like the mobile tabbar (colors style TEXT, icons come as a pair).
    private var normalIconPath = ""
    private var selectedIconPath = ""

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
        selectionBackground.layer?.cornerRadius = 7
        addSubview(selectionBackground)
        accentBar.translatesAutoresizingMaskIntoConstraints = false
        accentBar.wantsLayer = true
        accentBar.layer?.cornerRadius = 1.5
        accentBar.isHidden = true
        addSubview(accentBar)
        NSLayoutConstraint.activate([
            // Same axis as the group's attribution line (groupInset + 12,
            // and this row is inset by groupInset already): the bar is the
            // line's thickened, colored segment at the selected row.
            accentBar.centerXAnchor.constraint(equalTo: leadingAnchor, constant: 12.5),
            accentBar.centerYAnchor.constraint(equalTo: centerYAnchor),
            accentBar.widthAnchor.constraint(equalToConstant: 3),
            accentBar.heightAnchor.constraint(equalToConstant: 18),
        ])

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
        badgeBackground.layer?.cornerRadius = 7.5
        badgeBackground.isHidden = true
        addSubview(badgeBackground)

        // Badge label
        badgeLabel.translatesAutoresizingMaskIntoConstraints = false
        badgeLabel.font = NSFont.systemFont(ofSize: 9.5, weight: .semibold)
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
            selectionBackground.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            selectionBackground.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            selectionBackground.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -2),

            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.leadingPadding),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: Layout.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: Layout.iconSize),

            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: Layout.iconTitleSpacing),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: badgeBackground.leadingAnchor, constant: -6),

            // Inside the selection card's edge so a selected row keeps the
            // badge fully on the card, with clear air to the sidebar edge.
            badgeBackground.trailingAnchor.constraint(equalTo: selectionBackground.trailingAnchor, constant: -10),
            badgeBackground.centerYAnchor.constraint(equalTo: centerYAnchor),
            badgeBackground.heightAnchor.constraint(equalToConstant: 15),

            badgeLabel.centerXAnchor.constraint(equalTo: badgeBackground.centerXAnchor),
            badgeLabel.centerYAnchor.constraint(equalTo: badgeBackground.centerYAnchor),


            redDotView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.trailingPadding),
            redDotView.centerYAnchor.constraint(equalTo: centerYAnchor),
            redDotView.widthAnchor.constraint(equalToConstant: 8),
            redDotView.heightAnchor.constraint(equalToConstant: 8),
        ])

        updateAppearance()
    }

    /// Configure with TabBarItem data from Rust
    private func setBadgeVisible(_ visible: Bool) {
        if badgeCollapsed == nil {
            // Equalities pin the capsule's width to its text (inequalities
            // leave it under-determined and the engine stretches it); the
            // zero-width alternative collapses hidden capsules entirely.
            badgeTextPins = [
                badgeLabel.leadingAnchor.constraint(equalTo: badgeBackground.leadingAnchor, constant: 5),
                badgeLabel.trailingAnchor.constraint(equalTo: badgeBackground.trailingAnchor, constant: -5),
                badgeBackground.widthAnchor.constraint(greaterThanOrEqualToConstant: 15),
            ]
            badgeCollapsed = badgeBackground.widthAnchor.constraint(equalToConstant: 0)
        }
        if visible {
            badgeCollapsed?.isActive = false
            NSLayoutConstraint.activate(badgeTextPins)
        } else {
            NSLayoutConstraint.deactivate(badgeTextPins)
            badgeCollapsed?.isActive = true
        }
    }

    func configure(item: TabBarItem) {
        titleLabel.stringValue = item.cachedText

        // Icon pair: selection swaps normal/selected images (mobile parity).
        normalIconPath = item.cachedIconPath
        selectedIconPath = item.cachedSelectedIconPath
        loadIcon(path: isSelected && !selectedIconPath.isEmpty ? selectedIconPath : normalIconPath)

        // Badge / red dot from Rust state
        if let rustItem = getTabBarItem(appId, Int32(itemIndex)) {
            let badgeText = rustItem.badge.toString()
            hasNotification = !badgeText.isEmpty || rustItem.has_red_dot
            if !badgeText.isEmpty {
                badgeLabel.stringValue = badgeText
                badgeLabel.isHidden = false
                badgeBackground.isHidden = false
                setBadgeVisible(true)
                redDotView.isHidden = true
            } else if rustItem.has_red_dot {
                badgeLabel.isHidden = true
                badgeBackground.isHidden = true
                setBadgeVisible(false)
                redDotView.isHidden = false
            } else {
                badgeLabel.isHidden = true
                badgeBackground.isHidden = true
                setBadgeVisible(false)
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
        let accent = selectedTint ?? NSColor.controlAccentColor
        accentBar.isHidden = !isSelected
        accentBar.layer?.backgroundColor = accent.cgColor
        // Selection swaps the icon pair, mirroring the mobile tabbar.
        loadIcon(path: isSelected && !selectedIconPath.isEmpty ? selectedIconPath : normalIconPath)
        if isSelected {
            // Windows-baseline selected card: a light floating card on the
            // dark base, accent icon + accent bar. The title takes the
            // tabbar's selectedColor (mobile parity); a near-neutral dark
            // stands in when the app declares none.
            selectionBackground.layer?.backgroundColor =
                NSColor.white.withAlphaComponent(0.95).cgColor
            selectionBackground.shadow = {
                let shadow = NSShadow()
                shadow.shadowBlurRadius = 6
                shadow.shadowOffset = NSSize(width: 0, height: -1)
                shadow.shadowColor = NSColor.black.withAlphaComponent(0.35)
                return shadow
            }()
            titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .medium)
            titleLabel.textColor = selectedTint ?? NSColor(calibratedWhite: 0.15, alpha: 1)
            iconView.contentTintColor = accent
        } else if isHovered {
            selectionBackground.shadow = nil
            titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .regular)
            selectionBackground.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
            titleLabel.textColor = unselectedTint ?? NSColor.labelColor
            iconView.contentTintColor = NSColor.secondaryLabelColor
        } else {
            selectionBackground.shadow = nil
            titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .regular)
            selectionBackground.layer?.backgroundColor = NSColor.clear.cgColor
            titleLabel.textColor = unselectedTint ?? NSColor.labelColor
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
