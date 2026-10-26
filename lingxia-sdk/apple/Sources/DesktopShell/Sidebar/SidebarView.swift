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
            themeCGColor(SidebarActionChromePalette.pressed)
        } else if hovered {
            themeCGColor(SidebarActionChromePalette.hover)
        } else {
            NSColor.clear.cgColor
        }
    }
}

@MainActor
private final class SidebarRailPanelHoverView: NSView {
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

/// A window the collapsed rail floats beside itself. Both rail affordances use
/// it so they share one shape language — flat fill, hairline edge, no arrow and
/// no popover material — and so each paints its own background instead of
/// borrowing one; the rail can float over an lxapp WebView, which has none to
/// borrow. Never key: a hover affordance must not take focus from the page.
@MainActor
private final class SidebarRailFloatWindow: NSPanel {
    /// Where the float sits against the icon it belongs to.
    enum Alignment {
        /// Centered on the icon — right for a single line of text.
        case centeredOnAnchor
        /// Top edges flush, so a list reads as pulled out of the icon.
        case topWithAnchor
        /// Centered over the icon; for the footer, which sits on the window's
        /// bottom edge and whose icons are not on the column's edge.
        case aboveAnchor
    }

    private enum Layout {
        static let gap: CGFloat = 6
    }

    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }

    init(content: NSView, radius: CGFloat, interactive: Bool) {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        isOpaque = false
        backgroundColor = .clear
        isFloatingPanel = true
        hidesOnDeactivate = true
        // A menu floating over page content needs to read as lifted; a hint is
        // a label and stays flat.
        hasShadow = interactive
        // A hint takes no events at all: crossing it would end the hover that
        // is keeping it alive. A page list has rows to click, so it does.
        ignoresMouseEvents = !interactive

        let backdrop = LxAppHostThemeLayerView(role: .windowBackground)
        backdrop.wantsLayer = true
        backdrop.layer?.cornerRadius = radius
        backdrop.layer?.borderWidth = 1
        backdrop.layer?.borderColor = backdrop.themeCGColor(LxAppHostTheme.separator)
        backdrop.layer?.masksToBounds = true

        content.translatesAutoresizingMaskIntoConstraints = false
        backdrop.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: backdrop.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: backdrop.trailingAnchor),
            content.topAnchor.constraint(equalTo: backdrop.topAnchor),
            content.bottomAnchor.constraint(equalTo: backdrop.bottomAnchor),
        ])
        contentView = backdrop
    }

    /// `railFrame` is the rail's own screen rect, not the icon's: every float
    /// clears the whole column so a run of them lines up on one edge.
    func present(size: NSSize, anchor: NSView, railFrame: NSRect, alignment: Alignment) {
        guard let host = anchor.window else { return }
        let anchorFrame = host.convertToScreen(anchor.convert(anchor.bounds, to: nil))
        let origin: NSPoint
        switch alignment {
        case .centeredOnAnchor:
            origin = NSPoint(x: railFrame.maxX + Layout.gap, y: anchorFrame.midY - size.height / 2)
        case .topWithAnchor:
            origin = NSPoint(x: railFrame.maxX + Layout.gap, y: anchorFrame.maxY - size.height)
        case .aboveAnchor:
            origin = NSPoint(x: anchorFrame.midX - size.width / 2, y: anchorFrame.maxY + Layout.gap)
        }
        var frame = NSRect(origin: origin, size: size)
        // Keep inside the window when it fits, then inside the screen.
        for bounds in [host.frame, host.screen?.visibleFrame].compactMap({ $0 })
        where bounds.width >= size.width && bounds.height >= size.height {
            frame.origin.x = min(max(frame.origin.x, bounds.minX), bounds.maxX - size.width)
            frame.origin.y = min(max(frame.origin.y, bounds.minY), bounds.maxY - size.height)
        }
        setFrame(frame, display: false)
        if parent == nil {
            host.addChildWindow(self, ordered: .above)
        }
        orderFront(nil)
        DevServiceMarkWindow.noteChildWindow(of: host)
    }

    func dismiss() {
        parent?.removeChildWindow(self)
        orderOut(nil)
    }
}

/// The collapsed rail's passive hint: a flat pill that hugs its text, beside
/// the icon it names. Deliberately lighter than the page panel below — a name
/// is not a menu — and matched to the Windows rail tooltip's geometry.
@MainActor
private final class SidebarRailTooltip {
    private enum Layout {
        static let height: CGFloat = 32
        static let hPadding: CGFloat = 12
        static let minWidth: CGFloat = 72
        static let maxWidth: CGFloat = 240
        static let radius: CGFloat = 6
    }

    private var float: SidebarRailFloatWindow?
    private let label = NSTextField(labelWithString: "")
    private var resignObserver: NSObjectProtocol?

    var isVisible: Bool { float?.isVisible == true }

    init() {
        // A hint dismissed on deactivate would otherwise come back with the
        // app (`hidesOnDeactivate`), wherever the cursor is by then.
        resignObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.willResignActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.hide() }
        }
    }

    func show(
        text: String,
        anchor: NSView,
        railFrame: NSRect,
        alignment: SidebarRailFloatWindow.Alignment = .centeredOnAnchor
    ) {
        guard !text.isEmpty else {
            hide()
            return
        }
        let float = float ?? makeFloat()

        label.stringValue = text
        label.textColor = LxAppHostTheme.foreground
        let width = min(
            max(label.intrinsicContentSize.width + Layout.hPadding * 2, Layout.minWidth),
            Layout.maxWidth
        )
        float.present(
            size: NSSize(width: width, height: Layout.height),
            anchor: anchor,
            railFrame: railFrame,
            alignment: alignment
        )
    }

    func hide() {
        float?.dismiss()
    }

    private func makeFloat() -> SidebarRailFloatWindow {
        let content = NSView()
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = .systemFont(ofSize: 12)
        label.lineBreakMode = .byTruncatingTail
        label.maximumNumberOfLines = 1
        content.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: Layout.hPadding),
            label.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -Layout.hPadding),
            label.centerYAnchor.constraint(equalTo: content.centerYAnchor),
        ])
        let float = SidebarRailFloatWindow(content: content, radius: Layout.radius, interactive: false)
        self.float = float
        return float
    }
}

/// The collapsed rail's page panel: the tabBar of the lxapp under the cursor.
/// While the sidebar is a rail this is the only way to reach those pages, so it
/// also carries the app's name — the hint that names other icons is suppressed
/// here to avoid two hover affordances on one anchor.
@MainActor
private final class SidebarRailPagePanel {
    private enum Layout {
        static let width: CGFloat = 188
        static let inset: CGFloat = 8
        static let spacing: CGFloat = 2
        static let titleSpacing: CGFloat = 6
        static let radius: CGFloat = 10
    }

    private var float: SidebarRailFloatWindow?

    /// Owns the tabBar data and the item rows.
    private let providerAppId: String?
    /// Identifies the switcher to the shell; a managed main's surface id is not
    /// the lxapp id that provides its pages.
    private let selectionAppId: String
    private let label: String
    private let items: [TabBarItem]
    private let selectedIndex: Int

    var onPageSelected: ((String, Int) -> Void)?
    var onDismissRequested: (() -> Void)?
    var onHoverChanged: ((Bool) -> Void)?

    var isVisible: Bool { float?.isVisible == true }

    init(
        selectionAppId: String,
        providerAppId: String?,
        label: String,
        items: [TabBarItem],
        selectedIndex: Int
    ) {
        self.selectionAppId = selectionAppId
        self.providerAppId = providerAppId
        self.label = label
        self.items = items
        self.selectedIndex = selectedIndex
    }

    func present(anchor: NSView, railFrame: NSRect) {
        let (content, size) = makeContent()
        let float = SidebarRailFloatWindow(content: content, radius: Layout.radius, interactive: true)
        self.float = float
        float.present(size: size, anchor: anchor, railFrame: railFrame, alignment: .topWithAnchor)
    }

    func dismiss() {
        float?.dismiss()
        float = nil
    }

