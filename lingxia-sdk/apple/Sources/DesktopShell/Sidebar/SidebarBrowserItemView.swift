#if os(macOS)
import AppKit

/// A sidebar entry for a browser tab (36pt top-level row).
/// Displays globe icon + page title, a "···" menu button on hover and a close
/// button on the selected tab.
@MainActor
class SidebarBrowserItemView: NSView {

    struct Layout {
        static let height: CGFloat = 36
        static let iconSize: CGFloat = 16
        // Matches SidebarGroupView.Layout.headerHPadding so an lxapp group's
        // icon and a browser tab's icon line up in the same column (both sit
        // groupInset + this from the sidebar edge).
        static let leadingPadding: CGFloat = 8
        static let trailingPadding: CGFloat = 8
        static let iconTitleSpacing: CGFloat = 8
        static let cornerRadius: CGFloat = 6
        static let closeButtonSize: CGFloat = 16
    }

    private let selectionBackground = NSView()
    private let iconView = NSImageView()
    private let titleLabel = NSTextField(labelWithString: "")
    private let closeButton = NSButton()
    private let moreButton = NSButton()
    private let trailingControls = NSStackView()

    private var trackingArea: NSTrackingArea?
    private var closeTrackingArea: NSTrackingArea?
    private var moreTrackingArea: NSTrackingArea?
    private(set) var isHovered = false
    var isSelected = false { didSet { updateAppearance() } }

    let browserId: String
    var onClick: ((String) -> Void)?
    var onClose: ((String) -> Void)?
    var contextMenuProvider: ((String) -> NSMenu?)?

    init(id: String) {
        self.browserId = id
        super.init(frame: .zero)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var mouseDownCanMoveWindow: Bool { false }

    private func setupViews() {
        wantsLayer = true

        // Selection/hover background
        selectionBackground.translatesAutoresizingMaskIntoConstraints = false
        selectionBackground.wantsLayer = true
        selectionBackground.layer?.cornerRadius = Layout.cornerRadius
        addSubview(selectionBackground)

        // Globe icon
        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.image = LxIcon.image(
            named: "icon_globe",
            size: CGSize(width: Layout.iconSize, height: Layout.iconSize)
        )
        iconView.imageScaling = .scaleProportionallyUpOrDown
        iconView.contentTintColor = LxAppHostTheme.mutedForeground
        iconView.setAccessibilityLabel(L10n.string("lx_browser_label"))
        addSubview(iconView)

        // Title
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 12)
        titleLabel.textColor = LxAppHostTheme.foreground
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.maximumNumberOfLines = 1
        titleLabel.stringValue = L10n.string("lx_browser_new_tab")
        addSubview(titleLabel)

        // Close button (only visible for selected tab)
        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.image = LxIcon.image(
            named: "icon_close_x",
            size: CGSize(width: 12, height: 12)
        )
        closeButton.isBordered = false
        closeButton.bezelStyle = .regularSquare
        closeButton.imagePosition = .imageOnly
        closeButton.contentTintColor = LxAppHostTheme.mutedForeground
        closeButton.toolTip = L10n.string("lx_common_close")
        closeButton.setAccessibilityLabel(L10n.string("lx_common_close"))
        closeButton.target = self
        closeButton.action = #selector(closeClicked)
        closeButton.isHidden = true
        closeButton.wantsLayer = true
        closeButton.layer?.cornerRadius = Layout.closeButtonSize / 2

        // "⋮": the same menu as a right-click, shown on hover; the lxapp
        // group header's glyph.
        moreButton.translatesAutoresizingMaskIntoConstraints = false
        moreButton.image = SidebarGroupView.verticalEllipsisImage()
        moreButton.isBordered = false
        moreButton.bezelStyle = .regularSquare
        moreButton.imagePosition = .imageOnly
        moreButton.imageScaling = .scaleProportionallyDown
        moreButton.contentTintColor = LxAppHostTheme.mutedForeground
        moreButton.target = self
        moreButton.action = #selector(moreClicked)
        moreButton.isHidden = true
        moreButton.wantsLayer = true
        moreButton.layer?.cornerRadius = Layout.closeButtonSize / 2

        trailingControls.translatesAutoresizingMaskIntoConstraints = false
        trailingControls.orientation = .horizontal
        trailingControls.spacing = 2
        trailingControls.detachesHiddenViews = true
        trailingControls.addArrangedSubview(moreButton)
        trailingControls.addArrangedSubview(closeButton)
        addSubview(trailingControls)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: Layout.height),

