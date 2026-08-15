#if os(macOS)
import AppKit
import CLingXiaRustAPI
import os.log

// MARK: - Resize Handle

/// Draggable handle on the right edge of the sidebar for resizing.
@MainActor
private class SidebarResizeHandle: NSView {
    var onDrag: ((CGFloat) -> Void)?
    var onDragEnd: ((CGFloat) -> Void)?
    private var initialMouseX: CGFloat = 0
    private var initialWidth: CGFloat = 0

    // Prevent window drag — this view handles its own mouse events
    override var mouseDownCanMoveWindow: Bool { false }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .resizeLeftRight)
    }

    // Always return self so the separator subview doesn't steal events
    override func hitTest(_ point: NSPoint) -> NSView? {
        // AppKit passes this point in the superview's coordinate space, so the
        // handle must test its frame rather than its local bounds.
        guard !isHidden, frame.contains(point) else { return nil }
        return self
    }

    override func mouseDown(with event: NSEvent) {
        initialMouseX = event.locationInWindow.x
        initialWidth = superview?.frame.width ?? 0
    }

    override func mouseDragged(with event: NSEvent) {
        let deltaX = event.locationInWindow.x - initialMouseX
        let newWidth = initialWidth + deltaX
        onDrag?(newWidth)
    }

    override func mouseUp(with event: NSEvent) {
        let deltaX = event.locationInWindow.x - initialMouseX
        let newWidth = initialWidth + deltaX
        onDragEnd?(newWidth)
    }
}

@MainActor
private final class SidebarClipView: NSClipView {
    override var mouseDownCanMoveWindow: Bool { false }
}

@MainActor
private final class SidebarScrollView: NSScrollView {
    override var mouseDownCanMoveWindow: Bool { false }
}

@MainActor
private final class SidebarRailButton: NSButton {
    var onHoverChanged: ((Bool) -> Void)?
    var onContextMenuRequested: ((NSEvent, SidebarRailButton) -> Void)?
    var onCloseRequested: (() -> Void)?
    var closable = false {
        didSet { needsDisplay = true }
    }
    private var trackingArea: NSTrackingArea?
    private var hovered = false

    override var mouseDownCanMoveWindow: Bool { false }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
        // The rail rebuilds on every selection change, so the button under a
        // stationary cursor is replaced without ever getting `mouseEntered`.
        // Seed the state instead of waiting for the pointer to move.
        if let window, window.isKeyWindow {
            let inside = bounds.contains(convert(window.mouseLocationOutsideOfEventStream, from: nil))
            if inside != hovered {
                hovered = inside
                needsDisplay = true
            }
        }
    }

    override func mouseEntered(with event: NSEvent) {
        hovered = true
        needsDisplay = true
        onHoverChanged?(true)
    }

    override func mouseExited(with event: NSEvent) {
        hovered = false
        needsDisplay = true
        onHoverChanged?(false)
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        // Only while the badge is actually painted: an invisible hit target
        // would close the switcher on what looks like a plain re-select click.
        if hovered, closable, closeHitRect.contains(point) {
            onCloseRequested?()
            return
        }
        super.mouseDown(with: event)
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard hovered, closable else { return }

        // The chip has to fully hide the icon it replaces, so it stays wider
        // than the rendered icon; anything smaller leaves a ring of favicon
        // around the cross.
        let indicator = closeHitRect.insetBy(dx: 1.5, dy: 1.5)
        LxAppHostTheme.surfaceBackground.setFill()
        NSBezierPath(roundedRect: indicator, xRadius: 6, yRadius: 6).fill()

        let inset: CGFloat = 7
        let mark = indicator.insetBy(dx: inset, dy: inset)
        let path = NSBezierPath()
        path.lineWidth = 1.5
        path.lineCapStyle = .round
        path.move(to: NSPoint(x: mark.minX, y: mark.minY))
        path.line(to: NSPoint(x: mark.maxX, y: mark.maxY))
        path.move(to: NSPoint(x: mark.minX, y: mark.maxY))
        path.line(to: NSPoint(x: mark.maxX, y: mark.minY))
        LxAppHostTheme.foreground.setStroke()
        path.stroke()
    }

    private var closeHitRect: NSRect {
        let size = SidebarView.Layout.railCloseBadgeSize
        return NSRect(
            x: bounds.midX - size / 2,
            y: bounds.midY - size / 2,
            width: size,
            height: size
        )
    }

    override func rightMouseDown(with event: NSEvent) {
        if let onContextMenuRequested {
            onContextMenuRequested(event, self)
        } else {
            super.rightMouseDown(with: event)
        }
    }
}

@MainActor
private final class SidebarHeaderActionButton: NSButton {
    private var trackingArea: NSTrackingArea?
    private var hovered = false
    private var pressed = false

    override var mouseDownCanMoveWindow: Bool { false }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.cornerRadius = 6
        updateAppearance()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        hovered = true
        updateAppearance()
    }

    override func mouseExited(with event: NSEvent) {
        hovered = false
        updateAppearance()
    }

    override func mouseDown(with event: NSEvent) {
        pressed = true
        updateAppearance()
        super.mouseDown(with: event)
        pressed = false
        updateAppearance()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateAppearance()
    }

    private func updateAppearance() {
        layer?.backgroundColor = if pressed {
            SidebarActionChromePalette.pressed.cgColor
        } else if hovered {
            SidebarActionChromePalette.hover.cgColor
        } else {
            NSColor.clear.cgColor
        }
    }
}

@MainActor
private final class SidebarPopoverHoverView: NSView {
    var onHoverChanged: ((Bool) -> Void)?
    private var trackingArea: NSTrackingArea?

    override var mouseDownCanMoveWindow: Bool { false }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        onHoverChanged?(true)
    }

    override func mouseExited(with event: NSEvent) {
        onHoverChanged?(false)
    }
}

@MainActor
private final class SidebarRailTabPopoverViewController: NSViewController {
    private enum Layout {
        static let width: CGFloat = 188
        static let inset: CGFloat = 8
        static let spacing: CGFloat = 2
    }

    private let appId: String
    private let items: [TabBarItem]
    private let selectedIndex: Int

    var onPageSelected: ((String, Int) -> Void)?
    var onDismissRequested: (() -> Void)?
    var onHoverChanged: ((Bool) -> Void)?

    init(appId: String, items: [TabBarItem], selectedIndex: Int) {
        self.appId = appId
        self.items = items
        self.selectedIndex = selectedIndex
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func loadView() {
        let rootView = SidebarPopoverHoverView()
        rootView.wantsLayer = true
        rootView.layer?.cornerRadius = 8
        rootView.onHoverChanged = { [weak self] hovering in
            self?.onHoverChanged?(hovering)
        }

        let stack = NSStackView()
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = Layout.spacing
        rootView.addSubview(stack)

        for (index, item) in items.enumerated() {
            let itemView = SidebarItemView(appId: appId, itemIndex: index)
            itemView.translatesAutoresizingMaskIntoConstraints = false
            itemView.configure(item: item)
            itemView.isSelected = (index == selectedIndex)
            itemView.onClick = { [weak self] selectedIndex in
                guard let self else { return }
                self.onPageSelected?(self.appId, selectedIndex)
                self.onDismissRequested?()
            }
            stack.addArrangedSubview(itemView)
            itemView.widthAnchor.constraint(equalToConstant: Layout.width - (Layout.inset * 2)).isActive = true
        }

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: rootView.leadingAnchor, constant: Layout.inset),
            stack.trailingAnchor.constraint(equalTo: rootView.trailingAnchor, constant: -Layout.inset),
            stack.topAnchor.constraint(equalTo: rootView.topAnchor, constant: Layout.inset),
            stack.bottomAnchor.constraint(equalTo: rootView.bottomAnchor, constant: -Layout.inset),
        ])

        let itemHeight = CGFloat(items.count) * SidebarItemView.Layout.height
        let spacingHeight = CGFloat(max(0, items.count - 1)) * Layout.spacing
        preferredContentSize = NSSize(
            width: Layout.width,
            height: itemHeight + spacingHeight + (Layout.inset * 2)
        )
        view = rootView
    }
}

// MARK: - PanelIconItem

/// Minimal display info for a panel icon in the sidebar footer.
/// SidebarView only needs these — routing details (appId, path) are in Panel.swift.
struct PanelIconItem {
    var generation: UInt64 = 0
    let id: String
    let iconURL: URL?
    let label: String
    let active: Bool
    let disabled: Bool
}

private struct SidebarActionIdentity {
    let generation: UInt64
    let id: String
}

private struct ShellPinItem: Codable, Equatable {
    let kind: String
    let key: String
}

// MARK: - SidebarModel

/// Immutable description of everything the sidebar displays. The four public
/// `update*`/`setActiveHighlight` methods are thin mutators over the pieces of
/// this model; `render()` is the single place that turns it into AppKit views.
///
/// Contract: nothing outside the mutators writes `model`, and every mutator ends
/// with a `render()` call. `render()` never reads transient view state to decide
/// *what* to show — it reads only `model` — so selection truth lives in exactly
/// one place (`selection`).
private struct SidebarModel {
    /// One lxapp group entry. Carries only what `render()` needs; per-group page
    /// contents are still pulled from Rust by SidebarGroupView itself.
    struct AppGroupVM {
        let appId: String
        let asideSurfaceId: String?
        let managedLabel: String?
        let managedIcon: NSImage?
        let contentAppId: String?
        let showsLxappTabBar: Bool
        let closable: Bool

        var isManagedMain: Bool { managedLabel != nil }
    }

    /// One browser tab row.
    struct BrowserTabVM {
        let id: String
        let title: String
        let url: String
        let favicon: NSImage?
    }

    /// The single source of selection truth, shared by both the expanded list
    /// and the collapsed rail.
    enum Selection: Equatable {
        case none
        /// `pageIndex == nil` means "highlight the app, page index from Rust".
        case app(appId: String, pageIndex: Int?)
        case browser(id: String)
    }

    var appGroups: [AppGroupVM] = []
    var browserTabs: [BrowserTabVM] = []
    var browserRootVisible = false
    var panelItems: [PanelIconItem] = []
    var selection: Selection = .none
}

extension PanelIconItem: Equatable {
    static func == (lhs: PanelIconItem, rhs: PanelIconItem) -> Bool {
        lhs.id == rhs.id && lhs.iconURL == rhs.iconURL && lhs.label == rhs.label
            && lhs.active == rhs.active && lhs.disabled == rhs.disabled
    }
}

// MARK: - SidebarView

