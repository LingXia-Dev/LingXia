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
    /// The collapse/expand chevron toggle. Like `closeButton`, it must receive
    /// its own clicks — otherwise the header swallows them and the chevron can
    /// never toggle the group.
    var chevronButton: NSButton?
    var onHeaderClicked: (() -> Void)?
    var onRightClick: ((NSEvent) -> Void)?

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, frame.contains(point) else { return nil }
        let localPoint = convert(point, from: superview)
        if let close = closeButton, !close.isHidden {
            let closePoint = convert(localPoint, to: close)
            if close.bounds.contains(closePoint) {
                return close
            }
        }
        if let chevron = chevronButton, !chevron.isHidden {
            let chevronPoint = convert(localPoint, to: chevron)
            if chevron.bounds.contains(chevronPoint) {
                return chevron
            }
        }
        return self
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override var mouseDownCanMoveWindow: Bool { false }

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
        static let headerHeight: CGFloat = 36
        static let headerCornerRadius: CGFloat = 6
        static let groupInset: CGFloat = 8
        static let headerHPadding: CGFloat = 8
        static let chevronSize: CGFloat = 10
        static let closeButtonSize: CGFloat = 16
        static let itemTopPadding: CGFloat = 2
    }

    let appId: String
    private(set) var colorIndex: Int = 0
    private var palette: SidebarGroupColor.Palette = SidebarGroupColor.palette(for: 0)

    private let itemsBackground = NSView()
    private let headerView = SidebarGroupHeaderView()
    private let appIconView = NSImageView()
    private let appNameLabel = NSTextField(labelWithString: "")

    /// The bundled default LingXia mark, used when an lxapp declares no icon.
    private static let defaultAppIcon: NSImage? = {
        guard let url = Bundle.lingxiaResources.url(
            forResource: "lxapp_default", withExtension: "png", subdirectory: "icons")
        else { return nil }
        return NSImage(contentsOf: url)
    }()
    private let chevronIndicator = NSButton()
    /// Collapsed-state aggregate: a dot on the header while any tabbar item
    /// carries a badge or red dot (notifications never vanish
    /// with the collapse).
    private let aggregateDot = NSView()
    private let closeButton = NSButton()
    private let itemsContainer = NSView()
    private var itemViews: [SidebarItemView] = []

    private var isExpanded = true
    /// Last tabbar visibility applied from Rust. Collapse/expand only follows a
    /// visibility CHANGE so unrelated refreshes (badge, style) don't undo a
    /// manual chevron toggle.
    private var lastAppliedTabBarVisible: Bool?
    /// One-shot guard for applying the persisted user collapse state.
    private var didRestoreCollapsedState = false
    /// True when this group's lxapp is the active main — set by SidebarView.
    /// Clicking the active group's header toggles collapse; a non-active group's
    /// header switches to it instead.
    var isActiveGroup = false { didSet { updateActiveAppearance() } }
    /// Fired after a pin/unpin from the context menu so the sidebar
    /// re-renders its pin grid (the store itself lives in Rust).
    var onPinChanged: (() -> Void)?
    /// Selected-state tint from this lxapp's tabbar style (`selectedColor`);
    /// nil = system neutral. Feeds the items' selected state.
    private var tabBarTint: NSColor?
    /// Attribution-line base from the tabbar's borderStyle: "white" reads as
    /// a light hairline, "black" (default) as the darker separator.
    private var attributionBaseColor: NSColor = .separatorColor
    /// Unselected item title tint from the tabbar's `color`; nil = neutral.
    private var itemTint: NSColor?
    /// Expanded items-area wash from the tabbar's `backgroundColor`;
    /// nil = transparent (the sidebar base shows through).
    private var itemsAreaColor: NSColor?
    /// Thin vertical line binding the expanded items to their group header
    /// (Windows-baseline attribution line, tabbar-tinted).
    private let attributionLine = NSView()
    private var attributionHeightConstraint: NSLayoutConstraint?
    private var itemsHeightConstraint: NSLayoutConstraint?
    private var headerTrackingArea: NSTrackingArea?
    private var closeButtonTrackingArea: NSTrackingArea?
    private var isHeaderHovered = false
    private var isCloseHovered = false

    var onPageSelected: ((String, Int) -> Void)?
    /// Fired when the group header (the lxapp's name) is clicked — switches the
    /// main to this lxapp, so an lxapp with no tabBar items is still switchable.
    var onAppSelected: ((String) -> Void)?
    var onCloseRequested: ((String) -> Void)?
    var onLayoutChanged: (() -> Void)?

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

    /// Identity cues only — app icon tile, name, chevron. No accent pill,
    /// no tinted items area, no connector line: decoration would break the
    /// sidebar's uniform rhythm.
    /// The lxapp tab (group header) highlights while its app owns the main —
    /// regardless of page or tabbar item — so a collapsed group still shows
    /// where you are. Distinct from the item accent (two independent levels).
    private func updateActiveAppearance() {
        headerView.layer?.backgroundColor = isActiveGroup
            ? NSColor.labelColor.withAlphaComponent(0.09).cgColor
            : NSColor.clear.cgColor
    }

    private func applyColors() {
        updateActiveAppearance()
        attributionLine.layer?.backgroundColor = attributionBaseColor
            .withAlphaComponent(0.5).cgColor
        appNameLabel.textColor = .labelColor
        chevronIndicator.contentTintColor = .secondaryLabelColor
        closeButton.contentTintColor = NSColor.secondaryLabelColor.withAlphaComponent(0.9)
        // tabbar backgroundColor maps to the expanded items area (the group's
        // own strip surface); unset stays transparent on the sidebar base.
        itemsBackground.layer?.backgroundColor = (itemsAreaColor ?? NSColor.clear).cgColor
    }

    private func setupViews() {
        translatesAutoresizingMaskIntoConstraints = false

        // Items background (tinted, behind items)
        itemsBackground.translatesAutoresizingMaskIntoConstraints = false
        itemsBackground.wantsLayer = true
        itemsBackground.layer?.cornerRadius = Layout.headerCornerRadius
        // Bottom corners (non-flipped: minY = bottom) — the expanded group
        // reads as ONE card: header rounds the top, this rounds the bottom,
        // and the seam between them stays square.
        itemsBackground.layer?.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        addSubview(itemsBackground)

        // Header (colored pill, custom hitTest)
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.wantsLayer = true
        headerView.layer?.cornerRadius = Layout.headerCornerRadius
        headerView.onHeaderClicked = { [weak self] in
            guard let self else { return }
            // The ACTIVE app's header toggles its page list (collapse/expand) —
            // so clicking the group you're on retracts its items, not only the
            // small chevron. A DIFFERENT app's header switches to it and ensures
            // its list is shown (switching never hides another app's items).
            if self.isActiveGroup && !self.itemViews.isEmpty {
                self.toggleExpanded()
            } else {
                self.onAppSelected?(self.appId)
                if !self.isExpanded { self.toggleExpanded() }
            }
        }
        headerView.onRightClick = { [weak self] event in
            self?.showContextMenu(with: event)
        }
        addSubview(headerView)

        // App icon (left of the name)
        appIconView.translatesAutoresizingMaskIntoConstraints = false
        appIconView.imageScaling = .scaleProportionallyUpOrDown
        appIconView.wantsLayer = true
        appIconView.layer?.cornerRadius = 3
        appIconView.layer?.masksToBounds = true
        headerView.addSubview(appIconView)

        // App name (left-aligned in header)
        appNameLabel.translatesAutoresizingMaskIntoConstraints = false
        appNameLabel.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        appNameLabel.lineBreakMode = .byTruncatingTail
        appNameLabel.maximumNumberOfLines = 1
        headerView.addSubview(appNameLabel)

        // Chevron on right side of header — a real collapse/expand toggle
        // (clicking the app name switches + expands; the chevron collapses too).
        // Hidden when the lxapp has no tabBar items (nothing to collapse).
        chevronIndicator.translatesAutoresizingMaskIntoConstraints = false
        chevronIndicator.image = NSImage(systemSymbolName: "chevron.down", accessibilityDescription: "Collapse")
        chevronIndicator.imageScaling = .scaleProportionallyDown
        chevronIndicator.isBordered = false
        chevronIndicator.bezelStyle = .regularSquare
        chevronIndicator.imagePosition = .imageOnly
        chevronIndicator.target = self
        chevronIndicator.action = #selector(chevronClicked)
        chevronIndicator.isHidden = true
        headerView.addSubview(chevronIndicator)
        headerView.chevronButton = chevronIndicator

        aggregateDot.translatesAutoresizingMaskIntoConstraints = false
        aggregateDot.wantsLayer = true
        aggregateDot.layer?.cornerRadius = 3
        aggregateDot.layer?.backgroundColor = NSColor.systemRed.cgColor
        aggregateDot.isHidden = true
        headerView.addSubview(aggregateDot)
        NSLayoutConstraint.activate([
            aggregateDot.trailingAnchor.constraint(equalTo: chevronIndicator.leadingAnchor, constant: -6),
            aggregateDot.topAnchor.constraint(equalTo: headerView.topAnchor, constant: 9),
            aggregateDot.widthAnchor.constraint(equalToConstant: 6),
            aggregateDot.heightAnchor.constraint(equalToConstant: 6),
        ])

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

        // Attribution line binding items to their header
        attributionLine.translatesAutoresizingMaskIntoConstraints = false
        attributionLine.wantsLayer = true
        attributionLine.isHidden = true
        addSubview(attributionLine)

        // Items container (must clip so collapsed items are hidden)
        itemsContainer.translatesAutoresizingMaskIntoConstraints = false
        itemsContainer.wantsLayer = true
        itemsContainer.layer?.masksToBounds = true
        addSubview(itemsContainer)

        let itemsHeight = itemsContainer.heightAnchor.constraint(equalToConstant: 0)
        itemsHeightConstraint = itemsHeight
        let attributionHeight = attributionLine.heightAnchor.constraint(equalToConstant: 0)
        attributionHeightConstraint = attributionHeight


        NSLayoutConstraint.activate([
            // Header: top, inset left/right
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.groupInset),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.groupInset),
            headerView.heightAnchor.constraint(equalToConstant: Layout.headerHeight),

            // App icon: leading edge of the header
            appIconView.leadingAnchor.constraint(equalTo: headerView.leadingAnchor, constant: Layout.headerHPadding),
            appIconView.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            appIconView.widthAnchor.constraint(equalToConstant: 16),
            appIconView.heightAnchor.constraint(equalToConstant: 16),

            // App name: right after the icon
            appNameLabel.leadingAnchor.constraint(equalTo: appIconView.trailingAnchor, constant: 6),
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

            // Attribution line: left edge of the items area
            attributionLine.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.groupInset + 12),
            attributionLine.widthAnchor.constraint(equalToConstant: 1),
            attributionLine.topAnchor.constraint(equalTo: headerView.bottomAnchor, constant: Layout.itemTopPadding),
            attributionHeight,

            // Items container: below header
            itemsContainer.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            itemsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            itemsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            itemsContainer.bottomAnchor.constraint(equalTo: bottomAnchor),
            itemsHeight,
        ])

        applyColors()
    }

    /// Show the lxapp's icon (`path` is an absolute file path from the lxapp
    /// bundle), falling back to the bundled default LingXia mark when the
    /// lxapp declares none or the file can't be read.
    private func loadAppIcon(path: String) {
        if !path.isEmpty, let image = NSImage(contentsOfFile: path) {
            appIconView.image = image
        } else {
            appIconView.image = Self.defaultAppIcon
        }
    }

    /// Reload data from Rust API
    func refreshFromRust() {
        let info = getLxAppInfo(appId)
        appNameLabel.stringValue = info.app_name.toString().uppercased()
        loadAppIcon(path: info.icon.toString())

        // Hide close button for home lxapp
        let isHome = LxAppCore.isHomeLxApp(appId)
        closeButton.isHidden = isHome || !isHeaderHovered

        // Get tabbar config
        guard let tabBar = getTabBar(appId) else {
            rebuildItems(items: [])
            return
        }

        // Style follows the lxapp's tabbar config — but only fields the app
        // DECLARED (styled_mask). The color values always carry effective
        // mobile defaults, and those are designed for a light bar: inheriting
        // them here would paint every unstyled app light-on-dark.
        let mask = tabBar.styled_mask
        itemTint = mask & 0b0001 != 0 ? PlatformColor(argb: tabBar.color) : nil
        tabBarTint = mask & 0b0010 != 0 ? PlatformColor(argb: tabBar.selected_color) : nil
        itemsAreaColor = mask & 0b0100 != 0 ? PlatformColor(argb: tabBar.background_color) : nil
        attributionBaseColor = mask & 0b1000 != 0
            ? PlatformColor(argb: tabBar.border_style)
            : NSColor.separatorColor
        applyColors()

        let items = tabBar.getItems(appId: appId)
        rebuildItems(items: items)

        // Only an EXPLICIT lx.hideTabBar/showTabBar collapses/expands this
        // group. `is_visible` also flips on every navigation to a non-tab page
        // (mobile auto-hide) — on desktop the sidebar stays put for that; the
        // item selection clearing already covers it. Not persisted — the app
        // re-establishes API state on launch.
        let apiVisible = !tabBar.is_api_hidden
        if lastAppliedTabBarVisible != apiVisible {
            lastAppliedTabBarVisible = apiVisible
            if isExpanded != apiVisible && !itemViews.isEmpty {
                toggleExpanded(persist: false)
            }
        }

        // One-time restore of the user's saved collapse state (after the API
        // sync above, so a launch-time hideTabBar wins over the stored value).
        if !didRestoreCollapsedState, !itemViews.isEmpty {
            didRestoreCollapsedState = true
            if let collapsed = LxAppShellPersistence.groupCollapsed(appId: appId),
               collapsed == isExpanded {
                toggleExpanded(persist: false)
            }
        }
    }

    private func rebuildItems(items: [TabBarItem]) {
        for view in itemViews { view.removeFromSuperview() }
        itemViews.removeAll()

        var yOffset: CGFloat = Layout.itemTopPadding
        for (index, item) in items.enumerated() {
            let itemView = SidebarItemView(appId: appId, itemIndex: index)
            itemView.translatesAutoresizingMaskIntoConstraints = false
            itemView.selectedTint = tabBarTint
            itemView.unselectedTint = itemTint
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
            yOffset += SidebarItemView.Layout.height
            if index + 1 < items.count {
                yOffset += 1
            }
        }

        let totalHeight = yOffset
        // The chevron is a collapse/expand affordance — only meaningful when the
        // group actually has items. No tabBar items → no chevron.
        chevronIndicator.isHidden = items.isEmpty
        attributionLine.isHidden = items.isEmpty
        let hasNotifications = items.contains { !$0.badge.toString().isEmpty || $0.has_red_dot }
        aggregateDot.isHidden = isExpanded || !hasNotifications
        if isExpanded {
            itemsHeightConstraint?.constant = totalHeight
            attributionHeightConstraint?.constant = max(0, totalHeight - Layout.itemTopPadding * 2)
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

    /// `persist: true` only for user-driven toggles (chevron/header click);
    /// API sync and state restore pass false so only user intent is stored.
    private func toggleExpanded(persist: Bool = true) {
        isExpanded.toggle()
        if persist {
            LxAppShellPersistence.setGroupCollapsed(!isExpanded, appId: appId)
        }
        // Re-evaluate the collapsed aggregate against the current items.
        let hasNotifications = itemViews.contains { $0.hasNotification }
        aggregateDot.isHidden = isExpanded || !hasNotifications

        let itemGaps = CGFloat(max(0, itemViews.count - 1))
        let totalHeight = Layout.itemTopPadding
            + CGFloat(itemViews.count) * SidebarItemView.Layout.height
            + itemGaps

        // Show container before expand animation; hide after collapse animation
        if isExpanded {
            itemsContainer.isHidden = false
            itemsBackground.isHidden = false
        }

        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.2
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            itemsHeightConstraint?.animator().constant = isExpanded ? totalHeight : 0
            attributionHeightConstraint?.animator().constant =
                isExpanded ? max(0, totalHeight - Layout.itemTopPadding * 2) : 0
        }, completionHandler: { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if !self.isExpanded {
                    self.itemsContainer.isHidden = true
                    self.itemsBackground.isHidden = true
                }
                self.onLayoutChanged?()
            }
        })

        // Chevron: down = expanded, up = collapsed (like Chrome)
        chevronIndicator.image = NSImage(
            systemSymbolName: isExpanded ? "chevron.down" : "chevron.up",
            accessibilityDescription: nil
        )

        // Round only top corners when expanded (bottom blends into items bg), all corners when collapsed
        if isExpanded {
            headerView.layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        } else {
            headerView.layer?.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        }

    }

    @objc private func chevronClicked() {
        toggleExpanded()
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

        // Pin to the sidebar grid (Rust-owned user list, mirrors web pins).
        let pinned = shellPinnedLxapps().toString().contains("\"\(appId)\"")
        let pinItem = NSMenuItem(
            title: L10n.string(pinned ? "lx_browser_unpin" : "lx_browser_pin_to_sidebar"),
            action: #selector(contextMenuTogglePin),
            keyEquivalent: ""
        )
        pinItem.target = self
        menu.addItem(pinItem)

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

    @objc private func contextMenuTogglePin() {
        let pinned = shellPinnedLxapps().toString().contains("\"\(appId)\"")
        _ = shellSetLxappPinned(appId, !pinned)
        onPinChanged?()
    }

    @objc private func contextMenuRestart() {
        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, "restart")
    }

    @objc private func contextMenuCleanCache() {
        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, "clean_cache_restart")
    }

    @objc private func contextMenuUninstall() {
        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, "uninstall")
    }
}

#endif
