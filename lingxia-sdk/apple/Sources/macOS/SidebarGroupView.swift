#if os(macOS)
import AppKit
import CLingXiaRustAPI

// MARK: - Chrome-style color palette

/// Nine Chrome tab-group colors (light-mode header backgrounds).
/// Each tuple: (header background, header text, items tint, connector line)
@MainActor
enum SidebarGroupColor {
    struct Palette {
        let headerBg: NSColor
        let headerText: NSColor
        let itemsTint: NSColor
        let connector: NSColor
    }

    private static let palettes: [Palette] = [
        // Blue
        Palette(headerBg:  NSColor(red: 0.55, green: 0.73, blue: 0.97, alpha: 1.0),
                headerText: NSColor(red: 0.10, green: 0.25, blue: 0.50, alpha: 1.0),
                itemsTint:  NSColor(red: 0.55, green: 0.73, blue: 0.97, alpha: 0.12),
                connector:  NSColor(red: 0.55, green: 0.73, blue: 0.97, alpha: 0.40)),
        // Green
        Palette(headerBg:  NSColor(red: 0.55, green: 0.85, blue: 0.65, alpha: 1.0),
                headerText: NSColor(red: 0.10, green: 0.35, blue: 0.15, alpha: 1.0),
                itemsTint:  NSColor(red: 0.55, green: 0.85, blue: 0.65, alpha: 0.12),
                connector:  NSColor(red: 0.55, green: 0.85, blue: 0.65, alpha: 0.40)),
        // Yellow
        Palette(headerBg:  NSColor(red: 0.98, green: 0.85, blue: 0.45, alpha: 1.0),
                headerText: NSColor(red: 0.45, green: 0.35, blue: 0.05, alpha: 1.0),
                itemsTint:  NSColor(red: 0.98, green: 0.85, blue: 0.45, alpha: 0.12),
                connector:  NSColor(red: 0.98, green: 0.85, blue: 0.45, alpha: 0.40)),
        // Pink
        Palette(headerBg:  NSColor(red: 0.95, green: 0.60, blue: 0.72, alpha: 1.0),
                headerText: NSColor(red: 0.50, green: 0.10, blue: 0.25, alpha: 1.0),
                itemsTint:  NSColor(red: 0.95, green: 0.60, blue: 0.72, alpha: 0.12),
                connector:  NSColor(red: 0.95, green: 0.60, blue: 0.72, alpha: 0.40)),
        // Purple
        Palette(headerBg:  NSColor(red: 0.72, green: 0.58, blue: 0.92, alpha: 1.0),
                headerText: NSColor(red: 0.30, green: 0.15, blue: 0.55, alpha: 1.0),
                itemsTint:  NSColor(red: 0.72, green: 0.58, blue: 0.92, alpha: 0.12),
                connector:  NSColor(red: 0.72, green: 0.58, blue: 0.92, alpha: 0.40)),
        // Cyan
        Palette(headerBg:  NSColor(red: 0.45, green: 0.82, blue: 0.88, alpha: 1.0),
                headerText: NSColor(red: 0.08, green: 0.32, blue: 0.38, alpha: 1.0),
                itemsTint:  NSColor(red: 0.45, green: 0.82, blue: 0.88, alpha: 0.12),
                connector:  NSColor(red: 0.45, green: 0.82, blue: 0.88, alpha: 0.40)),
        // Orange
        Palette(headerBg:  NSColor(red: 0.98, green: 0.72, blue: 0.42, alpha: 1.0),
                headerText: NSColor(red: 0.50, green: 0.28, blue: 0.05, alpha: 1.0),
                itemsTint:  NSColor(red: 0.98, green: 0.72, blue: 0.42, alpha: 0.12),
                connector:  NSColor(red: 0.98, green: 0.72, blue: 0.42, alpha: 0.40)),
        // Red
        Palette(headerBg:  NSColor(red: 0.94, green: 0.55, blue: 0.52, alpha: 1.0),
                headerText: NSColor(red: 0.50, green: 0.12, blue: 0.10, alpha: 1.0),
                itemsTint:  NSColor(red: 0.94, green: 0.55, blue: 0.52, alpha: 0.12),
                connector:  NSColor(red: 0.94, green: 0.55, blue: 0.52, alpha: 0.40)),
        // Grey
        Palette(headerBg:  NSColor(red: 0.75, green: 0.76, blue: 0.78, alpha: 1.0),
                headerText: NSColor(red: 0.25, green: 0.26, blue: 0.28, alpha: 1.0),
                itemsTint:  NSColor(red: 0.75, green: 0.76, blue: 0.78, alpha: 0.10),
                connector:  NSColor(red: 0.75, green: 0.76, blue: 0.78, alpha: 0.35)),
    ]