    private func makeContent() -> (NSView, NSSize) {
        let rootView = SidebarRailPanelHoverView()
        rootView.onHoverChanged = { [weak self] hovering in
            self?.onHoverChanged?(hovering)
        }

        let stack = NSStackView()
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = Layout.spacing
        rootView.addSubview(stack)

        // Same treatment as the expanded group's header, so the float reads as
        // that group pulled out of the rail rather than a different control.
        let titleLabel = NSTextField(labelWithString: label.uppercased())
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = .systemFont(ofSize: 11, weight: .semibold)
        titleLabel.textColor = LxAppHostTheme.foreground
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.maximumNumberOfLines = 1
        stack.addArrangedSubview(titleLabel)

        for (index, item) in items.enumerated() {
            let itemView = SidebarItemView(
                appId: providerAppId ?? selectionAppId,
                itemIndex: index
            )
            itemView.translatesAutoresizingMaskIntoConstraints = false
            itemView.configure(item: item)
            itemView.isSelected = (item.cachedIndex == selectedIndex)
            itemView.onClick = { [weak self] tabIndex in
                guard let self else { return }
                self.onPageSelected?(self.selectionAppId, tabIndex)
                self.onDismissRequested?()
            }
            stack.addArrangedSubview(itemView)
            itemView.widthAnchor.constraint(equalToConstant: Layout.width - (Layout.inset * 2)).isActive = true
        }
        stack.setCustomSpacing(Layout.titleSpacing, after: titleLabel)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: rootView.leadingAnchor, constant: Layout.inset),
            stack.trailingAnchor.constraint(equalTo: rootView.trailingAnchor, constant: -Layout.inset),
            stack.topAnchor.constraint(equalTo: rootView.topAnchor, constant: Layout.inset),
            stack.bottomAnchor.constraint(equalTo: rootView.bottomAnchor, constant: -Layout.inset),
        ])

        let titleHeight = titleLabel.intrinsicContentSize.height
        let itemHeight = CGFloat(items.count) * SidebarItemView.Layout.height
        let rowSpacing = CGFloat(max(0, items.count - 1)) * Layout.spacing
        return (
            rootView,
            NSSize(
                width: Layout.width,
                height: titleHeight + Layout.titleSpacing + itemHeight + rowSpacing + (Layout.inset * 2)
            )
        )
    }
}

// MARK: - PanelIconItem

/// Minimal display info for a panel icon in the sidebar footer.
/// SidebarView only needs these — routing details (appId, path) are in Panel.swift.
enum SidebarActionSource: Equatable {
    case runtime
    case staticSettings
}

struct PanelIconItem {
    var generation: UInt64 = 0
    let id: String
    let iconURL: URL?
    /// Stable name: tooltip and accessibility label, never drawn.
    let label: String
    /// The changing fact drawn beside the icon in the expanded footer.
    var value: String? = nil
    let active: Bool
    let disabled: Bool
    var source: SidebarActionSource = .runtime
    var systemImageName: String? = nil

    /// `value` unless it repeats `label`, which is then shown once.
    var distinctValue: String? {
        value == label ? nil : value
    }

    /// `label`, or `label` and `value` joined, so an icon-only or truncated
    /// cell still carries the full value.
    var tooltipText: String {
        guard let value = distinctValue else { return label }
        return label + L10n.string("lx_sidebar_action_value_separator") + value
    }
}

private struct SidebarActionIdentity {
    let generation: UInt64
    let id: String
    let source: SidebarActionSource
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
            && lhs.value == rhs.value && lhs.active == rhs.active && lhs.disabled == rhs.disabled
            && lhs.source == rhs.source && lhs.systemImageName == rhs.systemImageName
    }
}

// MARK: - SidebarView

/// The main sidebar container view, modeled after Chrome vertical tab groups.
/// Supports drag-to-resize and an icon-only rail.
@MainActor
class SidebarView: NSView {
    private static let log = OSLog(subsystem: "LingXia", category: "Sidebar")

    struct Layout {
        /// One leading axis for every first-column icon in the sidebar — group
        /// headers place an 18pt icon at groupInset + headerHPadding, and the
        /// traffic lights, pin grid, and footer buttons center on the same line.
        static let iconAxis: CGFloat =
            SidebarGroupView.Layout.groupInset + SidebarGroupView.Layout.headerHPadding + 9

        static let expandedWidth: CGFloat = 148
        static let maxWidth: CGFloat = 400
        static let fullyHiddenThreshold: CGFloat = 1
        /// Minimum width of the collapsed icon-only rail. The effective width
        /// grows to clear the macOS traffic lights when they're wider (see
        /// `effectiveRailWidth`).
        static let railWidth: CGFloat = 56
        /// Drag-end below this snaps to the icon rail; at/above it the sidebar expands.
        static let railExpandThreshold: CGFloat = 128
        /// Square icon button in the rail.
        static let railButtonSize: CGFloat = 32
        /// Rendered icon size inside a rail button.
        static let railIconSize: CGFloat = 20
        /// Close chip overlaid on the active rail switcher. Wider than
        /// `railIconSize` so it covers the icon, narrower than
        /// `railButtonSize` so the tile edge still re-selects.
        static let railCloseBadgeSize: CGFloat = railIconSize + 6
        // Only the rail needs a separate toggle row below the traffic lights.
        static let trafficLightsHeight: CGFloat = 28
        static let controlRowHeight: CGFloat = 32
        /// Rail footer chrome around its action stack: 6pt above (below the
        /// 1pt separator) and 6pt below.
        static let railFooterChromeHeight: CGFloat = 6 + 1 + 6
        /// Header actions and the collapse toggle share a 24pt target.
        static let actionButtonSize: CGFloat = 24
        static let actionButtonSpacing: CGFloat = 2
        static let resizeHandleWidth: CGFloat = 5
        /// Bottom dock height — tall enough for one row of icon buttons plus breathing room.
        static let footerHeight: CGFloat = 48
        /// sidebar action row height — matches the tabbar item rhythm above.
        static let footerButtonSize: CGFloat = 30
        /// Rendered glyph size inside footer icon buttons.
        static let footerIconSize: CGFloat = 16
        /// Vertical inset of the sidebar action flow. Horizontally it is
        /// `footerHInset`, leaving 130pt of flow width in the standard 148pt rail.
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
    /// Top of the action footer, for overlays that must sit above every action row.
    var footerTopAnchor: NSLayoutYAxisAnchor { footerView.topAnchor }
    /// Adaptive flow that keeps short sidebar actions on the same visual row.
    private let panelFlow = SidebarActionFlowView()
    /// Caps the sidebar action area: rows beyond footerMaxRows scroll in here.
    private let panelScroll = NSScrollView()
    /// The expanded-state collapse toggle. Lives in the header, next to the
    /// sidebar actions; clicking it collapses the sidebar to the icon rail.
    private let hideButton = NSButton()
    private var hideButtonTrackingArea: NSTrackingArea?
    /// The rail-state expand toggle sits below the native window controls.
    private let railExpandButton = NSButton()
    private var railExpandTrackingArea: NSTrackingArea?
    /// False in a compact window, where the rail cannot be pinned open and
    /// the expand control previews instead.
    var railExpandEnabled = true {
        didSet { updateRailExpandTooltip() }
    }
    private var panelButtons: [SidebarActionRowView] = []
    /// The panel items currently materialized as footer buttons. Lets
    /// renderPanelItems() skip a rebuild when render() runs for an unrelated
    /// change — so `updatePanelIcon`'s resolved icons aren't wiped out.
    private var renderedPanelItems: [PanelIconItem] = []
    private var appUIOnlyMode = false

    // MARK: Icon-rail (collapsed) state

    /// True when the sidebar is collapsed to the icon-only rail.
    private(set) var isCompact = false
    /// The expanded layout shown as a transient overlay over a committed rail.
    /// `isCompact` stays the committed mode the shell persists.
    private(set) var isPeeking = false
    private var showsRail: Bool { isCompact && !isPeeking }
    private var isRailExpandHovered = false
    private var peekIntentTask: Task<Void, Never>?
    private var peekDismissTask: Task<Void, Never>?
    private var peekTrackingArea: NSTrackingArea?
    private var peekMenuObservers: [NSObjectProtocol] = []
    /// Pins the expand control to the rail's axis, which a widening peek
    /// must not drag along.
    private var railExpandCenterXConstraint: NSLayoutConstraint?
    private var isMenuTracking = false

