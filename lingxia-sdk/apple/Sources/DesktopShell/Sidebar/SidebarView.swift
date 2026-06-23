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
        /// Square icon button size in the dock.
        static let footerButtonSize: CGFloat = 28
        /// Rendered glyph size inside footer icon buttons.
        static let footerIconSize: CGFloat = 16
        /// Horizontal/vertical padding inside the dock.
        static let footerInset: CGFloat = 6
    }

    private let headerView = NSView()
    private let settingsButton = NSButton()
    private let downloadButton = NSButton()
    private let scrollView = SidebarScrollView()
    private let resizeHandle = SidebarResizeHandle()
    private let footerView = NSView()
    private let footerSeparator = NSView()
    /// Horizontal stack that holds trailing product/action buttons.
    private let panelStack = NSStackView()
    /// The expanded-state collapse toggle. Lives in the header, next to the
    /// sidebar actions; clicking it collapses the sidebar to the icon rail.
    private let hideButton = NSButton()
    private var hideButtonTrackingArea: NSTrackingArea?
    /// The rail-state expand toggle — the first icon in the collapsed rail,
    /// above the lxapp icons; clicking it restores the expanded sidebar.
    private let railExpandButton = NSButton()
    private var panelButtons: [NSButton] = []
    /// The panel items currently materialized as footer buttons. Lets
    /// renderPanelItems() skip a rebuild when render() runs for an unrelated
    /// change — so `updatePanelIcon`'s resolved icons aren't wiped out.
    private var renderedPanelItems: [PanelIconItem] = []
    private var appUIOnlyMode = false

    // MARK: Icon-rail (collapsed) state

    /// True when the sidebar is collapsed to the icon-only rail.
    private(set) var isCompact = false
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
        guard let url = Bundle.module.url(
            forResource: "lxapp_default", withExtension: "png", subdirectory: "icons")
        else { return nil }
        return NSImage(contentsOf: url)
    }()

    /// A shared design icon (bundled PDF) as a tintable template image, so the
    /// header affordances match their iOS counterparts.
    private static func designIcon(_ name: String) -> NSImage? {
        guard let url = Bundle.module.url(forResource: name, withExtension: "pdf", subdirectory: "icons")
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
    /// Called when settings button is clicked
    var onOpenSettings: (() -> Void)?
    /// Called when downloads button is clicked
    var onOpenDownloads: (() -> Void)?

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
        panelStack.orientation = .horizontal
        panelStack.spacing = 4
        panelStack.alignment = .centerY
        panelStack.distribution = .fill
        footerView.addSubview(panelStack)

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
            railExpandButton.bottomAnchor.constraint(equalTo: railScrollView.bottomAnchor, constant: -8),
        ])

        // Resize handle on right edge
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false
        resizeHandle.wantsLayer = true
        addSubview(resizeHandle)

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

            // Rail occupies the same region as the main scroll view.
            railScrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            railScrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            railScrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            railScrollView.bottomAnchor.constraint(equalTo: footerView.topAnchor),

            footerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            footerView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            footerView.bottomAnchor.constraint(equalTo: bottomAnchor),
            footerView.heightAnchor.constraint(equalToConstant: Layout.footerHeight),

            footerSeparator.topAnchor.constraint(equalTo: footerView.topAnchor),
            footerSeparator.leadingAnchor.constraint(equalTo: footerView.leadingAnchor),
            footerSeparator.trailingAnchor.constraint(equalTo: footerView.trailingAnchor),
            footerSeparator.heightAnchor.constraint(equalToConstant: 1.0),

            hideButton.widthAnchor.constraint(equalToConstant: Layout.actionButtonSize),
            hideButton.heightAnchor.constraint(equalToConstant: Layout.actionButtonSize),

            panelStack.leadingAnchor.constraint(equalTo: footerView.leadingAnchor, constant: Layout.footerInset),
            panelStack.trailingAnchor.constraint(lessThanOrEqualTo: footerView.trailingAnchor, constant: -Layout.footerInset),
            panelStack.centerYAnchor.constraint(equalTo: footerView.centerYAnchor),

            // Resize handle: right edge, full height
            resizeHandle.topAnchor.constraint(equalTo: topAnchor),
            resizeHandle.trailingAnchor.constraint(equalTo: trailingAnchor),
            resizeHandle.bottomAnchor.constraint(equalTo: bottomAnchor),
            resizeHandle.widthAnchor.constraint(equalToConstant: Layout.resizeHandleWidth),
        ])

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
            let image = item.favicon ?? NSImage(systemSymbolName: "globe", accessibilityDescription: item.title)
            let btn = makeRailButton(key: key, tooltip: item.title, image: image, isTemplate: item.favicon == nil)
            btn.action = #selector(railBrowserClicked(_:))
            if let railButton = btn as? SidebarRailButton {
                railButton.onHoverChanged = { [weak self] hovering in
                    if hovering { self?.closeRailTabPopover() }
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
                tooltip: "Add browser tab",
                image: NSImage(systemSymbolName: "plus", accessibilityDescription: "Add browser tab"),
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
        panelStack.isHidden = compact
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

        guard !items.isEmpty else { return }

        for item in items {
            let btn = NSButton()
            btn.translatesAutoresizingMaskIntoConstraints = false
            btn.isBordered = false
            btn.bezelStyle = .regularSquare
            btn.imagePosition = .imageOnly
            btn.imageScaling = .scaleProportionallyDown
            btn.wantsLayer = true
            btn.layer?.cornerRadius = 6
            btn.layer?.backgroundColor = NSColor.clear.cgColor
            btn.toolTip = item.label
            if let iconURL = item.iconURL,
               let image = NSImage(contentsOf: iconURL) {
                image.size = NSSize(width: Layout.footerIconSize, height: Layout.footerIconSize)
                image.isTemplate = true
                btn.image = image
                btn.contentTintColor = NSColor.secondaryLabelColor
            } else {
                let fallback = NSImage(systemSymbolName: "square.grid.2x2", accessibilityDescription: item.label)
                fallback?.size = NSSize(width: Layout.footerIconSize, height: Layout.footerIconSize)
                btn.image = fallback
                btn.contentTintColor = NSColor.secondaryLabelColor
            }
            btn.target = self
            btn.action = #selector(panelButtonClicked(_:))
            // Store panel id in the button's identifier
            btn.identifier = NSUserInterfaceItemIdentifier(item.id)
            NSLayoutConstraint.activate([
                btn.widthAnchor.constraint(equalToConstant: Layout.footerButtonSize),
                btn.heightAnchor.constraint(equalToConstant: Layout.footerButtonSize),
            ])
            panelStack.addArrangedSubview(btn)
            panelButtons.append(btn)
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

    /// Update a panel button's icon from a file:// URL (resolved via resolveLxUri after lxapp installs).
    func updatePanelIcon(panelId: String, iconFileUrl: String) {
        guard let btn = panelButtons.first(where: { $0.identifier?.rawValue == panelId }),
              let url = URL(string: iconFileUrl),
              let image = NSImage(contentsOf: url) else { return }
        btn.image = image
        btn.contentTintColor = nil
    }

    @objc private func panelButtonClicked(_ sender: NSButton) {
        guard let panelId = sender.identifier?.rawValue else { return }
        onPanelItemToggled?(panelId)
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
    func updateBrowserItems(_ items: [(id: String, title: String, favicon: NSImage?)], activeId: String?) {
        model.browserTabs = items.map {
            SidebarModel.BrowserTabVM(id: $0.id, title: $0.title, favicon: $0.favicon)
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
        let yOffset = renderAppGroups(in: docView)
        let finalY = layoutBrowserSection(in: docView, yOffset: yOffset)
        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: finalY)

        applySelection()

        if isCompact { rebuildRail() }
    }

    /// Diff app group views against `model.appGroups`, position them, and return
    /// the Y offset where the browser section begins.
    private func renderAppGroups(in docView: NSView) -> CGFloat {
        // Remove groups for apps no longer present.
        let currentAppIds = Set(model.appGroups.map { $0.appId })
        for (appId, groupView) in groupViews where !currentAppIds.contains(appId) {
            groupView.removeFromSuperview()
            groupViews.removeValue(forKey: appId)
            groupTopConstraints.removeValue(forKey: appId)
        }

        // Add/update groups.
        var yOffset: CGFloat = 4
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
            yOffset += groupView.fittingSize.height + 8
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
                existing.configure(title: item.title, isSelected: selected, favicon: item.favicon)
            } else {
                let itemView = SidebarBrowserItemView(id: item.id)
                itemView.translatesAutoresizingMaskIntoConstraints = false
                itemView.onClick = { [weak self] id in
                    self?.onBrowserTabSelected?(id)
                }
                itemView.onClose = { [weak self] id in
                    self?.onBrowserTabCloseRequested?(id)
                }
                itemView.configure(title: item.title, isSelected: selected, favicon: item.favicon)
                browserItemViews[item.id] = itemView
            }
        }
    }

    /// Layout browser items and add button after lxapp groups
    private func layoutBrowserSection(in docView: NSView, yOffset startY: CGFloat) -> CGFloat {
        let groupInset: CGFloat = SidebarGroupView.Layout.groupInset
        var yOffset = startY

        // Browser item views (ordered by model.browserTabs)
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
            yOffset += SidebarBrowserItemView.Layout.height + 2
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

        // Reposition all groups using stored top constraints
        var yOffset: CGFloat = 4
        for group in model.appGroups {
            guard let groupView = groupViews[group.appId] else { continue }
            groupTopConstraints[group.appId]?.constant = yOffset
            groupView.layoutSubtreeIfNeeded()
            yOffset += groupView.fittingSize.height + 8
        }

        // Re-layout browser section below groups
        yOffset = layoutBrowserSection(in: docView, yOffset: yOffset)

        docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: yOffset)
    }

    private func setupAddButton() {
        addButton.translatesAutoresizingMaskIntoConstraints = false
        addButton.title = ""
        addButton.image = NSImage(systemSymbolName: "plus", accessibilityDescription: "Add browser tab")
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

#endif