    static func palette(for index: Int) -> Palette {
        palettes[index % palettes.count]
    }
}

// MARK: - Header view (custom hitTest)

/// Header view that intercepts all clicks except on the close button.
/// Overrides hitTest so that child views (indicator, label) don't
/// swallow mouse events — the header itself receives mouseDown for toggling.
@MainActor
private class SidebarGroupHeaderView: NSView {
    var closeButton: NSButton?
    var onHeaderClicked: (() -> Void)?
    var onRightClick: ((NSEvent) -> Void)?

    override func hitTest(_ point: NSPoint) -> NSView? {
        // point is in superview's coordinate system → check against frame, not bounds
        guard !isHidden, frame.contains(point) else { return nil }
        // Convert to local coordinates for subview checks
        let localPoint = convert(point, from: superview)
        if let close = closeButton, !close.isHidden, close.frame.contains(localPoint) {
            return close
        }
        return self
    }

    override func mouseDown(with event: NSEvent) {
        onHeaderClicked?()
    }

    override func rightMouseDown(with event: NSEvent) {
        onRightClick?(event)
    }
}

// MARK: - SidebarGroupView

/// A collapsible group in the sidebar representing a single lxapp.
/// Chrome-style: colored header pill with app name + chevron on right,
/// tinted items area with vertical connector line.
@MainActor
class SidebarGroupView: NSView {

    struct Layout {
        static let headerHeight: CGFloat = 26
        static let headerCornerRadius: CGFloat = 6
        static let groupInset: CGFloat = 8
        static let headerHPadding: CGFloat = 8
        static let chevronSize: CGFloat = 10
        static let closeButtonSize: CGFloat = 16
        static let connectorLineWidth: CGFloat = 1.5
        static let itemTopPadding: CGFloat = 4
    }

    let appId: String
    private(set) var colorIndex: Int = 0
    private var palette: SidebarGroupColor.Palette = SidebarGroupColor.palette(for: 0)

    private let itemsBackground = NSView()
    private let headerView = SidebarGroupHeaderView()
    private let appNameLabel = NSTextField(labelWithString: "")
    private let chevronIndicator = NSImageView()
    private let closeButton = NSButton()
    private let itemsContainer = NSView()
    private let connectorLine = NSView()
    private var itemViews: [SidebarItemView] = []

    private var isExpanded = true
    private var itemsHeightConstraint: NSLayoutConstraint?
    private var connectorHeightConstraint: NSLayoutConstraint?
    private var headerTrackingArea: NSTrackingArea?
    private var closeButtonTrackingArea: NSTrackingArea?
    private var isHeaderHovered = false
    private var isCloseHovered = false

    var onPageSelected: ((String, Int) -> Void)?
    var onCloseRequested: ((String) -> Void)?