    private var headerHeightConstraint: NSLayoutConstraint?
    private var controlsAlignedToTop = false
    /// Supplies the minimum width that still clears the macOS traffic lights,
    /// so the rail can be as narrow as those controls allow.
    var trafficLightClearanceProvider: (() -> CGFloat)?
    /// Rail width that both honors the minimum and clears the traffic lights.
    /// The shell's clearance leaves ~12pt of breathing room for the expanded
    /// layout; the rail hugs the traffic lights with only a small gap to the
    /// webview edge.
    private var effectiveRailWidth: CGFloat {
        // The provider already reports where the rail has to end to hold the
        // window controls. Nothing is added on top of it here: the column used
        // to carry that padding twice, which is what left it wider than the
        // cluster it exists to hold.
        let clearance = trafficLightClearanceProvider?() ?? Layout.railWidth
        return max(Layout.railWidth, clearance)
    }
    var compactWidth: CGFloat {
        effectiveRailWidth
    }
    /// Container hosting the rail; shown only in compact mode.
    private let railScrollView = SidebarScrollView()
    private let railStack = NSStackView()
    /// Action shortcuts retain their bottom dock independently of the toggle.
    private let railFooterScrollView = SidebarScrollView()
    private let railFooterStack = NSStackView()
    /// Rail buttons keyed by a composite id ("app:<appId>" / "web:<tabId>").
    private var railButtons: [String: NSButton] = [:]
    private let railTooltip = SidebarRailTooltip()
    /// Names the hovered footer action above it.
    private let footerTooltip = SidebarRailTooltip()
    private var footerTooltipTask: Task<Void, Never>?
    private var railPagePanel: SidebarRailPagePanel?
    private weak var railHoverButton: NSButton?
    /// Identifies which rail icon the hover float belongs to, so re-entering the
    /// same icon keeps the open float instead of rebuilding it.
    private var railHoverKey: String?
    private var railHoverDismissTask: Task<Void, Never>?
    private var isRailHoverPanelHovered = false

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

    private static func sidebarActionIcon(_ url: URL?, size: CGFloat) -> NSImage? {
        guard let url, let image = TabBarHelper.chromeIconImage(contentsOf: url) else { return nil }
        return TabBarHelper.appKitIcon(image, path: url.path, size: size)
    }

    /// Called when a panel icon button is clicked: (panelId)
    var onPanelItemToggled: ((UInt64, String, SidebarActionSource) -> Void)?

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

    /// Expanded header buttons align with the measured window-control center.
    var windowControlsCenterYFromTop: CGFloat = Layout.trafficLightsHeight / 2 {
        didSet {
            guard oldValue != windowControlsCenterYFromTop else { return }
            updateControlRowGeometry()
        }
    }
    private var buttonCenterYConstraints: [NSLayoutConstraint] = []
    private var railExpandCenterYConstraint: NSLayoutConstraint?

    var isFullyHidden: Bool {
        return frame.width < Layout.fullyHiddenThreshold
    }

    /// Called when user selects a page: (appId, itemIndex)
    var onAppPageSelected: ((String, Int) -> Void)?
    /// Called when user clicks an lxapp's name (group header): (appId) — switch
    /// the main to that lxapp even if it has no tabBar pages.
    var onAppSelected: ((String) -> Void)?
    /// A click on the active group's header. Returns true when it was consumed
    /// as an intent to see main (a maximized aside went back to its dock), so
    /// the group does not also collapse.
    var onActiveGroupHeaderClicked: (() -> Bool)?
    /// Called when user requests to close an app: (appId)
    var onAppCloseRequested: ((String) -> Void)?
    var onManagedMainContextMenuRequested: ((String, NSEvent, NSView) -> Bool)?
    var onManagedMainRenameCommitted: ((String, String) -> Void)?
    /// Called when the header collapse button is clicked
    var onHideRequested: (() -> Void)?
    /// Called when the rail expand control pins the sidebar, from the rail or a peek
    var onShowRequested: (() -> Void)?
    /// Hovering the rail expand control (or clicking it where pinning is
    /// unavailable) asks the shell to present the peek.
    var onPeekRequested: (() -> Void)?
    var onPeekDismissRequested: (() -> Void)?
    /// Called when width changes via drag: (width, animated)
    var onWidthChanged: ((CGFloat, Bool) -> Void)?
    /// Called when the global "+" button requests content for the active main.
    var onAddRequested: (() -> Void)?
    /// An aside tab was dropped on the sidebar. Returns whether it became a main.
    var onAsideTabDropped: ((AsideTabDragPayload) -> Bool)?
    private let asideDropHighlight = SidebarAsideDropHighlightView()
    /// Called when a browser tab is selected
    var onBrowserTabSelected: ((String) -> Void)?
    /// Called when a browser tab close is requested
    var onBrowserTabCloseRequested: ((String) -> Void)?
    /// Called when all browser tabs except the given tab should close.
    var onBrowserTabCloseOtherRequested: ((String) -> Void)?
    /// Called when browser tabs visually below the given tab should close.
    var onBrowserTabCloseTabsBelowRequested: ((String) -> Void)?
    /// Move a main-area browser tab into the side panel, keeping its page.
    var onBrowserTabMoveToSidePanelRequested: ((String) -> Void)?
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
        footerSeparator.layer?.backgroundColor = themeCGColor(SidebarActionChromePalette.divider)
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
        headerActionStack.spacing = Layout.actionButtonSpacing
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
        footerSeparator.layer?.backgroundColor = themeCGColor(SidebarActionChromePalette.divider)
        footerView.addSubview(footerSeparator)

        panelFlow.translatesAutoresizingMaskIntoConstraints = false
        panelFlow.onWidthChanged = { [weak self] in self?.updateSidebarActionFooterHeight() }
        // Short icon+title cells share a visual row; cells wrap as a whole only
        // when the next minimum width no longer fits. The document remains
        // taller than the capped footer so overflow scrolls internally.
        panelScroll.translatesAutoresizingMaskIntoConstraints = false
        panelScroll.drawsBackground = false
        panelScroll.hasVerticalScroller = true
        panelScroll.autohidesScrollers = true
        // Keep the scrollbar out of the flow width. A legacy scroller reserves
        // enough space to force Terminal + Ping onto separate rows at compact widths.
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
        hideButton.toolTip = L10n.string("lx_sidebar_collapse")
        hideButton.setAccessibilityLabel(L10n.string("lx_sidebar_collapse"))
        hideButton.target = self
        hideButton.action = #selector(hideButtonClicked)
        headerView.addSubview(hideButton)

        // The fixed header keeps the toggle reachable while navigation scrolls.
        railExpandButton.translatesAutoresizingMaskIntoConstraints = false
        railExpandButton.isBordered = false
        railExpandButton.bezelStyle = .regularSquare
        railExpandButton.imagePosition = .imageOnly
        railExpandButton.imageScaling = .scaleProportionallyDown
        railExpandButton.wantsLayer = true
        // Rail item metrics: it reads as the first rail tile, not a header button.
        railExpandButton.layer?.cornerRadius = 8
        railExpandButton.layer?.backgroundColor = NSColor.clear.cgColor
        railExpandButton.setAccessibilityLabel(L10n.string("lx_sidebar_expand"))
        updateRailExpandTooltip()
        railExpandButton.contentTintColor = LxAppHostTheme.mutedForeground
        railExpandButton.image = LxIcon.image(
            named: "icon_sidebar_expand",
            size: NSSize(width: Layout.railIconSize, height: Layout.railIconSize))
        railExpandButton.target = self
        railExpandButton.action = #selector(railExpandClicked)
        headerView.addSubview(railExpandButton)
        let railExpandCenterX = railExpandButton.centerXAnchor.constraint(equalTo: leadingAnchor)
        railExpandCenterX.isActive = true
        railExpandCenterXConstraint = railExpandCenterX
        NSLayoutConstraint.activate([
            railExpandButton.widthAnchor.constraint(equalToConstant: Layout.railButtonSize),
            railExpandButton.heightAnchor.constraint(equalToConstant: Layout.railButtonSize),
            railFooterScrollView.topAnchor.constraint(equalTo: footerSeparator.bottomAnchor, constant: 6),
            railFooterScrollView.leadingAnchor.constraint(equalTo: footerView.leadingAnchor),
            railFooterScrollView.trailingAnchor.constraint(
                equalTo: footerView.trailingAnchor, constant: -Layout.resizeHandleWidth),
            railFooterScrollView.bottomAnchor.constraint(equalTo: footerView.bottomAnchor, constant: -6),
        ])