/// The main sidebar container view, modeled after Chrome vertical tab groups.
/// Supports drag-to-resize and an icon-only rail.
@MainActor
class SidebarView: NSView, NSPopoverDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "Sidebar")

    struct Layout {
        /// One leading axis for every first-column icon in the sidebar — group
        /// headers place an 18pt icon at groupInset + headerHPadding, and the
        /// traffic lights, pin grid, and footer buttons center on the same line.
        static let iconAxis: CGFloat =
            SidebarGroupView.Layout.groupInset + SidebarGroupView.Layout.headerHPadding + 9

        static let expandedWidth: CGFloat = 184
        static let maxWidth: CGFloat = 400
        static let fullyHiddenThreshold: CGFloat = 1
        /// Minimum width of the collapsed icon-only rail. The effective width
        /// grows to clear the macOS traffic lights when they're wider (see
        /// `effectiveRailWidth`).
        static let railWidth: CGFloat = 60
        /// Drag-end below this snaps to the icon rail; at/above it the sidebar expands.
        static let railExpandThreshold: CGFloat = 128
        /// Square icon button in the rail.
        static let railButtonSize: CGFloat = 34
        /// Rendered icon size inside a rail button.
        static let railIconSize: CGFloat = 22
        /// Close chip overlaid on the active rail switcher. Wider than
        /// `railIconSize` so it covers the icon, narrower than
        /// `railButtonSize` so the tile edge still re-selects.
        static let railCloseBadgeSize: CGFloat = railIconSize + 6
        // Reserve only the shared traffic-light / toolbar row; the titlebar offset is
        // already handled by `buttonCenterYFromTop`.
        static let trafficLightsHeight: CGFloat = 38
        static let actionButtonSize: CGFloat = 28
        static let resizeHandleWidth: CGFloat = 5
        /// Bottom dock height — tall enough for one row of icon buttons plus breathing room.
        static let footerHeight: CGFloat = 48
        /// sidebar action row height — matches the tabbar item rhythm above.
        static let footerButtonSize: CGFloat = 30
        /// Rendered glyph size inside footer icon buttons.
        static let footerIconSize: CGFloat = 16
        /// Shared outer inset for the sidebar action flow. Windows uses the same
        /// 6pt margin, leaving 172pt of flow width in the standard 184pt rail.
        static let footerInset: CGFloat = 6
        /// iconAxis − row internal padding (7) − half icon (9): footer button
        /// icons center on the shared first-column axis. Horizontal only —
        /// the vertical insets keep `footerInset` so rows still fit the
        /// fixed `footerHeight`.
        static let footerHInset: CGFloat = iconAxis - 16
        /// Rows shown before the sidebar action area caps and scrolls internally.
        static let footerMaxRows: CGFloat = 5
    }

    private let headerView = NSView()
    private let headerActionStack = NSStackView()
    /// Keeps the action buttons clear of the traffic lights. The clearance is
    /// measured, not assumed: the buttons may be hidden or placed in the
    /// toolbar, and reserving for them anyway leaves a visibly empty strip that
    /// the header then refuses to use.
    private var headerActionLeadingConstraint: NSLayoutConstraint?
    private var headerActionItems: [PanelIconItem] = []
    private var headerActionIdentities: [ObjectIdentifier: SidebarActionIdentity] = [:]
    private let scrollView = SidebarScrollView()
    private let resizeHandle = SidebarResizeHandle()
    private let footerView = NSView()
    private let footerSeparator = NSView()
    /// Footer height tracks the sidebar action row count (see renderPanelItems).
    private var footerHeightConstraint: NSLayoutConstraint?
    /// Adaptive flow that keeps short sidebar actions on the same visual row.
    private let panelFlow = SidebarActionFlowView()
    /// Caps the sidebar action area: rows beyond footerMaxRows scroll in here.
    private let panelScroll = NSScrollView()
    /// The expanded-state collapse toggle. Lives in the header, next to the
    /// sidebar actions; clicking it collapses the sidebar to the icon rail.
    private let hideButton = NSButton()
    private var hideButtonTrackingArea: NSTrackingArea?
    /// The rail-state expand toggle — the first icon in the collapsed rail,
    /// above the lxapp icons; clicking it restores the expanded sidebar.
    private let railExpandButton = NSButton()
    private var panelButtons: [SidebarActionRowView] = []
    /// The panel items currently materialized as footer buttons. Lets
    /// renderPanelItems() skip a rebuild when render() runs for an unrelated
    /// change — so `updatePanelIcon`'s resolved icons aren't wiped out.
    private var renderedPanelItems: [PanelIconItem] = []
    private var appUIOnlyMode = false

    // MARK: Icon-rail (collapsed) state

    /// True when the sidebar is collapsed to the icon-only rail.
    private(set) var isCompact = false

    /// Rail top inset. Normally clears the traffic lights; a host with no traffic
    /// lights (the frameless runner) zeroes it so the first rail icon aligns with
    /// the content/webview top instead of sitting a header-height below it.
    private var railTopConstraint: NSLayoutConstraint?
    /// Supplies the minimum width that still clears the macOS traffic lights,
    /// so the rail can be as narrow as those controls allow.
    var trafficLightClearanceProvider: (() -> CGFloat)?
    /// Rail width that both honors the minimum and clears the traffic lights.
    /// The shell's clearance leaves ~12pt of breathing room for the expanded
    /// layout; the rail hugs the traffic lights with only a small gap to the
    /// webview edge.
    private var effectiveRailWidth: CGFloat {
        let clearance = trafficLightClearanceProvider?() ?? Layout.railWidth
        return max(Layout.railWidth, clearance - 8)
    }
    var compactWidth: CGFloat {
        effectiveRailWidth
    }
    /// Container hosting the rail; shown only in compact mode.
    private let railScrollView = SidebarScrollView()
    private let railStack = NSStackView()
    /// Footer actions stay anchored above the rail expand control, matching the
    /// expanded footer and the Windows rail instead of joining navigation.
    private let railFooterScrollView = SidebarScrollView()
    private let railFooterStack = NSStackView()
    /// Rail buttons keyed by a composite id ("app:<appId>" / "web:<tabId>").
    private var railButtons: [String: NSButton] = [:]
    private var railTabPopover: NSPopover?
    private weak var railTabPopoverButton: NSButton?
    private var railTabPopoverAppId: String?
    private var railTabPopoverDismissTask: Task<Void, Never>?
    private var isRailTabPopoverHovered = false

    /// The bundled default LingXia mark, used when an lxapp declares no icon.
    private static let defaultAppIcon: NSImage? = {
        guard let url = Bundle.lingxiaResources.url(
            forResource: "lxapp_default", withExtension: "png", subdirectory: "icons")
        else { return nil }
        return NSImage(contentsOf: url)
    }()

    /// A shared design icon (bundled PDF) as a tintable template image, so the
    /// header affordances match their iOS counterparts.
    private static func designIcon(_ name: String) -> NSImage? {
        guard let url = Bundle.lingxiaResources.url(forResource: name, withExtension: "pdf", subdirectory: "icons")
        else { return nil }
        let image = NSImage(contentsOf: url)
        image?.isTemplate = true
        image?.size = NSSize(width: 16, height: 16)
        return image
    }

    private static func sidebarHeaderActionIcon(_ url: URL?) -> NSImage? {
        guard let url, let image = NSImage(contentsOf: url) else { return nil }
        image.isTemplate = true
        image.size = NSSize(width: 16, height: 16)
        return image
    }

    /// Called when a panel icon button is clicked: (panelId)
    var onPanelItemToggled: ((UInt64, String) -> Void)?

    /// Called when the update callout is clicked, with its current state
    /// (`.ready` → restart, `.available` → install).
    var onUpdateActionRequested: ((UpdateCalloutState) -> Void)?

    /// The transient "ready to update" callout shown above the footer dock.
    private var updateReadyCallout: UpdateReadyCallout?

    /// The single immutable model that drives the whole sidebar. Mutated only by
    /// the public `update*`/`setActiveHighlight`/`clearAllHighlights` methods,
    /// each of which calls `render()` afterwards.
    private var model = SidebarModel()
    private var lxappGroups: [SidebarModel.AppGroupVM] = []
    private var managedMainGroups: [SidebarModel.AppGroupVM] = []

    // MARK: Render-side view caches (rebuilt/diffed from `model` by render()).
    private var groupViews: [String: SidebarGroupView] = [:]

    // Browser tab views
    private var browserItemViews: [String: SidebarBrowserItemView] = [:]
    private var browserItemTopConstraints: [String: NSLayoutConstraint] = [:]
    private let browserRootHeader = NSView()
    private var browserRootTopConstraint: NSLayoutConstraint?
    private var browserRootHeaderConfigured = false
    private let addButton = NSButton()
    private var addButtonTopConstraint: NSLayoutConstraint?
    private var groupTopConstraints: [String: NSLayoutConstraint] = [:]
    private var addButtonTrackingArea: NSTrackingArea?
    private var isAddButtonHovered = false
    private var isHideButtonHovered = false

    /// Target center Y for the header buttons, measured from the header's top edge.
    var buttonCenterYFromTop: CGFloat = Layout.trafficLightsHeight / 2 {
        didSet {
            guard oldValue != buttonCenterYFromTop else { return }
            buttonCenterYConstraints.forEach { $0.constant = buttonCenterYFromTop }
        }
    }
    private var buttonCenterYConstraints: [NSLayoutConstraint] = []

    var isFullyHidden: Bool {
        return frame.width < Layout.fullyHiddenThreshold
    }

    /// Called when user selects a page: (appId, itemIndex)
    var onAppPageSelected: ((String, Int) -> Void)?
    /// Called when user clicks an lxapp's name (group header): (appId) — switch
    /// the main to that lxapp even if it has no tabBar pages.
    var onAppSelected: ((String) -> Void)?
    /// Called when user requests to close an app: (appId)
    var onAppCloseRequested: ((String) -> Void)?
    var onManagedMainContextMenuRequested: ((String, NSEvent, NSView) -> Void)?
    var onManagedMainRenameCommitted: ((String, String) -> Void)?
    /// Called when the bottom hide button is clicked
    var onHideRequested: (() -> Void)?
    /// Called when the rail expand button is clicked
    var onShowRequested: (() -> Void)?
    /// Called when width changes via drag: (width, animated)
    var onWidthChanged: ((CGFloat, Bool) -> Void)?
    /// Called when the global "+" button requests content for the active main.
    var onAddRequested: (() -> Void)?
    /// Called when a browser tab is selected
    var onBrowserTabSelected: ((String) -> Void)?
    /// Called when a browser tab close is requested
    var onBrowserTabCloseRequested: ((String) -> Void)?
    /// Called when all browser tabs except the given tab should close.
    var onBrowserTabCloseOtherRequested: ((String) -> Void)?
    /// Called when browser tabs visually below the given tab should close.
    var onBrowserTabCloseTabsBelowRequested: ((String) -> Void)?
    /// Called when a pin tile with no open tab is clicked (open its URL)
    var onBookmarkOpen: ((String) -> Void)?
    /// Called from tile menus to open the bookmarks manager page
    var onManageBookmarks: (() -> Void)?

    // MARK: Pin grid state. Pins are persistent website shortcuts above the
    // normal tab list; they never replace or hide an open tab.
    private var bookmarksSnapshot = SidebarBookmarksSnapshot.empty
    private var pinTileViews: [String: SidebarPinTileView] = [:]
    private var shellPinItems: [ShellPinItem] = []
    private var lxappPinTiles: [String: LxappPinTileView] = [:]
    private var pinTileTopConstraints: [String: NSLayoutConstraint] = [:]
    private var pinTileLeadingConstraints: [String: NSLayoutConstraint] = [:]

    var hasPinnedWebsites: Bool {
        !shellPinItems.isEmpty
    }


    private var pinnedLxappIds: [String] {
        shellPinItems.compactMap { $0.kind == "lxapp" ? $0.key : nil }
    }

    private var pinnedBookmarkEntries: [SidebarBookmarksSnapshot.Entry] {
        shellPinItems.compactMap { pin in
            guard pin.kind == "bookmark" else { return nil }
            return bookmarksSnapshot.entries.first { $0.id == pin.key }
        }
    }

    private func openTabId(for entry: SidebarBookmarksSnapshot.Entry) -> String? {
        let key = SidebarBookmarksSnapshot.normalize(entry.url)
        let matching = model.browserTabs.filter {
            SidebarBookmarksSnapshot.normalize($0.url) == key
        }
        if case .browser(let activeId) = model.selection,
           matching.contains(where: { $0.id == activeId }) {
            return activeId
        }
        return matching.first?.id
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // Prevent window drag when SidebarView itself receives events
    override public var mouseDownCanMoveWindow: Bool { false }

    override func layout() {
        super.layout()
        updateSidebarActionFooterHeight()
        updateHeaderActionVisibility()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        footerSeparator.layer?.backgroundColor = SidebarActionChromePalette.divider.cgColor
        updateAddButtonAppearance()
        updateHideButtonAppearance()
        applySelection()
    }

    // MARK: - Setup

    private func setupViews() {
        wantsLayer = true
        clipsToBounds = true

        // Header (traffic lights + actions)
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.wantsLayer = true
        addSubview(headerView)

        headerActionStack.translatesAutoresizingMaskIntoConstraints = false
        headerActionStack.orientation = .horizontal
        headerActionStack.alignment = .centerY
        headerActionStack.spacing = 4
        headerView.addSubview(headerActionStack)

        // Scroll view (trailing inset to leave room for resize handle)
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.contentView = SidebarClipView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.verticalScrollElasticity = .none
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        addSubview(scrollView)

        // Document view (flipped)
        let flipView = FlippedView()
        flipView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.documentView = flipView

        // Icon rail (collapsed state) — a centered vertical strip of app/tab icons.
        railScrollView.translatesAutoresizingMaskIntoConstraints = false
        railScrollView.contentView = SidebarClipView()
        railScrollView.hasVerticalScroller = false
        railScrollView.hasHorizontalScroller = false
        railScrollView.scrollerStyle = .overlay
        railScrollView.verticalScrollElasticity = .none
        railScrollView.drawsBackground = false
        railScrollView.borderType = .noBorder
        railScrollView.isHidden = true
        addSubview(railScrollView)

        let railDoc = FlippedView()
        railDoc.translatesAutoresizingMaskIntoConstraints = false
        railScrollView.documentView = railDoc

        railStack.translatesAutoresizingMaskIntoConstraints = false
        railStack.orientation = .vertical
        railStack.alignment = .centerX
        railStack.spacing = 6
        railDoc.addSubview(railStack)
        NSLayoutConstraint.activate([
            railDoc.leadingAnchor.constraint(equalTo: railScrollView.contentView.leadingAnchor),
            railDoc.trailingAnchor.constraint(equalTo: railScrollView.contentView.trailingAnchor),
            railDoc.topAnchor.constraint(equalTo: railScrollView.contentView.topAnchor),
            railStack.topAnchor.constraint(equalTo: railDoc.topAnchor, constant: 6),
            railStack.centerXAnchor.constraint(equalTo: railDoc.centerXAnchor),
            railStack.bottomAnchor.constraint(equalTo: railDoc.bottomAnchor, constant: -6),
        ])

        railFooterScrollView.translatesAutoresizingMaskIntoConstraints = false
        railFooterScrollView.contentView = SidebarClipView()
        railFooterScrollView.hasVerticalScroller = false
        railFooterScrollView.hasHorizontalScroller = false
        railFooterScrollView.verticalScrollElasticity = .none
        railFooterScrollView.drawsBackground = false
        railFooterScrollView.borderType = .noBorder
        footerView.addSubview(railFooterScrollView)

        let railFooterDoc = FlippedView()
        railFooterDoc.translatesAutoresizingMaskIntoConstraints = false
        railFooterScrollView.documentView = railFooterDoc
        railFooterStack.translatesAutoresizingMaskIntoConstraints = false
        railFooterStack.orientation = .vertical
        railFooterStack.alignment = .centerX
        railFooterStack.spacing = 6
        railFooterDoc.addSubview(railFooterStack)
        NSLayoutConstraint.activate([
            railFooterDoc.leadingAnchor.constraint(equalTo: railFooterScrollView.contentView.leadingAnchor),
            railFooterDoc.trailingAnchor.constraint(equalTo: railFooterScrollView.contentView.trailingAnchor),
            railFooterDoc.topAnchor.constraint(equalTo: railFooterScrollView.contentView.topAnchor),
            railFooterStack.topAnchor.constraint(equalTo: railFooterDoc.topAnchor),
            railFooterStack.centerXAnchor.constraint(equalTo: railFooterDoc.centerXAnchor),
            railFooterStack.bottomAnchor.constraint(equalTo: railFooterDoc.bottomAnchor),
        ])

        // Footer dock — bottom toolbar row for icon buttons
        footerView.translatesAutoresizingMaskIntoConstraints = false
        footerView.wantsLayer = true
        addSubview(footerView)

        // Hairline separator between scroll content and footer
        footerSeparator.translatesAutoresizingMaskIntoConstraints = false
        footerSeparator.wantsLayer = true
        // A subtle divider grouping the sidebar action dock. `separatorColor` washes
        // out on the sidebar material, so use a low-alpha label tint that keeps a
        // little contrast in both light and dark without being prominent.
        footerSeparator.layer?.backgroundColor = SidebarActionChromePalette.divider.cgColor
        footerView.addSubview(footerSeparator)

        panelFlow.translatesAutoresizingMaskIntoConstraints = false
        // Short icon+title cells share a visual row; cells wrap as a whole only
        // when the next minimum width no longer fits. The document remains
        // taller than the capped footer so overflow scrolls internally.
        panelScroll.translatesAutoresizingMaskIntoConstraints = false
        panelScroll.drawsBackground = false
        panelScroll.hasVerticalScroller = true
        panelScroll.autohidesScrollers = true
        // Keep the scrollbar out of the flow width. A legacy scroller reserves
        // enough space to force Terminal + Ping onto separate rows at 184pt.
        panelScroll.scrollerStyle = .overlay
        panelScroll.verticalScrollElasticity = .none
        let panelDoc = FlippedClipView()
        panelDoc.translatesAutoresizingMaskIntoConstraints = false
        panelDoc.addSubview(panelFlow)
        panelScroll.documentView = panelDoc
        NSLayoutConstraint.activate([
            panelFlow.topAnchor.constraint(equalTo: panelDoc.topAnchor),
            panelFlow.leadingAnchor.constraint(equalTo: panelDoc.leadingAnchor),
            panelFlow.trailingAnchor.constraint(equalTo: panelDoc.trailingAnchor),
            panelFlow.bottomAnchor.constraint(equalTo: panelDoc.bottomAnchor),
            panelDoc.widthAnchor.constraint(equalTo: panelScroll.widthAnchor),
        ])
        footerView.addSubview(panelScroll)

        hideButton.translatesAutoresizingMaskIntoConstraints = false
        hideButton.title = ""
        hideButton.image = LxIcon.image(
            named: "icon_sidebar_collapse",
            size: NSSize(width: 18, height: 18))
        hideButton.imagePosition = .imageOnly
        hideButton.isBordered = false
        hideButton.bezelStyle = .regularSquare
        hideButton.contentTintColor = LxAppHostTheme.mutedForeground
        hideButton.wantsLayer = true
        hideButton.layer?.cornerRadius = 6
        hideButton.layer?.backgroundColor = NSColor.clear.cgColor
        hideButton.toolTip = "Collapse sidebar"
        hideButton.setAccessibilityLabel("Collapse sidebar")
        hideButton.target = self
        hideButton.action = #selector(hideButtonClicked)
        headerView.addSubview(hideButton)

        // Rail expand toggle: pinned to the bottom of the rail (not in the
        // scrolling icon stack) so it stays anchored as chrome below the
        // sidebar actions, leaving the top free for a future branding header.
        railExpandButton.translatesAutoresizingMaskIntoConstraints = false
        railExpandButton.isBordered = false
        railExpandButton.bezelStyle = .regularSquare
        railExpandButton.imagePosition = .imageOnly
        railExpandButton.imageScaling = .scaleProportionallyDown
        railExpandButton.wantsLayer = true
        railExpandButton.layer?.cornerRadius = 8
        railExpandButton.layer?.backgroundColor = NSColor.clear.cgColor
        railExpandButton.toolTip = "Expand sidebar"
        railExpandButton.setAccessibilityLabel("Expand sidebar")
        railExpandButton.contentTintColor = LxAppHostTheme.mutedForeground
        railExpandButton.image = LxIcon.image(
            named: "icon_sidebar_expand",
            size: NSSize(width: Layout.railIconSize, height: Layout.railIconSize))
        railExpandButton.target = self
        railExpandButton.action = #selector(railExpandClicked)
        footerView.addSubview(railExpandButton)
        NSLayoutConstraint.activate([
            railExpandButton.widthAnchor.constraint(equalToConstant: Layout.railButtonSize),
            railExpandButton.heightAnchor.constraint(equalToConstant: Layout.railButtonSize),
            railExpandButton.centerXAnchor.constraint(equalTo: footerView.centerXAnchor),
            railExpandButton.bottomAnchor.constraint(equalTo: footerView.bottomAnchor, constant: -6),
            railFooterScrollView.topAnchor.constraint(equalTo: footerSeparator.bottomAnchor, constant: 6),
            railFooterScrollView.leadingAnchor.constraint(equalTo: footerView.leadingAnchor),
            railFooterScrollView.trailingAnchor.constraint(equalTo: footerView.trailingAnchor),
            railFooterScrollView.bottomAnchor.constraint(equalTo: railExpandButton.topAnchor, constant: -6),
        ])

        // Resize handle on right edge
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false
        resizeHandle.wantsLayer = true
        addSubview(resizeHandle)

        railTopConstraint = railScrollView.topAnchor.constraint(
            equalTo: topAnchor, constant: Layout.trafficLightsHeight)

        NSLayoutConstraint.activate([
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerView.heightAnchor.constraint(equalToConstant: Layout.trafficLightsHeight),

            hideButton.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -8),

            headerActionStack.trailingAnchor.constraint(equalTo: hideButton.leadingAnchor, constant: -4),
            headerActionLeadingClearance(),

            // Scroll view: inset trailing by resize handle width, extends above footer
            scrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            scrollView.bottomAnchor.constraint(equalTo: footerView.topAnchor),

            // Rail occupies the same region as the main scroll view, but its top
            // inset is adjustable (see railTopConstraint) — the rail's header is
            // empty in compact mode, so a frameless host can pull it to the top.
            railTopConstraint!,
            railScrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            railScrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            railScrollView.bottomAnchor.constraint(equalTo: footerView.topAnchor),

            footerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            footerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            footerView.bottomAnchor.constraint(equalTo: bottomAnchor),

            footerSeparator.topAnchor.constraint(equalTo: footerView.topAnchor),
            footerSeparator.leadingAnchor.constraint(
                equalTo: footerView.leadingAnchor, constant: Layout.footerInset),
            footerSeparator.trailingAnchor.constraint(
                equalTo: footerView.trailingAnchor, constant: -Layout.footerInset),
            footerSeparator.heightAnchor.constraint(equalToConstant: 1.0),

            hideButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            hideButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

            panelScroll.leadingAnchor.constraint(
                equalTo: footerView.leadingAnchor, constant: Layout.footerHInset),
            panelScroll.trailingAnchor.constraint(equalTo: footerView.trailingAnchor, constant: -Layout.footerHInset),
            panelScroll.topAnchor.constraint(equalTo: footerView.topAnchor, constant: Layout.footerInset + 1),
            panelScroll.bottomAnchor.constraint(equalTo: footerView.bottomAnchor, constant: -Layout.footerInset),

            // Resize handle: right edge, full height
            resizeHandle.topAnchor.constraint(equalTo: topAnchor),
            resizeHandle.trailingAnchor.constraint(equalTo: trailingAnchor),
            resizeHandle.bottomAnchor.constraint(equalTo: bottomAnchor),
            resizeHandle.widthAnchor.constraint(equalToConstant: Layout.resizeHandleWidth),
        ])

        let footerHeight = footerView.heightAnchor.constraint(equalToConstant: Layout.footerHeight)
        footerHeight.isActive = true
        footerHeightConstraint = footerHeight

        // Button center constraints — stored so we can align them to the effective traffic-light center.
        let centerY = buttonCenterYFromTop
        let headerActionsCenter = headerActionStack.centerYAnchor.constraint(
            equalTo: headerView.topAnchor, constant: centerY)
        let toggleCenter = hideButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        buttonCenterYConstraints = [headerActionsCenter, toggleCenter]
        NSLayoutConstraint.activate(buttonCenterYConstraints)

        // Document view fills scroll view width
        if let docView = scrollView.documentView {
            NSLayoutConstraint.activate([
                docView.leadingAnchor.constraint(equalTo: scrollView.contentView.leadingAnchor),
                docView.trailingAnchor.constraint(equalTo: scrollView.contentView.trailingAnchor),
                docView.topAnchor.constraint(equalTo: scrollView.contentView.topAnchor),
            ])
        }

        // Separator line inside the resize handle (hidden for seamless blending with Layer 2)
        let separatorLine = NSView()
        separatorLine.translatesAutoresizingMaskIntoConstraints = false
        separatorLine.wantsLayer = true
        separatorLine.layer?.backgroundColor = NSColor.clear.cgColor  // Hidden for seamless appearance
        resizeHandle.addSubview(separatorLine)

        NSLayoutConstraint.activate([
            separatorLine.topAnchor.constraint(equalTo: resizeHandle.topAnchor),
            separatorLine.trailingAnchor.constraint(equalTo: resizeHandle.trailingAnchor),
            separatorLine.bottomAnchor.constraint(equalTo: resizeHandle.bottomAnchor),
            separatorLine.widthAnchor.constraint(equalToConstant: 1),
        ])

        resizeHandle.onDrag = { [weak self] newWidth in
            self?.handleDrag(proposedWidth: newWidth)
        }
        resizeHandle.onDragEnd = { [weak self] newWidth in
            self?.handleDragEnd(proposedWidth: newWidth)
        }
    }

    // MARK: - Drag Handling

    private func handleDrag(proposedWidth: CGFloat) {
        let clamped = min(max(proposedWidth, 0), Layout.maxWidth)
        // Live feedback: show the icon rail while in the rail zone so the
        // expanded layout never has to render squished at narrow widths. A drag
        // cannot hide the rail; full hiding is an explicit host/user mode.
        let compact = clamped < Layout.railExpandThreshold
        setCompactMode(compact)
        onWidthChanged?(compact ? max(clamped, effectiveRailWidth) : clamped, false)
    }

    private func handleDragEnd(proposedWidth: CGFloat) {
        if proposedWidth < Layout.railExpandThreshold {
            setCompactMode(true)
            onWidthChanged?(effectiveRailWidth, true)
        } else {
            setCompactMode(false)
            let clamped = min(max(proposedWidth, Layout.expandedWidth), Layout.maxWidth)
            onWidthChanged?(clamped, true)
        }
    }

    // MARK: - Compact (icon-rail) mode

    /// Switch between the expanded sidebar and the collapsed icon rail.
    /// When true (a frameless host with no traffic lights), the collapsed rail's
    /// first icon aligns to the very top instead of clearing a traffic-light header.
    func setRailAlignedToTop(_ alignedToTop: Bool) {
        railTopConstraint?.constant = alignedToTop ? 0 : Layout.trafficLightsHeight
    }

    func setCompactMode(_ compact: Bool) {
        guard compact != isCompact else { return }
        isCompact = compact
        if compact {
            rebuildRail()
        } else {
            closeRailTabPopover()
        }
        updateSidebarActionFooterHeight()
        updateVisibilityState()
    }

    /// The composite rail/selection key ("app:<appId>" / "web:<tabId>") derived
    /// from the single `model.selection`, so rail and list agree by construction.
    private var activeRailKey: String? {
        switch model.selection {
        case .none: return nil
        case .app(let appId, _): return "app:\(appId)"
        case .browser(let id): return "web:\(id)"
        }
    }

    /// Rebuild the rail's icon buttons from the current lxapps + browser tabs.
    private func rebuildRail() {
        closeRailTabPopover()
        railStack.arrangedSubviews.forEach {
            railStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        railButtons.removeAll()

        var lastPinButton: NSView?

        for pin in shellPinItems {
            switch pin.kind {
            case "lxapp":
                let info = getLxAppInfo(pin.key)
                let iconPath = info.icon.toString()
                let image = (iconPath.isEmpty ? nil : NSImage(contentsOfFile: iconPath))
                    ?? Self.defaultAppIcon
                let name = info.app_name.toString()
                let key = "pin-lxapp:\(pin.key)"
                let button = makeRailButton(
                    key: key,
                    tooltip: name.isEmpty ? pin.key : name,
                    image: image,
                    isTemplate: false
                )
                button.action = #selector(railPinnedLxappClicked(_:))
                railStack.addArrangedSubview(button)
                railButtons[key] = button
                lastPinButton = button
            case "bookmark":
                guard let entry = bookmarksSnapshot.entries.first(where: { $0.id == pin.key }) else {
                    continue
                }
                let key = "pin-bookmark:\(pin.key)"
                let button = makeRailButton(
                    key: key,
                    tooltip: entry.title.isEmpty ? entry.url : entry.title,
                    image: LxIcon.image(
                        named: "icon_globe",
                        size: CGSize(width: Layout.railIconSize, height: Layout.railIconSize)
                    ),
                    isTemplate: true
                )
                button.action = #selector(railPinnedBookmarkClicked(_:))
                railStack.addArrangedSubview(button)
                railButtons[key] = button
                lastPinButton = button
                SidebarFaviconLoader.load(urlString: entry.url) { [weak self, weak button] image in
                    guard let self, let button,
                          self.railButtons[key] === button else { return }
                    let copy = image.copy() as? NSImage ?? image
                    copy.size = NSSize(width: Layout.railIconSize, height: Layout.railIconSize)
                    copy.isTemplate = false
                    button.image = copy
                    button.contentTintColor = nil
                }
            default:
                continue
            }
        }

        if let lastPinButton,
           !model.appGroups.isEmpty || !model.browserTabs.isEmpty {
            // A grouping hairline, not a rule: narrower than the icons it
            // separates, at the separator color's own alpha, and given more air
            // than the 6pt icon rhythm so the gap does the grouping.
            let divider = LxAppHostThemeLayerView(role: .separator, alpha: nil)
            divider.translatesAutoresizingMaskIntoConstraints = false
            railStack.addArrangedSubview(divider)
            railStack.setCustomSpacing(5, after: lastPinButton)
            railStack.setCustomSpacing(5, after: divider)
            NSLayoutConstraint.activate([
                divider.widthAnchor.constraint(equalToConstant: 18),
                divider.heightAnchor.constraint(equalToConstant: 1),
            ])
        }

        for group in model.appGroups {
            let tooltip: String
            let image: NSImage?
            if group.isManagedMain {
                tooltip = group.managedLabel ?? group.appId
                image = group.managedIcon ?? Self.defaultAppIcon
            } else {
                let info = getLxAppInfo(group.appId)
                let iconPath = info.icon.toString()
                tooltip = info.app_name.toString()
                if !iconPath.isEmpty, let img = NSImage(contentsOfFile: iconPath) {
                    image = img
                } else {
                    image = Self.defaultAppIcon
                }
            }
            let key = "app:\(group.appId)"
            let btn = makeRailButton(
                key: key,
                tooltip: tooltip,
                image: image,
                // Native/browser glyphs are tintable. Lxapp provider assets are
                // full-color artwork; treating their opaque tile as a template
                // turns the entire icon into a flat white/gray square.
                isTemplate: group.isManagedMain && group.contentAppId == nil
            )
            btn.action = #selector(railAppClicked(_:))
            if let railButton = btn as? SidebarRailButton {
                railButton.closable = group.closable && activeRailKey == key
                railButton.onCloseRequested = { [weak self] in
                    self?.closeRailTabPopover()
                    self?.onAppCloseRequested?(group.appId)
                }
                railButton.onHoverChanged = { [weak self, weak railButton] hovering in
                    guard let self, let railButton else { return }
                    if hovering && !group.isManagedMain {
                        self.showRailTabPopover(appId: group.appId, relativeTo: railButton)
                    } else {
                        self.scheduleRailTabPopoverDismiss()
                    }
                }
            }
            railStack.addArrangedSubview(btn)
            railButtons[key] = btn
        }

        for item in model.browserTabs {
            let key = "web:\(item.id)"
            let image = item.favicon ?? LxIcon.image(
                named: "icon_globe", size: CGSize(width: Layout.railIconSize, height: Layout.railIconSize))
            let btn = makeRailButton(
                key: key,
                tooltip: browserTooltip(title: item.title, url: item.url),
                image: image,
                isTemplate: item.favicon == nil
            )
            btn.action = #selector(railBrowserClicked(_:))
            if let railButton = btn as? SidebarRailButton {
                railButton.closable = activeRailKey == key
                railButton.onCloseRequested = { [weak self] in
                    self?.closeRailTabPopover()
                    self?.onBrowserTabCloseRequested?(item.id)
                }
                railButton.onHoverChanged = { [weak self] hovering in
                    if hovering { self?.closeRailTabPopover() }
                }
                railButton.onContextMenuRequested = { [weak self] event, button in
                    guard let menu = self?.browserContextMenu(for: item.id) else { return }
                    NSMenu.popUpContextMenu(menu, with: event, for: button)
                }
            }
            railStack.addArrangedSubview(btn)
            railButtons[key] = btn
        }

        // New-tab affordance for the collapsed rail. URL Runner hosts opt in
        // through the persistent browser root even without `capBrowser`.
        let browserEnabled = (LxAppCore.capabilities & LxAppCore.capBrowser) != 0
            || model.browserRootVisible
        if browserEnabled {
            let addRailButton = makeRailButton(
                key: "action:add-tab",
                tooltip: L10n.string("lx_browser_new_tab"),
                image: LxIcon.image(
                    named: "icon_browser_plus",
                    size: CGSize(width: Layout.railIconSize, height: Layout.railIconSize))
                    ?? NSImage(systemSymbolName: "plus", accessibilityDescription: nil),
                isTemplate: true
            )
            addRailButton.action = #selector(addButtonClicked)
            railStack.addArrangedSubview(addRailButton)
        }

        railFooterStack.arrangedSubviews.forEach {
            railFooterStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        // Footer actions keep their bottom ownership in the rail. Only their
        // icon remains; the label moves to the tooltip.
        for item in model.panelItems {
            let image = item.iconURL.flatMap { NSImage(contentsOf: $0) }
            let key = "sidebar-action:\(item.id)"
            let button = makeRailButton(
                key: key,
                tooltip: item.label,
                image: image,
                isTemplate: false
            )
            button.action = #selector(railSidebarActionClicked(_:))
            button.isEnabled = !item.disabled
            railFooterStack.addArrangedSubview(button)
            railButtons[key] = button
        }

        // The expand toggle is not part of this stack — it's pinned to the rail's
        // bottom in setup() so it always anchors the bottom regardless of how many
        // sidebar actions are present.

        refreshRailHighlight()
    }

    private func makeRailButton(key: String, tooltip: String, image: NSImage?, isTemplate: Bool) -> NSButton {
        let btn = SidebarRailButton()
        btn.translatesAutoresizingMaskIntoConstraints = false
        btn.isBordered = false
        btn.bezelStyle = .regularSquare
        btn.imagePosition = .imageOnly
        btn.imageScaling = .scaleProportionallyDown
        btn.wantsLayer = true
        btn.layer?.cornerRadius = 8
        btn.layer?.backgroundColor = NSColor.clear.cgColor
        btn.toolTip = tooltip
        btn.setAccessibilityLabel(tooltip)
        btn.target = self
        btn.identifier = NSUserInterfaceItemIdentifier(key)
        if let image {
            let copy = image.copy() as? NSImage ?? image
            copy.size = NSSize(width: Layout.railIconSize, height: Layout.railIconSize)
            copy.isTemplate = isTemplate
            btn.image = copy
            if isTemplate { btn.contentTintColor = LxAppHostTheme.mutedForeground }
        }
        NSLayoutConstraint.activate([
            btn.widthAnchor.constraint(equalToConstant: Layout.railButtonSize),
            btn.heightAnchor.constraint(equalToConstant: Layout.railButtonSize),
        ])
        return btn
    }

    /// Highlight the active app/tab button in the rail.
    private func refreshRailHighlight() {
        for (key, btn) in railButtons {
            let activeSidebarAction = key.hasPrefix("sidebar-action:")
                && model.panelItems.contains {
                    key == "sidebar-action:\($0.id)" && $0.active && !$0.disabled
                }
            let activePinnedLxapp = key.hasPrefix("pin-lxapp:")
                && activeRailKey == "app:\(key.dropFirst("pin-lxapp:".count))"
            let activePinnedBookmark = key.hasPrefix("pin-bookmark:")
                && pinnedBookmarkEntries.contains { entry in
                    key == "pin-bookmark:\(entry.id)"
                        && openTabId(for: entry).map { "web:\($0)" == activeRailKey } == true
                }
            let selected = key == activeRailKey
                || activeSidebarAction
                || activePinnedLxapp
                || activePinnedBookmark
            btn.layer?.backgroundColor = selected
                ? LxAppHostTheme.selectionBackground.cgColor
                : NSColor.clear.cgColor
        }
    }

    @objc private func railAppClicked(_ sender: NSButton) {
        guard let key = sender.identifier?.rawValue, key.hasPrefix("app:") else { return }
        let appId = String(key.dropFirst(4))
        let index = getTabBar(appId).map { Int($0.selected_index) } ?? 0
        onAppPageSelected?(appId, index)
    }

    @objc private func railPinnedLxappClicked(_ sender: NSButton) {
        guard let key = sender.identifier?.rawValue,
              key.hasPrefix("pin-lxapp:") else { return }
        _ = shellOpenLxappMain(String(key.dropFirst("pin-lxapp:".count)))
    }

    @objc private func railPinnedBookmarkClicked(_ sender: NSButton) {
        guard let key = sender.identifier?.rawValue,
              key.hasPrefix("pin-bookmark:") else { return }
        let id = String(key.dropFirst("pin-bookmark:".count))
        guard let entry = pinnedBookmarkEntries.first(where: { $0.id == id }) else { return }
        if let tabId = openTabId(for: entry) {
            onBrowserTabSelected?(tabId)
        } else {
            onBookmarkOpen?(entry.url)
        }
    }

    @objc private func railBrowserClicked(_ sender: NSButton) {
        closeRailTabPopover()
        guard let key = sender.identifier?.rawValue, key.hasPrefix("web:") else { return }
        onBrowserTabSelected?(String(key.dropFirst(4)))
    }

    @objc private func railSidebarActionClicked(_ sender: NSButton) {
        closeRailTabPopover()
        guard let key = sender.identifier?.rawValue, key.hasPrefix("sidebar-action:") else { return }
        let id = String(key.dropFirst("sidebar-action:".count))
        guard let item = model.panelItems.first(where: { $0.id == id }) else { return }
        onPanelItemToggled?(item.generation, id)
    }

    private func browserContextMenu(for id: String) -> NSMenu? {
        closeRailTabPopover()
        guard let tab = model.browserTabs.first(where: { $0.id == id }) else { return nil }
        let menu = NSMenu()

        // Page actions first (Arc keeps pin/copy on the tab row itself).
        let url = tab.url.trimmingCharacters(in: .whitespacesAndNewlines)
        if BrowserPageMenu.isBookmarkActionable(url) {
            let pinnedEntry = pinnedBookmarkEntries.first {
                SidebarBookmarksSnapshot.normalize($0.url)
                    == SidebarBookmarksSnapshot.normalize(url)
            }
            let pin = NSMenuItem(
                title: L10n.string(
                    pinnedEntry == nil ? "lx_browser_pin_to_sidebar" : "lx_browser_unpin"
                ),
                action: #selector(togglePinBrowserMenuItemClicked(_:)),
                keyEquivalent: ""
            )
            pin.image = LxIcon.image(
                named: pinnedEntry == nil ? "icon_pin" : "icon_unpin",
                size: CGSize(width: 16, height: 16)
            )
            pin.target = self
            pin.representedObject = id
            menu.addItem(pin)

            let copyLink = NSMenuItem(
                title: L10n.string("lx_browser_copy_link"),
                action: #selector(copyLinkBrowserMenuItemClicked(_:)),
                keyEquivalent: ""
            )
            copyLink.image = LxIcon.image(
                named: "icon_link", size: CGSize(width: 16, height: 16))
            copyLink.target = self
            copyLink.representedObject = id
            menu.addItem(copyLink)

            menu.addItem(.separator())
        }

        let close = NSMenuItem(
            title: L10n.string("lx_common_close"),
            action: #selector(closeBrowserMenuItemClicked(_:)),
            keyEquivalent: ""
        )
        close.target = self
        close.representedObject = id
        close.image = LxIcon.image(
            named: "icon_close_x", size: CGSize(width: 16, height: 16))
        menu.addItem(close)

        if let index = model.browserTabs.firstIndex(where: { $0.id == id }) {
            if model.browserTabs.count > 1 {
                let closeOther = NSMenuItem(
                    title: L10n.string("lx_browser_close_other_tabs"),
                    action: #selector(closeOtherBrowserMenuItemClicked(_:)),
                    keyEquivalent: ""
                )
                closeOther.target = self
                closeOther.representedObject = id
                closeOther.image = LxIcon.image(
                    named: "icon_close_other_tabs", size: CGSize(width: 16, height: 16))
                menu.addItem(closeOther)
            }

            if index < model.browserTabs.index(before: model.browserTabs.endIndex) {
                let closeBelow = NSMenuItem(
                    title: L10n.string("lx_browser_close_tabs_below"),
                    action: #selector(closeTabsBelowBrowserMenuItemClicked(_:)),
                    keyEquivalent: ""
                )
                closeBelow.target = self
                closeBelow.representedObject = id
                closeBelow.image = LxIcon.image(
                    named: "icon_close_tabs_below", size: CGSize(width: 16, height: 16))
                menu.addItem(closeBelow)
            }
        }

        return menu
    }

    @objc private func closeBrowserMenuItemClicked(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        onBrowserTabCloseRequested?(id)
    }

    @objc private func togglePinBrowserMenuItemClicked(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String,
              let tab = model.browserTabs.first(where: { $0.id == id }) else { return }
        if let pinnedEntry = pinnedBookmarkEntries.first(where: {
            SidebarBookmarksSnapshot.normalize($0.url)
                == SidebarBookmarksSnapshot.normalize(tab.url)
        }) {
            _ = browserBookmarksCommand(
                #"{"op":"setPinned","id":"\#(jsonEscape(pinnedEntry.id))","pinned":false}"#
            )
        } else {
            if !browserBookmarkPin(tab.url, tab.title) {
                showShellPinLimitAlert()
            }
        }
    }

    @objc private func copyLinkBrowserMenuItemClicked(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String,
              let tab = model.browserTabs.first(where: { $0.id == id }) else { return }
        BrowserPageMenu.copyLink(tab.url, toastHost: self)
    }

    @objc private func closeOtherBrowserMenuItemClicked(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        onBrowserTabCloseOtherRequested?(id)
    }

    @objc private func closeTabsBelowBrowserMenuItemClicked(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        onBrowserTabCloseTabsBelowRequested?(id)
    }

    private func showRailTabPopover(appId: String, relativeTo button: NSButton) {
        guard isCompact, !button.isHidden, let tabBar = getTabBar(appId) else {
            closeRailTabPopover()
            return
        }
        let items = tabBar.getItems(appId: appId)
        guard !items.isEmpty else {
            closeRailTabPopover()
            return
        }

        railTabPopoverDismissTask?.cancel()
        if railTabPopoverAppId == appId, railTabPopover?.isShown == true {
            railTabPopoverButton = button
            return
        }

        closeRailTabPopover()

        let content = SidebarRailTabPopoverViewController(
            appId: appId,
            items: items,
            selectedIndex: Int(tabBar.selected_index)
        )
        content.onPageSelected = { [weak self] appId, index in
            self?.onAppPageSelected?(appId, index)
        }
        content.onDismissRequested = { [weak self] in
            self?.closeRailTabPopover()
        }
        content.onHoverChanged = { [weak self] hovering in
            guard let self else { return }
            self.isRailTabPopoverHovered = hovering
            if hovering {
                self.railTabPopoverDismissTask?.cancel()
            } else {
                self.scheduleRailTabPopoverDismiss()
            }
        }

        let popover = NSPopover()
        popover.behavior = .semitransient
        popover.animates = true
        popover.contentViewController = content
        popover.delegate = self

        railTabPopover = popover
        railTabPopoverAppId = appId
        railTabPopoverButton = button
        isRailTabPopoverHovered = false
        popover.show(relativeTo: button.bounds.insetBy(dx: -4, dy: -4), of: button, preferredEdge: .maxX)
    }

    private func scheduleRailTabPopoverDismiss() {
        railTabPopoverDismissTask?.cancel()
        railTabPopoverDismissTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 250_000_000)
            guard !Task.isCancelled, let self else { return }
            guard !self.isRailTabPopoverHovered, !self.isMouseInsideRailTabPopoverButton() else { return }
            self.closeRailTabPopover()
        }
    }

    private func isMouseInsideRailTabPopoverButton() -> Bool {
        guard let button = railTabPopoverButton, let window = button.window else { return false }
        let windowPoint = window.mouseLocationOutsideOfEventStream
        let buttonPoint = button.convert(windowPoint, from: nil)
        return button.bounds.insetBy(dx: -6, dy: -6).contains(buttonPoint)
    }

    private func closeRailTabPopover() {
        railTabPopoverDismissTask?.cancel()
        railTabPopoverDismissTask = nil
        isRailTabPopoverHovered = false
        railTabPopoverAppId = nil
        railTabPopoverButton = nil
        railTabPopover?.delegate = nil
        railTabPopover?.close()
        railTabPopover = nil
    }

    func popoverDidClose(_ notification: Notification) {
        guard notification.object as? NSPopover === railTabPopover else { return }
        railTabPopoverDismissTask?.cancel()
        railTabPopoverDismissTask = nil
        isRailTabPopoverHovered = false
        railTabPopoverAppId = nil
        railTabPopoverButton = nil
        railTabPopover = nil
    }

    func updateVisibilityState() {
        let hidden = isFullyHidden
        let compact = isCompact && !hidden && !appUIOnlyMode
        scrollView.isHidden = hidden || appUIOnlyMode || compact
        railScrollView.isHidden = hidden || appUIOnlyMode || !compact
        // Footer actions and the expand toggle retain their bottom ownership in
        // the rail instead of entering the navigation scroll region.
        railExpandButton.isHidden = hidden || appUIOnlyMode || !compact
        railFooterScrollView.isHidden = hidden
            || appUIOnlyMode
            || !compact
            || model.panelItems.isEmpty
        footerSeparator.isHidden = compact
        // Header actions and footer cells don't fit the rail.
        updateHeaderActionVisibility()
        // The header collapse toggle shows only in the expanded layout; the rail
        // carries its own expand toggle anchored at the bottom when compact.
        hideButton.isHidden = hidden || appUIOnlyMode || compact
        panelScroll.isHidden = compact
        // The compact footer always keeps the expand affordance; expanded mode
        // needs a footer only when app-owned actions exist.
        footerView.isHidden = hidden || (compact ? false : model.panelItems.isEmpty)
        resizeHandle.isHidden = hidden
    }

    func setAppUIOnlyMode(_ enabled: Bool) {
        appUIOnlyMode = enabled
        // appUIOnlyMode is enforced in one place — render() empties the list/rail
        // sections when it's on. The footer panel survives, so panelItems is kept.
        if enabled {
            model.appGroups = []
            model.browserTabs = []
            model.browserRootVisible = false
            model.selection = .none
        }
        render()
        updateVisibilityState()
    }

    /// Tear down the list/rail/browser views when entering app-UI-only mode (or
    /// whenever the model has no list content). The footer panel is rendered
    /// separately and is not affected here.
    private func teardownListSections() {
        closeRailTabPopover()
        groupViews.values.forEach { $0.removeFromSuperview() }
        groupViews.removeAll()
        groupTopConstraints.removeAll()

        isCompact = false
        railStack.arrangedSubviews.forEach {
            railStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        railButtons.removeAll()

        browserItemViews.values.forEach { $0.removeFromSuperview() }
        browserItemViews.removeAll()
        browserItemTopConstraints.removeAll()
        browserRootTopConstraint?.isActive = false
        browserRootHeader.removeFromSuperview()
        browserRootTopConstraint = nil
        pinTileViews.values.forEach { $0.removeFromSuperview() }
        pinTileViews.removeAll()
        pinTileTopConstraints.removeAll()
        pinTileLeadingConstraints.removeAll()
        addButton.removeFromSuperview()
        addButtonTopConstraint = nil

        if let docView = scrollView.documentView {
            docView.subviews.forEach { $0.removeFromSuperview() }
            docView.frame = .zero
        }
    }

    /// Replace the icon-only action buttons in the sidebar header.
    func updateHeaderActionItems(_ items: [PanelIconItem]) {
        headerActionItems = items
        headerActionIdentities.removeAll()
        headerActionStack.arrangedSubviews.forEach {
            headerActionStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        for item in items {
            let button = SidebarHeaderActionButton()
            button.translatesAutoresizingMaskIntoConstraints = false
            button.isBordered = false
            button.bezelStyle = .regularSquare
            button.imagePosition = .imageOnly
            button.imageScaling = .scaleProportionallyDown
            button.contentTintColor = LxAppHostTheme.mutedForeground
            button.image = Self.sidebarHeaderActionIcon(item.iconURL)
            button.toolTip = item.label
            button.setAccessibilityLabel(item.label)
            button.isEnabled = !item.disabled
            headerActionIdentities[ObjectIdentifier(button)] = SidebarActionIdentity(
                generation: item.generation,
                id: item.id
            )
            button.target = self
            button.action = #selector(headerActionClicked(_:))
            headerActionStack.addArrangedSubview(button)
            NSLayoutConstraint.activate([
                button.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
                button.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            ])
        }
        updateHeaderActionVisibility()
    }

    /// The constraint keeping the actions clear of the traffic lights, created
    /// once and refreshed whenever the measured clearance changes.
    private func headerActionLeadingClearance() -> NSLayoutConstraint {
        let constraint = headerActionStack.leadingAnchor.constraint(
            greaterThanOrEqualTo: headerView.leadingAnchor,
            constant: measuredHeaderLeadingReserve()
        )
        headerActionLeadingConstraint = constraint
        return constraint
    }

    /// How much of the header's leading edge the window buttons actually take.
    private func measuredHeaderLeadingReserve() -> CGFloat {
        trafficLightClearanceProvider?() ?? Layout.railWidth
    }

    /// Show the buttons the header can seat, hiding only the overflow.
    ///
    /// Measuring the whole set and hiding the stack means one action too many
    /// removes the ones that did fit, which reads as the sidebar losing its
    /// buttons rather than being one narrower than it wants. The leading
    /// reserve is the traffic lights plus the collapse toggle.
    private func updateHeaderActionVisibility() {
        let hidden = isFullyHidden || appUIOnlyMode || isCompact || headerActionItems.isEmpty
        headerActionStack.isHidden = hidden
        guard !hidden else { return }
        let reserve = measuredHeaderLeadingReserve()
        headerActionLeadingConstraint?.constant = reserve
        let availableWidth = max(0, bounds.width - reserve - 8 - Layout.actionButtonSize - 4)
        let stride = Layout.actionButtonSize + headerActionStack.spacing
        let fits = availableWidth < Layout.actionButtonSize
            ? 0
            : Int((availableWidth + headerActionStack.spacing) / stride)
        for (index, button) in headerActionStack.arrangedSubviews.enumerated() {
            button.isHidden = index >= fits
        }
        headerActionStack.isHidden = fits == 0
    }

    func updatePanelItems(_ items: [PanelIconItem]) {
        model.panelItems = items
        renderPanelItems()
        if isCompact { rebuildRail() }
        updateVisibilityState()
    }

    /// Build the footer panel buttons from `model.panelItems`. Called by render()
    /// and the `updatePanelItems` mutator. Unaffected by appUIOnlyMode.
    private func renderPanelItems() {
        let items = model.panelItems
        // Skip when the button set is unchanged so render()'s frequent calls don't
        // clobber icons resolved later via updatePanelIcon.
        guard items != renderedPanelItems else { return }
        renderedPanelItems = items

        // Remove existing panel buttons.
        panelFlow.setEntries([])
        panelButtons.removeAll()

        guard !items.isEmpty else {
            footerHeightConstraint?.constant = 0
            return
        }

        var entries: [SidebarActionRowView] = []
        for item in items {
            let row = SidebarActionRowView(
                label: item.label,
                iconURL: item.iconURL,
                active: item.active,
                disabled: item.disabled
            )
            row.onClick = { [weak self] in
                self?.onPanelItemToggled?(item.generation, item.id)
            }
            entries.append(row)
            panelButtons.append(row)
        }
        panelFlow.setEntries(entries)
        updateSidebarActionFooterHeight()
    }

    private func updateSidebarActionFooterHeight() {
        if isCompact {
            let visibleActions = min(CGFloat(model.panelItems.count), Layout.footerMaxRows)
            let actionHeight = visibleActions > 0
                ? visibleActions * Layout.railButtonSize
                    + max(0, visibleActions - 1) * railFooterStack.spacing
                    + 6
                : 0
            footerHeightConstraint?.constant = 6
                + Layout.railButtonSize
                + actionHeight
                + (visibleActions > 0 ? 6 : 0)
            return
        }
        guard !model.panelItems.isEmpty else {
            footerHeightConstraint?.constant = 0
            return
        }
        let width = max(
            1,
            panelScroll.contentView.bounds.width > 1
                ? panelScroll.contentView.bounds.width
                : bounds.width - 2 * Layout.footerHInset
        )
        let rows = min(CGFloat(panelFlow.visualRowCount(for: width)), Layout.footerMaxRows)
        let height = Layout.footerInset * 2 + 1
            + rows * Layout.footerButtonSize
            + max(0, rows - 1) * SidebarActionFlowView.gap
        if footerHeightConstraint?.constant != height {
            footerHeightConstraint?.constant = height
        }
    }

    // MARK: - Update-ready callout

    /// Show the update callout floating just above the footer dock,
    /// leading-aligned over the bottom-left icon. `.ready` → click to restart,
    /// `.available` → click to install. Idempotent — replaces any existing one.
    func presentUpdateReadyCallout(appName: String, state: UpdateCalloutState) {
        updateReadyCallout?.removeFromSuperview()

        let callout = UpdateReadyCallout(appName: appName, state: state) { [weak self] in
            self?.onUpdateActionRequested?(state)
        }
        callout.translatesAutoresizingMaskIntoConstraints = false
        addSubview(callout, positioned: .above, relativeTo: footerView)
        updateReadyCallout = callout

        NSLayoutConstraint.activate([
            callout.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.footerInset),
            callout.trailingAnchor.constraint(
                lessThanOrEqualTo: footerView.trailingAnchor, constant: -Layout.footerInset),
            callout.bottomAnchor.constraint(equalTo: footerView.topAnchor, constant: -6),
        ])
    }

    /// Remove the callout (e.g. once the update is applied or dismissed).
    func dismissUpdateReadyCallout() {
        updateReadyCallout?.removeFromSuperview()
        updateReadyCallout = nil
    }

    /// Update a panel row's icon from a file:// URL (resolved via resolveLxUri after lxapp installs).
    func updatePanelIcon(panelId: String, iconFileUrl: String) {
        guard let index = renderedPanelItems.firstIndex(where: { $0.id == panelId }),
              index < panelButtons.count,
              let url = URL(string: iconFileUrl),
              let image = NSImage(contentsOf: url) else { return }
        panelButtons[index].setIcon(image)
        if let railButton = railButtons["sidebar-action:\(panelId)"] {
            let copy = image.copy() as? NSImage ?? image
            copy.size = NSSize(width: Layout.railIconSize, height: Layout.railIconSize)
            copy.isTemplate = false
            railButton.image = copy
            railButton.contentTintColor = nil
        }
    }

    // MARK: - Public API (thin model mutators)
    //
    // Each method below updates one part of `model` and calls `render()`. They
    // hold no layout or appUIOnlyMode logic — that all lives in render().

    /// Rebuild all groups based on current tabs.
    func updateForTabs(_ tabs: [LxAppTab], activeTab: LxAppTab?) {
        lxappGroups = tabs.map {
            SidebarModel.AppGroupVM(
                appId: $0.appId,
                asideSurfaceId: $0.asideSurfaceId,
                managedLabel: nil,
                managedIcon: nil,
                contentAppId: nil,
                showsLxappTabBar: true,
                closable: true
            )
        }
        rebuildAppGroups()
        // A provided active tab updates the selection; nil leaves it untouched,
        // matching the original (which only highlighted when activeTab existed).
        if let activeAppId = activeTab?.appId {
            model.selection = .app(appId: activeAppId, pageIndex: nil)
        }
        render()
    }

    func updateManagedMainItems(_ items: [LxAppUIActionItem], activeId: String?) {
        managedMainGroups = items.map { item in
            let fallbackIcon: NSImage?
            switch item.builtInIcon {
            case "terminal":
                fallbackIcon = NSImage(
                    systemSymbolName: "terminal",
                    accessibilityDescription: item.label
                )
            case "browser":
                fallbackIcon = Self.designIcon("icon_globe")
            default:
                fallbackIcon = nil
            }
            return SidebarModel.AppGroupVM(
                appId: item.id,
                asideSurfaceId: nil,
                managedLabel: item.label,
                managedIcon: item.iconURL.flatMap(NSImage.init(contentsOf:)) ?? fallbackIcon,
                contentAppId: item.contentAppId,
                showsLxappTabBar: item.showsLxappTabBar,
                closable: item.closable
            )
        }
        rebuildAppGroups()
        if let activeId {
            model.selection = .app(appId: activeId, pageIndex: nil)
        }
        render()
    }

    func beginManagedMainRename(surfaceId: String) {
        groupViews[surfaceId]?.beginManagedRename()
    }

    private func rebuildAppGroups() {
        let liveLxappIds = Set(lxappGroups.map(\.appId))
        model.appGroups = lxappGroups
            + managedMainGroups.filter { !liveLxappIds.contains($0.appId) }
    }

    /// Refresh a specific app group from Rust data
    func refreshAppGroup(appId: String) {
        guard !appUIOnlyMode else { return }
        groupViews[appId]?.refreshFromRust()
    }

    /// Set active highlight on the appropriate group and item.
    func setActiveHighlight(appId: String, pageIndex: Int? = nil) {
        model.selection = .app(appId: appId, pageIndex: pageIndex)
        render()
    }

    /// Clear all highlights (both lxapp and browser)
    func clearAllHighlights() {
        model.selection = .none
        render()
    }

    // MARK: - Browser Items

    func setBrowserRootVisible(_ visible: Bool) {
        model.browserRootVisible = visible
        render()
    }

    /// Update browser tab items in the sidebar
    func updateBrowserItems(_ items: [(id: String, title: String, url: String, favicon: NSImage?)], activeId: String?) {
        model.browserTabs = items.map {
            SidebarModel.BrowserTabVM(id: $0.id, title: $0.title, url: $0.url, favicon: $0.favicon)
        }
        if let activeId {
            model.selection = .browser(id: activeId)
        } else if case .browser = model.selection {
            // No active browser tab and we were on one — drop the selection so the
            // list/rail render unselected (the original cleared item selection
            // whenever activeId was nil).
            model.selection = .none
        }
        render()
    }

    // MARK: - Render (single model-driven entry point)

    /// The ONE place that turns `model` into views. appUIOnlyMode is checked here
    /// and nowhere else: when on, the list/rail/browser sections are emptied while
    /// the footer panel still renders. Otherwise it diffs the app groups + browser
    /// section, applies `model.selection`, and refreshes the rail when compact.
    ///
    /// render() delegates to per-section helpers (`renderAppGroups`,
    /// `layoutBrowserSection`, `applySelection`, `renderPanelItems`); keeping the
    /// existing constraint/animation code intact rather than rebuilding it.
    private func render() {
        // Footer panel is independent of appUIOnlyMode.
        renderPanelItems()

        guard !appUIOnlyMode else {
            teardownListSections()
            updateVisibilityState()
            return
        }

        guard let docView = scrollView.documentView else { return }

        renderBrowserItems()
        renderPinTiles()
        let pinY = layoutPinGrid(in: docView, yOffset: 6)
        let yOffset = renderAppGroups(in: docView, startY: pinY)
        let finalY = layoutBrowserSection(in: docView, yOffset: yOffset)
        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: finalY)

        applySelection()

        if isCompact { rebuildRail() }
    }

    /// Diff app group views against `model.appGroups`, position them, and return
    /// the Y offset where the browser section begins.
    private func renderAppGroups(in docView: NSView, startY: CGFloat = 4) -> CGFloat {
        // Remove groups for apps no longer present.
        let currentAppIds = Set(model.appGroups.map { $0.appId })
        for (appId, groupView) in groupViews where !currentAppIds.contains(appId) {
            groupView.removeFromSuperview()
            groupViews.removeValue(forKey: appId)
            groupTopConstraints.removeValue(forKey: appId)
        }

        // Add/update groups.
        var yOffset: CGFloat = startY
        for (index, group) in model.appGroups.enumerated() {
            let appId = group.appId
            let groupView: SidebarGroupView
            if let existing = groupViews[appId],
               existing.isManagedMain == group.isManagedMain,
               existing.showsLxappTabBar == group.showsLxappTabBar,
               existing.contentAppId == group.contentAppId,
               existing.closable == group.closable {
                groupView = existing
            } else {
                if let existing = groupViews.removeValue(forKey: appId) {
                    existing.removeFromSuperview()
                    groupTopConstraints.removeValue(forKey: appId)
                }
                groupView = SidebarGroupView(
                    appId: appId,
                    managedLabel: group.managedLabel,
                    managedIcon: group.managedIcon,
                    contentAppId: group.contentAppId,
                    showsLxappTabBar: group.showsLxappTabBar,
                    closable: group.closable
                )
                groupView.onPageSelected = { [weak self] appId, itemIndex in
                    self?.onAppPageSelected?(appId, itemIndex)
                }
                groupView.onAppSelected = { [weak self] appId in
                    self?.onAppSelected?(appId)
                }
                groupView.onCloseRequested = { [weak self] appId in
                    self?.onAppCloseRequested?(appId)
                }
                groupView.onManagedContextMenuRequested = { [weak self] surfaceId, event, view in
                    self?.onManagedMainContextMenuRequested?(surfaceId, event, view)
                }
                groupView.onManagedRenameCommitted = { [weak self] surfaceId, title in
                    self?.onManagedMainRenameCommitted?(surfaceId, title)
                }
                groupView.onLayoutChanged = { [weak self] in
                    self?.relayoutAfterGroupToggle()
                }
                groupView.onPinChanged = { [weak self] in
                    self?.reloadBookmarks()
                }
                groupViews[appId] = groupView
            }

            groupView.setColorIndex(index)
            if group.isManagedMain {
                groupView.updateManagedPresentation(
                    label: group.managedLabel,
                    icon: group.managedIcon
                )
            }

            if groupView.superview !== docView {
                groupView.removeFromSuperview()
                docView.addSubview(groupView)
                NSLayoutConstraint.activate([
                    groupView.leadingAnchor.constraint(equalTo: docView.leadingAnchor),
                    groupView.trailingAnchor.constraint(equalTo: docView.trailingAnchor),
                ])
            }

            if let tc = groupTopConstraints[appId] {
                tc.constant = yOffset
            } else {
                let tc = groupView.topAnchor.constraint(equalTo: docView.topAnchor, constant: yOffset)
                tc.isActive = true
                groupTopConstraints[appId] = tc
            }

            groupView.layoutSubtreeIfNeeded()
            yOffset += groupView.fittingSize.height + 4
        }
        return yOffset
    }

    /// Apply `model.selection` to the group views, browser item views, and rail.
    /// This is the single place selection truth is turned into highlight state.
    private func applySelection() {
        // Browser item selection.
        for (id, itemView) in browserItemViews {
            if case .browser(let activeId) = model.selection {
                itemView.isSelected = (id == activeId)
            } else {
                itemView.isSelected = false
            }
        }
        browserRootHeader.layer?.backgroundColor = {
            if case .browser = model.selection {
                return LxAppHostTheme.selectionBackground.cgColor
            }
            return NSColor.clear.cgColor
        }()

        // App group selection.
        for (id, group) in groupViews {
            if case .app(let appId, let pageIndex) = model.selection, id == appId {
                group.isActiveGroup = true
                if let idx = pageIndex {
                    group.setActiveHighlight(pageIndex: idx)
                } else if let tabBar = getTabBar(appId) {
                    group.setActiveHighlight(pageIndex: Int(tabBar.selected_index))
                }
            } else {
                group.isActiveGroup = false
                group.clearHighlight()
            }
        }

        refreshRailHighlight()
    }

    /// Whether a browser tab id is the current selection (used to configure new
    /// item views; final highlight state is reasserted by applySelection()).
    private func isBrowserSelected(_ id: String) -> Bool {
        if case .browser(let activeId) = model.selection { return id == activeId }
        return false
    }

    /// Diff `model.browserTabs` into `browserItemViews`: remove dropped tabs,
    /// create new item views (wiring their click/close callbacks), and configure
    /// existing ones with the latest title/favicon. Positioning happens in
    /// layoutBrowserSection; selection state is finalized in applySelection.
    private func renderBrowserItems() {
        let currentIds = Set(model.browserTabs.map { $0.id })
        for (id, itemView) in browserItemViews where !currentIds.contains(id) {
            if let topConstraint = browserItemTopConstraints[id] {
                topConstraint.isActive = false
                browserItemTopConstraints.removeValue(forKey: id)
            }
            itemView.removeFromSuperview()
            browserItemViews.removeValue(forKey: id)
        }

        for item in model.browserTabs {
            let selected = isBrowserSelected(item.id)
            if let existing = browserItemViews[item.id] {
                existing.configure(title: item.title, url: item.url, isSelected: selected, favicon: item.favicon)
            } else {
                let itemView = SidebarBrowserItemView(id: item.id)
                itemView.translatesAutoresizingMaskIntoConstraints = false
                itemView.onClick = { [weak self] id in
                    self?.onBrowserTabSelected?(id)
                }
                itemView.onClose = { [weak self] id in
                    self?.onBrowserTabCloseRequested?(id)
                }
                itemView.contextMenuProvider = { [weak self] id in
                    self?.browserContextMenu(for: id)
                }
                itemView.configure(title: item.title, url: item.url, isSelected: selected, favicon: item.favicon)
                browserItemViews[item.id] = itemView
            }
        }
    }

    private func browserTooltip(title: String, url: String) -> String {
        let resolvedTitle = title.isEmpty ? L10n.string("lx_browser_new_tab") : title
        let trimmedURL = url.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmedURL.isEmpty ? resolvedTitle : "\(resolvedTitle)\n\(trimmedURL)"
    }

    /// Layout browser items and add button after lxapp groups
    private func layoutBrowserSection(in docView: NSView, yOffset startY: CGFloat) -> CGFloat {
        let groupInset: CGFloat = SidebarGroupView.Layout.groupInset
        var yOffset = startY

        if model.browserRootVisible {
            ensureSubview(browserRootHeader, in: docView) {
                setupBrowserRootHeader()
                NSLayoutConstraint.activate([
                    browserRootHeader.leadingAnchor.constraint(
                        equalTo: docView.leadingAnchor,
                        constant: groupInset
                    ),
                    browserRootHeader.trailingAnchor.constraint(
                        equalTo: docView.trailingAnchor,
                        constant: -groupInset
                    ),
                    browserRootHeader.heightAnchor.constraint(
                        equalToConstant: SidebarGroupView.Layout.headerHeight
                    ),
                ])
            }
            updateOrCreate(
                &browserRootTopConstraint,
                on: browserRootHeader,
                in: docView,
                constant: yOffset
            )
            yOffset += SidebarGroupView.Layout.headerHeight + 4
        } else {
            browserRootTopConstraint?.isActive = false
            browserRootHeader.removeFromSuperview()
            browserRootTopConstraint = nil
        }

        // Browser item views remain visible independently of pinned shortcuts.
        for tab in model.browserTabs {
            let tabId = tab.id
            guard let itemView = browserItemViews[tabId] else { continue }
            ensureSubview(itemView, in: docView) {
                NSLayoutConstraint.activate([
                    itemView.leadingAnchor.constraint(equalTo: docView.leadingAnchor, constant: groupInset),
                    itemView.trailingAnchor.constraint(equalTo: docView.trailingAnchor, constant: -groupInset),
                ])
            }

            if let tc = browserItemTopConstraints[tabId] {
                tc.constant = yOffset
            } else {
                let tc = itemView.topAnchor.constraint(equalTo: docView.topAnchor, constant: yOffset)
                tc.isActive = true
                browserItemTopConstraints[tabId] = tc
            }
            yOffset += SidebarBrowserItemView.Layout.height + 4
        }

        // "+" button — only shown when the browser capability is available
        if (LxAppCore.capabilities & LxAppCore.capBrowser) != 0 || model.browserRootVisible {
            ensureSubview(addButton, in: docView) {
                setupAddButton()
                NSLayoutConstraint.activate([
                    addButton.leadingAnchor.constraint(equalTo: docView.leadingAnchor, constant: groupInset),
                    addButton.trailingAnchor.constraint(equalTo: docView.trailingAnchor, constant: -groupInset),
                    addButton.heightAnchor.constraint(equalToConstant: 28),
                ])
            }
            updateOrCreate(&addButtonTopConstraint, on: addButton, in: docView, constant: yOffset)
            yOffset += 28 + 8
        } else {
            addButton.removeFromSuperview()
            addButtonTopConstraint = nil
        }

        return yOffset
    }

    private func setupBrowserRootHeader() {
        guard !browserRootHeaderConfigured else { return }
        browserRootHeaderConfigured = true
        browserRootHeader.translatesAutoresizingMaskIntoConstraints = false
        browserRootHeader.wantsLayer = true
        browserRootHeader.layer?.cornerRadius = SidebarGroupView.Layout.headerCornerRadius

        let icon = NSImageView()
        icon.translatesAutoresizingMaskIntoConstraints = false
        icon.image = NSApp.applicationIconImage
            ?? Self.designIcon("icon_globe")
            ?? NSImage(systemSymbolName: "globe", accessibilityDescription: nil)
        icon.imageScaling = .scaleProportionallyDown
        browserRootHeader.addSubview(icon)

        let label = NSTextField(labelWithString: L10n.string("lx_browser_label").uppercased())
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        label.textColor = LxAppHostTheme.foreground
        browserRootHeader.addSubview(label)

        NSLayoutConstraint.activate([
            icon.leadingAnchor.constraint(
                equalTo: browserRootHeader.leadingAnchor,
                constant: SidebarGroupView.Layout.headerHPadding
            ),
            icon.centerYAnchor.constraint(equalTo: browserRootHeader.centerYAnchor),
            icon.widthAnchor.constraint(equalToConstant: 16),
            icon.heightAnchor.constraint(equalToConstant: 16),
            label.leadingAnchor.constraint(equalTo: icon.trailingAnchor, constant: 6),
            label.trailingAnchor.constraint(
                lessThanOrEqualTo: browserRootHeader.trailingAnchor,
                constant: -SidebarGroupView.Layout.headerHPadding
            ),
            label.centerYAnchor.constraint(equalTo: browserRootHeader.centerYAnchor),
        ])
    }

    /// Ensure a view is a subview of parent; run setup closure only on first add
    private func ensureSubview(_ view: NSView, in parent: NSView, setup: () -> Void) {
        if view.superview !== parent {
            view.removeFromSuperview()
            parent.addSubview(view)
            setup()
        }
    }

    /// Update an existing top constraint's constant, or create one
    private func updateOrCreate(_ constraint: inout NSLayoutConstraint?, on view: NSView, in parent: NSView, constant: CGFloat) {
        if let c = constraint {
            c.constant = constant
        } else {
            let c = view.topAnchor.constraint(equalTo: parent.topAnchor, constant: constant)
            c.isActive = true
            constraint = c
        }
    }

    /// Re-layout after a group expands/collapses — repositions groups + browser section
    private func relayoutAfterGroupToggle() {
        guard let docView = scrollView.documentView else { return }

        // Reposition all groups below pinned website shortcuts.
        var yOffset = layoutPinGrid(in: docView, yOffset: 6)
        for group in model.appGroups {
            guard let groupView = groupViews[group.appId] else { continue }
            groupTopConstraints[group.appId]?.constant = yOffset
            groupView.layoutSubtreeIfNeeded()
            yOffset += groupView.fittingSize.height + 4
        }

        // Re-layout browser section below groups
        yOffset = layoutBrowserSection(in: docView, yOffset: yOffset)

        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: yOffset)
    }

    // MARK: - Bookmarks section

    /// Re-read the bookmarks store (host FFI) and re-render. Called at setup
    /// and whenever the store changes (star toggle, tile action, manager
    /// page edit — routed through `LxApp.browserBookmarksChanged`).
    func reloadBookmarks() {
        let json = shellPins().toString()
        shellPinItems = (try? JSONDecoder().decode([ShellPinItem].self, from: Data(json.utf8))) ?? []
        guard (LxAppCore.capabilities & LxAppCore.capBrowser) != 0 else {
            render()
            return
        }
        bookmarksSnapshot = SidebarBookmarksSnapshot.loadFromHost()
        render()
    }

    func updateShellPins(_ json: String) {
        let pins = (try? JSONDecoder().decode([ShellPinItem].self, from: Data(json.utf8))) ?? []
        guard pins != shellPinItems else { return }
        shellPinItems = pins
        render()
    }

    /// Diff pin tiles against the snapshot's pinned subset.
    private func renderPinTiles() {
        // lxapp tiles first (they lead the grid), then web pins.
        let lxappIds = Set(pinnedLxappIds)
        for (id, view) in lxappPinTiles where !lxappIds.contains(id) {
            pinTileTopConstraints.removeValue(forKey: "lxapp:" + id)?.isActive = false
            pinTileLeadingConstraints.removeValue(forKey: "lxapp:" + id)?.isActive = false
            view.removeFromSuperview()
            lxappPinTiles.removeValue(forKey: id)
        }
        for id in pinnedLxappIds where lxappPinTiles[id] == nil {
            let tile = LxappPinTileView(appId: id)
            lxappPinTiles[id] = tile
        }

        let pinned = pinnedBookmarkEntries
        let pinnedIds = Set(pinned.map { $0.id })
        for (id, view) in pinTileViews where !pinnedIds.contains(id) {
            pinTileTopConstraints.removeValue(forKey: id)?.isActive = false
            pinTileLeadingConstraints.removeValue(forKey: id)?.isActive = false
            view.removeFromSuperview()
            pinTileViews.removeValue(forKey: id)
        }

        for entry in pinned {
            let tile = pinTileViews[entry.id] ?? {
                let view = SidebarPinTileView(bookmarkId: entry.id)
                view.translatesAutoresizingMaskIntoConstraints = false
                view.onOpen = { [weak self] url in
                    self?.onBookmarkOpen?(url)
                }
                view.onSelectTab = { [weak self] tabId in
                    self?.onBrowserTabSelected?(tabId)
                }
                view.onManageBookmarks = { [weak self] in
                    self?.onManageBookmarks?()
                }
                pinTileViews[entry.id] = view
                return view
            }()
            tile.configure(url: entry.url, title: entry.title)
            let openTabId = openTabId(for: entry)
            tile.openTabId = openTabId
            tile.isFocused = openTabId.map { isBrowserSelected($0) } ?? false
            if let openTabId,
               let tabIndex = model.browserTabs.firstIndex(where: { $0.id == openTabId }) {
                tile.onCloseTab = { [weak self] in
                    self?.onBrowserTabCloseRequested?(openTabId)
                }
                tile.onCloseOtherTabs = model.browserTabs.count > 1 ? { [weak self] in
                    self?.onBrowserTabCloseOtherRequested?(openTabId)
                } : nil
                tile.onCloseTabsBelow =
                    tabIndex < model.browserTabs.index(before: model.browserTabs.endIndex)
                    ? { [weak self] in
                        self?.onBrowserTabCloseTabsBelowRequested?(openTabId)
                    }
                    : nil
            } else {
                tile.onCloseTab = nil
                tile.onCloseOtherTabs = nil
                tile.onCloseTabsBelow = nil
            }
            tile.syncState()
        }
    }

    /// Lay out the pin grid at the very top of the list.
    private func layoutPinGrid(in docView: NSView, yOffset startY: CGFloat) -> CGFloat {
        let cells: [(String, NSView)] = shellPinItems.compactMap { pin in
            if pin.kind == "lxapp", let tile = lxappPinTiles[pin.key] {
                return ("lxapp:" + pin.key, tile)
            }
            if pin.kind == "bookmark", let tile = pinTileViews[pin.key] {
                return (pin.key, tile)
            }
            return nil
        }
        guard !cells.isEmpty else { return startY }
        let size = SidebarPinTileView.Layout.size
        let gap = SidebarPinTileView.Layout.gap
        let columns = SidebarPinTileView.Layout.columns
        // Leading-aligned on the shared icon axis (not centered): the first
        // tile's icon lines up with the group headers and traffic lights, and
        // the grid no longer drifts when the sidebar is resized.
        let gridLeft = Layout.iconAxis - size / 2
        var yOffset = startY

        for (index, cell) in cells.enumerated() {
            let (key, tile) = cell
            let column = index % columns
            let row = index / columns
            let x = gridLeft + CGFloat(column) * (size + gap)
            let y = startY + CGFloat(row) * (size + gap)
            ensureSubview(tile, in: docView) {}
            if let tc = pinTileTopConstraints[key] {
                tc.constant = y
            } else {
                let tc = tile.topAnchor.constraint(equalTo: docView.topAnchor, constant: y)
                tc.isActive = true
                pinTileTopConstraints[key] = tc
            }
            if let lc = pinTileLeadingConstraints[key] {
                lc.constant = x
            } else {
                let lc = tile.leadingAnchor.constraint(equalTo: docView.leadingAnchor, constant: x)
                lc.isActive = true
                pinTileLeadingConstraints[key] = lc
            }
            yOffset = y + size
        }

        return yOffset + 10
    }

    private func setupAddButton() {
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addButton.title = ""
        addButton.image = LxIcon.image(
            named: "icon_browser_plus", size: CGSize(width: 16, height: 16))
            ?? NSImage(systemSymbolName: "plus", accessibilityDescription: nil)
        addButton.toolTip = L10n.string("lx_browser_new_tab")
        addButton.setAccessibilityElement(true)
        addButton.setAccessibilityRole(.button)
        addButton.setAccessibilityLabel(L10n.string("lx_browser_new_tab"))
        addButton.isBordered = false
        addButton.bezelStyle = .regularSquare
        addButton.imagePosition = .imageOnly
        addButton.contentTintColor = LxAppHostTheme.mutedForeground
        addButton.wantsLayer = true
        addButton.layer?.cornerRadius = 6
        addButton.layer?.backgroundColor = LxAppHostTheme.foreground.withAlphaComponent(0.06).cgColor
        addButton.target = self
        addButton.action = #selector(addButtonClicked)
    }

    @objc private func addButtonClicked() {
        onAddRequested?()
    }

    @objc private func hideButtonClicked() {
        onHideRequested?()
    }

    @objc private func headerActionClicked(_ sender: NSButton) {
        guard let identity = headerActionIdentities[ObjectIdentifier(sender)] else { return }
        onPanelItemToggled?(identity.generation, identity.id)
    }

    @objc private func railExpandClicked() {
        onShowRequested?()
    }

    // MARK: - Footer / Add button hover

    override public func updateTrackingAreas() {
        super.updateTrackingAreas()

        // Add button hover tracking
        if let existing = addButtonTrackingArea {
            addButton.removeTrackingArea(existing)
        }
        let addArea = NSTrackingArea(
            rect: addButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addButton.addTrackingArea(addArea)
        addButtonTrackingArea = addArea

        // Hide button hover tracking
        if let existing = hideButtonTrackingArea {
            hideButton.removeTrackingArea(existing)
        }
        let hideArea = NSTrackingArea(
            rect: hideButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        hideButton.addTrackingArea(hideArea)
        hideButtonTrackingArea = hideArea
    }

    override public func mouseEntered(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(true)
        } else if event.trackingArea === hideButtonTrackingArea {
            setHideButtonHovered(true)
        }
    }

    override public func mouseExited(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(false)
        } else if event.trackingArea === hideButtonTrackingArea {
            setHideButtonHovered(false)
        }
    }

    private func setAddButtonHovered(_ hovered: Bool) {
        isAddButtonHovered = hovered
        updateAddButtonAppearance()
    }

    private func updateAddButtonAppearance() {
        let alpha: CGFloat = isAddButtonHovered ? 0.12 : 0.06
        addButton.layer?.backgroundColor = LxAppHostTheme.foreground.withAlphaComponent(alpha).cgColor
    }

    private func setHideButtonHovered(_ hovered: Bool) {
        isHideButtonHovered = hovered
        updateHideButtonAppearance()
    }

    private func updateHideButtonAppearance() {
        hideButton.layer?.backgroundColor = isHideButtonHovered
            ? LxAppHostTheme.foreground.withAlphaComponent(0.09).cgColor
            : NSColor.clear.cgColor
        hideButton.contentTintColor = isHideButtonHovered
            ? LxAppHostTheme.foreground
            : LxAppHostTheme.mutedForeground
    }
}

