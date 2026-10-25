#if os(macOS)
import AppKit

/// One tab in an aside slot's header strip.
struct AsideSlotTab: Equatable {
    let id: String
    let title: String
    /// The lxapp's registry icon file; nil falls back like every other lxapp
    /// row (`LxIcon.lxappImage`): the host icon, then the SDK mark.
    var iconPath: String? = nil
    /// A browser tab of the side panel: its favicon is `image`, and dragging
    /// it onto the sidebar moves the page into the main browser.
    var isBrowser: Bool = false
    /// A resolved icon (a favicon) that wins over `iconPath`.
    var image: NSImage? = nil
}

/// Header tab strip for an aside slot: the browser slot's bar metrics, with
/// title tabs (ChromeTabRowView, non-bridging) as chips centred in a
/// bar-height strip over a whole hairline. Always shown while the slot has
/// content, since the strip is the slot's management surface (switch, close,
/// drag out and the per-tab menu all live here). With a single child the
/// strip reads as a plain panel header — icon, name, "···" and ✕ — rather
/// than a lone tab chip; it becomes a tab row when a second child arrives,
/// at the same height, so the region never jumps.
@MainActor
final class AsideSlotTabStripView: NSView {
    /// Bar + the separator hairline under the tabs.
    static let stripHeight: CGFloat = ChromeTabMetrics.barHeight + 1

    var onSelect: ((String) -> Void)?
    var onClose: ((String) -> Void)?
    /// Builds the menu for one tab (its id): shown from the tab's "···"
    /// button and on right-click alike. Nil hides the button and offers no
    /// menu.
    var onMenu: ((String) -> NSMenu?)? {
        didSet { itemViews.forEach { $0.menuEnabled = onMenu != nil } }
    }
    /// Put the whole slot away. Nothing closes — the tabs come back with the
    /// region, so this is a chevron, never an ✕.
    var onCollapse: (() -> Void)?
    /// Toggle the slot covering the main area (spec §4.6 Maximize). The role
    /// never changes, so this sits with collapse as a slot control.
    var onToggleMaximize: (() -> Void)? {
        didSet { maximizeButton.isHidden = onToggleMaximize == nil }
    }

    /// Tabs share the strip in equal widths, Chrome style. As they narrow a
    /// tab drops its "···" (the menu stays on right-click), then its title
    /// and its ✕ unless it is the active one; below `minIconTabWidth` the
    /// tabs that do not fit wait behind an overflow button.
    private enum Layout {
        /// Each chip is inset half a foot from its row edge; neighbouring
        /// rows overlap by this much and leave a small gap between chips.
        static let overlap: CGFloat = ChromeTabMetrics.tabFoot - 3
        static let maxTabWidth: CGFloat = 200
        static let minIconTabWidth: CGFloat = 36
        static let overflowWidth: CGFloat = 34
    }

    private let separator = NSView()
    private let collapseButton = NSButton()
    private let maximizeButton = NSButton()
    private let overflowButton = NSButton()
    private var maximized = false
    private var tabs: [AsideSlotTab] = []
    private var activeId: String?
    private var itemViews: [AsideSlotTabItemView] = []
    /// Tabs laid out behind the overflow button, in strip order.
    private var overflowIds: [String] = []

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true

        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = themeCGColor(LxAppHostTheme.separator)
        addSubview(separator)