            selectionBackground.leadingAnchor.constraint(equalTo: leadingAnchor),
            selectionBackground.trailingAnchor.constraint(equalTo: trailingAnchor),
            selectionBackground.topAnchor.constraint(equalTo: topAnchor),
            selectionBackground.bottomAnchor.constraint(equalTo: bottomAnchor),

            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.leadingPadding),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: Layout.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: Layout.iconSize),

            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: Layout.iconTitleSpacing),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingControls.leadingAnchor, constant: -4),

            trailingControls.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.trailingPadding),
            trailingControls.centerYAnchor.constraint(equalTo: centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: Layout.closeButtonSize),
            closeButton.heightAnchor.constraint(equalToConstant: Layout.closeButtonSize),
            moreButton.widthAnchor.constraint(equalToConstant: Layout.closeButtonSize),
            moreButton.heightAnchor.constraint(equalToConstant: Layout.closeButtonSize),
        ])

        updateAppearance()
    }

    func configure(title: String, url: String, isSelected: Bool, favicon: NSImage?) {
        let resolvedTitle = title.isEmpty ? L10n.string("lx_browser_new_tab") : title
        titleLabel.stringValue = resolvedTitle
        let trimmedURL = url.trimmingCharacters(in: .whitespacesAndNewlines)
        toolTip = trimmedURL.isEmpty ? resolvedTitle : "\(resolvedTitle)\n\(trimmedURL)"
        // Update icon before isSelected so updateAppearance sees the correct tintColor state
        if let favicon {
            iconView.image = favicon
            iconView.contentTintColor = nil
        } else {
            iconView.image = LxIcon.image(
                named: "icon_globe",
                size: CGSize(width: Layout.iconSize, height: Layout.iconSize)
            )
            iconView.contentTintColor = LxAppHostTheme.mutedForeground  // updateAppearance will refine
        }
        self.isSelected = isSelected
    }

    private func updateAppearance() {
        closeButton.isHidden = !isSelected
        moreButton.isHidden = !isHovered || contextMenuProvider == nil
        moreButton.contentTintColor = isSelected ? LxAppHostTheme.accent : LxAppHostTheme.mutedForeground

        // Only tint the icon when it's the globe SF symbol (contentTintColor nil = favicon, skip tinting)
        let isFavicon = iconView.contentTintColor == nil
        if isSelected {
            selectionBackground.layer?.backgroundColor = themeCGColor(LxAppHostTheme.selectionBackground)
            titleLabel.textColor = LxAppHostTheme.accent
            if !isFavicon { iconView.contentTintColor = LxAppHostTheme.accent }
            closeButton.contentTintColor = LxAppHostTheme.accent
        } else if isHovered {
            selectionBackground.layer?.backgroundColor = themeCGColor(NSColor.labelColor.withAlphaComponent(0.06))
            titleLabel.textColor = LxAppHostTheme.foreground
            if !isFavicon { iconView.contentTintColor = LxAppHostTheme.mutedForeground }
            closeButton.contentTintColor = LxAppHostTheme.foreground
        } else {
            selectionBackground.layer?.backgroundColor = NSColor.clear.cgColor
            titleLabel.textColor = LxAppHostTheme.foreground
            if !isFavicon { iconView.contentTintColor = LxAppHostTheme.mutedForeground }
            closeButton.contentTintColor = LxAppHostTheme.mutedForeground
        }
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateAppearance()
    }

    @objc private func closeClicked() {
        onClose?(browserId)
    }

    @objc private func moreClicked() {
        guard let menu = contextMenuProvider?(browserId) else { return }
        menu.popUp(
            positioning: nil,
            at: NSPoint(x: 0, y: moreButton.bounds.maxY + 4),
            in: moreButton
        )
    }

    // MARK: - Hit testing

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, frame.contains(point) else { return nil }
        let localPoint = convert(point, from: superview)
        for button in [closeButton, moreButton] where !button.isHidden {
            if button.bounds.contains(convert(localPoint, to: button)) {
                return button
            }
        }
        return self
    }

    override func mouseDown(with event: NSEvent) {
        onClick?(browserId)
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        contextMenuProvider?(browserId) ?? super.menu(for: event)
    }

    // MARK: - Mouse tracking

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: ["zone": "item"]
        )
        addTrackingArea(area)
        trackingArea = area

        if let existing = closeTrackingArea {
            closeButton.removeTrackingArea(existing)
        }
        let closeArea = NSTrackingArea(
            rect: closeButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: ["zone": "close"]
        )
        closeButton.addTrackingArea(closeArea)
        closeTrackingArea = closeArea

        if let existing = moreTrackingArea {
            moreButton.removeTrackingArea(existing)
        }
        let moreArea = NSTrackingArea(
            rect: moreButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: ["zone": "more"]
        )
        moreButton.addTrackingArea(moreArea)
        moreTrackingArea = moreArea
    }

    override func mouseEntered(with event: NSEvent) {
        let zone = event.trackingArea?.userInfo?["zone"] as? String
        if zone == "item" {
            isHovered = true
            updateAppearance()
        } else if zone == "close" {
            closeButton.layer?.backgroundColor = themeCGColor(LxAppHostTheme.foreground.withAlphaComponent(0.12))
        } else if zone == "more" {
            moreButton.layer?.backgroundColor = themeCGColor(LxAppHostTheme.foreground.withAlphaComponent(0.12))
        }
    }

    override func mouseExited(with event: NSEvent) {
        let zone = event.trackingArea?.userInfo?["zone"] as? String
        if zone == "item" {
            isHovered = false
            closeButton.layer?.backgroundColor = nil
            moreButton.layer?.backgroundColor = nil
            updateAppearance()
        } else if zone == "close" {
            closeButton.layer?.backgroundColor = nil
        } else if zone == "more" {
            moreButton.layer?.backgroundColor = nil
        }
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        return true
    }
}

#endif