/// NSView subclass with flipped coordinate system (top-left origin)
@MainActor
private class FlippedView: NSView {
    nonisolated override var isFlipped: Bool { true }
    override var mouseDownCanMoveWindow: Bool { false }
}

/// Top-aligned scroll document host (NSClipView content is bottom-anchored
/// in non-flipped coordinates).
@MainActor
private final class FlippedClipView: NSView {
    nonisolated override var isFlipped: Bool { true }
}

/// Wraps whole icon+title cells while keeping each title single-line.
@MainActor
private final class SidebarActionFlowView: NSView {
    static let gap: CGFloat = 4
    private static let rowHeight: CGFloat = 30
    private static let minimumCellWidth: CGFloat = 72

    private var entries: [SidebarActionRowView] = []
    private var lastLayoutWidth: CGFloat = 0

    nonisolated override var isFlipped: Bool { true }

    func setEntries(_ entries: [SidebarActionRowView]) {
        self.entries.forEach { $0.removeFromSuperview() }
        self.entries = entries
        self.entries.forEach { entry in
            entry.translatesAutoresizingMaskIntoConstraints = true
            addSubview(entry)
        }
        invalidateIntrinsicContentSize()
        needsLayout = true
    }

    func visualRowCount(for width: CGFloat) -> Int {
        rowRanges(for: width).count
    }