        overflowButton.isBordered = false
        overflowButton.imagePosition = .imageTrailing
        overflowButton.image = NSImage(
            systemSymbolName: "chevron.down", accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 9, weight: .semibold))
        overflowButton.font = .systemFont(ofSize: 11, weight: .medium)
        overflowButton.contentTintColor = LxAppHostTheme.mutedForeground
        overflowButton.target = self
        overflowButton.action = #selector(overflowTapped)
        overflowButton.isHidden = true
        addSubview(overflowButton)

        collapseButton.translatesAutoresizingMaskIntoConstraints = false
        collapseButton.isBordered = false
        collapseButton.imagePosition = .imageOnly
        collapseButton.image = LxIcon.image(
            named: "icon_aside_collapse", size: CGSize(width: 15, height: 15))
        collapseButton.contentTintColor = LxAppHostTheme.mutedForeground
        collapseButton.toolTip = "Collapse panel"
        collapseButton.setAccessibilityLabel("Collapse panel")
        collapseButton.target = self
        collapseButton.action = #selector(collapseTapped)
        addSubview(collapseButton)

        maximizeButton.translatesAutoresizingMaskIntoConstraints = false
        maximizeButton.isBordered = false
        maximizeButton.imagePosition = .imageOnly
        maximizeButton.contentTintColor = LxAppHostTheme.mutedForeground
        maximizeButton.target = self
        maximizeButton.action = #selector(maximizeTapped)
        maximizeButton.isHidden = true
        addSubview(maximizeButton)
        applyMaximizeState()

        NSLayoutConstraint.activate([
            separator.leadingAnchor.constraint(equalTo: leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: trailingAnchor),
            separator.bottomAnchor.constraint(equalTo: bottomAnchor),
            separator.heightAnchor.constraint(equalToConstant: 1),
            collapseButton.trailingAnchor.constraint(
                equalTo: trailingAnchor, constant: -ChromeTabMetrics.edge),
            collapseButton.widthAnchor.constraint(equalToConstant: 22),
            collapseButton.heightAnchor.constraint(equalToConstant: 22),
            collapseButton.centerYAnchor.constraint(
                equalTo: separator.topAnchor,
                constant: -ChromeTabMetrics.barHeight / 2),
            maximizeButton.trailingAnchor.constraint(
                equalTo: collapseButton.leadingAnchor, constant: -2),
            maximizeButton.widthAnchor.constraint(equalToConstant: 22),
            maximizeButton.heightAnchor.constraint(equalToConstant: 22),
            maximizeButton.centerYAnchor.constraint(equalTo: collapseButton.centerYAnchor),
        ])
    }

    @objc private func collapseTapped() { onCollapse?() }
    @objc private func maximizeTapped() { onToggleMaximize?() }

    @objc private func overflowTapped() {
        let menu = NSMenu()
        for id in overflowIds {
            guard let tab = tabs.first(where: { $0.id == id }) else { continue }
            let item = NSMenuItem(
                title: tab.title, action: #selector(overflowItemSelected(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = id
            item.image = Self.icon(for: tab)
            menu.addItem(item)
        }
        menu.popUp(
            positioning: nil,
            at: NSPoint(x: 0, y: isFlipped ? overflowButton.bounds.maxY + 4 : -4),
            in: overflowButton)
    }

    @objc private func overflowItemSelected(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        onSelect?(id)
    }

    override func layout() {
        super.layout()
        layoutTabs()
    }

    /// Frames for the tabs between the leading edge and the slot controls:
    /// equal widths (capped), each tab told how much it can show; tabs that
    /// would fall below an icon's width overflow, the active one never.
    private func layoutTabs() {
        guard !itemViews.isEmpty else {
            overflowButton.isHidden = true
            return
        }
        let leading = ChromeTabMetrics.edge
        let controls = maximizeButton.isHidden ? collapseButton.frame : maximizeButton.frame
        let trailing = controls.minX - 6
        let avail = max(0, trailing - leading)
        // n tabs of width w overlap n-1 times.
        func width(for count: Int, in room: CGFloat) -> CGFloat {
            let n = CGFloat(count)
            return min(Layout.maxTabWidth, (room + (n - 1) * Layout.overlap) / n)
        }

        var visible = Array(itemViews.indices)
        if width(for: visible.count, in: avail) < Layout.minIconTabWidth {
            let room = avail - Layout.overflowWidth
            let fits = Int(
                (room - Layout.overlap) / (Layout.minIconTabWidth - Layout.overlap))
            visible = Array(itemViews.indices.prefix(max(1, fits)))
            if let active = tabs.firstIndex(where: { $0.id == activeId }),
               !visible.contains(active)
            {
                visible[visible.count - 1] = active
            }
        }
        let shown = Set(visible)
        overflowIds = itemViews.indices.filter { !shown.contains($0) }.map { tabs[$0].id }

        let room = overflowIds.isEmpty ? avail : avail - Layout.overflowWidth
        let tabWidth = max(Layout.minIconTabWidth, width(for: visible.count, in: room))
        let height = ChromeTabMetrics.tabHeight
        let y = collapseButton.frame.midY - height / 2
        var x = leading
        for (index, item) in itemViews.enumerated() {
            guard shown.contains(index) else {
                item.isHidden = true
                continue
            }
            item.isHidden = false
            item.frame = NSRect(x: x, y: y, width: tabWidth, height: height)
            item.fit(width: tabWidth)
            x += tabWidth - Layout.overlap
        }

        overflowButton.isHidden = overflowIds.isEmpty
        if !overflowIds.isEmpty {
            overflowButton.title = "\(overflowIds.count)"
            overflowButton.toolTip = overflowIds.compactMap { id in
                tabs.first(where: { $0.id == id })?.title
            }.joined(separator: "\n")
            overflowButton.frame = NSRect(
                x: x + Layout.overlap + 2, y: collapseButton.frame.midY - 11,
                width: Layout.overflowWidth - 4, height: 22)
        }
    }

    /// An lxapp slot tab's id is its app id; a browser tab brings its
    /// favicon (or the globe) already resolved.
    private static func icon(for tab: AsideSlotTab) -> NSImage? {
        let size = CGSize(width: ChromeTabMetrics.iconSize, height: ChromeTabMetrics.iconSize)
        if let image = tab.image {
            return image
        }
        if tab.isBrowser {
            return LxIcon.image(named: "icon_globe", size: size)
        }
        return LxIcon.lxappImage(appId: tab.id, path: tab.iconPath ?? "")
            .map { TabBarHelper.appTileIcon($0, size: ChromeTabMetrics.iconSize) }
    }

    func setMaximized(_ maximized: Bool) {
        guard self.maximized != maximized else { return }
        self.maximized = maximized
        applyMaximizeState()
    }

    private func applyMaximizeState() {
        let symbol = maximized
            ? "arrow.down.right.and.arrow.up.left"
            : "arrow.up.left.and.arrow.down.right"
        let label = maximized ? "Restore panel" : "Maximize panel"
        maximizeButton.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)?
            .withSymbolConfiguration(.init(pointSize: 11, weight: .medium))
        maximizeButton.toolTip = label
        maximizeButton.setAccessibilityLabel(label)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        separator.layer?.backgroundColor = themeCGColor(LxAppHostTheme.separator)
        applySelection()
    }

    func update(tabs: [AsideSlotTab], activeId: String?) {
        guard self.tabs != tabs || self.activeId != activeId else { return }
        self.tabs = tabs
        self.activeId = activeId
        itemViews.forEach { $0.removeFromSuperview() }
        itemViews.removeAll()
        for tab in tabs {
            let item = AsideSlotTabItemView(tab: tab, icon: Self.icon(for: tab))
            item.onSelect = { [weak self] in self?.onSelect?(tab.id) }
            item.onClose = { [weak self] in self?.onClose?(tab.id) }
            item.menuProvider = { [weak self] in self?.onMenu?(tab.id) }
            item.menuEnabled = onMenu != nil
            item.isHeader = tabs.count == 1
            addSubview(item)
            itemViews.append(item)
        }
        applySelection()
        needsLayout = true
    }

    /// Selection pass: active fill, tint steps for title/icon/close, and
    /// divider suppression beside the selection.
    private func applySelection() {
        for (index, item) in itemViews.enumerated() {
            let selected = tabs[index].id == activeId
            item.row.isActiveTab = selected
            item.row.layer?.zPosition = selected ? 2 : 0
            let nextSelected = index + 1 < tabs.count && tabs[index + 1].id == activeId
            item.row.suppressTrailingSeparator = selected || nextSelected
            item.applyTint(selected: selected)
        }
    }
}