        // Resize handle on right edge
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false
        resizeHandle.wantsLayer = true
        addSubview(resizeHandle)

        headerHeightConstraint = headerView.heightAnchor.constraint(
            equalToConstant: Layout.trafficLightsHeight)

        NSLayoutConstraint.activate([
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerHeightConstraint!,

            hideButton.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -8),

            headerActionStack.trailingAnchor.constraint(
                equalTo: hideButton.leadingAnchor, constant: -Layout.actionButtonSpacing),
            headerActionLeadingClearance(),

            // Scroll view: inset trailing by resize handle width, extends above footer
            scrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            scrollView.bottomAnchor.constraint(equalTo: footerView.topAnchor),

            railScrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
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

        let centerY = controlRowCenterY
        let headerActionsCenter = headerActionStack.centerYAnchor.constraint(
            equalTo: headerView.topAnchor, constant: centerY)
        let toggleCenter = hideButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        let expandCenter = railExpandButton.centerYAnchor.constraint(equalTo: headerView.topAnchor, constant: centerY)
        buttonCenterYConstraints = [headerActionsCenter, toggleCenter]
        railExpandCenterYConstraint = expandCenter
        NSLayoutConstraint.activate(buttonCenterYConstraints + [expandCenter])
        updateControlRowGeometry()

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

        asideDropHighlight.translatesAutoresizingMaskIntoConstraints = false
        addSubview(asideDropHighlight)
        NSLayoutConstraint.activate([
            asideDropHighlight.topAnchor.constraint(equalTo: topAnchor),
            asideDropHighlight.leadingAnchor.constraint(equalTo: leadingAnchor),
            asideDropHighlight.trailingAnchor.constraint(equalTo: trailingAnchor),
            asideDropHighlight.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        registerForDraggedTypes([.lxAsideTab])
    }

    // MARK: - Aside Tab Drop

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        updateAsideDropTarget(sender)
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        updateAsideDropTarget(sender)
    }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        asideDropHighlight.targeted = false
    }

    override func draggingEnded(_ sender: any NSDraggingInfo) {
        asideDropHighlight.targeted = false
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        asideDropHighlight.targeted = false
        guard let payload = AsideTabDragPayload.read(from: sender.draggingPasteboard) else {
            return false
        }
        return onAsideTabDropped?(payload) ?? false
    }