    override var intrinsicContentSize: NSSize {
        let width = bounds.width > 1 ? bounds.width : max(lastLayoutWidth, 200)
        let rows = visualRowCount(for: width)
        let height = CGFloat(rows) * Self.rowHeight
            + CGFloat(max(0, rows - 1)) * Self.gap
        return NSSize(width: NSView.noIntrinsicMetric, height: height)
    }

    override func layout() {
        super.layout()
        let width = max(1, bounds.width)
        if abs(width - lastLayoutWidth) > 0.5 {
            lastLayoutWidth = width
            invalidateIntrinsicContentSize()
        }
        var y: CGFloat = 0
        for range in rowRanges(for: width) {
            let row = Array(entries[range])
            let preferred = row.map {
                min(width, max(Self.minimumCellWidth, $0.preferredCellWidth))
            }
            let widths = fittedCellWidths(preferred, available: width)
            var x: CGFloat = 0
            for index in row.indices {
                let isLast = index == row.count - 1
                let cellWidth = isLast ? width - x : widths[index]
                row[index].frame = NSRect(
                    x: x,
                    y: y,
                    width: max(1, cellWidth),
                    height: Self.rowHeight
                )
                x += cellWidth + Self.gap
            }
            y += Self.rowHeight + Self.gap
        }
    }