/// A single slot tab assembled exactly like the browser's tab chip:
/// icon + title button (activates) + "···" (menu) + close button inside a
/// ChromeTabRowView. The strip sets its frame and `fit(width:)` decides what
/// it shows: a wide tab keeps the "···" room always (shown on hover, or
/// always on the single-child header) so revealing it never shifts the
/// title; a narrower one gives that room to the title and shows its ✕ only
/// while active or hovered; an icon-wide one shows the icon alone (✕ on the
/// active tab when it still fits). Right-click on the row opens the menu at
/// every width.
@MainActor
private final class AsideSlotTabItemView: NSView {
    private enum Fit {
        case full, compact, icon
    }

    var onSelect: (() -> Void)?
    var onClose: (() -> Void)?
    var menuProvider: (() -> NSMenu?)?
    var menuEnabled = false {
        didSet { applyControls() }
    }
    var isHeader = false {
        didSet {
            row.isHeader = isHeader
            applyControls()
        }
    }

    let row = ChromeTabRowView()
    private let iconView = NSImageView()
    private let titleButton = AsideTabTitleButton()
    private let moreButton = NSButton()
    private let closeButton = NSButton()
    private let title: String
    private var hovered = false
    private var selected = false
    private var fit = Fit.full
    private var width: CGFloat = 0