    private func updateAsideDropTarget(_ sender: any NSDraggingInfo) -> NSDragOperation {
        let accepts = onAsideTabDropped != nil && !isFullyHidden
            && AsideTabDragPayload.read(from: sender.draggingPasteboard) != nil
        asideDropHighlight.targeted = accepts
        return accepts ? .move : []
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

    /// The rail's control row, below the window controls. A peek keeps it, so
    /// the expand control under the pointer stays put.
    private var controlRowCenterY: CGFloat {
        guard isCompact else { return windowControlsCenterYFromTop }
        let captionHeight = controlsAlignedToTop
            ? 0 : max(Layout.trafficLightsHeight, windowControlsCenterYFromTop + Layout.trafficLightsHeight / 2)
        return captionHeight + Layout.controlRowHeight / 2
    }

    private func updateControlRowGeometry() {
        let centerY = controlRowCenterY
        // Header actions and the collapse toggle stay on the window-control
        // row, where a pinned sidebar draws them; only the expand control
        // drops to the rail's row.
        buttonCenterYConstraints.forEach { $0.constant = windowControlsCenterYFromTop }
        railExpandCenterYConstraint?.constant = centerY
        headerHeightConstraint?.constant = isCompact
            ? centerY + Layout.controlRowHeight / 2 : Layout.trafficLightsHeight
        // Rail controls share the rail's axis, which excludes the resize handle.
        railExpandCenterXConstraint?.constant = (effectiveRailWidth - Layout.resizeHandleWidth) / 2
    }

    /// Frameless hosts omit the rail window-control clearance.
    func setControlsAlignedToTop(_ alignedToTop: Bool) {
        controlsAlignedToTop = alignedToTop
        updateControlRowGeometry()
    }

    func setCompactMode(_ compact: Bool) {
        guard compact != isCompact else { return }
        isCompact = compact
        updateControlRowGeometry()
        if compact {
            rebuildRail()
        } else {
            closeRailHoverPanel()
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
    /// Redraw everything that shows an lxapp's registry-owned name or icon.
    /// Called when a refresh lands; the records are already cached, so this is
    /// a repaint, not a fetch.
    func reloadLxAppPresentation() {
        rebuildRail()
        rebuildAppGroups()
        // `rebuildAppGroups` only writes the model. Existing group views keep
        // the name/icon they loaded on creation unless refreshed; `render`
        // then relayouts the list and pin tiles against that model.
        if !appUIOnlyMode {
            for view in groupViews.values {
                view.refreshFromRust()
            }
        }
        render()
    }

    private func rebuildRail() {
        closeRailHoverPanel()
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
                let iconPath = getLxAppDisplayIconPath(pin.key).toString()
                let image = LxIcon.lxappImage(appId: pin.key, path: iconPath)
                let name = info.app_name.toString()
                let key = "pin-lxapp:\(pin.key)"
                let button = makeRailButton(
                    key: key,
                    tooltip: name.isEmpty ? pin.key : name,
                    image: image,
                    isTemplate: false,
                    roundRaster: true
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
                let iconAppId = group.contentAppId ?? group.appId
                image = group.managedIcon
                    ?? LxIcon.lxappImage(appId: iconAppId, path: "")
            } else {
                let info = getLxAppInfo(group.appId)
                let iconPath = getLxAppDisplayIconPath(group.appId).toString()
                tooltip = info.app_name.toString()
                image = LxIcon.lxappImage(appId: group.appId, path: iconPath)
            }
            let key = "app:\(group.appId)"
            let isTemplate = group.isManagedMain && group.contentAppId == nil
            let btn = makeRailButton(
                key: key,
                tooltip: tooltip,
                image: image,
                // Native/browser glyphs are tintable. Lxapp provider assets are
                // full-color artwork; treating their opaque tile as a template
                // turns the entire icon into a flat white/gray square.
                isTemplate: isTemplate,
                roundRaster: !isTemplate
            )
            btn.action = #selector(railAppClicked(_:))
            if let railButton = btn as? SidebarRailButton {
                railButton.closable = group.closable && activeRailKey == key
                railButton.onCloseRequested = { [weak self] in
                    self?.closeRailHoverPanel()
                    self?.onAppCloseRequested?(group.appId)
                }
                let surfaceId = group.appId
                railButton.onContextMenuRequested = { [weak self] event, button in
                    self?.closeRailHoverPanel()
                    _ = self?.onManagedMainContextMenuRequested?(surfaceId, event, button)
                }
                // Same gate as the expanded group: a managed main only offers
                // its pages when it actually hosts an lxapp, and the tabBar
                // belongs to that lxapp, not to the surface.
                let tabProviderAppId = !group.isManagedMain || group.showsLxappTabBar
                    ? (group.contentAppId ?? group.appId)
                    : nil
                railButton.onHoverChanged = { [weak self, weak railButton] hovering in
                    guard let self, let railButton else { return }
                    if hovering {
                        self.showRailHoverPanel(
                            key: key,
                            label: tooltip,
                            selectionAppId: group.appId,
                            tabProviderAppId: tabProviderAppId,
                            relativeTo: railButton
                        )
                    } else {
                        self.scheduleRailHoverDismiss()
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
                    self?.closeRailHoverPanel()
                    self?.onBrowserTabCloseRequested?(item.id)
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
        // icon remains; the label moves to the tooltip. The header row has no
        // room in the rail, so its actions (Settings, Downloads) lead the same
        // stack instead of becoming unreachable.
        let railActions = headerActionItems.map { ("sidebar-header-action:", $0) }
            + model.panelItems.map { ("sidebar-action:", $0) }
        for (prefix, item) in railActions {
            let iconPath = item.iconURL?.path ?? ""
            let image = Self.sidebarActionIcon(item.iconURL, size: Layout.railIconSize)
                ?? item.systemImageName.flatMap {
                    NSImage(systemSymbolName: $0, accessibilityDescription: item.label)
                }
            let key = prefix + item.id
            let isTemplate = TabBarHelper.isTemplateIcon(iconPath)
            let button = makeRailButton(
                key: key,
                tooltip: item.tooltipText,
                image: image,
                isTemplate: isTemplate,
                // Brand/PNG tiles match switcher rounding; SVG glyphs stay
                // unclipped so the shell can tint them.
                roundRaster: !isTemplate
            )
            button.action = #selector(railSidebarActionClicked(_:))
            button.setAccessibilityLabel(item.label)
            button.setAccessibilityValue(item.distinctValue)
            button.isEnabled = !item.disabled
            railFooterStack.addArrangedSubview(button)
            railButtons[key] = button
        }

        refreshRailHighlight()
    }

    private func makeRailButton(
        key: String,
        tooltip: String,
        image: NSImage?,
        isTemplate: Bool,
        roundRaster: Bool = false
    ) -> NSButton {
        let btn = SidebarRailButton()
        btn.translatesAutoresizingMaskIntoConstraints = false
        btn.isBordered = false
        btn.bezelStyle = .regularSquare
        btn.imagePosition = .imageOnly
        btn.imageScaling = .scaleProportionallyDown
        btn.wantsLayer = true
        btn.layer?.cornerRadius = 8
        btn.layer?.backgroundColor = NSColor.clear.cgColor
        // No `toolTip`: the hover panel below is the label, and the system
        // tooltip would arrive later and repeat it.
        btn.setAccessibilityLabel(tooltip)
        btn.target = self
        btn.identifier = NSUserInterfaceItemIdentifier(key)
        // Every icon-only switcher names itself on hover; the app-group rows
        // replace this with a panel that also carries their pages.
        btn.onHoverChanged = { [weak self, weak btn] hovering in
            guard let self, let btn else { return }
            if hovering {
                self.showRailHoverPanel(
                    key: key,
                    label: tooltip,
                    selectionAppId: key,
                    tabProviderAppId: nil,
                    relativeTo: btn
                )
            } else {
                self.scheduleRailHoverDismiss()
            }
        }
        if let image {
            // App tiles are square PNGs the shell rounds. Favicons and template
            // glyphs stay unclipped so a site mark is not given an app-icon mask.
            if isTemplate {
                btn.image = TabBarHelper.appKitIcon(image, path: "SF:circle", size: Layout.railIconSize)
                btn.contentTintColor = LxAppHostTheme.mutedForeground
            } else if roundRaster {
                btn.image = TabBarHelper.appTileIcon(image, size: Layout.railIconSize)
            } else {
                let copy = image.copy() as? NSImage ?? image
                copy.size = NSSize(width: Layout.railIconSize, height: Layout.railIconSize)
                copy.isTemplate = false
                btn.image = copy
            }
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
                ? btn.themeCGColor(LxAppHostTheme.selectionBackground)
                : NSColor.clear.cgColor
        }
    }

    @objc private func railAppClicked(_ sender: NSButton) {
        guard let key = sender.identifier?.rawValue, key.hasPrefix("app:") else { return }
        let appId = String(key.dropFirst(4))
        let providerId = model.appGroups.first(where: { $0.appId == appId })?.contentAppId ?? appId
        let index = getTabBar(providerId).map { Int($0.selected_index) } ?? 0
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
        closeRailHoverPanel()
        guard let key = sender.identifier?.rawValue, key.hasPrefix("web:") else { return }
        onBrowserTabSelected?(String(key.dropFirst(4)))
    }

    @objc private func railSidebarActionClicked(_ sender: NSButton) {
        closeRailHoverPanel()
        guard let key = sender.identifier?.rawValue else { return }
        let item: PanelIconItem?
        if key.hasPrefix("sidebar-header-action:") {
            let id = String(key.dropFirst("sidebar-header-action:".count))
            item = headerActionItems.first(where: { $0.id == id })
        } else if key.hasPrefix("sidebar-action:") {
            let id = String(key.dropFirst("sidebar-action:".count))
            item = model.panelItems.first(where: { $0.id == id })
        } else {
            item = nil
        }
        guard let item else { return }
        onPanelItemToggled?(item.generation, item.id, item.source)
    }

    /// The browser tab's "···" and right-click menu: the core's main-area
    /// row menu (page actions, Open in Side Panel, the close family), run
    /// here over the sidebar's tabs.
    private func browserContextMenu(for id: String) -> NSMenu? {
        closeRailHoverPanel()
        guard let snapshot = SurfaceMenuBridge.browserTabSnapshot(
            orderedTabIds: model.browserTabs.map(\.id),
            tabId: id
        ) else { return nil }
        let menu = SurfaceMenuPresenter.makeMenu(snapshot) { [weak self] _, tabId, action in
            self?.performBrowserTabMenuAction(action, tabId: tabId)
        }
        return menu.items.isEmpty ? nil : menu
    }

    private func performBrowserTabMenuAction(_ action: SurfaceMenuSnapshot.Item.Action, tabId: String) {
        switch (action.owner, action.action) {
        case ("browser", "pin"), ("browser", "unpin"):
            toggleBrowserTabPin(tabId)
        case ("browser", "copyLink"):
            guard let tab = model.browserTabs.first(where: { $0.id == tabId }) else { return }
            BrowserPageMenu.copyLink(tab.url, toastHost: self)
        case ("browser", "openInSide"):
            onBrowserTabMoveToSidePanelRequested?(tabId)
        case ("switcher", "close"):
            onBrowserTabCloseRequested?(tabId)
        case ("switcher", "closeOthers"):
            onBrowserTabCloseOtherRequested?(tabId)
        case ("switcher", "closeAfter"):
            onBrowserTabCloseTabsBelowRequested?(tabId)
        default:
            break
        }
    }

    private func toggleBrowserTabPin(_ id: String) {
        guard let tab = model.browserTabs.first(where: { $0.id == id }) else { return }
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

    /// Hover affordance for one collapsed-rail icon: the page panel when the
    /// icon is an lxapp offering a visible tabBar, otherwise the plain hint.
    /// Only ever one of the two, so a single anchor never sprouts both.
    private func showRailHoverPanel(
        key: String,
        label: String,
        selectionAppId: String,
        tabProviderAppId: String?,
        relativeTo button: NSButton
    ) {
        let label = label.isEmpty ? selectionAppId : label
        guard showsRail, !button.isHidden, button.window != nil else {
            closeRailHoverPanel()
            return
        }

        var items: [TabBarItem] = []
        var selectedIndex = 0
        // Desktop does not auto-hide tab pages on a detail route; only an
        // explicit API hide should drop the page list from the hover panel.
        if let tabProviderAppId,
           let tabBar = getTabBar(tabProviderAppId),
           tabBar.items_count > 0,
           !tabBar.is_api_hidden {
            items = tabBar.getItems(appId: tabProviderAppId)
            selectedIndex = Int(tabBar.selected_index)
        }
        guard !items.isEmpty else {
            showRailTooltip(key: key, label: label, relativeTo: button)
            return
        }
        // The panel is about the lxapp that owns those pages, so it carries
        // that app's own name — a managed main's surface label may just be its id.
        let providerName = tabProviderAppId
            .map { getLxAppInfo($0).app_name.toString() }
            .flatMap { $0.isEmpty ? nil : $0 }

        railHoverDismissTask?.cancel()
        if railHoverKey == key, railPagePanel?.isVisible == true {
            railHoverButton = button
            return
        }

        closeRailHoverPanel()

        let panel = SidebarRailPagePanel(
            selectionAppId: selectionAppId,
            providerAppId: tabProviderAppId,
            label: providerName ?? label,
            items: items,
            selectedIndex: selectedIndex
        )
        panel.onPageSelected = { [weak self] appId, index in
            self?.onAppPageSelected?(appId, index)
        }
        panel.onDismissRequested = { [weak self] in
            self?.closeRailHoverPanel()
        }
        panel.onHoverChanged = { [weak self] hovering in
            guard let self else { return }
            self.isRailHoverPanelHovered = hovering
            if hovering {
                self.railHoverDismissTask?.cancel()
            } else {
                self.scheduleRailHoverDismiss()
            }
        }

        railPagePanel = panel
        railHoverKey = key
        railHoverButton = button
        isRailHoverPanelHovered = false
        panel.present(anchor: button, railFrame: railScreenFrame())
    }

    private func showRailTooltip(key: String, label: String, relativeTo button: NSButton) {
        railHoverDismissTask?.cancel()
        if railHoverKey == key, railTooltip.isVisible {
            railHoverButton = button
            return
        }
        closeRailHoverPanel()
        railHoverKey = key
        railHoverButton = button
        railTooltip.show(text: label, anchor: button, railFrame: railScreenFrame())
    }

    /// The rail column in screen space — the edge every hover float clears.
    /// Use the scroll column, not `self.bounds`: the resize handle sits past
    /// that trailing edge, and shifting maxX would offset every rail float.
    private func railScreenFrame() -> NSRect {
        guard let window else { return .zero }
        return window.convertToScreen(railScrollView.convert(railScrollView.bounds, to: nil))
    }

    private func scheduleRailHoverDismiss() {
        railHoverDismissTask?.cancel()
        railHoverDismissTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 250_000_000)
            guard !Task.isCancelled, let self else { return }
            guard !self.isRailHoverPanelHovered, !self.isMouseInsideRailHoverButton() else { return }
            self.closeRailHoverPanel()
        }
    }

    private func isMouseInsideRailHoverButton() -> Bool {
        guard let button = railHoverButton, let window = button.window else { return false }
        let windowPoint = window.mouseLocationOutsideOfEventStream
        let buttonPoint = button.convert(windowPoint, from: nil)
        return button.bounds.insetBy(dx: -6, dy: -6).contains(buttonPoint)
    }

    private func closeRailHoverPanel() {
        railHoverDismissTask?.cancel()
        railHoverDismissTask = nil
        isRailHoverPanelHovered = false
        railHoverKey = nil
        railHoverButton = nil
        railTooltip.hide()
        railPagePanel?.dismiss()
        railPagePanel = nil
    }

    func updateVisibilityState() {
        let hidden = isFullyHidden
        let compact = showsRail && !hidden && !appUIOnlyMode
        scrollView.isHidden = hidden || appUIOnlyMode || compact
        railScrollView.isHidden = hidden || appUIOnlyMode || !compact
        // The toggle remains in the header; action shortcuts keep the footer.
        railExpandButton.isHidden = hidden || appUIOnlyMode || !isCompact
        railFooterScrollView.isHidden = hidden
            || appUIOnlyMode
            || !compact
            || (headerActionItems.isEmpty && model.panelItems.isEmpty)
        footerSeparator.isHidden = compact
        // Header actions and footer cells don't fit the rail.
        updateHeaderActionVisibility()
        // Exactly one toggle is visible in the header in either mode; a peek
        // keeps the rail's, which pins it.
        hideButton.isHidden = hidden || appUIOnlyMode || isCompact
        panelScroll.isHidden = compact
        footerView.isHidden = hidden || (compact
            ? headerActionItems.isEmpty && model.panelItems.isEmpty
            : model.panelItems.isEmpty)
        // A peek is transient: dragging it would commit a width.
        resizeHandle.isHidden = hidden || isPeeking
        // Hidden rows get no mouseExited.
        if panelScroll.isHiddenOrHasHiddenAncestor {
            footerHoverChanged(nil, hovering: false)
        }
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
        closeRailHoverPanel()
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
            let iconPath = item.iconURL?.path ?? ""
            let isTemplate = TabBarHelper.isTemplateIcon(iconPath)
            let button = SidebarHeaderActionButton()
            button.translatesAutoresizingMaskIntoConstraints = false
            button.isBordered = false
            button.bezelStyle = .regularSquare
            button.imagePosition = .imageOnly
            button.imageScaling = .scaleProportionallyDown
            button.contentTintColor = isTemplate ? LxAppHostTheme.mutedForeground : nil
            button.image = Self.sidebarActionIcon(item.iconURL, size: 16)
                ?? item.systemImageName.flatMap {
                    NSImage(systemSymbolName: $0, accessibilityDescription: item.label)
                }
            if !isTemplate, let image = button.image {
                button.image = TabBarHelper.appTileIcon(image, size: 16)
            }
            button.toolTip = item.label
            button.setAccessibilityLabel(item.label)
            button.isEnabled = !item.disabled
            headerActionIdentities[ObjectIdentifier(button)] = SidebarActionIdentity(
                generation: item.generation,
                id: item.id,
                source: item.source
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
        if isCompact {
            rebuildRail()
            updateSidebarActionFooterHeight()
        }
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
        let hidden = isFullyHidden || appUIOnlyMode || showsRail || headerActionItems.isEmpty
        headerActionStack.isHidden = hidden
        guard !hidden else { return }
        let reserve = measuredHeaderLeadingReserve()
        headerActionLeadingConstraint?.constant = reserve
        let availableWidth = max(
            0,
            bounds.width - reserve - 8 - Layout.actionButtonSize - Layout.actionButtonSpacing
        )
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
        footerHoverChanged(nil, hovering: false)
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
                value: item.value,
                tooltip: item.tooltipText,
                iconURL: item.iconURL,
                systemImageName: item.systemImageName,
                active: item.active,
                disabled: item.disabled
            )
            row.onClick = { [weak self] in
                self?.onPanelItemToggled?(item.generation, item.id, item.source)
            }
            row.onHoverChanged = { [weak self, weak row] hovering in
                self?.footerHoverChanged(row, hovering: hovering)
            }
            entries.append(row)
            panelButtons.append(row)
        }
        panelFlow.setEntries(entries)
        updateSidebarActionFooterHeight()
    }

    /// Shown after the system tooltip delay, like Windows' pinned footer.
    private func footerHoverChanged(_ row: SidebarActionRowView?, hovering: Bool) {
        footerTooltipTask?.cancel()
        guard hovering, let row else {
            footerTooltip.hide()
            return
        }
        footerTooltipTask = Task { @MainActor [weak self, weak row] in
            try? await Task.sleep(nanoseconds: 500_000_000)
            guard !Task.isCancelled, let self, let row, row.window != nil,
                  !row.isHiddenOrHasHiddenAncestor else { return }
            self.footerTooltip.show(
                text: row.tooltip,
                anchor: row,
                railFrame: .zero,
                alignment: .aboveAnchor
            )
        }
    }

    private func updateSidebarActionFooterHeight() {
        if showsRail {
            // The rail stack carries the header actions ahead of the footer ones.
            let railActions = headerActionItems.count + model.panelItems.count
            let visibleActions = min(CGFloat(railActions), Layout.footerMaxRows)
            footerHeightConstraint?.constant = visibleActions > 0
                ? visibleActions * Layout.railButtonSize
                    + max(0, visibleActions - 1) * railFooterStack.spacing
                    + Layout.railFooterChromeHeight
                : 0
            return
        }
        guard !model.panelItems.isEmpty else {
            footerHeightConstraint?.constant = 0
            return
        }
        let width = max(
            1,
            panelFlow.bounds.width > 1
                ? panelFlow.bounds.width
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
              let image = TabBarHelper.chromeIconImage(contentsOf: url) else { return }
        panelButtons[index].setIcon(image, path: url.path)
        if let railButton = railButtons["sidebar-action:\(panelId)"] {
            let template = TabBarHelper.isTemplateIcon(url.path)
            railButton.image = TabBarHelper.appKitIcon(
                image,
                path: url.path,
                size: Layout.railIconSize
            )
            railButton.contentTintColor = template ? LxAppHostTheme.mutedForeground : nil
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
        if let view = groupViews[appId] {
            view.refreshFromRust()
            return
        }
        // TabBar updates arrive with the lxapp id; a managed main's group is
        // keyed by its Surface id.
        groupViews.values.first { $0.appId == appId || $0.contentAppId == appId }?.refreshFromRust()
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
                groupView.onActiveHeaderClicked = { [weak self] in
                    self?.onActiveGroupHeaderClicked?() ?? false
                }
                groupView.onCloseRequested = { [weak self] appId in
                    self?.onAppCloseRequested?(appId)
                }
                groupView.onManagedContextMenuRequested = { [weak self] surfaceId, event, view in
                    self?.onManagedMainContextMenuRequested?(surfaceId, event, view) ?? false
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
                return browserRootHeader.themeCGColor(LxAppHostTheme.selectionBackground)
            }
            return NSColor.clear.cgColor
        }()

        // App group selection. Accordion matches Windows: only the active
        // lxapp keeps its tabbar open; a browser/native main collapses all.
        for (id, group) in groupViews {
            if case .app(let appId, let pageIndex) = model.selection, id == appId {
                let wasActive = group.isActiveGroup
                group.syncExpandedForSwitcher(isActive: true, wasActive: wasActive)
                group.isActiveGroup = true
                if let idx = pageIndex {
                    group.setActiveHighlight(pageIndex: idx)
                } else if let tabBar = getTabBar(group.contentAppId ?? group.appId) {
                    group.setActiveHighlight(pageIndex: Int(tabBar.selected_index))
                }
            } else {
                let wasActive = group.isActiveGroup
                group.syncExpandedForSwitcher(isActive: false, wasActive: wasActive)
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
        let hostIcon = LxIcon.hostAppImage()
        icon.image = hostIcon.map { TabBarHelper.appTileIcon($0, size: 16) }
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
        for id in pinnedLxappIds {
            if let existing = lxappPinTiles[id] {
                existing.refreshFromRust()
            } else {
                lxappPinTiles[id] = LxappPinTileView(appId: id)
            }
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
        addButton.layer?.backgroundColor = themeCGColor(LxAppHostTheme.foreground.withAlphaComponent(0.06))
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
        onPanelItemToggled?(identity.generation, identity.id, identity.source)
    }

    @objc private func railExpandClicked() {
        peekIntentTask?.cancel()
        if railExpandEnabled {
            onShowRequested?()
        } else if isPeeking {
            onPeekDismissRequested?()
        } else {
            // A compact window cannot pin; the click previews instead.
            onPeekRequested?()
        }
    }

    // MARK: - Peek

    private static let peekHoverDelay: UInt64 = 300_000_000
    /// The same grace the rail's hover floats use.
    private static let peekDismissGrace: UInt64 = 250_000_000

    /// Show the expanded layout over the committed rail, or return to it. The
    /// shell owns the overlay's width and stacking.
    func setPeeking(_ peeking: Bool) {
        guard peeking != isPeeking else { return }
        isPeeking = peeking
        peekIntentTask?.cancel()
        peekDismissTask?.cancel()
        if peeking {
            closeRailHoverPanel()
            observeMenuTracking()
        } else {
            peekMenuObservers.forEach(NotificationCenter.default.removeObserver)
            peekMenuObservers = []
            isMenuTracking = false
        }
        if !peeking { setRailExpandHovered(false) }
        updateControlRowGeometry()
        updateSidebarActionFooterHeight()
        updateVisibilityState()
        updateTrackingAreas()
        if peeking {
            NSAccessibility.post(element: self, notification: .layoutChanged)
        }
    }

    private func schedulePeekIntent() {
        peekIntentTask?.cancel()
        guard showsRail, railPagePanel?.isVisible != true, NSEvent.pressedMouseButtons == 0 else {
            return
        }
        peekIntentTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: Self.peekHoverDelay)
            guard !Task.isCancelled, let self, self.showsRail, self.isRailExpandHovered,
                  self.railPagePanel?.isVisible != true, NSEvent.pressedMouseButtons == 0
            else { return }
            self.onPeekRequested?()
        }
    }

    private func schedulePeekDismiss() {
        peekDismissTask?.cancel()
        peekDismissTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: Self.peekDismissGrace)
            guard !Task.isCancelled, let self, self.isPeeking,
                  !self.isPointerInside, !self.isMenuTracking, !self.isEditingInline
            else { return }
            self.onPeekDismissRequested?()
        }
    }

    private var isPointerInside: Bool {
        guard let window else { return false }
        let point = convert(window.mouseLocationOutsideOfEventStream, from: nil)
        return bounds.contains(point)
    }

    /// A rename field inside the peek keeps it open until the edit ends.
    private var isEditingInline: Bool {
        guard let editor = window?.firstResponder as? NSText,
              let field = editor.delegate as? NSView
        else { return false }
        return field.isDescendant(of: self)
    }

    /// A context menu opened from the peek runs its own tracking loop, during
    /// which the pointer is outside; the peek waits for it.
    private func observeMenuTracking() {
        let center = NotificationCenter.default
        peekMenuObservers = [
            center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: .main) {
                [weak self] _ in
                MainActor.assumeIsolated { self?.isMenuTracking = true }
            },
            center.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: .main) {
                [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    self.isMenuTracking = false
                    if self.isPeeking, !self.isPointerInside { self.schedulePeekDismiss() }
                }
            },
        ]
    }

    /// Names what a click does; a compact window's click only previews.
    private func updateRailExpandTooltip() {
        railExpandButton.toolTip = railExpandEnabled ? L10n.string("lx_sidebar_expand") : nil
    }

    private func setRailExpandHovered(_ hovered: Bool) {
        isRailExpandHovered = hovered
        railExpandButton.layer?.backgroundColor = hovered
            ? themeCGColor(LxAppHostTheme.foreground.withAlphaComponent(0.09))
            : NSColor.clear.cgColor
        railExpandButton.contentTintColor = hovered
            ? LxAppHostTheme.foreground
            : LxAppHostTheme.mutedForeground
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

        if let existing = railExpandTrackingArea {
            railExpandButton.removeTrackingArea(existing)
        }
        let expandArea = NSTrackingArea(
            rect: railExpandButton.bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        railExpandButton.addTrackingArea(expandArea)
        railExpandTrackingArea = expandArea

        if let existing = peekTrackingArea {
            removeTrackingArea(existing)
            peekTrackingArea = nil
        }
        if isPeeking {
            let area = NSTrackingArea(
                rect: bounds,
                options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
                owner: self,
                userInfo: nil
            )
            addTrackingArea(area)
            peekTrackingArea = area
        }
    }

    override public func mouseEntered(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(true)
        } else if event.trackingArea === hideButtonTrackingArea {
            setHideButtonHovered(true)
        } else if event.trackingArea === railExpandTrackingArea {
            setRailExpandHovered(true)
            schedulePeekIntent()
        } else if event.trackingArea === peekTrackingArea {
            peekDismissTask?.cancel()
        }
    }

    override public func mouseExited(with event: NSEvent) {
        if event.trackingArea === addButtonTrackingArea {
            setAddButtonHovered(false)
        } else if event.trackingArea === hideButtonTrackingArea {
            setHideButtonHovered(false)
        } else if event.trackingArea === railExpandTrackingArea {
            setRailExpandHovered(false)
            if !isPeeking { peekIntentTask?.cancel() }
        } else if event.trackingArea === peekTrackingArea {
            schedulePeekDismiss()
        }
    }

    private func setAddButtonHovered(_ hovered: Bool) {
        isAddButtonHovered = hovered
        updateAddButtonAppearance()
    }

    private func updateAddButtonAppearance() {
        let alpha: CGFloat = isAddButtonHovered ? 0.12 : 0.06
        addButton.layer?.backgroundColor = themeCGColor(LxAppHostTheme.foreground.withAlphaComponent(alpha))
    }

    private func setHideButtonHovered(_ hovered: Bool) {
        isHideButtonHovered = hovered
        updateHideButtonAppearance()
    }

    private func updateHideButtonAppearance() {
        hideButton.layer?.backgroundColor = isHideButtonHovered
            ? themeCGColor(LxAppHostTheme.foreground.withAlphaComponent(0.09))
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

/// Packs footer actions in declaration order. Icon buttons share rows; a value
/// cell joins the icons before it when its minimum fits (or when a single icon
/// precedes it, which never stands alone), takes the rest of that row, and
/// ends it.
@MainActor
private final class SidebarActionFlowView: NSView {
    /// Between rows.
    static let gap: CGFloat = 4
    /// Between icon buttons in a row; the header's action spacing.
    private static let iconGap: CGFloat = 2
    private static let rowHeight: CGFloat = 30

    private var entries: [SidebarActionRowView] = []
    private var lastLayoutWidth: CGFloat = 0
    /// The row count follows the width, and the flow learns its width after
    /// the sidebar's own layout pass has sized the footer.
    var onWidthChanged: (() -> Void)?

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
            onWidthChanged?()
        }
        var y: CGFloat = 0
        for range in rowRanges(for: width) {
            var x: CGFloat = 0
            for entry in entries[range] {
                let cellWidth = entry.isValueCell ? width - x : min(Self.rowHeight, width - x)
                entry.frame = NSRect(
                    x: x,
                    y: y,
                    width: max(1, cellWidth),
                    height: Self.rowHeight
                )
                x += cellWidth + Self.iconGap
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
            let cell = min(
                entries[index].isValueCell ? entries[index].minimumWidth : Self.rowHeight,
                available
            )
            let next = index == start ? cell : used + Self.iconGap + cell
            let keepsLoneIcon = entries[index].isValueCell && index - start == 1
            if index > start && next > available && !keepsLoneIcon {
                rows.append(start..<index)
                start = index
                used = cell
            } else {
                used = next
            }
            if entries[index].isValueCell {
                rows.append(start..<(index + 1))
                start = index + 1
                used = 0
            }
        }
        if start < entries.count {
            rows.append(start..<entries.count)
        }
        return rows
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

/// One footer action: a square icon button, or — when it carries a value — a
/// left-aligned icon + value cell sharing the tabbar items' rhythm (30pt, hover
/// wash). A custom view because a borderless NSButton centers its image+title
/// block and cannot left-align it.
@MainActor
final class SidebarActionRowView: NSView {
    private static let iconSize: CGFloat = 18
    private static let cellHeight: CGFloat = 30

    var onClick: (() -> Void)?
    /// The sidebar draws the name above the cell; the system tooltip opens
    /// below the cursor, past the window's bottom edge.
    var onHoverChanged: ((Bool) -> Void)?
    /// Whether this cell draws a value and so takes the rest of its row.
    let isValueCell: Bool
    /// Icon, padding and the whole value, or about two CJK characters and
    /// the ellipsis of a longer one.
    private(set) lazy var minimumWidth: CGFloat = {
        guard isValueCell else { return Self.cellHeight }
        let whole = titleLabel.intrinsicContentSize.width
        let shortest = NSTextField(labelWithString: "\u{4E00}\u{4E00}\u{2026}")
        shortest.font = titleLabel.font
        let chrome = (Self.cellHeight - Self.iconSize) / 2 + Self.iconSize + 8 + 8
        return chrome + ceil(min(whole, shortest.intrinsicContentSize.width))
    }()

    private let iconView = NSImageView()
    private let titleLabel: NSTextField
    private let active: Bool
    private let disabled: Bool
    let tooltip: String
    private var iconIsTemplate = true
    private var isHovered = false { didSet { updateAppearance() } }
    private var isPressed = false { didSet { updateAppearance() } }
    private var tracking: NSTrackingArea?

    private let washView = NSView()
    private let accentView = NSView()

    init(
        label: String,
        value: String?,
        tooltip: String,
        iconURL: URL?,
        systemImageName: String? = nil,
        active: Bool,
        disabled: Bool
    ) {
        titleLabel = NSTextField(labelWithString: value ?? "")
        isValueCell = value != nil
        self.active = active && !disabled
        self.disabled = disabled
        self.tooltip = tooltip
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel(label)
        setAccessibilityValue(value == label ? nil : value)

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
        accentView.layer?.backgroundColor = themeCGColor(LxAppHostTheme.accent)
        accentView.isHidden = !self.active
        addSubview(accentView)

        let icon = iconURL.flatMap { TabBarHelper.chromeIconImage(contentsOf: $0) }
            ?? systemImageName.flatMap {
                NSImage(systemSymbolName: $0, accessibilityDescription: label)
            }
        iconView.imageScaling = .scaleProportionallyDown
        iconView.wantsLayer = true
        iconView.alphaValue = disabled ? 0.42 : (self.active ? 1 : 0.82)
        iconView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(iconView)
        if let icon {
            setIcon(icon, path: iconURL?.path)
        }

        titleLabel.font = NSFont.systemFont(
            ofSize: 13,
            weight: self.active ? .medium : .regular
        )
        titleLabel.textColor = disabled
            ? LxAppHostTheme.mutedForeground
            : (self.active ? LxAppHostTheme.accent : SidebarActionChromePalette.mutedText)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.isHidden = !isValueCell
        addSubview(titleLabel)

        // A value cell's icon sits where an icon button's would, so icons
        // stay on one column across rows.
        let iconPlacement = isValueCell
            ? iconView.leadingAnchor.constraint(
                equalTo: leadingAnchor,
                constant: (Self.cellHeight - Self.iconSize) / 2
            )
            : iconView.centerXAnchor.constraint(equalTo: centerXAnchor)
        NSLayoutConstraint.activate([
            washView.leadingAnchor.constraint(equalTo: leadingAnchor),
            washView.trailingAnchor.constraint(equalTo: trailingAnchor),
            washView.topAnchor.constraint(equalTo: topAnchor),
            washView.bottomAnchor.constraint(equalTo: bottomAnchor),
            accentView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            accentView.centerYAnchor.constraint(equalTo: centerYAnchor),
            accentView.widthAnchor.constraint(equalToConstant: 2),
            accentView.heightAnchor.constraint(equalToConstant: 18),
            iconPlacement,
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: Self.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: Self.iconSize),
            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: 8),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: washView.trailingAnchor, constant: -8),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        setAccessibilityEnabled(!disabled)
        updateAppearance()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func setIcon(_ image: NSImage, path: String? = nil) {
        let iconPath = path ?? ""
        iconIsTemplate = TabBarHelper.isTemplateIcon(iconPath)
        iconView.image = iconIsTemplate
            ? TabBarHelper.appKitIcon(image, path: iconPath, size: Self.iconSize)
            : TabBarHelper.appTileIcon(image, size: Self.iconSize)
        iconView.layer?.cornerRadius = iconIsTemplate ? 0 : Self.iconSize * TabBarHelper.appTileCornerRatio
        iconView.layer?.masksToBounds = !iconIsTemplate
        iconView.contentTintColor = iconIsTemplate ? SidebarActionChromePalette.mutedText : nil
    }

    private func updateAppearance() {
        accentView.layer?.backgroundColor = themeCGColor(LxAppHostTheme.accent)
        titleLabel.textColor = disabled
            ? LxAppHostTheme.mutedForeground
            : (active ? LxAppHostTheme.accent : SidebarActionChromePalette.mutedText)
        iconView.contentTintColor = iconIsTemplate
            ? (active ? LxAppHostTheme.accent : SidebarActionChromePalette.mutedText)
            : nil
        if isPressed && !disabled {
            washView.layer?.backgroundColor = themeCGColor(SidebarActionChromePalette.pressed)
        } else if active {
            washView.layer?.backgroundColor = themeCGColor(SidebarActionChromePalette.activeSurface)
        } else if isHovered && !disabled {
            washView.layer?.backgroundColor = themeCGColor(SidebarActionChromePalette.hover)
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

    override func mouseEntered(with event: NSEvent) {
        if !disabled { isHovered = true }
        onHoverChanged?(true)
    }
    override func mouseExited(with event: NSEvent) {
        isHovered = false
        isPressed = false
        onHoverChanged?(false)
    }
    override func mouseDown(with event: NSEvent) {
        onHoverChanged?(false)
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