    private func rowRanges(for width: CGFloat) -> [Range<Int>] {
        guard !entries.isEmpty else { return [] }
        let available = max(1, width)
        var rows: [Range<Int>] = []
        var start = 0
        var used: CGFloat = 0
        for index in entries.indices {
            let minimum = min(available, Self.minimumCellWidth)
            let next = index == start ? minimum : used + Self.gap + minimum
            if index > start && next > available {
                rows.append(start..<index)
                start = index
                used = minimum
            } else {
                used = next
            }
        }
        rows.append(start..<entries.count)
        return rows
    }

    private func fittedCellWidths(_ preferred: [CGFloat], available: CGFloat) -> [CGFloat] {
        guard !preferred.isEmpty else { return [] }
        let count = CGFloat(preferred.count)
        let target = max(count, available - CGFloat(preferred.count - 1) * Self.gap)
        let minimum = max(1, min(Self.minimumCellWidth, target / count))
        var widths = preferred.map { min(target, max(minimum, $0)) }
        var delta = target - widths.reduce(0, +)

        while delta < -0.5 {
            let shrinkable = widths.indices.filter { widths[$0] > minimum + 0.5 }
            guard !shrinkable.isEmpty else { break }
            let share = -delta / CGFloat(shrinkable.count)
            for index in shrinkable {
                let shrink = min(widths[index] - minimum, share, -delta)
                widths[index] -= shrink
                delta += shrink
                if delta >= -0.5 { break }
            }
        }

        if delta > 0 {
            let share = delta / count
            widths = widths.map { $0 + share }
        }
        return widths
    }
}

