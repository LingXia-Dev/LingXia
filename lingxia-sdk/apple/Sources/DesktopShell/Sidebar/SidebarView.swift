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

    override func rightMouseDown(with event: NSEvent) {
        if let onContextMenuRequested {
            onContextMenuRequested(event, self)
        } else {
            super.rightMouseDown(with: event)
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
    /// Optional title color (writer-configurable); nil = secondary label.
    var labelColor: NSColor?
    let id: String
    let iconURL: URL?
    let label: String
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
    var panelItems: [PanelIconItem] = []
    var selection: Selection = .none
}

extension PanelIconItem: Equatable {
    static func == (lhs: PanelIconItem, rhs: PanelIconItem) -> Bool {
        lhs.id == rhs.id && lhs.iconURL == rhs.iconURL && lhs.label == rhs.label
    }
}

// MARK: - SidebarView

/// The main sidebar container view, modeled after Chrome vertical tab groups.
/// Supports drag-to-resize and a fully hidden state.
@MainActor
class SidebarView: NSView, NSPopoverDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "Sidebar")

    struct Layout {
        static let expandedWidth: CGFloat = 180
        static let maxWidth: CGFloat = 400
        static let fullyHiddenThreshold: CGFloat = 1
        /// Minimum width of the collapsed icon-only rail. The effective width
        /// grows to clear the macOS traffic lights when they're wider (see
        /// `effectiveRailWidth`).
        static let railWidth: CGFloat = 60
        /// Drag-end below this snaps to fully hidden (0).
        static let railHideThreshold: CGFloat = 32
        /// Drag-end below this (but at/above `railHideThreshold`) snaps to the icon rail;
        /// at/above it the sidebar expands.
        static let railExpandThreshold: CGFloat = 128
        /// Square icon button in the rail.
        static let railButtonSize: CGFloat = 34
        /// Rendered icon size inside a rail button.
        static let railIconSize: CGFloat = 22
        // Reserve only the shared traffic-light / toolbar row; the titlebar offset is
        // already handled by `buttonCenterYFromTop`.
        static let trafficLightsHeight: CGFloat = 38
        static let actionButtonSize: CGFloat = 28
        static let resizeHandleWidth: CGFloat = 5
        /// Bottom dock height — tall enough for one row of icon buttons plus breathing room.
        static let footerHeight: CGFloat = 48
        /// Activator row height — matches the tabbar item rhythm above.
        static let footerButtonSize: CGFloat = 30
        /// Rendered glyph size inside footer icon buttons.
        static let footerIconSize: CGFloat = 16
        /// Vertical padding inside the dock.
        static let footerInset: CGFloat = 6
        /// Activator rows span the same outer extents as the tabbar item
        /// rows (group inset), so their hover rect and icon axis line up.
        static let footerLeading: CGFloat = 8
        /// Rows shown before the activator area caps and scrolls internally.
        static let footerMaxRows: CGFloat = 5
    }

    private let headerView = NSView()
    private let settingsButton = NSButton()
    private let downloadButton = NSButton()
    private let scrollView = SidebarScrollView()
    private let resizeHandle = SidebarResizeHandle()
    private let footerView = NSView()
    private let footerSeparator = NSView()
    /// Footer height tracks the activator row count (see renderPanelItems).
    private var footerHeightConstraint: NSLayoutConstraint?
    /// Horizontal stack that holds trailing product/action buttons.
    private let panelStack = NSStackView()
    /// Caps the activator area: rows beyond footerMaxRows scroll in here.
    private let panelScroll = NSScrollView()
    /// The expanded-state collapse toggle. Lives in the header, next to the
    /// sidebar actions; clicking it collapses the sidebar to the icon rail.
    private let hideButton = NSButton()
    private var hideButtonTrackingArea: NSTrackingArea?
    /// The rail-state expand toggle — the first icon in the collapsed rail,
    /// above the lxapp icons; clicking it restores the expanded sidebar.
    private let railExpandButton = NSButton()
    private var panelButtons: [ActivatorRowView] = []
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

    /// Called when a panel icon button is clicked: (panelId)
    var onPanelItemToggled: ((String) -> Void)?

    /// Called when the update callout is clicked, with its current state
    /// (`.ready` → restart, `.available` → install).
    var onUpdateActionRequested: ((UpdateCalloutState) -> Void)?

    /// The transient "ready to update" callout shown above the footer dock.
    private var updateReadyCallout: UpdateReadyCallout?

    /// The single immutable model that drives the whole sidebar. Mutated only by
    /// the public `update*`/`setActiveHighlight`/`clearAllHighlights` methods,
    /// each of which calls `render()` afterwards.
    private var model = SidebarModel()

    // MARK: Render-side view caches (rebuilt/diffed from `model` by render()).
    private var groupViews: [String: SidebarGroupView] = [:]

    // Browser tab views
    private var browserItemViews: [String: SidebarBrowserItemView] = [:]
    private var browserItemTopConstraints: [String: NSLayoutConstraint] = [:]
    private let addButton = NSButton()
    private var addButtonTopConstraint: NSLayoutConstraint?
    private var groupTopConstraints: [String: NSLayoutConstraint] = [:]
    private var addButtonTrackingArea: NSTrackingArea?

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
    /// Called when the bottom hide button is clicked
    var onHideRequested: (() -> Void)?
    /// Called when width changes via drag: (width, animated)
    var onWidthChanged: ((CGFloat, Bool) -> Void)?
    /// Called when "+" button is clicked to add a browser tab
    var onAddBrowserTab: (() -> Void)?
    /// Called when a browser tab is selected
    var onBrowserTabSelected: ((String) -> Void)?
    /// Called when a browser tab close is requested
    var onBrowserTabCloseRequested: ((String) -> Void)?
    /// Called when all browser tabs except the given tab should close.
    var onBrowserTabCloseOtherRequested: ((String) -> Void)?
    /// Called when browser tabs visually below the given tab should close.
    var onBrowserTabCloseTabsBelowRequested: ((String) -> Void)?
    /// Called when settings button is clicked
    var onOpenSettings: (() -> Void)?
    /// Called when downloads button is clicked
    var onOpenDownloads: (() -> Void)?
    /// Called when a pin tile with no open tab is clicked (open its URL)
    var onBookmarkOpen: ((String) -> Void)?
    /// Called from tile menus to open the bookmarks manager page
    var onManageBookmarks: (() -> Void)?

    // MARK: Pin grid state. Pins are persistent website shortcuts above the
    // normal tab list; they never replace or hide an open tab.
    private var bookmarksSnapshot = SidebarBookmarksSnapshot.empty
    private var pinTileViews: [String: SidebarPinTileView] = [:]
    private var pinTileTopConstraints: [String: NSLayoutConstraint] = [:]
    private var pinTileLeadingConstraints: [String: NSLayoutConstraint] = [:]

    var hasPinnedWebsites: Bool {
        !bookmarksSnapshot.pinnedEntries.isEmpty
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

    // MARK: - Setup

    private func setupViews() {
        wantsLayer = true
        clipsToBounds = true

        // Header (traffic lights + actions)
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.wantsLayer = true
        addSubview(headerView)

        // Settings and download buttons — top-right in header
        settingsButton.translatesAutoresizingMaskIntoConstraints = false
        settingsButton.image = Self.designIcon("icon_settings") ?? NSImage(systemSymbolName: "gearshape", accessibilityDescription: nil)
        settingsButton.isBordered = false
        settingsButton.bezelStyle = .regularSquare
        settingsButton.imagePosition = .imageOnly
        settingsButton.contentTintColor = NSColor.secondaryLabelColor
        settingsButton.target = self
        settingsButton.action = #selector(settingsClicked)
        headerView.addSubview(settingsButton)

        downloadButton.translatesAutoresizingMaskIntoConstraints = false
        downloadButton.image = Self.designIcon("icon_download") ?? NSImage(systemSymbolName: "arrow.down.circle", accessibilityDescription: nil)
        downloadButton.isBordered = false
        downloadButton.bezelStyle = .regularSquare
        downloadButton.imagePosition = .imageOnly
        downloadButton.contentTintColor = NSColor.secondaryLabelColor
        downloadButton.target = self
        downloadButton.action = #selector(downloadClicked)
        headerView.addSubview(downloadButton)

        let browserEnabled = (LxAppCore.capabilities & LxAppCore.capBrowser) != 0
        os_log(
            "Sidebar setup browserEnabled=%{public}@ capabilities=%{public}u",
            log: Self.log,
            type: .info,
            browserEnabled ? "true" : "false",
            LxAppCore.capabilities
        )
        settingsButton.isHidden = !browserEnabled
        downloadButton.isHidden = !browserEnabled

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

        // Footer dock — bottom toolbar row for icon buttons
        footerView.translatesAutoresizingMaskIntoConstraints = false
        footerView.wantsLayer = true
        addSubview(footerView)

        // Hairline separator between scroll content and footer
        footerSeparator.translatesAutoresizingMaskIntoConstraints = false
        footerSeparator.wantsLayer = true
        // A subtle divider grouping the activator dock. `separatorColor` washes
        // out on the sidebar material, so use a low-alpha label tint that keeps a
        // little contrast in both light and dark without being prominent.
        footerSeparator.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.12).cgColor
        footerView.addSubview(footerSeparator)

        panelStack.translatesAutoresizingMaskIntoConstraints = false
        // Activator entries stack as full-width rows: icon on the left, title on
        // the right. Icon-only presentation lives in the collapsed rail. The
        // stack lives in a scroll view: past footerMaxRows the area caps and
        // scrolls internally instead of squeezing the tab list above.
        panelStack.orientation = .vertical
        panelStack.spacing = 2
        panelStack.alignment = .width
        panelStack.distribution = .fill
        panelScroll.translatesAutoresizingMaskIntoConstraints = false
        panelScroll.drawsBackground = false
        panelScroll.hasVerticalScroller = true
        panelScroll.autohidesScrollers = true
        panelScroll.verticalScrollElasticity = .none
        let panelDoc = FlippedClipView()
        panelDoc.translatesAutoresizingMaskIntoConstraints = false
        panelDoc.addSubview(panelStack)
        panelScroll.documentView = panelDoc
        NSLayoutConstraint.activate([
            panelStack.topAnchor.constraint(equalTo: panelDoc.topAnchor),
            panelStack.leadingAnchor.constraint(equalTo: panelDoc.leadingAnchor),
            panelStack.trailingAnchor.constraint(equalTo: panelDoc.trailingAnchor),
            panelStack.bottomAnchor.constraint(equalTo: panelDoc.bottomAnchor),
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
        hideButton.contentTintColor = NSColor.secondaryLabelColor
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
        // activators, leaving the top free for a future branding header.
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
        railExpandButton.contentTintColor = NSColor.secondaryLabelColor
        railExpandButton.image = LxIcon.image(
            named: "icon_sidebar_expand",
            size: NSSize(width: Layout.railIconSize, height: Layout.railIconSize))
        railExpandButton.target = self
        railExpandButton.action = #selector(railExpandClicked)
        addSubview(railExpandButton)
        NSLayoutConstraint.activate([
            railExpandButton.widthAnchor.constraint(equalToConstant: Layout.railButtonSize),
            railExpandButton.heightAnchor.constraint(equalToConstant: Layout.railButtonSize),
            railExpandButton.centerXAnchor.constraint(equalTo: railScrollView.centerXAnchor),
            // Pin to the sidebar's true bottom (the footer is hidden in compact),
            // not railScrollView.bottom which stops a footerHeight above it.
            railExpandButton.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -10),
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

            downloadButton.trailingAnchor.constraint(equalTo: hideButton.leadingAnchor, constant: -4),
            downloadButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            downloadButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

            settingsButton.trailingAnchor.constraint(equalTo: downloadButton.leadingAnchor, constant: -4),
            settingsButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            settingsButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

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
            footerView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            footerView.bottomAnchor.constraint(equalTo: bottomAnchor),

            footerSeparator.topAnchor.constraint(equalTo: footerView.topAnchor),
            footerSeparator.leadingAnchor.constraint(equalTo: footerView.leadingAnchor),
            footerSeparator.trailingAnchor.constraint(equalTo: footerView.trailingAnchor),
            footerSeparator.heightAnchor.constraint(equalToConstant: 1.0),

            hideButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            hideButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

            panelScroll.leadingAnchor.constraint(equalTo: footerView.leadingAnchor, constant: Layout.footerLeading),
            panelScroll.trailingAnchor.constraint(equalTo: footerView.trailingAnchor, constant: -Layout.footerInset),
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
        let downloadCenter = downloadButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        let settingsCenter = settingsButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        let toggleCenter = hideButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        buttonCenterYConstraints = [downloadCenter, settingsCenter, toggleCenter]
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
        // expanded layout never has to render squished at narrow widths.
        let compact = clamped >= Layout.railHideThreshold && clamped < Layout.railExpandThreshold
        setCompactMode(compact)
        onWidthChanged?(clamped, false)
    }

    private func handleDragEnd(proposedWidth: CGFloat) {
        if proposedWidth < Layout.railHideThreshold {
            // Fully hidden — restore the expanded layout for the next reveal.
            setCompactMode(false)
            onWidthChanged?(0, true)
        } else if proposedWidth < Layout.railExpandThreshold {
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

        for group in model.appGroups {
            let info = getLxAppInfo(group.appId)
            let iconPath = info.icon.toString()
            let image: NSImage?
            if !iconPath.isEmpty, let img = NSImage(contentsOfFile: iconPath) {
                image = img
            } else {
                image = Self.defaultAppIcon
            }
            let key = "app:\(group.appId)"
            let btn = makeRailButton(key: key, tooltip: info.app_name.toString(), image: image, isTemplate: false)
            btn.action = #selector(railAppClicked(_:))
            if let railButton = btn as? SidebarRailButton {
                railButton.onHoverChanged = { [weak self, weak railButton] hovering in
                    guard let self, let railButton else { return }
                    if hovering {
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

        // New-tab affordance for the collapsed rail — only when a full browser is
        // available (e.g. the showcase desktop app). In hosts without browser-shell
        // (e.g. the lxapp Runner) a "+" would just open a dead tab, so omit it.
        let browserEnabled = (LxAppCore.capabilities & LxAppCore.capBrowser) != 0
        if browserEnabled {
            let addRailButton = makeRailButton(
                key: "action:add-tab",
                tooltip: L10n.string("lx_browser_new_tab"),
                image: LxIcon.image(
                    named: "icon_browser_plus",
                    size: CGSize(width: Layout.railIconSize, height: Layout.railIconSize)),
                isTemplate: true
            )
            addRailButton.action = #selector(addButtonClicked)
            railStack.addArrangedSubview(addRailButton)
        }

        // The expand toggle is not part of this stack — it's pinned to the rail's
        // bottom in setup() so it always anchors the bottom regardless of how many
        // activators are present.

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
        btn.target = self
        btn.identifier = NSUserInterfaceItemIdentifier(key)
        if let image {
            let copy = image.copy() as? NSImage ?? image
            copy.size = NSSize(width: Layout.railIconSize, height: Layout.railIconSize)
            copy.isTemplate = isTemplate
            btn.image = copy
            if isTemplate { btn.contentTintColor = NSColor.secondaryLabelColor }
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
            let selected = key == activeRailKey
            btn.layer?.backgroundColor = selected
                ? NSColor.labelColor.withAlphaComponent(0.12).cgColor
                : NSColor.clear.cgColor
        }
    }

    @objc private func railAppClicked(_ sender: NSButton) {
        guard let key = sender.identifier?.rawValue, key.hasPrefix("app:") else { return }
        let appId = String(key.dropFirst(4))
        let index = getTabBar(appId).map { Int($0.selected_index) } ?? 0
        onAppPageSelected?(appId, index)
    }

    @objc private func railBrowserClicked(_ sender: NSButton) {
        closeRailTabPopover()
        guard let key = sender.identifier?.rawValue, key.hasPrefix("web:") else { return }
        onBrowserTabSelected?(String(key.dropFirst(4)))
    }

    private func browserContextMenu(for id: String) -> NSMenu? {
        closeRailTabPopover()
        guard let tab = model.browserTabs.first(where: { $0.id == id }) else { return nil }
        let menu = NSMenu()

        // Page actions first (Arc keeps pin/copy on the tab row itself).
        let url = tab.url.trimmingCharacters(in: .whitespacesAndNewlines)
        if BrowserPageMenu.isBookmarkActionable(url) {
            let pinnedEntry = bookmarksSnapshot.pinnedEntries.first {
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
        if let pinnedEntry = bookmarksSnapshot.pinnedEntries.first(where: {
            SidebarBookmarksSnapshot.normalize($0.url)
                == SidebarBookmarksSnapshot.normalize(tab.url)
        }) {
            _ = browserBookmarksCommand(
                #"{"op":"setPinned","id":"\#(jsonEscape(pinnedEntry.id))","pinned":false}"#
            )
        } else {
            _ = browserBookmarkPin(tab.url, tab.title)
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
        let browserEnabled = (LxAppCore.capabilities & LxAppCore.capBrowser) != 0
        os_log(
            "Sidebar visibility hidden=%{public}@ browserEnabled=%{public}@ capabilities=%{public}u",
            log: Self.log,
            type: .debug,
            hidden ? "true" : "false",
            browserEnabled ? "true" : "false",
            LxAppCore.capabilities
        )
        let compact = isCompact && !hidden && !appUIOnlyMode
        scrollView.isHidden = hidden || appUIOnlyMode || compact
        railScrollView.isHidden = hidden || appUIOnlyMode || !compact
        // The rail's bottom-pinned expand toggle lives outside the scroll view, so
        // toggle it with the rail.
        railExpandButton.isHidden = hidden || appUIOnlyMode || !compact
        // The header action buttons and footer panel icons don't fit the rail.
        settingsButton.isHidden = hidden || !browserEnabled || appUIOnlyMode || compact
        downloadButton.isHidden = hidden || !browserEnabled || appUIOnlyMode || compact
        // The header collapse toggle shows only in the expanded layout; the rail
        // carries its own expand toggle anchored at the bottom when compact.
        hideButton.isHidden = hidden || appUIOnlyMode || compact
        panelScroll.isHidden = compact
        // The footer only carries panel icons now; collapse it when empty so an
        // expanded sidebar with no panel actions has no dangling bottom bar.
        footerView.isHidden = hidden || compact || model.panelItems.isEmpty
        resizeHandle.isHidden = hidden
    }

    func setAppUIOnlyMode(_ enabled: Bool) {
        appUIOnlyMode = enabled
        // appUIOnlyMode is enforced in one place — render() empties the list/rail
        // sections when it's on. The footer panel survives, so panelItems is kept.
        if enabled {
            model.appGroups = []
            model.browserTabs = []
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

    /// Populate panel icon buttons in the footer.
    /// `PanelIconItem` only carries what the UI needs — id, icon, label.
    /// Routing details (appId, path, position) stay in Panel.swift.
    func updatePanelItems(_ items: [PanelIconItem]) {
        model.panelItems = items
        renderPanelItems()
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
        panelButtons.forEach {
            panelStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        panelButtons.removeAll()

        // Footer height tracks the row count, capped at footerMaxRows — past
        // that the stack scrolls inside the fixed-height area.
        let rows = min(CGFloat(items.count), Layout.footerMaxRows)
        footerHeightConstraint?.constant = items.isEmpty
            ? 0
            : Layout.footerInset * 2 + 1 + rows * Layout.footerButtonSize + (rows - 1) * panelStack.spacing

        guard !items.isEmpty else { return }

        for item in items {
            // A custom row, not an NSButton: a borderless button centers its
            // image+title block, so left alignment is impossible with it.
            let row = ActivatorRowView(
                label: item.label,
                iconURL: item.iconURL,
                labelColor: item.labelColor
            )
            row.onClick = { [weak self] in
                self?.onPanelItemToggled?(item.id)
            }
            row.heightAnchor.constraint(equalToConstant: Layout.footerButtonSize).isActive = true
            panelStack.addArrangedSubview(row)
            panelButtons.append(row)
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
    }

    // MARK: - Public API (thin model mutators)
    //
    // Each method below updates one part of `model` and calls `render()`. They
    // hold no layout or appUIOnlyMode logic — that all lives in render().

    /// Rebuild all groups based on current tabs.
    func updateForTabs(_ tabs: [LxAppTab], activeTab: LxAppTab?) {
        model.appGroups = tabs.map {
            SidebarModel.AppGroupVM(appId: $0.appId, asideSurfaceId: $0.asideSurfaceId)
        }
        // A provided active tab updates the selection; nil leaves it untouched,
        // matching the original (which only highlighted when activeTab existed).
        if let activeAppId = activeTab?.appId {
            model.selection = .app(appId: activeAppId, pageIndex: nil)
        }
        render()
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
            if let existing = groupViews[appId] {
                groupView = existing
            } else {
                groupView = SidebarGroupView(appId: appId)
                groupView.onPageSelected = { [weak self] appId, itemIndex in
                    self?.onAppPageSelected?(appId, itemIndex)
                }
                groupView.onAppSelected = { [weak self] appId in
                    self?.onAppSelected?(appId)
                }
                groupView.onCloseRequested = { [weak self] appId in
                    self?.onAppCloseRequested?(appId)
                }
                groupView.onLayoutChanged = { [weak self] in
                    self?.relayoutAfterGroupToggle()
                }
                groupViews[appId] = groupView
            }

            groupView.setColorIndex(index)

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
        if (LxAppCore.capabilities & LxAppCore.capBrowser) != 0 {
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
        guard (LxAppCore.capabilities & LxAppCore.capBrowser) != 0 else { return }
        bookmarksSnapshot = SidebarBookmarksSnapshot.loadFromHost()
        render()
    }

    /// Diff pin tiles against the snapshot's pinned subset.
    private func renderPinTiles() {
        let pinned = bookmarksSnapshot.pinnedEntries
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
        let pinned = bookmarksSnapshot.pinnedEntries
        guard !pinned.isEmpty else { return startY }
        let inset: CGFloat = SidebarGroupView.Layout.groupInset + 4
        let size = SidebarPinTileView.Layout.size
        let gap = SidebarPinTileView.Layout.gap
        let columns = SidebarPinTileView.Layout.columns
        var yOffset = startY

        for (index, entry) in pinned.enumerated() {
            guard let tile = pinTileViews[entry.id] else { continue }
            let column = index % columns
            let row = index / columns
            let x = inset + CGFloat(column) * (size + gap)
            let y = startY + CGFloat(row) * (size + gap)
            ensureSubview(tile, in: docView) {}
            if let tc = pinTileTopConstraints[entry.id] {
                tc.constant = y
            } else {
                let tc = tile.topAnchor.constraint(equalTo: docView.topAnchor, constant: y)
                tc.isActive = true
                pinTileTopConstraints[entry.id] = tc
            }
            if let lc = pinTileLeadingConstraints[entry.id] {
                lc.constant = x
            } else {
                let lc = tile.leadingAnchor.constraint(equalTo: docView.leadingAnchor, constant: x)
                lc.isActive = true
                pinTileLeadingConstraints[entry.id] = lc
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
        addButton.toolTip = L10n.string("lx_browser_new_tab")
        addButton.isBordered = false
        addButton.bezelStyle = .regularSquare
        addButton.imagePosition = .imageOnly
        addButton.contentTintColor = NSColor.secondaryLabelColor
        addButton.wantsLayer = true
        addButton.layer?.cornerRadius = 6
        addButton.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        addButton.target = self
        addButton.action = #selector(addButtonClicked)
    }

    @objc private func addButtonClicked() {
        onAddBrowserTab?()
    }

    @objc private func hideButtonClicked() {
        onHideRequested?()
    }

    @objc private func railExpandClicked() {
        // Restore the expanded sidebar from the icon rail.
        setCompactMode(false)
        onWidthChanged?(Layout.expandedWidth, true)
    }

    @objc private func settingsClicked() {
        onOpenSettings?()
    }

    @objc private func downloadClicked() {
        onOpenDownloads?()
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
        let alpha: CGFloat = hovered ? 0.12 : 0.06
        addButton.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(alpha).cgColor
    }

    private func setHideButtonHovered(_ hovered: Bool) {
        hideButton.layer?.backgroundColor = hovered
            ? NSColor.labelColor.withAlphaComponent(0.09).cgColor
            : NSColor.clear.cgColor
        hideButton.contentTintColor = hovered ? NSColor.labelColor : NSColor.secondaryLabelColor
    }
}

/// NSView subclass with flipped coordinate system (top-left origin)
@MainActor
private class FlippedView: NSView {
    override var isFlipped: Bool { true }
    override var mouseDownCanMoveWindow: Bool { false }
}

/// Top-aligned scroll document host (NSClipView content is bottom-anchored
/// in non-flipped coordinates).
@MainActor
private final class FlippedClipView: NSView {
    override var isFlipped: Bool { true }
}

/// One activator entry: a left-aligned icon + title row sharing the tabbar
/// items' rhythm (30pt, hover wash). A custom view because a borderless
/// NSButton centers its image+title block and cannot left-align it.
@MainActor
final class ActivatorRowView: NSView {
    var onClick: (() -> Void)?

    private let iconView = NSImageView()
    private let titleLabel: NSTextField
    private var isHovered = false { didSet { updateBackground() } }
    private var tracking: NSTrackingArea?

    private let washView = NSView()

    init(label: String, iconURL: URL?, labelColor: NSColor?) {
        titleLabel = NSTextField(labelWithString: label)
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        toolTip = label
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel(label)

        // Activator entries are TOP-LEVEL rows: hover rect and icon axis copy
        // the web-tab rows (full-row wash, radius 6, icon inset 8), not the
        // nested tabbar items' deeper card.
        washView.translatesAutoresizingMaskIntoConstraints = false
        washView.wantsLayer = true
        washView.layer?.cornerRadius = 6
        addSubview(washView)

        let icon = iconURL.flatMap { NSImage(contentsOf: $0) }
            ?? Bundle.lingxiaResources.url(
                forResource: "lxapp_default", withExtension: "png", subdirectory: "icons")
                .flatMap { NSImage(contentsOf: $0) }
        icon?.size = NSSize(width: 16, height: 16)
        iconView.image = icon
        iconView.imageScaling = .scaleProportionallyDown
        iconView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(iconView)

        titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .regular)
        titleLabel.textColor = labelColor ?? NSColor.labelColor
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(titleLabel)

        NSLayoutConstraint.activate([
            washView.leadingAnchor.constraint(equalTo: leadingAnchor),
            washView.trailingAnchor.constraint(equalTo: trailingAnchor),
            washView.topAnchor.constraint(equalTo: topAnchor),
            washView.bottomAnchor.constraint(equalTo: bottomAnchor),
            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 16),
            iconView.heightAnchor.constraint(equalToConstant: 16),
            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: 8),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: washView.trailingAnchor, constant: -8),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func setIcon(_ image: NSImage) {
        image.size = NSSize(width: 16, height: 16)
        iconView.image = image
    }

    private func updateBackground() {
        washView.layer?.backgroundColor = isHovered
            ? NSColor.labelColor.withAlphaComponent(0.06).cgColor
            : NSColor.clear.cgColor
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

    override func mouseEntered(with event: NSEvent) { isHovered = true }
    override func mouseExited(with event: NSEvent) { isHovered = false }
    override func mouseDown(with event: NSEvent) { onClick?() }
    override var mouseDownCanMoveWindow: Bool { false }
    override func accessibilityPerformPress() -> Bool {
        onClick?()
        return true
    }
}

#endif
