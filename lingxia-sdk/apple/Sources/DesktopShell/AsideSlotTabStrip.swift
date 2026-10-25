#if os(macOS)
import AppKit

/// One tab in an aside slot's header strip.
struct AsideSlotTab: Equatable {
    let id: String
    let title: String
    /// The lxapp's registry icon file; nil falls back like every other lxapp
    /// row (`LxIcon.lxappImage`): the host icon, then the SDK mark.
    var iconPath: String? = nil
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

    private let stack = NSStackView()
    private let separator = NSView()
    private let collapseButton = NSButton()
    private let maximizeButton = NSButton()
    private var maximized = false
    private var tabs: [AsideSlotTab] = []
    private var activeId: String?
    private var itemViews: [AsideSlotTabItemView] = []

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true

        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = themeCGColor(LxAppHostTheme.separator)
        addSubview(separator)

        stack.orientation = .horizontal
        // Each chip is inset half a foot from its row edge; this leaves a
        // small gap between neighbouring chips.
        stack.spacing = -ChromeTabMetrics.tabFoot + 3
        stack.alignment = .centerY
        stack.distribution = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

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
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: ChromeTabMetrics.edge),
            stack.trailingAnchor.constraint(
                lessThanOrEqualTo: maximizeButton.leadingAnchor, constant: -6),
            stack.centerYAnchor.constraint(equalTo: collapseButton.centerYAnchor),
        ])
    }

    @objc private func collapseTapped() { onCollapse?() }
    @objc private func maximizeTapped() { onToggleMaximize?() }

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
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        itemViews.removeAll()
        for tab in tabs {
            // An lxapp slot tab's id is its app id.
            let icon = LxIcon.lxappImage(appId: tab.id, path: tab.iconPath ?? "")
            let tiled = icon.map { TabBarHelper.appTileIcon($0, size: ChromeTabMetrics.iconSize) }
            let item = AsideSlotTabItemView(tab: tab, icon: tiled)
            item.onSelect = { [weak self] in self?.onSelect?(tab.id) }
            item.onClose = { [weak self] in self?.onClose?(tab.id) }
            item.menuProvider = { [weak self] in self?.onMenu?(tab.id) }
            item.menuEnabled = onMenu != nil
            item.isHeader = tabs.count == 1
            stack.addArrangedSubview(item)
            itemViews.append(item)
        }
        applySelection()
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
/// ChromeTabRowView. The "···" keeps its room always and shows on hover (a
/// chip) or always (the single-child header), so revealing it never shifts
/// the title; right-click on the row opens the same menu.
@MainActor
private final class AsideSlotTabItemView: NSView {
    var onSelect: (() -> Void)?
    var onClose: (() -> Void)?
    var menuProvider: (() -> NSMenu?)?
    var menuEnabled = false {
        didSet { applyMoreVisibility() }
    }
    var isHeader = false {
        didSet {
            row.isHeader = isHeader
            applyMoreVisibility()
        }
    }

    let row = ChromeTabRowView()
    private let iconView = NSImageView()
    private let titleButton = AsideTabTitleButton()
    private let moreButton = NSButton()
    private let closeButton = NSButton()
    private var hovered = false

    init(tab: AsideSlotTab, icon: NSImage?) {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false

        // Dragging a tab onto the sidebar promotes the lxapp to a main.
        titleButton.dragPayload = AsideTabDragPayload(kind: .lxapp, surfaceId: tab.id)
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
            self?.applyMoreVisibility()
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
        applyMoreVisibility()

        addSubview(row)
        titleButton.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        titleButton.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let minWidth = row.widthAnchor.constraint(
            greaterThanOrEqualToConstant: ChromeTabMetrics.minTabWidth)
        minWidth.priority = .defaultLow
        NSLayoutConstraint.activate([
            iconView.widthAnchor.constraint(equalToConstant: ChromeTabMetrics.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: ChromeTabMetrics.iconSize),
            closeButton.widthAnchor.constraint(equalToConstant: 16),
            moreButton.widthAnchor.constraint(equalToConstant: 16),
            row.heightAnchor.constraint(equalToConstant: ChromeTabMetrics.tabHeight),
            minWidth,
            row.widthAnchor.constraint(lessThanOrEqualToConstant: ChromeTabMetrics.maxTabWidth),
            row.topAnchor.constraint(equalTo: topAnchor),
            row.leadingAnchor.constraint(equalTo: leadingAnchor),
            row.trailingAnchor.constraint(equalTo: trailingAnchor),
            row.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func applyTint(selected: Bool) {
        titleButton.contentTintColor = selected
            ? LxAppHostTheme.foreground
            : LxAppHostTheme.mutedForeground
        closeButton.contentTintColor = selected
            ? LxAppHostTheme.mutedForeground
            : LxAppHostTheme.mutedForeground
        closeButton.alphaValue = selected ? 1 : 0.65
    }

    /// Room is reserved whenever a menu exists; the glyph itself shows on
    /// hover, or always in the header form, where it is a panel control.
    private func applyMoreVisibility() {
        moreButton.isHidden = !menuEnabled
        moreButton.alphaValue = (isHeader || hovered) ? 1 : 0
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