@MainActor
private enum SidebarActionChromePalette {
    static var activeSurface: NSColor {
        LxAppHostTheme.selectionBackground
    }

    static var mutedText: NSColor {
        LxAppHostTheme.mutedForeground
    }

    static var divider: NSColor {
        LxAppHostTheme.separator
    }

    static let hover = NSColor(name: nil) { appearance in
        isDark(appearance)
            ? NSColor.white.withAlphaComponent(0.10)
            : NSColor.black.withAlphaComponent(0.06)
    }

    static let pressed = NSColor(name: nil) { appearance in
        isDark(appearance)
            ? NSColor.white.withAlphaComponent(0.16)
            : NSColor.black.withAlphaComponent(0.10)
    }

    private static func isDark(_ appearance: NSAppearance) -> Bool {
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }
}

/// One sidebar action entry: a left-aligned icon + title row sharing the tabbar
/// items' rhythm (30pt, hover wash). A custom view because a borderless
/// NSButton centers its image+title block and cannot left-align it.
@MainActor
final class SidebarActionRowView: NSView {
    var onClick: (() -> Void)?

    private let iconView = NSImageView()
    private let titleLabel: NSTextField
    private let active: Bool
    private let disabled: Bool
    private var isHovered = false { didSet { updateAppearance() } }
    private var isPressed = false { didSet { updateAppearance() } }
    private var tracking: NSTrackingArea?

