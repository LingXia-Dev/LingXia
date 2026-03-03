#if os(macOS)
import AppKit
import CLingXiaRustAPI

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

// MARK: - SidebarView

/// The main sidebar container view, modeled after Chrome vertical tab groups.
/// Supports drag-to-resize and a minimized mode with colored dots.
/// In minimized mode, clicking a dot selects the tab; clicking elsewhere expands.
@MainActor
public class SidebarView: NSView {

    struct Layout {
        static let expandedWidth: CGFloat = 180
        static let minCollapsedWidth: CGFloat = 48
        static let maxWidth: CGFloat = 400
        static let collapseThreshold: CGFloat = 80
        static let trafficLightsHeight: CGFloat = 38
        static let toggleButtonSize: CGFloat = 28
        static let resizeHandleWidth: CGFloat = 5
        static let dotDiameter: CGFloat = 12
        static let dotSpacing: CGFloat = 16
        static let dotTopOffset: CGFloat = 50
    }

    private let backgroundView = NSVisualEffectView()
    private let headerView = NSView()
    private let toggleButton = NSButton()
    private let scrollView = NSScrollView()
    private let resizeHandle = SidebarResizeHandle()
    private let minimizedDotsContainer = NSView()

    private var groupViews: [String: SidebarGroupView] = [:]
    private var dotViews: [(appId: String, dot: NSView)] = []

    /// True when the sidebar is at minimized width (showing dots only)
    var isMinimized: Bool {
        return frame.width <= Layout.minCollapsedWidth + 1
    }

    /// Called when user selects a page: (appId, itemIndex)
    var onAppPageSelected: ((String, Int) -> Void)?
    /// Called when user requests to close an app: (appId)
    var onAppCloseRequested: ((String) -> Void)?
    /// Called when toggle button is clicked or minimized area is clicked
    var onToggleRequested: (() -> Void)?
    /// Called when width changes via drag: (width, animated)
    var onWidthChanged: ((CGFloat, Bool) -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupViews()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // Prevent window drag when SidebarView itself receives events (minimized mode)
    override public var mouseDownCanMoveWindow: Bool { false }

    // MARK: - Hit Testing (minimized mode)

    override public func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, frame.contains(point) else { return nil }

        if isMinimized {
            // In minimized mode, only dots and resize handle receive direct events.
            // Everything else returns self → mouseDown triggers expand.
            if let hit = super.hitTest(point) {
                if hit === resizeHandle || dotViews.contains(where: { $0.dot === hit }) {
                    return hit
                }
            }
            return self
        }

        return super.hitTest(point)
    }

    override public func mouseDown(with event: NSEvent) {
        if isMinimized {
            onToggleRequested?()
            return
        }
        super.mouseDown(with: event)
    }

    // MARK: - Setup

    private func setupViews() {
        wantsLayer = true
        clipsToBounds = true

        // Visual effect background (sidebar material)
        backgroundView.translatesAutoresizingMaskIntoConstraints = false
        backgroundView.material = .sidebar
        backgroundView.blendingMode = .behindWindow
        backgroundView.state = .active
        addSubview(backgroundView)

        // Header (traffic lights + toggle)
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.wantsLayer = true
        addSubview(headerView)

        // Toggle button — right-aligned in header
        toggleButton.translatesAutoresizingMaskIntoConstraints = false
        toggleButton.image = NSImage(systemSymbolName: "sidebar.left", accessibilityDescription: "Toggle sidebar")
        toggleButton.isBordered = false
        toggleButton.bezelStyle = .regularSquare
        toggleButton.imagePosition = .imageOnly
        toggleButton.contentTintColor = NSColor.secondaryLabelColor
        toggleButton.target = self
        toggleButton.action = #selector(toggleClicked)
        headerView.addSubview(toggleButton)

        // Scroll view (trailing inset to leave room for resize handle)
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        addSubview(scrollView)

        // Document view (flipped)
        let flipView = FlippedView()
        flipView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.documentView = flipView

        // Minimized dots container (hidden by default)
        minimizedDotsContainer.translatesAutoresizingMaskIntoConstraints = false
        minimizedDotsContainer.isHidden = true
        addSubview(minimizedDotsContainer)

        // Resize handle on right edge
        resizeHandle.translatesAutoresizingMaskIntoConstraints = false
        resizeHandle.wantsLayer = true
        addSubview(resizeHandle)

        NSLayoutConstraint.activate([
            backgroundView.topAnchor.constraint(equalTo: topAnchor),
            backgroundView.leadingAnchor.constraint(equalTo: leadingAnchor),
            backgroundView.trailingAnchor.constraint(equalTo: trailingAnchor),
            backgroundView.bottomAnchor.constraint(equalTo: bottomAnchor),

            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerView.heightAnchor.constraint(equalToConstant: Layout.trafficLightsHeight),

            // Toggle button: right-aligned in header
            toggleButton.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -12),
            toggleButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            toggleButton.widthAnchor.constraint(equalToConstant: Layout.toggleButtonSize),
            toggleButton.heightAnchor.constraint(equalToConstant: Layout.toggleButtonSize),

            // Scroll view: inset trailing by resize handle width
            scrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.resizeHandleWidth),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),