    init(appId: String) {
        self.appId = appId
        super.init(frame: .zero)
        setupViews()
        refreshFromRust()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// Assign a color index (called by SidebarView based on tab order)
    func setColorIndex(_ index: Int) {
        guard index != colorIndex else { return }
        colorIndex = index
        palette = SidebarGroupColor.palette(for: index)
        applyColors()
    }

    private func applyColors() {
        headerView.layer?.backgroundColor = palette.headerBg.cgColor
        appNameLabel.textColor = palette.headerText
        chevronIndicator.contentTintColor = palette.headerText
        closeButton.contentTintColor = palette.headerText.withAlphaComponent(0.7)
        itemsBackground.layer?.backgroundColor = palette.itemsTint.cgColor
        connectorLine.layer?.backgroundColor = palette.connector.cgColor
    }

    private func setupViews() {
        translatesAutoresizingMaskIntoConstraints = false

        // Items background (tinted, behind items)
        itemsBackground.translatesAutoresizingMaskIntoConstraints = false
        itemsBackground.wantsLayer = true
        itemsBackground.layer?.cornerRadius = Layout.headerCornerRadius
        itemsBackground.layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        addSubview(itemsBackground)

        // Header (colored pill, custom hitTest)
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.wantsLayer = true
        headerView.layer?.cornerRadius = Layout.headerCornerRadius
        headerView.onHeaderClicked = { [weak self] in
            self?.toggleExpanded()
        }
        headerView.onRightClick = { [weak self] event in
            self?.showContextMenu(with: event)
        }
        addSubview(headerView)

        // App name (left-aligned in header)
        appNameLabel.translatesAutoresizingMaskIntoConstraints = false
        appNameLabel.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        appNameLabel.lineBreakMode = .byTruncatingTail
        appNameLabel.maximumNumberOfLines = 1
        headerView.addSubview(appNameLabel)

        // Chevron on right side of header
        chevronIndicator.translatesAutoresizingMaskIntoConstraints = false
        chevronIndicator.image = NSImage(systemSymbolName: "chevron.down", accessibilityDescription: nil)
        chevronIndicator.imageScaling = .scaleProportionallyDown
        headerView.addSubview(chevronIndicator)

        // Close button (hidden by default, shown on hover)
        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")
        closeButton.isBordered = false
        closeButton.bezelStyle = .regularSquare
        closeButton.imagePosition = .imageOnly
        closeButton.wantsLayer = true
        closeButton.layer?.cornerRadius = Layout.closeButtonSize / 2
        closeButton.target = self
        closeButton.action = #selector(closeClicked)
        closeButton.isHidden = true
        headerView.addSubview(closeButton)
        headerView.closeButton = closeButton

        // Vertical connector line
        connectorLine.translatesAutoresizingMaskIntoConstraints = false
        connectorLine.wantsLayer = true
        connectorLine.isHidden = true
        addSubview(connectorLine)

        // Items container (must clip so collapsed items are hidden)
        itemsContainer.translatesAutoresizingMaskIntoConstraints = false
        itemsContainer.wantsLayer = true
        itemsContainer.layer?.masksToBounds = true
        addSubview(itemsContainer)

        let itemsHeight = itemsContainer.heightAnchor.constraint(equalToConstant: 0)
        itemsHeightConstraint = itemsHeight

        let connectorHeight = connectorLine.heightAnchor.constraint(equalToConstant: 0)
        connectorHeightConstraint = connectorHeight

        NSLayoutConstraint.activate([
            // Header: top, inset left/right
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.groupInset),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.groupInset),
            headerView.heightAnchor.constraint(equalToConstant: Layout.headerHeight),

            // App name: left side of header
            appNameLabel.leadingAnchor.constraint(equalTo: headerView.leadingAnchor, constant: Layout.headerHPadding),
            appNameLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            appNameLabel.trailingAnchor.constraint(lessThanOrEqualTo: closeButton.leadingAnchor, constant: -2),

            // Close button: right of label, left of chevron (only visible on hover)
            closeButton.trailingAnchor.constraint(equalTo: chevronIndicator.leadingAnchor, constant: -2),
            closeButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: Layout.closeButtonSize),
            closeButton.heightAnchor.constraint(equalToConstant: Layout.closeButtonSize),

            // Chevron: right side of header
            chevronIndicator.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -Layout.headerHPadding),
            chevronIndicator.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            chevronIndicator.widthAnchor.constraint(equalToConstant: Layout.chevronSize),
            chevronIndicator.heightAnchor.constraint(equalToConstant: Layout.chevronSize),