    init(tab: AsideSlotTab, icon: NSImage?) {
        title = tab.title
        super.init(frame: .zero)

        // Dragging a tab onto the sidebar promotes it to a main: the lxapp
        // moves, or the page joins the main browser.
        titleButton.dragPayload = AsideTabDragPayload(
            kind: tab.isBrowser ? .browser : .lxapp, surfaceId: tab.id)
        titleButton.dragImageSource = row
        titleButton.translatesAutoresizingMaskIntoConstraints = false
        titleButton.isBordered = false
        titleButton.bezelStyle = .regularSquare
        titleButton.font = .systemFont(ofSize: 12, weight: .medium)
        titleButton.alignment = .left
        titleButton.lineBreakMode = .byTruncatingTail
        titleButton.title = tab.title
        titleButton.target = self
        titleButton.action = #selector(selectTapped)

        iconView.translatesAutoresizingMaskIntoConstraints = false
        if let icon {
            icon.size = NSSize(width: ChromeTabMetrics.iconSize, height: ChromeTabMetrics.iconSize)
            iconView.image = icon
        }
        iconView.imageScaling = .scaleProportionallyDown
        if tab.isBrowser && tab.image == nil {
            iconView.contentTintColor = LxAppHostTheme.mutedForeground
        }

        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.isBordered = false
        closeButton.imagePosition = .imageOnly
        closeButton.image = LxIcon.image(
            named: "icon_close_x", size: CGSize(width: 12, height: 12))
        closeButton.contentTintColor = LxAppHostTheme.mutedForeground
        closeButton.target = self
        closeButton.action = #selector(closeTapped)
        closeButton.setAccessibilityLabel("Close \(tab.title)")

        moreButton.translatesAutoresizingMaskIntoConstraints = false
        moreButton.isBordered = false
        moreButton.imagePosition = .imageOnly
        moreButton.image = LxIcon.image(
            named: "icon_more_vertical", size: CGSize(width: 12, height: 12))
        moreButton.contentTintColor = LxAppHostTheme.mutedForeground
        moreButton.target = self
        moreButton.action = #selector(moreTapped)
        moreButton.toolTip = L10n.string("lx_aside_tab_menu")
        moreButton.setAccessibilityLabel(L10n.string("lx_aside_tab_menu"))
        row.onHoverChanged = { [weak self] hovered in
            self?.hovered = hovered
            self?.applyControls()
        }
        // Right-click anywhere on the row: the same menu as the button.
        let rowMenu = NSMenu()
        rowMenu.delegate = self
        row.menu = rowMenu

        // The slot below is an lxapp: its page (or its declared navigation
        // bar colour) meets the strip, so the active tab must not pretend to
        // merge into it — the hairline stays whole under the selection.
        row.bridgesIntoContent = false
        row.orientation = .horizontal
        row.spacing = 5
        row.alignment = .centerY
        // The chip is centred in the row and inset half a foot each side.
        row.edgeInsets = NSEdgeInsets(
            top: 4,
            left: ChromeTabMetrics.tabFoot * 0.5 + 8,
            bottom: 4,
            right: ChromeTabMetrics.tabFoot * 0.5 + 4
        )
        row.addArrangedSubview(iconView)
        row.addArrangedSubview(titleButton)
        row.addArrangedSubview(moreButton)
        row.addArrangedSubview(closeButton)
        row.translatesAutoresizingMaskIntoConstraints = false
        applyControls()

        addSubview(row)
        titleButton.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        titleButton.setContentHuggingPriority(.defaultLow, for: .horizontal)
        NSLayoutConstraint.activate([
            iconView.widthAnchor.constraint(equalToConstant: ChromeTabMetrics.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: ChromeTabMetrics.iconSize),
            closeButton.widthAnchor.constraint(equalToConstant: 16),
            moreButton.widthAnchor.constraint(equalToConstant: 16),
            row.heightAnchor.constraint(equalToConstant: ChromeTabMetrics.tabHeight),
            row.topAnchor.constraint(equalTo: topAnchor),
            row.leadingAnchor.constraint(equalTo: leadingAnchor),
            row.trailingAnchor.constraint(equalTo: trailingAnchor),
            row.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func applyTint(selected: Bool) {
        self.selected = selected
        applyControls()
        titleButton.contentTintColor = selected
            ? LxAppHostTheme.foreground
            : LxAppHostTheme.mutedForeground
        closeButton.contentTintColor = selected
            ? LxAppHostTheme.mutedForeground
            : LxAppHostTheme.mutedForeground
        closeButton.alphaValue = selected ? 1 : 0.65
    }

    /// Choose what the tab shows at `width`, the frame the strip gave it.
    func fit(width: CGFloat) {
        guard width != self.width else { return }
        self.width = width
        fit = width >= 110 ? .full : width >= 64 ? .compact : .icon
        applyControls()
    }

    /// Hidden controls give their room back to the title, except the wide
    /// tab's "···", whose room is reserved whenever a menu exists so the
    /// glyph can show on hover (or always in the header form, where it is a
    /// panel control) without shifting the title.
    private func applyControls() {
        let leadingInset = ChromeTabMetrics.tabFoot * 0.5 + 8
        let trailingInset = ChromeTabMetrics.tabFoot * 0.5 + 4
        // The icon-only form still keeps the title button (empty) as the
        // tab's click and drag target.
        titleButton.title = fit == .icon ? "" : title
        moreButton.isHidden = fit != .full || !menuEnabled
        moreButton.alphaValue = (isHeader || hovered) ? 1 : 0
        switch fit {
        case .full:
            closeButton.isHidden = false
        case .compact:
            closeButton.isHidden = !(selected || hovered || isHeader)
        case .icon:
            // Icon + ✕ needs the insets, both glyphs and one gap.
            let room = leadingInset + trailingInset + ChromeTabMetrics.iconSize + 16 + row.spacing
            closeButton.isHidden = !(selected || isHeader) || width < room
        }
        // Centred, short of what the empty title button and trailing inset
        // still need.
        let iconInset = closeButton.isHidden
            ? max(0, min(
                (width - ChromeTabMetrics.iconSize) / 2,
                width - ChromeTabMetrics.iconSize - row.spacing - trailingInset))
            : leadingInset
        row.edgeInsets.left = fit == .icon ? iconInset : leadingInset
    }

    @objc private func selectTapped() { onSelect?() }
    @objc private func closeTapped() { onClose?() }

    @objc private func moreTapped() {
        guard let menu = menuProvider?() else { return }
        let origin = NSPoint(x: 0, y: moreButton.bounds.maxY + 4)
        menu.popUp(positioning: nil, at: origin, in: moreButton)
    }
}

extension AsideSlotTabItemView: NSMenuDelegate {
    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        guard let built = menuProvider?() else { return }
        for item in built.items {
            built.removeItem(item)
            menu.addItem(item)
        }
    }
}
#endif