            minimizedDotsContainer.topAnchor.constraint(equalTo: topAnchor, constant: Layout.dotTopOffset),
            minimizedDotsContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            minimizedDotsContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            minimizedDotsContainer.bottomAnchor.constraint(equalTo: bottomAnchor),

            // Resize handle: right edge, full height
            resizeHandle.topAnchor.constraint(equalTo: topAnchor),
            resizeHandle.trailingAnchor.constraint(equalTo: trailingAnchor),
            resizeHandle.bottomAnchor.constraint(equalTo: bottomAnchor),
            resizeHandle.widthAnchor.constraint(equalToConstant: Layout.resizeHandleWidth),
        ])

        // Document view fills scroll view width
        if let docView = scrollView.documentView {
            NSLayoutConstraint.activate([
                docView.leadingAnchor.constraint(equalTo: scrollView.contentView.leadingAnchor),
                docView.trailingAnchor.constraint(equalTo: scrollView.contentView.trailingAnchor),
                docView.topAnchor.constraint(equalTo: scrollView.contentView.topAnchor),
            ])
        }

        // Separator line inside the resize handle
        let separatorLine = NSView()
        separatorLine.translatesAutoresizingMaskIntoConstraints = false
        separatorLine.wantsLayer = true
        separatorLine.layer?.backgroundColor = NSColor.separatorColor.cgColor
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
        let clamped = min(max(proposedWidth, Layout.minCollapsedWidth), Layout.maxWidth)
        onWidthChanged?(clamped, false)
    }

    private func handleDragEnd(proposedWidth: CGFloat) {
        if proposedWidth < Layout.collapseThreshold {
            onWidthChanged?(Layout.minCollapsedWidth, true)
        } else {
            let clamped = min(max(proposedWidth, Layout.collapseThreshold), Layout.maxWidth)
            onWidthChanged?(clamped, true)
        }
    }

    // MARK: - Minimized Mode

    /// Update display mode based on current width
    func updateMinimizedState() {
        let minimized = isMinimized
        scrollView.isHidden = minimized
        toggleButton.isHidden = minimized
        minimizedDotsContainer.isHidden = !minimized
    }

    /// Rebuild colored dots for current groups
    private func rebuildDots(tabs: [LxAppTab]) {
        for item in dotViews {
            item.dot.removeFromSuperview()
        }
        dotViews.removeAll()

        var yOffset: CGFloat = 0
        for (tabIndex, tab) in tabs.enumerated() {
            let palette = SidebarGroupColor.palette(for: tabIndex)
            let dot = NSView()
            dot.translatesAutoresizingMaskIntoConstraints = false
            dot.wantsLayer = true
            dot.layer?.backgroundColor = palette.headerBg.cgColor
            dot.layer?.cornerRadius = Layout.dotDiameter / 2
            minimizedDotsContainer.addSubview(dot)

            NSLayoutConstraint.activate([
                dot.centerXAnchor.constraint(equalTo: minimizedDotsContainer.centerXAnchor),
                dot.topAnchor.constraint(equalTo: minimizedDotsContainer.topAnchor, constant: yOffset),
                dot.widthAnchor.constraint(equalToConstant: Layout.dotDiameter),
                dot.heightAnchor.constraint(equalToConstant: Layout.dotDiameter),
            ])

            let clickGesture = NSClickGestureRecognizer(target: self, action: #selector(dotClicked(_:)))
            dot.addGestureRecognizer(clickGesture)

            dotViews.append((appId: tab.appId, dot: dot))
            yOffset += Layout.dotDiameter + Layout.dotSpacing
        }
    }

    @objc private func dotClicked(_ gesture: NSClickGestureRecognizer) {
        guard let clickedDot = gesture.view else { return }
        if let match = dotViews.first(where: { $0.dot === clickedDot }) {
            onAppPageSelected?(match.appId, 0)
        }
    }

    // MARK: - Public API

    /// Rebuild all groups based on current tabs
    func updateForTabs(_ tabs: [LxAppTab], activeTab: LxAppTab?) {
        guard let docView = scrollView.documentView else { return }

        // Remove groups for apps no longer in tabs
        let currentAppIds = Set(tabs.map { $0.appId })
        for (appId, groupView) in groupViews {
            if !currentAppIds.contains(appId) {
                groupView.removeFromSuperview()
                groupViews.removeValue(forKey: appId)
            }
        }

        // Add/update groups
        var yOffset: CGFloat = 4
        for (tabIndex, tab) in tabs.enumerated() {
            let groupView: SidebarGroupView
            let isNew: Bool
            if let existing = groupViews[tab.appId] {
                groupView = existing
                isNew = false
            } else {
                groupView = SidebarGroupView(appId: tab.appId)
                groupView.onPageSelected = { [weak self] appId, itemIndex in
                    self?.onAppPageSelected?(appId, itemIndex)
                }
                groupView.onCloseRequested = { [weak self] appId in
                    self?.onAppCloseRequested?(appId)
                }
                groupViews[tab.appId] = groupView
                isNew = true
            }

            groupView.setColorIndex(tabIndex)

            if isNew || groupView.superview !== docView {
                groupView.removeFromSuperview()
                docView.addSubview(groupView)
            }

            for constraint in docView.constraints {
                if constraint.firstItem === groupView && constraint.firstAttribute == .top {
                    constraint.isActive = false
                }
            }

            NSLayoutConstraint.activate([
                groupView.topAnchor.constraint(equalTo: docView.topAnchor, constant: yOffset),
                groupView.leadingAnchor.constraint(equalTo: docView.leadingAnchor),
                groupView.trailingAnchor.constraint(equalTo: docView.trailingAnchor),
            ])

            groupView.layoutSubtreeIfNeeded()
            yOffset += groupView.fittingSize.height + 8
        }

        // Update document view height
        let docHeight = max(yOffset, scrollView.contentView.bounds.height)
        if let docView = scrollView.documentView {
            docView.frame = NSRect(x: 0, y: 0, width: docView.frame.width, height: docHeight)
        }

        if let activeAppId = activeTab?.appId {
            setActiveHighlight(appId: activeAppId)
        }

        rebuildDots(tabs: tabs)
    }

    /// Refresh a specific app group from Rust data
    func refreshAppGroup(appId: String) {
        groupViews[appId]?.refreshFromRust()
    }

    /// Set active highlight on the appropriate group and item
    func setActiveHighlight(appId: String, pageIndex: Int? = nil) {
        for (id, group) in groupViews {
            if id == appId {
                if let idx = pageIndex {
                    group.setActiveHighlight(pageIndex: idx)
                } else {
                    if let tabBar = getTabBar(appId) {
                        group.setActiveHighlight(pageIndex: Int(tabBar.selected_index))
                    }
                }
            } else {
                group.clearHighlight()
            }
        }
    }

    @objc private func toggleClicked() {
        onToggleRequested?()
    }
}

/// NSView subclass with flipped coordinate system (top-left origin)
@MainActor
private class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

#endif