    private let washView = NSView()
    private let accentView = NSView()

    var preferredCellWidth: CGFloat {
        8 + 16 + 8 + titleLabel.intrinsicContentSize.width + 8
    }

    init(label: String, iconURL: URL?, active: Bool, disabled: Bool) {
        titleLabel = NSTextField(labelWithString: label)
        self.active = active && !disabled
        self.disabled = disabled
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        toolTip = label
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel(label)

        // sidebar action entries are TOP-LEVEL rows: hover rect and icon axis copy
        // the web-tab rows (full-row wash, radius 6, icon inset 8), not the
        // nested tabbar items' deeper card.
        washView.translatesAutoresizingMaskIntoConstraints = false
        washView.wantsLayer = true
        washView.layer?.cornerRadius = 6
        addSubview(washView)

        accentView.translatesAutoresizingMaskIntoConstraints = false
        accentView.wantsLayer = true
        accentView.layer?.cornerRadius = 1
        accentView.layer?.backgroundColor = LxAppHostTheme.accent.cgColor
        accentView.isHidden = !self.active
        addSubview(accentView)

        let icon = iconURL.flatMap { NSImage(contentsOf: $0) }
        icon?.size = NSSize(width: 16, height: 16)
        iconView.image = icon
        iconView.imageScaling = .scaleProportionallyDown
        iconView.alphaValue = disabled ? 0.42 : (self.active ? 1 : 0.82)
        iconView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(iconView)

        titleLabel.font = NSFont.systemFont(
            ofSize: 13,
            weight: self.active ? .medium : .regular
        )
        titleLabel.textColor = disabled
            ? LxAppHostTheme.mutedForeground
            : (self.active ? LxAppHostTheme.accent : SidebarActionChromePalette.mutedText)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(titleLabel)

        NSLayoutConstraint.activate([
            washView.leadingAnchor.constraint(equalTo: leadingAnchor),
            washView.trailingAnchor.constraint(equalTo: trailingAnchor),
            washView.topAnchor.constraint(equalTo: topAnchor),
            washView.bottomAnchor.constraint(equalTo: bottomAnchor),
            accentView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            accentView.centerYAnchor.constraint(equalTo: centerYAnchor),
            accentView.widthAnchor.constraint(equalToConstant: 2),
            accentView.heightAnchor.constraint(equalToConstant: 18),
            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 16),
            iconView.heightAnchor.constraint(equalToConstant: 16),
            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: 8),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: washView.trailingAnchor, constant: -8),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        setAccessibilityEnabled(!disabled)
        updateAppearance()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func setIcon(_ image: NSImage) {
        image.size = NSSize(width: 16, height: 16)
        iconView.image = image
    }