            // Items background: below header, same horizontal inset
            itemsBackground.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            itemsBackground.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.groupInset),
            itemsBackground.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.groupInset),
            itemsBackground.bottomAnchor.constraint(equalTo: bottomAnchor),

            // Connector line: left edge of items area, below header
            connectorLine.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.groupInset + 14),
            connectorLine.widthAnchor.constraint(equalToConstant: Layout.connectorLineWidth),
            connectorLine.topAnchor.constraint(equalTo: headerView.bottomAnchor, constant: 2),
            connectorHeight,

            // Items container: below header
            itemsContainer.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            itemsContainer.bottomAnchor.constraint(equalTo: bottomAnchor),
            itemsHeight,
        ])

        applyColors()
    }

    /// Reload data from Rust API
    func refreshFromRust() {
        let info = getLxAppInfo(appId)
        appNameLabel.stringValue = info.app_name.toString().uppercased()

        // Hide close button for home lxapp
        let isHome = LxAppCore.isHomeLxApp(appId)
        closeButton.isHidden = isHome || !isHeaderHovered

        // Get tabbar config
        guard let tabBar = getTabBar(appId) else {
            rebuildItems(items: [])
            return
        }

        let items = tabBar.getItems(appId: appId)
        rebuildItems(items: items)
    }

    private func rebuildItems(items: [TabBarItem]) {
        for view in itemViews { view.removeFromSuperview() }
        itemViews.removeAll()

        var yOffset: CGFloat = Layout.itemTopPadding
        for (index, item) in items.enumerated() {
            let itemView = SidebarItemView(appId: appId, itemIndex: index)
            itemView.translatesAutoresizingMaskIntoConstraints = false
            itemView.configure(item: item)
            itemView.onClick = { [weak self] idx in
                guard let self else { return }
                self.onPageSelected?(self.appId, idx)
            }
            itemsContainer.addSubview(itemView)

            NSLayoutConstraint.activate([
                itemView.topAnchor.constraint(equalTo: itemsContainer.topAnchor, constant: yOffset),
                itemView.leadingAnchor.constraint(equalTo: itemsContainer.leadingAnchor, constant: Layout.groupInset),
                itemView.trailingAnchor.constraint(equalTo: itemsContainer.trailingAnchor, constant: -Layout.groupInset),
            ])

            itemViews.append(itemView)
            yOffset += SidebarItemView.Layout.height + 2
        }

        let totalHeight = yOffset
        connectorLine.isHidden = items.count <= 1
        if isExpanded {
            itemsHeightConstraint?.constant = totalHeight
            connectorHeightConstraint?.constant = max(0, totalHeight - Layout.itemTopPadding - 6)
        }
    }

    /// Set the active highlight on a specific item index
    func setActiveHighlight(pageIndex: Int) {
        for (index, itemView) in itemViews.enumerated() {
            itemView.isSelected = (index == pageIndex)
        }
    }

    /// Clear all selection highlights
    func clearHighlight() {
        for itemView in itemViews {
            itemView.isSelected = false
        }
    }

    private func toggleExpanded() {
        isExpanded.toggle()

        let totalHeight = CGFloat(itemViews.count) * (SidebarItemView.Layout.height + 2) + Layout.itemTopPadding
        let connectorTarget = isExpanded ? max(0, totalHeight - Layout.itemTopPadding - 6) : 0

        // Show container before expand animation; hide after collapse animation
        if isExpanded {
            itemsContainer.isHidden = false
            itemsBackground.isHidden = false
        }

        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.2
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            itemsHeightConstraint?.animator().constant = isExpanded ? totalHeight : 0
            connectorHeightConstraint?.animator().constant = connectorTarget
        }, completionHandler: { [weak self] in
            guard let self, !self.isExpanded else { return }
            self.itemsContainer.isHidden = true
            self.itemsBackground.isHidden = true
        })

        // Chevron: down = expanded, up = collapsed (like Chrome)
        chevronIndicator.image = NSImage(
            systemSymbolName: isExpanded ? "chevron.down" : "chevron.up",
            accessibilityDescription: nil
        )

        // Round only top corners when expanded (bottom blends into items bg), all corners when collapsed
        if isExpanded {
            headerView.layer?.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        } else {
            headerView.layer?.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        }

        connectorLine.isHidden = !isExpanded || itemViews.count <= 1
    }

    @objc private func closeClicked() {
        onCloseRequested?(appId)
    }

    // MARK: - Header hover tracking

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = headerTrackingArea {
            headerView.removeTrackingArea(existing)
        }
        let headerArea = NSTrackingArea(
            rect: headerView.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: ["zone": "header"]
        )
        headerView.addTrackingArea(headerArea)
        headerTrackingArea = headerArea

        if let existing = closeButtonTrackingArea {
            closeButton.removeTrackingArea(existing)
        }
        let closeArea = NSTrackingArea(
            rect: closeButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: ["zone": "close"]
        )
        closeButton.addTrackingArea(closeArea)
        closeButtonTrackingArea = closeArea
    }

    override func mouseEntered(with event: NSEvent) {
        let zone = event.trackingArea?.userInfo?["zone"] as? String
        if zone == "header" {
            isHeaderHovered = true
            let isHome = LxAppCore.isHomeLxApp(appId)
            if !isHome {
                closeButton.isHidden = false
            }
        } else if zone == "close" {
            isCloseHovered = true
            closeButton.layer?.backgroundColor = palette.headerText.withAlphaComponent(0.15).cgColor
        }
    }

    override func mouseExited(with event: NSEvent) {
        let zone = event.trackingArea?.userInfo?["zone"] as? String
        if zone == "header" {
            isHeaderHovered = false
            isCloseHovered = false
            closeButton.isHidden = true
            closeButton.layer?.backgroundColor = nil
        } else if zone == "close" {
            isCloseHovered = false
            closeButton.layer?.backgroundColor = nil
        }
    }

    // MARK: - Context menu

    private func buildContextMenu() -> NSMenu {
        let menu = NSMenu()

        let info = getLxAppInfo(appId)
        let appName = info.app_name.toString()
        let version = info.version.toString()
        let releaseType = info.release_type.toString()

        // App info header (disabled item)
        var headerTitle = "\(appName) v\(version)"
        switch releaseType.lowercased() {
        case "developer": headerTitle += " [DEV]"
        case "preview": headerTitle += " [PRE]"
        default: break
        }
        let headerItem = NSMenuItem(title: headerTitle, action: nil, keyEquivalent: "")
        headerItem.isEnabled = false
        menu.addItem(headerItem)
        menu.addItem(NSMenuItem.separator())

        // Restart
        let restartItem = NSMenuItem(
            title: L10n.string("lx_capsule_restart"),
            action: #selector(contextMenuRestart),
            keyEquivalent: ""
        )
        restartItem.target = self
        menu.addItem(restartItem)

        // Clean Cache & Restart
        let cleanItem = NSMenuItem(
            title: L10n.string("lx_capsule_clean_cache"),
            action: #selector(contextMenuCleanCache),
            keyEquivalent: ""
        )
        cleanItem.target = self
        menu.addItem(cleanItem)

        // Uninstall (only for non-home lxapps)
        if !LxAppCore.isHomeLxApp(appId) {
            menu.addItem(NSMenuItem.separator())
            let uninstallItem = NSMenuItem(
                title: L10n.string("lx_capsule_uninstall"),
                action: #selector(contextMenuUninstall),
                keyEquivalent: ""
            )
            uninstallItem.target = self
            menu.addItem(uninstallItem)
        }

        return menu
    }

    private func showContextMenu(with event: NSEvent) {
        let menu = buildContextMenu()
        NSMenu.popUpContextMenu(menu, with: event, for: headerView)
    }

    @objc private func contextMenuRestart() {
        _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, "restart")
    }

    @objc private func contextMenuCleanCache() {
        _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, "clean_cache_restart")
    }

    @objc private func contextMenuUninstall() {
        _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, "uninstall")
    }
}

#endif