    private func updateAppearance() {
        accentView.layer?.backgroundColor = LxAppHostTheme.accent.cgColor
        titleLabel.textColor = disabled
            ? LxAppHostTheme.mutedForeground
            : (active ? LxAppHostTheme.accent : SidebarActionChromePalette.mutedText)
        if isPressed && !disabled {
            washView.layer?.backgroundColor = SidebarActionChromePalette.pressed.cgColor
        } else if active {
            washView.layer?.backgroundColor = SidebarActionChromePalette.activeSurface.cgColor
        } else if isHovered && !disabled {
            washView.layer?.backgroundColor = SidebarActionChromePalette.hover.cgColor
        } else {
            washView.layer?.backgroundColor = NSColor.clear.cgColor
        }
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateAppearance()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) { if !disabled { isHovered = true } }
    override func mouseExited(with event: NSEvent) {
        isHovered = false
        isPressed = false
    }
    override func mouseDown(with event: NSEvent) {
        if !disabled { isPressed = true }
    }
    override func mouseDragged(with event: NSEvent) {
        guard !disabled else { return }
        isPressed = bounds.contains(convert(event.locationInWindow, from: nil))
    }
    override func mouseUp(with event: NSEvent) {
        guard !disabled else { return }
        let shouldActivate = isPressed
            && bounds.contains(convert(event.locationInWindow, from: nil))
        isPressed = false
        if shouldActivate { onClick?() }
    }
    override var mouseDownCanMoveWindow: Bool { false }
    override func accessibilityPerformPress() -> Bool {
        guard !disabled else { return false }
        onClick?()
        return true
    }
}

#endif
